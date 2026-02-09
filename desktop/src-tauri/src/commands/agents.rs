//! Agent Commands
//!
//! Tauri commands for managing AI agents.

use tauri::State;

use crate::models::agent::{
    Agent, AgentCreateRequest, AgentRun, AgentRunList, AgentStats, AgentUpdateRequest,
    AgentWithStats,
};
use crate::models::response::CommandResponse;
use crate::services::agent::AgentService;
use crate::state::AppState;

/// List all agents
#[tauri::command]
pub async fn list_agents(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<Agent>>, String> {
    // Initialize service from database
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    match service.list_agents().await {
        Ok(agents) => Ok(CommandResponse::ok(agents)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List all agents with their statistics
#[tauri::command]
pub async fn list_agents_with_stats(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<AgentWithStats>>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    match service.list_agents_with_stats().await {
        Ok(agents) => Ok(CommandResponse::ok(agents)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get a single agent by ID
#[tauri::command]
pub async fn get_agent(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResponse<Option<Agent>>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    match service.get_agent(&id).await {
        Ok(agent) => Ok(CommandResponse::ok(agent)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a new agent
#[tauri::command]
pub async fn create_agent(
    state: State<'_, AppState>,
    request: AgentCreateRequest,
) -> Result<CommandResponse<Agent>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    match service.create_agent(request).await {
        Ok(agent) => Ok(CommandResponse::ok(agent)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update an existing agent
#[tauri::command]
pub async fn update_agent(
    state: State<'_, AppState>,
    id: String,
    request: AgentUpdateRequest,
) -> Result<CommandResponse<Agent>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    match service.update_agent(&id, request).await {
        Ok(agent) => Ok(CommandResponse::ok(agent)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete an agent
#[tauri::command]
pub async fn delete_agent(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResponse<()>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    match service.delete_agent(&id).await {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get agent run history with pagination
#[tauri::command]
pub async fn get_agent_history(
    state: State<'_, AppState>,
    agent_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<CommandResponse<AgentRunList>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    match service
        .get_run_history(&agent_id, limit.unwrap_or(50), offset.unwrap_or(0))
        .await
    {
        Ok(history) => Ok(CommandResponse::ok(history)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get statistics for an agent
#[tauri::command]
pub async fn get_agent_stats(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<CommandResponse<AgentStats>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    match service.get_agent_stats(&agent_id).await {
        Ok(stats) => Ok(CommandResponse::ok(stats)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get a single agent run by ID
#[tauri::command]
pub async fn get_agent_run(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<CommandResponse<Option<AgentRun>>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    match service.get_run(&run_id).await {
        Ok(run) => Ok(CommandResponse::ok(run)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Prune old runs for an agent
#[tauri::command]
pub async fn prune_agent_runs(
    state: State<'_, AppState>,
    agent_id: String,
    keep_count: u32,
) -> Result<CommandResponse<u32>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    match service.prune_old_runs(&agent_id, keep_count).await {
        Ok(deleted) => Ok(CommandResponse::ok(deleted)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Run an agent with the given input (non-streaming)
/// For streaming, use the event-based run_agent_stream command
#[tauri::command]
pub async fn run_agent(
    state: State<'_, AppState>,
    agent_id: String,
    input: String,
) -> Result<CommandResponse<AgentRun>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = std::sync::Arc::new(AgentService::new(pool));

    // For a basic run without provider, we just create a pending run
    // The actual execution would require a configured LLM provider
    match service.create_run(&agent_id, &input).await {
        Ok(run) => Ok(CommandResponse::ok(run)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Export agents as JSON
#[tauri::command]
pub async fn export_agents(
    state: State<'_, AppState>,
    agent_ids: Option<Vec<String>>,
) -> Result<CommandResponse<String>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    let agents = match service.list_agents().await {
        Ok(agents) => agents,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // Filter if specific IDs provided
    let filtered = if let Some(ids) = agent_ids {
        agents.into_iter().filter(|a| ids.contains(&a.id)).collect()
    } else {
        agents
    };

    match serde_json::to_string_pretty(&filtered) {
        Ok(json) => Ok(CommandResponse::ok(json)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Import agents from JSON
#[tauri::command]
pub async fn import_agents(
    state: State<'_, AppState>,
    json: String,
) -> Result<CommandResponse<Vec<Agent>>, String> {
    // Parse the JSON
    let agents_to_import: Vec<AgentCreateRequest> = match serde_json::from_str(&json) {
        Ok(agents) => agents,
        Err(e) => return Ok(CommandResponse::err(format!("Invalid JSON: {}", e))),
    };

    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = AgentService::new(pool);

    let mut imported = Vec::new();
    let mut errors = Vec::new();

    for request in agents_to_import {
        match service.create_agent(request.clone()).await {
            Ok(agent) => imported.push(agent),
            Err(e) => errors.push(format!("{}: {}", request.name, e)),
        }
    }

    if errors.is_empty() {
        Ok(CommandResponse::ok(imported))
    } else {
        Ok(CommandResponse::err(format!(
            "Imported {} agents with {} errors: {}",
            imported.len(),
            errors.len(),
            errors.join(", ")
        )))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_commands_module() {
        // Basic module compilation test
        assert!(true);
    }
}
