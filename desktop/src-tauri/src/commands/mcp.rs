//! MCP Commands
//!
//! Tauri commands for MCP server management and runtime tool integration.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::models::{
    CreateMcpServerRequest, HealthCheckResult, ImportResult, McpServer, McpServerType,
    UpdateMcpServerRequest,
};
use crate::services::llm::types::ToolDefinition;
use crate::services::mcp::McpService;
use crate::services::tools::mcp_manager::{ConnectedServerInfo, McpManager};
use crate::services::tools::runtime_tools;
use crate::services::tools::trait_def::ToolRegistry;

pub struct McpRuntimeState {
    pub manager: Arc<McpManager>,
    pub registry: Arc<RwLock<ToolRegistry>>,
}

impl McpRuntimeState {
    pub fn new() -> Self {
        runtime_tools::clear();
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpAutoConnectResult {
    pub connected: Vec<ConnectedServerInfo>,
    pub failed: Vec<String>,
}

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
        "stream_http" | "sse" => McpServerType::StreamHttp,
        _ => McpServerType::Stdio,
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

#[tauri::command]
pub fn import_mcp_from_file(path: String) -> Result<CommandResponse<ImportResult>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.import_from_file(&path) {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn connect_mcp_server(
    id: String,
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<ConnectedServerInfo>, String> {
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

    let config = match McpManager::config_from_model(&server) {
        Ok(c) => c,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let mut registry = state.registry.write().await;
    match state.manager.connect_server(&config, &mut registry).await {
        Ok(info) => {
            runtime_tools::replace_from_registry(&registry);
            Ok(CommandResponse::ok(info))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn disconnect_mcp_server(
    id: String,
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<()>, String> {
    let mut registry = state.registry.write().await;
    match state.manager.disconnect_server(&id, &mut registry).await {
        Ok(()) => {
            runtime_tools::replace_from_registry(&registry);
            Ok(CommandResponse::ok(()))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn connect_enabled_mcp_servers(
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<McpAutoConnectResult>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let servers = match service.list_enabled_auto_connect_servers() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let mut connected = Vec::new();
    let mut failed = Vec::new();
    let mut registry = state.registry.write().await;

    for server in servers {
        let config = match McpManager::config_from_model(&server) {
            Ok(c) => c,
            Err(e) => {
                failed.push(format!("{}: {}", server.name, e));
                continue;
            }
        };

        match state.manager.connect_server(&config, &mut registry).await {
            Ok(info) => connected.push(info),
            Err(e) => failed.push(format!("{}: {}", server.name, e)),
        }
    }

    runtime_tools::replace_from_registry(&registry);
    Ok(CommandResponse::ok(McpAutoConnectResult {
        connected,
        failed,
    }))
}

#[tauri::command]
pub async fn list_connected_mcp_servers(
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<Vec<ConnectedServerInfo>>, String> {
    let servers = state.manager.list_connected_servers().await;
    Ok(CommandResponse::ok(servers))
}

#[tauri::command]
pub async fn list_mcp_tools(
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<Vec<ToolDefinition>>, String> {
    let _ = state;
    Ok(CommandResponse::ok(runtime_tools::definitions()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_mcp_servers() {
        let result = list_mcp_servers().unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_mcp_runtime_state_manager_no_connections() {
        let state = McpRuntimeState::new();
        assert_eq!(state.manager.connected_count().await, 0);
    }
}
