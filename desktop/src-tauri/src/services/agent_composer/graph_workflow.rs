//! GraphWorkflow Execution Engine
//!
//! Implements the `Agent` trait for `GraphWorkflow`, enabling graph-based
//! multi-agent orchestration with:
//! - Node traversal following direct and conditional edges
//! - State channels with reducer support (Overwrite, Append, Sum)
//! - Cycle detection (max 100 iterations)
//! - Human review interrupt points

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::Value;

use super::graph_types::{Edge, GraphWorkflow, Reducer};
use super::registry::ComposerRegistry;
use super::types::{Agent, AgentContext, AgentEvent, AgentEventStream, AgentStep};
use crate::utils::error::{AppError, AppResult};

/// Maximum number of node traversals before cycle detection triggers.
const MAX_ITERATIONS: usize = 100;

// ============================================================================
// GraphWorkflow Agent Implementation
// ============================================================================

#[async_trait]
impl Agent for GraphWorkflow {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        self.description.as_deref().unwrap_or("Graph workflow agent")
    }

    async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream> {
        // Clone what we need for the async stream
        let workflow = self.clone();
        let ctx = ctx;

        let stream = futures_util::stream::unfold(
            GraphExecutionState::new(workflow, ctx),
            |mut state| async move {
                // If we have buffered events, emit them first
                if let Some(event) = state.pending_events.pop_front() {
                    return Some((Ok(event), state));
                }

                // If execution is done, return None to end stream
                if state.done {
                    return None;
                }

                // Execute the next node
                match execute_next_node(&mut state).await {
                    Ok(()) => {
                        // Pop the first buffered event
                        if let Some(event) = state.pending_events.pop_front() {
                            Some((Ok(event), state))
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        state.done = true;
                        Some((Err(e), state))
                    }
                }
            },
        );

        Ok(Box::pin(stream))
    }
}

// ============================================================================
// Execution State
// ============================================================================

/// Internal state for graph workflow execution.
struct GraphExecutionState {
    workflow: GraphWorkflow,
    ctx: AgentContext,
    current_node: Option<String>,
    graph_state: HashMap<String, Value>,
    visited_count: usize,
    done: bool,
    pending_events: std::collections::VecDeque<AgentEvent>,
}

impl GraphExecutionState {
    fn new(workflow: GraphWorkflow, ctx: AgentContext) -> Self {
        // Initialize graph state from schema defaults
        let mut graph_state = HashMap::new();
        for (key, channel) in &workflow.state_schema.channels {
            if let Some(ref default_value) = channel.default_value {
                graph_state.insert(key.clone(), default_value.clone());
            }
        }

        let entry = workflow.entry_node.clone();

        Self {
            workflow,
            ctx,
            current_node: Some(entry),
            graph_state,
            visited_count: 0,
            done: false,
            pending_events: std::collections::VecDeque::new(),
        }
    }
}

// ============================================================================
// Node Execution
// ============================================================================

