//! A2A Client
//!
//! HTTP client for Agent-to-Agent protocol communication.
//! Supports agent discovery, task execution (JSON-RPC 2.0),
//! and streaming responses via SSE (Server-Sent Events).

use std::collections::VecDeque;
use std::pin::Pin;
use std::time::Duration;

use futures_util::{Stream, StreamExt};

use super::discovery::discover_agent;
use super::types::{
    A2aError, A2aStreamEvent, A2aTaskRequest, A2aTaskResponse, AgentCard,
};

/// Default request timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Configuration for the A2A client.
#[derive(Debug, Clone)]
pub struct A2aClientConfig {
    /// Request timeout duration.
    pub timeout: Duration,
    /// Optional bearer token for authenticated agents.
    pub auth_token: Option<String>,
}

impl Default for A2aClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            auth_token: None,
        }
    }
}

/// HTTP client for A2A (Agent-to-Agent) protocol communication.
///
/// Provides methods for:
/// - Discovering remote agents via `/.well-known/agent.json`
/// - Sending task requests via JSON-RPC 2.0
/// - Receiving streaming responses via SSE
pub struct A2aClient {
    client: reqwest::Client,
    config: A2aClientConfig,
}

impl A2aClient {
    /// Creates a new A2A client with default configuration.
    pub fn new() -> Result<Self, A2aError> {
        Self::with_config(A2aClientConfig::default())
    }

    /// Creates a new A2A client with the given configuration.
    pub fn with_config(config: A2aClientConfig) -> Result<Self, A2aError> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| A2aError::Network(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, config })
    }

    /// Creates a new A2A client wrapping an existing reqwest::Client.
    ///
    /// Useful for testing or when the caller wants to control the client
    /// configuration (e.g., custom TLS, proxy settings).
    pub fn with_reqwest_client(client: reqwest::Client, config: A2aClientConfig) -> Self {
        Self { client, config }
    }

    /// Discovers a remote agent by fetching its agent card.
    ///
    /// Sends GET `{base_url}/.well-known/agent.json` and parses the response.
    pub async fn discover(&self, base_url: &str) -> Result<AgentCard, A2aError> {
        discover_agent(&self.client, base_url).await
    }

    /// Sends a task request to a remote agent endpoint and waits for the response.
    ///
    /// This is the non-streaming variant: the entire response is returned at once.
    ///
    /// # Arguments
    /// * `endpoint` - The agent's task endpoint URL
    /// * `request` - The JSON-RPC 2.0 task request
    pub async fn send_task(
        &self,
        endpoint: &str,
        request: A2aTaskRequest,
    ) -> Result<A2aTaskResponse, A2aError> {
        let mut req_builder = self.client
            .post(endpoint)
            .json(&request);

        if let Some(ref token) = self.config.auth_token {
            req_builder = req_builder.bearer_auth(token);
        }

        let response = req_builder.send().await?;
        let status = response.status().as_u16();

        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(A2aError::HttpError { status, body });
        }

        let task_response: A2aTaskResponse = response.json().await.map_err(|e| {
            A2aError::InvalidResponse(format!("Failed to parse task response: {}", e))
        })?;

        Ok(task_response)
    }

    /// Sends a task request and returns a stream of SSE events.
    ///
    /// The remote agent responds with `Content-Type: text/event-stream` and
    /// sends incremental updates as Server-Sent Events. Each SSE `data:` line
    /// is parsed as an `A2aStreamEvent`.
    ///
    /// # Arguments
    /// * `endpoint` - The agent's task endpoint URL
    /// * `request` - The JSON-RPC 2.0 task request (should have `stream: true`)
    pub async fn send_task_streaming(
        &self,
        endpoint: &str,
        request: A2aTaskRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<A2aStreamEvent, A2aError>> + Send>>, A2aError> {
        let mut req_builder = self.client
            .post(endpoint)
            .json(&request);

        if let Some(ref token) = self.config.auth_token {
            req_builder = req_builder.bearer_auth(token);
        }

        let response = req_builder.send().await?;
        let status = response.status().as_u16();

        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(A2aError::HttpError { status, body });
        }

        // Convert the response body into a byte stream, then parse SSE events.
        // Use the same SSE parsing pattern as the OpenAI provider.
        let byte_stream = response.bytes_stream();
        let event_stream = parse_byte_stream_as_sse(byte_stream);

        Ok(Box::pin(event_stream))
    }
}

