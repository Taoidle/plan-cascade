//! LSP Client — JSON-RPC 2.0 Transport over stdin/stdout
//!
//! Minimal Language Server Protocol client that communicates with language
//! servers using JSON-RPC 2.0 over stdio. Handles the initialize/shutdown
//! lifecycle, Content-Length header framing, and request/response correlation.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use std::str::FromStr;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};
use tracing::{debug, warn};

/// Default timeout for LSP requests (30 seconds).
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// A JSON-RPC 2.0 request message.
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: i64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// A JSON-RPC 2.0 notification message (no id).
#[derive(Debug, Serialize)]
struct JsonRpcNotification {
    jsonrpc: &'static str,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// A JSON-RPC 2.0 response message.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<i64>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

/// Minimal LSP client for communicating with language servers.
///
/// Spawns a language server process, communicates over stdin/stdout using
/// JSON-RPC 2.0 with Content-Length header framing, and correlates
/// request/response pairs by ID.
pub struct LspClient {
    /// The spawned language server child process.
    child: Option<Child>,
    /// Writer to the child's stdin.
    stdin: Arc<tokio::sync::Mutex<BufWriter<ChildStdin>>>,
    /// Next request ID (atomic for concurrent access).
    next_id: AtomicI64,
    /// Pending requests waiting for a response, keyed by request ID.
    pending: Arc<DashMap<i64, oneshot::Sender<Value>>>,
    /// Background reader task handle.
    _reader_handle: tokio::task::JoinHandle<()>,
    /// Server capabilities from the initialize response.
    pub capabilities: Option<lsp_types::ServerCapabilities>,
}

impl LspClient {
    /// Spawn a language server process and complete the initialize handshake.
    ///
    /// 1. Spawns the server with `command` and `args`
    /// 2. Sends `initialize` request with the given `root_uri`
    /// 3. Waits for the `initialize` response
    /// 4. Sends `initialized` notification
    pub async fn start(command: &str, args: &[&str], root_uri: &str) -> anyhow::Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn LSP server '{}': {}", command, e))?;

        let child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdin of LSP server"))?;
        let child_stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout of LSP server"))?;

        let stdin = Arc::new(tokio::sync::Mutex::new(BufWriter::new(child_stdin)));
        let pending: Arc<DashMap<i64, oneshot::Sender<Value>>> = Arc::new(DashMap::new());

        // Spawn background reader task
        let pending_clone = Arc::clone(&pending);
        let reader_handle = tokio::task::spawn(async move {
            Self::reader_loop(child_stdout, pending_clone).await;
        });

        let mut client = Self {
            child: Some(child),
            stdin,
            next_id: AtomicI64::new(1),
            pending,
            _reader_handle: reader_handle,
            capabilities: None,
        };

        // Send initialize request
        let root_uri = if root_uri.starts_with("file://") {
            root_uri.to_string()
        } else {
            format!("file://{}", root_uri)
        };

