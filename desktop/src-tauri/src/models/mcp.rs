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
    /// Whether this server is installed/managed by MCP catalog installer
    #[serde(default)]
    pub managed_install: bool,
    /// Catalog item identifier if managed install
    #[serde(default)]
    pub catalog_item_id: Option<String>,
    /// Trust level inherited from catalog item
    #[serde(default)]
    pub trust_level: Option<McpCatalogTrustLevel>,
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
            managed_install: false,
            catalog_item_id: None,
            trust_level: None,
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
            managed_install: false,
            catalog_item_id: None,
            trust_level: None,
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

/// Catalog trust level for a recommended MCP service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum McpCatalogTrustLevel {
    Official,
    Verified,
    Community,
}

/// Supported runtime kinds for MCP install requirements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum McpRuntimeKind {
    Node,
    Uv,
    Python,
    Docker,
}

/// Strategy kind used by the installer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpInstallStrategyKind {
    UvTool,
    PythonVenv,
    NodeManagedPkg,
    Docker,
    StreamHttpApiKey,
    StreamHttpApiKeyOptional,
    OauthBridgeMcpRemote,
    GoBinary,
}

/// Runtime requirement for an installation strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRequirement {
    pub runtime: McpRuntimeKind,
    #[serde(default)]
    pub min_version: Option<String>,
    #[serde(default)]
    pub optional: bool,
}

/// Verification contract executed after installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInstallVerification {
    #[serde(default = "default_true")]
    pub require_initialize: bool,
    #[serde(default = "default_true")]
    pub require_tools_list: bool,
}

fn default_true() -> bool {
    true
}

/// Strategy metadata for each catalog item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInstallStrategy {
    pub id: String,
    pub kind: McpInstallStrategyKind,
    pub priority: u32,
    #[serde(default)]
    pub requirements: Vec<RuntimeRequirement>,
    /// Strategy recipe is intentionally structured JSON DSL.
    pub recipe: serde_json::Value,
    pub verification: McpInstallVerification,
}

/// Secret requirement descriptor for UI forms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSecretSchemaField {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub secret_type: Option<String>,
}

/// Recommended MCP service catalog item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCatalogItem {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub trust_level: McpCatalogTrustLevel,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub docs_url: Option<String>,
    #[serde(default)]
    pub maintained_by: Option<String>,
    #[serde(default)]
    pub os_support: Vec<String>,
    #[serde(default)]
    pub strategies: Vec<McpInstallStrategy>,
    #[serde(default)]
    pub secrets_schema: Vec<McpSecretSchemaField>,
}

/// Filter options for MCP catalog listing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpCatalogFilter {
    #[serde(default)]
    pub trust_levels: Vec<McpCatalogTrustLevel>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub query: Option<String>,
}

/// Response payload for listing catalog entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCatalogListResponse {
    pub items: Vec<McpCatalogItem>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub fetched_at: Option<String>,
    #[serde(default)]
    pub signature_valid: bool,
}

/// Refresh result for catalog updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCatalogRefreshResult {
    pub source: String,
    pub fetched_at: String,
    pub item_count: u32,
    pub updated: bool,
    #[serde(default)]
    pub signature_valid: bool,
    #[serde(default)]
    pub error: Option<String>,
}

/// Preview for a selected install strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInstallPreview {
    pub item_id: String,
    pub selected_strategy: String,
    #[serde(default)]
    pub missing_runtimes: Vec<McpRuntimeKind>,
    #[serde(default)]
    pub install_commands: Vec<String>,
    #[serde(default)]
    pub required_secrets: Vec<McpSecretSchemaField>,
    #[serde(default)]
    pub risk_flags: Vec<String>,
}

/// One-click install request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInstallRequest {
    pub item_id: String,
    pub server_alias: String,
    #[serde(default)]
    pub selected_strategy: Option<String>,
    #[serde(default)]
    pub secrets: HashMap<String, String>,
    #[serde(default)]
    pub oauth_mode: Option<String>,
    #[serde(default)]
    pub auto_connect: Option<bool>,
}

/// Install status values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpInstallStatus {
    Running,
    Success,
    Failed,
}

/// Installer phases (state machine).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McpInstallPhase {
    Precheck,
    Elevate,
    InstallRuntime,
    InstallPackage,
    WriteConfig,
    VerifyProtocol,
    AutoConnect,
    Commit,
    Rollback,
}

/// Install command result summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInstallResult {
    pub job_id: String,
    #[serde(default)]
    pub server_id: Option<String>,
    pub phase: McpInstallPhase,
    pub status: McpInstallStatus,
    #[serde(default)]
    pub diagnostics: Option<String>,
}

/// Runtime inventory row exposed to UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRuntimeInfo {
    pub runtime: McpRuntimeKind,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub managed: bool,
    #[serde(default)]
    pub healthy: bool,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub last_checked: Option<String>,
}

/// Runtime repair command result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRuntimeRepairResult {
    pub runtime: McpRuntimeKind,
    pub status: String,
    pub message: String,
}

/// Persisted managed-install metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInstallRecord {
    pub server_id: String,
    pub catalog_item_id: String,
    #[serde(default)]
    pub catalog_version: Option<String>,
    pub strategy_id: String,
    pub trust_level: McpCatalogTrustLevel,
    #[serde(default)]
    pub package_lock_json: Option<serde_json::Value>,
    #[serde(default)]
    pub runtime_snapshot_json: Option<serde_json::Value>,
    #[serde(default)]
    pub installed_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
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
