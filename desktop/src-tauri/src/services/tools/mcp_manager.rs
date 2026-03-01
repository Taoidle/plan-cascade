//! MCP Manager Service
//!
//! Manages the lifecycle of MCP server connections and integrates
//! discovered tools into the ToolRegistry.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::services::tools::mcp_adapter::McpToolAdapter;
use crate::services::tools::mcp_client::{McpClient, McpServerConfig, McpTransportConfig};
use crate::services::tools::trait_def::ToolRegistry;
use crate::utils::error::{AppError, AppResult};

/// Information about a connected MCP server and its tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedServerInfo {
    /// Server ID (database primary key)
    pub server_id: String,
    /// Server display name
    pub server_name: String,
    /// Current connection state
    pub connection_state: String,
    /// List of tool names registered from this server
    pub tool_names: Vec<String>,
    /// Qualified tool names (mcp:server:tool format)
    pub qualified_tool_names: Vec<String>,
    /// Protocol version
    pub protocol_version: String,
    /// Connection established timestamp
    pub connected_at: Option<String>,
    /// Last connection error if any
    pub last_error: Option<String>,
    /// Retry count for diagnostics
    pub retry_count: u32,
}

/// Manages MCP server connections and tool registration.
pub struct McpManager {
    /// Active MCP client connections, keyed by server id
    clients: RwLock<HashMap<String, Arc<McpClient>>>,
    /// Tracking which tools belong to which server id
    server_tools: RwLock<HashMap<String, Vec<String>>>,
    /// Server names by server id
    server_names: RwLock<HashMap<String, String>>,
    /// First connection timestamp by server id
    connected_at: RwLock<HashMap<String, String>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            server_tools: RwLock::new(HashMap::new()),
            server_names: RwLock::new(HashMap::new()),
            connected_at: RwLock::new(HashMap::new()),
        }
    }

    pub async fn connect_server(
        &self,
        config: &McpServerConfig,
        registry: &mut ToolRegistry,
    ) -> AppResult<ConnectedServerInfo> {
        let server_id = if config.id.is_empty() {
            config.name.clone()
        } else {
            config.id.clone()
        };
        let server_name = config.name.clone();

        {
            let clients = self.clients.read().await;
            if clients.contains_key(&server_id) {
                return Err(AppError::validation(format!(
                    "MCP server '{}' is already connected",
                    server_name
                )));
            }
        }

        let client = Arc::new(McpClient::connect(config).await?);
        let protocol_version = client.server_info().protocol_version.clone();
        let tools = client.list_tools().await?;

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

        let connected_at = chrono::Utc::now().to_rfc3339();

        {
            let mut clients = self.clients.write().await;
            clients.insert(server_id.clone(), client);
        }
        {
            let mut server_tools = self.server_tools.write().await;
            server_tools.insert(server_id.clone(), qualified_names.clone());
        }
        {
            let mut server_names = self.server_names.write().await;
            server_names.insert(server_id.clone(), server_name.clone());
        }
        {
            let mut connected = self.connected_at.write().await;
            connected.insert(server_id.clone(), connected_at.clone());
        }

        Ok(ConnectedServerInfo {
            server_id,
            server_name,
            connection_state: "connected".to_string(),
            tool_names,
            qualified_tool_names: qualified_names,
            protocol_version,
            connected_at: Some(connected_at),
            last_error: None,
            retry_count: 0,
        })
    }

    pub async fn disconnect_server(
        &self,
        server_id: &str,
        registry: &mut ToolRegistry,
    ) -> AppResult<()> {
        let client = {
            let mut clients = self.clients.write().await;
            clients.remove(server_id)
        };

        match client {
            Some(c) => {
                let _ = c.disconnect().await;
            }
            None => {
                return Err(AppError::not_found(format!(
                    "MCP server '{}' is not connected",
                    server_id
                )));
            }
        }

        let tool_names = {
            let mut server_tools = self.server_tools.write().await;
            server_tools.remove(server_id).unwrap_or_default()
        };

        {
            let mut server_names = self.server_names.write().await;
            server_names.remove(server_id);
        }

        {
            let mut connected = self.connected_at.write().await;
            connected.remove(server_id);
        }

        for name in &tool_names {
            registry.unregister(name);
        }

        Ok(())
    }

    pub async fn disconnect_all(&self, registry: &mut ToolRegistry) -> AppResult<()> {
        let server_ids: Vec<String> = {
            let clients = self.clients.read().await;
            clients.keys().cloned().collect()
        };

        for id in server_ids {
            if let Err(e) = self.disconnect_server(&id, registry).await {
                tracing::warn!("Failed to disconnect MCP server '{}': {}", id, e);
            }
        }

        Ok(())
    }

    pub async fn list_connected_servers(&self) -> Vec<ConnectedServerInfo> {
        let clients = self.clients.read().await;
        let server_tools = self.server_tools.read().await;
        let server_names = self.server_names.read().await;
        let connected_at = self.connected_at.read().await;

        let mut servers = Vec::new();
        for (id, client) in clients.iter() {
            let qualified_names = server_tools.get(id).cloned().unwrap_or_default();
            let tool_names: Vec<String> = qualified_names
                .iter()
                .filter_map(|qn| {
                    McpToolAdapter::parse_qualified_name(qn).map(|(_, tool)| tool.to_string())
                })
                .collect();

            servers.push(ConnectedServerInfo {
                server_id: id.clone(),
                server_name: server_names.get(id).cloned().unwrap_or_else(|| id.clone()),
                connection_state: "connected".to_string(),
                tool_names,
                qualified_tool_names: qualified_names,
                protocol_version: client.server_info().protocol_version.clone(),
                connected_at: connected_at.get(id).cloned(),
                last_error: None,
                retry_count: 0,
            });
        }

        servers
    }

    pub async fn is_connected(&self, server_id: &str) -> bool {
        let clients = self.clients.read().await;
        clients.contains_key(server_id)
    }

    pub async fn connected_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }

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
            crate::models::McpServerType::StreamHttp => {
                let base_url = server.url.clone().ok_or_else(|| {
                    AppError::validation("Stream HTTP server requires a URL".to_string())
                })?;
                McpTransportConfig::Http {
                    base_url,
                    headers: server.headers.clone(),
                }
            }
        };

        Ok(McpServerConfig {
            id: server.id.clone(),
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

    #[tokio::test]
    async fn test_connected_count_initially_zero() {
        let manager = McpManager::new();
        assert_eq!(manager.connected_count().await, 0);
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
        assert_eq!(config.id, "test-id");
        assert_eq!(config.name, "test-server");
    }

    #[test]
    fn test_config_from_model_stream_http() {
        use crate::models::McpServer;

        let server = McpServer::new_stream_http(
            "http-id".to_string(),
            "http-server".to_string(),
            "http://localhost:8080".to_string(),
        );

        let config = McpManager::config_from_model(&server).unwrap();
        assert_eq!(config.id, "http-id");
        match &config.transport {
            McpTransportConfig::Http { base_url, .. } => {
                assert_eq!(base_url, "http://localhost:8080");
            }
            _ => panic!("Expected Http transport"),
        }
    }
}