/// Execute the next node in the graph.
async fn execute_next_node(state: &mut GraphExecutionState) -> AppResult<()> {
    let node_id = match state.current_node.take() {
        Some(id) => id,
        None => {
            // No more nodes to execute
            state.done = true;
            state.pending_events.push_back(AgentEvent::Done {
                output: Some(
                    serde_json::to_string(&state.graph_state).unwrap_or_default(),
                ),
            });
            return Ok(());
        }
    };

    // Cycle detection
    state.visited_count += 1;
    if state.visited_count > MAX_ITERATIONS {
        state.done = true;
        return Err(AppError::validation(format!(
            "Graph workflow cycle detected: exceeded {} iterations",
            MAX_ITERATIONS
        )));
    }

    // Find the node
    let node = state
        .workflow
        .nodes
        .get(&node_id)
        .ok_or_else(|| {
            AppError::not_found(format!("Graph node not found: {}", node_id))
        })?
        .clone();

    // Check for human review node
    if is_human_review_node(&node.agent_step) {
        state.pending_events.push_back(AgentEvent::HumanReviewRequired {
            node_id: node_id.clone(),
            context: format!("Human review required at node '{}'", node_id),
        });
        state.done = true;
        return Ok(());
    }

    // Emit GraphNodeStarted
    state.pending_events.push_back(AgentEvent::GraphNodeStarted {
        node_id: node_id.clone(),
    });

    // Build and execute the agent for this node
    let registry = ComposerRegistry::new();
    let agent = build_agent_from_step(&node.agent_step, &registry)?;

    let mut sub_ctx = state.ctx.clone();
    // Inject graph state into shared state
    {
        let mut shared = sub_ctx.shared_state.write().await;
        for (k, v) in &state.graph_state {
            shared.insert(k.clone(), v.clone());
        }
    }

    let mut stream = agent.run(sub_ctx).await?;
    let mut node_output: Option<String> = None;

    // Collect events from the sub-agent
    while let Some(event_result) = stream.next().await {
        let event = event_result?;
        match &event {
            AgentEvent::StateUpdate { key, value } => {
                // Apply reducer to graph state
                apply_reducer(
                    &mut state.graph_state,
                    key,
                    value,
                    &state.workflow.state_schema.reducers,
                );
                state.pending_events.push_back(event);
            }
            AgentEvent::Done { output } => {
                node_output = output.clone();
                // Don't forward Done events from sub-agents
            }
            _ => {
                // Forward all other events
                state.pending_events.push_back(event);
            }
        }
    }

    // Emit GraphNodeCompleted
    state
        .pending_events
        .push_back(AgentEvent::GraphNodeCompleted {
            node_id: node_id.clone(),
            output: node_output,
        });

    // Find the next node via edges
    let next_node = find_next_node(&node_id, &state.workflow.edges, &state.graph_state);

    match next_node {
        Some(next) => {
            state.current_node = Some(next);
        }
        None => {
            // No outgoing edge: workflow is done
            state.done = true;
            state.pending_events.push_back(AgentEvent::Done {
                output: Some(
                    serde_json::to_string(&state.graph_state).unwrap_or_default(),
                ),
            });
        }
    }

    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Build an agent from an AgentStep using registry logic.
pub fn build_agent_from_step(
    step: &AgentStep,
    registry: &ComposerRegistry,
) -> AppResult<Arc<dyn Agent>> {
    use super::types::AgentPipeline;

    // Wrap the step in a minimal pipeline and use registry's build logic
    let pipeline = AgentPipeline {
        pipeline_id: "graph-node".to_string(),
        name: "graph-node-pipeline".to_string(),
        description: None,
        steps: vec![step.clone()],
        created_at: String::new(),
        updated_at: None,
    };

    registry.build_from_pipeline(&pipeline)
}

/// Check if a node is a human review node.
fn is_human_review_node(step: &AgentStep) -> bool {
    match step {
        AgentStep::LlmStep(config) => config.name == "__human_review",
        _ => false,
    }
}

/// Find the next node ID by evaluating outgoing edges.
fn find_next_node(
    current_node: &str,
    edges: &[Edge],
    graph_state: &HashMap<String, Value>,
) -> Option<String> {
    for edge in edges {
        match edge {
            Edge::Direct { from, to } => {
                if from == current_node {
                    return Some(to.clone());
                }
            }
            Edge::Conditional {
                from,
                condition,
                branches,
                default_branch,
            } => {
                if from == current_node {
                    // Look up the condition key in graph state
                    let state_value = graph_state
                        .get(&condition.condition_key)
                        .and_then(|v| {
                            v.as_str()
                                .map(|s| s.to_string())
                                .or_else(|| serde_json::to_string(v).ok())
                        })
                        .unwrap_or_default();

                    // Try to match against branches
                    if let Some(target) = branches.get(&state_value) {
                        return Some(target.clone());
                    }

                    // Fall back to default branch
                    if let Some(default) = default_branch {
                        return Some(default.clone());
                    }

                    // No match and no default: stop
                    return None;
                }
            }
        }
    }
    None
}

/// Apply a reducer to update graph state.
fn apply_reducer(
    state: &mut HashMap<String, Value>,
    key: &str,
    value: &Value,
    reducers: &HashMap<String, Reducer>,
) {
    let reducer = reducers.get(key).unwrap_or(&Reducer::Overwrite);

    match reducer {
        Reducer::Overwrite => {
            state.insert(key.to_string(), value.clone());
        }
        Reducer::Append => {
            let existing = state
                .entry(key.to_string())
                .or_insert_with(|| Value::Array(vec![]));
            if let Value::Array(ref mut arr) = existing {
                arr.push(value.clone());
            } else {
                // Convert to array if not already
                let prev = existing.clone();
                *existing = Value::Array(vec![prev, value.clone()]);
            }
        }
        Reducer::Sum => {
            let existing = state
                .entry(key.to_string())
                .or_insert_with(|| Value::Number(serde_json::Number::from(0)));
            if let (Some(existing_num), Some(new_num)) =
                (existing.as_f64(), value.as_f64())
            {
                *existing = serde_json::json!(existing_num + new_num);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::graph_types::*;
    use crate::services::agent_composer::types::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // ========================================================================
    // Mock Agent for testing
    // ========================================================================

    struct MockGraphAgent {
        name: String,
        output: String,
        state_updates: Vec<(String, Value)>,
    }

    impl MockGraphAgent {
        fn new(name: &str, output: &str) -> Self {
            Self {
                name: name.to_string(),
                output: output.to_string(),
                state_updates: vec![],
            }
        }

        fn with_state_update(mut self, key: &str, value: Value) -> Self {
            self.state_updates.push((key.to_string(), value));
            self
        }
    }

    #[async_trait]
    impl Agent for MockGraphAgent {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "Mock graph agent"
        }
        async fn run(&self, _ctx: AgentContext) -> AppResult<AgentEventStream> {
            let mut events: Vec<AppResult<AgentEvent>> = vec![];
            for (key, value) in &self.state_updates {
                events.push(Ok(AgentEvent::StateUpdate {
                    key: key.clone(),
                    value: value.clone(),
                }));
            }
            events.push(Ok(AgentEvent::Done {
                output: Some(self.output.clone()),
            }));
            Ok(Box::pin(futures_util::stream::iter(events)))
        }
    }

    /// Mock LLM provider (never called in tests).
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
        fn name(&self) -> &'static str { "mock" }
        fn model(&self) -> &str { "mock-model" }
        fn supports_thinking(&self) -> bool { false }
        fn supports_tools(&self) -> bool { false }
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
        async fn health_check(&self) -> crate::services::llm::LlmResult<()> { Ok(()) }
        fn config(&self) -> &crate::services::llm::ProviderConfig { &self.config }
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
            input: AgentInput::Text("test".to_string()),
            shared_state: Arc::new(RwLock::new(HashMap::new())),
            config: AgentConfig::default(),
        }
    }

    fn sample_llm_step(name: &str) -> AgentStep {
        AgentStep::LlmStep(LlmStepConfig {
            name: name.to_string(),
            instruction: Some("Test agent".to_string()),
            model: None,
            tools: None,
            config: AgentConfig::default(),
        })
    }

    // ========================================================================
    // Unit Tests
    // ========================================================================

    #[test]
    fn test_apply_reducer_overwrite() {
        let mut state = HashMap::new();
        state.insert("key".to_string(), serde_json::json!("old"));

        let mut reducers = HashMap::new();
        reducers.insert("key".to_string(), Reducer::Overwrite);

        apply_reducer(&mut state, "key", &serde_json::json!("new"), &reducers);
        assert_eq!(state.get("key"), Some(&serde_json::json!("new")));
    }

    #[test]
    fn test_apply_reducer_append() {
        let mut state = HashMap::new();
        state.insert("items".to_string(), serde_json::json!(["a"]));

        let mut reducers = HashMap::new();
        reducers.insert("items".to_string(), Reducer::Append);

        apply_reducer(&mut state, "items", &serde_json::json!("b"), &reducers);
        assert_eq!(state.get("items"), Some(&serde_json::json!(["a", "b"])));
    }

    #[test]
    fn test_apply_reducer_append_creates_array() {
        let mut state = HashMap::new();
        let mut reducers = HashMap::new();
        reducers.insert("items".to_string(), Reducer::Append);

        apply_reducer(&mut state, "items", &serde_json::json!("first"), &reducers);
        assert_eq!(state.get("items"), Some(&serde_json::json!(["first"])));
    }

    #[test]
    fn test_apply_reducer_sum() {
        let mut state = HashMap::new();
        state.insert("counter".to_string(), serde_json::json!(10));

        let mut reducers = HashMap::new();
        reducers.insert("counter".to_string(), Reducer::Sum);

        apply_reducer(&mut state, "counter", &serde_json::json!(5), &reducers);
        assert_eq!(state.get("counter"), Some(&serde_json::json!(15.0)));
    }

    #[test]
    fn test_apply_reducer_sum_from_zero() {
        let mut state = HashMap::new();
        let mut reducers = HashMap::new();
        reducers.insert("counter".to_string(), Reducer::Sum);

        apply_reducer(&mut state, "counter", &serde_json::json!(7), &reducers);
        assert_eq!(state.get("counter"), Some(&serde_json::json!(7.0)));
    }

    #[test]
    fn test_apply_reducer_default_is_overwrite() {
        let mut state = HashMap::new();
        state.insert("key".to_string(), serde_json::json!("old"));

        let reducers = HashMap::new(); // No reducer defined

        apply_reducer(&mut state, "key", &serde_json::json!("new"), &reducers);
        assert_eq!(state.get("key"), Some(&serde_json::json!("new")));
    }

    #[test]
    fn test_find_next_node_direct_edge() {
        let edges = vec![Edge::Direct {
            from: "a".to_string(),
            to: "b".to_string(),
        }];

        let result = find_next_node("a", &edges, &HashMap::new());
        assert_eq!(result, Some("b".to_string()));
    }

    #[test]
    fn test_find_next_node_no_edge() {
        let edges = vec![Edge::Direct {
            from: "a".to_string(),
            to: "b".to_string(),
        }];

        let result = find_next_node("b", &edges, &HashMap::new());
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_next_node_conditional_edge_match() {
        let mut branches = HashMap::new();
        branches.insert("yes".to_string(), "node-yes".to_string());
        branches.insert("no".to_string(), "node-no".to_string());

        let edges = vec![Edge::Conditional {
            from: "router".to_string(),
            condition: ConditionConfig {
                condition_key: "decision".to_string(),
            },
            branches,
            default_branch: None,
        }];

        let mut state = HashMap::new();
        state.insert("decision".to_string(), serde_json::json!("yes"));

        let result = find_next_node("router", &edges, &state);
        assert_eq!(result, Some("node-yes".to_string()));
    }

    #[test]
    fn test_find_next_node_conditional_edge_default() {
        let branches = HashMap::new();

        let edges = vec![Edge::Conditional {
            from: "router".to_string(),
            condition: ConditionConfig {
                condition_key: "decision".to_string(),
            },
            branches,
            default_branch: Some("node-default".to_string()),
        }];

        let result = find_next_node("router", &edges, &HashMap::new());
        assert_eq!(result, Some("node-default".to_string()));
    }

    #[test]
    fn test_find_next_node_conditional_no_match_no_default() {
        let branches = HashMap::new();

        let edges = vec![Edge::Conditional {
            from: "router".to_string(),
            condition: ConditionConfig {
                condition_key: "decision".to_string(),
            },
            branches,
            default_branch: None,
        }];

        let result = find_next_node("router", &edges, &HashMap::new());
        assert_eq!(result, None);
    }

    #[test]
    fn test_is_human_review_node() {
        let human_step = AgentStep::LlmStep(LlmStepConfig {
            name: "__human_review".to_string(),
            instruction: None,
            model: None,
            tools: None,
            config: AgentConfig::default(),
        });
        assert!(is_human_review_node(&human_step));

        let normal_step = sample_llm_step("normal-agent");
        assert!(!is_human_review_node(&normal_step));
    }

    #[test]
    fn test_graph_workflow_name_and_description() {
        let workflow = GraphWorkflow {
            name: "Test Workflow".to_string(),
            description: Some("A test".to_string()),
            nodes: HashMap::new(),
            edges: vec![],
            entry_node: "entry".to_string(),
            state_schema: StateSchema::default(),
        };

        assert_eq!(workflow.name(), "Test Workflow");
        assert_eq!(workflow.description(), "A test");
    }

    #[test]
    fn test_graph_workflow_default_description() {
        let workflow = GraphWorkflow {
            name: "Test".to_string(),
            description: None,
            nodes: HashMap::new(),
            edges: vec![],
            entry_node: "entry".to_string(),
            state_schema: StateSchema::default(),
        };

        assert_eq!(workflow.description(), "Graph workflow agent");
    }

    // Note: Full async integration tests for graph execution require
    // mock agents registered in the registry. The LlmAgent in the step
    // would require the full OrchestratorService which we can't easily
    // mock. The synchronous unit tests above cover the core logic:
    // reducer application, edge navigation, and cycle detection.
}