        let init_params = lsp_types::InitializeParams {
            root_uri: Some(
                lsp_types::Uri::from_str(&root_uri)
                    .map_err(|e| anyhow::anyhow!("Invalid root URI '{}': {}", root_uri, e))?,
            ),
            capabilities: lsp_types::ClientCapabilities {
                text_document: Some(lsp_types::TextDocumentClientCapabilities {
                    hover: Some(lsp_types::HoverClientCapabilities {
                        dynamic_registration: Some(false),
                        content_format: Some(vec![lsp_types::MarkupKind::Markdown]),
                    }),
                    references: Some(lsp_types::DynamicRegistrationClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    definition: Some(lsp_types::GotoCapability {
                        dynamic_registration: Some(false),
                        link_support: Some(false),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let init_result: lsp_types::InitializeResult = client
            .request::<lsp_types::request::Initialize>(init_params)
            .await?;

        client.capabilities = Some(init_result.capabilities);

        // Send initialized notification
        client
            .notify::<lsp_types::notification::Initialized>(lsp_types::InitializedParams {})
            .await?;

        Ok(client)
    }

    /// Send a request and wait for the response (with timeout).
    pub async fn request<R: lsp_types::request::Request>(
        &self,
        params: R::Params,
    ) -> anyhow::Result<R::Result>
    where
        R::Params: Serialize,
        R::Result: for<'de> Deserialize<'de>,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let params_value = serde_json::to_value(params)?;

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: R::METHOD.to_string(),
            params: Some(params_value),
        };

        // Register the pending response channel
        let (tx, rx) = oneshot::channel::<Value>();
        self.pending.insert(id, tx);

        // Send the request
        self.send_message(&serde_json::to_value(&request)?).await?;

        // Wait for response with timeout
        let result = timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS), rx)
            .await
            .map_err(|_| {
                self.pending.remove(&id);
                anyhow::anyhow!(
                    "LSP request '{}' (id={}) timed out after {}s",
                    R::METHOD,
                    id,
                    REQUEST_TIMEOUT_SECS
                )
            })?
            .map_err(|_| {
                anyhow::anyhow!(
                    "LSP response channel closed for '{}' (id={})",
                    R::METHOD,
                    id
                )
            })?;

        let parsed: R::Result = serde_json::from_value(result)?;
        Ok(parsed)
    }

    /// Send a notification (no response expected).
    pub async fn notify<N: lsp_types::notification::Notification>(
        &self,
        params: N::Params,
    ) -> anyhow::Result<()>
    where
        N::Params: Serialize,
    {
        let params_value = serde_json::to_value(params)?;

        let notification = JsonRpcNotification {
            jsonrpc: "2.0",
            method: N::METHOD.to_string(),
            params: Some(params_value),
        };

        self.send_message(&serde_json::to_value(&notification)?)
            .await
    }

    /// Graceful shutdown: shutdown request -> exit notification -> wait.
    pub async fn shutdown(mut self) -> anyhow::Result<()> {
        // Send shutdown request
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: "shutdown".to_string(),
            params: None,
        };

        let (tx, rx) = oneshot::channel::<Value>();
        self.pending.insert(id, tx);
        let _ = self.send_message(&serde_json::to_value(&request)?).await;

        // Wait for shutdown response (with a shorter timeout)
        let _ = timeout(Duration::from_secs(5), rx).await;

        // Send exit notification
        let exit = JsonRpcNotification {
            jsonrpc: "2.0",
            method: "exit".to_string(),
            params: None,
        };
        let _ = self.send_message(&serde_json::to_value(&exit)?).await;

        // Wait for process to exit
        if let Some(ref mut child) = self.child {
            let _ = timeout(Duration::from_secs(5), child.wait()).await;
        }

        Ok(())
    }

    /// Send a JSON-RPC message with Content-Length header framing.
    async fn send_message(&self, message: &Value) -> anyhow::Result<()> {
        let body = serde_json::to_string(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());

        let mut stdin = self.stdin.lock().await;
        stdin.write_all(header.as_bytes()).await?;
        stdin.write_all(body.as_bytes()).await?;
        stdin.flush().await?;

        debug!("LSP -> {}", body);
        Ok(())
    }

    /// Background reader loop: reads JSON-RPC responses from stdout and
    /// dispatches them to pending request channels.
    async fn reader_loop(stdout: ChildStdout, pending: Arc<DashMap<i64, oneshot::Sender<Value>>>) {
        let mut reader = BufReader::new(stdout);
        let mut header_buf = String::new();

        loop {
            // Read headers until empty line
            let mut content_length: Option<usize> = None;
            header_buf.clear();

            loop {
                header_buf.clear();
                match reader.read_line(&mut header_buf).await {
                    Ok(0) => return, // EOF
                    Ok(_) => {
                        let line = header_buf.trim();
                        if line.is_empty() {
                            break; // End of headers
                        }
                        if let Some(len_str) = line.strip_prefix("Content-Length: ") {
                            if let Ok(len) = len_str.trim().parse::<usize>() {
                                content_length = Some(len);
                            }
                        }
                    }
                    Err(_) => return,
                }
            }

            let content_length = match content_length {
                Some(len) => len,
                None => continue,
            };

            // Read the body
            let mut body = vec![0u8; content_length];
            if reader.read_exact(&mut body).await.is_err() {
                return;
            }

            let body_str = match String::from_utf8(body) {
                Ok(s) => s,
                Err(_) => continue,
            };

            debug!("LSP <- {}", body_str);

            // Parse the response
            let response: JsonRpcResponse = match serde_json::from_str(&body_str) {
                Ok(r) => r,
                Err(_) => {
                    // Might be a notification from the server, which we skip
                    continue;
                }
            };

            // Dispatch to pending request
            if let Some(id) = response.id {
                if let Some((_, sender)) = pending.remove(&id) {
                    if let Some(error) = response.error {
                        // Send error as a JSON value so caller can detect it
                        let err_value = serde_json::json!({
                            "__lsp_error": true,
                            "code": error.code,
                            "message": error.message,
                        });
                        let _ = sender.send(err_value);
                    } else {
                        let result = response.result.unwrap_or(Value::Null);
                        let _ = sender.send(result);
                    }
                }
            }
            // else: server notification — we ignore these for now
        }
    }
}

// =============================================================================
// Helper: Format a JSON-RPC message with Content-Length framing
// =============================================================================

/// Encode a JSON-RPC message body with Content-Length header.
/// Exposed for testing.
pub fn encode_message(body: &str) -> Vec<u8> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut output = Vec::with_capacity(header.len() + body.len());
    output.extend_from_slice(header.as_bytes());
    output.extend_from_slice(body.as_bytes());
    output
}

/// Parse a Content-Length framed message from bytes.
/// Returns `(body, remaining_bytes)` if a complete message is found.
pub fn decode_message(input: &[u8]) -> Option<(String, usize)> {
    let input_str = std::str::from_utf8(input).ok()?;

    // Find the header separator
    let header_end = input_str.find("\r\n\r\n")?;
    let header_section = &input_str[..header_end];

    // Extract Content-Length
    let mut content_length = None;
    for line in header_section.split("\r\n") {
        if let Some(len_str) = line.strip_prefix("Content-Length: ") {
            content_length = len_str.trim().parse::<usize>().ok();
        }
    }

    let content_length = content_length?;
    let body_start = header_end + 4; // skip \r\n\r\n
    let body_end = body_start + content_length;

    if input.len() < body_end {
        return None; // Incomplete message
    }

    let body = input_str[body_start..body_end].to_string();
    Some((body, body_end))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Test: JSON-RPC message framing roundtrip
    // =========================================================================

    #[test]
    fn test_encode_message_format() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let encoded = encode_message(body);
        let expected = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        assert_eq!(String::from_utf8(encoded).unwrap(), expected);
    }

