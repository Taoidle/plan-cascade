//! Context Hierarchy
//!
//! Defines the three-level context hierarchy for the architecture:
//!
//! 1. `ExecutionContext` trait - Base immutable context shared across all scopes
//! 2. `ToolContext` - Concrete struct for tool-level execution (extends ExecutionContext)
//! 3. `OrchestratorContext` - Concrete struct for orchestrator-level control (extends ExecutionContext)
//!
//! This hierarchy enforces separation of concerns at compile time:
//! - Tools only see `ToolContext` (no session mutation, no execution control)
//! - Orchestrators see `OrchestratorContext` (full session control)
//! - Both share the immutable `ExecutionContext` base

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde_json::Value;

use crate::error::{CoreError, CoreResult};

// ============================================================================
// ExecutionContext Trait
// ============================================================================

/// Base execution context trait providing immutable, shared session information.
///
/// All execution scopes (tool, orchestrator, agent) share this base context.
/// It provides read-only access to fundamental session properties.
///
/// # Design Rationale
/// By defining this as a trait, we enable:
/// - Compile-time enforcement of read-only access in tool implementations
/// - Easy mocking in tests
/// - Clear API boundaries between core and higher-level crates
pub trait ExecutionContext: Send + Sync {
    /// Returns the unique session identifier for this execution.
    fn session_id(&self) -> &str;

    /// Returns the project root directory path.
    fn project_root(&self) -> &Path;

    /// Returns the name of the currently executing agent.
    fn agent_name(&self) -> &str;

    /// Returns an optional execution tag for categorization (e.g., "chat", "task", "analysis").
    fn execution_tag(&self) -> Option<&str> {
        None
    }
}

// ============================================================================
// ToolContext
// ============================================================================

/// Context for tool-level execution.
///
/// Extends `ExecutionContext` with tool-specific capabilities:
/// - Tool call identification
/// - Read-only memory search
///
/// Tools receive a `ToolContext` and CANNOT mutate session state or control
/// execution flow. This is enforced at compile time.
pub struct ToolContext {
    session_id: String,
    project_root: PathBuf,
    agent_name: String,
    execution_tag: Option<String>,
    /// Unique identifier for this specific tool call.
    tool_call_id: String,
    /// Shared read-only memory store for semantic search across the session.
    /// Tools can read from memory but not write to it.
    memory_store: Arc<RwLock<HashMap<String, Value>>>,
}

impl ToolContext {
    /// Create a new ToolContext.
    pub fn new(
        session_id: impl Into<String>,
        project_root: impl Into<PathBuf>,
        agent_name: impl Into<String>,
        tool_call_id: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            project_root: project_root.into(),
            agent_name: agent_name.into(),
            execution_tag: None,
            tool_call_id: tool_call_id.into(),
            memory_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set the execution tag.
    pub fn with_execution_tag(mut self, tag: impl Into<String>) -> Self {
        self.execution_tag = Some(tag.into());
        self
    }

    /// Set the memory store (shared with orchestrator).
    pub fn with_memory_store(mut self, store: Arc<RwLock<HashMap<String, Value>>>) -> Self {
        self.memory_store = store;
        self
    }

    /// Returns the unique tool call identifier.
    pub fn tool_call_id(&self) -> &str {
        &self.tool_call_id
    }

    /// Search the memory store for entries matching the given key pattern.
    ///
    /// Returns matching key-value pairs. This is a read-only operation.
    pub fn search_memory(&self, key_pattern: &str) -> Vec<(String, Value)> {
        let store = self.memory_store.read().unwrap_or_else(|e| e.into_inner());
        store
            .iter()
            .filter(|(k, _)| k.contains(key_pattern))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

impl ExecutionContext for ToolContext {
    fn session_id(&self) -> &str {
        &self.session_id
    }

    fn project_root(&self) -> &Path {
        &self.project_root
    }

    fn agent_name(&self) -> &str {
        &self.agent_name
    }

    fn execution_tag(&self) -> Option<&str> {
        self.execution_tag.as_deref()
    }
}

// ============================================================================
// OrchestratorContext
// ============================================================================

/// Context for orchestrator-level execution control.
///
/// Extends `ExecutionContext` with orchestrator-specific capabilities:
/// - Mutable session state access
/// - Execution lifecycle control (end execution)
/// - Memory store management
///
/// Only the orchestrator layer receives this context, allowing it to control
/// the execution lifecycle and manage shared state.
pub struct OrchestratorContext {
    session_id: String,
    project_root: PathBuf,
    agent_name: String,
    execution_tag: Option<String>,
    /// Mutable session state shared across the orchestrator scope.
    session_state: Arc<RwLock<HashMap<String, Value>>>,
    /// Memory store (shared with tool contexts created from this orchestrator context).
    memory_store: Arc<RwLock<HashMap<String, Value>>>,
    /// Flag indicating whether execution should be terminated.
    should_end: Arc<RwLock<bool>>,
}

impl OrchestratorContext {
    /// Create a new OrchestratorContext.
    pub fn new(
        session_id: impl Into<String>,
        project_root: impl Into<PathBuf>,
        agent_name: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            project_root: project_root.into(),
            agent_name: agent_name.into(),
            execution_tag: None,
            session_state: Arc::new(RwLock::new(HashMap::new())),
            memory_store: Arc::new(RwLock::new(HashMap::new())),
            should_end: Arc::new(RwLock::new(false)),
        }
    }

    /// Set the execution tag.
    pub fn with_execution_tag(mut self, tag: impl Into<String>) -> Self {
        self.execution_tag = Some(tag.into());
        self
    }

    /// Get a mutable reference to the session state.
    ///
    /// Allows the orchestrator to read and write session state entries.
    pub fn session_mut(&self) -> CoreResult<std::sync::RwLockWriteGuard<'_, HashMap<String, Value>>> {
        self.session_state
            .write()
            .map_err(|e| CoreError::internal(format!("Session state lock poisoned: {}", e)))
    }

    /// Get a read reference to the session state.
    pub fn session_ref(&self) -> CoreResult<std::sync::RwLockReadGuard<'_, HashMap<String, Value>>> {
        self.session_state
            .read()
            .map_err(|e| CoreError::internal(format!("Session state lock poisoned: {}", e)))
    }

