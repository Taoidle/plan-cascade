//! SequentialAgent — runs sub-agents in order, chaining outputs
//!
//! The output of agent N becomes the input of agent N+1 via `AgentInput::Text`.
//! All sub-agent events are forwarded to the caller's stream with agent-name
//! prefixed keys for StateUpdate events.

use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;

use super::types::{Agent, AgentContext, AgentEvent, AgentEventStream, AgentInput};
use crate::utils::error::{AppError, AppResult};

/// A composite agent that runs sub-agents sequentially.
///
/// Each sub-agent's output (from the `Done` event) is passed as the input
/// to the next sub-agent. All intermediate events are forwarded to the
/// caller, with `StateUpdate` keys prefixed by the sub-agent's name.
pub struct SequentialAgent {
    /// Display name for this composite agent.
    name: String,
    /// Description of this composite agent.
    description: String,
    /// Ordered list of sub-agents to run.
    agents: Vec<Arc<dyn Agent>>,
}

impl SequentialAgent {
    /// Create a new SequentialAgent with the given name and sub-agents.
    pub fn new(name: impl Into<String>, agents: Vec<Arc<dyn Agent>>) -> Self {
        Self {
            name: name.into(),
            description: "Runs sub-agents sequentially, chaining outputs".to_string(),
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
impl Agent for SequentialAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream> {
        if self.agents.is_empty() {
            return Err(AppError::validation("SequentialAgent has no sub-agents"));
        }

        // Clone what we need for the async block
        let agents: Vec<Arc<dyn Agent>> = self.agents.clone();
        let ctx = ctx;

        // Build a stream that runs agents one at a time
        let stream = futures_util::stream::unfold(
            SequentialState::new(agents, ctx),
            |mut state| async move {
                loop {
                    // If we have an active sub-stream, try to get the next event
                    if let Some(ref mut sub_stream) = state.current_stream {
                        match sub_stream.next().await {
                            Some(Ok(event)) => {
                                match &event {
                                    AgentEvent::Done { output } => {
                                        // Capture output for next agent's input
                                        state.last_output = output.clone();
                                        state.current_stream = None;
                                        state.current_index += 1;

                                        // If this was the last agent, emit Done
                                        if state.current_index >= state.agents.len() {
                                            return Some((
                                                Ok(AgentEvent::Done {
                                                    output: state.last_output.clone(),
                                                }),
                                                state,
                                            ));
                                        }

                                        // Otherwise, don't emit the intermediate Done,
                                        // just continue to start the next agent
                                        continue;
                                    }
                                    AgentEvent::StateUpdate { key, value } => {
                                        // Prefix state updates with agent name
                                        let agent_name = &state.agents[state.current_index].name();
                                        let prefixed = AgentEvent::StateUpdate {
                                            key: format!("{}.{}", agent_name, key),
                                            value: value.clone(),
                                        };
                                        return Some((Ok(prefixed), state));
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
                                // Stream ended without Done event; move to next
                                state.current_stream = None;
                                state.current_index += 1;

                                if state.current_index >= state.agents.len() {
                                    return Some((
                                        Ok(AgentEvent::Done {
                                            output: state.last_output.clone(),
                                        }),
                                        state,
                                    ));
                                }
                                continue;
                            }
                        }
                    }

                    // No active stream — check if we have more agents to run
                    if state.current_index >= state.agents.len() {
                        return None; // All agents done, end stream
                    }

                    // Start the next agent
                    let agent = &state.agents[state.current_index];
                    let mut sub_ctx = state.base_ctx.clone();

                    // Chain output from previous agent as input to current agent
                    if let Some(ref prev_output) = state.last_output {
                        sub_ctx.input = AgentInput::Text(prev_output.clone());
                    }

                    match agent.run(sub_ctx).await {
                        Ok(stream) => {
                            state.current_stream = Some(stream);
                            continue;
                        }
                        Err(e) => {
                            return Some((Err(e), state));
                        }
                    }
                }
            },
        );

        Ok(Box::pin(stream))
    }
}

/// Internal state for the sequential unfold stream.
struct SequentialState {
    agents: Vec<Arc<dyn Agent>>,
    base_ctx: AgentContext,
    current_index: usize,
    current_stream: Option<AgentEventStream>,
    last_output: Option<String>,
}

impl SequentialState {
    fn new(agents: Vec<Arc<dyn Agent>>, ctx: AgentContext) -> Self {
        Self {
            agents,
            base_ctx: ctx,
            current_index: 0,
            current_stream: None,
            last_output: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::types::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// A mock agent for testing that emits a TextDelta and Done.
    struct MockAgent {
        name: String,
        output: String,
    }

    impl MockAgent {
        fn new(name: &str, output: &str) -> Self {
            Self {
                name: name.to_string(),
                output: output.to_string(),
            }
        }
    }

    #[async_trait]
    impl Agent for MockAgent {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Mock agent for testing"
        }

        async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream> {
            let input_text = ctx.input.as_text();
            let output = if input_text.is_empty() {
                self.output.clone()
            } else {
                format!("{}+{}", input_text, self.output)
            };
            let output_clone = output.clone();

            let stream = futures_util::stream::iter(vec![
                Ok(AgentEvent::TextDelta {
                    content: output.clone(),
                }),
                Ok(AgentEvent::Done {
                    output: Some(output_clone),
                }),
            ]);
            Ok(Box::pin(stream))
        }
    }

    /// Create a minimal AgentContext for testing (no real provider needed).
    fn mock_context() -> AgentContext {
        use crate::services::llm::types::ProviderConfig;

        // We need a real LlmProvider for the context, but we won't call it.
        // Use a mock approach by creating a struct that implements the trait.
        let provider = Arc::new(MockProvider::new());
        let tool_executor = Arc::new(crate::services::tools::ToolExecutor::new(&PathBuf::from(
            "/tmp",
        )));
        let hooks = Arc::new(crate::services::orchestrator::hooks::AgenticHooks::new());

        AgentContext {
            session_id: "test-session".to_string(),
            project_root: PathBuf::from("/tmp"),
            provider,
            tool_executor,
            plugin_manager: None,
            hooks,
            input: AgentInput::Text("initial".to_string()),
            shared_state: Arc::new(RwLock::new(HashMap::new())),
            config: AgentConfig::default(),
            orchestrator_ctx: None,
        }
    }

    /// Minimal mock LLM provider that is never actually called.
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
            unimplemented!("MockProvider::send_message should not be called in unit tests")
        }
        async fn stream_message(
            &self,
            _messages: Vec<crate::services::llm::Message>,
            _system: Option<String>,
            _tools: Vec<crate::services::llm::ToolDefinition>,
            _tx: tokio::sync::mpsc::Sender<crate::services::streaming::UnifiedStreamEvent>,
            _options: crate::services::llm::LlmRequestOptions,
        ) -> crate::services::llm::LlmResult<crate::services::llm::LlmResponse> {
            unimplemented!("MockProvider::stream_message should not be called in unit tests")
        }
        async fn health_check(&self) -> crate::services::llm::LlmResult<()> {
            Ok(())
        }
        fn config(&self) -> &crate::services::llm::ProviderConfig {
            &self.config
        }
    }

    #[tokio::test]
    async fn test_sequential_agent_chains_output() {
        let agent1 = Arc::new(MockAgent::new("agent-1", "A")) as Arc<dyn Agent>;
        let agent2 = Arc::new(MockAgent::new("agent-2", "B")) as Arc<dyn Agent>;

        let seq = SequentialAgent::new("seq", vec![agent1, agent2]);
        let ctx = mock_context();

        let mut stream = seq.run(ctx).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // Should have: TextDelta from agent1, TextDelta from agent2, Done
        assert!(
            events.len() >= 3,
            "Expected at least 3 events, got {}",
            events.len()
        );

        // Final Done should have chained output
        if let Some(AgentEvent::Done { output }) = events.last() {
            let out = output.as_ref().unwrap();
            assert!(out.contains("A"), "Expected output to contain A");
            assert!(out.contains("B"), "Expected output to contain B");
        } else {
            panic!("Expected Done event at end");
        }
    }

    #[tokio::test]
    async fn test_sequential_agent_empty_returns_error() {
        let seq = SequentialAgent::new("empty-seq", vec![]);
        let ctx = mock_context();
        let result = seq.run(ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sequential_agent_single_agent() {
        let agent = Arc::new(MockAgent::new("solo", "output")) as Arc<dyn Agent>;
        let seq = SequentialAgent::new("single-seq", vec![agent]);
        let ctx = mock_context();

        let mut stream = seq.run(ctx).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // Should have TextDelta and Done
        assert!(events.len() >= 2);
        if let Some(AgentEvent::Done { output }) = events.last() {
            assert!(output.is_some());
        } else {
            panic!("Expected Done event");
        }
    }
}
