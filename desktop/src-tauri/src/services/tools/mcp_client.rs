//! MCP Client
//!
//! JSON-RPC 2.0 client for communicating with MCP (Model Context Protocol) servers.
//! Supports stdio and HTTP/SSE transports.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::utils::error::{AppError, AppResult};

/// Configuration for connecting to an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
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
    /// HTTP/SSE transport: connect to a remote server
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
}

/// JSON-RPC 2.0 request
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Option<u64>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[allow(dead_code)]
    data: Option<Value>,
}

/// Internal transport state for stdio connections
struct StdioTransport {
    process: Child,
    stdin: tokio::process::ChildStdin,
    stdout_reader: BufReader<tokio::process::ChildStdout>,
}

/// Internal transport state for HTTP connections
struct HttpTransport {
    base_url: String,
    client: reqwest::Client,
    headers: HashMap<String, String>,
}

/// Active transport connection
enum ActiveTransport {
    Stdio(StdioTransport),
    Http(HttpTransport),
}

/// MCP client for communicating with MCP servers
pub struct McpClient {
    transport: Mutex<ActiveTransport>,
    server_info: McpServerInfo,
    request_id: AtomicU64,
}

impl McpClient {
    /// Connect to an MCP server using the provided configuration.
    ///
    /// Performs the MCP initialization handshake:
    /// 1. Send `initialize` request
    /// 2. Receive server capabilities
    /// 3. Send `notifications/initialized` notification
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

    /// Connect via stdio transport
    async fn connect_stdio(
        name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> AppResult<Self> {
        // Build the process with inherited environment + custom env vars
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env {
            cmd.env(key, value);
        }

        let mut process = cmd.spawn().map_err(|e| {
            AppError::command(format!(
                "Failed to spawn MCP server '{}' (command: {}): {}",
                name, command, e
            ))
        })?;

        let stdin = process.stdin.take().ok_or_else(|| {
            AppError::command(format!("Failed to capture stdin for MCP server '{}'", name))
        })?;

        let stdout = process.stdout.take().ok_or_else(|| {
            AppError::command(format!(
                "Failed to capture stdout for MCP server '{}'",
                name
            ))
        })?;

        let stdout_reader = BufReader::new(stdout);

        let mut transport = StdioTransport {
            process,
            stdin,
            stdout_reader,
        };

        // Perform initialization handshake
        let request_id = AtomicU64::new(1);
        let server_info =
            Self::perform_init_handshake_stdio(&mut transport, &request_id, name).await?;

        Ok(Self {
            transport: Mutex::new(ActiveTransport::Stdio(transport)),
            server_info,
            request_id,
        })
    }

    /// Connect via HTTP transport
    async fn connect_http(
        name: &str,
        base_url: &str,
        headers: &HashMap<String, String>,
    ) -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::command(format!("Failed to create HTTP client: {}", e)))?;

        let mut transport = HttpTransport {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            headers: headers.clone(),
        };

        let request_id = AtomicU64::new(1);
        let server_info =
            Self::perform_init_handshake_http(&mut transport, &request_id, name).await?;