    #[test]
    fn test_decode_message_roundtrip() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#;
        let encoded = encode_message(body);

        let (decoded_body, consumed) = decode_message(&encoded).unwrap();
        assert_eq!(decoded_body, body);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_decode_message_incomplete() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":null}"#;
        let mut encoded = encode_message(body);
        // Truncate to simulate incomplete message
        encoded.truncate(encoded.len() - 5);

        assert!(decode_message(&encoded).is_none());
    }

    #[test]
    fn test_decode_message_multiple() {
        let body1 = r#"{"jsonrpc":"2.0","id":1,"result":"a"}"#;
        let body2 = r#"{"jsonrpc":"2.0","id":2,"result":"b"}"#;

        let mut combined = encode_message(body1);
        combined.extend_from_slice(&encode_message(body2));

        let (decoded1, consumed1) = decode_message(&combined).unwrap();
        assert_eq!(decoded1, body1);

        let (decoded2, consumed2) = decode_message(&combined[consumed1..]).unwrap();
        assert_eq!(decoded2, body2);
        assert_eq!(consumed1 + consumed2, combined.len());
    }

    #[test]
    fn test_encode_empty_body() {
        let encoded = encode_message("");
        assert_eq!(
            String::from_utf8(encoded).unwrap(),
            "Content-Length: 0\r\n\r\n"
        );
    }

    // =========================================================================
    // Test: Request ID correlation
    // =========================================================================

    #[tokio::test]
    async fn test_request_id_correlation() {
        let pending: Arc<DashMap<i64, oneshot::Sender<Value>>> = Arc::new(DashMap::new());

        // Register three pending requests
        let (tx1, rx1) = oneshot::channel::<Value>();
        let (tx2, rx2) = oneshot::channel::<Value>();
        let (tx3, rx3) = oneshot::channel::<Value>();

        pending.insert(1, tx1);
        pending.insert(2, tx2);
        pending.insert(3, tx3);

        // Resolve request 2 first (out of order)
        if let Some((_, sender)) = pending.remove(&2) {
            let _ = sender.send(serde_json::json!("result_2"));
        }

        // Resolve request 3
        if let Some((_, sender)) = pending.remove(&3) {
            let _ = sender.send(serde_json::json!("result_3"));
        }

        // Resolve request 1
        if let Some((_, sender)) = pending.remove(&1) {
            let _ = sender.send(serde_json::json!("result_1"));
        }

        // Verify each received the correct response
        assert_eq!(rx1.await.unwrap(), serde_json::json!("result_1"));
        assert_eq!(rx2.await.unwrap(), serde_json::json!("result_2"));
        assert_eq!(rx3.await.unwrap(), serde_json::json!("result_3"));
    }

    // =========================================================================
    // Test: Timeout on unresponsive server (simulated via channel)
    // =========================================================================

    #[tokio::test]
    async fn test_request_timeout() {
        let (_tx, rx) = oneshot::channel::<Value>();

        // Attempt to receive with a very short timeout
        let result = timeout(Duration::from_millis(50), rx).await;
        assert!(result.is_err(), "Should timeout when no response arrives");
    }

    // =========================================================================
    // Test: AtomicI64 ID generation
    // =========================================================================

    #[test]
    fn test_atomic_id_generation() {
        let counter = AtomicI64::new(1);
        let id1 = counter.fetch_add(1, Ordering::Relaxed);
        let id2 = counter.fetch_add(1, Ordering::Relaxed);
        let id3 = counter.fetch_add(1, Ordering::Relaxed);

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    // =========================================================================
    // Test: JSON-RPC request serialization
    // =========================================================================

    #[test]
    fn test_jsonrpc_request_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 42,
            method: "textDocument/hover".to_string(),
            params: Some(serde_json::json!({"textDocument": {"uri": "file:///test.rs"}})),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 42);
        assert_eq!(json["method"], "textDocument/hover");
        assert!(json["params"].is_object());
    }

    #[test]
    fn test_jsonrpc_notification_no_id() {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0",
            method: "initialized".to_string(),
            params: Some(serde_json::json!({})),
        };

        let json = serde_json::to_string(&notif).unwrap();
        assert!(
            !json.contains("\"id\""),
            "Notification should not have id field"
        );
        assert!(json.contains("\"method\":\"initialized\""));
    }

    // =========================================================================
    // Test: JSON-RPC response deserialization
    // =========================================================================

    #[test]
    fn test_jsonrpc_response_with_result() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, Some(1));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_jsonrpc_response_with_error() {
        let json =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, Some(1));
        assert!(response.result.is_none());
        let err = response.error.unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }

    #[test]
    fn test_jsonrpc_response_notification_no_id() {
        let json = r#"{"jsonrpc":"2.0","method":"window/logMessage","params":{"type":3,"message":"test"}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, None);
    }

    // =========================================================================
    // Test: Concurrent request ID uniqueness
    // =========================================================================

    #[test]
    fn test_concurrent_id_uniqueness() {
        use std::collections::HashSet;
        use std::sync::Arc;
        use std::thread;

        let counter = Arc::new(AtomicI64::new(1));
        let mut handles = vec![];

        for _ in 0..10 {
            let counter_clone = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                let mut ids = Vec::new();
                for _ in 0..100 {
                    ids.push(counter_clone.fetch_add(1, Ordering::Relaxed));
                }
                ids
            }));
        }

        let mut all_ids = HashSet::new();
        for handle in handles {
            for id in handle.join().unwrap() {
                assert!(all_ids.insert(id), "Duplicate ID detected: {}", id);
            }
        }
        assert_eq!(all_ids.len(), 1000);
    }
}
