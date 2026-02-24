//! Agent Composer Commands
//!
//! Tauri commands for managing and executing composable agent pipelines.
//! Supports CRUD operations on AgentPipeline definitions persisted in SQLite,
//! and execution of pipelines via the ComposerRegistry.

use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::agent_composer::{AgentPipeline, AgentPipelineInfo};
use crate::state::AppState;
use crate::utils::error::{AppError, AppResult};

/// List all saved agent pipelines (summary info only).
#[tauri::command]
pub async fn list_agent_pipelines(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<AgentPipelineInfo>>, String> {
    let result = state
        .with_database(|db| {
            let conn = db
                .pool()
                .get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            ensure_agent_pipelines_table(&conn)?;

            let mut stmt = conn.prepare(
                "SELECT id, name, description, definition, created_at, updated_at
                 FROM agent_pipelines ORDER BY name ASC",
            )?;

            let pipelines: Vec<AgentPipelineInfo> = stmt
                .query_map([], |row| {
                    let definition_json: String = row.get(3)?;
                    let pipeline: AgentPipeline = serde_json::from_str(&definition_json)
                        .unwrap_or_else(|_| AgentPipeline {
                            pipeline_id: row.get::<_, String>(0).unwrap_or_default(),
                            name: row.get::<_, String>(1).unwrap_or_default(),
                            description: row.get::<_, Option<String>>(2).ok().flatten(),
                            steps: vec![],
                            created_at: row.get::<_, String>(4).unwrap_or_default(),
                            updated_at: row.get::<_, Option<String>>(5).ok().flatten(),
                        });
                    Ok(AgentPipelineInfo::from(&pipeline))
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(pipelines)
        })
        .await;

    match result {
        Ok(pipelines) => Ok(CommandResponse::ok(pipelines)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get a single agent pipeline by ID.
#[tauri::command]
pub async fn get_agent_pipeline(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResponse<Option<AgentPipeline>>, String> {
    let result = state
        .with_database(|db| {
            let conn = db
                .pool()
                .get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            ensure_agent_pipelines_table(&conn)?;

            let pipeline_result = conn.query_row(
                "SELECT definition FROM agent_pipelines WHERE id = ?1",
                rusqlite::params![id],
                |row| {
                    let json: String = row.get(0)?;
                    Ok(json)
                },
            );

            match pipeline_result {
                Ok(json) => {
                    let pipeline: AgentPipeline = serde_json::from_str(&json)
                        .map_err(|e| AppError::parse(format!("Failed to parse pipeline: {}", e)))?;
                    Ok(Some(pipeline))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(AppError::database(e.to_string())),
            }
        })
        .await;

    match result {
        Ok(pipeline) => Ok(CommandResponse::ok(pipeline)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a new agent pipeline.
#[tauri::command]
pub async fn create_agent_pipeline(
    state: State<'_, AppState>,
    pipeline: AgentPipeline,
) -> Result<CommandResponse<AgentPipeline>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_agent_pipelines_table(&conn)?;

            // Generate ID if not provided
            let pipeline_id = if pipeline.pipeline_id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                pipeline.pipeline_id.clone()
            };

            let now = chrono::Utc::now().to_rfc3339();
            let saved_pipeline = AgentPipeline {
                pipeline_id: pipeline_id.clone(),
                created_at: now.clone(),
                updated_at: Some(now.clone()),
                ..pipeline
            };

            let definition_json = serde_json::to_string(&saved_pipeline)?;

            conn.execute(
                "INSERT INTO agent_pipelines (id, name, description, definition, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    saved_pipeline.pipeline_id,
                    saved_pipeline.name,
                    saved_pipeline.description,
                    definition_json,
                    saved_pipeline.created_at,
                    saved_pipeline.updated_at,
                ],
            )?;

            Ok(saved_pipeline)
        })
        .await;

    match result {
        Ok(pipeline) => Ok(CommandResponse::ok(pipeline)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update an existing agent pipeline.
#[tauri::command]
pub async fn update_agent_pipeline(
    state: State<'_, AppState>,
    id: String,
    pipeline: AgentPipeline,
) -> Result<CommandResponse<AgentPipeline>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_agent_pipelines_table(&conn)?;

            // Verify it exists
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM agent_pipelines WHERE id = ?1",
                    rusqlite::params![id],
                    |row| row.get::<_, i64>(0),
                )
                .map(|count| count > 0)
                .unwrap_or(false);

            if !exists {
                return Err(AppError::not_found(format!(
                    "Pipeline not found: {}",
                    id
                )));
            }

            let now = chrono::Utc::now().to_rfc3339();
            let updated_pipeline = AgentPipeline {
                pipeline_id: id.clone(),
                updated_at: Some(now.clone()),
                ..pipeline
            };

            let definition_json = serde_json::to_string(&updated_pipeline)?;

            conn.execute(
                "UPDATE agent_pipelines SET name = ?2, description = ?3, definition = ?4, updated_at = ?5
                 WHERE id = ?1",
                rusqlite::params![
                    id,
                    updated_pipeline.name,
                    updated_pipeline.description,
                    definition_json,
                    updated_pipeline.updated_at,
                ],
            )?;

            Ok(updated_pipeline)
        })
        .await;

    match result {
        Ok(pipeline) => Ok(CommandResponse::ok(pipeline)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete an agent pipeline.
#[tauri::command]
pub async fn delete_agent_pipeline(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResponse<bool>, String> {
    let result = state
        .with_database(|db| {
            let conn = db
                .pool()
                .get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            ensure_agent_pipelines_table(&conn)?;

            let deleted = conn.execute(
                "DELETE FROM agent_pipelines WHERE id = ?1",
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

/// Ensure the agent_pipelines table exists.
fn ensure_agent_pipelines_table(
    conn: &r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
) -> AppResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_pipelines (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            definition TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::types::{AgentConfig, AgentStep, LlmStepConfig};

    #[test]
    fn test_pipeline_serialization_roundtrip() {
        let pipeline = AgentPipeline {
            pipeline_id: "test-1".to_string(),
            name: "Test Pipeline".to_string(),
            description: Some("A test pipeline".to_string()),
            steps: vec![AgentStep::LlmStep(LlmStepConfig {
                name: "agent-1".to_string(),
                instruction: Some("Be helpful".to_string()),
                model: None,
                tools: None,
                config: AgentConfig::default(),
            })],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: None,
        };

        let json = serde_json::to_string(&pipeline).unwrap();
        let parsed: AgentPipeline = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pipeline_id, "test-1");
        assert_eq!(parsed.name, "Test Pipeline");
        assert_eq!(parsed.steps.len(), 1);
    }

    #[test]
    fn test_pipeline_info_from_pipeline() {
        let pipeline = AgentPipeline {
            pipeline_id: "p-1".to_string(),
            name: "My Pipeline".to_string(),
            description: Some("desc".to_string()),
            steps: vec![AgentStep::LlmStep(LlmStepConfig {
                name: "s1".to_string(),
                instruction: None,
                model: None,
                tools: None,
                config: AgentConfig::default(),
            })],
            created_at: "2026-01-01".to_string(),
            updated_at: None,
        };

        let info = AgentPipelineInfo::from(&pipeline);
        assert_eq!(info.pipeline_id, "p-1");
        assert_eq!(info.step_count, 1);
    }
}
