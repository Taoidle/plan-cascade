//! ParallelAgent — runs sub-agents concurrently, merging event streams
//!
//! All sub-agents are spawned via `tokio::spawn` and their event streams
//! are merged using `futures_util::stream::select_all`. Each sub-agent's
//! events are tagged with the agent name for disambiguation.
//!
//! The ParallelAgent completes when ALL sub-agents have emitted a `Done` event.

use std::sync::Arc;

use async_trait::async_trait;
use futures_util::stream::SelectAll;
use futures_util::StreamExt;

use super::types::{Agent, AgentContext, AgentEvent, AgentEventStream};
use crate::utils::error::{AppError, AppResult};

/// A composite agent that runs sub-agents concurrently.
///
/// All sub-agents receive the same input and run in parallel via `tokio::spawn`.
/// Their event streams are merged. `StateUpdate` events are prefixed with
/// the agent name to avoid key collisions.
///
/// The merged stream ends when all sub-agents have emitted their `Done` events,
/// at which point a final `Done` event is emitted with combined outputs.
pub struct ParallelAgent {
    /// Display name for this composite agent.
    name: String,
    /// Description of this composite agent.
    description: String,
    /// Sub-agents to run concurrently.
    agents: Vec<Arc<dyn Agent>>,
}

impl ParallelAgent {
    /// Create a new ParallelAgent with the given name and sub-agents.
    pub fn new(name: impl Into<String>, agents: Vec<Arc<dyn Agent>>) -> Self {
        Self {
            name: name.into(),
            description: "Runs sub-agents concurrently, merging event streams".to_string(),
            agents,
        }
    }

    /// Set a custom description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
}

#[async_trait]
impl Agent for ParallelAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream> {
        if self.agents.is_empty() {
            return Err(AppError::validation("ParallelAgent has no sub-agents"));
        }

        let total_agents = self.agents.len();

        // Start all sub-agents and collect their event streams
        let mut tagged_streams: Vec<AgentEventStream> = Vec::with_capacity(total_agents);

        for agent in &self.agents {
            let sub_ctx = ctx.clone();
            let agent_name = agent.name().to_string();

            match agent.run(sub_ctx).await {
                Ok(stream) => {
                    // Tag each event with the agent name
                    let tagged = stream.map(move |result| {
                        result.map(|event| tag_event(event, &agent_name))
                    });
                    tagged_streams.push(Box::pin(tagged));
                }
                Err(e) => {
                    return Err(AppError::internal(format!(
                        "Failed to start sub-agent '{}': {}",
                        agent.name(),
                        e
                    )));
                }
            }
        }

        // Merge all streams using select_all
        let mut merged = SelectAll::new();
        for stream in tagged_streams {
            merged.push(stream);
        }

        // Track Done events and produce a final Done when all complete
        let stream = futures_util::stream::unfold(
            ParallelState {
                merged,
                done_count: 0,
                total: total_agents,
                outputs: Vec::new(),
            },
            |mut state| async move {
                loop {
                    match state.merged.next().await {
                        Some(Ok(event)) => {
                            match &event {
                                AgentEvent::Done { output } => {
                                    if let Some(out) = output {
                                        state.outputs.push(out.clone());
                                    }
                                    state.done_count += 1;

                                    if state.done_count >= state.total {
                                        // All agents done — emit final Done
                                        let combined = if state.outputs.is_empty() {
                                            None
                                        } else {
                                            Some(state.outputs.join("\n\n---\n\n"))
                                        };
                                        return Some((
                                            Ok(AgentEvent::Done { output: combined }),
                                            state,
                                        ));
                                    }
                                    // Don't emit intermediate Done events
                                    continue;
                                }
                                _ => {
                                    return Some((Ok(event), state));
                                }
                            }
                        }
                        Some(Err(e)) => {
                            return Some((Err(e), state));
                        }
                        None => {
                            // All streams ended
                            if state.done_count < state.total {
                                // Some agents ended without Done
                                let combined = if state.outputs.is_empty() {
                                    None
                                } else {
                                    Some(state.outputs.join("\n\n---\n\n"))
                                };
                                return Some((
                                    Ok(AgentEvent::Done { output: combined }),
                                    state,
                                ));
                            }
                            return None;
                        }
                    }
                }
            },
        );

        Ok(Box::pin(stream))
    }
}

/// Tag an event with the agent name.
///
/// For `StateUpdate` events, the key is prefixed with `"agent_name."`.
/// Other events pass through unchanged.
fn tag_event(event: AgentEvent, agent_name: &str) -> AgentEvent {
    match event {
        AgentEvent::StateUpdate { key, value } => AgentEvent::StateUpdate {
            key: format!("{}.{}", agent_name, key),
            value,
        },
        other => other,
    }
}

/// Internal state for the parallel unfold stream.
struct ParallelState {
    merged: SelectAll<AgentEventStream>,
    done_count: usize,
    total: usize,
    outputs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::types::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// A mock agent that emits events with a small delay to simulate async work.
    struct MockParallelAgent {
        name: String,
        output: String,
        delay_ms: u64,
    }

    impl MockParallelAgent {
        fn new(name: &str, output: &str, delay_ms: u64) -> Self {
            Self {
                name: name.to_string(),
                output: output.to_string(),
                delay_ms,
            }
        }
    }

    #[async_trait]
    impl Agent for MockParallelAgent {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Mock parallel agent"
        }

