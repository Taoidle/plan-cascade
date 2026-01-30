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
    /// Server-Sent Events based server (HTTP connection)
    Sse,
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
    /// Server type (stdio or sse)
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
    /// URL for SSE server
    pub url: Option<String>,
    /// HTTP headers for SSE server
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Whether the server is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Current connection status
    #[serde(default)]
    pub status: McpServerStatus,
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
            enabled: true,
            status: McpServerStatus::Unknown,
            last_checked: None,
            created_at: None,
            updated_at: None,
        }
    }

    /// Create a new SSE-type MCP server
    pub fn new_sse(id: String, name: String, url: String) -> Self {
        Self {
            id,
            name,
            server_type: McpServerType::Sse,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            url: Some(url),
            headers: HashMap::new(),
            enabled: true,
            status: McpServerStatus::Unknown,
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
                if self.command.is_none() || self.command.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                    return Err("Command is required for stdio servers".to_string());
                }
            }
            McpServerType::Sse => {
                if self.url.is_none() || self.url.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                    return Err("URL is required for SSE servers".to_string());
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
}

/// Request to update an existing MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMcpServerRequest {
    pub name: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub enabled: Option<bool>,
}

/// Result of health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub server_id: String,
    pub status: McpServerStatus,
    pub checked_at: String,
}

/// Result of importing servers from Claude Desktop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub added: u32,
    pub skipped: u32,
    pub failed: u32,
    pub servers: Vec<String>,
    pub errors: Vec<String>,
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
    fn test_new_sse_server() {
        let server = McpServer::new_sse(
            "sse-id".to_string(),
            "SSE Server".to_string(),
            "http://localhost:8080".to_string(),
        );

        assert_eq!(server.server_type, McpServerType::Sse);
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
    fn test_validate_sse_server() {
        let server = McpServer::new_sse(
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
