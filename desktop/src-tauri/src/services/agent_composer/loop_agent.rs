//! LoopAgent — condition-based iteration agent
//!
//! Repeatedly executes a sub-agent until a condition function evaluating shared
//! state returns `false`, the loop stalls, or the hard iteration limit is reached. The output of iteration
//! N becomes the input of iteration N+1 via `AgentInput::Text` (same chaining
//! pattern as `SequentialAgent`). `StateUpdate` events are prefixed with
//! `loop.{iteration_index}.{agent_name}.{key}`.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::Value;

use super::types::{Agent, AgentContext, AgentEvent, AgentEventStream, AgentInput};
use crate::services::orchestrator::{
    build_iteration_budget, ExecutionKind, IterationBudget, IterationBudgetHints,
};
use crate::utils::error::AppResult;

/// Type alias for the loop condition function.
///
/// Takes a reference to the shared state map and returns a `bool`:
/// - `true` means continue looping
/// - `false` means stop looping
pub type LoopConditionFn = Box<dyn Fn(&HashMap<String, Value>) -> bool + Send + Sync>;

/// A composite agent that repeatedly executes a sub-agent until a condition
/// evaluates to `false`, the loop stalls, or the hard limit is reached.
///
/// On each iteration, the sub-agent runs with the current context. The output
/// of iteration N becomes the `AgentInput::Text` for iteration N+1.
/// `StateUpdate` events are prefixed with `loop.{iteration}.{agent_name}.{key}`.
/// The final `Done` event contains the output from the last iteration.
pub struct LoopAgent {
    /// Display name for this composite agent.
    name: String,
    /// Description of this composite agent.
    description: String,
    /// The sub-agent to execute repeatedly.
    agent: Arc<dyn Agent>,
    /// Condition function stored in Arc so it can be shared with the unfold closure.
    condition: Arc<LoopConditionFn>,
    /// Soft limit override used by the loop agent.
    soft_limit_override: Option<u32>,
}

impl LoopAgent {
    /// Create a new LoopAgent with the given name, sub-agent, and condition.
    pub fn new(name: impl Into<String>, agent: Arc<dyn Agent>, condition: LoopConditionFn) -> Self {
        Self {
            name: name.into(),
            description: "Repeatedly executes a sub-agent until a condition is met".to_string(),
            agent,
            condition: Arc::new(condition),
            soft_limit_override: None,
        }
    }

    /// Set a custom description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set a custom soft limit override for the loop agent.
    pub fn with_soft_limit_override(mut self, soft_limit_override: Option<u32>) -> Self {
        self.soft_limit_override = soft_limit_override;
        self
    }
}

#[async_trait]
impl Agent for LoopAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream> {
        let condition = self.condition.clone();
        let budget = build_iteration_budget(
            ExecutionKind::AgentComposerLoopStep,
            &IterationBudgetHints {
                prompt_chars: ctx.input.as_text().chars().count(),
                complexity_score: 0,
                has_specialized_tools: false,
                analysis_profile: None,
                soft_limit_override: self.soft_limit_override,
            },
        );

        // Check the condition before starting the first iteration
        {
            let shared_state = ctx.shared_state.read().await;
            if !condition(&shared_state) {
                // Condition is already false; emit Done with no output
                let stream =
                    futures_util::stream::iter(vec![Ok(AgentEvent::Done { output: None })]);
                return Ok(Box::pin(stream));
            }
        }

