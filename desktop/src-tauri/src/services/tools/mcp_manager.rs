//! MCP Manager Service
//!
//! Manages the lifecycle of MCP server connections and integrates
//! discovered tools into the ToolRegistry. Provides connect/disconnect
//! operations that can be triggered from the frontend.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::services::tools::mcp_adapter::McpToolAdapter;
use crate::services::tools::mcp_client::{McpClient, McpServerConfig, McpTransportConfig};
use crate::services::tools::trait_def::ToolRegistry;
use crate::utils::error::{AppError, AppResult};

/// Information about a connected MCP server and its tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedServerInfo {
    /// Server name
    pub server_name: String,
    /// List of tool names registered from this server
    pub tool_names: Vec<String>,
    /// Qualified tool names (mcp:server:tool format)
    pub qualified_tool_names: Vec<String>,
    /// Protocol version
    pub protocol_version: String,
}

/// Manages MCP server connections and tool registration.
///
/// Thread-safe: uses RwLock for concurrent read access to the registry
/// and exclusive write access for connect/disconnect operations.
pub struct McpManager {
    /// Active MCP client connections, keyed by server name
    clients: RwLock<HashMap<String, Arc<McpClient>>>,
    /// Tracking which tools belong to which server
    server_tools: RwLock<HashMap<String, Vec<String>>>,
}

