//! Plugin Lifecycle Hooks Extension
//!
//! Extends the plugin system with execution-level lifecycle hooks that plugins
//! can use to inject context, intercept messages, filter events, and perform
//! cleanup during agent execution.
//!
//! ## Hook Points
//!
//! 1. `before_execution` - Called before agent execution starts. Plugins can
//!    inject context or abort execution.
//! 2. `on_message` - Called when a message is processed. Plugins can modify
//!    or filter messages.
//! 3. `on_event` - Called for each AgentEvent. Plugins can filter or enrich events.
//! 4. `after_execution` - Called after agent execution completes. Used for cleanup.
//!
//! ## Short-Circuit Behavior
//!
//! Hooks execute sequentially. If a hook returns an error with short-circuit
//! enabled, subsequent hooks in the chain are skipped.

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::services::agent_composer::types::AgentEvent;

// ============================================================================
// Lifecycle Context
// ============================================================================

/// Context passed to lifecycle hooks during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleContext {
    /// Session identifier.
    pub session_id: String,
    /// Pipeline or workflow identifier.
    pub execution_id: String,
    /// Name of the agent being executed.
    pub agent_name: String,
    /// Optional plugin-specific data.
    #[serde(default)]
    pub plugin_data: Option<Value>,
}

// ============================================================================
// Hook Results
// ============================================================================

/// Result from a `before_execution` hook.
#[derive(Debug, Clone)]
pub struct BeforeExecutionResult {
    /// If true, abort execution.
    pub abort: bool,
    /// Reason for aborting (if abort is true).
    pub abort_reason: Option<String>,
    /// Optional context to inject into the execution.
    pub injected_context: Option<String>,
}

impl Default for BeforeExecutionResult {
    fn default() -> Self {
        Self {
            abort: false,
            abort_reason: None,
            injected_context: None,
        }
    }
}

/// Result from an `on_message` hook.
#[derive(Debug, Clone)]
pub struct MessageHookResult {
    /// If Some, the message is replaced with this value.
    pub modified_message: Option<String>,
    /// If true, the message is dropped entirely.
    pub drop_message: bool,
}

impl Default for MessageHookResult {
    fn default() -> Self {
        Self {
            modified_message: None,
            drop_message: false,
        }
    }
}

/// Result from an `on_event` hook.
#[derive(Debug, Clone)]
pub struct EventHookResult {
    /// If true, the event is filtered out (not forwarded).
    pub filter_out: bool,
    /// Optional modified event to replace the original.
    pub modified_event: Option<AgentEvent>,
}

impl Default for EventHookResult {
    fn default() -> Self {
        Self {
            filter_out: false,
            modified_event: None,
        }
    }
}

/// Result from an `after_execution` hook.
#[derive(Debug, Clone)]
pub struct AfterExecutionResult {
    /// Summary or notes from the plugin about the execution.
    pub notes: Option<String>,
}

impl Default for AfterExecutionResult {
    fn default() -> Self {
        Self { notes: None }
    }
}

// ============================================================================
// Hook Type Aliases
// ============================================================================

/// Hook called before execution starts.
pub type BeforeExecutionHook = Box<
    dyn Fn(
            LifecycleContext,
        ) -> Pin<Box<dyn Future<Output = Result<BeforeExecutionResult, String>> + Send>>
        + Send
        + Sync,
>;

/// Hook called when a message is processed.
pub type OnMessageHook = Box<
    dyn Fn(
            LifecycleContext,
            String,
        ) -> Pin<Box<dyn Future<Output = Result<MessageHookResult, String>> + Send>>
        + Send
        + Sync,
>;

/// Hook called for each AgentEvent.
pub type OnEventHook = Box<
    dyn Fn(
            LifecycleContext,
            AgentEvent,
        ) -> Pin<Box<dyn Future<Output = Result<EventHookResult, String>> + Send>>
        + Send
        + Sync,
>;

/// Hook called after execution completes.
pub type AfterExecutionHook = Box<
    dyn Fn(
            LifecycleContext,
            bool,
        ) -> Pin<Box<dyn Future<Output = Result<AfterExecutionResult, String>> + Send>>
        + Send
        + Sync,
