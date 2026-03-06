//! MCP Service
//!
//! Business logic for MCP server management.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::models::{
    CreateMcpServerRequest, HealthCheckResult, ImportResult, McpServer, McpServerStatus,
    McpServerType, UpdateMcpServerRequest,
};
use crate::services::tools::mcp_client::McpClient;
use crate::services::tools::mcp_manager::McpManager;
use crate::storage::database::Database;
use crate::storage::KeyringService;
use crate::utils::error::{AppError, AppResult};

/// Service for managing MCP servers.
pub struct McpService {
    db: Database,
    keyring: KeyringService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpImportConflictPolicy {
    Skip,
    Rename,
    Replace,
}

const REDACTED_PLACEHOLDER: &str = "__REDACTED__";

impl McpService {
    /// Create a new MCP service.
    pub fn new() -> AppResult<Self> {
        let db = Database::new()?;
        Ok(Self {
            db,
            keyring: KeyringService::new(),
        })
    }

    /// Create with existing database instance.
    pub fn with_database(db: Database) -> Self {
        Self {
            db,
            keyring: KeyringService::new(),
        }
    }

    fn env_secret_key(id: &str) -> String {
        format!("mcp/{}/env", id)
    }

    fn headers_secret_key(id: &str) -> String {
        format!("mcp/{}/headers", id)
    }

    fn generate_id() -> String {
        format!("mcp-{}", Uuid::new_v4())
    }

    fn hydrate_sensitive(&self, server: &mut McpServer) {
        if server.has_env_secret {
            let key = Self::env_secret_key(&server.id);
            if let Ok(Some(raw)) = self.keyring.get_api_key(&key) {
                if let Ok(env) = serde_json::from_str::<HashMap<String, String>>(&raw) {
                    server.env = env;
                }
            }
        }

        if server.has_headers_secret {
            let key = Self::headers_secret_key(&server.id);
            if let Ok(Some(raw)) = self.keyring.get_api_key(&key) {
                if let Ok(headers) = serde_json::from_str::<HashMap<String, String>>(&raw) {
                    server.headers = headers;
                }
            }
        }
    }

    fn clear_sensitive_payload(server: &mut McpServer) {
        server.env.clear();
        server.headers.clear();
    }

    fn persist_sensitive(
        &self,
        server: &mut McpServer,
        rewrite_env: bool,
        rewrite_headers: bool,
    ) -> AppResult<()> {
        let env_key = Self::env_secret_key(&server.id);
        if rewrite_env {
            if server.env.is_empty() {
                server.has_env_secret = false;
                let _ = self.keyring.delete_api_key(&env_key);
            } else {
                let value = serde_json::to_string(&server.env)?;
                self.keyring.set_api_key(&env_key, &value)?;
                server.has_env_secret = true;
            }
        }
        // Keep secrets out of DB payload.
        server.env.clear();

        let headers_key = Self::headers_secret_key(&server.id);
        if rewrite_headers {
            if server.headers.is_empty() {
                server.has_headers_secret = false;
                let _ = self.keyring.delete_api_key(&headers_key);
            } else {
                let value = serde_json::to_string(&server.headers)?;
                self.keyring.set_api_key(&headers_key, &value)?;
                server.has_headers_secret = true;
            }
        }
        // Keep secrets out of DB payload.
        server.headers.clear();

        Ok(())
    }

    fn clear_sensitive(&self, id: &str) {
        let _ = self.keyring.delete_api_key(&Self::env_secret_key(id));
        let _ = self.keyring.delete_api_key(&Self::headers_secret_key(id));
    }

    fn read_secret_map_from_keyring(&self, key: &str) -> HashMap<String, String> {
        self.keyring
            .get_api_key(key)
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str::<HashMap<String, String>>(&raw).ok())
            .unwrap_or_default()
    }