        async fn run(&self, _ctx: AgentContext) -> AppResult<AgentEventStream> {
            let output = self.output.clone();
            let delay_ms = self.delay_ms;

            // Use a channel-based approach so we can add delays
            let (tx, rx) = tokio::sync::mpsc::channel::<AppResult<AgentEvent>>(16);

            let output_clone = output.clone();
            tokio::spawn(async move {
                // Simulate some work
                if delay_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
                let _ = tx
                    .send(Ok(AgentEvent::TextDelta {
                        content: output_clone.clone(),
                    }))
                    .await;
                let _ = tx
                    .send(Ok(AgentEvent::Done {
                        output: Some(output_clone),
                    }))
                    .await;
            });

            let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
            Ok(Box::pin(stream))
        }
    }

    /// Minimal mock LLM provider for testing.
    struct MockProvider {
        config: crate::services::llm::ProviderConfig,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                config: crate::services::llm::ProviderConfig::default(),
            }
        }
    }

    #[async_trait]
    impl crate::services::llm::LlmProvider for MockProvider {
        fn name(&self) -> &'static str {
            "mock"
        }
        fn model(&self) -> &str {
            "mock-model"
        }
        fn supports_thinking(&self) -> bool {
            false
        }
        fn supports_tools(&self) -> bool {
            false
        }
        async fn send_message(
            &self,
            _messages: Vec<crate::services::llm::Message>,
            _system: Option<String>,
            _tools: Vec<crate::services::llm::ToolDefinition>,
            _options: crate::services::llm::LlmRequestOptions,
        ) -> crate::services::llm::LlmResult<crate::services::llm::LlmResponse> {
            unimplemented!()
        }
        async fn stream_message(
            &self,
            _messages: Vec<crate::services::llm::Message>,
            _system: Option<String>,
            _tools: Vec<crate::services::llm::ToolDefinition>,
            _tx: tokio::sync::mpsc::Sender<crate::services::streaming::UnifiedStreamEvent>,
            _options: crate::services::llm::LlmRequestOptions,
        ) -> crate::services::llm::LlmResult<crate::services::llm::LlmResponse> {
            unimplemented!()
        }
        async fn health_check(&self) -> crate::services::llm::LlmResult<()> {
            Ok(())
        }
        fn config(&self) -> &crate::services::llm::ProviderConfig {
            &self.config
        }
    }

    fn mock_context() -> AgentContext {
        let provider = Arc::new(MockProvider::new());
        let tool_executor = Arc::new(
            crate::services::tools::ToolExecutor::new(&PathBuf::from("/tmp")),
        );
        let hooks = Arc::new(crate::services::orchestrator::hooks::AgenticHooks::new());

        AgentContext {
            session_id: "test-session".to_string(),
            project_root: PathBuf::from("/tmp"),
            provider,
            tool_executor,
            plugin_manager: None,
            hooks,
            input: AgentInput::Text("test input".to_string()),
            shared_state: Arc::new(RwLock::new(HashMap::new())),
            config: AgentConfig::default(),
            orchestrator_ctx: None,
        }
    }

    #[tokio::test]
    async fn test_parallel_agent_runs_concurrently() {
        let agent1 = Arc::new(MockParallelAgent::new("agent-1", "output-A", 10)) as Arc<dyn Agent>;
        let agent2 = Arc::new(MockParallelAgent::new("agent-2", "output-B", 10)) as Arc<dyn Agent>;

        let parallel = ParallelAgent::new("par", vec![agent1, agent2]);
        let ctx = mock_context();

        let mut stream = parallel.run(ctx).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // Should have TextDelta from each agent + final Done
        assert!(events.len() >= 3, "Expected at least 3 events, got {}", events.len());

        // Final event should be Done with combined output
        if let Some(AgentEvent::Done { output }) = events.last() {
            let out = output.as_ref().unwrap();
            assert!(out.contains("output-A"), "Expected output-A in combined output");
            assert!(out.contains("output-B"), "Expected output-B in combined output");
        } else {
            panic!("Expected final Done event");
        }
    }

    #[tokio::test]
    async fn test_parallel_agent_empty_returns_error() {
        let parallel = ParallelAgent::new("empty-par", vec![]);
        let ctx = mock_context();
        let result = parallel.run(ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parallel_agent_single_agent() {
        let agent = Arc::new(MockParallelAgent::new("solo", "only-output", 0)) as Arc<dyn Agent>;
        let parallel = ParallelAgent::new("single-par", vec![agent]);
        let ctx = mock_context();

        let mut stream = parallel.run(ctx).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // Should have TextDelta + Done
        assert!(events.len() >= 2);
        if let Some(AgentEvent::Done { output }) = events.last() {
            let out = output.as_ref().unwrap();
            assert!(out.contains("only-output"));
        } else {
            panic!("Expected Done event");
        }
    }

    #[test]
    fn test_tag_event_state_update() {
        let event = AgentEvent::StateUpdate {
            key: "result".to_string(),
            value: serde_json::json!("ok"),
        };
        let tagged = tag_event(event, "agent-1");
        match tagged {
            AgentEvent::StateUpdate { key, .. } => {
                assert_eq!(key, "agent-1.result");
            }
            _ => panic!("Expected StateUpdate"),
        }
    }

    #[test]
    fn test_tag_event_text_delta_passthrough() {
        let event = AgentEvent::TextDelta {
            content: "hello".to_string(),
        };
        let tagged = tag_event(event, "agent-1");
        match tagged {
            AgentEvent::TextDelta { content } => {
                assert_eq!(content, "hello");
            }
            _ => panic!("Expected TextDelta"),
        }
    }
}