>;

// ============================================================================
// PluginLifecycleHooks
// ============================================================================

/// Registry of plugin lifecycle hooks for agent execution.
///
/// Hooks execute sequentially in registration order with optional
/// short-circuit behavior on errors.
pub struct PluginLifecycleHooks {
    before_execution: Vec<BeforeExecutionHook>,
    on_message: Vec<OnMessageHook>,
    on_event: Vec<OnEventHook>,
    after_execution: Vec<AfterExecutionHook>,
    /// If true, an error in a hook prevents subsequent hooks from running.
    short_circuit_on_error: bool,
}

impl std::fmt::Debug for PluginLifecycleHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginLifecycleHooks")
            .field("before_execution", &self.before_execution.len())
            .field("on_message", &self.on_message.len())
            .field("on_event", &self.on_event.len())
            .field("after_execution", &self.after_execution.len())
            .field("short_circuit_on_error", &self.short_circuit_on_error)
            .finish()
    }
}

impl PluginLifecycleHooks {
    /// Create a new empty lifecycle hooks registry.
    pub fn new() -> Self {
        Self {
            before_execution: Vec::new(),
            on_message: Vec::new(),
            on_event: Vec::new(),
            after_execution: Vec::new(),
            short_circuit_on_error: false,
        }
    }

    /// Create a new registry with short-circuit behavior enabled.
    pub fn with_short_circuit() -> Self {
        Self {
            before_execution: Vec::new(),
            on_message: Vec::new(),
            on_event: Vec::new(),
            after_execution: Vec::new(),
            short_circuit_on_error: true,
        }
    }

    /// Returns true if short-circuit behavior is enabled.
    pub fn is_short_circuit(&self) -> bool {
        self.short_circuit_on_error
    }

    /// Set the short-circuit behavior.
    pub fn set_short_circuit(&mut self, enabled: bool) {
        self.short_circuit_on_error = enabled;
    }

    /// Returns true if no hooks are registered.
    pub fn is_empty(&self) -> bool {
        self.before_execution.is_empty()
            && self.on_message.is_empty()
            && self.on_event.is_empty()
            && self.after_execution.is_empty()
    }

    /// Total number of registered hooks.
    pub fn total_hooks(&self) -> usize {
        self.before_execution.len()
            + self.on_message.len()
            + self.on_event.len()
            + self.after_execution.len()
    }

    // ========================================================================
    // Registration Methods
    // ========================================================================

    /// Register a before_execution hook.
    pub fn register_before_execution(&mut self, hook: BeforeExecutionHook) {
        self.before_execution.push(hook);
    }

    /// Register an on_message hook.
    pub fn register_on_message(&mut self, hook: OnMessageHook) {
        self.on_message.push(hook);
    }

    /// Register an on_event hook.
    pub fn register_on_event(&mut self, hook: OnEventHook) {
        self.on_event.push(hook);
    }

    /// Register an after_execution hook.
    pub fn register_after_execution(&mut self, hook: AfterExecutionHook) {
        self.after_execution.push(hook);
    }

    // ========================================================================
    // Fire Methods
    // ========================================================================

    /// Fire all before_execution hooks sequentially.
    ///
    /// Returns the combined result. If any hook returns `abort: true`,
    /// execution should be aborted. Injected contexts from all hooks
    /// are concatenated.
    ///
    /// With short-circuit enabled, an error in a hook stops further hooks.
    pub async fn fire_before_execution(
        &self,
        ctx: &LifecycleContext,
    ) -> Result<BeforeExecutionResult, String> {
        let mut combined = BeforeExecutionResult::default();
        let mut injected_parts: Vec<String> = Vec::new();

        for (i, hook) in self.before_execution.iter().enumerate() {
            match hook(ctx.clone()).await {
                Ok(result) => {
                    if result.abort {
                        return Ok(BeforeExecutionResult {
                            abort: true,
                            abort_reason: result.abort_reason,
                            injected_context: if injected_parts.is_empty() {
                                None
                            } else {
                                Some(injected_parts.join("\n"))
                            },
                        });
                    }
                    if let Some(ref injected) = result.injected_context {
                        injected_parts.push(injected.clone());
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[lifecycle] before_execution hook {} failed: {}",
                        i, e
                    );
                    if self.short_circuit_on_error {
                        return Err(e);
                    }
                }
            }
        }

        if !injected_parts.is_empty() {
            combined.injected_context = Some(injected_parts.join("\n"));
        }

        Ok(combined)
    }

