//! MCP Client
//!
//! MCP client built on the official `rmcp` Rust SDK.
//! Supports stdio (child process) and streamable HTTP transports.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::Mutex;

use rmcp::model::{CallToolRequestParams, Content};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::ServiceExt;

use crate::utils::configure_background_process;
use crate::utils::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDebugMetadata {
    pub capability_class: Option<String>,
    #[serde(default)]
    pub debug_categories: Vec<String>,
    #[serde(default)]
    pub environment_allowlist: Vec<String>,
    pub write_behavior: Option<String>,
    pub approval_required: Option<bool>,
}

/// Configuration for connecting to an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Stable server ID (database primary key)
    #[serde(default)]
    pub id: String,
    /// Server name (for display/identification)
    pub name: String,
    /// Transport type
    pub transport: McpTransportConfig,
}

/// Transport-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpTransportConfig {
    /// Stdio transport: spawn a child process
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    /// Streamable HTTP transport: connect to a remote server
    #[serde(rename = "http")]
    Http {
        base_url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

/// Information about a connected MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    /// Server name
    pub name: String,
    /// Protocol version
    pub protocol_version: String,
    /// Server capabilities
    pub capabilities: Value,
    /// Server-provided metadata
    pub server_info: Value,
}

/// Information about a tool provided by an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// Tool name (as provided by the MCP server)
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON Schema for the tool's input parameters
    pub input_schema: Value,
    /// Optional debug/capability metadata derived from vendor extensions.
    pub debug_metadata: Option<McpToolDebugMetadata>,
}

/// MCP client for communicating with MCP servers
pub struct McpClient {
    session: Mutex<RunningService<RoleClient, ()>>,
    server_info: McpServerInfo,
}

impl McpClient {
    fn parse_string_array(value: Option<&Value>) -> Vec<String> {
        value
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|item| item.trim().to_ascii_lowercase())
                    .filter(|item| !item.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn extract_debug_metadata(
        input_schema: &Value,
        description: &str,
    ) -> Option<McpToolDebugMetadata> {
        let object = input_schema.as_object()?;
        let debug_object = object
            .get("x-plan-cascade-debug")
            .or_else(|| object.get("x-debug"))
            .and_then(Value::as_object);

        let mut debug_categories = Self::parse_string_array(
            debug_object
                .and_then(|map| map.get("debug_categories"))
                .or_else(|| object.get("x-debug-categories")),
        );

        if debug_categories.is_empty() {
            debug_categories = description
                .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';')
                .filter_map(|token| {
                    let normalized = token
                        .trim_matches(|ch: char| ch == '[' || ch == ']' || ch == '(' || ch == ')')
                        .to_ascii_lowercase();
                    normalized
                        .strip_prefix("debug:")
                        .map(|suffix| format!("debug:{suffix}"))
                })
                .collect();
        }

        let environment_allowlist = Self::parse_string_array(
            debug_object
                .and_then(|map| map.get("environment_allowlist"))
                .or_else(|| object.get("x-environment-allowlist")),
        );

        let capability_class = debug_object
            .and_then(|map| map.get("capability_class"))
            .or_else(|| object.get("x-capability-class"))
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase());

