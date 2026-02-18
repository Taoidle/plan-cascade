//! Unified Tool Trait
//!
//! Defines the core-layer tool abstraction with split definition/execution traits:
//!
//! - `ToolDefinitionTrait` - Identity, schema, permissions, metadata
//! - `ToolExecutable` - Execution capability
//! - `UnifiedTool` - Combined trait (auto-implemented via blanket impl)
//! - `UnifiedToolRegistry` - O(1) lookup registry with ordered iteration
//!
//! This is the architectural foundation that the existing `services::tools::trait_def::Tool`
//! trait will eventually migrate to. The split design enables:
//! - Schema-only consumers (LLM prompt builders) to avoid execution dependencies
//! - MCP tool proxies to separate schema from RPC execution
//! - Clean test doubles with independent definition/execution mocking

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::{CoreError, CoreResult};
use crate::context::ToolContext;

// ============================================================================
// Trait Definitions
// ============================================================================

/// Tool definition metadata trait.
///
/// Provides identity and schema information about a tool without
/// requiring execution capability. Separating definition from execution
/// allows the registry to enumerate tools without instantiating executors.
pub trait ToolDefinitionTrait: Send + Sync {
    /// Unique name of this tool (e.g., "Read", "Bash", "Grep").
    fn name(&self) -> &str;

    /// Human-readable description of what this tool does.
    fn description(&self) -> &str;

    /// JSON schema describing input parameters.
    ///
    /// Should conform to JSON Schema draft-07. Example:
    /// ```json
    /// {
    ///   "type": "object",
    ///   "properties": {
    ///     "path": { "type": "string", "description": "File path to read" }
    ///   },
    ///   "required": ["path"]
    /// }
    /// ```
    fn parameters_schema(&self) -> Value;

    /// Permissions required by this tool.
    ///
    /// Examples: `"filesystem:read"`, `"filesystem:write"`, `"shell:execute"`,
    /// `"network:fetch"`. Used by guardrails and permission checking.
    fn required_permissions(&self) -> Vec<String> {
        vec![]
    }

    /// Whether this tool is potentially long-running.
    ///
    /// Long-running tools (e.g., Bash with timeout, web fetch) should
    /// be subject to timeout enforcement and progress reporting.
    fn is_long_running(&self) -> bool {
        false
    }
}

/// Tool execution trait.
///
/// Provides the execution capability for a tool. Separated from
/// `ToolDefinitionTrait` so that definition-only consumers (e.g.,
/// schema generation) don't need to depend on execution infrastructure.
#[async_trait]
pub trait ToolExecutable: Send + Sync {
    /// Execute the tool with the given context and arguments.
    ///
    /// # Arguments
    /// - `ctx` - The tool execution context (session info, memory access)
    /// - `args` - JSON arguments matching the tool's `parameters_schema()`
    ///
    /// # Returns
    /// - `Ok(Value)` - The tool's output as a JSON value
    /// - `Err(CoreError)` - If the tool execution failed
    async fn execute(&self, ctx: &ToolContext, args: Value) -> CoreResult<Value>;
}

/// Combined trait for tools that provide both definition and execution.
///
/// Most tools implement this combined trait. The separation into
/// `ToolDefinitionTrait` + `ToolExecutable` is useful for:
/// - MCP tool proxies (definition from schema, execution via RPC)
/// - Test doubles (mock execution, real definition)
/// - Schema-only consumers (LLM prompt generation)
pub trait UnifiedTool: ToolDefinitionTrait + ToolExecutable {}

// Blanket implementation: anything that implements both traits is a UnifiedTool
impl<T: ToolDefinitionTrait + ToolExecutable> UnifiedTool for T {}

// ============================================================================
// UnifiedToolRegistry
// ============================================================================

/// Registry for `UnifiedTool` implementations.
///
/// Provides O(1) lookup by name, ordered iteration, and dynamic
/// registration/unregistration. This is the core-layer counterpart
/// of the existing `ToolRegistry` in `services::tools::trait_def`.
///
/// Key differences from the existing `ToolRegistry`:
/// - Uses `ToolContext` from the core context hierarchy (not `ToolExecutionContext`)
/// - Returns `CoreResult<Value>` instead of `ToolResult` struct
/// - Supports permission checking via `required_permissions()`
pub struct UnifiedToolRegistry {
    tools: HashMap<String, Arc<dyn UnifiedTool>>,
    /// Insertion order for deterministic iteration.
    order: Vec<String>,
}

