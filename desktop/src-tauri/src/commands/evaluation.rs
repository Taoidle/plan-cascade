//! Evaluation Commands
//!
//! Tauri commands for managing evaluation runs: CRUD for evaluators,
//! creating evaluation runs, starting evaluations, and retrieving reports.

use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::agent_composer::eval_types::*;
use crate::services::agent_composer::evaluation::{ensure_evaluation_tables, load_reports, persist_report};
use crate::state::AppState;
use crate::utils::error::{AppError, AppResult};

/// List all evaluators.
#[tauri::command]
pub async fn list_evaluators(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<EvaluatorInfo>>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_evaluation_tables(&conn)?;

            let mut stmt = conn.prepare(
                "SELECT id, name, definition FROM evaluators ORDER BY name ASC",
            )?;

            let evaluators: Vec<EvaluatorInfo> = stmt
                .query_map([], |row| {
                    let id: String = row.get(0)?;
                    let name: String = row.get(1)?;
                    let definition_json: String = row.get(2)?;

                    let (has_tt, has_rs, has_lj) =
                        if let Ok(eval) = serde_json::from_str::<Evaluator>(&definition_json) {
                            (
                                eval.criteria.tool_trajectory.is_some(),
                                eval.criteria.response_similarity.is_some(),
                                eval.criteria.llm_judge.is_some(),
                            )
                        } else {
                            (false, false, false)
                        };

                    Ok(EvaluatorInfo {
                        id,
                        name,
                        has_tool_trajectory: has_tt,
                        has_response_similarity: has_rs,
                        has_llm_judge: has_lj,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(evaluators)
        })
        .await;

    match result {
        Ok(evaluators) => Ok(CommandResponse::ok(evaluators)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a new evaluator.
#[tauri::command]
pub async fn create_evaluator(
    state: State<'_, AppState>,
    evaluator: Evaluator,
) -> Result<CommandResponse<Evaluator>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_evaluation_tables(&conn)?;

            let id = if evaluator.id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                evaluator.id.clone()
            };

            let saved = Evaluator {
                id: id.clone(),
                ..evaluator
            };
            let definition_json = serde_json::to_string(&saved)?;

            conn.execute(
                "INSERT INTO evaluators (id, name, definition, created_at, updated_at)
                 VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                rusqlite::params![saved.id, saved.name, definition_json],
            )?;

            Ok(saved)
        })
        .await;

    match result {
        Ok(evaluator) => Ok(CommandResponse::ok(evaluator)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete an evaluator by ID.
#[tauri::command]
pub async fn delete_evaluator(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResponse<bool>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_evaluation_tables(&conn)?;

            let deleted = conn.execute(
                "DELETE FROM evaluators WHERE id = ?1",
                rusqlite::params![id],
            )?;

            Ok(deleted > 0)
        })
        .await;

    match result {
        Ok(deleted) => Ok(CommandResponse::ok(deleted)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a new evaluation run.
#[tauri::command]
pub async fn create_evaluation_run(
    state: State<'_, AppState>,
    run: EvaluationRun,
) -> Result<CommandResponse<EvaluationRun>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_evaluation_tables(&conn)?;

            let id = if run.id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                run.id.clone()
            };

            let now = chrono::Utc::now().to_rfc3339();
            let saved = EvaluationRun {
                id: id.clone(),
                status: "pending".to_string(),
                created_at: now.clone(),
                ..run
            };
            let definition_json = serde_json::to_string(&saved)?;

            conn.execute(
                "INSERT INTO evaluation_runs (id, evaluator_id, definition, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'pending', ?4, ?4)",
                rusqlite::params![saved.id, saved.evaluator_id, definition_json, now],
            )?;

            Ok(saved)
        })
        .await;

    match result {
        Ok(run) => Ok(CommandResponse::ok(run)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List all evaluation runs.
#[tauri::command]
pub async fn list_evaluation_runs(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<EvaluationRunInfo>>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_evaluation_tables(&conn)?;

            let mut stmt = conn.prepare(
                "SELECT id, evaluator_id, definition, status, created_at
                 FROM evaluation_runs ORDER BY created_at DESC",
            )?;

            let runs: Vec<EvaluationRunInfo> = stmt
                .query_map([], |row| {
                    let id: String = row.get(0)?;
                    let evaluator_id: String = row.get(1)?;
                    let definition_json: String = row.get(2)?;
                    let status: String = row.get(3)?;
                    let created_at: String = row.get(4)?;

                    let (model_count, case_count) =
                        if let Ok(run) = serde_json::from_str::<EvaluationRun>(&definition_json) {
                            (run.models.len(), run.cases.len())
                        } else {
                            (0, 0)
                        };

                    Ok(EvaluationRunInfo {
                        id,
                        evaluator_id,
                        model_count,
                        case_count,
                        status,
                        created_at,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(runs)
        })
        .await;

    match result {
        Ok(runs) => Ok(CommandResponse::ok(runs)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get evaluation reports for a run.
#[tauri::command]
pub async fn get_evaluation_reports(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<CommandResponse<Vec<EvaluationReport>>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_evaluation_tables(&conn)?;
            load_reports(&conn, &run_id)
        })
        .await;

    match result {
        Ok(reports) => Ok(CommandResponse::ok(reports)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete an evaluation run and its reports.
#[tauri::command]
pub async fn delete_evaluation_run(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<CommandResponse<bool>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_evaluation_tables(&conn)?;

            // Delete reports first (due to FK)
            conn.execute(
                "DELETE FROM evaluation_reports WHERE run_id = ?1",
                rusqlite::params![run_id],
            )?;

            let deleted = conn.execute(
                "DELETE FROM evaluation_runs WHERE id = ?1",
                rusqlite::params![run_id],
            )?;

            Ok(deleted > 0)
        })
        .await;

    match result {
        Ok(deleted) => Ok(CommandResponse::ok(deleted)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::eval_types::*;
    use crate::services::agent_composer::evaluation::ensure_evaluation_tables;
    use crate::storage::database::Database;

    #[test]
    fn test_evaluator_crud_in_database() {
        let db = Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();
        ensure_evaluation_tables(&conn).unwrap();

        let evaluator = Evaluator {
            id: "eval-1".to_string(),
            name: "Quality Check".to_string(),
            criteria: EvaluationCriteria {
                tool_trajectory: Some(ToolTrajectoryConfig {
                    expected_tools: vec!["read_file".to_string()],
                    order_matters: false,
                }),
                response_similarity: None,
                llm_judge: None,
            },
        };

        let definition_json = serde_json::to_string(&evaluator).unwrap();

        // Create
        conn.execute(
            "INSERT INTO evaluators (id, name, definition)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![evaluator.id, evaluator.name, definition_json],
        )
        .unwrap();

        // Read
        let loaded_json: String = conn
            .query_row(
                "SELECT definition FROM evaluators WHERE id = 'eval-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let loaded: Evaluator = serde_json::from_str(&loaded_json).unwrap();
        assert_eq!(loaded.name, "Quality Check");
        assert!(loaded.criteria.tool_trajectory.is_some());

        // Delete
        conn.execute("DELETE FROM evaluators WHERE id = 'eval-1'", [])
            .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evaluators WHERE id = 'eval-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_evaluation_run_crud_in_database() {
        let db = Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();
        ensure_evaluation_tables(&conn).unwrap();

        let run = EvaluationRun {
            id: "run-1".to_string(),
            evaluator_id: "eval-1".to_string(),
            models: vec![ModelConfig {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
                display_name: None,
            }],
            cases: vec![],
            status: "pending".to_string(),
            created_at: "2026-02-17T00:00:00Z".to_string(),
        };

        let definition_json = serde_json::to_string(&run).unwrap();

        // Create
        conn.execute(
            "INSERT INTO evaluation_runs (id, evaluator_id, definition, status)
             VALUES (?1, ?2, ?3, 'pending')",
            rusqlite::params![run.id, run.evaluator_id, definition_json],
        )
        .unwrap();

        // Read
        let status: String = conn
            .query_row(
                "SELECT status FROM evaluation_runs WHERE id = 'run-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "pending");

        // Delete
        conn.execute("DELETE FROM evaluation_runs WHERE id = 'run-1'", [])
            .unwrap();
    }
}
