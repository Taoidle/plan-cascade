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
use crate::services::orchestrator::embedding_manager::EmbeddingManager;
use crate::services::orchestrator::embedding_service::EmbeddingService;
use crate::services::orchestrator::hnsw_index::HnswIndex;
use crate::services::orchestrator::index_store::IndexStore;
use crate::services::tools::executor::ReadCacheEntry;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::task_spawner::TaskContext;
use crate::services::tools::web_fetch::WebFetchService;
use crate::services::tools::web_search::WebSearchService;

/// Context provided to each tool during execution.
///
/// Contains all the shared state that tools need: session info,
/// paths, caches, cancellation support, and service references.
///
/// Designed to carry ALL state that tools need so that tool implementations
/// do not depend on executor-private fields. This is the ADK-inspired
/// "ToolContext" pattern: tools receive everything through context.
pub struct ToolExecutionContext {
    /// Unique session identifier
    pub session_id: String,
    /// Project root directory
    pub project_root: PathBuf,
    /// Current working directory (may differ from project_root after `cd`).
    /// Shared via Arc<Mutex> so Bash tool can update it persistently.
    pub working_directory: Arc<Mutex<PathBuf>>,
    /// Shared read cache for file deduplication across tools and sub-agents.
    /// Key: (canonical path, offset, limit) -> cache entry.
    pub read_cache: Arc<Mutex<HashMap<(PathBuf, usize, usize), ReadCacheEntry>>>,
    /// Set of files that have been read (for read-before-write enforcement)
    pub read_files: Arc<Mutex<std::collections::HashSet<PathBuf>>>,
    /// Cancellation token for cooperative cancellation
    pub cancellation_token: tokio_util::sync::CancellationToken,

    // --- New fields: shared services from executor ---

    /// WebFetch service for fetching web pages (always available)
    pub web_fetch: Arc<WebFetchService>,
    /// WebSearch service (None if no search provider configured)
    pub web_search: Option<Arc<WebSearchService>>,
    /// Optional index store for CodebaseSearch tool
    pub index_store: Option<Arc<IndexStore>>,
    /// Optional embedding service for semantic search in CodebaseSearch
    pub embedding_service: Option<Arc<EmbeddingService>>,
    /// Optional EmbeddingManager for provider-aware semantic search (ADR-F002)
    pub embedding_manager: Option<Arc<EmbeddingManager>>,
    /// Optional HNSW index for O(log n) approximate nearest neighbor search
    pub hnsw_index: Option<Arc<HnswIndex>>,
    /// Task sub-agent deduplication cache.
    /// Keyed by hash of the prompt string. Only successful results are cached.
    pub task_dedup_cache: Arc<Mutex<HashMap<u64, String>>>,
    /// Optional TaskContext for sub-agent spawning.
    /// When None, the Task tool returns a depth-limit error.
    pub task_context: Option<Arc<TaskContext>>,

    /// Optional core-layer context providing memory access.
    /// When set, tools can read from the shared memory store
    /// via `core_context.search_memory(pattern)`.
    pub core_context: Option<plan_cascade_core::context::ToolContext>,
}

impl ToolExecutionContext {
    /// Get the current working directory (snapshot).
    pub fn working_directory_snapshot(&self) -> PathBuf {
        self.working_directory
            .lock()
            .map(|cwd| cwd.clone())
            .unwrap_or_else(|_| self.project_root.clone())
    }
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
    /// Registered toolsets for dynamic refresh
    toolsets: Vec<Arc<dyn Toolset>>,
}