/// Parses a raw byte stream (from reqwest response) into A2A SSE events.
///
/// Buffers incoming byte chunks, splits on newlines, and parses `data:` lines
/// as JSON `A2aStreamEvent`s. Follows the SSE specification:
///
/// ```text
/// data: {"type":"text_delta","content":"Hello"}
///
/// data: {"type":"status_update","task_id":"t-1","status":"completed"}
///
/// data: [DONE]
/// ```
fn parse_byte_stream_as_sse<S>(byte_stream: S) -> impl Stream<Item = Result<A2aStreamEvent, A2aError>> + Send
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    // Use futures_util::stream::unfold to manage internal state
    // while processing the byte stream into SSE events.
    let state = SseParserState {
        inner: Box::pin(byte_stream),
        buffer: String::new(),
        pending_events: VecDeque::new(),
    };

    futures_util::stream::unfold(state, |mut state| async move {
        // Return any pending events first (FIFO order)
        if let Some(event) = state.pending_events.pop_front() {
            return Some((event, state));
        }

        loop {
            use futures_util::StreamExt;
            match state.inner.next().await {
                Some(Ok(chunk)) => {
                    state.buffer.push_str(&String::from_utf8_lossy(&chunk));

                    // Process complete lines
                    while let Some(pos) = state.buffer.find('\n') {
                        let line = state.buffer[..pos].to_string();
                        state.buffer = state.buffer[pos + 1..].to_string();

                        if let Some(event) = parse_sse_line(&line) {
                            state.pending_events.push_back(event);
                        }
                    }

                    // Return first pending event if any
                    if let Some(event) = state.pending_events.pop_front() {
                        return Some((event, state));
                    }
                    // No events yet, continue reading
                }
                Some(Err(e)) => {
                    return Some((
                        Err(A2aError::Network(format!("Stream read error: {}", e))),
                        state,
                    ));
                }
                None => {
                    // Stream ended. Process any remaining buffer content.
                    if !state.buffer.is_empty() {
                        let remaining = std::mem::take(&mut state.buffer);
                        if let Some(event) = parse_sse_line(&remaining) {
                            return Some((event, state));
                        }
                    }
                    return None;
                }
            }
        }
    })
}

/// Internal state for the SSE byte stream parser.
struct SseParserState {
    inner: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    buffer: String,
    pending_events: VecDeque<Result<A2aStreamEvent, A2aError>>,
}

