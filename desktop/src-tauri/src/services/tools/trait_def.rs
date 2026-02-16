//! Tool Trait and Registry
//!
//! Defines the unified `Tool` trait interface and `ToolRegistry` for
//! dynamic tool registration, lookup, and execution. This replaces the
//! hardcoded match statement in executor.rs with a trait-based,
//! registry-driven architecture.
//!
//! Inspired by adk-rust's `Tool` trait + `Toolset` pattern.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::services::llm::types::{ParameterSchema, ToolDefinition};
use crate::services::tools::executor::ReadCacheEntry;
use crate::services::tools::executor::ToolResult;

/// Context provided to each tool during execution.
///
/// Contains all the shared state that tools need: session info,
/// paths, caches, and cancellation support.
pub struct ToolExecutionContext {
    /// Unique session identifier
    pub session_id: String,
    /// Project root directory
    pub project_root: PathBuf,
    /// Current working directory (may differ from project_root after `cd`)
    pub working_directory: PathBuf,
    /// Shared read cache for file deduplication across tools and sub-agents.
    /// Key: (canonical path, offset, limit) -> cache entry.
    pub read_cache: Arc<Mutex<HashMap<(PathBuf, usize, usize), ReadCacheEntry>>>,
    /// Set of files that have been read (for read-before-write enforcement)
    pub read_files: Arc<Mutex<std::collections::HashSet<PathBuf>>>,
    /// Cancellation token for cooperative cancellation
    pub cancellation_token: tokio_util::sync::CancellationToken,
}

/// Unified tool interface.
///
/// Each tool in the system implements this trait, providing:
/// - Identity (name, description, parameters schema)
/// - Execution logic
/// - Optional long-running flag
///
/// Tools are registered in a `ToolRegistry` and dispatched dynamically.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique name of this tool (e.g., "Read", "Bash", "Grep")
    fn name(&self) -> &str;

    /// Human-readable description of what this tool does
    fn description(&self) -> &str;

    /// JSON schema describing the tool's input parameters
    fn parameters_schema(&self) -> ParameterSchema;

    /// Whether this tool is potentially long-running (e.g., Bash with timeout).
    /// Default: false.
    fn is_long_running(&self) -> bool {
        false
    }

    /// Execute the tool with the given context and arguments.
    ///
    /// Returns a `ToolResult` indicating success/failure with output or error.
    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult;
}

/// Registry of available tools.
///
/// Provides O(1) lookup by name, dynamic registration/unregistration,
/// and bulk operations like generating all tool definitions.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    /// Insertion order for deterministic iteration
    order: Vec<String>,
}

