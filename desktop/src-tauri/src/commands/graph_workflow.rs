//! Graph Workflow Commands
//!
//! Tauri commands for managing graph workflows: CRUD operations and execution.
//! Workflows are persisted in SQLite and executed via the graph execution engine.

use std::sync::Arc;

use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::agent_composer::graph_types::{GraphWorkflow, GraphWorkflowInfo};
use crate::state::AppState;
use crate::utils::error::{AppError, AppResult};

/// Ensure the graph_workflows table exists.
fn ensure_graph_workflows_table(
    conn: &r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
) -> AppResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS graph_workflows (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            definition TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;
    Ok(())
}

/// List all saved graph workflows (summary info only).
#[tauri::command]
pub async fn list_graph_workflows(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<GraphWorkflowInfo>>, String> {
    let result = state
        .with_database(|db| {
            let conn = db
                .pool()
                .get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            ensure_graph_workflows_table(&conn)?;

            let mut stmt =
                conn.prepare("SELECT id, name, definition FROM graph_workflows ORDER BY name ASC")?;

            let workflows: Vec<GraphWorkflowInfo> = stmt
                .query_map([], |row| {
                    let id: String = row.get(0)?;
                    let name: String = row.get(1)?;
                    let definition_json: String = row.get(2)?;

                    let (node_count, edge_count) =
                        if let Ok(wf) = serde_json::from_str::<GraphWorkflow>(&definition_json) {
                            (wf.nodes.len(), wf.edges.len())
                        } else {
                            (0, 0)
                        };

                    Ok(GraphWorkflowInfo {
                        id,
                        name,
                        node_count,
                        edge_count,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(workflows)
        })
        .await;

    match result {
        Ok(workflows) => Ok(CommandResponse::ok(workflows)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get a single graph workflow by ID.
#[tauri::command]
pub async fn get_graph_workflow(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResponse<Option<GraphWorkflow>>, String> {
    let result = state
        .with_database(|db| {
            let conn = db
                .pool()
                .get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            ensure_graph_workflows_table(&conn)?;

            let wf_result = conn.query_row(
                "SELECT definition FROM graph_workflows WHERE id = ?1",
                rusqlite::params![id],
                |row| {
                    let json: String = row.get(0)?;
                    Ok(json)
                },
            );

            match wf_result {
                Ok(json) => {
                    let workflow: GraphWorkflow = serde_json::from_str(&json)
                        .map_err(|e| AppError::parse(format!("Failed to parse workflow: {}", e)))?;
                    Ok(Some(workflow))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(AppError::database(e.to_string())),
            }
        })
        .await;

    match result {
        Ok(workflow) => Ok(CommandResponse::ok(workflow)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a new graph workflow.
#[tauri::command]
pub async fn create_graph_workflow(
    state: State<'_, AppState>,
    workflow: GraphWorkflow,
) -> Result<CommandResponse<GraphWorkflow>, String> {
    let result = state
        .with_database(|db| {
            let conn = db
                .pool()
                .get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            ensure_graph_workflows_table(&conn)?;

            let id = uuid::Uuid::new_v4().to_string();
            let definition_json = serde_json::to_string(&workflow)?;

            conn.execute(
                "INSERT INTO graph_workflows (id, name, definition, created_at, updated_at)
                 VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                rusqlite::params![id, workflow.name, definition_json],
            )?;

            Ok(workflow)
        })
        .await;

    match result {
        Ok(workflow) => Ok(CommandResponse::ok(workflow)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update an existing graph workflow.
#[tauri::command]
pub async fn update_graph_workflow(
    state: State<'_, AppState>,
    id: String,
    workflow: GraphWorkflow,
) -> Result<CommandResponse<GraphWorkflow>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_graph_workflows_table(&conn)?;

            // Verify it exists
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM graph_workflows WHERE id = ?1",
                    rusqlite::params![id],
                    |row| row.get::<_, i64>(0),
                )
                .map(|count| count > 0)
                .unwrap_or(false);

            if !exists {
                return Err(AppError::not_found(format!(
                    "Graph workflow not found: {}",
                    id
                )));
            }

            let definition_json = serde_json::to_string(&workflow)?;

            conn.execute(
                "UPDATE graph_workflows SET name = ?2, definition = ?3, updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?1",
                rusqlite::params![id, workflow.name, definition_json],
            )?;

            Ok(workflow)
        })
        .await;

    match result {
        Ok(workflow) => Ok(CommandResponse::ok(workflow)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Export a graph workflow in the specified format (json or mermaid).
#[tauri::command]
pub async fn export_graph_workflow(
    state: State<'_, AppState>,
    id: String,
    format: String,
) -> Result<CommandResponse<String>, String> {
    let result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            ensure_graph_workflows_table(&conn)?;

            let wf_result = conn.query_row(
                "SELECT definition FROM graph_workflows WHERE id = ?1",
                rusqlite::params![id],
                |row| {
                    let json: String = row.get(0)?;
                    Ok(json)
                },
            );

            match wf_result {
                Ok(json) => {
                    let workflow: GraphWorkflow = serde_json::from_str(&json)
                        .map_err(|e| AppError::parse(format!("Failed to parse workflow: {}", e)))?;

                    match format.as_str() {
                        "json" => {
                            let pretty = serde_json::to_string_pretty(&workflow)
                                .map_err(|e| AppError::parse(format!("Failed to serialize workflow: {}", e)))?;
                            Ok(pretty)
                        }
                        "mermaid" => {
                            Ok(generate_mermaid(&workflow))
                        }
                        "typescript" => {
                            Ok(crate::services::agent_composer::codegen::generate_typescript(&workflow))
                        }
                        "rust" => {
                            Ok(crate::services::agent_composer::codegen::generate_rust(&workflow))
                        }
                        _ => Err(AppError::validation(format!(
                            "Unsupported export format: {}. Supported: json, mermaid, typescript, rust",
                            format
                        ))),
                    }
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    Err(AppError::not_found(format!(
                        "Graph workflow not found: {}",
                        id
                    )))
                }
                Err(e) => Err(AppError::database(e.to_string())),
            }
        })
        .await;

    match result {
        Ok(exported) => Ok(CommandResponse::ok(exported)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Generate a Mermaid flowchart from a GraphWorkflow.
fn generate_mermaid(workflow: &GraphWorkflow) -> String {
    use crate::services::agent_composer::graph_types::Edge;

    let mut lines = Vec::new();
    lines.push("graph TD".to_string());

    // Emit nodes sorted by ID for deterministic output
    let mut node_ids: Vec<&String> = workflow.nodes.keys().collect();
    node_ids.sort();
    for node_id in node_ids {
        if let Some(node) = workflow.nodes.get(node_id) {
            let node_name = get_agent_step_name(&node.agent_step);
            lines.push(format!("  {}[\"{}\"]", node_id, node_name));
        }
    }

    // Emit edges
    for edge in &workflow.edges {
        match edge {
            Edge::Direct { from, to } => {
                lines.push(format!("  {} --> {}", from, to));
            }
            Edge::Conditional {
                from,
                condition: _,
                branches,
                default_branch: _,
            } => {
                // Sort branch keys for deterministic output
                let mut branch_keys: Vec<&String> = branches.keys().collect();
                branch_keys.sort();
                for key in branch_keys {
                    if let Some(target) = branches.get(key) {
                        lines.push(format!("  {} -->|{}| {}", from, key, target));
                    }
                }
            }
        }
    }

    lines.join("\n")
}

/// Extract the name from an AgentStep variant.
fn get_agent_step_name(step: &crate::services::agent_composer::types::AgentStep) -> String {
    use crate::services::agent_composer::types::AgentStep;
    match step {
        AgentStep::LlmStep(config) => config.name.clone(),
        AgentStep::SequentialStep { name, .. } => name.clone(),
        AgentStep::ParallelStep { name, .. } => name.clone(),
        AgentStep::ConditionalStep { name, .. } => name.clone(),
        AgentStep::LoopStep { name, .. } => name.clone(),
    }
}

/// Delete a graph workflow.
#[tauri::command]
pub async fn delete_graph_workflow(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResponse<bool>, String> {
    let result = state
        .with_database(|db| {
            let conn = db
                .pool()
                .get()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            ensure_graph_workflows_table(&conn)?;

            let deleted = conn.execute(
                "DELETE FROM graph_workflows WHERE id = ?1",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::graph_types::*;
    use crate::services::agent_composer::types::{AgentConfig, AgentStep, LlmStepConfig};
    use crate::storage::database::Database;
    use std::collections::HashMap;

    fn sample_workflow() -> GraphWorkflow {
        let mut nodes = HashMap::new();
        nodes.insert(
            "a".to_string(),
            GraphNode {
                id: "a".to_string(),
                agent_step: AgentStep::LlmStep(LlmStepConfig {
                    name: "agent-a".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );
        nodes.insert(
            "b".to_string(),
            GraphNode {
                id: "b".to_string(),
                agent_step: AgentStep::LlmStep(LlmStepConfig {
                    name: "agent-b".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );

        GraphWorkflow {
            name: "Test Flow".to_string(),
            description: Some("A test workflow".to_string()),
            nodes,
            edges: vec![Edge::Direct {
                from: "a".to_string(),
                to: "b".to_string(),
            }],
            entry_node: "a".to_string(),
            state_schema: StateSchema::default(),
        }
    }

    #[test]
    fn test_ensure_graph_workflows_table_creates_table() {
        let db = Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();
        ensure_graph_workflows_table(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='graph_workflows'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_ensure_graph_workflows_table_idempotent() {
        let db = Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();
        ensure_graph_workflows_table(&conn).unwrap();
        ensure_graph_workflows_table(&conn).unwrap(); // Should not fail
    }

    #[test]
    fn test_graph_workflow_crud() {
        let db = Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();
        ensure_graph_workflows_table(&conn).unwrap();

        let workflow = sample_workflow();
        let definition_json = serde_json::to_string(&workflow).unwrap();

        // Create
        conn.execute(
            "INSERT INTO graph_workflows (id, name, definition)
             VALUES ('wf-1', 'Test Flow', ?1)",
            rusqlite::params![definition_json],
        )
        .unwrap();

        // Read
        let loaded_json: String = conn
            .query_row(
                "SELECT definition FROM graph_workflows WHERE id = 'wf-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let loaded: GraphWorkflow = serde_json::from_str(&loaded_json).unwrap();
        assert_eq!(loaded.name, "Test Flow");
        assert_eq!(loaded.nodes.len(), 2);

        // Update
        let mut updated = loaded;
        updated.name = "Updated Flow".to_string();
        let updated_json = serde_json::to_string(&updated).unwrap();
        conn.execute(
            "UPDATE graph_workflows SET name = 'Updated Flow', definition = ?1 WHERE id = 'wf-1'",
            rusqlite::params![updated_json],
        )
        .unwrap();

        let name: String = conn
            .query_row(
                "SELECT name FROM graph_workflows WHERE id = 'wf-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "Updated Flow");

        // Delete
        conn.execute("DELETE FROM graph_workflows WHERE id = 'wf-1'", [])
            .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM graph_workflows WHERE id = 'wf-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_generate_mermaid_direct_edges() {
        let workflow = sample_workflow();
        let mermaid = generate_mermaid(&workflow);

        assert!(mermaid.starts_with("graph TD"));
        assert!(mermaid.contains("a[\"agent-a\"]"));
        assert!(mermaid.contains("b[\"agent-b\"]"));
        assert!(mermaid.contains("a --> b"));
    }

    #[test]
    fn test_generate_mermaid_conditional_edges() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "router".to_string(),
            GraphNode {
                id: "router".to_string(),
                agent_step: AgentStep::LlmStep(LlmStepConfig {
                    name: "Router Agent".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );
        nodes.insert(
            "yes_node".to_string(),
            GraphNode {
                id: "yes_node".to_string(),
                agent_step: AgentStep::LlmStep(LlmStepConfig {
                    name: "Yes Handler".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );
        nodes.insert(
            "no_node".to_string(),
            GraphNode {
                id: "no_node".to_string(),
                agent_step: AgentStep::LlmStep(LlmStepConfig {
                    name: "No Handler".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
                position: None,
                interrupt_before: false,
                interrupt_after: false,
            },
        );

        let mut branches = HashMap::new();
        branches.insert("yes".to_string(), "yes_node".to_string());
        branches.insert("no".to_string(), "no_node".to_string());

        let workflow = GraphWorkflow {
            name: "Conditional Flow".to_string(),
            description: None,
            nodes,
            edges: vec![Edge::Conditional {
                from: "router".to_string(),
                condition: ConditionConfig {
                    condition_key: "decision".to_string(),
                },
                branches,
                default_branch: None,
            }],
            entry_node: "router".to_string(),
            state_schema: StateSchema::default(),
        };

        let mermaid = generate_mermaid(&workflow);

        assert!(mermaid.starts_with("graph TD"));
        assert!(mermaid.contains("router[\"Router Agent\"]"));
        assert!(mermaid.contains("yes_node[\"Yes Handler\"]"));
        assert!(mermaid.contains("no_node[\"No Handler\"]"));
        assert!(mermaid.contains("router -->|no| no_node"));
        assert!(mermaid.contains("router -->|yes| yes_node"));
    }

    #[test]
    fn test_generate_mermaid_empty_workflow() {
        let workflow = GraphWorkflow {
            name: "Empty".to_string(),
            description: None,
            nodes: HashMap::new(),
            edges: vec![],
            entry_node: String::new(),
            state_schema: StateSchema::default(),
        };

        let mermaid = generate_mermaid(&workflow);
        assert_eq!(mermaid, "graph TD");
    }

    #[test]
    fn test_get_agent_step_name_variants() {
        let llm = AgentStep::LlmStep(LlmStepConfig {
            name: "llm-agent".to_string(),
            instruction: None,
            model: None,
            tools: None,
            config: AgentConfig::default(),
        });
        assert_eq!(get_agent_step_name(&llm), "llm-agent");

        let seq = AgentStep::SequentialStep {
            name: "seq-step".to_string(),
            steps: vec![],
        };
        assert_eq!(get_agent_step_name(&seq), "seq-step");

        let par = AgentStep::ParallelStep {
            name: "par-step".to_string(),
            steps: vec![],
        };
        assert_eq!(get_agent_step_name(&par), "par-step");

        let cond = AgentStep::ConditionalStep {
            name: "cond-step".to_string(),
            condition_key: "key".to_string(),
            branches: HashMap::new(),
            default_branch: None,
        };
        assert_eq!(get_agent_step_name(&cond), "cond-step");
    }
}