    fn cleanup_managed_install_assets(&self, id: &str) -> AppResult<()> {
        let Some(record) = self.db.get_mcp_install_record(id)? else {
            return Ok(());
        };
        let Some(lock) = record.package_lock_json else {
            return Ok(());
        };

        let root = managed_root_path().ok_or_else(|| {
            AppError::config("Cannot determine home directory for managed MCP cleanup")
        })?;
        if !root.exists() {
            return Ok(());
        }
        let root = root.canonicalize().unwrap_or(root);
        let mut seen = std::collections::HashSet::new();
        for path in extract_managed_paths(&lock) {
            let path_key = path.to_string_lossy().to_string();
            if !seen.insert(path_key) {
                continue;
            }
            if !path.exists() {
                continue;
            }
            let canonical = match path.canonicalize() {
                Ok(value) => value,
                Err(_) => continue,
            };
            if !canonical.starts_with(&root) {
                tracing::warn!(
                    server_id = %id,
                    path = %canonical.display(),
                    "Skip cleanup outside managed MCP root"
                );
                continue;
            }

            if canonical.is_dir() {
                std::fs::remove_dir_all(&canonical)?;
            } else {
                std::fs::remove_file(&canonical)?;
            }
        }
        Ok(())
    }

    /// List all MCP servers.
    pub fn list_servers(&self) -> AppResult<Vec<McpServer>> {
        self.list_servers_with_secrets(false)
    }

    /// List all MCP servers, optionally including hydrated sensitive values.
    pub fn list_servers_with_secrets(&self, include_secrets: bool) -> AppResult<Vec<McpServer>> {
        let mut servers = self.db.list_mcp_servers()?;
        for server in &mut servers {
            if include_secrets {
                self.hydrate_sensitive(server);
            } else {
                Self::clear_sensitive_payload(server);
            }
        }
        Ok(servers)
    }

    /// List enabled MCP servers that should auto-connect.
    pub fn list_enabled_auto_connect_servers(&self) -> AppResult<Vec<McpServer>> {
        Ok(self
            .list_servers_with_secrets(true)?
            .into_iter()
            .filter(|s| s.enabled && s.auto_connect)
            .collect())
    }

    /// Reconcile persisted MCP status with current runtime connections.
    ///
    /// Returns (reconciled_count, connected_runtime_count).
    pub fn reconcile_runtime_statuses(
        &self,
        connected_server_ids: &std::collections::HashSet<String>,
    ) -> AppResult<(u32, u32)> {
        let servers = self.list_servers_with_secrets(false)?;
        let mut reconciled = 0u32;
        for server in servers {
            if matches!(server.status, McpServerStatus::Connected)
                && !connected_server_ids.contains(&server.id)
            {
                self.mark_server_disconnected(&server.id)?;
                reconciled += 1;
            }
        }
        Ok((reconciled, connected_server_ids.len() as u32))
    }

    /// Get a server by ID.
    pub fn get_server(&self, id: &str) -> AppResult<Option<McpServer>> {
        self.get_server_with_secrets(id, true)
    }

    /// Get a server by ID, optionally including hydrated sensitive values.
    pub fn get_server_with_secrets(
        &self,
        id: &str,
        include_secrets: bool,
    ) -> AppResult<Option<McpServer>> {
        let mut server = match self.db.get_mcp_server(id)? {
            Some(s) => s,
            None => return Ok(None),
        };
        if include_secrets {
            self.hydrate_sensitive(&mut server);
        } else {
            Self::clear_sensitive_payload(&mut server);
        }
        Ok(Some(server))
    }

    /// Add a new MCP server.
    pub fn add_server(&self, request: CreateMcpServerRequest) -> AppResult<McpServer> {
        let id = Self::generate_id();
        let name = request.name.trim().to_string();
        self.ensure_name_unique(&name, None)?;

        let mut server = McpServer {
            id: id.clone(),
            name,
            server_type: request.server_type,
            command: request.command,
            args: request.args.unwrap_or_default(),
            env: request.env.unwrap_or_default(),
            url: request.url,
            headers: request.headers.unwrap_or_default(),
            has_env_secret: false,
            has_headers_secret: false,
            enabled: true,
            auto_connect: request.auto_connect.unwrap_or(true),
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
        };

        server.validate().map_err(AppError::validation)?;
        self.persist_sensitive(&mut server, true, true)?;

        self.db.insert_mcp_server(&server)?;
        self.get_server(&id)?.ok_or_else(|| {
            AppError::database("Failed to retrieve newly created server".to_string())
        })
    }

