//! RemoteA2aAgent — Agent trait wrapper for remote A2A agents
//!
//! Provides a `RemoteA2aAgent` struct that implements the `Agent` trait
//! (from `agent_composer/types.rs`), wrapping a remote agent discovered
//! via the A2A protocol. When `run()` is called, it:
//!
//! 1. Serializes the `AgentInput` into a JSON-RPC task request
//! 2. Sends to the remote endpoint via `A2aClient`
//! 3. Converts the remote response stream into an `AgentEventStream`
//!
//! This allows pipelines (Sequential, Parallel, Graph) to transparently
//! mix local and remote agents.
//!
//! # Example
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use crate::services::a2a::remote_agent::RemoteA2aAgent;
//! use crate::services::agent_composer::registry::ComposerRegistry;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let agent = RemoteA2aAgent::from_discovery("https://agent.example.com").await?;
//! let mut registry = ComposerRegistry::new();
//! registry.register(agent.name().to_string(), Arc::new(agent));
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use std::pin::Pin;
use uuid::Uuid;

use super::client::A2aClient;
use super::types::{A2aError, A2aStreamEvent, A2aTaskRequest, AgentCard};
use crate::services::agent_composer::types::{
    Agent, AgentContext, AgentEvent, AgentEventStream, AgentInput,
};
use crate::utils::error::{AppError, AppResult};

/// An agent that delegates execution to a remote A2A-compatible agent.
///
/// Implements the `Agent` trait so it can be used interchangeably with local
/// agents in pipelines (Sequential, Parallel, Conditional, Graph).
pub struct RemoteA2aAgent {
    /// The discovered agent card describing the remote agent.
    agent_card: AgentCard,
    /// The A2A HTTP client for communicating with the remote agent.
    client: A2aClient,
}

impl RemoteA2aAgent {
    /// Creates a new `RemoteA2aAgent` from an already-discovered agent card and client.
    pub fn new(agent_card: AgentCard, client: A2aClient) -> Self {
        Self { agent_card, client }
    }

    /// Creates a `RemoteA2aAgent` by discovering the agent at the given base URL.
    ///
    /// This performs an HTTP GET to `{base_url}/.well-known/agent.json` to
    /// fetch the agent card, then constructs the wrapper.
    ///
    /// # Errors
    ///
    /// Returns `AppError` if discovery fails (network error, invalid card, etc.)
    pub async fn from_discovery(base_url: &str) -> AppResult<Self> {
        let client = A2aClient::new().map_err(|e| AppError::internal(e.to_string()))?;
        let agent_card = client
            .discover(base_url)
            .await
            .map_err(|e| AppError::internal(format!("A2A discovery failed: {}", e)))?;
        Ok(Self { agent_card, client })
    }

    /// Returns a reference to the underlying agent card.
    pub fn agent_card(&self) -> &AgentCard {
        &self.agent_card
    }

    /// Returns the endpoint URL of the remote agent.
    pub fn endpoint(&self) -> &str {
        &self.agent_card.endpoint
    }
}

/// Maps an `A2aStreamEvent` to an `AgentEvent`.
///
/// The mapping follows these rules:
/// - `A2aStreamEvent::TextDelta` -> `AgentEvent::TextDelta`
/// - `A2aStreamEvent::TaskComplete` -> `AgentEvent::Done` (with output from result)
/// - `A2aStreamEvent::TaskError` -> `AgentEvent::Failed`
/// - `A2aStreamEvent::StatusUpdate` -> Ignored (filtered out of the stream)
fn map_a2a_event_to_agent_event(
    event: Result<A2aStreamEvent, A2aError>,
) -> Option<AppResult<AgentEvent>> {
    match event {
        Ok(A2aStreamEvent::TextDelta { content }) => Some(Ok(AgentEvent::TextDelta { content })),
        Ok(A2aStreamEvent::TaskComplete { task_id: _, result }) => Some(Ok(AgentEvent::Done {
            output: result.output,
        })),
        Ok(A2aStreamEvent::TaskError { task_id, error }) => Some(Ok(AgentEvent::Failed {
            run_id: task_id,
            error,
            duration_ms: 0,
        })),
        Ok(A2aStreamEvent::StatusUpdate { .. }) => {
            // Status updates are informational; skip them in the agent event stream
            None
        }
        Err(e) => Some(Err(AppError::internal(format!("A2A stream error: {}", e)))),
    }
}

