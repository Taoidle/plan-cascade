//! MCP Commands
//!
//! Tauri commands for MCP server management and runtime tool integration.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::models::{
    CreateMcpServerRequest, HealthCheckResult, ImportResult, McpCatalogFilter,
    McpCatalogListResponse, McpCatalogRefreshResult, McpInstallPreview, McpInstallRecord,
    McpInstallRequest, McpInstallResult, McpRuntimeInfo, McpRuntimeKind, McpRuntimeRepairResult,
    McpServer, McpServerType, UpdateMcpServerRequest,
};
use crate::services::llm::types::ToolDefinition;
use crate::services::mcp::McpImportConflictPolicy;
use crate::services::mcp::McpService;
use crate::services::mcp_catalog::McpCatalogService;
use crate::services::mcp_installer::McpInstallerService;
use crate::services::mcp_runtime_manager::McpRuntimeManager;
use crate::services::tools::mcp_manager::{ConnectedServerInfo, McpManager};
use crate::services::tools::runtime_tools;
use crate::services::tools::trait_def::ToolRegistry;
use crate::utils::error::AppResult;

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectedMcpToolDetail {
    pub qualified_name: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub is_parallel_safe: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpExportSecretMode {
    Redacted,
    Include,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpExportOptions {
    #[serde(alias = "secretMode")]
    pub secret_mode: Option<McpExportSecretMode>,
    #[serde(alias = "formatVersion")]
    pub format_version: Option<String>,
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
        let by_name_prefix = server_name_hint
            .as_ref()
            .map(|name| format!("mcp:{}:", name));
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
pub fn export_mcp_servers(
    options: Option<McpExportOptions>,
) -> Result<CommandResponse<serde_json::Value>, String> {
    let secret_mode = options
        .as_ref()
        .and_then(|o| o.secret_mode.clone())
        .unwrap_or(McpExportSecretMode::Redacted);
    if secret_mode == McpExportSecretMode::Include {
        return Ok(CommandResponse::err(
            "Plaintext MCP secret export is disabled; use redacted export".to_string(),
        ));
    }

    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match service.list_servers_with_secrets(true) {
        Ok(servers) => {
            let format_version = options
                .as_ref()
                .and_then(|o| o.format_version.as_deref())
                .unwrap_or("2");
            match build_export_payload(servers, secret_mode, format_version) {
                Ok(payload) => Ok(CommandResponse::ok(payload)),
                Err(e) => Ok(CommandResponse::err(e)),
            }
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

fn build_export_payload(
    servers: Vec<McpServer>,
    secret_mode: McpExportSecretMode,
    format_version: &str,
) -> Result<serde_json::Value, String> {
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
                    let env = match secret_mode {
                        McpExportSecretMode::Include => server.env,
                        McpExportSecretMode::Redacted => server
                            .env
                            .keys()
                            .map(|k| (k.clone(), "__REDACTED__".to_string()))
                            .collect(),
                    };
                    item.insert("env".to_string(), serde_json::json!(env));
                }
            }
            McpServerType::StreamHttp => {
                if let Some(url) = server.url {
                    item.insert("url".to_string(), serde_json::Value::String(url));
                }
                if !server.headers.is_empty() {
                    let headers = match secret_mode {
                        McpExportSecretMode::Include => server.headers,
                        McpExportSecretMode::Redacted => server
                            .headers
                            .keys()
                            .map(|k| (k.clone(), "__REDACTED__".to_string()))
                            .collect(),
                    };
                    item.insert("headers".to_string(), serde_json::json!(headers));
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

    match format_version {
        "1" => Ok(serde_json::json!({ "mcpServers": mcp_servers })),
        "2" => Ok(serde_json::json!({
            "version": "2",
            "exported_at": chrono::Utc::now().to_rfc3339(),
            "secrets_redacted": secret_mode == McpExportSecretMode::Redacted,
            "mcpServers": mcp_servers,
        })),
        other => Err(format!("Unsupported MCP export format version: {}", other)),
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
    conflict_policy: Option<String>,
) -> Result<CommandResponse<ImportResult>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let policy = match conflict_policy.as_deref().unwrap_or("skip") {
        "skip" => McpImportConflictPolicy::Skip,
        "rename" => McpImportConflictPolicy::Rename,
        "replace" => McpImportConflictPolicy::Replace,
        other => {
            return Ok(CommandResponse::err(format!(
                "Unsupported conflict policy: {}",
                other
            )))
        }
    };

    match service.import_from_claude_desktop_with_options(dry_run.unwrap_or(false), policy) {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub fn import_mcp_from_file(
    path: String,
    dry_run: Option<bool>,
    conflict_policy: Option<String>,
) -> Result<CommandResponse<ImportResult>, String> {
    let service = match McpService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let policy = match conflict_policy.as_deref().unwrap_or("skip") {
        "skip" => McpImportConflictPolicy::Skip,
        "rename" => McpImportConflictPolicy::Rename,
        "replace" => McpImportConflictPolicy::Replace,
        other => {
            return Ok(CommandResponse::err(format!(
                "Unsupported conflict policy: {}",
                other
            )))
        }
    };

    match service.import_from_file_with_options(&path, dry_run.unwrap_or(false), policy) {
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
    match reconcile_and_connect_enabled_servers(
        state.manager.clone(),
        state.registry.clone(),
        "manual",
    )
    .await
    {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn list_connected_mcp_servers(
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<Vec<ConnectedServerInfo>>, String> {
    let mut servers = state.manager.list_connected_servers().await;
    if let Ok(service) = McpService::new() {
        if let Ok(db_servers) = service.list_servers() {
            let map: std::collections::HashMap<_, _> =
                db_servers.into_iter().map(|s| (s.id.clone(), s)).collect();
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

fn collect_connected_tool_details(
    info: &ConnectedServerInfo,
    registry: &ToolRegistry,
) -> Vec<ConnectedMcpToolDetail> {
    let mut tools = Vec::new();
    for qualified_name in &info.qualified_tool_names {
        let Some(tool) = registry.get(qualified_name) else {
            continue;
        };
        let tool_name = qualified_name
            .splitn(3, ':')
            .nth(2)
            .unwrap_or(qualified_name.as_str())
            .to_string();
        tools.push(ConnectedMcpToolDetail {
            qualified_name: qualified_name.clone(),
            tool_name,
            description: tool.description().to_string(),
            input_schema: serde_json::to_value(tool.parameters_schema())
                .unwrap_or_else(|_| serde_json::json!({})),
            is_parallel_safe: tool.is_parallel_safe(),
        });
    }
    tools
}

#[tauri::command]
pub async fn get_connected_mcp_server_tools(
    server_id: String,
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<Vec<ConnectedMcpToolDetail>>, String> {
    let connected = state.manager.list_connected_servers().await;
    let info = match connected.into_iter().find(|entry| entry.server_id == server_id) {
        Some(value) => value,
        None => {
            return Ok(CommandResponse::err(format!(
                "MCP server '{}' is not connected",
                server_id
            )))
        }
    };

    let registry = state.registry.read().await;
    let tools = collect_connected_tool_details(&info, &registry);

    Ok(CommandResponse::ok(tools))
}

#[tauri::command]
pub fn list_mcp_catalog(
    filter: Option<McpCatalogFilter>,
) -> Result<CommandResponse<McpCatalogListResponse>, String> {
    let service = match McpCatalogService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    match service.list_catalog(filter) {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub fn refresh_mcp_catalog(
    force: Option<bool>,
) -> Result<CommandResponse<McpCatalogRefreshResult>, String> {
    let service = match McpCatalogService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    match service.refresh_catalog(force.unwrap_or(false)) {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub fn preview_install_mcp_catalog_item(
    item_id: String,
    preferred_strategy: Option<String>,
) -> Result<CommandResponse<McpInstallPreview>, String> {
    let service = match McpInstallerService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    match service.preview_install(&item_id, preferred_strategy.as_deref()) {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn install_mcp_catalog_item(
    request: McpInstallRequest,
    app: tauri::AppHandle,
    state: tauri::State<'_, McpRuntimeState>,
) -> Result<CommandResponse<McpInstallResult>, String> {
    let installer = match McpInstallerService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    let auto_connect = request.auto_connect.unwrap_or(true);
    match installer
        .install_catalog_item(request.clone(), Some(&app))
        .await
    {
        Ok(mut result) => {
            if auto_connect {
                if let Some(server_id) = result.server_id.clone() {
                    let service = match McpService::new() {
                        Ok(s) => s,
                        Err(e) => return Ok(CommandResponse::err(e.to_string())),
                    };
                    if let Ok(Some(server)) = service.get_server(&server_id) {
                        if let Ok(config) = McpManager::config_from_model(&server) {
                            match state
                                .manager
                                .connect_server_with_registry_lock(&config, state.registry.clone())
                                .await
                            {
                                Ok(_) => {
                                    let _ = service.mark_server_connected(&server_id);
                                    let registry = state.registry.read().await;
                                    runtime_tools::replace_from_registry(&registry);
                                }
                                Err(e) => {
                                    let _ = service.mark_server_connection_error(
                                        &server_id,
                                        &format!("auto_connect: {}", e),
                                    );
                                    let previous = result.diagnostics.take().unwrap_or_default();
                                    result.diagnostics = Some(if previous.is_empty() {
                                        e.to_string()
                                    } else {
                                        format!("{}; {}", previous, e)
                                    });
                                }
                            }
                        }
                    }
                }
            }
            Ok(CommandResponse::ok(result))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn retry_mcp_install(
    job_id: String,
    app: tauri::AppHandle,
) -> Result<CommandResponse<McpInstallResult>, String> {
    let installer = match McpInstallerService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    match installer.retry_install(&job_id, Some(&app)).await {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub fn list_mcp_runtime_inventory() -> Result<CommandResponse<Vec<McpRuntimeInfo>>, String> {
    let manager = match McpRuntimeManager::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    match manager.refresh_inventory() {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn repair_mcp_runtime(
    runtime_kind: String,
) -> Result<CommandResponse<McpRuntimeRepairResult>, String> {
    let manager = match McpRuntimeManager::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    let runtime = match parse_runtime_kind(&runtime_kind) {
        Some(kind) => kind,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unsupported runtime kind: {}",
                runtime_kind
            )))
        }
    };
    match manager.repair_runtime(runtime).await {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub fn get_mcp_install_record(
    server_id: String,
) -> Result<CommandResponse<McpInstallRecord>, String> {
    let installer = match McpInstallerService::new() {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    match installer.get_install_record(&server_id) {
        Ok(Some(record)) => Ok(CommandResponse::ok(record)),
        Ok(None) => Ok(CommandResponse::err(format!(
            "MCP install record not found: {}",
            server_id
        ))),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

fn parse_runtime_kind(value: &str) -> Option<McpRuntimeKind> {
    match value {
        "node" => Some(McpRuntimeKind::Node),
        "uv" => Some(McpRuntimeKind::Uv),
        "python" => Some(McpRuntimeKind::Python),
        "docker" => Some(McpRuntimeKind::Docker),
        _ => None,
    }
}

pub(crate) async fn reconcile_and_connect_enabled_servers(
    manager: Arc<McpManager>,
    registry: Arc<RwLock<ToolRegistry>>,
    reason: &str,
) -> AppResult<McpAutoConnectResult> {
    let service = McpService::new()?;
    let connected_ids: std::collections::HashSet<String> = manager
        .list_connected_servers()
        .await
        .into_iter()
        .map(|s| s.server_id)
        .collect();
    let (reconciled_count, runtime_connected_count) =
        service.reconcile_runtime_statuses(&connected_ids)?;
    tracing::info!(
        event = "mcp_startup_reconcile_summary",
        reason = reason,
        reconciled_count = reconciled_count,
        runtime_connected_count = runtime_connected_count,
        "Reconciled persisted MCP statuses with runtime connections"
    );

    let servers = service.list_enabled_auto_connect_servers()?;
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

        let manager = manager.clone();
        let registry = registry.clone();
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
        event = "mcp_autoconnect_summary",
        reason = reason,
        success_count = success_count,
        failed_count = failed.len(),
        average_latency_ms = average_latency_ms,
        "Completed MCP auto-connect batch"
    );

    let registry = registry.read().await;
    runtime_tools::replace_from_registry(&registry);
    Ok(McpAutoConnectResult { connected, failed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::McpServerStatus;
    use crate::services::llm::types::ParameterSchema;
    use crate::services::tools::executor::ToolResult;
    use crate::services::tools::trait_def::{Tool, ToolExecutionContext};
    use async_trait::async_trait;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;

    struct TestTool {
        name: String,
        description: String,
        parallel_safe: bool,
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn parameters_schema(&self) -> ParameterSchema {
            ParameterSchema::object(None, HashMap::new(), vec![])
        }

        fn is_parallel_safe(&self) -> bool {
            self.parallel_safe
        }

        async fn execute(&self, _ctx: &ToolExecutionContext, _args: Value) -> ToolResult {
            ToolResult::ok("ok")
        }
    }

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

    #[test]
    fn test_build_export_payload_redacted_mode_hides_secrets() {
        let servers = vec![
            McpServer {
                id: "mcp-1".to_string(),
                name: "stdio-server".to_string(),
                server_type: McpServerType::Stdio,
                command: Some("echo".to_string()),
                args: vec!["hello".to_string()],
                env: HashMap::from([("API_KEY".to_string(), "plaintext".to_string())]),
                url: None,
                headers: HashMap::new(),
                has_env_secret: false,
                has_headers_secret: false,
                enabled: true,
                auto_connect: true,
                status: McpServerStatus::Unknown,
                last_error: None,
                last_connected_at: None,
                retry_count: 0,
                last_checked: None,
                managed_install: false,
                catalog_item_id: None,
                trust_level: None,
                created_at: None,
                updated_at: None,
            },
            McpServer {
                id: "mcp-2".to_string(),
                name: "http-server".to_string(),
                server_type: McpServerType::StreamHttp,
                command: None,
                args: vec![],
                env: HashMap::new(),
                url: Some("https://example.com/mcp".to_string()),
                headers: HashMap::from([("Authorization".to_string(), "Bearer token".to_string())]),
                has_env_secret: false,
                has_headers_secret: false,
                enabled: true,
                auto_connect: true,
                status: McpServerStatus::Unknown,
                last_error: None,
                last_connected_at: None,
                retry_count: 0,
                last_checked: None,
                managed_install: false,
                catalog_item_id: None,
                trust_level: None,
                created_at: None,
                updated_at: None,
            },
        ];

        let payload =
            build_export_payload(servers, McpExportSecretMode::Redacted, "2").expect("payload");
        let secrets_redacted = payload
            .get("secrets_redacted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(secrets_redacted);

        let map = payload
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .expect("mcpServers object");
        let stdio = map.get("stdio-server").and_then(|v| v.as_object()).unwrap();
        let http = map.get("http-server").and_then(|v| v.as_object()).unwrap();
        assert_eq!(
            stdio["env"]["API_KEY"].as_str().unwrap_or_default(),
            "__REDACTED__"
        );
        assert_eq!(
            http["headers"]["Authorization"]
                .as_str()
                .unwrap_or_default(),
            "__REDACTED__"
        );
    }

    #[test]
    fn test_build_export_payload_include_mode_keeps_secrets() {
        let servers = vec![McpServer {
            id: "mcp-1".to_string(),
            name: "stdio-server".to_string(),
            server_type: McpServerType::Stdio,
            command: Some("echo".to_string()),
            args: vec![],
            env: HashMap::from([("API_KEY".to_string(), "plaintext".to_string())]),
            url: None,
            headers: HashMap::new(),
            has_env_secret: false,
            has_headers_secret: false,
            enabled: true,
            auto_connect: true,
            status: McpServerStatus::Unknown,
            last_error: None,
            last_connected_at: None,
            retry_count: 0,
            last_checked: None,
            managed_install: false,
            catalog_item_id: None,
            trust_level: None,
            created_at: None,
            updated_at: None,
        }];

        let payload =
            build_export_payload(servers, McpExportSecretMode::Include, "2").expect("payload");
        let secrets_redacted = payload
            .get("secrets_redacted")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        assert!(!secrets_redacted);

        let map = payload
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .expect("mcpServers object");
        let stdio = map.get("stdio-server").and_then(|v| v.as_object()).unwrap();
        assert_eq!(
            stdio["env"]["API_KEY"].as_str().unwrap_or_default(),
            "plaintext"
        );
    }

    #[test]
    fn test_build_export_payload_invalid_format_version() {
        let payload = build_export_payload(vec![], McpExportSecretMode::Redacted, "9");
        assert!(payload.is_err());
    }

    #[test]
    fn test_export_mcp_servers_rejects_include_mode() {
        let result = export_mcp_servers(Some(McpExportOptions {
            secret_mode: Some(McpExportSecretMode::Include),
            format_version: Some("2".to_string()),
        }))
        .expect("command response");
        assert!(!result.success);
        assert!(result
            .error
            .unwrap_or_default()
            .contains("disabled"));
    }

    #[test]
    fn test_collect_connected_tool_details_returns_tool_metadata() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(TestTool {
            name: "mcp:server-1:list_files".to_string(),
            description: "List files in workspace".to_string(),
            parallel_safe: true,
        }));

        let info = ConnectedServerInfo {
            server_id: "server-1".to_string(),
            server_name: "Server One".to_string(),
            connection_state: "connected".to_string(),
            tool_names: vec!["list_files".to_string()],
            qualified_tool_names: vec!["mcp:server-1:list_files".to_string()],
            protocol_version: "2025-03-26".to_string(),
            connected_at: None,
            last_error: None,
            retry_count: 0,
        };

        let details = collect_connected_tool_details(&info, &registry);
        assert_eq!(details.len(), 1);
        assert_eq!(details[0].qualified_name, "mcp:server-1:list_files");
        assert_eq!(details[0].tool_name, "list_files");
        assert_eq!(details[0].description, "List files in workspace");
        assert!(details[0].input_schema.is_object());
        assert!(details[0].is_parallel_safe);
    }
}