    /// Update an existing MCP server.
    pub fn update_server(&self, id: &str, request: UpdateMcpServerRequest) -> AppResult<McpServer> {
        let mut server = self
            .get_server(id)?
            .ok_or_else(|| AppError::not_found(format!("Server not found: {}", id)))?;

        if let Some(name) = request.name {
            server.name = name.trim().to_string();
        }
        if let Some(server_type) = request.server_type {
            server.server_type = server_type;
        }
        if request.clear_command {
            server.command = None;
        }
        if let Some(command) = request.command {
            server.command = Some(command);
        }
        if let Some(args) = request.args {
            server.args = args;
        }
        let mut env_updated = request.env.is_some();
        if let Some(env) = request.env {
            server.env = env;
        }
        if request.clear_url {
            server.url = None;
        }
        if let Some(url) = request.url {
            server.url = Some(url);
        }
        let mut headers_updated = request.headers.is_some();
        if let Some(headers) = request.headers {
            server.headers = headers;
        }
        if let Some(enabled) = request.enabled {
            server.enabled = enabled;
        }
        if let Some(auto_connect) = request.auto_connect {
            server.auto_connect = auto_connect;
        }

        // Keep the non-selected transport clean.
        match server.server_type {
            McpServerType::Stdio => {
                server.url = None;
                server.headers.clear();
                headers_updated = true;
            }
            McpServerType::StreamHttp => {
                server.command = None;
                server.args.clear();
                server.env.clear();
                env_updated = true;
            }
        }

        server.validate().map_err(AppError::validation)?;
        self.ensure_name_unique(&server.name, Some(id))?;
        self.persist_sensitive(&mut server, env_updated, headers_updated)?;

        self.db.update_mcp_server(&server)?;
        self.get_server(id)?
            .ok_or_else(|| AppError::not_found(format!("Server not found: {}", id)))
    }

    /// Remove an MCP server.
    pub fn remove_server(&self, id: &str) -> AppResult<()> {
        let cleanup_result = self.cleanup_managed_install_assets(id);
        let delete_result = self.db.delete_mcp_server(id);
        self.clear_sensitive(id);
        if let Err(delete_error) = delete_result {
            if let Err(cleanup_error) = cleanup_result {
                return Err(AppError::database(format!(
                    "Failed to cleanup managed assets ({}) and remove server ({})",
                    cleanup_error, delete_error
                )));
            }
            return Err(delete_error);
        }
        let _ = self.db.delete_mcp_install_record(id);
        if let Err(cleanup_error) = cleanup_result {
            tracing::warn!(
                server_id = %id,
                error = %cleanup_error,
                "MCP server removed but managed asset cleanup was partial"
            );
        }
        Ok(())
    }

    /// Toggle server enabled status.
    pub fn toggle_server(&self, id: &str, enabled: bool) -> AppResult<McpServer> {
        self.db.toggle_mcp_server_enabled(id, enabled)?;
        self.get_server(id)?
            .ok_or_else(|| AppError::not_found(format!("Server not found: {}", id)))
    }

    /// Test a server connection.
    pub async fn test_server(&self, id: &str) -> AppResult<HealthCheckResult> {
        let server = self
            .get_server(id)?
            .ok_or_else(|| AppError::not_found(format!("Server not found: {}", id)))?;
        let transport = match server.server_type {
            McpServerType::Stdio => "stdio",
            McpServerType::StreamHttp => "stream_http",
        };

        tracing::info!(
            event = "test_attempt",
            server_id = %id,
            server_name = %server.name,
            transport = transport,
            "Testing MCP server"
        );

        let config = McpManager::config_from_model(&server)?;
        let start = Instant::now();

        let test_result = tokio::time::timeout(Duration::from_secs(10), async {
            let client = McpClient::connect(&config).await?;
            let protocol_version = client.server_info().protocol_version.clone();
            let tools = client.list_tools().await?;
            let tool_count = tools.len() as u32;
            if let Some(health_tool) = tools.iter().find(|tool| {
                matches!(
                    tool.name.as_str(),
                    "ping" | "health" | "health_check" | "healthcheck"
                )
            }) {
                let requires_args = health_tool
                    .input_schema
                    .get("required")
                    .and_then(|value| value.as_array())
                    .map(|required| !required.is_empty())
                    .unwrap_or(false);
                if !requires_args {
                    client
                        .call_tool(&health_tool.name, serde_json::json!({}))
                        .await?;
                }
            }
            let _ = client.disconnect().await;
            Ok::<(String, u32), AppError>((protocol_version, tool_count))
        })
        .await;

        let latency_ms = start.elapsed().as_millis() as u64;

        let (status, protocol_version, tool_count) = match test_result {
            Ok(Ok((protocol_version, tool_count))) => {
                self.mark_server_connected(id)?;
                tracing::info!(
                    event = "test_success",
                    server_id = %id,
                    server_name = %server.name,
                    transport = transport,
                    latency_ms = latency_ms,
                    tool_count = tool_count,
                    protocol_version = %protocol_version,
                    "MCP server health check succeeded"
                );
                (
                    McpServerStatus::Connected,
                    Some(protocol_version),
                    Some(tool_count),
                )
            }
            Ok(Err(e)) => {
                let classified = Self::classify_error(&e.to_string());
                self.mark_server_connection_error(id, &classified)?;
                tracing::warn!(
                    event = "test_failure",
                    server_id = %id,
                    server_name = %server.name,
                    transport = transport,
                    latency_ms = latency_ms,
                    error_class = %classified.split(':').next().unwrap_or("transport"),
                    error = %classified,
                    "MCP server health check failed"
                );
                (McpServerStatus::Error(classified), None, None)
            }
            Err(_) => {
                let timeout_err = "transport: connection timeout".to_string();
                self.mark_server_connection_error(id, &timeout_err)?;
                tracing::warn!(
                    event = "test_failure",
                    server_id = %id,
                    server_name = %server.name,
                    transport = transport,
                    latency_ms = latency_ms,
                    error_class = "transport",
                    error = %timeout_err,
                    "MCP server health check timed out"
                );
                (McpServerStatus::Error(timeout_err), None, None)
            }
        };

        Ok(HealthCheckResult {
            server_id: id.to_string(),
            status,
            checked_at: chrono::Utc::now().to_rfc3339(),
            latency_ms: Some(latency_ms),
            protocol_version,
            tool_count,
        })
    }

