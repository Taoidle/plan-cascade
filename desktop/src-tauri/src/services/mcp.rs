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
use crate::utils::error::{AppError, AppResult};

/// Service for managing MCP servers
pub struct McpService {
    db: Database,
}

impl McpService {
    /// Create a new MCP service
    pub fn new() -> AppResult<Self> {
        let db = Database::new()?;
        Ok(Self { db })
    }

    /// Create with existing database instance
    pub fn with_database(db: Database) -> Self {
        Self { db }
    }

    /// Generate a unique ID for a new server
    fn generate_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("mcp-{}", timestamp)
    }

    /// List all MCP servers
    pub fn list_servers(&self) -> AppResult<Vec<McpServer>> {
        self.db.list_mcp_servers()
    }

    /// Get a server by ID
    pub fn get_server(&self, id: &str) -> AppResult<Option<McpServer>> {
        self.db.get_mcp_server(id)
    }

    /// Add a new MCP server
    pub fn add_server(&self, request: CreateMcpServerRequest) -> AppResult<McpServer> {
        let id = Self::generate_id();

        let server = McpServer {
            id: id.clone(),
            name: request.name,
            server_type: request.server_type,
            command: request.command,
            args: request.args.unwrap_or_default(),
            env: request.env.unwrap_or_default(),
            url: request.url,
            headers: request.headers.unwrap_or_default(),
            enabled: true,
            status: McpServerStatus::Unknown,
            last_checked: None,
            created_at: None,
            updated_at: None,
        };

        server.validate().map_err(|e| AppError::validation(e))?;

        self.db.insert_mcp_server(&server)?;
        self.db.get_mcp_server(&id)?.ok_or_else(|| {
            AppError::database("Failed to retrieve newly created server".to_string())
        })
    }

    /// Update an existing MCP server
    pub fn update_server(&self, id: &str, request: UpdateMcpServerRequest) -> AppResult<McpServer> {
        let mut server = self
            .db
            .get_mcp_server(id)?
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
        if let Some(env) = request.env {
            server.env = env;
        }
        if let Some(url) = request.url {
            server.url = Some(url);
        }
        if let Some(headers) = request.headers {
            server.headers = headers;
        }
        if let Some(enabled) = request.enabled {
            server.enabled = enabled;
        }

        server.validate().map_err(|e| AppError::validation(e))?;

        self.db.update_mcp_server(&server)?;
        Ok(server)
    }

    /// Remove an MCP server
    pub fn remove_server(&self, id: &str) -> AppResult<()> {
        self.db.delete_mcp_server(id)
    }

    /// Toggle server enabled status
    pub fn toggle_server(&self, id: &str, enabled: bool) -> AppResult<McpServer> {
        self.db.toggle_mcp_server_enabled(id, enabled)?;
        self.db
            .get_mcp_server(id)?
            .ok_or_else(|| AppError::not_found(format!("Server not found: {}", id)))
    }

    /// Test a server connection
    pub async fn test_server(&self, id: &str) -> AppResult<HealthCheckResult> {
        let server = self
            .db
            .get_mcp_server(id)?
            .ok_or_else(|| AppError::not_found(format!("Server not found: {}", id)))?;

        let status = match server.server_type {
            McpServerType::Stdio => self.test_stdio_server(&server).await,
            McpServerType::Sse => self.test_sse_server(&server).await,
        };

        // Update status in database
        self.db.update_mcp_server_status(id, &status)?;

        let checked_at = chrono::Utc::now().to_rfc3339();

        Ok(HealthCheckResult {
            server_id: id.to_string(),
            status,
            checked_at,
        })
    }

    /// Test a stdio-type server by spawning it briefly
    async fn test_stdio_server(&self, server: &McpServer) -> McpServerStatus {
        let command = match &server.command {
            Some(cmd) => cmd.clone(),
            None => return McpServerStatus::Error("No command specified".to_string()),
        };

        let args = server.args.clone();
        let env = server.env.clone();

        // Run spawn in a blocking task with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                // Build environment variables
                let mut env_vars: HashMap<String, String> = std::env::vars().collect();
                for (key, value) in &env {
                    env_vars.insert(key.clone(), value.clone());
                }

                // Try to spawn the process
                let spawn_result = std::process::Command::new(&command)
                    .args(&args)
                    .envs(&env_vars)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn();

                match spawn_result {
                    Ok(mut child) => {
                        // Process spawned successfully, kill it
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

    /// Test an SSE-type server by making an HTTP request
    async fn test_sse_server(&self, server: &McpServer) -> McpServerStatus {
        let url = match &server.url {
            Some(u) => u,
            None => return McpServerStatus::Error("No URL specified".to_string()),
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => return McpServerStatus::Error(format!("HTTP client error: {}", e)),
        };

        let mut request = client.get(url);

        // Add custom headers
        for (key, value) in &server.headers {
            request = request.header(key, value);
        }

        match request.send().await {
            Ok(response) => {
                if response.status().is_success() || response.status().is_informational() {
                    McpServerStatus::Connected
                } else {
                    McpServerStatus::Error(format!("HTTP {}", response.status()))
                }
            }
            Err(e) => McpServerStatus::Error(format!("Request failed: {}", e)),
        }
    }

    /// Import servers from Claude Desktop configuration
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
        let config: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| AppError::parse(format!("Invalid JSON: {}", e)))?;

        let mut result = ImportResult {
            added: 0,
            skipped: 0,
            failed: 0,
            servers: vec![],
            errors: vec![],
        };

        // Parse mcpServers object
        if let Some(mcp_servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, server_config) in mcp_servers {
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
        }

        Ok(result)
    }

    /// Get the Claude Desktop config path based on platform
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
                return home.join(".config").join("claude-desktop").join("config.json");
            }
        }

        // Fallback
        PathBuf::from("config.json")
    }

    /// Import a single server from Claude Desktop config
    fn import_single_server(
        &self,
        name: &str,
        config: &serde_json::Value,
    ) -> AppResult<ImportSingleResult> {
        // Check for duplicate
        if self.db.get_mcp_server_by_name(name)?.is_some() {
            return Ok(ImportSingleResult::Skipped("Already exists".to_string()));
        }

        // Parse server config
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

        let url = config
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Determine server type
        let server_type = if url.is_some() {
            McpServerType::Sse
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
            headers: None,
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
        // IDs should be different (unless generated in same millisecond)
    }
}