    /// Signal that execution should end.
    pub fn end_execution(&self) {
        if let Ok(mut should_end) = self.should_end.write() {
            *should_end = true;
        }
    }

    /// Check if execution has been signaled to end.
    pub fn should_end(&self) -> bool {
        self.should_end
            .read()
            .map(|v| *v)
            .unwrap_or(false)
    }

    /// Create a `ToolContext` from this orchestrator context for a specific tool call.
    ///
    /// The created ToolContext shares the same memory store but has no access
    /// to session state mutation or execution control.
    pub fn create_tool_context(&self, tool_call_id: impl Into<String>) -> ToolContext {
        ToolContext {
            session_id: self.session_id.clone(),
            project_root: self.project_root.clone(),
            agent_name: self.agent_name.clone(),
            execution_tag: self.execution_tag.clone(),
            tool_call_id: tool_call_id.into(),
            memory_store: Arc::clone(&self.memory_store),
        }
    }

    /// Write a value to the memory store (shared with tool contexts).
    pub fn set_memory(&self, key: impl Into<String>, value: Value) -> CoreResult<()> {
        let mut store = self.memory_store.write().map_err(|e| {
            CoreError::internal(format!("Memory store lock poisoned: {}", e))
        })?;
        store.insert(key.into(), value);
        Ok(())
    }

    /// Read a value from the memory store.
    pub fn get_memory(&self, key: &str) -> Option<Value> {
        let store = self.memory_store.read().ok()?;
        store.get(key).cloned()
    }
}

impl ExecutionContext for OrchestratorContext {
    fn session_id(&self) -> &str {
        &self.session_id
    }

    fn project_root(&self) -> &Path {
        &self.project_root
    }

    fn agent_name(&self) -> &str {
        &self.agent_name
    }

    fn execution_tag(&self) -> Option<&str> {
        self.execution_tag.as_deref()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- ExecutionContext trait tests --

    #[test]
    fn test_tool_context_implements_execution_context() {
        let ctx = ToolContext::new("sess-1", "/project", "test-agent", "tc-001");
        assert_eq!(ctx.session_id(), "sess-1");
        assert_eq!(ctx.project_root(), Path::new("/project"));
        assert_eq!(ctx.agent_name(), "test-agent");
        assert_eq!(ctx.execution_tag(), None);
    }

    #[test]
    fn test_tool_context_with_execution_tag() {
        let ctx = ToolContext::new("sess-1", "/project", "agent", "tc-001")
            .with_execution_tag("chat");
        assert_eq!(ctx.execution_tag(), Some("chat"));
    }

    #[test]
    fn test_orchestrator_context_implements_execution_context() {
        let ctx = OrchestratorContext::new("sess-2", "/project", "orchestrator-agent");
        assert_eq!(ctx.session_id(), "sess-2");
        assert_eq!(ctx.project_root(), Path::new("/project"));
        assert_eq!(ctx.agent_name(), "orchestrator-agent");
        assert_eq!(ctx.execution_tag(), None);
    }

    #[test]
    fn test_orchestrator_context_with_execution_tag() {
        let ctx = OrchestratorContext::new("sess-2", "/project", "agent")
            .with_execution_tag("task");
        assert_eq!(ctx.execution_tag(), Some("task"));
    }

    // -- ToolContext tests --

    #[test]
    fn test_tool_context_tool_call_id() {
        let ctx = ToolContext::new("sess-1", "/project", "agent", "tc-42");
        assert_eq!(ctx.tool_call_id(), "tc-42");
    }

    #[test]
    fn test_tool_context_search_memory_empty() {
        let ctx = ToolContext::new("sess-1", "/project", "agent", "tc-1");
        let results = ctx.search_memory("any-key");
        assert!(results.is_empty());
    }

    #[test]
    fn test_tool_context_search_memory_with_data() {
        let store = Arc::new(RwLock::new(HashMap::new()));
        {
            let mut s = store.write().unwrap();
            s.insert("file:main.rs".to_string(), Value::String("content".to_string()));
            s.insert("file:lib.rs".to_string(), Value::String("lib content".to_string()));
            s.insert("meta:version".to_string(), Value::String("1.0".to_string()));
        }

        let ctx = ToolContext::new("sess-1", "/project", "agent", "tc-1")
            .with_memory_store(store);

        let results = ctx.search_memory("file:");
        assert_eq!(results.len(), 2);

        let results = ctx.search_memory("meta:");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "meta:version");

        let results = ctx.search_memory("nonexistent");
        assert!(results.is_empty());
    }