        let write_behavior = debug_object
            .and_then(|map| map.get("write_behavior"))
            .or_else(|| object.get("x-write-behavior"))
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase());

        let approval_required = debug_object
            .and_then(|map| map.get("approval_required"))
            .or_else(|| object.get("x-approval-required"))
            .and_then(Value::as_bool);

        if capability_class.is_none()
            && debug_categories.is_empty()
            && environment_allowlist.is_empty()
            && write_behavior.is_none()
            && approval_required.is_none()
        {
            return None;
        }

        Some(McpToolDebugMetadata {
            capability_class,
            debug_categories,
            environment_allowlist,
            write_behavior,
            approval_required,
        })
    }

    /// Connect to an MCP server using the provided configuration.
    pub async fn connect(config: &McpServerConfig) -> AppResult<Self> {
        match &config.transport {
            McpTransportConfig::Stdio { command, args, env } => {
                Self::connect_stdio(&config.name, command, args, env).await
            }
            McpTransportConfig::Http { base_url, headers } => {
                Self::connect_http(&config.name, base_url, headers).await
            }
        }
    }

    async fn connect_stdio(
        name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> AppResult<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        for (key, value) in env {
            cmd.env(key, value);
        }
        configure_background_process(&mut cmd);

        let transport = TokioChildProcess::new(cmd).map_err(|e| {
            AppError::command(format!(
                "Failed to spawn MCP server '{}' (command: {}): {}",
                name, command, e
            ))
        })?;

        let service = ().serve(transport).await.map_err(|e| {
            AppError::command(format!(
                "MCP server '{}' initialization failed over stdio: {}",
                name, e
            ))
        })?;

        let server_info = Self::extract_server_info(name, &service);

        Ok(Self {
            session: Mutex::new(service),
            server_info,
        })
    }

    async fn connect_http(
        name: &str,
        base_url: &str,
        headers: &HashMap<String, String>,
    ) -> AppResult<Self> {
        let mut custom_headers = HashMap::new();
        for (key, value) in headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    AppError::validation(format!("Invalid header name '{}': {}", key, e))
                })?;
            let header_value = reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                AppError::validation(format!("Invalid header value for '{}': {}", key, e))
            })?;
            custom_headers.insert(header_name, header_value);
        }

        let transport = StreamableHttpClientTransport::from_config(
            StreamableHttpClientTransportConfig::with_uri(base_url.to_string())
                .custom_headers(custom_headers),
        );

        let service = ().serve(transport).await.map_err(|e| {
            AppError::command(format!(
                "MCP server '{}' initialization failed over streamable HTTP: {}",
                name, e
            ))
        })?;

        let server_info = Self::extract_server_info(name, &service);

        Ok(Self {
            session: Mutex::new(service),
            server_info,
        })
    }

    fn extract_server_info(name: &str, service: &RunningService<RoleClient, ()>) -> McpServerInfo {
        let peer_info = service.peer_info().cloned();

        match peer_info {
            Some(info) => McpServerInfo {
                name: name.to_string(),
                protocol_version: info.protocol_version.to_string(),
                capabilities: serde_json::to_value(&info.capabilities)
                    .unwrap_or_else(|_| Value::Object(serde_json::Map::new())),
                server_info: serde_json::to_value(&info.server_info)
                    .unwrap_or_else(|_| Value::Object(serde_json::Map::new())),
            },
            None => McpServerInfo {
                name: name.to_string(),
                protocol_version: "unknown".to_string(),
                capabilities: Value::Object(serde_json::Map::new()),
                server_info: Value::Object(serde_json::Map::new()),
            },
        }
    }

    /// List all tools available on the connected MCP server
    pub async fn list_tools(&self) -> AppResult<Vec<McpToolInfo>> {
        let session = self.session.lock().await;
        let tools = session
            .list_all_tools()
            .await
            .map_err(|e| AppError::command(format!("tools/list failed: {}", e)))?;

        let mut result = Vec::with_capacity(tools.len());
        for tool in tools {
            let input_schema = serde_json::to_value(tool.input_schema.as_ref())
                .unwrap_or_else(|_| serde_json::json!({"type": "object"}));
            let description = tool.description.map(|d| d.into_owned()).unwrap_or_default();
            let debug_metadata = Self::extract_debug_metadata(&input_schema, &description);

            result.push(McpToolInfo {
                name: tool.name.into_owned(),
                description,
                debug_metadata,
                input_schema,
            });
        }

        Ok(result)
    }

    fn extract_text_content(content: &[Content]) -> Vec<String> {
        content
            .iter()
            .filter_map(|item| item.raw.as_text().map(|text| text.text.clone()))
            .filter(|text| !text.is_empty())
            .collect()
    }

    /// Call a tool on the connected MCP server
    pub async fn call_tool(&self, name: &str, args: Value) -> AppResult<Value> {
        let arguments = match args {
            Value::Object(map) => Some(map),
            Value::Null => None,
            _ => {
                return Err(AppError::validation(
                    "MCP tool arguments must be a JSON object or null".to_string(),
                ));
            }
        };

        let session = self.session.lock().await;
        let result = session
            .call_tool(CallToolRequestParams {
                meta: None,
                name: name.to_string().into(),
                arguments,
                task: None,
            })
            .await
            .map_err(|e| AppError::command(format!("MCP tool '{}' call failed: {}", name, e)))?;

        let text_parts = Self::extract_text_content(&result.content);

        if result.is_error.unwrap_or(false) {
            let err_msg = if text_parts.is_empty() {
                "Unknown MCP tool error".to_string()
            } else {
                text_parts.join("\n")
            };
            return Err(AppError::command(format!(
                "MCP tool '{}' returned an error: {}",
                name, err_msg
            )));
        }

        if let Some(value) = result.structured_content {
            return Ok(value);
        }

        if !text_parts.is_empty() {
            return Ok(Value::String(text_parts.join("\n")));
        }

        serde_json::to_value(&result)
            .map_err(|e| AppError::command(format!("Failed to serialize MCP tool result: {}", e)))
    }

    /// Disconnect from the MCP server
    pub async fn disconnect(&self) -> AppResult<()> {
        let mut session = self.session.lock().await;
        match tokio::time::timeout(Duration::from_secs(3), session.close()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(AppError::command(format!(
                "Failed to close MCP connection: {}",
                e
            ))),
            Err(_) => {
                session.cancellation_token().cancel();
                Ok(())
            }
        }
    }

    /// Get server info
    pub fn server_info(&self) -> &McpServerInfo {
        &self.server_info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_mcp_server_config_stdio() {
        let config = McpServerConfig {
            id: "server-1".to_string(),
            name: "test-server".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "node".to_string(),
                args: vec!["server.js".to_string()],
                env: HashMap::new(),
            },
        };

        assert_eq!(config.id, "server-1");
        assert_eq!(config.name, "test-server");
        match &config.transport {
            McpTransportConfig::Stdio { command, args, .. } => {
                assert_eq!(command, "node");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[test]
    fn test_mcp_server_config_http() {
        let config = McpServerConfig {
            id: "server-http".to_string(),
            name: "http-server".to_string(),
            transport: McpTransportConfig::Http {
                base_url: "http://localhost:8080".to_string(),
                headers: HashMap::new(),
            },
        };

        assert_eq!(config.id, "server-http");
        match &config.transport {
            McpTransportConfig::Http { base_url, .. } => {
                assert_eq!(base_url, "http://localhost:8080");
            }
            _ => panic!("Expected Http transport"),
        }
    }

    #[test]
    fn test_extract_text_content() {
        let content = vec![
            Content::text("hello"),
            Content::text("world"),
            Content::text(""),
        ];

        let parts = McpClient::extract_text_content(&content);
        assert_eq!(parts, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn test_extract_debug_metadata_from_schema_extensions() {
        let schema = serde_json::json!({
            "type": "object",
            "x-plan-cascade-debug": {
                "capability_class": "observe",
                "debug_categories": ["debug:logs", "debug:trace"],
                "environment_allowlist": ["staging", "prod"],
                "write_behavior": "read_only",
                "approval_required": true
            }
        });

        let metadata =
            McpClient::extract_debug_metadata(&schema, "Inspect logs").expect("metadata");

        assert_eq!(metadata.capability_class.as_deref(), Some("observe"));
        assert_eq!(
            metadata.debug_categories,
            vec!["debug:logs".to_string(), "debug:trace".to_string()]
        );
        assert_eq!(
            metadata.environment_allowlist,
            vec!["staging".to_string(), "prod".to_string()]
        );
        assert_eq!(metadata.write_behavior.as_deref(), Some("read_only"));
        assert_eq!(metadata.approval_required, Some(true));
    }

    #[test]
    fn test_extract_debug_metadata_from_description_tags() {
        let schema = serde_json::json!({ "type": "object" });
        let metadata = McpClient::extract_debug_metadata(
            &schema,
            "Read application logs [debug:logs] [debug:metrics]",
        )
        .expect("metadata");

        assert_eq!(
            metadata.debug_categories,
            vec!["debug:logs".to_string(), "debug:metrics".to_string()]
        );
    }

    #[tokio::test]
    async fn test_connect_stdio_nonexistent_command() {
        let config = McpServerConfig {
            id: "missing-command".to_string(),
            name: "missing-command".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "/nonexistent/command/for-mcp".to_string(),
                args: vec![],
                env: HashMap::new(),
            },
        };

        let result = McpClient::connect(&config).await;
        let err = match result {
            Ok(_) => panic!("Expected connect() to fail for nonexistent stdio command"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("Failed to spawn MCP server"));
    }

    #[tokio::test]
    async fn test_connect_http_invalid_header_name() {
        let mut headers = HashMap::new();
        headers.insert("bad header".to_string(), "x".to_string());

        let config = McpServerConfig {
            id: "http-invalid-header".to_string(),
            name: "http-invalid-header".to_string(),
            transport: McpTransportConfig::Http {
                base_url: "http://127.0.0.1:9/mcp".to_string(),
                headers,
            },
        };

        let result = McpClient::connect(&config).await;
        let err = match result {
            Ok(_) => panic!("Expected connect() to fail for invalid HTTP header name"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("Invalid header name"));
    }

    fn write_mock_server(
        script_name: &str,
        script: &str,
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        let temp_dir = tempdir().expect("temp dir");
        let script_path = temp_dir.path().join(script_name);
        fs::write(&script_path, script).expect("write script");
        (temp_dir, script_path)
    }

    #[tokio::test]
    async fn test_stdio_mock_server_interaction() {
        let script = r#"
import json
import sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        msg = json.loads(line)
    except Exception:
        continue

    method = msg.get("method", "")
    msg_id = msg.get("id")

    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "protocolVersion": "2025-06-18",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "mock-server", "version": "0.1.0"}
            }
        })
    elif method == "notifications/initialized":
        continue
    elif method == "tools/list":
        send({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "tools": [{
                    "name": "echo",
                    "description": "Echoes text",
                    "inputSchema": {
                        "type": "object",
                        "properties": {"message": {"type": "string"}}
                    }
                }]
            }
        })
    elif method == "tools/call":
        params = msg.get("params", {})
        arguments = params.get("arguments", {}) or {}
        name = params.get("name", "")

        if name == "echo":
            send({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "content": [{"type": "text", "text": arguments.get("message", "")}]
                }
            })
        elif name == "structured":
            send({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "structuredContent": {"ok": True, "value": 42},
                    "content": [{"type": "text", "text": "fallback"}]
                }
            })