        Ok(Self {
            transport: Mutex::new(ActiveTransport::Http(transport)),
            server_info,
            request_id,
        })
    }

    /// Perform the MCP initialization handshake over stdio
    async fn perform_init_handshake_stdio(
        transport: &mut StdioTransport,
        request_id: &AtomicU64,
        name: &str,
    ) -> AppResult<McpServerInfo> {
        let id = request_id.fetch_add(1, Ordering::SeqCst);

        let init_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "plan-cascade-desktop",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
        };

        // Send initialize request
        Self::send_stdio_message(&mut transport.stdin, &init_request).await?;

        // Read response
        let response = Self::read_stdio_response(&mut transport.stdout_reader).await?;

        let result = response.result.ok_or_else(|| {
            let err_msg = response
                .error
                .map(|e| format!("code={}, message={}", e.code, e.message))
                .unwrap_or_else(|| "No result in initialize response".to_string());
            AppError::command(format!(
                "MCP server '{}' initialization failed: {}",
                name, err_msg
            ))
        })?;

        // Send initialized notification (no id, no response expected)
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        let notification_str = serde_json::to_string(&notification)
            .map_err(|e| AppError::command(format!("Failed to serialize notification: {}", e)))?;
        transport
            .stdin
            .write_all(notification_str.as_bytes())
            .await
            .map_err(|e| AppError::command(format!("Failed to send notification: {}", e)))?;
        transport
            .stdin
            .write_all(b"\n")
            .await
            .map_err(|e| AppError::command(format!("Failed to write newline: {}", e)))?;
        transport
            .stdin
            .flush()
            .await
            .map_err(|e| AppError::command(format!("Failed to flush: {}", e)))?;

        let protocol_version = result
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let capabilities = result
            .get("capabilities")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        let server_info_val = result
            .get("serverInfo")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        Ok(McpServerInfo {
            name: name.to_string(),
            protocol_version,
            capabilities,
            server_info: server_info_val,
        })
    }

    /// Perform the MCP initialization handshake over HTTP
    async fn perform_init_handshake_http(
        transport: &mut HttpTransport,
        request_id: &AtomicU64,
        name: &str,
    ) -> AppResult<McpServerInfo> {
        let id = request_id.fetch_add(1, Ordering::SeqCst);

        let init_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "plan-cascade-desktop",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
        };

        let response = Self::send_http_request(transport, &init_request).await?;

        let result = response.result.ok_or_else(|| {
            let err_msg = response
                .error
                .map(|e| format!("code={}, message={}", e.code, e.message))
                .unwrap_or_else(|| "No result in initialize response".to_string());
            AppError::command(format!(
                "MCP server '{}' initialization failed: {}",
                name, err_msg
            ))
        })?;

        // Send initialized notification
        let notification_id = request_id.fetch_add(1, Ordering::SeqCst);
        let notification = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: notification_id,
            method: "notifications/initialized".to_string(),
            params: Some(serde_json::json!({})),
        };
        // Best effort - notifications don't require a response
        let _ = Self::send_http_request(transport, &notification).await;

        let protocol_version = result
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let capabilities = result
            .get("capabilities")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        let server_info_val = result
            .get("serverInfo")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        Ok(McpServerInfo {
            name: name.to_string(),
            protocol_version,
            capabilities,
            server_info: server_info_val,
        })
    }

    /// Send a JSON-RPC message over stdio (newline-delimited JSON)
    async fn send_stdio_message(
        stdin: &mut tokio::process::ChildStdin,
        request: &JsonRpcRequest,
    ) -> AppResult<()> {
        let msg = serde_json::to_string(request).map_err(|e| {
            AppError::command(format!("Failed to serialize JSON-RPC request: {}", e))
        })?;
        stdin.write_all(msg.as_bytes()).await.map_err(|e| {
            AppError::command(format!("Failed to write to MCP server stdin: {}", e))
        })?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| AppError::command(format!("Failed to write newline: {}", e)))?;
        stdin
            .flush()
            .await
            .map_err(|e| AppError::command(format!("Failed to flush stdin: {}", e)))?;
        Ok(())
    }

    /// Read a JSON-RPC response from stdio (newline-delimited JSON)
    async fn read_stdio_response(
        reader: &mut BufReader<tokio::process::ChildStdout>,
    ) -> AppResult<JsonRpcResponse> {
        let mut line = String::new();

        // Read lines until we get a valid JSON-RPC response
        // Skip any empty lines or non-JSON output
        loop {
            line.clear();
            let bytes_read = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                reader.read_line(&mut line),
            )
            .await
            .map_err(|_| AppError::command("Timeout waiting for MCP server response".to_string()))?
            .map_err(|e| {
                AppError::command(format!("Failed to read from MCP server stdout: {}", e))
            })?;

            if bytes_read == 0 {
                return Err(AppError::command(
                    "MCP server closed stdout (process may have crashed)".to_string(),
                ));
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Try to parse as JSON-RPC response
            match serde_json::from_str::<JsonRpcResponse>(trimmed) {
                Ok(response) => return Ok(response),
                Err(_) => {
                    // Skip non-JSON lines (e.g., log output from the server)
                    continue;
                }
            }
        }
    }

    /// Send a JSON-RPC request over HTTP
    async fn send_http_request(
        transport: &HttpTransport,
        request: &JsonRpcRequest,
    ) -> AppResult<JsonRpcResponse> {
        let url = format!("{}/jsonrpc", transport.base_url);

        let mut req_builder = transport
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        for (key, value) in &transport.headers {
            req_builder = req_builder.header(key, value);
        }

        let body = serde_json::to_string(request)
            .map_err(|e| AppError::command(format!("Failed to serialize request: {}", e)))?;

        let response = req_builder
            .body(body)
            .send()
            .await
            .map_err(|e| AppError::command(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::command(format!(
                "MCP server returned HTTP {}: {}",
                response.status(),
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unknown".to_string())
            )));
        }

        let text = response
            .text()
            .await
            .map_err(|e| AppError::command(format!("Failed to read response body: {}", e)))?;

        serde_json::from_str::<JsonRpcResponse>(&text)
            .map_err(|e| AppError::command(format!("Failed to parse JSON-RPC response: {}", e)))
    }

    /// List all tools available on the connected MCP server
    pub async fn list_tools(&self) -> AppResult<Vec<McpToolInfo>> {
        let response = self.send_request("tools/list", None).await?;

        let result = response.result.ok_or_else(|| {
            let err_msg = response
                .error
                .map(|e| format!("code={}, message={}", e.code, e.message))
                .unwrap_or_else(|| "No result in tools/list response".to_string());
            AppError::command(format!("tools/list failed: {}", err_msg))
        })?;

        let tools_array = result
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tools = Vec::new();
        for tool_val in tools_array {
            let name = tool_val
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let description = tool_val
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let input_schema = tool_val
                .get("inputSchema")
                .cloned()
                .unwrap_or(serde_json::json!({"type": "object"}));

            tools.push(McpToolInfo {
                name,
                description,
                input_schema,
            });
        }

        Ok(tools)
    }

    /// Call a tool on the connected MCP server
    pub async fn call_tool(&self, name: &str, args: Value) -> AppResult<Value> {
        let params = serde_json::json!({
            "name": name,
            "arguments": args,
        });

        let response = self.send_request("tools/call", Some(params)).await?;

        if let Some(error) = response.error {
            return Err(AppError::command(format!(
                "MCP tool '{}' call failed: [{}] {}",
                name, error.code, error.message
            )));
        }

        let result = response.result.unwrap_or(Value::Null);

        // Extract content from MCP tool result
        // MCP returns { content: [{ type: "text", text: "..." }] }
        if let Some(content_array) = result.get("content").and_then(|v| v.as_array()) {
            let mut text_parts: Vec<String> = Vec::new();
            for content in content_array {
                if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                    text_parts.push(text.to_string());
                }
            }
            if !text_parts.is_empty() {
                return Ok(Value::String(text_parts.join("\n")));
            }
        }

        // Return raw result if no text content found
        Ok(result)
    }

    /// Disconnect from the MCP server
    pub async fn disconnect(&self) -> AppResult<()> {
        let mut transport = self.transport.lock().await;
        match &mut *transport {
            ActiveTransport::Stdio(ref mut stdio) => {
                // Try to send a clean shutdown, but don't fail if it errors
                let _ = stdio.stdin.shutdown().await;
                let _ = stdio.process.kill().await;
                Ok(())
            }
            ActiveTransport::Http(_) => {
                // HTTP connections are stateless, nothing to close
                Ok(())
            }
        }
    }

    /// Get server info
    pub fn server_info(&self) -> &McpServerInfo {
        &self.server_info
    }

    /// Send a JSON-RPC request using the active transport
    async fn send_request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> AppResult<JsonRpcResponse> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let mut transport = self.transport.lock().await;
        match &mut *transport {
            ActiveTransport::Stdio(ref mut stdio) => {
                Self::send_stdio_message(&mut stdio.stdin, &request).await?;
                Self::read_stdio_response(&mut stdio.stdout_reader).await
            }
            ActiveTransport::Http(ref transport) => {
                Self::send_http_request(transport, &request).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_config_stdio() {
        let config = McpServerConfig {
            name: "test-server".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "node".to_string(),
                args: vec!["server.js".to_string()],
                env: HashMap::new(),
            },
        };
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
    fn test_mcp_server_config_http() {
        let config = McpServerConfig {
            name: "http-server".to_string(),
            transport: McpTransportConfig::Http {
                base_url: "http://localhost:8080".to_string(),
                headers: HashMap::new(),
            },
        };
        assert_eq!(config.name, "http-server");
        match &config.transport {
            McpTransportConfig::Http { base_url, .. } => {
                assert_eq!(base_url, "http://localhost:8080");
            }
            _ => panic!("Expected Http transport"),
        }
    }

    #[test]
    fn test_mcp_tool_info_serde() {
        let tool = McpToolInfo {
            name: "read_file".to_string(),
            description: "Reads a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        };

        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: McpToolInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "read_file");
        assert_eq!(deserialized.description, "Reads a file");
    }

    #[test]
    fn test_mcp_server_info() {
        let info = McpServerInfo {
            name: "test".to_string(),
            protocol_version: "2024-11-05".to_string(),
            capabilities: serde_json::json!({"tools": {}}),
            server_info: serde_json::json!({"name": "test-server", "version": "1.0"}),
        };
        assert_eq!(info.name, "test");
        assert_eq!(info.protocol_version, "2024-11-05");
    }

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "tools/list".to_string(),
            params: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["method"], "tools/list");
        // params should be absent when None
        assert!(parsed.get("params").is_none());
    }

    #[test]
    fn test_json_rpc_request_with_params() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 2,
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "read_file",
                "arguments": {"path": "/tmp/test.txt"}
            })),
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["params"]["name"], "read_file");
    }

    #[test]
    fn test_json_rpc_response_success() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"tools": [{"name": "test", "description": "A test tool"}]}
        }"#;

        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_json_rpc_response_error() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32601, "message": "Method not found"}
        }"#;

        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        let error = response.error.unwrap();
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
    }

    #[test]
    fn test_mcp_transport_config_serde() {
        let stdio_config = McpTransportConfig::Stdio {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@mcp/server".to_string()],
            env: {
                let mut m = HashMap::new();
                m.insert("API_KEY".to_string(), "secret".to_string());
                m
            },
        };

        let json = serde_json::to_string(&stdio_config).unwrap();
        assert!(json.contains("npx"));
        assert!(json.contains("API_KEY"));
    }

    #[tokio::test]
    async fn test_connect_stdio_nonexistent_command() {
        let config = McpServerConfig {
            name: "bad-server".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "/nonexistent/command/that/does/not/exist".to_string(),
                args: vec![],
                env: HashMap::new(),
            },
        };

        let result = McpClient::connect(&config).await;
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("Failed to spawn"),
            "Unexpected error message: {}",
            err
        );
    }

    /// Test that a mock stdio MCP server interaction works end-to-end.
    /// We create a tiny script that responds to JSON-RPC messages.
    #[tokio::test]
    async fn test_stdio_mock_server_interaction() {
        // Create a mock MCP server script that:
        // 1. Reads JSON-RPC requests from stdin (line by line)
        // 2. Responds to "initialize" with server info
        // 3. Responds to "tools/list" with a test tool
        // 4. Responds to "tools/call" with a result
        let script = r#"
import sys, json

def respond(request_id, result):
    response = {"jsonrpc": "2.0", "id": request_id, "result": result}
    sys.stdout.write(json.dumps(response) + "\n")
    sys.stdout.flush()

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
        respond(msg_id, {
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "mock-server", "version": "0.1.0"}
        })
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        respond(msg_id, {
            "tools": [
                {
                    "name": "echo",
                    "description": "Echoes the input",
                    "inputSchema": {
                        "type": "object",
                        "properties": {"message": {"type": "string"}},
                        "required": ["message"]
                    }
                }
            ]
        })
    elif method == "tools/call":
        tool_name = msg.get("params", {}).get("name", "")
        tool_args = msg.get("params", {}).get("arguments", {})
        if tool_name == "echo":
            respond(msg_id, {
                "content": [{"type": "text", "text": tool_args.get("message", "")}]
            })
        else:
            response = {"jsonrpc": "2.0", "id": msg_id, "error": {"code": -32601, "message": "Unknown tool"}}
            sys.stdout.write(json.dumps(response) + "\n")
            sys.stdout.flush()