impl McpManager {
    /// Create a new MCP manager
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            server_tools: RwLock::new(HashMap::new()),
        }
    }

    /// Connect to an MCP server, discover its tools, and register them
    /// in the provided ToolRegistry.
    ///
    /// Returns information about the connected server and discovered tools.
    pub async fn connect_server(
        &self,
        config: &McpServerConfig,
        registry: &mut ToolRegistry,
    ) -> AppResult<ConnectedServerInfo> {
        let server_name = config.name.clone();

        // Check if already connected
        {
            let clients = self.clients.read().await;
            if clients.contains_key(&server_name) {
                return Err(AppError::validation(format!(
                    "MCP server '{}' is already connected",
                    server_name
                )));
            }
        }

        // Connect to the server
        let client = Arc::new(McpClient::connect(config).await?);

        let protocol_version = client.server_info().protocol_version.clone();

        // Discover tools
        let tools = client.list_tools().await?;

        // Create adapters and register them
        let mut tool_names = Vec::new();
        let mut qualified_names = Vec::new();

        for tool_info in &tools {
            let adapter = McpToolAdapter::new(
                server_name.clone(),
                tool_info.name.clone(),
                tool_info.description.clone(),
                tool_info.input_schema.clone(),
                client.clone(),
            );

            let qualified_name = adapter.qualified_name().to_string();
            tool_names.push(tool_info.name.clone());
            qualified_names.push(qualified_name.clone());

            registry.register(Arc::new(adapter));
        }

        // Store the client and tool mapping
        {
            let mut clients = self.clients.write().await;
            clients.insert(server_name.clone(), client);
        }
        {
            let mut server_tools = self.server_tools.write().await;
            server_tools.insert(server_name.clone(), qualified_names.clone());
        }

        Ok(ConnectedServerInfo {
            server_name,
            tool_names,
            qualified_tool_names: qualified_names,
            protocol_version,
        })
    }

    /// Disconnect from an MCP server and unregister its tools from the registry.
    pub async fn disconnect_server(
        &self,
        server_name: &str,
        registry: &mut ToolRegistry,
    ) -> AppResult<()> {
        // Remove and disconnect the client
        let client = {
            let mut clients = self.clients.write().await;
            clients.remove(server_name)
        };

        match client {
            Some(c) => {
                // Best effort disconnect
                let _ = c.disconnect().await;
            }
            None => {
                return Err(AppError::not_found(format!(
                    "MCP server '{}' is not connected",
                    server_name
                )));
            }
        }

        // Unregister tools
        let tool_names = {
            let mut server_tools = self.server_tools.write().await;
            server_tools.remove(server_name).unwrap_or_default()
        };

        for name in &tool_names {
            registry.unregister(name);
        }

        Ok(())
    }

    /// Disconnect all connected MCP servers
    pub async fn disconnect_all(&self, registry: &mut ToolRegistry) -> AppResult<()> {
        let server_names: Vec<String> = {
            let clients = self.clients.read().await;
            clients.keys().cloned().collect()
        };

        for name in server_names {
            // Best effort: log errors but continue disconnecting
            if let Err(e) = self.disconnect_server(&name, registry).await {
                tracing::warn!("Failed to disconnect MCP server '{}': {}", name, e);
            }
        }

        Ok(())
    }

    /// List all currently connected servers
    pub async fn list_connected_servers(&self) -> Vec<ConnectedServerInfo> {
        let clients = self.clients.read().await;
        let server_tools = self.server_tools.read().await;

        let mut servers = Vec::new();
        for (name, client) in clients.iter() {
            let qualified_names = server_tools.get(name).cloned().unwrap_or_default();

            let tool_names: Vec<String> = qualified_names
                .iter()
                .filter_map(|qn| {
                    McpToolAdapter::parse_qualified_name(qn).map(|(_, tool)| tool.to_string())
                })
                .collect();

            servers.push(ConnectedServerInfo {
                server_name: name.clone(),
                tool_names,
                qualified_tool_names: qualified_names,
                protocol_version: client.server_info().protocol_version.clone(),
            });
        }

        servers
    }

    /// Check if a server is currently connected
    pub async fn is_connected(&self, server_name: &str) -> bool {
        let clients = self.clients.read().await;
        clients.contains_key(server_name)
    }

    /// Get the number of connected servers
    pub async fn connected_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }

    /// Create an McpServerConfig from an existing McpServer model.
    ///
    /// Bridges the existing MCP server database model to the
    /// McpServerConfig needed by McpClient.
    pub fn config_from_model(server: &crate::models::McpServer) -> AppResult<McpServerConfig> {
        let transport = match server.server_type {
            crate::models::McpServerType::Stdio => {
                let command = server.command.clone().ok_or_else(|| {
                    AppError::validation("Stdio server requires a command".to_string())
                })?;
                McpTransportConfig::Stdio {
                    command,
                    args: server.args.clone(),
                    env: server.env.clone(),
                }
            }
            crate::models::McpServerType::Sse => {
                let base_url = server
                    .url
                    .clone()
                    .ok_or_else(|| AppError::validation("SSE server requires a URL".to_string()))?;
                McpTransportConfig::Http {
                    base_url,
                    headers: server.headers.clone(),
                }
            }
        };

        Ok(McpServerConfig {
            name: server.name.clone(),
            transport,
        })
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_manager_new() {
        let _manager = McpManager::new();
        // Can construct; internal state is private, we test via methods
    }

    #[tokio::test]
    async fn test_connected_count_initially_zero() {
        let manager = McpManager::new();
        assert_eq!(manager.connected_count().await, 0);
    }

    #[tokio::test]
    async fn test_is_connected_returns_false_for_unknown() {
        let manager = McpManager::new();
        assert!(!manager.is_connected("nonexistent").await);
    }

    #[tokio::test]
    async fn test_list_connected_servers_initially_empty() {
        let manager = McpManager::new();
        let servers = manager.list_connected_servers().await;
        assert!(servers.is_empty());
    }

    #[tokio::test]
    async fn test_disconnect_nonexistent_server() {
        let manager = McpManager::new();
        let mut registry = ToolRegistry::new();
        let result = manager
            .disconnect_server("nonexistent", &mut registry)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not connected"));
    }

    #[tokio::test]
    async fn test_disconnect_all_when_empty() {
        let manager = McpManager::new();
        let mut registry = ToolRegistry::new();
        let result = manager.disconnect_all(&mut registry).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_from_model_stdio() {
        use crate::models::McpServer;

        let server = McpServer::new_stdio(
            "test-id".to_string(),
            "test-server".to_string(),
            "node".to_string(),
            vec!["server.js".to_string()],
        );

        let config = McpManager::config_from_model(&server).unwrap();
        assert_eq!(config.name, "test-server");
        match &config.transport {
            McpTransportConfig::Stdio { command, args, .. } => {
                assert_eq!(command, "node");
                assert_eq!(args, &vec!["server.js".to_string()]);
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[test]
    fn test_config_from_model_sse() {
        use crate::models::McpServer;

        let server = McpServer::new_sse(
            "sse-id".to_string(),
            "sse-server".to_string(),
            "http://localhost:8080".to_string(),
        );

        let config = McpManager::config_from_model(&server).unwrap();
        assert_eq!(config.name, "sse-server");
        match &config.transport {
            McpTransportConfig::Http { base_url, .. } => {
                assert_eq!(base_url, "http://localhost:8080");
            }
            _ => panic!("Expected Http transport"),
        }
    }

    #[test]
    fn test_config_from_model_stdio_no_command() {
        use crate::models::{McpServer, McpServerStatus, McpServerType};

        let server = McpServer {
            id: "test".to_string(),
            name: "bad-server".to_string(),
            server_type: McpServerType::Stdio,
            command: None,
            args: vec![],
            env: HashMap::new(),
            url: None,
            headers: HashMap::new(),
            enabled: true,
            status: McpServerStatus::Unknown,
            last_checked: None,
            created_at: None,
            updated_at: None,
        };

        let result = McpManager::config_from_model(&server);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires a command"));
    }

    #[test]
    fn test_config_from_model_sse_no_url() {
        use crate::models::{McpServer, McpServerStatus, McpServerType};

        let server = McpServer {
            id: "test".to_string(),
            name: "bad-sse".to_string(),
            server_type: McpServerType::Sse,
            command: None,
            args: vec![],
            env: HashMap::new(),
            url: None,
            headers: HashMap::new(),
            enabled: true,
            status: McpServerStatus::Unknown,
            last_checked: None,
            created_at: None,
            updated_at: None,
        };

        let result = McpManager::config_from_model(&server);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires a URL"));
    }

    #[test]
    fn test_connected_server_info_serde() {
        let info = ConnectedServerInfo {
            server_name: "test".to_string(),
            tool_names: vec!["echo".to_string(), "read".to_string()],
            qualified_tool_names: vec!["mcp:test:echo".to_string(), "mcp:test:read".to_string()],
            protocol_version: "2024-11-05".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ConnectedServerInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.server_name, "test");
        assert_eq!(deserialized.tool_names.len(), 2);
    }

    /// Full integration test: connect, list tools, verify registry, disconnect
    #[tokio::test]
    async fn test_full_connect_disconnect_lifecycle() {
        let script = r#"
import sys, json

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        msg = json.loads(line)
    except:
        continue

    method = msg.get("method", "")
    msg_id = msg.get("id")

    if method == "initialize":
        response = {"jsonrpc": "2.0", "id": msg_id, "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "lifecycle-test", "version": "0.1.0"}
        }}
    elif method == "notifications/initialized":
        continue
    elif method == "tools/list":
        response = {"jsonrpc": "2.0", "id": msg_id, "result": {
            "tools": [
                {
                    "name": "tool_a",
                    "description": "Tool A",
                    "inputSchema": {"type": "object", "properties": {}}
                },
                {
                    "name": "tool_b",
                    "description": "Tool B",
                    "inputSchema": {"type": "object", "properties": {"x": {"type": "string"}}}
                }
            ]
        }}
    else:
        continue

    sys.stdout.write(json.dumps(response) + "\n")
    sys.stdout.flush()
"#;

        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("lifecycle_test_server.py");
        std::fs::write(&script_path, script).unwrap();

        let config = McpServerConfig {
            name: "lifecycle-test".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "python3".to_string(),
                args: vec![script_path.to_string_lossy().to_string()],
                env: HashMap::new(),
            },
        };

        let manager = McpManager::new();
        let mut registry = ToolRegistry::new();

        // Initially empty
        assert_eq!(manager.connected_count().await, 0);
        assert!(registry.is_empty());

        // Connect
        let info = manager
            .connect_server(&config, &mut registry)
            .await
            .unwrap();
        assert_eq!(info.server_name, "lifecycle-test");
        assert_eq!(info.tool_names, vec!["tool_a", "tool_b"]);
        assert_eq!(info.qualified_tool_names.len(), 2);
        assert_eq!(info.protocol_version, "2024-11-05");

        // Verify manager state
        assert_eq!(manager.connected_count().await, 1);
        assert!(manager.is_connected("lifecycle-test").await);

        // Verify registry
        assert_eq!(registry.len(), 2);
        assert!(registry.get("mcp:lifecycle-test:tool_a").is_some());
        assert!(registry.get("mcp:lifecycle-test:tool_b").is_some());

        // Verify tool definitions are available
        let defs = registry.definitions();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].name, "mcp:lifecycle-test:tool_a");
        assert_eq!(defs[1].name, "mcp:lifecycle-test:tool_b");

        // List connected servers
        let servers = manager.list_connected_servers().await;
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].server_name, "lifecycle-test");

        // Disconnect
        manager
            .disconnect_server("lifecycle-test", &mut registry)
            .await
            .unwrap();

        // Verify cleanup
        assert_eq!(manager.connected_count().await, 0);
        assert!(!manager.is_connected("lifecycle-test").await);
        assert!(registry.is_empty());
        assert!(registry.get("mcp:lifecycle-test:tool_a").is_none());
    }

    #[tokio::test]
    async fn test_connect_duplicate_server_rejected() {
        let script = r#"
import sys, json

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        msg = json.loads(line)
    except:
        continue

    method = msg.get("method", "")
    msg_id = msg.get("id")

    if method == "initialize":
        response = {"jsonrpc": "2.0", "id": msg_id, "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "serverInfo": {"name": "dup-test", "version": "0.1.0"}
        }}
    elif method == "notifications/initialized":
        continue
    elif method == "tools/list":
        response = {"jsonrpc": "2.0", "id": msg_id, "result": {"tools": []}}
    else:
        continue

    sys.stdout.write(json.dumps(response) + "\n")
    sys.stdout.flush()
"#;

        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("dup_test_server.py");
        std::fs::write(&script_path, script).unwrap();

        let config = McpServerConfig {
            name: "dup-test".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "python3".to_string(),
                args: vec![script_path.to_string_lossy().to_string()],
                env: HashMap::new(),
            },
        };

        let manager = McpManager::new();
        let mut registry = ToolRegistry::new();

        // First connect succeeds
        manager
            .connect_server(&config, &mut registry)
            .await
            .unwrap();

        // Second connect should fail
        let result = manager.connect_server(&config, &mut registry).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already connected"));

        // Cleanup
        manager
            .disconnect_server("dup-test", &mut registry)
            .await
            .unwrap();
    }
}