impl ToolRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            order: Vec::new(),
            toolsets: Vec::new(),
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

    /// Register a Toolset. Each tool returned by the toolset is registered
    /// individually. The toolset is also stored for future `refresh_toolsets()`.
    pub fn register_toolset(&mut self, toolset: Arc<dyn Toolset>) {
        let filter_ctx = ToolFilterContext::default();
        let tools: Vec<Arc<dyn Tool>> = toolset.available_tools(&filter_ctx);
        self.toolsets.push(toolset);
        for tool in tools {
            self.register(tool);
        }
    }

    /// Re-evaluate all registered toolsets with a given filter context.
    ///
    /// Removes tools that were previously supplied by toolsets, then
    /// re-queries each toolset with the new context and re-registers
    /// the resulting tools.
    pub fn refresh_toolsets(&mut self, filter_ctx: &ToolFilterContext) {
        // Collect names from all toolsets using default context to find what to remove
        let default_ctx = ToolFilterContext::default();
        let mut toolset_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for ts in &self.toolsets {
            for tool in ts.available_tools(&default_ctx) {
                toolset_names.insert(tool.name().to_string());
            }
        }

        // Remove toolset-provided tools
        for name in &toolset_names {
            self.unregister(name);
        }

        // Collect new tools from toolsets (avoids borrow conflict)
        let new_tools: Vec<Arc<dyn Tool>> = self
            .toolsets
            .iter()
            .flat_map(|ts| ts.available_tools(filter_ctx))
            .collect();

        // Re-register
        for tool in new_tools {
            self.register(tool);
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── FunctionTool ─────────────────────────────────────────────────────

/// Type alias for the async handler function used by `FunctionTool`.
///
/// The handler receives a reference to `ToolExecutionContext` and the
/// JSON arguments, and returns a `ToolResult`.
pub type FunctionToolHandler = Box<
    dyn Fn(&ToolExecutionContext, Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = ToolResult> + Send + '_>>
        + Send
        + Sync,
>;

/// A tool created from an async closure.
///
/// `FunctionTool` allows rapid creation of simple tools without
/// defining a dedicated struct. Useful for one-off tools, plugin-supplied
/// tools, or tools generated at runtime.
///
/// # Example
///
/// ```ignore
/// let tool = FunctionTool::new(
///     "Echo",
///     "Echoes the input",
///     ParameterSchema::object(None, HashMap::new(), vec![]),
///     |_ctx, args| Box::pin(async move {
///         let msg = args.get("message").and_then(|v| v.as_str()).unwrap_or("(empty)");
///         ToolResult::ok(msg)
///     }),
/// );
/// ```
pub struct FunctionTool {
    tool_name: String,
    tool_description: String,
    schema: ParameterSchema,
    handler: FunctionToolHandler,
    long_running: bool,
}

impl FunctionTool {
    /// Create a new FunctionTool from an async closure.
    pub fn new<F>(name: impl Into<String>, description: impl Into<String>, schema: ParameterSchema, handler: F) -> Self
    where
        F: Fn(&ToolExecutionContext, Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = ToolResult> + Send + '_>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            tool_name: name.into(),
            tool_description: description.into(),
            schema,
            handler: Box::new(handler),
            long_running: false,
        }
    }

    /// Mark this tool as long-running.
    pub fn with_long_running(mut self, long_running: bool) -> Self {
        self.long_running = long_running;
        self
    }
}

#[async_trait]
impl Tool for FunctionTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn parameters_schema(&self) -> ParameterSchema {
        self.schema.clone()
    }

    fn is_long_running(&self) -> bool {
        self.long_running
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        (self.handler)(ctx, args).await
    }
}

// ── Toolset ──────────────────────────────────────────────────────────

/// Context for filtering which tools are available.
///
/// Passed to `Toolset::available_tools()` so toolsets can dynamically
/// decide which tools to expose based on project type, execution phase,
/// or skill configuration.
#[derive(Debug, Clone, Default)]
pub struct ToolFilterContext {
    /// Project type (e.g., "rust", "python", "nodejs")
    pub project_type: Option<String>,
    /// Current execution phase (e.g., "planning", "implementation", "review")
    pub execution_phase: Option<String>,
    /// Tools explicitly allowed by a skill configuration
    pub skill_allowed_tools: Option<Vec<String>>,
}

/// A collection of tools that can be dynamically filtered.
///
/// Toolsets allow groups of related tools to be registered/unregistered
/// together and to vary their available tools based on context. For
/// example, a "WebToolset" might expose WebFetch + WebSearch only when
/// a search provider is configured.
///
/// Inspired by adk-rust's `Toolset` pattern.
pub trait Toolset: Send + Sync {
    /// Return the tools available in this toolset given the current filter context.
    fn available_tools(&self, ctx: &ToolFilterContext) -> Vec<Arc<dyn Tool>>;

    /// Human-readable name for this toolset.
    fn name(&self) -> &str;
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
            working_directory: Arc::new(Mutex::new(PathBuf::from("/tmp/test"))),
            read_cache: Arc::new(Mutex::new(HashMap::new())),
            read_files: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            web_fetch: Arc::new(crate::services::tools::web_fetch::WebFetchService::new()),
            web_search: None,
            index_store: None,
            embedding_service: None,
            embedding_manager: None,
            hnsw_index: None,
            task_dedup_cache: Arc::new(Mutex::new(HashMap::new())),
            task_context: None,
            core_context: None,
        }
    }

    #[test]
    fn test_context_has_all_shared_state_fields() {
        let ctx = make_test_context();
        assert_eq!(ctx.session_id, "test-session");
        assert_eq!(ctx.project_root, PathBuf::from("/tmp/test"));
        assert_eq!(ctx.working_directory_snapshot(), PathBuf::from("/tmp/test"));
        // Verify service fields are accessible
        assert!(ctx.web_search.is_none());
        assert!(ctx.index_store.is_none());
        assert!(ctx.embedding_service.is_none());
        assert!(ctx.embedding_manager.is_none());
        assert!(ctx.hnsw_index.is_none());
        assert!(ctx.task_context.is_none());
        // web_fetch is always present
        assert!(Arc::strong_count(&ctx.web_fetch) >= 1);
        // task_dedup_cache is empty
        assert!(ctx.task_dedup_cache.lock().unwrap().is_empty());
    }

    #[test]
    fn test_context_working_directory_mutable() {
        let ctx = make_test_context();
        // Verify working_directory can be updated through Arc<Mutex>
        {
            let mut cwd = ctx.working_directory.lock().unwrap();
            *cwd = PathBuf::from("/tmp/new-dir");
        }
        assert_eq!(ctx.working_directory_snapshot(), PathBuf::from("/tmp/new-dir"));
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

    // ── FunctionTool tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_function_tool_create_and_execute() {
        let tool = FunctionTool::new(
            "Echo",
            "Echoes the message",
            ParameterSchema::object(
                Some("Echo parameters"),
                {
                    let mut props = HashMap::new();
                    props.insert(
                        "message".to_string(),
                        ParameterSchema::string(Some("The message to echo")),
                    );
                    props
                },
                vec!["message".to_string()],
            ),
            |_ctx, args| {
                Box::pin(async move {
                    let msg = args
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(empty)");
                    ToolResult::ok(format!("echo: {}", msg))
                })
            },
        );

        assert_eq!(tool.name(), "Echo");
        assert_eq!(tool.description(), "Echoes the message");
        assert!(!tool.is_long_running());

        let ctx = make_test_context();
        let args = serde_json::json!({"message": "hello"});
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        assert_eq!(result.output.unwrap(), "echo: hello");
    }

    #[tokio::test]
    async fn test_function_tool_long_running() {
        let tool = FunctionTool::new(
            "Slow",
            "A slow tool",
            ParameterSchema::object(None, HashMap::new(), vec![]),
            |_ctx, _args| Box::pin(async move { ToolResult::ok("done") }),
        )
        .with_long_running(true);

        assert!(tool.is_long_running());
    }

    #[tokio::test]
    async fn test_function_tool_in_registry() {
        let mut registry = ToolRegistry::new();
        let tool = Arc::new(FunctionTool::new(
            "Counter",
            "Returns a count",
            ParameterSchema::object(None, HashMap::new(), vec![]),
            |_ctx, _args| Box::pin(async move { ToolResult::ok("42") }),
        ));
        registry.register(tool);

        assert_eq!(registry.len(), 1);
        let defs = registry.definitions();
        assert_eq!(defs[0].name, "Counter");

        let ctx = make_test_context();
        let result = registry.execute("Counter", &ctx, Value::Null).await;
        assert!(result.success);
        assert_eq!(result.output.unwrap(), "42");
    }

    // ── ToolFilterContext tests ──────────────────────────────────────

    #[test]
    fn test_tool_filter_context_default() {
        let ctx = ToolFilterContext::default();
        assert!(ctx.project_type.is_none());
        assert!(ctx.execution_phase.is_none());
        assert!(ctx.skill_allowed_tools.is_none());
    }

    #[test]
    fn test_tool_filter_context_with_values() {
        let ctx = ToolFilterContext {
            project_type: Some("rust".to_string()),
            execution_phase: Some("implementation".to_string()),
            skill_allowed_tools: Some(vec!["Read".to_string(), "Write".to_string()]),
        };
        assert_eq!(ctx.project_type.as_deref(), Some("rust"));
        assert_eq!(ctx.execution_phase.as_deref(), Some("implementation"));
        assert_eq!(ctx.skill_allowed_tools.as_ref().unwrap().len(), 2);
    }

    // ── Toolset tests ───────────────────────────────────────────────

    /// Mock toolset that returns different tools based on project type
    struct MockToolset {
        toolset_name: String,
    }

    impl MockToolset {
        fn new(name: &str) -> Self {
            Self {
                toolset_name: name.to_string(),
            }
        }
    }

    impl Toolset for MockToolset {
        fn name(&self) -> &str {
            &self.toolset_name
        }

        fn available_tools(&self, ctx: &ToolFilterContext) -> Vec<Arc<dyn Tool>> {
            let mut tools: Vec<Arc<dyn Tool>> = vec![
                Arc::new(MockTool::new("ToolsetA", "Toolset tool A")),
            ];
            // Only include ToolsetB for "rust" projects
            if ctx.project_type.as_deref() == Some("rust") {
                tools.push(Arc::new(MockTool::new("ToolsetB", "Toolset tool B (rust only)")));
            }
            tools
        }
    }

    #[test]
    fn test_toolset_register() {
        let mut registry = ToolRegistry::new();
        let toolset = Arc::new(MockToolset::new("TestToolset"));
        registry.register_toolset(toolset);

        // Default context: no project_type, so only ToolsetA
        assert!(registry.get("ToolsetA").is_some());
        assert!(registry.get("ToolsetB").is_none());
    }

    #[test]
    fn test_toolset_refresh_with_filter() {
        let mut registry = ToolRegistry::new();
        // Add a non-toolset tool first
        registry.register(Arc::new(MockTool::new("Static", "A static tool")));

        let toolset = Arc::new(MockToolset::new("TestToolset"));
        registry.register_toolset(toolset);

        // Initially: Static + ToolsetA
        assert!(registry.get("Static").is_some());
        assert!(registry.get("ToolsetA").is_some());
        assert!(registry.get("ToolsetB").is_none());

        // Refresh with rust project type
        let filter = ToolFilterContext {
            project_type: Some("rust".to_string()),
            execution_phase: None,
            skill_allowed_tools: None,
        };
        registry.refresh_toolsets(&filter);

        // Now: Static + ToolsetA + ToolsetB
        assert!(registry.get("Static").is_some());
        assert!(registry.get("ToolsetA").is_some());
        assert!(registry.get("ToolsetB").is_some());
    }

    #[tokio::test]
    async fn test_toolset_tools_execute() {
        let mut registry = ToolRegistry::new();
        let toolset = Arc::new(MockToolset::new("TestToolset"));
        registry.register_toolset(toolset);

        let ctx = make_test_context();
        let result = registry.execute("ToolsetA", &ctx, Value::Null).await;
        assert!(result.success);
        assert_eq!(result.output.unwrap(), "ToolsetA executed");
    }
}