    fn classify_error(raw: &str) -> String {
        let lower = raw.to_lowercase();
        let class = if lower.contains("unauthorized")
            || lower.contains("forbidden")
            || lower.contains("401")
            || lower.contains("403")
            || lower.contains("auth")
        {
            "auth"
        } else if lower.contains("protocol")
            || lower.contains("initialize")
            || lower.contains("tools/list")
            || lower.contains("jsonrpc")
        {
            "protocol"
        } else if lower.contains("header")
            || lower.contains("url")
            || lower.contains("schema")
            || lower.contains("validation")
        {
            "schema"
        } else {
            "transport"
        };

        format!("{}: {}", class, raw)
    }

    pub fn mark_server_connected(&self, id: &str) -> AppResult<()> {
        self.db.mark_mcp_server_connected(id)
    }

    pub fn mark_server_disconnected(&self, id: &str) -> AppResult<()> {
        self.db.mark_mcp_server_disconnected(id)
    }

    pub fn mark_server_connection_error(&self, id: &str, error: &str) -> AppResult<()> {
        self.db.mark_mcp_server_connection_error(id, error)
    }

    /// Import servers from Claude Desktop configuration.
    pub fn import_from_claude_desktop(&self) -> AppResult<ImportResult> {
        self.import_from_claude_desktop_with_options(false, McpImportConflictPolicy::Skip)
    }

    /// Import servers from Claude Desktop configuration with options.
    pub fn import_from_claude_desktop_with_options(
        &self,
        dry_run: bool,
        conflict_policy: McpImportConflictPolicy,
    ) -> AppResult<ImportResult> {
        let config_path = Self::get_claude_desktop_config_path();

        if !config_path.exists() {
            return Ok(ImportResult {
                added: 0,
                skipped: 0,
                failed: 0,
                servers: vec![],
                errors: vec!["Claude Desktop config not found".to_string()],
                will_add: vec![],
                will_skip: vec![],
                will_fail: vec![],
            });
        }

        let content = std::fs::read_to_string(&config_path)?;
        self.import_from_json_str(&content, dry_run, conflict_policy)
    }

    /// Import servers from a JSON configuration file path.
    pub fn import_from_file(&self, path: &str) -> AppResult<ImportResult> {
        self.import_from_file_with_options(path, false, McpImportConflictPolicy::Skip)
    }

    /// Import servers from a JSON configuration file path with options.
    pub fn import_from_file_with_options(
        &self,
        path: &str,
        dry_run: bool,
        conflict_policy: McpImportConflictPolicy,
    ) -> AppResult<ImportResult> {
        let content = std::fs::read_to_string(path)?;
        self.import_from_json_str(&content, dry_run, conflict_policy)
    }

