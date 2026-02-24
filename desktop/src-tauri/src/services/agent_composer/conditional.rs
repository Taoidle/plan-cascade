//! ConditionalAgent — routes execution based on shared state
//!
//! Evaluates a condition function against the shared state to determine
//! which branch agent to run. Useful for implementing routing, A/B testing,
//! and decision trees in agent pipelines.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use super::types::{Agent, AgentContext, AgentEventStream};
use crate::utils::error::{AppError, AppResult};

/// Type alias for the condition function.
///
/// Takes a reference to the shared state map and returns a branch key
/// (a `String`) that selects which agent to run.
pub type ConditionFn = Box<dyn Fn(&HashMap<String, Value>) -> String + Send + Sync>;

/// A composite agent that routes to different sub-agents based on shared state.
///
/// When `run()` is called, the condition function is evaluated against the
/// current shared state to produce a branch key. The agent registered for
/// that key is then executed. If no matching branch is found, an error is returned.
pub struct ConditionalAgent {
    /// Display name for this agent.
    name: String,
    /// Description of this agent.
    description: String,
    /// Condition function that evaluates shared state to select a branch.
    condition: ConditionFn,
    /// Map of branch keys to agents.
    branches: HashMap<String, Arc<dyn Agent>>,
    /// Optional default agent to use if no branch matches.
    default_branch: Option<Arc<dyn Agent>>,
}

impl ConditionalAgent {
    /// Create a new ConditionalAgent with a condition function and branches.
    pub fn new(
        name: impl Into<String>,
        condition: ConditionFn,
        branches: HashMap<String, Arc<dyn Agent>>,
    ) -> Self {
        Self {
            name: name.into(),
            description: "Routes to sub-agents based on shared state conditions".to_string(),
            condition,
            branches,
            default_branch: None,
        }
    }

    /// Set a custom description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set a default branch agent for when no branch key matches.
    pub fn with_default(mut self, agent: Arc<dyn Agent>) -> Self {
        self.default_branch = Some(agent);
        self
    }
}

#[async_trait]
impl Agent for ConditionalAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream> {
        // Read the shared state to evaluate the condition
        let shared_state = ctx.shared_state.read().await;
        let branch_key = (self.condition)(&shared_state);
        drop(shared_state); // Release the read lock

        // Find the agent for this branch
        let agent = self
            .branches
            .get(&branch_key)
            .cloned()
            .or_else(|| self.default_branch.clone())
            .ok_or_else(|| {
                AppError::validation(format!(
                    "ConditionalAgent '{}': no branch matches key '{}' and no default branch configured. Available branches: {:?}",
                    self.name,
                    branch_key,
                    self.branches.keys().collect::<Vec<_>>()
                ))
            })?;

        // Delegate to the selected agent
        agent.run(ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::types::*;
    use futures_util::StreamExt;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Mock agent that returns its name as output.
    struct MockBranchAgent {
        name: String,
    }

    impl MockBranchAgent {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl Agent for MockBranchAgent {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "Mock branch agent"
        }
        async fn run(&self, _ctx: AgentContext) -> AppResult<AgentEventStream> {
            let output = self.name.clone();
            let stream = futures_util::stream::iter(vec![
                Ok(AgentEvent::TextDelta {
                    content: format!("Running {}", output),
                }),
                Ok(AgentEvent::Done {
                    output: Some(output),
                }),
            ]);
            Ok(Box::pin(stream))
        }
    }

    /// Mock LLM provider for testing (never called).
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

    fn mock_context_with_state(state: HashMap<String, Value>) -> AgentContext {
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
            input: AgentInput::Text("test".to_string()),
            shared_state: Arc::new(RwLock::new(state)),
            config: AgentConfig::default(),
            orchestrator_ctx: None,
        }
    }

    #[tokio::test]
    async fn test_conditional_routes_to_correct_branch() {
        let mut branches: HashMap<String, Arc<dyn Agent>> = HashMap::new();
        branches.insert(
            "yes".to_string(),
            Arc::new(MockBranchAgent::new("yes-agent")),
        );
        branches.insert("no".to_string(), Arc::new(MockBranchAgent::new("no-agent")));

        let condition: ConditionFn = Box::new(|state| {
            state
                .get("decision")
                .and_then(|v| v.as_str())
                .unwrap_or("no")
                .to_string()
        });

        let agent = ConditionalAgent::new("cond", condition, branches);

        // Route to "yes" branch
        let mut state = HashMap::new();
        state.insert("decision".to_string(), Value::String("yes".to_string()));
        let ctx = mock_context_with_state(state);

        let mut stream = agent.run(ctx).await.unwrap();
        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // Should have run the "yes" agent
        if let Some(AgentEvent::Done { output }) = events.last() {
            assert_eq!(output.as_ref().unwrap(), "yes-agent");
        } else {
            panic!("Expected Done event with yes-agent output");
        }
    }

    #[tokio::test]
    async fn test_conditional_missing_branch_returns_error() {
        let branches: HashMap<String, Arc<dyn Agent>> = HashMap::new();
        let condition: ConditionFn = Box::new(|_| "nonexistent".to_string());

        let agent = ConditionalAgent::new("cond", condition, branches);
        let ctx = mock_context_with_state(HashMap::new());

        let result = agent.run(ctx).await;
        match result {
            Err(e) => {
                let err = e.to_string();
                assert!(
                    err.contains("nonexistent"),
                    "Error should mention 'nonexistent': {}",
                    err
                );
            }
            Ok(_) => panic!("Expected error for missing branch"),
        }
    }

    #[tokio::test]
    async fn test_conditional_uses_default_branch() {
        let branches: HashMap<String, Arc<dyn Agent>> = HashMap::new();
        let condition: ConditionFn = Box::new(|_| "unknown".to_string());

        let default_agent = Arc::new(MockBranchAgent::new("default-agent"));
        let agent = ConditionalAgent::new("cond", condition, branches).with_default(default_agent);

        let ctx = mock_context_with_state(HashMap::new());

        let mut stream = agent.run(ctx).await.unwrap();
        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        if let Some(AgentEvent::Done { output }) = events.last() {
            assert_eq!(output.as_ref().unwrap(), "default-agent");
        } else {
            panic!("Expected Done event with default-agent output");
        }
    }
}