impl ToolRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Register a tool. If a tool with the same name already exists, it is replaced.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        if !self.tools.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.tools.insert(name, tool);
    }

    /// Unregister a tool by name. Returns the removed tool, or None if not found.
    pub fn unregister(&mut self, name: &str) -> Option<Arc<dyn Tool>> {
        self.order.retain(|n| n != name);
        self.tools.remove(name)
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Get all tool definitions, suitable for sending to LLM providers.
    /// Returned in registration order.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.order
            .iter()
            .filter_map(|name| self.tools.get(name))
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.parameters_schema(),
            })
            .collect()
    }

    /// Get all registered tool names in registration order.
    pub fn names(&self) -> Vec<String> {
        self.order.clone()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Execute a tool by name with the given context and arguments.
    ///
    /// Returns `ToolResult::err` if the tool is not found.
    pub async fn execute(
        &self,
        name: &str,
        ctx: &ToolExecutionContext,
        args: Value,
    ) -> ToolResult {
        match self.tools.get(name) {
            Some(tool) => tool.execute(ctx, args).await,
            None => ToolResult::err(format!("Unknown tool: {}", name)),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple mock tool for testing the registry
    struct MockTool {
        tool_name: String,
        tool_description: String,
    }

    impl MockTool {
        fn new(name: &str, description: &str) -> Self {
            Self {
                tool_name: name.to_string(),
                tool_description: description.to_string(),
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.tool_name
        }

        fn description(&self) -> &str {
            &self.tool_description
        }

        fn parameters_schema(&self) -> ParameterSchema {
            ParameterSchema::object(
                Some("Mock parameters"),
                HashMap::new(),
                vec![],
            )
        }

        async fn execute(&self, _ctx: &ToolExecutionContext, _args: Value) -> ToolResult {
            ToolResult::ok(format!("{} executed", self.tool_name))
        }
    }

    /// A long-running mock tool
    struct LongRunningMockTool;

    #[async_trait]
    impl Tool for LongRunningMockTool {
        fn name(&self) -> &str {
            "LongRunning"
        }

        fn description(&self) -> &str {
            "A long-running mock tool"
        }

        fn parameters_schema(&self) -> ParameterSchema {
            ParameterSchema::object(None, HashMap::new(), vec![])
        }

        fn is_long_running(&self) -> bool {
            true
        }

        async fn execute(&self, _ctx: &ToolExecutionContext, _args: Value) -> ToolResult {
            ToolResult::ok("done")
        }
    }

    fn make_test_context() -> ToolExecutionContext {
        ToolExecutionContext {
            session_id: "test-session".to_string(),
            project_root: PathBuf::from("/tmp/test"),
            working_directory: PathBuf::from("/tmp/test"),
            read_cache: Arc::new(Mutex::new(HashMap::new())),
            read_files: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    #[test]
    fn test_registry_new_is_empty() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.names().is_empty());
        assert!(registry.definitions().is_empty());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ToolRegistry::new();
        let tool = Arc::new(MockTool::new("Read", "Read a file"));
        registry.register(tool);

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let retrieved = registry.get("Read");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name(), "Read");
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let registry = ToolRegistry::new();
        assert!(registry.get("Nonexistent").is_none());
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(MockTool::new("Read", "Read a file")));
        registry.register(Arc::new(MockTool::new("Write", "Write a file")));

        assert_eq!(registry.len(), 2);

        let removed = registry.unregister("Read");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name(), "Read");
        assert_eq!(registry.len(), 1);
        assert!(registry.get("Read").is_none());
        assert!(registry.get("Write").is_some());
    }

    #[test]
    fn test_registry_unregister_nonexistent() {
        let mut registry = ToolRegistry::new();
        let removed = registry.unregister("Nonexistent");
        assert!(removed.is_none());
    }

    #[test]
    fn test_registry_register_replaces_existing() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(MockTool::new("Read", "Old description")));
        registry.register(Arc::new(MockTool::new("Read", "New description")));

        assert_eq!(registry.len(), 1);
        let tool = registry.get("Read").unwrap();
        assert_eq!(tool.description(), "New description");
    }

    #[test]
    fn test_registry_names_preserves_insertion_order() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(MockTool::new("Bash", "Execute commands")));
        registry.register(Arc::new(MockTool::new("Read", "Read files")));
        registry.register(Arc::new(MockTool::new("Write", "Write files")));

        let names = registry.names();
        assert_eq!(names, vec!["Bash", "Read", "Write"]);
    }

    #[test]
    fn test_registry_definitions() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(MockTool::new("Read", "Read a file")));
        registry.register(Arc::new(MockTool::new("Write", "Write a file")));

        let defs = registry.definitions();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].name, "Read");
        assert_eq!(defs[0].description, "Read a file");
        assert_eq!(defs[1].name, "Write");
        assert_eq!(defs[1].description, "Write a file");
    }

    #[tokio::test]
    async fn test_registry_execute_known_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(MockTool::new("Read", "Read a file")));

        let ctx = make_test_context();
        let result = registry.execute("Read", &ctx, Value::Null).await;
        assert!(result.success);
        assert_eq!(result.output.unwrap(), "Read executed");
    }

    #[tokio::test]
    async fn test_registry_execute_unknown_tool() {
        let registry = ToolRegistry::new();

        let ctx = make_test_context();
        let result = registry.execute("Unknown", &ctx, Value::Null).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Unknown tool"));
    }

    #[test]
    fn test_tool_is_long_running_default() {
        let tool = MockTool::new("Quick", "A quick tool");
        assert!(!tool.is_long_running());
    }

    #[test]
    fn test_tool_is_long_running_override() {
        let tool = LongRunningMockTool;
        assert!(tool.is_long_running());
    }

    #[test]
    fn test_registry_default() {
        let registry = ToolRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_definitions_order_matches_names() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(MockTool::new("C", "Third")));
        registry.register(Arc::new(MockTool::new("A", "First")));
        registry.register(Arc::new(MockTool::new("B", "Second")));

        let names = registry.names();
        let defs = registry.definitions();

        assert_eq!(names.len(), defs.len());
        for (name, def) in names.iter().zip(defs.iter()) {
            assert_eq!(name, &def.name);
        }
    }

    #[test]
    fn test_registry_unregister_preserves_other_order() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(MockTool::new("A", "a")));
        registry.register(Arc::new(MockTool::new("B", "b")));
        registry.register(Arc::new(MockTool::new("C", "c")));

        registry.unregister("B");
        let names = registry.names();
        assert_eq!(names, vec!["A", "C"]);
    }
}
