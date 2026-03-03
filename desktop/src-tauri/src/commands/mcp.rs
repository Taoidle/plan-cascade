//! MCP Commands
//!
//! Tauri commands for MCP server management and runtime tool integration.

use std::sync::Arc;
use std::time::Duration;
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
    auto_connect: Option<bool>,
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
        auto_connect,
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
    server_type: Option<String>,
    command: Option<String>,
    clear_command: Option<bool>,
    args: Option<Vec<String>>,
    env: Option<std::collections::HashMap<String, String>>,
    url: Option<String>,
    clear_url: Option<bool>,
    headers: Option<std::collections::HashMap<String, String>>,
    enabled: Option<bool>,
    auto_connect: Option<bool>,
) -> Result<CommandResponse<McpServer>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let parsed_server_type = match server_type.as_deref() {
        Some("stdio") => Some(McpServerType::Stdio),
        Some("stream_http") | Some("sse") => Some(McpServerType::StreamHttp),
        Some(other) => {
            return Ok(CommandResponse::err(format!(
                "Unsupported MCP server type: {}",
                other
            )))
        }
        None => None,
    };

    let request = UpdateMcpServerRequest {
        name,
        server_type: parsed_server_type,
        command,
        clear_command: clear_command.unwrap_or(false),
        args,
        env,
        url,
        clear_url: clear_url.unwrap_or(false),
        headers,
        enabled,
        auto_connect,
    };

    match service.update_server(&id, request) {
        Ok(server) => Ok(CommandResponse::ok(server)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn remove_mcp_server(
    id: String,
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<()>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    let server_name_hint = service
        .get_server_with_secrets(&id, false)
        .ok()
        .flatten()
        .map(|s| s.name);

    let mut cleanup_error: Option<String> = None;
    {
        let mut registry = state.registry.write().await;
        if state.manager.is_connected(&id).await {
            match tokio::time::timeout(
                Duration::from_secs(3),
                state.manager.disconnect_server(&id, &mut registry),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => cleanup_error = Some(e.to_string()),
                Err(_) => cleanup_error = Some("disconnect timeout".to_string()),
            }
        }

        let by_id_prefix = format!("mcp:{}:", id);
        let by_name_prefix = server_name_hint.as_ref().map(|name| format!("mcp:{}:", name));
        let stale_tools: Vec<String> = registry
            .names()
            .into_iter()
            .filter(|name| {
                name.starts_with(&by_id_prefix)
                    || by_name_prefix
                        .as_ref()
                        .map(|prefix| name.starts_with(prefix))
                        .unwrap_or(false)
            })
            .collect();
        for stale in stale_tools {
            registry.unregister(&stale);
        }

        runtime_tools::replace_from_registry(&registry);
    }

    match service.remove_server(&id) {
        Ok(()) => {
            tracing::info!(
                event = "delete",
                server_id = %id,
                "Removed MCP server"
            );
            if let Some(err) = cleanup_error {
                tracing::warn!(
                    server_id = %id,
                    error = %err,
                    "MCP server removed but runtime cleanup was partial"
                );
            }
            Ok(CommandResponse::ok(()))
        }
        Err(e) => {
            tracing::warn!(
                event = "delete_failure",
                server_id = %id,
                error = %e,
                "Failed to remove MCP server"
            );
            if let Some(cleanup) = cleanup_error {
                Ok(CommandResponse::err(format!(
                    "Failed to cleanup runtime ({}) and remove server ({})",
                    cleanup, e
                )))
            } else {
                Ok(CommandResponse::err(e.to_string()))
            }
        }
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
pub fn get_mcp_server_detail(
    id: String,
    include_secrets: Option<bool>,
) -> Result<CommandResponse<McpServer>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.get_server_with_secrets(&id, include_secrets.unwrap_or(false)) {
        Ok(Some(server)) => Ok(CommandResponse::ok(server)),
        Ok(None) => Ok(CommandResponse::err(format!("Server not found: {}", id))),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub fn export_mcp_servers() -> Result<CommandResponse<serde_json::Value>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.list_servers_with_secrets(true) {
        Ok(servers) => {
            let mut mcp_servers = serde_json::Map::new();
            let mut name_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for server in servers {
                let mut item = serde_json::Map::new();
                match server.server_type {
                    McpServerType::Stdio => {
                        if let Some(command) = server.command {
                            item.insert("command".to_string(), serde_json::Value::String(command));
                        }
                        if !server.args.is_empty() {
                            item.insert("args".to_string(), serde_json::json!(server.args));
                        }
                        if !server.env.is_empty() {
                            item.insert("env".to_string(), serde_json::json!(server.env));
                        }
                    }
                    McpServerType::StreamHttp => {
                        if let Some(url) = server.url {
                            item.insert("url".to_string(), serde_json::Value::String(url));
                        }
                        if !server.headers.is_empty() {
                            item.insert("headers".to_string(), serde_json::json!(server.headers));
                        }
                    }
                }
                let count = name_counts.entry(server.name.clone()).or_insert(0);
                *count += 1;
                let export_name = if *count == 1 {
                    server.name
                } else {
                    format!("{} ({})", server.name, count)
                };
                mcp_servers.insert(export_name, serde_json::Value::Object(item));
            }
            Ok(CommandResponse::ok(serde_json::json!({ "mcpServers": mcp_servers })))
        }
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
pub fn import_from_claude_desktop(
    dry_run: Option<bool>,
) -> Result<CommandResponse<ImportResult>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.import_from_claude_desktop_with_options(dry_run.unwrap_or(false)) {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub fn import_mcp_from_file(
    path: String,
    dry_run: Option<bool>,
) -> Result<CommandResponse<ImportResult>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.import_from_file_with_options(&path, dry_run.unwrap_or(false)) {
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

    match state
        .manager
        .connect_server_with_registry_lock(&config, state.registry.clone())
        .await
    {
        Ok(info) => {
            if let Err(e) = service.mark_server_connected(&id) {
                let mut registry = state.registry.write().await;
                let _ = state.manager.disconnect_server(&id, &mut registry).await;
                runtime_tools::replace_from_registry(&registry);
                return Ok(CommandResponse::err(format!(
                    "Connected but failed to persist MCP status: {}",
                    e
                )));
            }
            let registry = state.registry.read().await;
            runtime_tools::replace_from_registry(&registry);
            Ok(CommandResponse::ok(info))
        }
        Err(e) => {
            let _ = service.mark_server_connection_error(&id, &e.to_string());
            Ok(CommandResponse::err(e.to_string()))
        }
    }
}

#[tauri::command]
pub async fn disconnect_mcp_server(
    id: String,
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<()>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let mut registry = state.registry.write().await;
    match state.manager.disconnect_server(&id, &mut registry).await {
        Ok(()) => {
            runtime_tools::replace_from_registry(&registry);
            if let Err(e) = service.mark_server_disconnected(&id) {
                return Ok(CommandResponse::err(format!(
                    "Disconnected but failed to persist MCP status: {}",
                    e
                )));
            }
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
    let semaphore = Arc::new(tokio::sync::Semaphore::new(2));
    let mut join_set = tokio::task::JoinSet::new();

    for server in servers {
        let config = match McpManager::config_from_model(&server) {
            Ok(c) => c,
            Err(e) => {
                let msg = e.to_string();
                let _ = service.mark_server_connection_error(&server.id, &msg);
                failed.push(format!("{}: {}", server.name, msg));
                continue;
            }
        };

        let permit = match semaphore.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => {
                let msg = "transport: connector semaphore closed".to_string();
                let _ = service.mark_server_connection_error(&server.id, &msg);
                failed.push(format!("{}: {}", server.name, msg));
                continue;
            }
        };

        let manager = state.manager.clone();
        let registry = state.registry.clone();
        join_set.spawn(async move {
            let _permit = permit;
            let started = std::time::Instant::now();
            let result = tokio::time::timeout(
                Duration::from_secs(10),
                manager.connect_server_with_registry_lock(&config, registry),
            )
            .await;
            let latency_ms = started.elapsed().as_millis() as u64;
            (server, result, latency_ms)
        });
    }

    let mut success_count = 0u32;
    let mut latency_total_ms = 0u64;
    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok((server, Ok(Ok(info)), latency_ms)) => {
                let _ = service.mark_server_connected(&server.id);
                success_count += 1;
                latency_total_ms += latency_ms;
                connected.push(info);
            }
            Ok((server, Ok(Err(e)), _latency_ms)) => {
                let msg = e.to_string();
                let _ = service.mark_server_connection_error(&server.id, &msg);
                failed.push(format!("{}: {}", server.name, msg));
            }
            Ok((server, Err(_), _latency_ms)) => {
                let msg = "transport: connection timeout".to_string();
                let _ = service.mark_server_connection_error(&server.id, &msg);
                failed.push(format!("{}: {}", server.name, msg));
            }
            Err(e) => {
                failed.push(format!("task join error: {}", e));
            }
        }
    }

    let average_latency_ms = if success_count > 0 {
        latency_total_ms / u64::from(success_count)
    } else {
        0
    };
    tracing::info!(
        event = "connect_enabled_summary",
        success_count = success_count,
        failed_count = failed.len(),
        average_latency_ms = average_latency_ms,
        "Completed MCP auto-connect batch"
    );

    let registry = state.registry.read().await;
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
    let mut servers = state.manager.list_connected_servers().await;
    if let Ok(service) = McpService::new() {
        if let Ok(db_servers) = service.list_servers() {
            let map: std::collections::HashMap<_, _> = db_servers
                .into_iter()
                .map(|s| (s.id.clone(), s))
                .collect();
            for runtime in &mut servers {
                if let Some(db) = map.get(&runtime.server_id) {
                    runtime.last_error = db.last_error.clone();
                    runtime.retry_count = db.retry_count;
                    if runtime.connected_at.is_none() {
                        runtime.connected_at = db.last_connected_at.clone();
                    }
                }
            }
        }
    }
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