/// Builds the input text from an `AgentInput` for sending to a remote agent.
fn build_input_text(input: &AgentInput) -> String {
    input.as_text()
}

#[async_trait]
impl Agent for RemoteA2aAgent {
    fn name(&self) -> &str {
        &self.agent_card.name
    }

    fn description(&self) -> &str {
        &self.agent_card.description
    }

    async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream> {
        let task_id = Uuid::new_v4().to_string();
        let input_text = build_input_text(&ctx.input);

        // Build a streaming JSON-RPC task request
        let request = A2aTaskRequest::send_task_streaming(
            task_id.as_str(),
            input_text.as_str(),
            task_id.clone(),
        );

        // Send the request and get the SSE stream
        let a2a_stream = self
            .client
            .send_task_streaming(&self.agent_card.endpoint, request)
            .await
            .map_err(|e| AppError::internal(format!("A2A request failed: {}", e)))?;

        // Map A2aStreamEvents to AgentEvents, filtering out StatusUpdate events
        let agent_stream = a2a_stream
            .filter_map(|event| futures_util::future::ready(map_a2a_event_to_agent_event(event)));

        Ok(Box::pin(agent_stream))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::a2a::types::{A2aStreamEvent, A2aTaskResult};

    // ========================================================================
    // Helper: create a test AgentCard
    // ========================================================================

    fn test_agent_card() -> AgentCard {
        AgentCard {
            name: "remote-reviewer".to_string(),
            description: "A remote code review agent".to_string(),
            capabilities: vec!["code_review".to_string(), "testing".to_string()],
            endpoint: "https://agent.example.com/tasks".to_string(),
            version: "1.0.0".to_string(),
            auth_required: false,
            supported_inputs: vec!["text/plain".to_string()],
        }
    }

    fn test_client() -> A2aClient {
        A2aClient::new().unwrap()
    }

    // ========================================================================
    // Agent trait method tests
    // ========================================================================

    #[test]
    fn test_name_returns_agent_card_name() {
        let agent = RemoteA2aAgent::new(test_agent_card(), test_client());
        assert_eq!(agent.name(), "remote-reviewer");
    }

    #[test]
    fn test_description_returns_agent_card_description() {
        let agent = RemoteA2aAgent::new(test_agent_card(), test_client());
        assert_eq!(agent.description(), "A remote code review agent");
    }

    #[test]
    fn test_agent_card_accessor() {
        let card = test_agent_card();
        let agent = RemoteA2aAgent::new(card.clone(), test_client());
        assert_eq!(agent.agent_card().name, "remote-reviewer");
        assert_eq!(agent.agent_card().version, "1.0.0");
    }

    #[test]
    fn test_endpoint_accessor() {
        let agent = RemoteA2aAgent::new(test_agent_card(), test_client());
        assert_eq!(agent.endpoint(), "https://agent.example.com/tasks");
    }

    // ========================================================================
    // A2aStreamEvent -> AgentEvent mapping tests
    // ========================================================================

    #[test]
    fn test_map_text_delta_to_agent_event() {
        let event = Ok(A2aStreamEvent::TextDelta {
            content: "Hello from remote".to_string(),
        });
        let result = map_a2a_event_to_agent_event(event);
        assert!(result.is_some());
        let agent_event = result.unwrap().unwrap();
        match agent_event {
            AgentEvent::TextDelta { content } => {
                assert_eq!(content, "Hello from remote");
            }
            _ => panic!("Expected AgentEvent::TextDelta"),
        }
    }

    #[test]
    fn test_map_task_complete_to_done_event() {
        let event = Ok(A2aStreamEvent::TaskComplete {
            task_id: "task-1".to_string(),
            result: A2aTaskResult {
                task_id: "task-1".to_string(),
                status: "completed".to_string(),
                output: Some("Review complete: code looks good".to_string()),
                artifacts: None,
            },
        });
        let result = map_a2a_event_to_agent_event(event);
        assert!(result.is_some());
        let agent_event = result.unwrap().unwrap();
        match agent_event {
            AgentEvent::Done { output } => {
                assert_eq!(output, Some("Review complete: code looks good".to_string()));
            }
            _ => panic!("Expected AgentEvent::Done"),
        }
    }

    #[test]
    fn test_map_task_complete_without_output() {
        let event = Ok(A2aStreamEvent::TaskComplete {
            task_id: "task-2".to_string(),
            result: A2aTaskResult {
                task_id: "task-2".to_string(),
                status: "completed".to_string(),
                output: None,
                artifacts: None,
            },
        });
        let result = map_a2a_event_to_agent_event(event);
        assert!(result.is_some());
        let agent_event = result.unwrap().unwrap();
        match agent_event {
            AgentEvent::Done { output } => {
                assert!(output.is_none());
            }
            _ => panic!("Expected AgentEvent::Done"),
        }
    }

    #[test]
    fn test_map_task_error_to_failed_event() {
        let event = Ok(A2aStreamEvent::TaskError {
            task_id: "task-err".to_string(),
            error: "Remote agent crashed".to_string(),
        });
        let result = map_a2a_event_to_agent_event(event);
        assert!(result.is_some());
        let agent_event = result.unwrap().unwrap();
        match agent_event {
            AgentEvent::Failed {
                run_id,
                error,
                duration_ms,
            } => {
                assert_eq!(run_id, "task-err");
                assert_eq!(error, "Remote agent crashed");
                assert_eq!(duration_ms, 0);
            }
            _ => panic!("Expected AgentEvent::Failed"),
        }
    }

    #[test]
    fn test_map_status_update_returns_none() {
        let event = Ok(A2aStreamEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            status: "in_progress".to_string(),
            message: Some("Processing...".to_string()),
        });
        let result = map_a2a_event_to_agent_event(event);
        assert!(result.is_none(), "StatusUpdate should be filtered out");
    }