        let stream = futures_util::stream::unfold(
            LoopState {
                agent: self.agent.clone(),
                base_ctx: ctx,
                condition,
                budget,
                next_review_iteration: budget.soft_limit,
                current_iteration: 0,
                current_stream: None,
                last_output: None,
                recent_snapshots: VecDeque::new(),
                finished: false,
            },
            |mut state| async move {
                // If we already emitted the final Done, end the stream
                if state.finished {
                    return None;
                }

                loop {
                    // If we have an active sub-stream, poll it
                    if let Some(ref mut sub_stream) = state.current_stream {
                        match sub_stream.next().await {
                            Some(Ok(event)) => {
                                match &event {
                                    AgentEvent::Done { output } => {
                                        // Capture output for next iteration's input
                                        state.last_output = output.clone();
                                        state.current_stream = None;
                                        state.current_iteration += 1;

                                        state.capture_progress_snapshot().await;

                                        if state.current_iteration >= state.budget.hard_limit {
                                            state.finished = true;
                                            return Some((state.finish_on_hard_limit(), state));
                                        }

                                        if state.should_review_progress()
                                            && !state.has_recent_progress()
                                        {
                                            state.finished = true;
                                            return Some((
                                                Ok(AgentEvent::Failed {
                                                    run_id: state.base_ctx.session_id.clone(),
                                                    error: "iteration_stalled".to_string(),
                                                    duration_ms: 0,
                                                }),
                                                state,
                                            ));
                                        }

                                        // Check condition for next iteration
                                        let shared_state = state.base_ctx.shared_state.read().await;
                                        let should_continue = (state.condition)(&shared_state);
                                        drop(shared_state);

                                        if !should_continue {
                                            state.finished = true;
                                            return Some((
                                                Ok(AgentEvent::Done {
                                                    output: state.last_output.clone(),
                                                }),
                                                state,
                                            ));
                                        }

                                        // Don't emit intermediate Done; continue to
                                        // next iteration
                                        continue;
                                    }
                                    AgentEvent::StateUpdate { key, value } => {
                                        // Prefix with loop.{iteration}.{agent_name}.{key}
                                        let agent_name = state.agent.name().to_string();
                                        let prefixed = AgentEvent::StateUpdate {
                                            key: format!(
                                                "loop.{}.{}.{}",
                                                state.current_iteration, agent_name, key
                                            ),
                                            value: value.clone(),
                                        };
                                        return Some((Ok(prefixed), state));
                                    }
                                    _ => {
                                        // Forward all other events unchanged
                                        return Some((Ok(event), state));
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                return Some((Err(e), state));
                            }
                            None => {
                                // Stream ended without a Done event
                                state.current_stream = None;
                                state.current_iteration += 1;

                                state.capture_progress_snapshot().await;

                                if state.current_iteration >= state.budget.hard_limit {
                                    state.finished = true;
                                    return Some((state.finish_on_hard_limit(), state));
                                }

                                if state.should_review_progress() && !state.has_recent_progress() {
                                    state.finished = true;
                                    return Some((
                                        Ok(AgentEvent::Failed {
                                            run_id: state.base_ctx.session_id.clone(),
                                            error: "iteration_stalled".to_string(),
                                            duration_ms: 0,
                                        }),
                                        state,
                                    ));
                                }

                                let shared_state = state.base_ctx.shared_state.read().await;
                                let should_continue = (state.condition)(&shared_state);
                                drop(shared_state);

                                if !should_continue {
                                    state.finished = true;
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

                    // No active stream — check if we still have iterations left
                    if state.finished {
                        return None;
                    }

                    // Start the next iteration
                    let mut sub_ctx = state.base_ctx.clone();

                    // Chain output from previous iteration as input
                    if let Some(ref prev_output) = state.last_output {
                        sub_ctx.input = AgentInput::Text(prev_output.clone());
                    }

                    match state.agent.run(sub_ctx).await {
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

/// Internal state for the loop unfold stream.
struct LoopState {
    agent: Arc<dyn Agent>,
    base_ctx: AgentContext,
    condition: Arc<LoopConditionFn>,
    budget: IterationBudget,
    next_review_iteration: u32,
    current_iteration: u32,
    current_stream: Option<AgentEventStream>,
    last_output: Option<String>,
    recent_snapshots: VecDeque<LoopProgressSnapshot>,
    /// Set to true after emitting the final Done event, so the unfold
    /// terminates on the next call.
    finished: bool,
}

#[derive(Clone, Debug)]
struct LoopProgressSnapshot {
    shared_state_fingerprint: Option<u64>,
    output_fingerprint: Option<u64>,
}

impl LoopState {
    async fn capture_progress_snapshot(&mut self) {
        let shared_state_fingerprint = {
            let shared = self.base_ctx.shared_state.read().await;
            fingerprint_shared_state(&shared)
        };

        self.recent_snapshots.push_back(LoopProgressSnapshot {
            shared_state_fingerprint,
            output_fingerprint: self.last_output.as_ref().map(|value| hash_text(value)),
        });

        while self.recent_snapshots.len() > self.budget.review_window as usize {
            self.recent_snapshots.pop_front();
        }
    }

    fn should_review_progress(&mut self) -> bool {
        if self.current_iteration < self.next_review_iteration {
            return false;
        }
        self.next_review_iteration = self
            .next_review_iteration
            .saturating_add(self.budget.review_interval);
        true
    }

    fn has_recent_progress(&self) -> bool {
        if self.recent_snapshots.len() < self.budget.review_window as usize {
            return true;
        }
        let Some(first) = self.recent_snapshots.front() else {
            return true;
        };
        let Some(last) = self.recent_snapshots.back() else {
            return true;
        };

        first.shared_state_fingerprint != last.shared_state_fingerprint
            || first.output_fingerprint != last.output_fingerprint
    }

    fn finish_on_hard_limit(&self) -> AppResult<AgentEvent> {
        if self.last_output.is_some() {
            Ok(AgentEvent::Done {
                output: self.last_output.clone(),
            })
        } else {
            Ok(AgentEvent::Failed {
                run_id: self.base_ctx.session_id.clone(),
                error: "iteration_hard_limit_reached".to_string(),
                duration_ms: 0,
            })
        }
    }
}

fn fingerprint_shared_state(state: &HashMap<String, Value>) -> Option<u64> {
    let ordered: BTreeMap<String, Value> = state
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    serde_json::to_string(&ordered)
        .ok()
        .map(|serialized| hash_text(&serialized))
}

fn hash_text(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Construct a condition function from a `condition_key` for pipeline definitions.
///
/// The condition reads the given key from shared state:
/// - If the key is absent, the loop continues (`true`) — this allows the first
///   iteration to run before the sub-agent has set the key.
/// - If the key is present and its value is truthy, the loop continues.
/// - If the key is present and its value is falsy (`false`, `0`, `""`, `null`,
///   empty array, empty object), the loop stops.
pub fn build_loop_condition(condition_key: String) -> LoopConditionFn {
    Box::new(
        move |state: &HashMap<String, Value>| match state.get(&condition_key) {
            None => true,
            Some(value) => is_truthy(value),
        },
    )
}

/// Determine if a JSON value is "truthy".
///
/// Falsy values: `null`, `false`, `0`, `""`, empty array, empty object.
/// Everything else is truthy.
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i != 0
            } else if let Some(f) = n.as_f64() {
                f != 0.0
            } else {
                true
            }
        }
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::types::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// A mock agent for testing that emits a TextDelta and Done.
    /// If input is non-empty, output = "{input}+{self.output}";
    /// otherwise output = self.output.
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

            let stream = futures_util::stream::iter(vec![
                Ok(AgentEvent::TextDelta {
                    content: output.clone(),
                }),
                Ok(AgentEvent::Done {
                    output: Some(output),
                }),
            ]);
            Ok(Box::pin(stream))
        }
    }

    /// A mock agent that emits StateUpdate events.
    struct MockStateAgent {
        name: String,
    }

    impl MockStateAgent {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl Agent for MockStateAgent {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Mock agent that emits StateUpdate"
        }

        async fn run(&self, _ctx: AgentContext) -> AppResult<AgentEventStream> {
            let stream = futures_util::stream::iter(vec![
                Ok(AgentEvent::StateUpdate {
                    key: "result".to_string(),
                    value: serde_json::json!("some_value"),
                }),
                Ok(AgentEvent::Done {
                    output: Some("state-done".to_string()),
                }),
            ]);
            Ok(Box::pin(stream))
        }
    }

    /// A mock agent that decrements a counter in shared state.
    /// When counter reaches 0, sets "loop_continue" to false.
    struct CountdownAgent {
        name: String,
    }

    impl CountdownAgent {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl Agent for CountdownAgent {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Mock agent that decrements a counter"
        }

        async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream> {
            let iteration = ctx.input.as_text();

            // Read current counter and decrement
            let mut shared = ctx.shared_state.write().await;
            let counter = shared.get("counter").and_then(|v| v.as_i64()).unwrap_or(3);
            let new_counter = counter - 1;
            shared.insert("counter".to_string(), serde_json::json!(new_counter));

            // When counter reaches 0, set "loop_continue" to false
            if new_counter <= 0 {
                shared.insert("loop_continue".to_string(), serde_json::json!(false));
            }
            drop(shared);

            let output = format!("iter-{}", iteration);
            let stream = futures_util::stream::iter(vec![
                Ok(AgentEvent::TextDelta {
                    content: output.clone(),
                }),
                Ok(AgentEvent::Done {
                    output: Some(output),
                }),
            ]);
            Ok(Box::pin(stream))
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

    /// Create a minimal AgentContext for testing.
    fn mock_context() -> AgentContext {
        mock_context_with_state(HashMap::new())
    }

    /// Create a minimal AgentContext with pre-populated shared state.
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
            input: AgentInput::Text("start".to_string()),
            shared_state: Arc::new(RwLock::new(state)),
            config: AgentConfig::default(),
            orchestrator_ctx: None,
        }
    }

    // ========================================================================
    // Test: loop terminates on condition
    // ========================================================================

    #[tokio::test]
    async fn test_loop_terminates_on_condition() {
        let sub_agent = Arc::new(CountdownAgent::new("countdown")) as Arc<dyn Agent>;

        let condition: LoopConditionFn = Box::new(|state| match state.get("loop_continue") {
            Some(v) => is_truthy(v),
            None => true,
        });

        let loop_agent =
            LoopAgent::new("test-loop", sub_agent, condition).with_soft_limit_override(Some(10));

        let mut initial_state = HashMap::new();
        initial_state.insert("counter".to_string(), serde_json::json!(3));
        let ctx = mock_context_with_state(initial_state);

        let mut stream = loop_agent.run(ctx).await.unwrap();
        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // CountdownAgent decrements 3->2->1->0, setting loop_continue=false on
        // the third iteration. We expect 3 iterations of TextDelta + 1 Done.
        let done_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, AgentEvent::Done { .. }))
            .collect();
        assert_eq!(done_events.len(), 1, "Should have exactly one Done event");

        let text_deltas: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, AgentEvent::TextDelta { .. }))
            .collect();
        assert_eq!(
            text_deltas.len(),
            3,
            "Should have 3 TextDelta events (one per iteration)"
        );
    }

    // ========================================================================
    // Test: loop terminates on stalled progress after soft limit
    // ========================================================================

    #[tokio::test]
    async fn test_loop_terminates_on_stalled_progress() {
        struct ConstantAgent {
            name: String,
        }

        #[async_trait]
        impl Agent for ConstantAgent {
            fn name(&self) -> &str {
                &self.name
            }

            fn description(&self) -> &str {
                "Constant output agent"
            }

            async fn run(&self, _ctx: AgentContext) -> AppResult<AgentEventStream> {
                let stream = futures_util::stream::iter(vec![
                    Ok(AgentEvent::TextDelta {
                        content: "same".to_string(),
                    }),
                    Ok(AgentEvent::Done {
                        output: Some("same".to_string()),
                    }),
                ]);
                Ok(Box::pin(stream))
            }
        }

        let sub_agent = Arc::new(ConstantAgent {
            name: "repeater".to_string(),
        }) as Arc<dyn Agent>;

        let condition: LoopConditionFn = Box::new(|_| true);

        let loop_agent =
            LoopAgent::new("stall-loop", sub_agent, condition).with_soft_limit_override(Some(8));

        let ctx = mock_context();
        let mut stream = loop_agent.run(ctx).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        let failed_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, AgentEvent::Failed { .. }))
            .collect();
        assert_eq!(
            failed_events.len(),
            1,
            "Should have exactly one failed terminal event"
        );
    }

    // ========================================================================
    // Test: output chaining works (N's output -> N+1's input)
    // ========================================================================

    #[tokio::test]
    async fn test_loop_output_chaining() {
        struct ChainingCounterAgent {
            name: String,
        }

        #[async_trait]
        impl Agent for ChainingCounterAgent {
            fn name(&self) -> &str {
                &self.name
            }

            fn description(&self) -> &str {
                "Chaining counter agent"
            }

            async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream> {
                let input_text = ctx.input.as_text();
                let output = format!("{}+step", input_text);

                let mut shared = ctx.shared_state.write().await;
                let count = shared.get("count").and_then(|v| v.as_u64()).unwrap_or(0) + 1;
                shared.insert("count".to_string(), serde_json::json!(count));
                if count >= 3 {
                    shared.insert("loop_continue".to_string(), serde_json::json!(false));
                }
                drop(shared);

                let stream = futures_util::stream::iter(vec![
                    Ok(AgentEvent::TextDelta {
                        content: output.clone(),
                    }),
                    Ok(AgentEvent::Done {
                        output: Some(output),
                    }),
                ]);
                Ok(Box::pin(stream))
            }
        }

        let sub_agent = Arc::new(ChainingCounterAgent {
            name: "chainer".to_string(),
        }) as Arc<dyn Agent>;

        let condition: LoopConditionFn = Box::new(|state| match state.get("loop_continue") {
            Some(v) => is_truthy(v),
            None => true,
        });

        let loop_agent =
            LoopAgent::new("chain-loop", sub_agent, condition).with_soft_limit_override(Some(10));

        let ctx = mock_context();
        let mut stream = loop_agent.run(ctx).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // ChainingCounterAgent appends "+step" on each of 3 iterations.
        if let Some(AgentEvent::Done { output }) = events.last() {
            let out = output.as_ref().unwrap();
            assert_eq!(out, "start+step+step+step");
        } else {
            panic!("Expected Done event at end");
        }
    }

    // ========================================================================
    // Test: StateUpdate prefix is correct
    // ========================================================================

    #[tokio::test]
    async fn test_loop_state_update_prefix() {
        let sub_agent = Arc::new(MockStateAgent::new("my-agent")) as Arc<dyn Agent>;

        let counter = Arc::new(AtomicUsize::new(0));
        let condition_counter = counter.clone();
        let condition: LoopConditionFn =
            Box::new(move |_| condition_counter.fetch_add(1, Ordering::SeqCst) < 2);

        let loop_agent =
            LoopAgent::new("prefix-loop", sub_agent, condition).with_soft_limit_override(Some(10));

        let ctx = mock_context();
        let mut stream = loop_agent.run(ctx).await.unwrap();

        let mut state_updates = vec![];
        while let Some(event) = stream.next().await {
            let event = event.unwrap();
            if let AgentEvent::StateUpdate { ref key, .. } = event {
                state_updates.push(key.clone());
            }
        }

        assert_eq!(state_updates.len(), 2);
        assert_eq!(state_updates[0], "loop.0.my-agent.result");
        assert_eq!(state_updates[1], "loop.1.my-agent.result");
    }

    // ========================================================================
    // Test: condition false before first iteration
    // ========================================================================

    #[tokio::test]
    async fn test_loop_condition_false_initially() {
        let sub_agent = Arc::new(MockAgent::new("never-run", "X")) as Arc<dyn Agent>;

        let condition: LoopConditionFn = Box::new(|_| false);

        let loop_agent =
            LoopAgent::new("no-loop", sub_agent, condition).with_soft_limit_override(Some(10));

        let ctx = mock_context();
        let mut stream = loop_agent.run(ctx).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // Should immediately emit Done with no output
        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::Done { output } => {
                assert!(output.is_none());
            }
            _ => panic!("Expected Done event"),
        }
    }