/// Parses a single SSE line into an optional stream event.
///
/// Returns `None` for empty lines, comments, [DONE], and non-data fields.
/// Returns `Some(Ok(event))` for valid data lines, `Some(Err(...))` for
/// data lines with invalid JSON.
fn parse_sse_line(line: &str) -> Option<Result<A2aStreamEvent, A2aError>> {
    let trimmed = line.trim();

    // Skip empty lines and comments
    if trimmed.is_empty() || trimmed.starts_with(':') {
        return None;
    }

    // Parse SSE data lines
    if let Some(data) = trimmed.strip_prefix("data: ") {
        // [DONE] signals end of stream
        if data == "[DONE]" {
            return None;
        }
        match serde_json::from_str::<A2aStreamEvent>(data) {
            Ok(event) => Some(Ok(event)),
            Err(e) => Some(Err(A2aError::InvalidResponse(
                format!("Failed to parse SSE event: {} (data: {})", e, data),
            ))),
        }
    } else {
        // Ignore non-data SSE fields (event:, id:, retry:)
        None
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_default() {
        let config = A2aClientConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(config.auth_token.is_none());
    }

    #[test]
    fn test_client_config_custom() {
        let config = A2aClientConfig {
            timeout: Duration::from_secs(60),
            auth_token: Some("test-token".to_string()),
        };
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.auth_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_client_creation() {
        let client = A2aClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_with_config() {
        let config = A2aClientConfig {
            timeout: Duration::from_secs(10),
            auth_token: Some("my-token".to_string()),
        };
        let client = A2aClient::with_config(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_with_reqwest_client() {
        let reqwest_client = reqwest::Client::new();
        let config = A2aClientConfig::default();
        let _client = A2aClient::with_reqwest_client(reqwest_client, config);
    }

    #[tokio::test]
    async fn test_send_task_connection_failure() {
        // Attempt to connect to a non-routable address to guarantee a connection error.
        // Using 192.0.2.1 (TEST-NET-1, RFC 5737) which is guaranteed non-routable.
        let client = A2aClient::with_config(A2aClientConfig {
            timeout: Duration::from_secs(2),
            auth_token: None,
        })
        .unwrap();

        let request = A2aTaskRequest::send_task("task-1", "test", 1);
        let result = client
            .send_task("http://192.0.2.1:1/nonexistent", request)
            .await;

        // Must fail with some A2aError variant (Network, Timeout, or HttpError depending on env)
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_discover_network_error() {
        let client = A2aClient::with_config(A2aClientConfig {
            timeout: Duration::from_secs(1),
            auth_token: None,
        })
        .unwrap();

        let result = client.discover("http://127.0.0.1:1").await;
        assert!(result.is_err());
    }

    // ---- parse_sse_line unit tests ----

    #[test]
    fn test_parse_sse_line_empty() {
        assert!(parse_sse_line("").is_none());
        assert!(parse_sse_line("   ").is_none());
    }

    #[test]
    fn test_parse_sse_line_comment() {
        assert!(parse_sse_line(": this is a comment").is_none());
    }

    #[test]
    fn test_parse_sse_line_done() {
        assert!(parse_sse_line("data: [DONE]").is_none());
    }

    #[test]
    fn test_parse_sse_line_non_data_field() {
        assert!(parse_sse_line("event: message").is_none());
        assert!(parse_sse_line("id: 123").is_none());
        assert!(parse_sse_line("retry: 5000").is_none());
    }

    #[test]
    fn test_parse_sse_line_valid_text_delta() {
        let result = parse_sse_line(r#"data: {"type":"text_delta","content":"Hello"}"#);
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            A2aStreamEvent::TextDelta { content } => assert_eq!(content, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_parse_sse_line_valid_status_update() {
        let result = parse_sse_line(
            r#"data: {"type":"status_update","task_id":"t-1","status":"in_progress"}"#,
        );
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            A2aStreamEvent::StatusUpdate { task_id, status, .. } => {
                assert_eq!(task_id, "t-1");
                assert_eq!(status, "in_progress");
            }
            _ => panic!("Expected StatusUpdate"),
        }
    }

    #[test]
    fn test_parse_sse_line_valid_task_complete() {
        let result = parse_sse_line(
            r#"data: {"type":"task_complete","task_id":"t-1","result":{"task_id":"t-1","status":"completed","output":"Done!"}}"#,
        );
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            A2aStreamEvent::TaskComplete { task_id, result } => {
                assert_eq!(task_id, "t-1");
                assert_eq!(result.status, "completed");
                assert_eq!(result.output, Some("Done!".to_string()));
            }
            _ => panic!("Expected TaskComplete"),
        }
    }

    #[test]
    fn test_parse_sse_line_valid_task_error() {
        let result = parse_sse_line(
            r#"data: {"type":"task_error","task_id":"t-1","error":"Something failed"}"#,
        );
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            A2aStreamEvent::TaskError { task_id, error } => {
                assert_eq!(task_id, "t-1");
                assert_eq!(error, "Something failed");
            }
            _ => panic!("Expected TaskError"),
        }
    }

    #[test]
    fn test_parse_sse_line_invalid_json() {
        let result = parse_sse_line("data: not-valid-json");
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    // ---- parse_byte_stream_as_sse integration tests ----
    // Note: The Unfold stream from parse_byte_stream_as_sse is !Unpin,
    // so we must Box::pin it before calling .next().await in tests.

    #[tokio::test]
    async fn test_byte_stream_sse_valid_events() {
        let data = b"data: {\"type\":\"text_delta\",\"content\":\"Hello\"}\n\ndata: {\"type\":\"text_delta\",\"content\":\" World\"}\n\ndata: [DONE]\n";
        let chunk_stream = futures_util::stream::iter(vec![
            Ok::<bytes::Bytes, reqwest::Error>(bytes::Bytes::copy_from_slice(data)),
        ]);

        let mut event_stream = Box::pin(parse_byte_stream_as_sse(chunk_stream));
        let mut events = Vec::new();
        while let Some(result) = event_stream.next().await {
            events.push(result.unwrap());
        }

        assert_eq!(events.len(), 2);
        match &events[0] {
            A2aStreamEvent::TextDelta { content } => assert_eq!(content, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
        match &events[1] {
            A2aStreamEvent::TextDelta { content } => assert_eq!(content, " World"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[tokio::test]
    async fn test_byte_stream_sse_split_chunks() {
        // Data split across two chunks
        let chunks = vec![
            Ok::<bytes::Bytes, reqwest::Error>(bytes::Bytes::from_static(b"data: {\"type\":\"text_")),
            Ok(bytes::Bytes::from_static(b"delta\",\"content\":\"Hi\"}\n\n")),
        ];
        let chunk_stream = futures_util::stream::iter(chunks);

        let mut event_stream = Box::pin(parse_byte_stream_as_sse(chunk_stream));
        let mut events = Vec::new();
        while let Some(result) = event_stream.next().await {
            events.push(result.unwrap());
        }

        assert_eq!(events.len(), 1);
        match &events[0] {
            A2aStreamEvent::TextDelta { content } => assert_eq!(content, "Hi"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[tokio::test]
    async fn test_byte_stream_sse_skips_comments_and_empty() {
        let data = b": this is a comment\n\n\ndata: {\"type\":\"text_delta\",\"content\":\"ok\"}\n\n";
        let chunk_stream = futures_util::stream::iter(vec![
            Ok::<bytes::Bytes, reqwest::Error>(bytes::Bytes::copy_from_slice(data)),
        ]);

        let mut event_stream = Box::pin(parse_byte_stream_as_sse(chunk_stream));
        let mut events = Vec::new();
        while let Some(result) = event_stream.next().await {
            events.push(result.unwrap());
        }

        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_byte_stream_sse_invalid_json() {
        let data = b"data: not-valid-json\n\n";
        let chunk_stream = futures_util::stream::iter(vec![
            Ok::<bytes::Bytes, reqwest::Error>(bytes::Bytes::copy_from_slice(data)),
        ]);

        let mut event_stream = Box::pin(parse_byte_stream_as_sse(chunk_stream));
        let result = event_stream.next().await;
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[tokio::test]
    async fn test_byte_stream_sse_ignores_non_data_fields() {
        let data = b"event: message\nid: 123\nretry: 5000\ndata: {\"type\":\"text_delta\",\"content\":\"hello\"}\n\n";
        let chunk_stream = futures_util::stream::iter(vec![
            Ok::<bytes::Bytes, reqwest::Error>(bytes::Bytes::copy_from_slice(data)),
        ]);

        let mut event_stream = Box::pin(parse_byte_stream_as_sse(chunk_stream));
        let mut events = Vec::new();
        while let Some(result) = event_stream.next().await {
            events.push(result.unwrap());
        }

        assert_eq!(events.len(), 1);
        match &events[0] {
            A2aStreamEvent::TextDelta { content } => assert_eq!(content, "hello"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[tokio::test]
    async fn test_byte_stream_sse_multiple_events_in_single_chunk() {
        let data = b"data: {\"type\":\"status_update\",\"task_id\":\"t-1\",\"status\":\"in_progress\"}\n\ndata: {\"type\":\"text_delta\",\"content\":\"working...\"}\n\ndata: {\"type\":\"task_complete\",\"task_id\":\"t-1\",\"result\":{\"task_id\":\"t-1\",\"status\":\"completed\",\"output\":\"done\"}}\n\n";
        let chunk_stream = futures_util::stream::iter(vec![
            Ok::<bytes::Bytes, reqwest::Error>(bytes::Bytes::copy_from_slice(data)),
        ]);

        let mut event_stream = Box::pin(parse_byte_stream_as_sse(chunk_stream));
        let mut events = Vec::new();
        while let Some(result) = event_stream.next().await {
            events.push(result.unwrap());
        }

        assert_eq!(events.len(), 3);
        assert!(matches!(&events[0], A2aStreamEvent::StatusUpdate { .. }));
        assert!(matches!(&events[1], A2aStreamEvent::TextDelta { .. }));
        assert!(matches!(&events[2], A2aStreamEvent::TaskComplete { .. }));
    }
}
