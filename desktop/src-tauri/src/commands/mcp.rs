//! MCP Commands
//!
//! Tauri commands for MCP server management.

use crate::models::response::CommandResponse;
use crate::models::{
    CreateMcpServerRequest, HealthCheckResult, ImportResult, McpServer, UpdateMcpServerRequest,
};
use crate::services::mcp::McpService;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_mcp_servers() {
        // This will work with an empty database
        let result = list_mcp_servers().unwrap();
        assert!(result.success);
    }
}
