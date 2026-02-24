//! A2A Protocol Types
//!
//! Defines the core data structures for the Agent-to-Agent (A2A) protocol:
//! - `AgentCard`: Agent discovery metadata (served at `/.well-known/agent.json`)
//! - `A2aTaskRequest` / `A2aTaskResponse`: JSON-RPC 2.0 task messaging
//! - `A2aError`: Protocol-level error types

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

// ============================================================================
// Agent Card (Discovery)
// ============================================================================

/// Metadata describing a remote A2A agent.
///
/// Served at `{base_url}/.well-known/agent.json` and used by clients to
/// discover agent capabilities before sending task requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Human-readable agent name.
    pub name: String,
    /// Brief description of what the agent does.
    pub description: String,
    /// List of capabilities the agent supports (e.g., "code_review", "testing").
    pub capabilities: Vec<String>,
    /// The HTTP(S) endpoint URL for sending task requests.
    pub endpoint: String,
    /// Protocol/agent version string.
    pub version: String,
    /// Whether the agent requires authentication.
    #[serde(default)]
    pub auth_required: bool,
    /// MIME types or input formats the agent accepts.
    #[serde(default)]
    pub supported_inputs: Vec<String>,
}

impl AgentCard {
    /// Validates that all required fields are non-empty.
    pub fn validate(&self) -> Result<(), A2aError> {
        if self.name.trim().is_empty() {
            return Err(A2aError::InvalidAgentCard("name is empty".to_string()));
        }
        if self.endpoint.trim().is_empty() {
            return Err(A2aError::InvalidAgentCard("endpoint is empty".to_string()));
        }
        if self.version.trim().is_empty() {
            return Err(A2aError::InvalidAgentCard("version is empty".to_string()));
        }
        Ok(())
    }
}

// ============================================================================
// JSON-RPC 2.0 Task Request
// ============================================================================

/// Parameters for an A2A task request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskParams {
    /// Unique task identifier (client-generated).
    pub task_id: String,
    /// The task instruction or prompt.
    pub input: String,
    /// Optional structured metadata for the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    /// Whether the client wants a streaming response.
    #[serde(default)]
    pub stream: bool,
}

/// A JSON-RPC 2.0 request for A2A task execution.
///
/// Conforms to the JSON-RPC 2.0 specification with `method` indicating the
/// operation and `params` carrying the task payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskRequest {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// The RPC method name (e.g., "tasks/send", "tasks/sendSubscribe").
    pub method: String,
    /// Task parameters.
    pub params: A2aTaskParams,
    /// Request identifier for correlating responses.
    pub id: Value,
}

impl A2aTaskRequest {
    /// Creates a new task request with the given method and params.
    ///
    /// Automatically sets `jsonrpc` to "2.0".
    pub fn new(method: impl Into<String>, params: A2aTaskParams, id: impl Into<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
            id: id.into(),
        }
    }

    /// Creates a `tasks/send` request (non-streaming).
    pub fn send_task(
        task_id: impl Into<String>,
        input: impl Into<String>,
        id: impl Into<Value>,
    ) -> Self {
        Self::new(
            "tasks/send",
            A2aTaskParams {
                task_id: task_id.into(),
                input: input.into(),
                metadata: None,
                stream: false,
            },
            id,
        )
    }

    /// Creates a `tasks/sendSubscribe` request (streaming via SSE).
    pub fn send_task_streaming(
        task_id: impl Into<String>,
        input: impl Into<String>,
        id: impl Into<Value>,
    ) -> Self {
        Self::new(
            "tasks/sendSubscribe",
            A2aTaskParams {
                task_id: task_id.into(),
                input: input.into(),
                metadata: None,
                stream: true,
            },
            id,
        )
    }
}

// ============================================================================
// JSON-RPC 2.0 Task Response
// ============================================================================

