//! Evaluation Framework Types
//!
//! Defines the data structures for multi-model evaluation:
//! - `Evaluator`: Named evaluation configuration with criteria
//! - `EvaluationCriteria`: Tool trajectory, response similarity, LLM judge
//! - `EvaluationCase`: Test case with input and expected outputs
//! - `EvaluationRun`: Execution of an evaluator across multiple models
//! - `EvaluationReport`: Results from evaluating one model
//! - `TestResult`: Per-case scoring details

use serde::{Deserialize, Serialize};

use super::types::AgentInput;

// ============================================================================
// Evaluator
// ============================================================================

/// An evaluator configuration defining how to assess agent performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluator {
    /// Unique evaluator identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Evaluation criteria configuration.
    pub criteria: EvaluationCriteria,
}

/// Criteria for evaluating agent performance.
///
/// Each criterion is optional; only enabled criteria contribute to scoring.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvaluationCriteria {
    /// Tool trajectory comparison configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_trajectory: Option<ToolTrajectoryConfig>,
    /// Response similarity comparison configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_similarity: Option<ResponseSimilarityConfig>,
    /// LLM judge evaluation configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_judge: Option<LlmJudgeConfig>,
}

// ============================================================================
// Evaluation Criteria Configs
// ============================================================================

/// Configuration for tool trajectory evaluation.
///
/// Compares the tools called by the agent against expected tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTrajectoryConfig {
    /// Expected tools that should be called.
    pub expected_tools: Vec<String>,
    /// Whether the order of tool calls matters.
    #[serde(default)]
    pub order_matters: bool,
}

/// Configuration for response similarity evaluation.
///
/// Compares the agent's response against a reference response using
/// string distance metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseSimilarityConfig {
    /// The reference response to compare against.
    pub reference_response: String,
    /// Similarity threshold (0.0-1.0) for passing.
    pub threshold: f32,
}

/// Configuration for LLM judge evaluation.
///
/// Uses a separate LLM to judge the quality of the agent's response
/// based on a rubric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmJudgeConfig {
    /// The model to use as judge.
    pub judge_model: String,
    /// The provider for the judge model.
    pub judge_provider: String,
    /// Rubric describing how to evaluate the response.
    pub rubric: String,
}

// ============================================================================
// Evaluation Case
// ============================================================================

/// A test case for evaluation.
///
/// Defines the input to give the agent and optional expected outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationCase {
    /// Unique case identifier.
    pub id: String,
    /// Human-readable case name.
    pub name: String,
    /// Input to provide to the agent.
    pub input: AgentInput,
    /// Optional expected output for comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output: Option<String>,
    /// Optional expected tool calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_tools: Option<Vec<String>>,
}

// ============================================================================
// Model Config
// ============================================================================

/// Configuration for a model to evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Provider name (e.g., "anthropic", "openai").
    pub provider: String,
    /// Model identifier (e.g., "claude-sonnet-4-20250514").
    pub model: String,
    /// Optional display name for UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

// ============================================================================
// Evaluation Run
// ============================================================================

/// An evaluation run executing cases across multiple models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRun {
    /// Unique run identifier.
    pub id: String,
    /// ID of the evaluator to use.
    pub evaluator_id: String,
    /// Models to evaluate.
    pub models: Vec<ModelConfig>,
    /// Test cases to run.
    pub cases: Vec<EvaluationCase>,
    /// Current status ("pending", "running", "completed", "failed").
    pub status: String,
    /// When the run was created (ISO 8601).
    pub created_at: String,
}

/// Summary information about an evaluation run (for list views).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRunInfo {
    /// Run identifier.
    pub id: String,
    /// Evaluator ID used.
    pub evaluator_id: String,
    /// Number of models.
    pub model_count: usize,
    /// Number of cases.
    pub case_count: usize,
    /// Current status.
    pub status: String,
    /// When created.
    pub created_at: String,
}

