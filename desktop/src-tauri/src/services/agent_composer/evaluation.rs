//! Evaluation Engine
//!
//! Provides multi-model evaluation for agent pipelines with three
//! scoring criteria:
//! - Tool trajectory: Compare tool calls against expected tools
//! - Response similarity: String distance comparison
//! - LLM judge: Use a separate model to score the response
//!
//! Results are persisted to SQLite as `EvaluationReport` records.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use futures_util::StreamExt;

use super::eval_types::*;
use super::llm_agent::LlmAgent;
use super::types::{Agent, AgentConfig, AgentContext, AgentEvent, AgentInput};
use crate::storage::database::DbPool;
use crate::utils::error::{AppError, AppResult};

// ============================================================================
// Evaluation Engine
// ============================================================================

/// Engine for running multi-model evaluations.
pub struct EvaluationEngine {
    /// Database pool for persisting results.
    _pool: DbPool,
}

impl EvaluationEngine {
    /// Create a new EvaluationEngine with a database pool.
    pub fn new(pool: DbPool) -> Self {
        Self { _pool: pool }
    }

    /// Run an evaluation across all models and cases.
    ///
    /// For each model, for each case, builds an LlmAgent, runs it with the
    /// case input, collects events, and scores using the evaluator's criteria.
    pub async fn run_evaluation<F>(
        &self,
        evaluator: &Evaluator,
        run: &EvaluationRun,
        ctx_factory: F,
    ) -> AppResult<Vec<EvaluationReport>>
    where
        F: Fn(&ModelConfig) -> AgentContext,
    {
        let mut reports = Vec::new();

        for model_config in &run.models {
            let report = self
                .evaluate_model(evaluator, run, model_config, &ctx_factory)
                .await?;
            reports.push(report);
        }

        Ok(reports)
    }