    // ========================================================================
    // Test: build_loop_condition with truthy/falsy values
    // ========================================================================

    #[test]
    fn test_is_truthy() {
        assert!(!is_truthy(&Value::Null));
        assert!(!is_truthy(&Value::Bool(false)));
        assert!(is_truthy(&Value::Bool(true)));
        assert!(!is_truthy(&serde_json::json!(0)));
        assert!(is_truthy(&serde_json::json!(1)));
        assert!(!is_truthy(&serde_json::json!("")));
        assert!(is_truthy(&serde_json::json!("hello")));
        assert!(!is_truthy(&serde_json::json!([])));
        assert!(is_truthy(&serde_json::json!([1])));
        assert!(!is_truthy(&serde_json::json!({})));
        assert!(is_truthy(&serde_json::json!({"a": 1})));
    }

    #[test]
    fn test_build_loop_condition_key_absent() {
        let condition = build_loop_condition("my_key".to_string());
        let state = HashMap::new();
        assert!(condition(&state));
    }

    #[test]
    fn test_build_loop_condition_key_truthy() {
        let condition = build_loop_condition("my_key".to_string());
        let mut state = HashMap::new();
        state.insert("my_key".to_string(), serde_json::json!(true));
        assert!(condition(&state));
    }

    #[test]
    fn test_build_loop_condition_key_falsy() {
        let condition = build_loop_condition("my_key".to_string());
        let mut state = HashMap::new();

        state.insert("my_key".to_string(), serde_json::json!(false));
        assert!(!condition(&state));

        state.insert("my_key".to_string(), serde_json::json!(0));
        assert!(!condition(&state));

        state.insert("my_key".to_string(), serde_json::json!(""));
        assert!(!condition(&state));

        state.insert("my_key".to_string(), Value::Null);
        assert!(!condition(&state));
    }