"#;

        // Write script to a temp file
        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("mock_mcp_server.py");
        std::fs::write(&script_path, script).unwrap();

        let config = McpServerConfig {
            name: "mock-server".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "python3".to_string(),
                args: vec![script_path.to_string_lossy().to_string()],
                env: HashMap::new(),
            },
        };

        // Connect to mock server
        let client = McpClient::connect(&config).await.unwrap();

        // Verify server info
        assert_eq!(client.server_info().name, "mock-server");
        assert_eq!(client.server_info().protocol_version, "2024-11-05");

        // List tools
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
        assert_eq!(tools[0].description, "Echoes the input");

        // Call a tool
        let result = client
            .call_tool("echo", serde_json::json!({"message": "hello world"}))
            .await
            .unwrap();
        assert_eq!(result, Value::String("hello world".to_string()));

        // Disconnect
        client.disconnect().await.unwrap();
    }

    #[tokio::test]
    async fn test_call_tool_error_response() {
        // Create a mock server that returns an error for tools/call
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
            "serverInfo": {"name": "error-server", "version": "0.1.0"}
        }}
        sys.stdout.write(json.dumps(response) + "\n")
        sys.stdout.flush()
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        response = {"jsonrpc": "2.0", "id": msg_id, "result": {"tools": []}}
        sys.stdout.write(json.dumps(response) + "\n")
        sys.stdout.flush()
    elif method == "tools/call":
        response = {"jsonrpc": "2.0", "id": msg_id, "error": {"code": -32000, "message": "Tool execution failed: file not found"}}
        sys.stdout.write(json.dumps(response) + "\n")
        sys.stdout.flush()
"#;

        let temp_dir = tempfile::tempdir().unwrap();
        let script_path = temp_dir.path().join("error_mcp_server.py");
        std::fs::write(&script_path, script).unwrap();

        let config = McpServerConfig {
            name: "error-server".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "python3".to_string(),
                args: vec![script_path.to_string_lossy().to_string()],
                env: HashMap::new(),
            },
        };

        let client = McpClient::connect(&config).await.unwrap();

        let result = client.call_tool("nonexistent", serde_json::json!({})).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Tool execution failed"), "Error: {}", err);

        client.disconnect().await.unwrap();
    }
}