impl UnifiedToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Register a tool. Replaces any existing tool with the same name.
    pub fn register(&mut self, tool: Arc<dyn UnifiedTool>) {
        let name = tool.name().to_string();
        if !self.tools.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.tools.insert(name, tool);
    }

    /// Unregister a tool by name. Returns the removed tool, or None.
    pub fn unregister(&mut self, name: &str) -> Option<Arc<dyn UnifiedTool>> {
        self.order.retain(|n| n != name);
        self.tools.remove(name)
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn UnifiedTool>> {
        self.tools.get(name).cloned()
    }

    /// Check if a tool is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get all tool names in registration order.
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

    /// Get tool definitions as JSON values in registration order.
    ///
    /// Suitable for sending to LLM providers or generating documentation.
    pub fn definitions(&self) -> Vec<Value> {
        self.order
            .iter()
            .filter_map(|name| self.tools.get(name))
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "parameters": tool.parameters_schema(),
                    "required_permissions": tool.required_permissions(),
                    "is_long_running": tool.is_long_running(),
                })
            })
            .collect()
    }

    /// Get all tools that require a specific permission.
    pub fn tools_with_permission(&self, permission: &str) -> Vec<String> {
        self.order
            .iter()
            .filter(|name| {
                self.tools
                    .get(*name)
                    .map(|t| t.required_permissions().iter().any(|p| p == permission))
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Get all long-running tools.
    pub fn long_running_tools(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|name| {
                self.tools
                    .get(*name)
                    .map(|t| t.is_long_running())
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Execute a tool by name.
    ///
    /// Returns `Err(CoreError::NotFound)` if the tool is not registered.
    pub async fn execute(
        &self,
        name: &str,
        ctx: &ToolContext,
        args: Value,
    ) -> CoreResult<Value> {
        match self.tools.get(name) {
            Some(tool) => tool.execute(ctx, args).await,
            None => Err(CoreError::not_found(format!("Tool not found: {}", name))),
        }
    }
}

impl Default for UnifiedToolRegistry {
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

    // -- Mock Tool --

    /// A mock tool for testing the unified tool traits and registry.
    struct MockUnifiedTool {
        tool_name: String,
        tool_description: String,
        permissions: Vec<String>,
        long_running: bool,
    }

    impl MockUnifiedTool {
        fn new(name: &str, description: &str) -> Self {
            Self {
                tool_name: name.to_string(),
                tool_description: description.to_string(),
                permissions: vec![],
                long_running: false,
            }
        }

        fn with_permissions(mut self, perms: Vec<&str>) -> Self {
            self.permissions = perms.into_iter().map(String::from).collect();
            self
        }

        fn with_long_running(mut self, lr: bool) -> Self {
            self.long_running = lr;
            self
        }
    }

    impl ToolDefinitionTrait for MockUnifiedTool {
        fn name(&self) -> &str {
            &self.tool_name
        }

        fn description(&self) -> &str {
            &self.tool_description
        }

        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
                },
                "required": ["input"]
            })
        }

        fn required_permissions(&self) -> Vec<String> {
            self.permissions.clone()
        }

        fn is_long_running(&self) -> bool {
            self.long_running
        }
    }

    #[async_trait]
    impl ToolExecutable for MockUnifiedTool {
        async fn execute(&self, _ctx: &ToolContext, args: Value) -> CoreResult<Value> {
            let input = args
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("(none)");
            Ok(Value::String(format!("{}: {}", self.tool_name, input)))
        }
    }

    /// Mock tool that always fails
    struct FailingTool;

    impl ToolDefinitionTrait for FailingTool {
        fn name(&self) -> &str {
            "Failing"
        }

        fn description(&self) -> &str {
            "Always fails"
        }

        fn parameters_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }
    }

    #[async_trait]
    impl ToolExecutable for FailingTool {
        async fn execute(&self, _ctx: &ToolContext, _args: Value) -> CoreResult<Value> {
            Err(CoreError::command("Tool execution failed"))
        }
    }

    fn make_tool_context() -> ToolContext {
        ToolContext::new("test-session", "/tmp/test", "test-agent", "tc-001")
    }

    // -- ToolDefinitionTrait tests --

    #[test]
    fn test_tool_definition_basic() {
        let tool = MockUnifiedTool::new("Read", "Read a file");
        assert_eq!(tool.name(), "Read");
        assert_eq!(tool.description(), "Read a file");
        assert!(tool.parameters_schema().is_object());
    }

    #[test]
    fn test_tool_definition_default_permissions_empty() {
        let tool = MockUnifiedTool::new("Read", "Read a file");
        assert!(tool.required_permissions().is_empty());
    }

    #[test]
    fn test_tool_definition_custom_permissions() {
        let tool = MockUnifiedTool::new("Read", "Read a file")
            .with_permissions(vec!["filesystem:read"]);
        assert_eq!(tool.required_permissions(), vec!["filesystem:read"]);
    }

    #[test]
    fn test_tool_definition_default_not_long_running() {
        let tool = MockUnifiedTool::new("Read", "Read a file");
        assert!(!tool.is_long_running());
    }

    #[test]
    fn test_tool_definition_long_running() {
        let tool = MockUnifiedTool::new("Bash", "Execute commands")
            .with_long_running(true);
        assert!(tool.is_long_running());
    }

    // -- ToolExecutable tests --

    #[tokio::test]
    async fn test_tool_execute_success() {
        let tool = MockUnifiedTool::new("Echo", "Echoes input");
        let ctx = make_tool_context();
        let args = serde_json::json!({"input": "hello"});
        let result = tool.execute(&ctx, args).await.unwrap();
        assert_eq!(result, Value::String("Echo: hello".to_string()));
    }

    #[tokio::test]
    async fn test_tool_execute_failure() {
        let tool = FailingTool;
        let ctx = make_tool_context();
        let result = tool.execute(&ctx, Value::Null).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Tool execution failed"));
    }

    // -- UnifiedTool blanket impl tests --

    #[test]
    fn test_unified_tool_blanket_impl() {
        let tool = MockUnifiedTool::new("Test", "Test tool");
        let _unified: &dyn UnifiedTool = &tool;
        assert_eq!(_unified.name(), "Test");
    }

    #[test]
    fn test_unified_tool_as_trait_object() {
        let tool: Arc<dyn UnifiedTool> = Arc::new(MockUnifiedTool::new("Test", "A test tool"));
        assert_eq!(tool.name(), "Test");
        assert_eq!(tool.description(), "A test tool");
        assert!(!tool.is_long_running());
    }

    // -- UnifiedToolRegistry tests --

    #[test]
    fn test_registry_new_is_empty() {
        let registry = UnifiedToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.names().is_empty());
        assert!(registry.definitions().is_empty());
    }

    #[test]
    fn test_registry_default_is_empty() {
        let registry = UnifiedToolRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = UnifiedToolRegistry::new();
        let tool: Arc<dyn UnifiedTool> = Arc::new(MockUnifiedTool::new("Read", "Read a file"));
        registry.register(tool);

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
        assert!(registry.contains("Read"));

        let retrieved = registry.get("Read");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name(), "Read");
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let registry = UnifiedToolRegistry::new();
        assert!(registry.get("Nonexistent").is_none());
        assert!(!registry.contains("Nonexistent"));
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = UnifiedToolRegistry::new();
        registry.register(Arc::new(MockUnifiedTool::new("Read", "Read")));
        registry.register(Arc::new(MockUnifiedTool::new("Write", "Write")));

        assert_eq!(registry.len(), 2);

        let removed = registry.unregister("Read");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name(), "Read");
        assert_eq!(registry.len(), 1);
        assert!(!registry.contains("Read"));
        assert!(registry.contains("Write"));
    }

    #[test]
    fn test_registry_unregister_nonexistent() {
        let mut registry = UnifiedToolRegistry::new();
        assert!(registry.unregister("Nope").is_none());
    }

    #[test]
    fn test_registry_register_replaces_existing() {
        let mut registry = UnifiedToolRegistry::new();
        registry.register(Arc::new(MockUnifiedTool::new("Read", "Old desc")));
        registry.register(Arc::new(MockUnifiedTool::new("Read", "New desc")));

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.get("Read").unwrap().description(), "New desc");
    }

    #[test]
    fn test_registry_names_preserves_insertion_order() {
        let mut registry = UnifiedToolRegistry::new();
        registry.register(Arc::new(MockUnifiedTool::new("Bash", "Execute")));
        registry.register(Arc::new(MockUnifiedTool::new("Read", "Read")));
        registry.register(Arc::new(MockUnifiedTool::new("Write", "Write")));

        assert_eq!(registry.names(), vec!["Bash", "Read", "Write"]);
    }

    #[test]
    fn test_registry_definitions() {
        let mut registry = UnifiedToolRegistry::new();
        registry.register(Arc::new(
            MockUnifiedTool::new("Read", "Read a file")
                .with_permissions(vec!["filesystem:read"]),
        ));
        registry.register(Arc::new(
            MockUnifiedTool::new("Bash", "Execute commands")
                .with_permissions(vec!["shell:execute"])
                .with_long_running(true),
        ));

        let defs = registry.definitions();
        assert_eq!(defs.len(), 2);

        assert_eq!(defs[0]["name"], "Read");
        assert_eq!(defs[0]["description"], "Read a file");
        assert!(defs[0]["parameters"].is_object());
        assert_eq!(defs[0]["required_permissions"][0], "filesystem:read");
        assert_eq!(defs[0]["is_long_running"], false);

        assert_eq!(defs[1]["name"], "Bash");
        assert_eq!(defs[1]["is_long_running"], true);
    }

    #[test]
    fn test_registry_tools_with_permission() {
        let mut registry = UnifiedToolRegistry::new();
        registry.register(Arc::new(
            MockUnifiedTool::new("Read", "Read")
                .with_permissions(vec!["filesystem:read"]),
        ));
        registry.register(Arc::new(
            MockUnifiedTool::new("Write", "Write")
                .with_permissions(vec!["filesystem:write"]),
        ));
        registry.register(Arc::new(
            MockUnifiedTool::new("Bash", "Execute")
                .with_permissions(vec!["shell:execute", "filesystem:read"]),
        ));
        registry.register(Arc::new(MockUnifiedTool::new("Grep", "Search")));

        let fs_read = registry.tools_with_permission("filesystem:read");
        assert_eq!(fs_read, vec!["Read", "Bash"]);

        let shell = registry.tools_with_permission("shell:execute");
        assert_eq!(shell, vec!["Bash"]);

        let none = registry.tools_with_permission("network:fetch");
        assert!(none.is_empty());
    }

    #[test]
    fn test_registry_long_running_tools() {
        let mut registry = UnifiedToolRegistry::new();
        registry.register(Arc::new(MockUnifiedTool::new("Read", "Read")));
        registry.register(Arc::new(
            MockUnifiedTool::new("Bash", "Execute").with_long_running(true),
        ));
        registry.register(Arc::new(
            MockUnifiedTool::new("WebFetch", "Fetch").with_long_running(true),
        ));

        let lr = registry.long_running_tools();
        assert_eq!(lr, vec!["Bash", "WebFetch"]);
    }

    #[tokio::test]
    async fn test_registry_execute_known_tool() {
        let mut registry = UnifiedToolRegistry::new();
        registry.register(Arc::new(MockUnifiedTool::new("Echo", "Echo input")));

        let ctx = make_tool_context();
        let result = registry
            .execute("Echo", &ctx, serde_json::json!({"input": "test"}))
            .await
            .unwrap();
        assert_eq!(result, Value::String("Echo: test".to_string()));
    }

    #[tokio::test]
    async fn test_registry_execute_unknown_tool() {
        let registry = UnifiedToolRegistry::new();
        let ctx = make_tool_context();
        let result = registry.execute("Unknown", &ctx, Value::Null).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Tool not found: Unknown"));
    }

    #[tokio::test]
    async fn test_registry_execute_failing_tool() {
        let mut registry = UnifiedToolRegistry::new();
        registry.register(Arc::new(FailingTool));

        let ctx = make_tool_context();
        let result = registry.execute("Failing", &ctx, Value::Null).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_unregister_preserves_order() {
        let mut registry = UnifiedToolRegistry::new();
        registry.register(Arc::new(MockUnifiedTool::new("A", "a")));
        registry.register(Arc::new(MockUnifiedTool::new("B", "b")));
        registry.register(Arc::new(MockUnifiedTool::new("C", "c")));

        registry.unregister("B");
        assert_eq!(registry.names(), vec!["A", "C"]);
    }

    #[test]
    fn test_registry_definitions_order_matches_names() {
        let mut registry = UnifiedToolRegistry::new();
        registry.register(Arc::new(MockUnifiedTool::new("C", "third")));
        registry.register(Arc::new(MockUnifiedTool::new("A", "first")));
        registry.register(Arc::new(MockUnifiedTool::new("B", "second")));

        let names = registry.names();
        let defs = registry.definitions();

        assert_eq!(names.len(), defs.len());
        for (name, def) in names.iter().zip(defs.iter()) {
            assert_eq!(name, def["name"].as_str().unwrap());
        }
    }

    // -- Send + Sync assertion tests --

    #[test]
    fn test_traits_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockUnifiedTool>();
        // Arc<dyn UnifiedTool> should be Send + Sync
        fn assert_arc_unified_send_sync<T: Send + Sync>() {}
        assert_arc_unified_send_sync::<Arc<dyn UnifiedTool>>();
    }
}