    // ========================================================================
    // Test: all sub-agent events forwarded
    // ========================================================================

    #[tokio::test]
    async fn test_loop_forwards_all_sub_agent_events() {
        let sub_agent = Arc::new(MockAgent::new("fwd", "out")) as Arc<dyn Agent>;

        let counter = Arc::new(AtomicUsize::new(0));
        let condition_counter = counter.clone();
        let condition: LoopConditionFn =
            Box::new(move |_| condition_counter.fetch_add(1, Ordering::SeqCst) < 1);

        let loop_agent =
            LoopAgent::new("fwd-loop", sub_agent, condition).with_soft_limit_override(Some(10));

        let ctx = mock_context();
        let mut stream = loop_agent.run(ctx).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // Should have TextDelta + Done
        assert!(events.len() >= 2);
        assert!(matches!(&events[0], AgentEvent::TextDelta { .. }));
        assert!(matches!(events.last().unwrap(), AgentEvent::Done { .. }));
    }

    // ========================================================================
    // Test: single iteration
    // ========================================================================

    #[tokio::test]
    async fn test_loop_single_iteration() {
        let sub_agent = Arc::new(MockAgent::new("once", "result")) as Arc<dyn Agent>;

        let counter = Arc::new(AtomicUsize::new(0));
        let condition_counter = counter.clone();
        let condition: LoopConditionFn =
            Box::new(move |_| condition_counter.fetch_add(1, Ordering::SeqCst) < 1);

        let loop_agent =
            LoopAgent::new("single-loop", sub_agent, condition).with_soft_limit_override(Some(10));

        let ctx = mock_context();
        let mut stream = loop_agent.run(ctx).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        if let Some(AgentEvent::Done { output }) = events.last() {
            assert!(output.is_some());
            assert!(output.as_ref().unwrap().contains("result"));
        } else {
            panic!("Expected Done event");
        }
    }
}
