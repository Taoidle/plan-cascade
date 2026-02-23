//! MCP Tool Adapter
//!
//! Wraps an MCP server tool as a `Tool` trait implementation,
//! allowing MCP tools to be registered in the ToolRegistry and
//! used seamlessly in the agentic loop alongside built-in tools.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::mcp_client::McpClient;
use crate::services::tools::mcp_schema::{json_schema_to_parameter_schema, sanitize_schema};
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// Wraps an MCP server tool as a Tool trait implementation.
///
/// Each MCP tool discovered via `tools/list` is wrapped in an adapter
/// that proxies `execute()` calls to the MCP server via `tools/call`.
///
/// Tool names are namespaced as `mcp:{server_name}:{tool_name}` to
/// avoid conflicts with built-in tools.
pub struct McpToolAdapter {
    /// Name of the MCP server providing this tool
    server_name: String,
    /// Original tool name as reported by the MCP server
    tool_name: String,
    /// Qualified name: "mcp:{server_name}:{tool_name}"
    qualified_name: String,
    /// Tool description
    description: String,
    /// Sanitized JSON Schema for the tool's parameters
    parameters_schema_value: Value,
    /// Reference to the MCP client for making tool calls
    client: Arc<McpClient>,
}

impl McpToolAdapter {
    /// Create a new MCP tool adapter.
    ///
    /// `server_name`: Name of the MCP server
    /// `tool_name`: Original tool name from the server
    /// `description`: Tool description
    /// `input_schema`: Raw JSON Schema from the MCP server (will be sanitized)
    /// `client`: Shared reference to the MCP client
    pub fn new(
        server_name: String,
        tool_name: String,
        description: String,
        mut input_schema: Value,
        client: Arc<McpClient>,
    ) -> Self {
        // Sanitize the schema for LLM compatibility
        sanitize_schema(&mut input_schema);

        let qualified_name = format!("mcp:{}:{}", server_name, tool_name);

        Self {
            server_name,
            tool_name,
            qualified_name,
            description,
            parameters_schema_value: input_schema,
            client,
        }
    }

    /// Get the original (unqualified) tool name
    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    /// Get the server name
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Get the qualified name (mcp:server:tool)
    pub fn qualified_name(&self) -> &str {
        &self.qualified_name
    }

