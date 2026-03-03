//! MCP Server Models
//!
//! Data models for MCP (Model Context Protocol) server management.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of MCP server connection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum McpServerType {
    /// Standard I/O based server (spawns a process)
    Stdio,
    /// Streamable HTTP based server (HTTP connection)
    #[serde(rename = "stream_http", alias = "sse")]
    StreamHttp,
}

impl Default for McpServerType {
    fn default() -> Self {
        Self::Stdio
    }
}

/// Status of an MCP server connection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum McpServerStatus {
    /// Server is connected and responding
    Connected,
    /// Server is not connected or not checked
    Disconnected,
    /// Server check failed with error
    Error(String),
    /// Server status is unknown (never checked)
    Unknown,
}

impl Default for McpServerStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// MCP Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Server type (stdio or stream_http)
    #[serde(default)]
    pub server_type: McpServerType,
    /// Command to run (for stdio)
    pub command: Option<String>,
    /// Arguments for the command (for stdio)
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables (for stdio)
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// URL for Streamable HTTP server
    pub url: Option<String>,
    /// HTTP headers for Streamable HTTP server
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Whether environment variables are stored in secret storage
    #[serde(default)]
    pub has_env_secret: bool,
    /// Whether headers are stored in secret storage
    #[serde(default)]
    pub has_headers_secret: bool,
    /// Whether the server is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Whether this server should auto-connect on app startup
    #[serde(default = "default_auto_connect")]
    pub auto_connect: bool,
    /// Current connection status
    #[serde(default)]
    pub status: McpServerStatus,
    /// Last connection error message (if any)
    #[serde(default)]
    pub last_error: Option<String>,
    /// Last successful connection timestamp
    #[serde(default)]
    pub last_connected_at: Option<String>,
    /// Connection retry count (for diagnostics)
    #[serde(default)]
    pub retry_count: u32,
    /// Last time the server was checked
    pub last_checked: Option<String>,
    /// When the server was created
    pub created_at: Option<String>,
    /// When the server was last updated
    pub updated_at: Option<String>,
}

fn default_enabled() -> bool {
    true
}

fn default_auto_connect() -> bool {
    true
}

impl McpServer {
    /// Create a new stdio-type MCP server
    pub fn new_stdio(id: String, name: String, command: String, args: Vec<String>) -> Self {
        Self {
            id,
            name,
            server_type: McpServerType::Stdio,
            command: Some(command),
            args,
            env: HashMap::new(),
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
            created_at: None,
            updated_at: None,
        }
    }

    /// Create a new Streamable HTTP-type MCP server
    pub fn new_stream_http(id: String, name: String, url: String) -> Self {
        Self {
            id,
            name,
            server_type: McpServerType::StreamHttp,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            url: Some(url),
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
            created_at: None,
            updated_at: None,
        }
    }

    /// Validate the server configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Server name is required".to_string());
        }

        match self.server_type {
            McpServerType::Stdio => {
                if self.command.is_none()
                    || self.command.as_ref().map(|s| s.is_empty()).unwrap_or(true)
                {
                    return Err("Command is required for stdio servers".to_string());
                }
            }
            McpServerType::StreamHttp => {
                if self.url.is_none() || self.url.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                    return Err("URL is required for stream_http servers".to_string());
                }
            }
        }

        Ok(())
    }
}

/// Request to create a new MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMcpServerRequest {
    pub name: String,
    pub server_type: McpServerType,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub auto_connect: Option<bool>,
}

/// Request to update an existing MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMcpServerRequest {
    pub name: Option<String>,
    pub server_type: Option<McpServerType>,
    pub command: Option<String>,
    /// Explicitly clear command (set to null) when true.
    #[serde(default)]
    pub clear_command: bool,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub url: Option<String>,
    /// Explicitly clear URL (set to null) when true.
    #[serde(default)]
    pub clear_url: bool,
    pub headers: Option<HashMap<String, String>>,
    pub enabled: Option<bool>,
    pub auto_connect: Option<bool>,
}

/// Result of health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub server_id: String,
    pub status: McpServerStatus,
    pub checked_at: String,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub protocol_version: Option<String>,
    #[serde(default)]
    pub tool_count: Option<u32>,
}

/// Result of importing servers from Claude Desktop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub added: u32,
    pub skipped: u32,
    pub failed: u32,
    pub servers: Vec<String>,
    pub errors: Vec<String>,
    #[serde(default)]
    pub will_add: Vec<String>,
    #[serde(default)]
    pub will_skip: Vec<String>,
    #[serde(default)]
    pub will_fail: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_stdio_server() {
        let server = McpServer::new_stdio(
            "test-id".to_string(),
            "Test Server".to_string(),
            "node".to_string(),
            vec!["server.js".to_string()],
        );

        assert_eq!(server.id, "test-id");
        assert_eq!(server.name, "Test Server");
        assert_eq!(server.server_type, McpServerType::Stdio);
        assert_eq!(server.command, Some("node".to_string()));
        assert!(server.enabled);
    }

    #[test]
    fn test_new_stream_http_server() {
        let server = McpServer::new_stream_http(
            "sse-id".to_string(),
            "SSE Server".to_string(),
            "http://localhost:8080".to_string(),
        );

        assert_eq!(server.server_type, McpServerType::StreamHttp);
        assert_eq!(server.url, Some("http://localhost:8080".to_string()));
    }

    #[test]
    fn test_validate_stdio_server() {
        let server = McpServer::new_stdio(
            "id".to_string(),
            "Name".to_string(),
            "cmd".to_string(),
            vec![],
        );
        assert!(server.validate().is_ok());

        let invalid = McpServer {
            command: None,
            ..server.clone()
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_validate_stream_http_server() {
        let server = McpServer::new_stream_http(
            "id".to_string(),
            "Name".to_string(),
            "http://localhost".to_string(),
        );
        assert!(server.validate().is_ok());

        let invalid = McpServer {
            url: None,
            ..server.clone()
        };
        assert!(invalid.validate().is_err());
    }
}
