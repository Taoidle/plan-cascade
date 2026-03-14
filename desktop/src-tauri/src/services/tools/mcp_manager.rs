//! MCP Manager Service
//!
//! Manages the lifecycle of MCP server connections and integrates
//! discovered tools into the ToolRegistry.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::services::tools::mcp_adapter::McpToolAdapter;
use crate::services::tools::mcp_client::{
    McpClient, McpServerConfig, McpToolDebugMetadata, McpTransportConfig,
};
use crate::services::tools::runtime_tools::RuntimeToolMetadata;
use crate::services::tools::trait_def::{Tool, ToolRegistry};
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
    /// Runtime metadata for connected server tools, keyed by server id then tool name.
    server_tool_metadata: RwLock<HashMap<String, HashMap<String, RuntimeToolMetadata>>>,
}

struct PreparedConnection {
    server_id: String,
    server_name: String,
    transport_kind: String,
    client: Arc<McpClient>,
    protocol_version: String,
    tool_names: Vec<String>,
    qualified_names: Vec<String>,
    adapters: Vec<Arc<dyn Tool>>,
    metadata: HashMap<String, RuntimeToolMetadata>,
}

fn runtime_metadata_from_debug_metadata(
    server_name: &str,
    debug_metadata: Option<&McpToolDebugMetadata>,
) -> RuntimeToolMetadata {
    RuntimeToolMetadata {
        source: format!("mcp:{server_name}"),
        capability_class: debug_metadata.and_then(|meta| meta.capability_class.clone()),
        debug_categories: debug_metadata
            .map(|meta| meta.debug_categories.clone())
            .unwrap_or_default(),
        environment_allowlist: debug_metadata
            .map(|meta| meta.environment_allowlist.clone())
            .unwrap_or_default(),
        write_behavior: debug_metadata.and_then(|meta| meta.write_behavior.clone()),
        approval_required: debug_metadata.and_then(|meta| meta.approval_required),
    }
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            server_tools: RwLock::new(HashMap::new()),
            server_names: RwLock::new(HashMap::new()),
            connected_at: RwLock::new(HashMap::new()),
            server_tool_metadata: RwLock::new(HashMap::new()),
        }
    }

    fn server_id_from_config(config: &McpServerConfig) -> String {
        if config.id.is_empty() {
            config.name.clone()
        } else {
            config.id.clone()
        }
    }

    fn transport_kind(config: &McpServerConfig) -> &'static str {
        match &config.transport {
            McpTransportConfig::Stdio { .. } => "stdio",
            McpTransportConfig::Http { .. } => "stream_http",
        }
    }

    fn error_class(raw: &str) -> &'static str {
        let lower = raw.to_lowercase();
        if lower.contains("unauthorized")
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
        }
    }

    async fn prepare_connection(&self, config: &McpServerConfig) -> AppResult<PreparedConnection> {
        let server_id = Self::server_id_from_config(config);
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

        tracing::info!(
            event = "connect_attempt",
            server_id = %server_id,
            server_name = %server_name,
            transport = Self::transport_kind(config),
            "Connecting MCP server"
        );

        let client = Arc::new(McpClient::connect(config).await?);
        let protocol_version = client.server_info().protocol_version.clone();
        let tools = client.list_tools().await?;

        let mut tool_names = Vec::new();
        let mut qualified_names = Vec::new();
        let mut seen = HashSet::new();
        let mut adapters: Vec<Arc<dyn Tool>> = Vec::new();
        let mut metadata = HashMap::new();

        for tool_info in &tools {
            let adapter = Arc::new(McpToolAdapter::new(
                server_id.clone(),
                server_name.clone(),
                tool_info.name.clone(),
                tool_info.description.clone(),
                tool_info.input_schema.clone(),
                client.clone(),
            ));

            let qualified_name = adapter.qualified_name().to_string();
            if !seen.insert(qualified_name.clone()) {
                let _ = client.disconnect().await;
                return Err(AppError::validation(format!(
                    "Duplicate MCP tool '{}' reported by server '{}'",
                    qualified_name, server_name
                )));
            }

            tool_names.push(tool_info.name.clone());
            qualified_names.push(qualified_name);
            metadata.insert(
                adapter.qualified_name().to_string(),
                runtime_metadata_from_debug_metadata(
                    &server_name,
                    tool_info.debug_metadata.as_ref(),
                ),
            );
            adapters.push(adapter);
        }

        Ok(PreparedConnection {
            server_id,
            server_name,
            transport_kind: Self::transport_kind(config).to_string(),
            client,
            protocol_version,
            tool_names,
            qualified_names,
            adapters,
            metadata,
        })
    }

    fn register_prepared_tools(
        prepared: &PreparedConnection,
        registry: &mut ToolRegistry,
    ) -> AppResult<()> {
        for qualified_name in &prepared.qualified_names {
            if registry.get(qualified_name).is_some() {
                return Err(AppError::validation(format!(
                    "MCP tool '{}' is already registered",
                    qualified_name
                )));
            }
        }

        for adapter in &prepared.adapters {
            registry.register(adapter.clone());
        }

        Ok(())
    }

    async fn commit_prepared_connection(&self, prepared: &PreparedConnection) -> String {
        let connected_at = chrono::Utc::now().to_rfc3339();
        {
            let mut clients = self.clients.write().await;
            clients.insert(prepared.server_id.clone(), prepared.client.clone());
        }
        {
            let mut server_tools = self.server_tools.write().await;
            server_tools.insert(prepared.server_id.clone(), prepared.qualified_names.clone());
        }
        {
            let mut server_names = self.server_names.write().await;
            server_names.insert(prepared.server_id.clone(), prepared.server_name.clone());
        }
        {
            let mut connected = self.connected_at.write().await;
            connected.insert(prepared.server_id.clone(), connected_at.clone());
        }
        {
            let mut metadata = self.server_tool_metadata.write().await;
            metadata.insert(prepared.server_id.clone(), prepared.metadata.clone());
        }
        connected_at
    }

    fn as_connected_info(
        prepared: &PreparedConnection,
        connected_at: String,
    ) -> ConnectedServerInfo {
        ConnectedServerInfo {
            server_id: prepared.server_id.clone(),
            server_name: prepared.server_name.clone(),
            connection_state: "connected".to_string(),
            tool_names: prepared.tool_names.clone(),
            qualified_tool_names: prepared.qualified_names.clone(),
            protocol_version: prepared.protocol_version.clone(),
            connected_at: Some(connected_at),
            last_error: None,
            retry_count: 0,
        }
    }

    pub async fn connect_server_with_registry_lock(
        &self,
        config: &McpServerConfig,
        registry: Arc<RwLock<ToolRegistry>>,
    ) -> AppResult<ConnectedServerInfo> {
        let prepared = match self.prepare_connection(config).await {
            Ok(prepared) => prepared,
            Err(e) => {
                tracing::warn!(
                    event = "connect_failure",
                    server_id = %Self::server_id_from_config(config),
                    server_name = %config.name,
                    transport = Self::transport_kind(config),
                    error = %e,
                    error_class = Self::error_class(&e.to_string()),
                    "Failed to connect MCP server"
                );
                return Err(e);
            }
        };

        {
            let mut guard = registry.write().await;
            if let Err(e) = Self::register_prepared_tools(&prepared, &mut guard) {
                let _ = prepared.client.disconnect().await;
                tracing::warn!(
                    event = "connect_failure",
                    server_id = %prepared.server_id,
                    server_name = %prepared.server_name,
                    transport = %prepared.transport_kind,
                    error = %e,
                    error_class = "registry",
                    "Failed to register MCP tools"
                );
                return Err(e);
            }
        }

        let connected_at = self.commit_prepared_connection(&prepared).await;

        tracing::info!(
            event = "connect_success",
            server_id = %prepared.server_id,
            server_name = %prepared.server_name,
            transport = %prepared.transport_kind,
            tool_count = prepared.qualified_names.len(),
            protocol_version = %prepared.protocol_version,
            "Connected MCP server"
        );

        Ok(Self::as_connected_info(&prepared, connected_at))
    }

    pub async fn connect_server(
        &self,
        config: &McpServerConfig,
        registry: &mut ToolRegistry,
    ) -> AppResult<ConnectedServerInfo> {
        let prepared = match self.prepare_connection(config).await {
            Ok(prepared) => prepared,
            Err(e) => {
                tracing::warn!(
                    event = "connect_failure",
                    server_id = %Self::server_id_from_config(config),
                    server_name = %config.name,
                    transport = Self::transport_kind(config),
                    error = %e,
                    error_class = Self::error_class(&e.to_string()),
                    "Failed to connect MCP server"
                );
                return Err(e);
            }
        };
        if let Err(e) = Self::register_prepared_tools(&prepared, registry) {
            let _ = prepared.client.disconnect().await;
            tracing::warn!(
                event = "connect_failure",
                server_id = %prepared.server_id,
                server_name = %prepared.server_name,
                transport = %prepared.transport_kind,
                error = %e,
                error_class = "registry",
                "Failed to register MCP tools"
            );
            return Err(e);
        }

        let connected_at = self.commit_prepared_connection(&prepared).await;

        tracing::info!(
            event = "connect_success",
            server_id = %prepared.server_id,
            server_name = %prepared.server_name,
            transport = %prepared.transport_kind,
            tool_count = prepared.qualified_names.len(),
            protocol_version = %prepared.protocol_version,
            "Connected MCP server"
        );

        Ok(Self::as_connected_info(&prepared, connected_at))
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
            let mut metadata = self.server_tool_metadata.write().await;
            metadata.remove(server_id);
        }

        {
            let mut connected = self.connected_at.write().await;
            connected.remove(server_id);
        }

        for name in &tool_names {
            registry.unregister(name);
        }

        tracing::info!(
            event = "disconnect",
            server_id = %server_id,
            removed_tool_count = tool_names.len(),
            "Disconnected MCP server"
        );

        Ok(())
    }

    pub async fn runtime_tool_metadata(&self) -> HashMap<String, RuntimeToolMetadata> {
        self.server_tool_metadata
            .read()
            .await
            .values()
            .flat_map(|items| {
                items
                    .iter()
                    .map(|(name, metadata)| (name.clone(), metadata.clone()))
            })
            .collect()
    }

    pub async fn invoke_connected_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> AppResult<(String, String, serde_json::Value)> {
        let client = {
            let clients = self.clients.read().await;
            clients.get(server_id).cloned()
        }
        .ok_or_else(|| {
            AppError::not_found(format!("MCP server '{}' is not connected", server_id))
        })?;

        let server_name = {
            let server_names = self.server_names.read().await;
            server_names
                .get(server_id)
                .cloned()
                .unwrap_or_else(|| server_id.to_string())
        };

        let normalized_tool_name = if let Some((server_token, raw_tool_name)) =
            McpToolAdapter::parse_qualified_name(tool_name)
        {
            if server_token != server_id && server_token != server_name {
                return Err(AppError::validation(format!(
                    "Qualified MCP tool '{}' does not belong to server '{}'",
                    tool_name, server_id
                )));
            }
            raw_tool_name.to_string()
        } else {
            tool_name.trim().to_string()
        };

        if normalized_tool_name.is_empty() {
            return Err(AppError::validation(
                "MCP tool name cannot be empty".to_string(),
            ));
        }

        let qualified_name = format!("mcp:{}:{}", server_id, normalized_tool_name);
        let tool_registered = {
            let server_tools = self.server_tools.read().await;
            server_tools
                .get(server_id)
                .map(|names| names.iter().any(|name| name == &qualified_name))
                .unwrap_or(false)
        };
        if !tool_registered {
            return Err(AppError::not_found(format!(
                "Tool '{}' is not registered on MCP server '{}'",
                normalized_tool_name, server_id
            )));
        }

        let value = client.call_tool(&normalized_tool_name, arguments).await?;
        Ok((server_name, normalized_tool_name, value))
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