/// Result payload of a completed A2A task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskResult {
    /// The task ID echoed from the request.
    pub task_id: String,
    /// Current task status: "completed", "failed", "in_progress", etc.
    pub status: String,
    /// The output/result of the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Optional structured artifacts produced by the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<Vec<Value>>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code (standard JSON-RPC or application-specific).
    pub code: i64,
    /// Human-readable error message.
    pub message: String,
    /// Optional additional error data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// A JSON-RPC 2.0 response for A2A task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskResponse {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// The result if the call succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<A2aTaskResult>,
    /// The error if the call failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// Correlation ID matching the request.
    pub id: Value,
}

impl A2aTaskResponse {
    /// Returns true if the response indicates success.
    pub fn is_success(&self) -> bool {
        self.result.is_some() && self.error.is_none()
    }

    /// Extracts the result or returns an error.
    pub fn into_result(self) -> Result<A2aTaskResult, A2aError> {
        if let Some(err) = self.error {
            return Err(A2aError::JsonRpcError {
                code: err.code,
                message: err.message,
            });
        }
        self.result.ok_or_else(|| {
            A2aError::InvalidResponse("response contains neither result nor error".to_string())
        })
    }
}

// ============================================================================
// Streaming Event
// ============================================================================

/// A single event from an A2A streaming (SSE) response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum A2aStreamEvent {
    /// Incremental text output from the agent.
    TextDelta { content: String },
    /// Task status update.
    StatusUpdate {
        task_id: String,
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// Task completed with final result.
    TaskComplete {
        task_id: String,
        result: A2aTaskResult,
    },
    /// Task failed with error.
    TaskError { task_id: String, error: String },
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during A2A protocol operations.
#[derive(Error, Debug)]
pub enum A2aError {
    /// HTTP request failed.
    #[error("Network error: {0}")]
    Network(String),

    /// Request timed out.
    #[error("Request timed out after {0}s")]
    Timeout(u64),

    /// Failed to parse response body.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Agent card validation failed.
    #[error("Invalid agent card: {0}")]
    InvalidAgentCard(String),

    /// JSON-RPC error returned by the remote agent.
    #[error("JSON-RPC error ({code}): {message}")]
    JsonRpcError { code: i64, message: String },

    /// HTTP error status code.
    #[error("HTTP error {status}: {body}")]
    HttpError { status: u16, body: String },

    /// SSE stream ended unexpectedly.
    #[error("Stream ended unexpectedly: {0}")]
    StreamError(String),
}

impl From<reqwest::Error> for A2aError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            // Extract timeout duration from the error message if possible
            A2aError::Timeout(30)
        } else if err.is_connect() {
            A2aError::Network(format!("Connection failed: {}", err))
        } else {
            A2aError::Network(err.to_string())
        }
    }
}