    /// Evaluate a single model across all cases.
    async fn evaluate_model<F>(
        &self,
        evaluator: &Evaluator,
        run: &EvaluationRun,
        model_config: &ModelConfig,
        ctx_factory: &F,
    ) -> AppResult<EvaluationReport>
    where
        F: Fn(&ModelConfig) -> AgentContext,
    {
        let start = Instant::now();
        let mut results = Vec::new();
        let mut total_tokens: u64 = 0;

        for case in &run.cases {
            let ctx = ctx_factory(model_config);
            let result = self
                .evaluate_case(evaluator, case, model_config, ctx)
                .await?;
            results.push(result);
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // Calculate overall score as average of all case scores
        let overall_score = if results.is_empty() {
            0.0
        } else {
            results.iter().map(|r| r.score).sum::<f32>() / results.len() as f32
        };

        // Estimate cost (simplified - real implementation would use pricing data)
        let estimated_cost = total_tokens as f64 * 0.000003; // rough estimate

        Ok(EvaluationReport {
            run_id: run.id.clone(),
            model: model_config.model.clone(),
            provider: model_config.provider.clone(),
            results,
            overall_score,
            duration_ms,
            total_tokens,
            estimated_cost,
        })
    }

    /// Evaluate a single case with a single model.
    async fn evaluate_case(
        &self,
        evaluator: &Evaluator,
        case: &EvaluationCase,
        model_config: &ModelConfig,
        ctx: AgentContext,
    ) -> AppResult<TestResult> {
        // Build an LlmAgent for this model
        let agent = LlmAgent::new(format!("eval-{}", model_config.model))
            .with_model(model_config.model.clone())
            .with_config(AgentConfig {
                max_iterations: 10,
                streaming: false,
                ..Default::default()
            });

        let mut eval_ctx = ctx;
        eval_ctx.input = case.input.clone();

        // Run the agent and collect events
        let mut tool_calls = Vec::new();
        let mut response = String::new();

        match agent.run(eval_ctx).await {
            Ok(mut stream) => {
                while let Some(event_result) = stream.next().await {
                    match event_result {
                        Ok(event) => match &event {
                            AgentEvent::ToolCall { name, .. } => {
                                tool_calls.push(name.clone());
                            }
                            AgentEvent::TextDelta { content } => {
                                response.push_str(content);
                            }
                            AgentEvent::Done { output } => {
                                if let Some(out) = output {
                                    if response.is_empty() {
                                        response = out.clone();
                                    }
                                }
                            }
                            _ => {}
                        },
                        Err(_) => break,
                    }
                }
            }
            Err(e) => {
                return Ok(TestResult {
                    case_id: case.id.clone(),
                    passed: false,
                    score: 0.0,
                    details: format!("Agent execution failed: {}", e),
                    tool_calls: vec![],
                    response: String::new(),
                });
            }
        }

        // Score using all enabled criteria
        let mut scores = Vec::new();
        let mut details_parts = Vec::new();

        // Tool trajectory scoring
        if let Some(ref traj_config) = evaluator.criteria.tool_trajectory {
            let score = score_tool_trajectory(&tool_calls, traj_config);
            scores.push(score);
            details_parts.push(format!("tool_trajectory: {:.2}", score));
        }

        // Response similarity scoring
        if let Some(ref sim_config) = evaluator.criteria.response_similarity {
            let score = score_response_similarity(&response, sim_config);
            scores.push(score);
            details_parts.push(format!("response_similarity: {:.2}", score));
        }

        // LLM judge scoring (simplified - would need actual LLM call)
        if let Some(ref _judge_config) = evaluator.criteria.llm_judge {
            // In a full implementation, this would call the judge model.
            // For now, we skip LLM judge in tests and return a neutral score.
            let score = 0.5_f32;
            scores.push(score);
            details_parts.push(format!("llm_judge: {:.2} (placeholder)", score));
        }

        let overall = if scores.is_empty() {
            0.0
        } else {
            scores.iter().sum::<f32>() / scores.len() as f32
        };

        let passed = overall >= 0.5;

        Ok(TestResult {
            case_id: case.id.clone(),
            passed,
            score: overall,
            details: details_parts.join(", "),
            tool_calls,
            response,
        })
    }
}

// ============================================================================
// Scoring Functions
// ============================================================================

/// Score tool trajectory by comparing actual tool calls against expected.
pub fn score_tool_trajectory(
    actual_tools: &[String],
    config: &ToolTrajectoryConfig,
) -> f32 {
    if config.expected_tools.is_empty() {
        return 1.0;
    }

    if config.order_matters {
        // Check sequence match
        let mut score = 0.0_f32;
        let max_len = config.expected_tools.len().max(actual_tools.len());
        if max_len == 0 {
            return 1.0;
        }

        let min_len = config.expected_tools.len().min(actual_tools.len());
        let mut matched = 0;
        for i in 0..min_len {
            if config.expected_tools[i] == actual_tools[i] {
                matched += 1;
            }
        }
        score = matched as f32 / config.expected_tools.len() as f32;
        score
    } else {
        // Check set match (order doesn't matter)
        let expected_set: std::collections::HashSet<&str> =
            config.expected_tools.iter().map(|s| s.as_str()).collect();
        let actual_set: std::collections::HashSet<&str> =
            actual_tools.iter().map(|s| s.as_str()).collect();

        let matched = expected_set.intersection(&actual_set).count();
        matched as f32 / config.expected_tools.len() as f32
    }
}

/// Score response similarity using Levenshtein distance.
pub fn score_response_similarity(
    response: &str,
    config: &ResponseSimilarityConfig,
) -> f32 {
    let reference = &config.reference_response;
    let distance = levenshtein_distance(response, reference);
    let max_len = response.len().max(reference.len());

    if max_len == 0 {
        return 1.0;
    }

    let similarity = 1.0 - (distance as f32 / max_len as f32);
    similarity.max(0.0)
}

/// Simple Levenshtein distance implementation.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

// ============================================================================
// Database Persistence
// ============================================================================

/// Ensure evaluation tables exist in the database.
pub fn ensure_evaluation_tables(
    conn: &r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
) -> AppResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS evaluation_runs (
            id TEXT PRIMARY KEY,
            evaluator_id TEXT NOT NULL,
            definition TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS evaluation_reports (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            model TEXT NOT NULL,
            provider TEXT NOT NULL,
            overall_score REAL NOT NULL DEFAULT 0.0,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            estimated_cost REAL NOT NULL DEFAULT 0.0,
            results_json TEXT NOT NULL DEFAULT '[]',
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (run_id) REFERENCES evaluation_runs(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS evaluators (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            definition TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_evaluation_reports_run_id
         ON evaluation_reports(run_id)",
        [],
    )?;

    Ok(())
}

/// Persist an evaluation report to the database.
pub fn persist_report(
    conn: &r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
    report: &EvaluationReport,
) -> AppResult<()> {
    let results_json = serde_json::to_string(&report.results)?;
    let report_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO evaluation_reports (id, run_id, model, provider, overall_score, duration_ms, total_tokens, estimated_cost, results_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            report_id,
            report.run_id,
            report.model,
            report.provider,
            report.overall_score,
            report.duration_ms,
            report.total_tokens,
            report.estimated_cost,
            results_json,
        ],
    )?;

    Ok(())
}

/// Load evaluation reports for a run from the database.
pub fn load_reports(
    conn: &r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
    run_id: &str,
) -> AppResult<Vec<EvaluationReport>> {
    let mut stmt = conn.prepare(
        "SELECT run_id, model, provider, overall_score, duration_ms, total_tokens, estimated_cost, results_json
         FROM evaluation_reports WHERE run_id = ?1",
    )?;

    let reports = stmt
        .query_map(rusqlite::params![run_id], |row| {
            let results_json: String = row.get(7)?;
            let results: Vec<TestResult> =
                serde_json::from_str(&results_json).unwrap_or_default();

            Ok(EvaluationReport {
                run_id: row.get(0)?,
                model: row.get(1)?,
                provider: row.get(2)?,
                overall_score: row.get(3)?,
                duration_ms: row.get(4)?,
                total_tokens: row.get(5)?,
                estimated_cost: row.get(6)?,
                results,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    // ========================================================================
    // Tool Trajectory Scoring Tests
    // ========================================================================

    #[test]
    fn test_score_tool_trajectory_exact_match_ordered() {
        let actual = vec!["read_file".to_string(), "grep".to_string()];
        let config = ToolTrajectoryConfig {
            expected_tools: vec!["read_file".to_string(), "grep".to_string()],
            order_matters: true,
        };
        let score = score_tool_trajectory(&actual, &config);
        assert!((score - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_score_tool_trajectory_partial_match_ordered() {
        let actual = vec!["read_file".to_string(), "write_file".to_string()];
        let config = ToolTrajectoryConfig {
            expected_tools: vec!["read_file".to_string(), "grep".to_string()],
            order_matters: true,
        };
        let score = score_tool_trajectory(&actual, &config);
        assert!((score - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_score_tool_trajectory_no_match_ordered() {
        let actual = vec!["write_file".to_string()];
        let config = ToolTrajectoryConfig {
            expected_tools: vec!["read_file".to_string(), "grep".to_string()],
            order_matters: true,
        };
        let score = score_tool_trajectory(&actual, &config);
        assert!((score - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_score_tool_trajectory_unordered() {
        let actual = vec!["grep".to_string(), "read_file".to_string()];
        let config = ToolTrajectoryConfig {
            expected_tools: vec!["read_file".to_string(), "grep".to_string()],
            order_matters: false,
        };
        let score = score_tool_trajectory(&actual, &config);
        assert!((score - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_score_tool_trajectory_partial_unordered() {
        let actual = vec!["grep".to_string()];
        let config = ToolTrajectoryConfig {
            expected_tools: vec!["read_file".to_string(), "grep".to_string()],
            order_matters: false,
        };
        let score = score_tool_trajectory(&actual, &config);
        assert!((score - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_score_tool_trajectory_empty_expected() {
        let actual = vec!["grep".to_string()];
        let config = ToolTrajectoryConfig {
            expected_tools: vec![],
            order_matters: false,
        };
        let score = score_tool_trajectory(&actual, &config);
        assert!((score - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_score_tool_trajectory_empty_actual() {
        let actual: Vec<String> = vec![];
        let config = ToolTrajectoryConfig {
            expected_tools: vec!["read_file".to_string()],
            order_matters: false,
        };
        let score = score_tool_trajectory(&actual, &config);
        assert!((score - 0.0).abs() < f32::EPSILON);
    }

    // ========================================================================
    // Response Similarity Scoring Tests
    // ========================================================================

    #[test]
    fn test_score_response_similarity_exact_match() {
        let config = ResponseSimilarityConfig {
            reference_response: "Hello, world!".to_string(),
            threshold: 0.8,
        };
        let score = score_response_similarity("Hello, world!", &config);
        assert!((score - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_score_response_similarity_partial_match() {
        let config = ResponseSimilarityConfig {
            reference_response: "Hello, world!".to_string(),
            threshold: 0.8,
        };
        let score = score_response_similarity("Hello, World!", &config);
        // One character difference
        assert!(score > 0.9);
        assert!(score < 1.0);
    }

    #[test]
    fn test_score_response_similarity_completely_different() {
        let config = ResponseSimilarityConfig {
            reference_response: "AAAA".to_string(),
            threshold: 0.8,
        };
        let score = score_response_similarity("BBBB", &config);
        assert!(score < 0.5);
    }

    #[test]
    fn test_score_response_similarity_empty_both() {
        let config = ResponseSimilarityConfig {
            reference_response: String::new(),
            threshold: 0.8,
        };
        let score = score_response_similarity("", &config);
        assert!((score - 1.0).abs() < f32::EPSILON);
    }

    // ========================================================================
    // Levenshtein Distance Tests
    // ========================================================================

    #[test]
    fn test_levenshtein_same_strings() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_one_edit() {
        assert_eq!(levenshtein_distance("hello", "hallo"), 1);
    }

    #[test]
    fn test_levenshtein_empty_first() {
        assert_eq!(levenshtein_distance("", "abc"), 3);
    }

    #[test]
    fn test_levenshtein_empty_second() {
        assert_eq!(levenshtein_distance("abc", ""), 3);
    }

    #[test]
    fn test_levenshtein_completely_different() {
        assert_eq!(levenshtein_distance("abc", "xyz"), 3);
    }

    // ========================================================================
    // Database Persistence Tests
    // ========================================================================

    #[test]
    fn test_ensure_evaluation_tables() {
        let db = Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();
        ensure_evaluation_tables(&conn).unwrap();

        // Verify tables exist
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='evaluation_runs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='evaluation_reports'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='evaluators'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_ensure_evaluation_tables_idempotent() {
        let db = Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();
        ensure_evaluation_tables(&conn).unwrap();
        ensure_evaluation_tables(&conn).unwrap(); // Should not fail
    }

    #[test]
    fn test_persist_and_load_report() {
        let db = Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();
        ensure_evaluation_tables(&conn).unwrap();

        // Insert a run first (FK constraint)
        conn.execute(
            "INSERT INTO evaluation_runs (id, evaluator_id, definition, status)
             VALUES ('run-1', 'eval-1', '{}', 'completed')",
            [],
        )
        .unwrap();

        let report = EvaluationReport {
            run_id: "run-1".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            provider: "anthropic".to_string(),
            results: vec![TestResult {
                case_id: "case-1".to_string(),
                passed: true,
                score: 0.95,
                details: "Good".to_string(),
                tool_calls: vec!["read_file".to_string()],
                response: "Hello!".to_string(),
            }],
            overall_score: 0.95,
            duration_ms: 1500,
            total_tokens: 500,
            estimated_cost: 0.003,
        };

        persist_report(&conn, &report).unwrap();

        let reports = load_reports(&conn, "run-1").unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].model, "claude-sonnet-4-20250514");
        assert!((reports[0].overall_score - 0.95).abs() < f32::EPSILON);
        assert_eq!(reports[0].results.len(), 1);
        assert_eq!(reports[0].results[0].case_id, "case-1");
    }

    #[test]
    fn test_load_reports_empty() {
        let db = Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();
        ensure_evaluation_tables(&conn).unwrap();

        let reports = load_reports(&conn, "nonexistent").unwrap();
        assert!(reports.is_empty());
    }

    #[test]
    fn test_persist_multiple_reports() {
        let db = Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();
        ensure_evaluation_tables(&conn).unwrap();

        conn.execute(
            "INSERT INTO evaluation_runs (id, evaluator_id, definition, status)
             VALUES ('run-2', 'eval-1', '{}', 'completed')",
            [],
        )
        .unwrap();

        for (model, score) in [("model-a", 0.9_f32), ("model-b", 0.7_f32)] {
            let report = EvaluationReport {
                run_id: "run-2".to_string(),
                model: model.to_string(),
                provider: "test".to_string(),
                results: vec![],
                overall_score: score,
                duration_ms: 1000,
                total_tokens: 100,
                estimated_cost: 0.001,
            };
            persist_report(&conn, &report).unwrap();
        }

        let reports = load_reports(&conn, "run-2").unwrap();
        assert_eq!(reports.len(), 2);
    }
}
