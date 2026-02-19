//! A2A Commands
//!
//! Tauri commands for managing remote A2A (Agent-to-Agent) agents.
//! Provides discover, register, list, and remove operations.

use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::a2a::service::{A2aService, DiscoveredAgent, RegisteredRemoteAgent};
use crate::services::a2a::types::AgentCard;
use crate::state::AppState;

/// Discover a remote A2A agent at the given base URL.
///
/// Fetches the agent card from `{base_url}/.well-known/agent.json` and
/// validates it. Returns the discovered agent card without registering it.
#[tauri::command]
pub async fn discover_a2a_agent(
    state: State<'_, AppState>,
    base_url: String,
) -> Result<CommandResponse<DiscoveredAgent>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = A2aService::new(pool);

    match service.discover(&base_url).await {
        Ok(discovered) => Ok(CommandResponse::ok(discovered)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List all registered remote A2A agents.
#[tauri::command]
pub async fn list_a2a_agents(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<RegisteredRemoteAgent>>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = A2aService::new(pool);

    match service.list() {
        Ok(agents) => Ok(CommandResponse::ok(agents)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Register a remote A2A agent for use in pipelines.
///
/// Takes a base URL and the discovered agent card, and persists
/// the agent in the database. If an agent with the same URL already
/// exists, it is updated with the new card data.
#[tauri::command]
pub async fn register_a2a_agent(
    state: State<'_, AppState>,
    base_url: String,
    agent_card: AgentCard,
) -> Result<CommandResponse<RegisteredRemoteAgent>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = A2aService::new(pool);

    match service.register(&base_url, &agent_card).await {
        Ok(registered) => Ok(CommandResponse::ok(registered)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Remove (unregister) a remote A2A agent by its ID.
#[tauri::command]
pub async fn remove_a2a_agent(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResponse<bool>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = A2aService::new(pool);

    match service.remove(&id) {
        Ok(removed) => Ok(CommandResponse::ok(removed)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_a2a_commands_module() {
        // Basic module compilation test
        assert!(true);
    }
}