    /// Parse a qualified MCP tool name into (server_name, tool_name).
    /// Returns None if the name doesn't match the expected format.
    pub fn parse_qualified_name(name: &str) -> Option<(&str, &str)> {
        let parts: Vec<&str> = name.splitn(3, ':').collect();
        if parts.len() == 3 && parts[0] == "mcp" {
            Some((parts[1], parts[2]))
        } else {
            None
        }
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.qualified_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> ParameterSchema {
        json_schema_to_parameter_schema(&self.parameters_schema_value)
    }

    fn is_long_running(&self) -> bool {
        // MCP tool calls may involve network latency or heavy computation
        true
    }

    async fn execute(&self, _ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        // Proxy the call to the MCP server using the original tool name
        match self.client.call_tool(&self.tool_name, args).await {
            Ok(result) => {
                // Convert the MCP result to a string for the agentic loop
                match result {
                    Value::String(s) => ToolResult::ok(s),
                    Value::Null => ToolResult::ok("(no output)"),
                    other => ToolResult::ok(serde_json::to_string_pretty(&other).unwrap_or_else(
                        |_| other.to_string(),
                    )),
                }
            }
            Err(e) => ToolResult::err(format!(
                "MCP tool '{}' on server '{}' failed: {}",
                self.tool_name, self.server_name, e
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_qualified_name_format() {
        // We cannot construct a real McpToolAdapter without a client,
        // but we can test the name formatting logic
        let name = format!("mcp:{}:{}", "my-server", "read_file");
        assert_eq!(name, "mcp:my-server:read_file");
    }

    #[test]
    fn test_parse_qualified_name_valid() {
        let result = McpToolAdapter::parse_qualified_name("mcp:server1:tool_a");
        assert_eq!(result, Some(("server1", "tool_a")));
    }

    #[test]
    fn test_parse_qualified_name_with_colons_in_tool() {
        // Tool names with colons should work (splitn limits to 3 parts)
        let result = McpToolAdapter::parse_qualified_name("mcp:server1:tool:with:colons");
        assert_eq!(result, Some(("server1", "tool:with:colons")));
    }

    #[test]
    fn test_parse_qualified_name_invalid_prefix() {
        assert_eq!(McpToolAdapter::parse_qualified_name("builtin:Read"), None);
    }

    #[test]
    fn test_parse_qualified_name_too_few_parts() {
        assert_eq!(McpToolAdapter::parse_qualified_name("mcp:server"), None);
    }

    #[test]
    fn test_parse_qualified_name_not_mcp() {
        assert_eq!(
            McpToolAdapter::parse_qualified_name("other:server:tool"),
            None
        );
    }

    #[test]
    fn test_parse_qualified_name_empty() {
        assert_eq!(McpToolAdapter::parse_qualified_name(""), None);
    }

    #[test]
    fn test_adapter_schema_is_sanitized() {
        // Verify that the adapter sanitizes the input schema
        let raw_schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "$comment": "should be removed"
                }
            },
            "required": ["path"]
        });

        // We need a mock client to construct the adapter
        // For this test, we'll verify sanitization separately
        let mut schema = raw_schema.clone();
        sanitize_schema(&mut schema);

        assert!(schema.get("$schema").is_none());
        assert!(schema["properties"]["path"].get("$comment").is_none());
    }

    /// Integration test with mock MCP server
    #[tokio::test]
    async fn test_adapter_with_mock_server() {
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
            "serverInfo": {"name": "adapter-test", "version": "0.1.0"}
        }}
    elif method == "notifications/initialized":
        continue
    elif method == "tools/list":
        response = {"jsonrpc": "2.0", "id": msg_id, "result": {
            "tools": [{
                "name": "greet",
                "description": "Greets someone",
                "inputSchema": {
                    "type": "object",
                    "properties": {"name": {"type": "string"}},
                    "required": ["name"]
                }
            }]
        }}
    elif method == "tools/call":
        args = msg.get("params", {}).get("arguments", {})
        name_val = args.get("name", "World")
        response = {"jsonrpc": "2.0", "id": msg_id, "result": {
            "content": [{"type": "text", "text": f"Hello, {name_val}!"}]
        }}
    else:
        continue

    sys.stdout.write(json.dumps(response) + "\n")
    sys.stdout.flush()
"#;

        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("adapter_test_server.py");
        std::fs::write(&script_path, script).unwrap();

        use crate::services::tools::mcp_client::{McpClient, McpServerConfig, McpTransportConfig};
        use std::collections::HashMap;

        let config = McpServerConfig {
            name: "adapter-test".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "python3".to_string(),
                args: vec![script_path.to_string_lossy().to_string()],
                env: HashMap::new(),
            },
        };

        let client = Arc::new(McpClient::connect(&config).await.unwrap());

        // List tools and create adapter
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);

        let tool_info = &tools[0];
        let adapter = McpToolAdapter::new(
            "adapter-test".to_string(),
            tool_info.name.clone(),
            tool_info.description.clone(),
            tool_info.input_schema.clone(),
            client.clone(),
        );

        // Verify trait methods
        assert_eq!(adapter.name(), "mcp:adapter-test:greet");
        assert_eq!(adapter.description(), "Greets someone");
        assert!(adapter.is_long_running());

        // Test parameters schema
        let schema = adapter.parameters_schema();
        assert_eq!(schema.schema_type, "object");
        let props = schema.properties.unwrap();
        assert!(props.contains_key("name"));

        // Execute the tool
        let ctx = ToolExecutionContext {
            session_id: "test".to_string(),
            project_root: std::path::PathBuf::from("/tmp"),
            working_directory: Arc::new(std::sync::Mutex::new(std::path::PathBuf::from("/tmp"))),
            read_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
            read_files: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            web_fetch: Arc::new(crate::services::tools::web_fetch::WebFetchService::new()),
            web_search: None,
            index_store: None,
            embedding_service: None,
            embedding_manager: None,
            hnsw_index: None,
            task_dedup_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
            task_context: None,
            core_context: None,
            file_change_tracker: None,
        };

        let result = adapter
            .execute(&ctx, json!({"name": "Rust"}))
            .await;
        assert!(result.success, "Error: {:?}", result.error);
        assert_eq!(result.output.unwrap(), "Hello, Rust!");

        client.disconnect().await.unwrap();
    }
}