    // -- OrchestratorContext tests --

    #[test]
    fn test_orchestrator_session_mut() {
        let ctx = OrchestratorContext::new("sess-1", "/project", "agent");

        // Write to session
        {
            let mut state = ctx.session_mut().unwrap();
            state.insert("key1".to_string(), Value::String("value1".to_string()));
        }

        // Read from session
        {
            let state = ctx.session_ref().unwrap();
            assert_eq!(
                state.get("key1"),
                Some(&Value::String("value1".to_string()))
            );
        }
    }

    #[test]
    fn test_orchestrator_end_execution() {
        let ctx = OrchestratorContext::new("sess-1", "/project", "agent");

        assert!(!ctx.should_end());
        ctx.end_execution();
        assert!(ctx.should_end());
    }

    #[test]
    fn test_orchestrator_create_tool_context() {
        let ctx = OrchestratorContext::new("sess-1", "/project", "my-agent")
            .with_execution_tag("chat");

        // Set memory in orchestrator
        ctx.set_memory("shared-key", Value::Bool(true)).unwrap();

        // Create tool context
        let tool_ctx = ctx.create_tool_context("tc-100");
        assert_eq!(tool_ctx.session_id(), "sess-1");
        assert_eq!(tool_ctx.project_root(), Path::new("/project"));
        assert_eq!(tool_ctx.agent_name(), "my-agent");
        assert_eq!(tool_ctx.execution_tag(), Some("chat"));
        assert_eq!(tool_ctx.tool_call_id(), "tc-100");

        // Tool context can read the shared memory
        let results = tool_ctx.search_memory("shared");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "shared-key");
    }

    #[test]
    fn test_orchestrator_memory_operations() {
        let ctx = OrchestratorContext::new("sess-1", "/project", "agent");

        // Set memory
        ctx.set_memory("key1", Value::Number(42.into())).unwrap();
        ctx.set_memory("key2", Value::String("hello".to_string())).unwrap();

        // Get memory
        assert_eq!(ctx.get_memory("key1"), Some(Value::Number(42.into())));
        assert_eq!(ctx.get_memory("key2"), Some(Value::String("hello".to_string())));
        assert_eq!(ctx.get_memory("nonexistent"), None);
    }

    #[test]
    fn test_orchestrator_memory_shared_with_tool_context() {
        let ctx = OrchestratorContext::new("sess-1", "/project", "agent");
        ctx.set_memory("shared_data", Value::Bool(true)).unwrap();

        let tool_ctx = ctx.create_tool_context("tc-1");
        let results = tool_ctx.search_memory("shared_data");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Value::Bool(true));
    }

    // -- Trait object tests --

    #[test]
    fn test_execution_context_trait_object_from_tool_context() {
        let ctx = ToolContext::new("sess-1", "/project", "agent", "tc-1");
        let trait_obj: &dyn ExecutionContext = &ctx;
        assert_eq!(trait_obj.session_id(), "sess-1");
        assert_eq!(trait_obj.project_root(), Path::new("/project"));
        assert_eq!(trait_obj.agent_name(), "agent");
    }

    #[test]
    fn test_execution_context_trait_object_from_orchestrator_context() {
        let ctx = OrchestratorContext::new("sess-2", "/other", "orch");
        let trait_obj: &dyn ExecutionContext = &ctx;
        assert_eq!(trait_obj.session_id(), "sess-2");
        assert_eq!(trait_obj.project_root(), Path::new("/other"));
        assert_eq!(trait_obj.agent_name(), "orch");
    }

    #[test]
    fn test_contexts_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ToolContext>();
        assert_send_sync::<OrchestratorContext>();
    }
}