    /// Fire all on_message hooks sequentially.
    ///
    /// Each hook can modify or drop the message. If a hook drops the message,
    /// subsequent hooks are not called.
    pub async fn fire_on_message(
        &self,
        ctx: &LifecycleContext,
        message: String,
    ) -> Result<Option<String>, String> {
        let mut current_message = message;

        for (i, hook) in self.on_message.iter().enumerate() {
            match hook(ctx.clone(), current_message.clone()).await {
                Ok(result) => {
                    if result.drop_message {
                        return Ok(None);
                    }
                    if let Some(modified) = result.modified_message {
                        current_message = modified;
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[lifecycle] on_message hook {} failed: {}",
                        i, e
                    );
                    if self.short_circuit_on_error {
                        return Err(e);
                    }
                }
            }
        }

        Ok(Some(current_message))
    }

    /// Fire all on_event hooks sequentially.
    ///
    /// Each hook can filter out or modify the event. If a hook filters out
    /// the event, subsequent hooks are not called.
    pub async fn fire_on_event(
        &self,
        ctx: &LifecycleContext,
        event: AgentEvent,
    ) -> Result<Option<AgentEvent>, String> {
        let mut current_event = event;

        for (i, hook) in self.on_event.iter().enumerate() {
            match hook(ctx.clone(), current_event.clone()).await {
                Ok(result) => {
                    if result.filter_out {
                        return Ok(None);
                    }
                    if let Some(modified) = result.modified_event {
                        current_event = modified;
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[lifecycle] on_event hook {} failed: {}",
                        i, e
                    );
                    if self.short_circuit_on_error {
                        return Err(e);
                    }
                }
            }
        }

        Ok(Some(current_event))
    }

    /// Fire all after_execution hooks sequentially.
    ///
    /// Hooks receive the success status of the execution.
    pub async fn fire_after_execution(
        &self,
        ctx: &LifecycleContext,
        success: bool,
    ) -> Vec<AfterExecutionResult> {
        let mut results = Vec::new();

        for (i, hook) in self.after_execution.iter().enumerate() {
            match hook(ctx.clone(), success).await {
                Ok(result) => {
                    results.push(result);
                }
                Err(e) => {
                    eprintln!(
                        "[lifecycle] after_execution hook {} failed: {}",
                        i, e
                    );
                    if self.short_circuit_on_error {
                        break;
                    }
                }
            }
        }

        results
    }
}

impl Default for PluginLifecycleHooks {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lifecycle_context() -> LifecycleContext {
        LifecycleContext {
            session_id: "test-session".to_string(),
            execution_id: "exec-1".to_string(),
            agent_name: "test-agent".to_string(),
            plugin_data: None,
        }
    }

    // ========================================================================
    // LifecycleContext Tests
    // ========================================================================