    fn import_from_json_str(
        &self,
        content: &str,
        dry_run: bool,
        conflict_policy: McpImportConflictPolicy,
    ) -> AppResult<ImportResult> {
        let config: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| AppError::parse(format!("Invalid JSON: {}", e)))?;

        let mut result = ImportResult {
            added: 0,
            skipped: 0,
            failed: 0,
            servers: vec![],
            errors: vec![],
            will_add: vec![],
            will_skip: vec![],
            will_fail: vec![],
        };

        if let Some(mcp_servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, server_config) in mcp_servers {
                self.apply_import_result(
                    name,
                    server_config,
                    dry_run,
                    conflict_policy,
                    &mut result,
                );
            }
            return Ok(result);
        }

        if let Some(servers) = config.get("servers").and_then(|v| v.as_array()) {
            for item in servers {
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unnamed");
                self.apply_import_result(name, item, dry_run, conflict_policy, &mut result);
            }
            return Ok(result);
        }

        result.errors.push(
            "No MCP server definitions found (expected `mcpServers` object or `servers` array)"
                .to_string(),
        );
        if dry_run {
            result.will_fail.push(
                "No MCP server definitions found (expected `mcpServers` object or `servers` array)"
                    .to_string(),
            );
        }
        Ok(result)
    }

    fn apply_import_result(
        &self,
        name: &str,
        server_config: &serde_json::Value,
        dry_run: bool,
        conflict_policy: McpImportConflictPolicy,
        result: &mut ImportResult,
    ) {
        match self.import_single_server(name, server_config, dry_run, conflict_policy) {
            Ok(ImportSingleResult::Added(server_name)) => {
                result.added += 1;
                result.servers.push(server_name.clone());
                if dry_run {
                    result.will_add.push(server_name);
                }
            }
            Ok(ImportSingleResult::Skipped(reason)) => {
                result.skipped += 1;
                result.will_skip.push(reason);
            }
            Err(e) => {
                result.failed += 1;
                result.errors.push(format!("{}: {}", name, e));
                if dry_run {
                    result.will_fail.push(format!("{}: {}", name, e));
                }
            }
        }
    }

    fn get_claude_desktop_config_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                return PathBuf::from(appdata).join("Claude").join("config.json");
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(home) = dirs::home_dir() {
                return home
                    .join("Library")
                    .join("Application Support")
                    .join("Claude")
                    .join("config.json");
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(home) = dirs::home_dir() {
                return home
                    .join(".config")
                    .join("claude-desktop")
                    .join("config.json");
            }
        }

        PathBuf::from("config.json")
    }

    fn import_single_server(
        &self,
        name: &str,
        config: &serde_json::Value,
        dry_run: bool,
        conflict_policy: McpImportConflictPolicy,
    ) -> AppResult<ImportSingleResult> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::validation("Server name is required".to_string()));
        }
        let existing = self.db.get_mcp_server_by_name_case_insensitive(name)?;
        if matches!(conflict_policy, McpImportConflictPolicy::Skip) && existing.is_some() {
            return Ok(ImportSingleResult::Skipped(format!(
                "{}: duplicate_name",
                name
            )));
        }

        let command = config
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let args: Vec<String> = config
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let (env, redacted_env_keys) = parse_secret_map(config.get("env"));
        let (headers, redacted_header_keys) = parse_secret_map(config.get("headers"));

        let url = config
            .get("url")
            .or_else(|| config.get("baseUrl"))
            .or_else(|| config.get("endpoint"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let server_type = if url.is_some() {
            McpServerType::StreamHttp
        } else if command.is_some() {
            McpServerType::Stdio
        } else {
            return Err(AppError::validation(
                "Server must have either command or url".to_string(),
            ));
        };

        let target_name = match conflict_policy {
            McpImportConflictPolicy::Skip => name.to_string(),
            McpImportConflictPolicy::Rename => {
                if existing.is_some() {
                    self.resolve_unique_import_name(name)?
                } else {
                    name.to_string()
                }
            }
            McpImportConflictPolicy::Replace => name.to_string(),
        };

        if matches!(conflict_policy, McpImportConflictPolicy::Replace) {
            if let Some(existing) = existing {
                let mut merged_env = env;
                let mut merged_headers = headers;

                if (!redacted_env_keys.is_empty() || !redacted_header_keys.is_empty()) && !dry_run {
                    let existing_with_secrets =
                        self.get_server(&existing.id)?.ok_or_else(|| {
                            AppError::not_found(format!(
                                "Cannot load existing MCP server for replace: {}",
                                existing.id
                            ))
                        })?;
                    let existing_env =
                        if existing_with_secrets.env.is_empty() && existing.has_env_secret {
                            self.read_secret_map_from_keyring(&Self::env_secret_key(&existing.id))
                        } else {
                            existing_with_secrets.env.clone()
                        };
                    let existing_headers = if existing_with_secrets.headers.is_empty()
                        && existing.has_headers_secret
                    {
                        self.read_secret_map_from_keyring(&Self::headers_secret_key(&existing.id))
                    } else {
                        existing_with_secrets.headers.clone()
                    };
                    for key in &redacted_env_keys {
                        if let Some(value) = existing_env.get(key) {
                            merged_env.insert(key.clone(), value.clone());
                        }
                    }
                    for key in &redacted_header_keys {
                        if let Some(value) = existing_headers.get(key) {
                            merged_headers.insert(key.clone(), value.clone());
                        }
                    }
                }

                let update_request = UpdateMcpServerRequest {
                    name: Some(target_name.clone()),
                    server_type: Some(server_type.clone()),
                    command: command.clone(),
                    clear_command: !matches!(server_type, McpServerType::Stdio),
                    args: Some(args.clone()),
                    env: Some(merged_env),
                    url: url.clone(),
                    clear_url: !matches!(server_type, McpServerType::StreamHttp),
                    headers: Some(merged_headers),
                    enabled: None,
                    auto_connect: None,
                };
                if !dry_run {
                    self.update_server(&existing.id, update_request)?;
                }
                return Ok(ImportSingleResult::Added(target_name));
            }
        }

        let request = CreateMcpServerRequest {
            name: target_name.clone(),
            server_type,
            command,
            args: Some(args),
            env: Some(env),
            url,
            headers: Some(headers),
            auto_connect: Some(true),
        };

        if !dry_run {
            self.add_server(request)?;
        }
        Ok(ImportSingleResult::Added(target_name))
    }

    fn resolve_unique_import_name(&self, base: &str) -> AppResult<String> {
        if self
            .db
            .get_mcp_server_by_name_case_insensitive(base)?
            .is_none()
        {
            return Ok(base.to_string());
        }

        let mut suffix = 2u32;
        loop {
            let candidate = format!("{} ({})", base, suffix);
            if self
                .db
                .get_mcp_server_by_name_case_insensitive(&candidate)?
                .is_none()
            {
                return Ok(candidate);
            }
            suffix += 1;
        }
    }

    fn ensure_name_unique(&self, name: &str, ignore_id: Option<&str>) -> AppResult<()> {
        let duplicate = self
            .db
            .find_mcp_server_name_conflict_case_insensitive(name, ignore_id)?;
        if duplicate {
            return Err(AppError::validation(format!(
                "MCP server name already exists: {}",
                name
            )));
        }
        Ok(())
    }
}