    #[test]
    fn test_map_status_update_without_message_returns_none() {
        let event = Ok(A2aStreamEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            status: "queued".to_string(),
            message: None,
        });
        let result = map_a2a_event_to_agent_event(event);
        assert!(result.is_none());
    }

    #[test]
    fn test_map_a2a_error_to_app_error() {
        let event = Err(A2aError::Network("connection refused".to_string()));
        let result = map_a2a_event_to_agent_event(event);
        assert!(result.is_some());
        let app_result = result.unwrap();
        assert!(app_result.is_err());
        let err = app_result.unwrap_err();
        assert!(err.to_string().contains("A2A stream error"));
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn test_map_a2a_timeout_error() {
        let event = Err(A2aError::Timeout(30));
        let result = map_a2a_event_to_agent_event(event);
        assert!(result.is_some());
        let app_result = result.unwrap();
        assert!(app_result.is_err());
        assert!(app_result.unwrap_err().to_string().contains("timed out"));
    }

    #[test]
    fn test_map_a2a_stream_error() {
        let event = Err(A2aError::StreamError("unexpected EOF".to_string()));
        let result = map_a2a_event_to_agent_event(event);
        assert!(result.is_some());
        let err = result.unwrap().unwrap_err();
        assert!(err.to_string().contains("unexpected EOF"));
    }

    // ========================================================================
    // build_input_text tests
    // ========================================================================

    #[test]
    fn test_build_input_text_from_text_input() {
        let input = AgentInput::Text("Review this code for bugs".to_string());
        let text = build_input_text(&input);
        assert_eq!(text, "Review this code for bugs");
    }

    #[test]
    fn test_build_input_text_from_structured_input() {
        let input = AgentInput::Structured(serde_json::json!({
            "task": "review",
            "file": "main.rs"
        }));
        let text = build_input_text(&input);
        assert!(text.contains("review"));
        assert!(text.contains("main.rs"));
    }

    #[test]
    fn test_build_input_text_from_empty_text() {
        let input = AgentInput::Text(String::new());
        let text = build_input_text(&input);
        assert_eq!(text, "");
    }

    // ========================================================================
    // Integration test: stream mapping with mock SSE data
    // ========================================================================

    #[tokio::test]
    async fn test_agent_event_stream_from_a2a_events() {
        // Simulate a sequence of A2A stream events and verify the mapped output
        let a2a_events: Vec<Result<A2aStreamEvent, A2aError>> = vec![
            Ok(A2aStreamEvent::StatusUpdate {
                task_id: "t-1".to_string(),
                status: "in_progress".to_string(),
                message: None,
            }),
            Ok(A2aStreamEvent::TextDelta {
                content: "Reviewing ".to_string(),
            }),
            Ok(A2aStreamEvent::TextDelta {
                content: "your code...".to_string(),
            }),
            Ok(A2aStreamEvent::TaskComplete {
                task_id: "t-1".to_string(),
                result: A2aTaskResult {
                    task_id: "t-1".to_string(),
                    status: "completed".to_string(),
                    output: Some("All good!".to_string()),
                    artifacts: None,
                },
            }),
        ];

        let a2a_stream = futures_util::stream::iter(a2a_events);
        let mut agent_stream =
            Box::pin(a2a_stream.filter_map(|event| {
                futures_util::future::ready(map_a2a_event_to_agent_event(event))
            }));

        // First event: TextDelta "Reviewing " (StatusUpdate was filtered)
        let ev1 = agent_stream.next().await.unwrap().unwrap();
        match ev1 {
            AgentEvent::TextDelta { content } => assert_eq!(content, "Reviewing "),
            _ => panic!("Expected TextDelta, got {:?}", ev1),
        }

        // Second event: TextDelta "your code..."
        let ev2 = agent_stream.next().await.unwrap().unwrap();
        match ev2 {
            AgentEvent::TextDelta { content } => assert_eq!(content, "your code..."),
            _ => panic!("Expected TextDelta, got {:?}", ev2),
        }

        // Third event: Done with output
        let ev3 = agent_stream.next().await.unwrap().unwrap();
        match ev3 {
            AgentEvent::Done { output } => {
                assert_eq!(output, Some("All good!".to_string()));
            }
            _ => panic!("Expected Done, got {:?}", ev3),
        }

        // Stream should be exhausted
        assert!(agent_stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_agent_event_stream_with_error_mid_stream() {
        // Simulate a stream that produces events then errors
        let a2a_events: Vec<Result<A2aStreamEvent, A2aError>> = vec![
            Ok(A2aStreamEvent::TextDelta {
                content: "Starting...".to_string(),
            }),
            Err(A2aError::Network("connection reset".to_string())),
        ];

        let a2a_stream = futures_util::stream::iter(a2a_events);
        let mut agent_stream =
            Box::pin(a2a_stream.filter_map(|event| {
                futures_util::future::ready(map_a2a_event_to_agent_event(event))
            }));

        // First: TextDelta
        let ev1 = agent_stream.next().await.unwrap();
        assert!(ev1.is_ok());

        // Second: Error
        let ev2 = agent_stream.next().await.unwrap();
        assert!(ev2.is_err());
        assert!(ev2.unwrap_err().to_string().contains("connection reset"));

        // Stream ends
        assert!(agent_stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_agent_event_stream_with_task_error_event() {
        // Simulate a stream where the remote agent reports a task error
        let a2a_events: Vec<Result<A2aStreamEvent, A2aError>> = vec![
            Ok(A2aStreamEvent::TextDelta {
                content: "Trying...".to_string(),
            }),
            Ok(A2aStreamEvent::TaskError {
                task_id: "t-fail".to_string(),
                error: "Out of memory".to_string(),
            }),
        ];

        let a2a_stream = futures_util::stream::iter(a2a_events);
        let mut agent_stream =
            Box::pin(a2a_stream.filter_map(|event| {
                futures_util::future::ready(map_a2a_event_to_agent_event(event))
            }));

        // First: TextDelta
        let ev1 = agent_stream.next().await.unwrap().unwrap();
        assert!(matches!(ev1, AgentEvent::TextDelta { .. }));

        // Second: Failed
        let ev2 = agent_stream.next().await.unwrap().unwrap();
        match ev2 {
            AgentEvent::Failed { run_id, error, .. } => {
                assert_eq!(run_id, "t-fail");
                assert_eq!(error, "Out of memory");
            }
            _ => panic!("Expected Failed, got {:?}", ev2),
        }
    }

    // ========================================================================
    // ComposerRegistry integration test
    // ========================================================================

    #[test]
    fn test_remote_agent_registrable_in_composer_registry() {
        use crate::services::agent_composer::registry::ComposerRegistry;
        use std::sync::Arc;

        let agent = RemoteA2aAgent::new(test_agent_card(), test_client());
        let mut registry = ComposerRegistry::new();

        registry.register("remote-reviewer", Arc::new(agent));

        assert!(registry.contains("remote-reviewer"));
        let fetched = registry.get("remote-reviewer").unwrap();
        assert_eq!(fetched.name(), "remote-reviewer");
        assert_eq!(fetched.description(), "A remote code review agent");
    }

    #[test]
    fn test_remote_agent_listed_alongside_local_agents() {
        use crate::services::agent_composer::registry::ComposerRegistry;
        use std::sync::Arc;

        let mut registry = ComposerRegistry::new();

        // Register a remote agent
        let remote = RemoteA2aAgent::new(test_agent_card(), test_client());
        registry.register("remote-reviewer", Arc::new(remote));

        // Register a "local" agent (we reuse RemoteA2aAgent with different card for simplicity)
        let local_card = AgentCard {
            name: "local-coder".to_string(),
            description: "A local coding agent".to_string(),
            capabilities: vec!["coding".to_string()],
            endpoint: "http://localhost:8080/tasks".to_string(),
            version: "0.1.0".to_string(),
            auth_required: false,
            supported_inputs: vec![],
        };
        let local = RemoteA2aAgent::new(local_card, test_client());
        registry.register("local-coder", Arc::new(local));

        let list = registry.list();
        assert_eq!(list.len(), 2);

        let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"remote-reviewer"));
        assert!(names.contains(&"local-coder"));
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_remote_agent_with_custom_name_and_description() {
        let card = AgentCard {
            name: "specialized-tester".to_string(),
            description: "Runs integration tests on Kubernetes clusters".to_string(),
            capabilities: vec!["testing".to_string(), "kubernetes".to_string()],
            endpoint: "https://k8s-tester.internal/api/v1/tasks".to_string(),
            version: "2.3.1".to_string(),
            auth_required: true,
            supported_inputs: vec!["text/plain".to_string(), "application/json".to_string()],
        };
        let agent = RemoteA2aAgent::new(card, test_client());

        assert_eq!(agent.name(), "specialized-tester");
        assert_eq!(
            agent.description(),
            "Runs integration tests on Kubernetes clusters"
        );
        assert_eq!(agent.endpoint(), "https://k8s-tester.internal/api/v1/tasks");
        assert!(agent.agent_card().auth_required);
    }

    #[tokio::test]
    async fn test_agent_event_stream_only_status_updates_produces_empty_stream() {
        // A stream with only StatusUpdate events should produce an empty agent stream
        let a2a_events: Vec<Result<A2aStreamEvent, A2aError>> = vec![
            Ok(A2aStreamEvent::StatusUpdate {
                task_id: "t-1".to_string(),
                status: "queued".to_string(),
                message: None,
            }),
            Ok(A2aStreamEvent::StatusUpdate {
                task_id: "t-1".to_string(),
                status: "in_progress".to_string(),
                message: Some("Working...".to_string()),
            }),
        ];

        let a2a_stream = futures_util::stream::iter(a2a_events);
        let mut agent_stream =
            Box::pin(a2a_stream.filter_map(|event| {
                futures_util::future::ready(map_a2a_event_to_agent_event(event))
            }));

        // Should produce no events
        assert!(agent_stream.next().await.is_none());
    }
}