/// Summary information about an evaluator (for list views).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorInfo {
    /// Evaluator identifier.
    pub id: String,
    /// Evaluator name.
    pub name: String,
    /// Whether tool trajectory is enabled.
    pub has_tool_trajectory: bool,
    /// Whether response similarity is enabled.
    pub has_response_similarity: bool,
    /// Whether LLM judge is enabled.
    pub has_llm_judge: bool,
}

// ============================================================================
// Evaluation Report
// ============================================================================

/// Results from evaluating a single model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationReport {
    /// Evaluation run ID.
    pub run_id: String,
    /// Model that was evaluated.
    pub model: String,
    /// Provider used.
    pub provider: String,
    /// Per-case results.
    pub results: Vec<TestResult>,
    /// Overall score (0.0-1.0).
    pub overall_score: f32,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Total tokens consumed.
    pub total_tokens: u64,
    /// Estimated cost in USD.
    pub estimated_cost: f64,
}

/// Result for a single test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Case identifier.
    pub case_id: String,
    /// Whether the case passed.
    pub passed: bool,
    /// Score for this case (0.0-1.0).
    pub score: f32,
    /// Details about the scoring.
    pub details: String,
    /// Tool calls made by the agent.
    pub tool_calls: Vec<String>,
    /// Agent's response text.
    pub response: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluator_serialization_roundtrip() {
        let evaluator = Evaluator {
            id: "eval-1".to_string(),
            name: "Code Quality".to_string(),
            criteria: EvaluationCriteria {
                tool_trajectory: Some(ToolTrajectoryConfig {
                    expected_tools: vec!["read_file".to_string(), "grep".to_string()],
                    order_matters: true,
                }),
                response_similarity: None,
                llm_judge: Some(LlmJudgeConfig {
                    judge_model: "claude-sonnet-4-20250514".to_string(),
                    judge_provider: "anthropic".to_string(),
                    rubric: "Rate code quality 0-1".to_string(),
                }),
            },
        };

        let json = serde_json::to_string_pretty(&evaluator).unwrap();
        let parsed: Evaluator = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "eval-1");
        assert_eq!(parsed.name, "Code Quality");
        assert!(parsed.criteria.tool_trajectory.is_some());
        assert!(parsed.criteria.response_similarity.is_none());
        assert!(parsed.criteria.llm_judge.is_some());
    }

    #[test]
    fn test_evaluation_criteria_all_none() {
        let criteria = EvaluationCriteria::default();
        assert!(criteria.tool_trajectory.is_none());
        assert!(criteria.response_similarity.is_none());
        assert!(criteria.llm_judge.is_none());

        let json = serde_json::to_string(&criteria).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_tool_trajectory_config_serialization() {
        let config = ToolTrajectoryConfig {
            expected_tools: vec!["tool_a".to_string(), "tool_b".to_string()],
            order_matters: false,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ToolTrajectoryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.expected_tools.len(), 2);
        assert!(!parsed.order_matters);
    }

    #[test]
    fn test_response_similarity_config_serialization() {
        let config = ResponseSimilarityConfig {
            reference_response: "Hello, world!".to_string(),
            threshold: 0.8,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ResponseSimilarityConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.reference_response, "Hello, world!");
        assert!((parsed.threshold - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_llm_judge_config_serialization() {
        let config = LlmJudgeConfig {
            judge_model: "gpt-4".to_string(),
            judge_provider: "openai".to_string(),
            rubric: "Is the response helpful?".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: LlmJudgeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.judge_model, "gpt-4");
        assert_eq!(parsed.judge_provider, "openai");
    }

    #[test]
    fn test_evaluation_case_serialization() {
        let case = EvaluationCase {
            id: "case-1".to_string(),
            name: "Basic greeting".to_string(),
            input: AgentInput::Structured(serde_json::json!({"prompt": "Say hello"})),
            expected_output: Some("Hello!".to_string()),
            expected_tools: Some(vec!["greet".to_string()]),
        };

        let json = serde_json::to_string(&case).unwrap();
        let parsed: EvaluationCase = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "case-1");
        assert_eq!(parsed.name, "Basic greeting");
        assert_eq!(parsed.expected_output, Some("Hello!".to_string()));
    }

    #[test]
    fn test_evaluation_case_minimal() {
        let case = EvaluationCase {
            id: "case-2".to_string(),
            name: "Minimal".to_string(),
            input: AgentInput::Structured(serde_json::json!({"prompt": "test"})),
            expected_output: None,
            expected_tools: None,
        };

        let json = serde_json::to_string(&case).unwrap();
        assert!(!json.contains("expected_output"));
        assert!(!json.contains("expected_tools"));
    }

    #[test]
    fn test_model_config_serialization() {
        let config = ModelConfig {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            display_name: Some("Claude 3.5 Sonnet".to_string()),
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: ModelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.provider, "anthropic");
        assert_eq!(parsed.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_evaluation_run_serialization() {
        let run = EvaluationRun {
            id: "run-1".to_string(),
            evaluator_id: "eval-1".to_string(),
            models: vec![ModelConfig {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
                display_name: None,
            }],
            cases: vec![EvaluationCase {
                id: "case-1".to_string(),
                name: "Test".to_string(),
                input: AgentInput::Structured(serde_json::json!({"prompt": "hello"})),
                expected_output: None,
                expected_tools: None,
            }],
            status: "pending".to_string(),
            created_at: "2026-02-17T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&run).unwrap();
        let parsed: EvaluationRun = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "run-1");
        assert_eq!(parsed.models.len(), 1);
        assert_eq!(parsed.cases.len(), 1);
        assert_eq!(parsed.status, "pending");
    }

    #[test]
    fn test_evaluation_report_serialization() {
        let report = EvaluationReport {
            run_id: "run-1".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            provider: "anthropic".to_string(),
            results: vec![TestResult {
                case_id: "case-1".to_string(),
                passed: true,
                score: 0.95,
                details: "Good response".to_string(),
                tool_calls: vec!["read_file".to_string()],
                response: "Hello!".to_string(),
            }],
            overall_score: 0.95,
            duration_ms: 1500,
            total_tokens: 500,
            estimated_cost: 0.003,
        };

        let json = serde_json::to_string_pretty(&report).unwrap();
        let parsed: EvaluationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.run_id, "run-1");
        assert!((parsed.overall_score - 0.95).abs() < f32::EPSILON);
        assert_eq!(parsed.results.len(), 1);
        assert!(parsed.results[0].passed);
        assert_eq!(parsed.duration_ms, 1500);
        assert_eq!(parsed.total_tokens, 500);
    }

    #[test]
    fn test_test_result_serialization() {
        let result = TestResult {
            case_id: "c1".to_string(),
            passed: false,
            score: 0.3,
            details: "Missing expected tools".to_string(),
            tool_calls: vec![],
            response: "Partial response".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: TestResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.case_id, "c1");
        assert!(!parsed.passed);
        assert!((parsed.score - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_evaluation_run_info_serialization() {
        let info = EvaluationRunInfo {
            id: "run-1".to_string(),
            evaluator_id: "eval-1".to_string(),
            model_count: 3,
            case_count: 10,
            status: "completed".to_string(),
            created_at: "2026-02-17T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: EvaluationRunInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "run-1");
        assert_eq!(parsed.model_count, 3);
        assert_eq!(parsed.case_count, 10);
    }

    #[test]
    fn test_evaluator_info_serialization() {
        let info = EvaluatorInfo {
            id: "eval-1".to_string(),
            name: "Quality Check".to_string(),
            has_tool_trajectory: true,
            has_response_similarity: false,
            has_llm_judge: true,
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: EvaluatorInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "eval-1");
        assert!(parsed.has_tool_trajectory);
        assert!(!parsed.has_response_similarity);
        assert!(parsed.has_llm_judge);
    }
}