fn managed_root_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".plan-cascade"))
}

fn extract_managed_paths(value: &serde_json::Value) -> Vec<PathBuf> {
    const KEYS: [&str; 3] = ["wrapper_path", "tool_dir", "venv_dir"];
    KEYS.iter()
        .filter_map(|key| value.get(*key))
        .filter_map(|raw| raw.as_str())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() || is_managed_relative(path))
        .map(normalize_managed_path)
        .collect()
}

fn is_managed_relative(path: &Path) -> bool {
    use std::ffi::OsStr;
    let first = path.components().next();
    matches!(
        first,
        Some(std::path::Component::Normal(part))
            if part == OsStr::new(".plan-cascade")
                || part == OsStr::new("mcp-tools")
                || part == OsStr::new("mcp-launchers")
    )
}

fn normalize_managed_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else if let Some(home) = dirs::home_dir() {
        home.join(path)
    } else {
        path
    }
}

enum ImportSingleResult {
    Added(String),
    Skipped(String),
}

fn parse_secret_map(value: Option<&serde_json::Value>) -> (HashMap<String, String>, Vec<String>) {
    let mut values = HashMap::new();
    let mut redacted = Vec::new();

    let Some(object) = value.and_then(|raw| raw.as_object()) else {
        return (values, redacted);
    };

    for (key, raw) in object {
        let Some(text) = raw.as_str() else {
            continue;
        };
        if text == REDACTED_PLACEHOLDER {
            redacted.push(key.clone());
            continue;
        }
        values.insert(key.clone(), text.to_string());
    }
    (values, redacted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::{HashMap, HashSet};
    use std::sync::{Mutex, OnceLock};

    fn keyring_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn test_generate_id() {
        let id1 = McpService::generate_id();
        let id2 = McpService::generate_id();
        assert!(id1.starts_with("mcp-"));
        assert!(id2.starts_with("mcp-"));
    }

    #[test]
    fn test_generate_id_uniqueness() {
        let mut ids = HashSet::new();
        for _ in 0..2048 {
            ids.insert(McpService::generate_id());
        }
        assert_eq!(ids.len(), 2048);
    }

    #[test]
    fn test_reconcile_runtime_statuses_marks_stale_connected_as_disconnected() {
        let db = Database::new_in_memory().unwrap();
        let service = McpService::with_database(db);
        let server = service
            .add_server(CreateMcpServerRequest {
                name: "Reconcile Test".to_string(),
                server_type: McpServerType::Stdio,
                command: Some("echo".to_string()),
                args: Some(vec!["ok".to_string()]),
                env: Some(HashMap::new()),
                url: None,
                headers: Some(HashMap::new()),
                auto_connect: Some(true),
            })
            .unwrap();
        service.mark_server_connected(&server.id).unwrap();

        let (reconciled, runtime_connected) =
            service.reconcile_runtime_statuses(&HashSet::new()).unwrap();
        assert_eq!(reconciled, 1);
        assert_eq!(runtime_connected, 0);

        let refreshed = service.get_server(&server.id).unwrap().unwrap();
        assert!(matches!(refreshed.status, McpServerStatus::Disconnected));
    }

    #[test]
    fn test_import_result_reports_added_skipped_failed_and_skip_items() {
        let db = Database::new_in_memory().unwrap();
        let service = McpService::with_database(db);
        service
            .add_server(CreateMcpServerRequest {
                name: "Existing".to_string(),
                server_type: McpServerType::Stdio,
                command: Some("echo".to_string()),
                args: Some(vec![]),
                env: Some(HashMap::new()),
                url: None,
                headers: Some(HashMap::new()),
                auto_connect: Some(true),
            })
            .unwrap();

        let payload = json!({
            "mcpServers": {
                "Existing": { "command": "echo", "args": [] },
                "Fresh": { "command": "echo", "args": ["ok"] },
                "Broken": { "args": [] }
            }
        });
        let content = serde_json::to_string(&payload).unwrap();

        let result = service
            .import_from_json_str(&content, false, McpImportConflictPolicy::Skip)
            .unwrap();
        assert_eq!(result.added, 1);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.failed, 1);
        assert!(result.servers.iter().any(|name| name == "Fresh"));
        assert!(result
            .will_skip
            .iter()
            .any(|entry| entry.contains("Existing")));
        assert!(result
            .errors
            .iter()
            .any(|entry| entry.starts_with("Broken:")));
    }

    #[test]
    fn test_import_conflict_policy_rename_creates_unique_server_name() {
        let _guard = keyring_test_guard();
        let db = Database::new_in_memory().unwrap();
        let service = McpService::with_database(db);
        service
            .add_server(CreateMcpServerRequest {
                name: "Existing".to_string(),
                server_type: McpServerType::Stdio,
                command: Some("echo".to_string()),
                args: Some(vec![]),
                env: Some(HashMap::new()),
                url: None,
                headers: Some(HashMap::new()),
                auto_connect: Some(true),
            })
            .unwrap();

        let payload = json!({
            "mcpServers": {
                "Existing": { "command": "echo", "args": ["ok"] }
            }
        });
        let content = serde_json::to_string(&payload).unwrap();

        let result = service
            .import_from_json_str(&content, false, McpImportConflictPolicy::Rename)
            .unwrap();
        assert_eq!(result.added, 1);
        assert_eq!(result.skipped, 0);
        assert!(result.servers.iter().any(|name| name == "Existing (2)"));

        let names: Vec<String> = service
            .list_servers()
            .unwrap()
            .into_iter()
            .map(|server| server.name)
            .collect();
        assert!(names.iter().any(|name| name == "Existing"));
        assert!(names.iter().any(|name| name == "Existing (2)"));
    }

    #[test]
    fn test_import_conflict_policy_replace_overwrites_existing_server() {
        let _guard = keyring_test_guard();
        let db = Database::new_in_memory().unwrap();
        let service = McpService::with_database(db);
        let existing = service
            .add_server(CreateMcpServerRequest {
                name: "Existing".to_string(),
                server_type: McpServerType::Stdio,
                command: Some("old".to_string()),
                args: Some(vec!["v1".to_string()]),
                env: Some(HashMap::from([(
                    "API_KEY".to_string(),
                    "old-secret".to_string(),
                )])),
                url: None,
                headers: Some(HashMap::new()),
                auto_connect: Some(true),
            })
            .unwrap();

        let payload = json!({
            "mcpServers": {
                "Existing": {
                    "command": "new",
                    "args": ["v2"],
                    "env": {
                        "API_KEY": "new-secret",
                        "TOKEN": "token-v2"
                    }
                }
            }
        });
        let content = serde_json::to_string(&payload).unwrap();

        let result = service
            .import_from_json_str(&content, false, McpImportConflictPolicy::Replace)
            .unwrap();
        assert_eq!(result.added, 1);
        assert_eq!(result.skipped, 0);

        let updated = service.get_server(&existing.id).unwrap().unwrap();
        assert_eq!(updated.command.as_deref(), Some("new"));
        assert_eq!(updated.args, vec!["v2".to_string()]);
        assert_eq!(
            updated.env.get("API_KEY").map(String::as_str),
            Some("new-secret")
        );
        assert_eq!(
            updated.env.get("TOKEN").map(String::as_str),
            Some("token-v2")
        );
    }

    #[test]
    fn test_import_replace_redacted_keeps_existing_secret_values() {
        let _guard = keyring_test_guard();
        let db = Database::new_in_memory().unwrap();
        let service = McpService::with_database(db);
        let existing = service
            .add_server(CreateMcpServerRequest {
                name: "Existing".to_string(),
                server_type: McpServerType::Stdio,
                command: Some("old".to_string()),
                args: Some(vec![]),
                env: Some(HashMap::from([
                    ("API_KEY".to_string(), "old-secret".to_string()),
                    ("VISIBLE".to_string(), "old-value".to_string()),
                ])),
                url: None,
                headers: Some(HashMap::new()),
                auto_connect: Some(true),
            })
            .unwrap();

        let payload = json!({
            "mcpServers": {
                "Existing": {
                    "command": "new",
                    "args": ["next"],
                    "env": {
                        "API_KEY": "__REDACTED__",
                        "VISIBLE": "new-value"
                    }
                }
            }
        });
        let content = serde_json::to_string(&payload).unwrap();

        let result = service
            .import_from_json_str(&content, false, McpImportConflictPolicy::Replace)
            .unwrap();
        assert_eq!(result.added, 1);
        assert_eq!(result.failed, 0);

        let updated = service.get_server(&existing.id).unwrap().unwrap();
        assert_eq!(updated.command.as_deref(), Some("new"));
        assert_eq!(
            updated.env.get("API_KEY").map(String::as_str),
            Some("old-secret")
        );
        assert_eq!(
            updated.env.get("VISIBLE").map(String::as_str),
            Some("new-value")
        );
    }

    #[test]
    fn test_managed_relative_path_detection() {
        assert!(is_managed_relative(Path::new(
            ".plan-cascade/mcp-launchers/x.sh"
        )));
        assert!(is_managed_relative(Path::new("mcp-tools/node/pkg")));
        assert!(is_managed_relative(Path::new("mcp-launchers/a.cmd")));
        assert!(!is_managed_relative(Path::new("../outside")));
        assert!(!is_managed_relative(Path::new("Downloads/something")));
    }

    #[test]
    fn test_extract_managed_paths_filters_unknown_fields() {
        let payload = json!({
            "wrapper_path": "/tmp/mcp-wrapper.sh",
            "tool_dir": "mcp-tools/node/test",
            "venv_dir": "../escape",
            "other_path": "/tmp/ignored"
        });
        let paths = extract_managed_paths(&payload);
        let rendered: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        assert!(rendered
            .iter()
            .any(|value| value.contains("/tmp/mcp-wrapper.sh")));
        assert!(rendered
            .iter()
            .any(|value| value.contains("mcp-tools/node/test")));
        assert!(!rendered.iter().any(|value| value.contains("escape")));
        assert!(!rendered.iter().any(|value| value.contains("ignored")));
    }
}