"#;

        let (_temp_dir, script_path) = write_mock_server("mock_mcp_server.py", script);

        let config = McpServerConfig {
            id: "mock-stdio".to_string(),
            name: "mock-stdio".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "python3".to_string(),
                args: vec![script_path.to_string_lossy().to_string()],
                env: HashMap::new(),
            },
        };

        let client = McpClient::connect(&config).await.expect("connect");
        assert_eq!(client.server_info().name, "mock-stdio");
        assert_eq!(client.server_info().protocol_version, "2025-06-18");

        let tools = client.list_tools().await.expect("list tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");

        let echoed = client
            .call_tool("echo", serde_json::json!({"message": "hello mcp"}))
            .await
            .expect("call echo");
        assert_eq!(echoed, Value::String("hello mcp".to_string()));

        let structured = client
            .call_tool("structured", serde_json::json!({}))
            .await
            .expect("call structured");
        assert_eq!(structured, serde_json::json!({"ok": true, "value": 42}));

        client.disconnect().await.expect("disconnect");
    }

    #[tokio::test]
    async fn test_call_tool_error_result_is_propagated() {
        let script = r#"
import json
import sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        msg = json.loads(line)
    except Exception:
        continue

    method = msg.get("method", "")
    msg_id = msg.get("id")

    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "protocolVersion": "2025-06-18",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "error-server", "version": "0.1.0"}
            }
        })
    elif method == "notifications/initialized":
        continue
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": msg_id, "result": {"tools": []}})
    elif method == "tools/call":
        send({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "isError": True,
                "content": [{"type": "text", "text": "tool exploded"}]
            }
        })
"#;

        let (_temp_dir, script_path) = write_mock_server("mock_mcp_server_error.py", script);

        let config = McpServerConfig {
            id: "mock-error".to_string(),
            name: "mock-error".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "python3".to_string(),
                args: vec![script_path.to_string_lossy().to_string()],
                env: HashMap::new(),
            },
        };

        let client = McpClient::connect(&config).await.expect("connect");
        let err = client
            .call_tool("any", serde_json::json!({}))
            .await
            .expect_err("call should fail");
        assert!(err.to_string().contains("tool exploded"));
        client.disconnect().await.expect("disconnect");
    }
}
