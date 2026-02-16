//! MCP Commands
//!
//! Tauri commands for MCP server management and runtime tool integration.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::models::{
    CreateMcpServerRequest, HealthCheckResult, ImportResult, McpServer, UpdateMcpServerRequest,
};
use crate::services::mcp::McpService;
use crate::services::tools::mcp_client::McpToolInfo;
use crate::services::tools::mcp_manager::{ConnectedServerInfo, McpManager};
use crate::services::tools::trait_def::ToolRegistry;

/// Tauri-managed state for MCP runtime tool integration.
///
/// Holds the McpManager (manages connections) and a ToolRegistry
/// that MCP tools are registered into for use in the agentic loop.
pub struct McpRuntimeState {
    /// MCP connection manager
    pub manager: Arc<McpManager>,
    /// Tool registry for MCP tools (separate from built-in tools;
    /// merged at query time in the agentic loop)
    pub registry: Arc<RwLock<ToolRegistry>>,
}

impl McpRuntimeState {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(McpManager::new()),
            registry: Arc::new(RwLock::new(ToolRegistry::new())),
        }
    }
}

impl Default for McpRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

/// List all MCP servers
#[tauri::command]
pub fn list_mcp_servers() -> Result<CommandResponse<Vec<McpServer>>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.list_servers() {
        Ok(servers) => Ok(CommandResponse::ok(servers)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Add a new MCP server
#[tauri::command]
pub fn add_mcp_server(
    name: String,
    server_type: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<std::collections::HashMap<String, String>>,
    url: Option<String>,
    headers: Option<std::collections::HashMap<String, String>>,
) -> Result<CommandResponse<McpServer>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let server_type = match server_type.as_str() {
        "sse" => crate::models::McpServerType::Sse,
        _ => crate::models::McpServerType::Stdio,
    };

    let request = CreateMcpServerRequest {
        name,
        server_type,
        command,
        args,
        env,
        url,
        headers,
    };

    match service.add_server(request) {
        Ok(server) => Ok(CommandResponse::ok(server)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update an existing MCP server
#[tauri::command]
pub fn update_mcp_server(
    id: String,
    name: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<std::collections::HashMap<String, String>>,
    url: Option<String>,
    headers: Option<std::collections::HashMap<String, String>>,
    enabled: Option<bool>,
) -> Result<CommandResponse<McpServer>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let request = UpdateMcpServerRequest {
        name,
        command,
        args,
        env,
        url,
        headers,
        enabled,
    };

    match service.update_server(&id, request) {
        Ok(server) => Ok(CommandResponse::ok(server)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Remove an MCP server
#[tauri::command]
pub fn remove_mcp_server(id: String) -> Result<CommandResponse<()>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.remove_server(&id) {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Test an MCP server connection
#[tauri::command]
pub async fn test_mcp_server(id: String) -> Result<CommandResponse<HealthCheckResult>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.test_server(&id).await {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Toggle MCP server enabled status
#[tauri::command]
pub fn toggle_mcp_server(id: String, enabled: bool) -> Result<CommandResponse<McpServer>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.toggle_server(&id, enabled) {
        Ok(server) => Ok(CommandResponse::ok(server)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Import MCP servers from Claude Desktop configuration
#[tauri::command]
pub fn import_from_claude_desktop() -> Result<CommandResponse<ImportResult>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.import_from_claude_desktop() {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Connect to an MCP server and register its tools into the ToolRegistry.
///
/// Looks up the server by ID from the database, connects to it via
/// the appropriate transport (stdio/HTTP), discovers available tools,
/// and registers them as Tool trait implementations.
#[tauri::command]
pub async fn connect_mcp_server(
    id: String,
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<ConnectedServerInfo>, String> {
    // Look up the server config from the database
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let server = match service.get_server(&id) {
        Ok(Some(s)) => s,
        Ok(None) => return Ok(CommandResponse::err(format!("Server not found: {}", id))),
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    if !server.enabled {
        return Ok(CommandResponse::err(format!(
            "Server '{}' is disabled",
            server.name
        )));
    }

    // Convert the database model to McpServerConfig
    let config = match McpManager::config_from_model(&server) {
        Ok(c) => c,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // Connect and register tools
    let mut registry = state.registry.write().await;
    match state.manager.connect_server(&config, &mut registry).await {
        Ok(info) => Ok(CommandResponse::ok(info)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Disconnect from an MCP server and unregister its tools.
#[tauri::command]
pub async fn disconnect_mcp_server(
    server_name: String,
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<()>, String> {
    let mut registry = state.registry.write().await;
    match state
        .manager
        .disconnect_server(&server_name, &mut registry)
        .await
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List currently connected MCP servers and their registered tools.
#[tauri::command]
pub async fn list_connected_mcp_servers(
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<Vec<ConnectedServerInfo>>, String> {
    let servers = state.manager.list_connected_servers().await;
    Ok(CommandResponse::ok(servers))
}

/// Get the MCP tool definitions currently available in the registry.
///
/// Returns tool definitions in the format expected by LLM providers,
/// allowing the frontend to display available MCP tools.
#[tauri::command]
pub async fn list_mcp_tools(
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<Vec<String>>, String> {
    let registry = state.registry.read().await;
    Ok(CommandResponse::ok(registry.names()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_mcp_servers() {
        // This will work with an empty database
        let result = list_mcp_servers().unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_mcp_runtime_state_new() {
        let state = McpRuntimeState::new();
        // Verify it can be constructed
        assert!(Arc::strong_count(&state.manager) == 1);
    }

    #[test]
    fn test_mcp_runtime_state_default() {
        let state = McpRuntimeState::default();
        assert!(Arc::strong_count(&state.manager) == 1);
    }

    #[tokio::test]
    async fn test_mcp_runtime_state_registry_empty() {
        let state = McpRuntimeState::new();
        let registry = state.registry.read().await;
        assert!(registry.is_empty());
    }

    #[tokio::test]
    async fn test_mcp_runtime_state_manager_no_connections() {
        let state = McpRuntimeState::new();
        assert_eq!(state.manager.connected_count().await, 0);
    }
}
