//! MCP Service
//!
//! Business logic for MCP server management.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use crate::models::{
    CreateMcpServerRequest, HealthCheckResult, ImportResult, McpServer, McpServerStatus,
    McpServerType, UpdateMcpServerRequest,
};
use crate::storage::database::Database;
use crate::storage::KeyringService;
use crate::utils::error::{AppError, AppResult};

/// Service for managing MCP servers.
pub struct McpService {
    db: Database,
    keyring: KeyringService,
}

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
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("mcp-{}", timestamp)
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

    /// List all MCP servers.
    pub fn list_servers(&self) -> AppResult<Vec<McpServer>> {
        let mut servers = self.db.list_mcp_servers()?;
        for server in &mut servers {
            self.hydrate_sensitive(server);
        }
        Ok(servers)
    }

    /// List enabled MCP servers that should auto-connect.
    pub fn list_enabled_auto_connect_servers(&self) -> AppResult<Vec<McpServer>> {
        Ok(self
            .list_servers()?
            .into_iter()
            .filter(|s| s.enabled && s.auto_connect)
            .collect())
    }

    /// Get a server by ID.
    pub fn get_server(&self, id: &str) -> AppResult<Option<McpServer>> {
        let mut server = match self.db.get_mcp_server(id)? {
            Some(s) => s,
            None => return Ok(None),
        };
        self.hydrate_sensitive(&mut server);
        Ok(Some(server))
    }

    /// Add a new MCP server.
    pub fn add_server(&self, request: CreateMcpServerRequest) -> AppResult<McpServer> {
        let id = Self::generate_id();

        let mut server = McpServer {
            id: id.clone(),
            name: request.name,
            server_type: request.server_type,
            command: request.command,
            args: request.args.unwrap_or_default(),
            env: request.env.unwrap_or_default(),
            url: request.url,
            headers: request.headers.unwrap_or_default(),
            has_env_secret: false,
            has_headers_secret: false,
            enabled: true,
            auto_connect: true,
            status: McpServerStatus::Unknown,
            last_error: None,
            last_connected_at: None,
            retry_count: 0,
            last_checked: None,
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
            server.name = name;
        }
        if let Some(command) = request.command {
            server.command = Some(command);
        }
        if let Some(args) = request.args {
            server.args = args;
        }
        let env_updated = request.env.is_some();
        if let Some(env) = request.env {
            server.env = env;
        }
        if let Some(url) = request.url {
            server.url = Some(url);
        }
        let headers_updated = request.headers.is_some();
        if let Some(headers) = request.headers {
            server.headers = headers;
        }
        if let Some(enabled) = request.enabled {
            server.enabled = enabled;
        }

        server.validate().map_err(AppError::validation)?;
        self.persist_sensitive(&mut server, env_updated, headers_updated)?;

        self.db.update_mcp_server(&server)?;
        self.get_server(id)?
            .ok_or_else(|| AppError::not_found(format!("Server not found: {}", id)))
    }

    /// Remove an MCP server.
    pub fn remove_server(&self, id: &str) -> AppResult<()> {
        self.db.delete_mcp_server(id)?;
        self.clear_sensitive(id);
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

        let status = match server.server_type {
            McpServerType::Stdio => self.test_stdio_server(&server).await,
            McpServerType::StreamHttp => self.test_stream_http_server(&server).await,
        };

        self.db.update_mcp_server_status(id, &status)?;

        Ok(HealthCheckResult {
            server_id: id.to_string(),
            status,
            checked_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    async fn test_stdio_server(&self, server: &McpServer) -> McpServerStatus {
        let command = match &server.command {
            Some(cmd) => cmd.clone(),
            None => return McpServerStatus::Error("No command specified".to_string()),
        };

        let args = server.args.clone();
        let env = server.env.clone();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                let mut env_vars: HashMap<String, String> = std::env::vars().collect();
                for (key, value) in &env {
                    env_vars.insert(key.clone(), value.clone());
                }

                let spawn_result = std::process::Command::new(&command)
                    .args(&args)
                    .envs(&env_vars)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn();

                match spawn_result {
                    Ok(mut child) => {
                        let _ = child.kill();
                        Ok(())
                    }
                    Err(e) => Err(format!("Failed to spawn: {}", e)),
                }
            }),
        )
        .await;

        match result {
            Ok(Ok(Ok(()))) => McpServerStatus::Connected,
            Ok(Ok(Err(e))) => McpServerStatus::Error(e),
            Ok(Err(e)) => McpServerStatus::Error(format!("Task error: {}", e)),
            Err(_) => McpServerStatus::Error("Spawn timeout".to_string()),
        }
    }

    async fn test_stream_http_server(&self, server: &McpServer) -> McpServerStatus {
        let url = match &server.url {
            Some(u) => u,
            None => return McpServerStatus::Error("No URL specified".to_string()),
        };

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => return McpServerStatus::Error(format!("HTTP client error: {}", e)),
        };

        let mut request = client.get(url);
        for (key, value) in &server.headers {
            request = request.header(key, value);
        }

        match request.send().await {
            Ok(response)
                if response.status().is_success() || response.status().is_informational() =>
            {
                McpServerStatus::Connected
            }
            Ok(response) => McpServerStatus::Error(format!("HTTP {}", response.status())),
            Err(e) => McpServerStatus::Error(format!("Request failed: {}", e)),
        }
    }

    /// Import servers from Claude Desktop configuration.
    pub fn import_from_claude_desktop(&self) -> AppResult<ImportResult> {
        let config_path = Self::get_claude_desktop_config_path();

        if !config_path.exists() {
            return Ok(ImportResult {
                added: 0,
                skipped: 0,
                failed: 0,
                servers: vec![],
                errors: vec!["Claude Desktop config not found".to_string()],
            });
        }

        let content = std::fs::read_to_string(&config_path)?;
        self.import_from_json_str(&content)
    }

    /// Import servers from a JSON configuration file path.
    pub fn import_from_file(&self, path: &str) -> AppResult<ImportResult> {
        let content = std::fs::read_to_string(path)?;
        self.import_from_json_str(&content)
    }

    fn import_from_json_str(&self, content: &str) -> AppResult<ImportResult> {
        let config: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| AppError::parse(format!("Invalid JSON: {}", e)))?;

        let mut result = ImportResult {
            added: 0,
            skipped: 0,
            failed: 0,
            servers: vec![],
            errors: vec![],
        };

        if let Some(mcp_servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, server_config) in mcp_servers {
                self.apply_import_result(name, server_config, &mut result);
            }
            return Ok(result);
        }

        if let Some(servers) = config.get("servers").and_then(|v| v.as_array()) {
            for item in servers {
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unnamed");
                self.apply_import_result(name, item, &mut result);
            }
            return Ok(result);
        }

        result.errors.push(
            "No MCP server definitions found (expected `mcpServers` object or `servers` array)"
                .to_string(),
        );
        Ok(result)
    }

    fn apply_import_result(
        &self,
        name: &str,
        server_config: &serde_json::Value,
        result: &mut ImportResult,
    ) {
        match self.import_single_server(name, server_config) {
            Ok(ImportSingleResult::Added(server_name)) => {
                result.added += 1;
                result.servers.push(server_name);
            }
            Ok(ImportSingleResult::Skipped(reason)) => {
                result.skipped += 1;
                result.errors.push(format!("{}: {}", name, reason));
            }
            Err(e) => {
                result.failed += 1;
                result.errors.push(format!("{}: {}", name, e));
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
    ) -> AppResult<ImportSingleResult> {
        if self.db.get_mcp_server_by_name(name)?.is_some() {
            return Ok(ImportSingleResult::Skipped("Already exists".to_string()));
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

        let env: HashMap<String, String> = config
            .get("env")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let headers: HashMap<String, String> = config
            .get("headers")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

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

        let request = CreateMcpServerRequest {
            name: name.to_string(),
            server_type,
            command,
            args: Some(args),
            env: Some(env),
            url,
            headers: Some(headers),
        };

        self.add_server(request)?;
        Ok(ImportSingleResult::Added(name.to_string()))
    }
}

enum ImportSingleResult {
    Added(String),
    Skipped(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id1 = McpService::generate_id();
        let id2 = McpService::generate_id();
        assert!(id1.starts_with("mcp-"));
        assert!(id2.starts_with("mcp-"));
    }
}