impl From<serde_json::Error> for A2aError {
    fn from(err: serde_json::Error) -> Self {
        A2aError::InvalidResponse(format!("JSON parse error: {}", err))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- AgentCard tests ----

    #[test]
    fn test_agent_card_serialization() {
        let card = AgentCard {
            name: "test-agent".to_string(),
            description: "A test agent".to_string(),
            capabilities: vec!["code_review".to_string(), "testing".to_string()],
            endpoint: "https://agent.example.com/tasks".to_string(),
            version: "1.0.0".to_string(),
            auth_required: false,
            supported_inputs: vec!["text/plain".to_string()],
        };

        let json = serde_json::to_string(&card).unwrap();
        assert!(json.contains("\"name\":\"test-agent\""));
        assert!(json.contains("\"endpoint\":\"https://agent.example.com/tasks\""));
        assert!(json.contains("\"version\":\"1.0.0\""));
        assert!(json.contains("\"code_review\""));

        let parsed: AgentCard = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test-agent");
        assert_eq!(parsed.capabilities.len(), 2);
        assert!(!parsed.auth_required);
    }

    #[test]
    fn test_agent_card_deserialization_defaults() {
        // auth_required and supported_inputs should default when missing
        let json = r#"{
            "name": "minimal-agent",
            "description": "Minimal",
            "capabilities": [],
            "endpoint": "https://example.com/tasks",
            "version": "0.1.0"
        }"#;

        let card: AgentCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.name, "minimal-agent");
        assert!(!card.auth_required);
        assert!(card.supported_inputs.is_empty());
    }

    #[test]
    fn test_agent_card_validation_ok() {
        let card = AgentCard {
            name: "agent".to_string(),
            description: "desc".to_string(),
            capabilities: vec![],
            endpoint: "https://example.com".to_string(),
            version: "1.0".to_string(),
            auth_required: false,
            supported_inputs: vec![],
        };
        assert!(card.validate().is_ok());
    }

    #[test]
    fn test_agent_card_validation_empty_name() {
        let card = AgentCard {
            name: "".to_string(),
            description: "desc".to_string(),
            capabilities: vec![],
            endpoint: "https://example.com".to_string(),
            version: "1.0".to_string(),
            auth_required: false,
            supported_inputs: vec![],
        };
        let err = card.validate().unwrap_err();
        assert!(matches!(err, A2aError::InvalidAgentCard(_)));
    }

    #[test]
    fn test_agent_card_validation_empty_endpoint() {
        let card = AgentCard {
            name: "agent".to_string(),
            description: "".to_string(),
            capabilities: vec![],
            endpoint: "  ".to_string(),
            version: "1.0".to_string(),
            auth_required: false,
            supported_inputs: vec![],
        };
        let err = card.validate().unwrap_err();
        assert!(matches!(err, A2aError::InvalidAgentCard(_)));
    }

    #[test]
    fn test_agent_card_validation_empty_version() {
        let card = AgentCard {
            name: "agent".to_string(),
            description: "".to_string(),
            capabilities: vec![],
            endpoint: "https://example.com".to_string(),
            version: "".to_string(),
            auth_required: false,
            supported_inputs: vec![],
        };
        let err = card.validate().unwrap_err();
        assert!(matches!(err, A2aError::InvalidAgentCard(_)));
    }

    // ---- A2aTaskRequest tests ----

    #[test]
    fn test_task_request_json_rpc_format() {
        let req = A2aTaskRequest::send_task("task-1", "Review this code", 1);

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "tasks/send");
        assert_eq!(json["id"], 1);
        assert_eq!(json["params"]["task_id"], "task-1");
        assert_eq!(json["params"]["input"], "Review this code");
        assert_eq!(json["params"]["stream"], false);
    }

    #[test]
    fn test_task_request_streaming_format() {
        let req = A2aTaskRequest::send_task_streaming("task-2", "Generate tests", "req-abc");

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "tasks/sendSubscribe");
        assert_eq!(json["id"], "req-abc");
        assert_eq!(json["params"]["stream"], true);
    }

    #[test]
    fn test_task_request_roundtrip() {
        let req = A2aTaskRequest::new(
            "tasks/send",
            A2aTaskParams {
                task_id: "t-1".to_string(),
                input: "Hello".to_string(),
                metadata: Some(serde_json::json!({"priority": "high"})),
                stream: false,
            },
            42,
        );

        let json = serde_json::to_string(&req).unwrap();
        let parsed: A2aTaskRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.jsonrpc, "2.0");
        assert_eq!(parsed.method, "tasks/send");
        assert_eq!(parsed.params.task_id, "t-1");
        assert_eq!(parsed.params.metadata.unwrap()["priority"], "high");
    }

    // ---- A2aTaskResponse tests ----

    #[test]
    fn test_task_response_success() {
        let json = r#"{
            "jsonrpc": "2.0",
            "result": {
                "task_id": "task-1",
                "status": "completed",
                "output": "Code looks good, no issues found."
            },
            "id": 1
        }"#;

        let resp: A2aTaskResponse = serde_json::from_str(json).unwrap();
        assert!(resp.is_success());
        let result = resp.into_result().unwrap();
        assert_eq!(result.task_id, "task-1");
        assert_eq!(result.status, "completed");
        assert_eq!(result.output.unwrap(), "Code looks good, no issues found.");
    }

    #[test]
    fn test_task_response_error() {
        let json = r#"{
            "jsonrpc": "2.0",
            "error": {
                "code": -32600,
                "message": "Invalid request"
            },
            "id": 1
        }"#;

        let resp: A2aTaskResponse = serde_json::from_str(json).unwrap();
        assert!(!resp.is_success());
        let err = resp.into_result().unwrap_err();
        match err {
            A2aError::JsonRpcError { code, message } => {
                assert_eq!(code, -32600);
                assert_eq!(message, "Invalid request");
            }
            _ => panic!("Expected JsonRpcError"),
        }
    }

    #[test]
    fn test_task_response_neither_result_nor_error() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1
        }"#;

        let resp: A2aTaskResponse = serde_json::from_str(json).unwrap();
        let err = resp.into_result().unwrap_err();
        assert!(matches!(err, A2aError::InvalidResponse(_)));
    }

    // ---- A2aStreamEvent tests ----

    #[test]
    fn test_stream_event_text_delta() {
        let event = A2aStreamEvent::TextDelta {
            content: "Hello world".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"text_delta\""));

        let parsed: A2aStreamEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            A2aStreamEvent::TextDelta { content } => assert_eq!(content, "Hello world"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_stream_event_status_update() {
        let event = A2aStreamEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            status: "in_progress".to_string(),
            message: Some("Processing...".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"status_update\""));

        let parsed: A2aStreamEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            A2aStreamEvent::StatusUpdate {
                task_id,
                status,
                message,
            } => {
                assert_eq!(task_id, "task-1");
                assert_eq!(status, "in_progress");
                assert_eq!(message, Some("Processing...".to_string()));
            }
            _ => panic!("Expected StatusUpdate"),
        }
    }

    #[test]
    fn test_stream_event_task_complete() {
        let event = A2aStreamEvent::TaskComplete {
            task_id: "task-1".to_string(),
            result: A2aTaskResult {
                task_id: "task-1".to_string(),
                status: "completed".to_string(),
                output: Some("Done!".to_string()),
                artifacts: None,
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"task_complete\""));

        let parsed: A2aStreamEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            A2aStreamEvent::TaskComplete { task_id, result } => {
                assert_eq!(task_id, "task-1");
                assert_eq!(result.output, Some("Done!".to_string()));
            }
            _ => panic!("Expected TaskComplete"),
        }
    }

    #[test]
    fn test_stream_event_task_error() {
        let event = A2aStreamEvent::TaskError {
            task_id: "task-1".to_string(),
            error: "Something went wrong".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"task_error\""));

        let parsed: A2aStreamEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            A2aStreamEvent::TaskError { task_id, error } => {
                assert_eq!(task_id, "task-1");
                assert_eq!(error, "Something went wrong");
            }
            _ => panic!("Expected TaskError"),
        }
    }

    // ---- Error tests ----

    #[test]
    fn test_a2a_error_display() {
        let err = A2aError::Network("connection refused".to_string());
        assert_eq!(err.to_string(), "Network error: connection refused");

        let err = A2aError::Timeout(30);
        assert_eq!(err.to_string(), "Request timed out after 30s");

        let err = A2aError::JsonRpcError {
            code: -32600,
            message: "Invalid request".to_string(),
        };
        assert_eq!(err.to_string(), "JSON-RPC error (-32600): Invalid request");

        let err = A2aError::HttpError {
            status: 404,
            body: "Not Found".to_string(),
        };
        assert_eq!(err.to_string(), "HTTP error 404: Not Found");
    }

    #[test]
    fn test_a2a_error_from_serde_json() {
        let bad_json = "not valid json";
        let json_err = serde_json::from_str::<AgentCard>(bad_json).unwrap_err();
        let a2a_err: A2aError = json_err.into();
        assert!(matches!(a2a_err, A2aError::InvalidResponse(_)));
    }
}