    #[test]
    fn test_lifecycle_context_serialization() {
        let ctx = LifecycleContext {
            session_id: "s1".to_string(),
            execution_id: "e1".to_string(),
            agent_name: "agent-1".to_string(),
            plugin_data: Some(serde_json::json!({"key": "value"})),
        };

        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: LifecycleContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id, "s1");
        assert_eq!(parsed.execution_id, "e1");
        assert_eq!(parsed.agent_name, "agent-1");
        assert!(parsed.plugin_data.is_some());
    }

    #[test]
    fn test_lifecycle_context_no_plugin_data() {
        let json = r#"{"session_id": "s1", "execution_id": "e1", "agent_name": "a1"}"#;
        let ctx: LifecycleContext = serde_json::from_str(json).unwrap();
        assert!(ctx.plugin_data.is_none());
    }

    // ========================================================================
    // Result Defaults Tests
    // ========================================================================

    #[test]
    fn test_before_execution_result_default() {
        let result = BeforeExecutionResult::default();
        assert!(!result.abort);
        assert!(result.abort_reason.is_none());
        assert!(result.injected_context.is_none());
    }

    #[test]
    fn test_message_hook_result_default() {
        let result = MessageHookResult::default();
        assert!(result.modified_message.is_none());
        assert!(!result.drop_message);
    }

    #[test]
    fn test_event_hook_result_default() {
        let result = EventHookResult::default();
        assert!(!result.filter_out);
        assert!(result.modified_event.is_none());
    }

    #[test]
    fn test_after_execution_result_default() {
        let result = AfterExecutionResult::default();
        assert!(result.notes.is_none());
    }

    // ========================================================================
    // PluginLifecycleHooks Construction Tests
    // ========================================================================

    #[test]
    fn test_lifecycle_hooks_new() {
        let hooks = PluginLifecycleHooks::new();
        assert!(hooks.is_empty());
        assert_eq!(hooks.total_hooks(), 0);
        assert!(!hooks.is_short_circuit());
    }

    #[test]
    fn test_lifecycle_hooks_with_short_circuit() {
        let hooks = PluginLifecycleHooks::with_short_circuit();
        assert!(hooks.is_short_circuit());
    }

    #[test]
    fn test_lifecycle_hooks_default() {
        let hooks = PluginLifecycleHooks::default();
        assert!(hooks.is_empty());
        assert!(!hooks.is_short_circuit());
    }

    #[test]
    fn test_lifecycle_hooks_set_short_circuit() {
        let mut hooks = PluginLifecycleHooks::new();
        assert!(!hooks.is_short_circuit());
        hooks.set_short_circuit(true);
        assert!(hooks.is_short_circuit());
        hooks.set_short_circuit(false);
        assert!(!hooks.is_short_circuit());
    }

    #[test]
    fn test_lifecycle_hooks_debug() {
        let hooks = PluginLifecycleHooks::new();
        let debug = format!("{:?}", hooks);
        assert!(debug.contains("PluginLifecycleHooks"));
        assert!(debug.contains("before_execution: 0"));
    }

    #[test]
    fn test_lifecycle_hooks_registration_counts() {
        let mut hooks = PluginLifecycleHooks::new();
        assert_eq!(hooks.total_hooks(), 0);

        hooks.register_before_execution(Box::new(|_ctx| {
            Box::pin(async { Ok(BeforeExecutionResult::default()) })
        }));
        assert_eq!(hooks.total_hooks(), 1);

        hooks.register_on_message(Box::new(|_ctx, _msg| {
            Box::pin(async { Ok(MessageHookResult::default()) })
        }));
        assert_eq!(hooks.total_hooks(), 2);

        hooks.register_on_event(Box::new(|_ctx, _evt| {
            Box::pin(async { Ok(EventHookResult::default()) })
        }));
        assert_eq!(hooks.total_hooks(), 3);

        hooks.register_after_execution(Box::new(|_ctx, _success| {
            Box::pin(async { Ok(AfterExecutionResult::default()) })
        }));
        assert_eq!(hooks.total_hooks(), 4);

        assert!(!hooks.is_empty());
    }

    // ========================================================================
    // fire_before_execution Tests
    // ========================================================================

    #[tokio::test]
    async fn test_fire_before_execution_no_hooks() {
        let hooks = PluginLifecycleHooks::new();
        let ctx = test_lifecycle_context();
        let result = hooks.fire_before_execution(&ctx).await.unwrap();
        assert!(!result.abort);
        assert!(result.injected_context.is_none());
    }

    #[tokio::test]
    async fn test_fire_before_execution_inject_context() {
        let mut hooks = PluginLifecycleHooks::new();
        hooks.register_before_execution(Box::new(|_ctx| {
            Box::pin(async {
                Ok(BeforeExecutionResult {
                    abort: false,
                    abort_reason: None,
                    injected_context: Some("Use TypeScript strict mode".to_string()),
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let result = hooks.fire_before_execution(&ctx).await.unwrap();
        assert!(!result.abort);
        assert_eq!(
            result.injected_context.as_deref(),
            Some("Use TypeScript strict mode")
        );
    }

    #[tokio::test]
    async fn test_fire_before_execution_abort() {
        let mut hooks = PluginLifecycleHooks::new();
        hooks.register_before_execution(Box::new(|_ctx| {
            Box::pin(async {
                Ok(BeforeExecutionResult {
                    abort: true,
                    abort_reason: Some("Security policy violation".to_string()),
                    injected_context: None,
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let result = hooks.fire_before_execution(&ctx).await.unwrap();
        assert!(result.abort);
        assert_eq!(
            result.abort_reason.as_deref(),
            Some("Security policy violation")
        );
    }

    #[tokio::test]
    async fn test_fire_before_execution_abort_stops_chain() {
        let mut hooks = PluginLifecycleHooks::new();

        // First hook injects context
        hooks.register_before_execution(Box::new(|_ctx| {
            Box::pin(async {
                Ok(BeforeExecutionResult {
                    abort: false,
                    abort_reason: None,
                    injected_context: Some("context-1".to_string()),
                })
            })
        }));

        // Second hook aborts
        hooks.register_before_execution(Box::new(|_ctx| {
            Box::pin(async {
                Ok(BeforeExecutionResult {
                    abort: true,
                    abort_reason: Some("blocked".to_string()),
                    injected_context: None,
                })
            })
        }));

        // Third hook should not run
        hooks.register_before_execution(Box::new(|_ctx| {
            Box::pin(async {
                Ok(BeforeExecutionResult {
                    abort: false,
                    abort_reason: None,
                    injected_context: Some("context-3".to_string()),
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let result = hooks.fire_before_execution(&ctx).await.unwrap();
        assert!(result.abort);
        // Context from first hook should be preserved
        assert_eq!(result.injected_context.as_deref(), Some("context-1"));
    }

    #[tokio::test]
    async fn test_fire_before_execution_error_no_short_circuit() {
        let mut hooks = PluginLifecycleHooks::new();

        // First hook fails
        hooks.register_before_execution(Box::new(|_ctx| {
            Box::pin(async { Err("hook 1 failed".to_string()) })
        }));

        // Second hook succeeds
        hooks.register_before_execution(Box::new(|_ctx| {
            Box::pin(async {
                Ok(BeforeExecutionResult {
                    abort: false,
                    abort_reason: None,
                    injected_context: Some("from-hook-2".to_string()),
                })
            })
        }));

        let ctx = test_lifecycle_context();
        // Without short-circuit, should continue to second hook
        let result = hooks.fire_before_execution(&ctx).await.unwrap();
        assert!(!result.abort);
        assert_eq!(result.injected_context.as_deref(), Some("from-hook-2"));
    }

    #[tokio::test]
    async fn test_fire_before_execution_error_with_short_circuit() {
        let mut hooks = PluginLifecycleHooks::with_short_circuit();

        // First hook fails
        hooks.register_before_execution(Box::new(|_ctx| {
            Box::pin(async { Err("hook 1 failed".to_string()) })
        }));

        // Second hook should not run
        hooks.register_before_execution(Box::new(|_ctx| {
            Box::pin(async {
                Ok(BeforeExecutionResult {
                    abort: false,
                    abort_reason: None,
                    injected_context: Some("should not appear".to_string()),
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let result = hooks.fire_before_execution(&ctx).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "hook 1 failed");
    }

    // ========================================================================
    // fire_on_message Tests
    // ========================================================================

    #[tokio::test]
    async fn test_fire_on_message_no_hooks() {
        let hooks = PluginLifecycleHooks::new();
        let ctx = test_lifecycle_context();
        let result = hooks
            .fire_on_message(&ctx, "hello".to_string())
            .await
            .unwrap();
        assert_eq!(result, Some("hello".to_string()));
    }

    #[tokio::test]
    async fn test_fire_on_message_modify() {
        let mut hooks = PluginLifecycleHooks::new();
        hooks.register_on_message(Box::new(|_ctx, msg| {
            Box::pin(async move {
                Ok(MessageHookResult {
                    modified_message: Some(format!("[enhanced] {}", msg)),
                    drop_message: false,
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let result = hooks
            .fire_on_message(&ctx, "hello".to_string())
            .await
            .unwrap();
        assert_eq!(result, Some("[enhanced] hello".to_string()));
    }

    #[tokio::test]
    async fn test_fire_on_message_drop() {
        let mut hooks = PluginLifecycleHooks::new();
        hooks.register_on_message(Box::new(|_ctx, _msg| {
            Box::pin(async {
                Ok(MessageHookResult {
                    modified_message: None,
                    drop_message: true,
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let result = hooks
            .fire_on_message(&ctx, "hello".to_string())
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_fire_on_message_chain_modification() {
        let mut hooks = PluginLifecycleHooks::new();

        hooks.register_on_message(Box::new(|_ctx, msg| {
            Box::pin(async move {
                Ok(MessageHookResult {
                    modified_message: Some(format!("[hook1] {}", msg)),
                    drop_message: false,
                })
            })
        }));

        hooks.register_on_message(Box::new(|_ctx, msg| {
            Box::pin(async move {
                Ok(MessageHookResult {
                    modified_message: Some(format!("[hook2] {}", msg)),
                    drop_message: false,
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let result = hooks
            .fire_on_message(&ctx, "hello".to_string())
            .await
            .unwrap();
        assert_eq!(result, Some("[hook2] [hook1] hello".to_string()));
    }

    #[tokio::test]
    async fn test_fire_on_message_drop_stops_chain() {
        let mut hooks = PluginLifecycleHooks::new();

        // First hook drops
        hooks.register_on_message(Box::new(|_ctx, _msg| {
            Box::pin(async {
                Ok(MessageHookResult {
                    modified_message: None,
                    drop_message: true,
                })
            })
        }));

        // Second hook should not run
        hooks.register_on_message(Box::new(|_ctx, msg| {
            Box::pin(async move {
                Ok(MessageHookResult {
                    modified_message: Some(format!("[should not appear] {}", msg)),
                    drop_message: false,
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let result = hooks
            .fire_on_message(&ctx, "hello".to_string())
            .await
            .unwrap();
        assert!(result.is_none());
    }

    // ========================================================================
    // fire_on_event Tests
    // ========================================================================

    #[tokio::test]
    async fn test_fire_on_event_no_hooks() {
        let hooks = PluginLifecycleHooks::new();
        let ctx = test_lifecycle_context();
        let event = AgentEvent::TextDelta {
            content: "hello".to_string(),
        };
        let result = hooks.fire_on_event(&ctx, event).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_fire_on_event_filter_out() {
        let mut hooks = PluginLifecycleHooks::new();
        hooks.register_on_event(Box::new(|_ctx, event| {
            Box::pin(async move {
                // Filter out thinking deltas
                match &event {
                    AgentEvent::ThinkingDelta { .. } => Ok(EventHookResult {
                        filter_out: true,
                        modified_event: None,
                    }),
                    _ => Ok(EventHookResult::default()),
                }
            })
        }));

        let ctx = test_lifecycle_context();

        // TextDelta should pass through
        let text_event = AgentEvent::TextDelta {
            content: "hello".to_string(),
        };
        let result = hooks.fire_on_event(&ctx, text_event).await.unwrap();
        assert!(result.is_some());

        // ThinkingDelta should be filtered
        let thinking_event = AgentEvent::ThinkingDelta {
            content: "thinking...".to_string(),
        };
        let result = hooks.fire_on_event(&ctx, thinking_event).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_fire_on_event_modify() {
        let mut hooks = PluginLifecycleHooks::new();
        hooks.register_on_event(Box::new(|_ctx, _event| {
            Box::pin(async {
                Ok(EventHookResult {
                    filter_out: false,
                    modified_event: Some(AgentEvent::TextDelta {
                        content: "modified".to_string(),
                    }),
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let event = AgentEvent::TextDelta {
            content: "original".to_string(),
        };
        let result = hooks.fire_on_event(&ctx, event).await.unwrap();
        assert!(result.is_some());
        match result.unwrap() {
            AgentEvent::TextDelta { content } => assert_eq!(content, "modified"),
            _ => panic!("Expected TextDelta"),
        }
    }

    // ========================================================================
    // fire_after_execution Tests
    // ========================================================================

    #[tokio::test]
    async fn test_fire_after_execution_no_hooks() {
        let hooks = PluginLifecycleHooks::new();
        let ctx = test_lifecycle_context();
        let results = hooks.fire_after_execution(&ctx, true).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_fire_after_execution_with_notes() {
        let mut hooks = PluginLifecycleHooks::new();
        hooks.register_after_execution(Box::new(|_ctx, success| {
            Box::pin(async move {
                Ok(AfterExecutionResult {
                    notes: Some(format!("Execution success: {}", success)),
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let results = hooks.fire_after_execution(&ctx, true).await;
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].notes.as_deref(),
            Some("Execution success: true")
        );
    }

    #[tokio::test]
    async fn test_fire_after_execution_multiple_hooks() {
        let mut hooks = PluginLifecycleHooks::new();

        hooks.register_after_execution(Box::new(|_ctx, _success| {
            Box::pin(async {
                Ok(AfterExecutionResult {
                    notes: Some("hook-1 done".to_string()),
                })
            })
        }));

        hooks.register_after_execution(Box::new(|_ctx, _success| {
            Box::pin(async {
                Ok(AfterExecutionResult {
                    notes: Some("hook-2 done".to_string()),
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let results = hooks.fire_after_execution(&ctx, false).await;
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_fire_after_execution_error_no_short_circuit() {
        let mut hooks = PluginLifecycleHooks::new();

        hooks.register_after_execution(Box::new(|_ctx, _success| {
            Box::pin(async { Err("hook 1 failed".to_string()) })
        }));

        hooks.register_after_execution(Box::new(|_ctx, _success| {
            Box::pin(async {
                Ok(AfterExecutionResult {
                    notes: Some("hook-2 ok".to_string()),
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let results = hooks.fire_after_execution(&ctx, true).await;
        // Without short-circuit, hook-2 should still run
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].notes.as_deref(), Some("hook-2 ok"));
    }

    #[tokio::test]
    async fn test_fire_after_execution_error_with_short_circuit() {
        let mut hooks = PluginLifecycleHooks::with_short_circuit();

        hooks.register_after_execution(Box::new(|_ctx, _success| {
            Box::pin(async { Err("hook 1 failed".to_string()) })
        }));

        hooks.register_after_execution(Box::new(|_ctx, _success| {
            Box::pin(async {
                Ok(AfterExecutionResult {
                    notes: Some("should not run".to_string()),
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let results = hooks.fire_after_execution(&ctx, true).await;
        // With short-circuit, hook-2 should not run
        assert!(results.is_empty());
    }

    // ========================================================================
    // Short-circuit behavior across hook types
    // ========================================================================

    #[tokio::test]
    async fn test_on_message_short_circuit() {
        let mut hooks = PluginLifecycleHooks::with_short_circuit();

        hooks.register_on_message(Box::new(|_ctx, _msg| {
            Box::pin(async { Err("blocked".to_string()) })
        }));

        hooks.register_on_message(Box::new(|_ctx, msg| {
            Box::pin(async move {
                Ok(MessageHookResult {
                    modified_message: Some(format!("[should not run] {}", msg)),
                    drop_message: false,
                })
            })
        }));

        let ctx = test_lifecycle_context();
        let result = hooks
            .fire_on_message(&ctx, "hello".to_string())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_on_event_short_circuit() {
        let mut hooks = PluginLifecycleHooks::with_short_circuit();

        hooks.register_on_event(Box::new(|_ctx, _event| {
            Box::pin(async { Err("event processing failed".to_string()) })
        }));

        let ctx = test_lifecycle_context();
        let event = AgentEvent::TextDelta {
            content: "test".to_string(),
        };
        let result = hooks.fire_on_event(&ctx, event).await;
        assert!(result.is_err());
    }
}
