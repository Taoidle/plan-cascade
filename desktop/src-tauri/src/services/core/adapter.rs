//! Adapter Module: Core Traits <-> Existing Layer Bridge
//!
//! Provides bidirectional adapters between the new core-layer traits
//! (`UnifiedTool`, `ToolDefinitionTrait`, `ToolExecutable`, `ToolContext`)
//! and the existing implementation-layer traits (`Tool`, `ToolExecutionContext`, `ToolResult`).
//!
//! Architecture: ADR-001 Adapter Pattern
//!
//! - `ToolAdapter` wraps an old `Tool` and exposes it as a `UnifiedTool`
//! - `UnifiedToolAdapter` wraps a `UnifiedTool` and exposes it as an old `Tool`
//! - Conversion utilities handle `ToolContext <-> ToolExecutionContext` mapping
//! - `ParameterSchema <-> serde_json::Value` conversion
//! - `ToolResult <-> AppResult<Value>` conversion

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value;

use plan_cascade_core::context::ToolContext;
use plan_cascade_core::tool_trait::{ToolDefinitionTrait, ToolExecutable, UnifiedTool, UnifiedToolRegistry};
use plan_cascade_core::error::{CoreError, CoreResult};
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext, ToolRegistry};
use crate::services::llm::types::ParameterSchema;
use crate::utils::error::{AppError, AppResult};

// ============================================================================
// Conversion Utilities
// ============================================================================

/// Convert a `ParameterSchema` (old system) to a `serde_json::Value` (new system).
///
/// The new `ToolDefinitionTrait::parameters_schema()` returns `Value`, while the
/// old `Tool::parameters_schema()` returns `ParameterSchema`. This function bridges
/// the two by serializing `ParameterSchema` to JSON.
pub fn parameter_schema_to_value(schema: &ParameterSchema) -> Value {
    serde_json::to_value(schema).unwrap_or_else(|_| {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    })
}

/// Convert a `serde_json::Value` (new system) to a `ParameterSchema` (old system).
///
/// Best-effort conversion: if deserialization fails, returns a minimal object schema.
pub fn value_to_parameter_schema(value: &Value) -> ParameterSchema {
    serde_json::from_value::<ParameterSchema>(value.clone()).unwrap_or_else(|_| {
        ParameterSchema::object(None, HashMap::new(), vec![])
    })
}

/// Convert a `ToolResult` (old system) to `CoreResult<Value>` (new system).
///
/// Maps successful results to `Ok(Value::String(...))` and errors to
/// `Err(CoreError::Command(...))`.
pub fn tool_result_to_core_result(result: ToolResult) -> CoreResult<Value> {
    if result.success {
        Ok(Value::String(
            result.output.unwrap_or_default(),
        ))
    } else {
        Err(CoreError::command(
            result.error.unwrap_or_else(|| "Unknown tool error".to_string()),
        ))
    }
}

/// Convert a `ToolResult` (old system) to `AppResult<Value>` (app-level).
///
/// Maps successful results to `Ok(Value::String(...))` and errors to
/// `Err(AppError::Command(...))`.
pub fn tool_result_to_app_result(result: ToolResult) -> AppResult<Value> {
    if result.success {
        Ok(Value::String(
            result.output.unwrap_or_default(),
        ))
    } else {
        Err(AppError::command(
            result.error.unwrap_or_else(|| "Unknown tool error".to_string()),
        ))
    }
}

/// Convert a `CoreResult<Value>` (new system) to a `ToolResult` (old system).
///
/// Maps `Ok(Value)` to a successful `ToolResult` and `Err(CoreError)` to an error `ToolResult`.
pub fn core_result_to_tool_result(result: CoreResult<Value>) -> ToolResult {
    match result {
        Ok(value) => {
            let output = match value {
                Value::String(s) => s,
                other => other.to_string(),
            };
            ToolResult::ok(output)
        }
        Err(err) => ToolResult::err(err.to_string()),
    }
}

/// Convert an `AppResult<Value>` (app-level) to a `ToolResult` (old system).
///
/// Maps `Ok(Value)` to a successful `ToolResult` and `Err(AppError)` to an error `ToolResult`.
pub fn app_result_to_tool_result(result: AppResult<Value>) -> ToolResult {
    match result {
        Ok(value) => {
            let output = match value {
                Value::String(s) => s,
                other => other.to_string(),
            };
            ToolResult::ok(output)
        }
        Err(err) => ToolResult::err(err.to_string()),
    }
}

/// Create a `ToolContext` (new system) from a `ToolExecutionContext` (old system).
///
/// Maps the common fields: session_id, project_root, and uses sensible defaults
/// for fields not present in the old context (agent_name = "legacy-adapter",
/// tool_call_id = "adapter-call").
pub fn tool_execution_context_to_tool_context(ctx: &ToolExecutionContext) -> ToolContext {
    ToolContext::new(
        &ctx.session_id,
        ctx.project_root.clone(),
        "legacy-adapter",
        "adapter-call",
    )
}

/// Create a `ToolExecutionContext` (old system) from a `ToolContext` (new system).
///
/// Creates a minimal `ToolExecutionContext` with sensible defaults for service
/// references that don't exist in the new context system. The project_root is
/// used as the working directory, and all optional services are set to None.
pub fn tool_context_to_tool_execution_context(ctx: &ToolContext) -> ToolExecutionContext {
    use plan_cascade_core::context::ExecutionContext;

    let project_root = ctx.project_root().to_path_buf();
    ToolExecutionContext {
        session_id: ctx.session_id().to_string(),
        project_root: project_root.clone(),
        working_directory: Arc::new(Mutex::new(project_root)),
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
        file_change_tracker: None,
    }
}

// ============================================================================
// ToolAdapter: Old Tool -> New UnifiedTool
// ============================================================================

/// Adapter that wraps an old `Tool` trait object and presents it as a `UnifiedTool`.
///
/// This enables existing tool implementations to be used in the new core-layer
/// registry and execution system without modification.
///
/// # Conversion Details
/// - `name()`, `description()`, `is_long_running()` are passed through directly
/// - `parameters_schema()` converts `ParameterSchema` -> `serde_json::Value`
/// - `execute()` converts `ToolContext` -> `ToolExecutionContext`, calls the old tool,
///   then converts `ToolResult` -> `AppResult<Value>`
pub struct ToolAdapter {
    inner: Arc<dyn Tool>,
}

impl ToolAdapter {
    /// Create a new ToolAdapter wrapping an old Tool.
    pub fn new(tool: Arc<dyn Tool>) -> Self {
        Self { inner: tool }
    }

    /// Get a reference to the wrapped tool.
    pub fn inner(&self) -> &Arc<dyn Tool> {
        &self.inner
    }
}

impl ToolDefinitionTrait for ToolAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> Value {
        parameter_schema_to_value(&self.inner.parameters_schema())
    }

    fn is_long_running(&self) -> bool {
        self.inner.is_long_running()
    }
}

#[async_trait]
impl ToolExecutable for ToolAdapter {
    async fn execute(&self, ctx: &ToolContext, args: Value) -> CoreResult<Value> {
        let legacy_ctx = tool_context_to_tool_execution_context(ctx);
        let result = self.inner.execute(&legacy_ctx, args).await;
        tool_result_to_core_result(result)
    }
}

// ============================================================================
// UnifiedToolAdapter: New UnifiedTool -> Old Tool
// ============================================================================

/// Adapter that wraps a new `UnifiedTool` trait object and presents it as an old `Tool`.
///
/// This enables new-style tool implementations to be used in the existing
/// tool registry and execution system without modification.
///
/// # Conversion Details
/// - `name()`, `description()`, `is_long_running()` are passed through directly
/// - `parameters_schema()` converts `serde_json::Value` -> `ParameterSchema`
/// - `execute()` converts `ToolExecutionContext` -> `ToolContext`, calls the UnifiedTool,
///   then converts `AppResult<Value>` -> `ToolResult`
pub struct UnifiedToolAdapter {
    inner: Arc<dyn UnifiedTool>,
}

impl UnifiedToolAdapter {
    /// Create a new UnifiedToolAdapter wrapping a UnifiedTool.
    pub fn new(tool: Arc<dyn UnifiedTool>) -> Self {
        Self { inner: tool }
    }

    /// Get a reference to the wrapped tool.
    pub fn inner(&self) -> &Arc<dyn UnifiedTool> {
        &self.inner
    }
}

#[async_trait]
impl Tool for UnifiedToolAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> ParameterSchema {
        value_to_parameter_schema(&self.inner.parameters_schema())
    }

    fn is_long_running(&self) -> bool {
        self.inner.is_long_running()
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let new_ctx = tool_execution_context_to_tool_context(ctx);
        let result = self.inner.execute(&new_ctx, args).await;
        core_result_to_tool_result(result)
    }
}

// ============================================================================
// Registry Import
// ============================================================================

/// Import all tools from an existing `ToolRegistry` into a `UnifiedToolRegistry`
/// by wrapping each in a `ToolAdapter`.
///
/// This allows bulk migration of existing tools to the new registry system.
/// Tools are registered in their original order.
///
/// This is a free function rather than an inherent method because
/// `UnifiedToolRegistry` is defined in the `plan-cascade-core` crate.
pub fn import_legacy_tools(registry: &mut UnifiedToolRegistry, legacy: &ToolRegistry) {
    for name in legacy.names() {
        if let Some(tool) = legacy.get(&name) {
            let adapter = Arc::new(ToolAdapter::new(tool)) as Arc<dyn UnifiedTool>;
            registry.register(adapter);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use plan_cascade_core::context::ExecutionContext;

    // ── Mock Old Tool ──────────────────────────────────────────────────

    struct MockOldTool {
        tool_name: String,
        tool_description: String,
    }

    impl MockOldTool {
        fn new(name: &str, desc: &str) -> Self {
            Self {
                tool_name: name.to_string(),
                tool_description: desc.to_string(),
            }
        }
    }

    #[async_trait]
    impl Tool for MockOldTool {
        fn name(&self) -> &str {
            &self.tool_name
        }

        fn description(&self) -> &str {
            &self.tool_description
        }

        fn parameters_schema(&self) -> ParameterSchema {
            ParameterSchema::object(
                Some("Mock parameters"),
                {
                    let mut props = HashMap::new();
                    props.insert(
                        "input".to_string(),
                        ParameterSchema::string(Some("The input")),
                    );
                    props
                },
                vec!["input".to_string()],
            )
        }

        async fn execute(&self, _ctx: &ToolExecutionContext, args: Value) -> ToolResult {
            let input = args
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("(none)");
            ToolResult::ok(format!("old-{}: {}", self.tool_name, input))
        }
    }

    struct FailingOldTool;

    #[async_trait]
    impl Tool for FailingOldTool {
        fn name(&self) -> &str {
            "FailingOld"
        }

        fn description(&self) -> &str {
            "Always fails"
        }

        fn parameters_schema(&self) -> ParameterSchema {
            ParameterSchema::object(None, HashMap::new(), vec![])
        }

        async fn execute(&self, _ctx: &ToolExecutionContext, _args: Value) -> ToolResult {
            ToolResult::err("old tool failed")
        }
    }

    // ── Mock New UnifiedTool ───────────────────────────────────────────

    struct MockNewTool {
        tool_name: String,
        tool_description: String,
    }

    impl MockNewTool {
        fn new(name: &str, desc: &str) -> Self {
            Self {
                tool_name: name.to_string(),
                tool_description: desc.to_string(),
            }
        }
    }

    impl ToolDefinitionTrait for MockNewTool {
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
                    "input": { "type": "string", "description": "The input" }
                },
                "required": ["input"]
            })
        }

        fn required_permissions(&self) -> Vec<String> {
            vec!["filesystem:read".to_string()]
        }
    }

    #[async_trait]
    impl ToolExecutable for MockNewTool {
        async fn execute(&self, _ctx: &ToolContext, args: Value) -> CoreResult<Value> {
            let input = args
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("(none)");
            Ok(Value::String(format!("new-{}: {}", self.tool_name, input)))
        }
    }

    struct FailingNewTool;

    impl ToolDefinitionTrait for FailingNewTool {
        fn name(&self) -> &str {
            "FailingNew"
        }

        fn description(&self) -> &str {
            "Always fails"
        }

        fn parameters_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }
    }

    #[async_trait]
    impl ToolExecutable for FailingNewTool {
        async fn execute(&self, _ctx: &ToolContext, _args: Value) -> CoreResult<Value> {
            Err(CoreError::command("new tool failed"))
        }
    }

    fn make_tool_context() -> ToolContext {
        ToolContext::new("test-session", "/tmp/test", "test-agent", "tc-001")
    }

    fn make_tool_execution_context() -> ToolExecutionContext {
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
            file_change_tracker: None,
        }
    }

    // ── Conversion utility tests ───────────────────────────────────────

    #[test]
    fn test_parameter_schema_to_value() {
        let schema = ParameterSchema::object(
            Some("Test schema"),
            {
                let mut props = HashMap::new();
                props.insert("name".to_string(), ParameterSchema::string(Some("A name")));
                props
            },
            vec!["name".to_string()],
        );
        let value = parameter_schema_to_value(&schema);
        assert_eq!(value["type"], "object");
        assert!(value["properties"]["name"].is_object());
        assert_eq!(value["required"][0], "name");
    }

    #[test]
    fn test_value_to_parameter_schema() {
        let value = serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path" }
            },
            "required": ["path"]
        });
        let schema = value_to_parameter_schema(&value);
        assert_eq!(schema.schema_type, "object");
        assert!(schema.properties.is_some());
        let props = schema.properties.unwrap();
        assert!(props.contains_key("path"));
    }

    #[test]
    fn test_value_to_parameter_schema_fallback() {
        let value = serde_json::json!("not a schema");
        let schema = value_to_parameter_schema(&value);
        assert_eq!(schema.schema_type, "object");
    }

    #[test]
    fn test_tool_result_to_app_result_success() {
        let result = ToolResult::ok("hello world");
        let app_result = tool_result_to_app_result(result);
        assert!(app_result.is_ok());
        assert_eq!(app_result.unwrap(), Value::String("hello world".to_string()));
    }

    #[test]
    fn test_tool_result_to_app_result_error() {
        let result = ToolResult::err("something failed");
        let app_result = tool_result_to_app_result(result);
        assert!(app_result.is_err());
        let err = app_result.unwrap_err();
        assert!(err.to_string().contains("something failed"));
    }

    #[test]
    fn test_app_result_to_tool_result_success_string() {
        let result: AppResult<Value> = Ok(Value::String("output text".to_string()));
        let tool_result = app_result_to_tool_result(result);
        assert!(tool_result.success);
        assert_eq!(tool_result.output.unwrap(), "output text");
    }

    #[test]
    fn test_app_result_to_tool_result_success_json() {
        let result: AppResult<Value> = Ok(serde_json::json!({"key": "value"}));
        let tool_result = app_result_to_tool_result(result);
        assert!(tool_result.success);
        assert!(tool_result.output.unwrap().contains("key"));
    }

    #[test]
    fn test_app_result_to_tool_result_error() {
        let result: AppResult<Value> = Err(AppError::command("it broke"));
        let tool_result = app_result_to_tool_result(result);
        assert!(!tool_result.success);
        assert!(tool_result.error.unwrap().contains("it broke"));
    }

    #[test]
    fn test_tool_execution_context_to_tool_context() {
        let old_ctx = make_tool_execution_context();
        let new_ctx = tool_execution_context_to_tool_context(&old_ctx);
        assert_eq!(new_ctx.session_id(), "test-session");
        assert_eq!(new_ctx.project_root(), std::path::Path::new("/tmp/test"));
    }

    #[test]
    fn test_tool_context_to_tool_execution_context() {
        let new_ctx = make_tool_context();
        let old_ctx = tool_context_to_tool_execution_context(&new_ctx);
        assert_eq!(old_ctx.session_id, "test-session");
        assert_eq!(old_ctx.project_root, PathBuf::from("/tmp/test"));
        assert_eq!(old_ctx.working_directory_snapshot(), PathBuf::from("/tmp/test"));
        assert!(old_ctx.web_search.is_none());
        assert!(old_ctx.index_store.is_none());
        assert!(old_ctx.task_context.is_none());
    }

    #[test]
    fn test_conversion_roundtrip_preserves_session_and_root() {
        let original_ctx = make_tool_execution_context();
        let new_ctx = tool_execution_context_to_tool_context(&original_ctx);
        let roundtripped = tool_context_to_tool_execution_context(&new_ctx);
        assert_eq!(original_ctx.session_id, roundtripped.session_id);
        assert_eq!(original_ctx.project_root, roundtripped.project_root);
    }

    // ── ToolAdapter tests (old Tool -> new UnifiedTool) ────────────────

    #[test]
    fn test_tool_adapter_definition() {
        let old_tool: Arc<dyn Tool> = Arc::new(MockOldTool::new("Read", "Read a file"));
        let adapter = ToolAdapter::new(old_tool);

        assert_eq!(adapter.name(), "Read");
        assert_eq!(adapter.description(), "Read a file");
        assert!(!adapter.is_long_running());

        let schema = adapter.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["input"].is_object());
    }

    #[test]
    fn test_tool_adapter_is_unified_tool() {
        let old_tool: Arc<dyn Tool> = Arc::new(MockOldTool::new("Read", "Read a file"));
        let adapter = ToolAdapter::new(old_tool);
        // Verify it implements UnifiedTool via blanket impl
        let _unified: &dyn UnifiedTool = &adapter;
        assert_eq!(_unified.name(), "Read");
    }

    #[test]
    fn test_tool_adapter_as_arc_unified() {
        let old_tool: Arc<dyn Tool> = Arc::new(MockOldTool::new("Test", "Test tool"));
        let adapter: Arc<dyn UnifiedTool> = Arc::new(ToolAdapter::new(old_tool));
        assert_eq!(adapter.name(), "Test");
    }

    #[tokio::test]
    async fn test_tool_adapter_execute_success() {
        let old_tool: Arc<dyn Tool> = Arc::new(MockOldTool::new("Echo", "Echoes input"));
        let adapter = ToolAdapter::new(old_tool);

        let ctx = make_tool_context();
        let args = serde_json::json!({"input": "hello"});
        let result = adapter.execute(&ctx, args).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::String("old-Echo: hello".to_string()));
    }

    #[tokio::test]
    async fn test_tool_adapter_execute_failure() {
        let old_tool: Arc<dyn Tool> = Arc::new(FailingOldTool);
        let adapter = ToolAdapter::new(old_tool);

        let ctx = make_tool_context();
        let result = adapter.execute(&ctx, Value::Null).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("old tool failed"));
    }

    // ── UnifiedToolAdapter tests (new UnifiedTool -> old Tool) ─────────

    #[test]
    fn test_unified_tool_adapter_definition() {
        let new_tool: Arc<dyn UnifiedTool> = Arc::new(MockNewTool::new("Write", "Write a file"));
        let adapter = UnifiedToolAdapter::new(new_tool);

        assert_eq!(adapter.name(), "Write");
        assert_eq!(adapter.description(), "Write a file");
        assert!(!adapter.is_long_running());

        let schema = adapter.parameters_schema();
        assert_eq!(schema.schema_type, "object");
        assert!(schema.properties.is_some());
    }

    #[test]
    fn test_unified_tool_adapter_is_old_tool() {
        let new_tool: Arc<dyn UnifiedTool> = Arc::new(MockNewTool::new("Write", "Write a file"));
        let adapter = UnifiedToolAdapter::new(new_tool);
        let _old: &dyn Tool = &adapter;
        assert_eq!(_old.name(), "Write");
    }

    #[tokio::test]
    async fn test_unified_tool_adapter_execute_success() {
        let new_tool: Arc<dyn UnifiedTool> = Arc::new(MockNewTool::new("Grep", "Search files"));
        let adapter = UnifiedToolAdapter::new(new_tool);

        let ctx = make_tool_execution_context();
        let args = serde_json::json!({"input": "pattern"});
        let result = adapter.execute(&ctx, args).await;
        assert!(result.success);
        assert_eq!(result.output.unwrap(), "new-Grep: pattern");
    }

    #[tokio::test]
    async fn test_unified_tool_adapter_execute_failure() {
        let new_tool: Arc<dyn UnifiedTool> = Arc::new(FailingNewTool);
        let adapter = UnifiedToolAdapter::new(new_tool);

        let ctx = make_tool_execution_context();
        let result = adapter.execute(&ctx, Value::Null).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("new tool failed"));
    }

    // ── Registry import tests ──────────────────────────────────────────

    #[test]
    fn test_import_from_legacy_empty() {
        let legacy = ToolRegistry::new();
        let mut unified = UnifiedToolRegistry::new();
        import_legacy_tools(&mut unified, &legacy);
        assert!(unified.is_empty());
    }

    #[test]
    fn test_import_from_legacy_with_tools() {
        let mut legacy = ToolRegistry::new();
        legacy.register(Arc::new(MockOldTool::new("Read", "Read files")));
        legacy.register(Arc::new(MockOldTool::new("Write", "Write files")));
        legacy.register(Arc::new(MockOldTool::new("Grep", "Search files")));

        let mut unified = UnifiedToolRegistry::new();
        import_legacy_tools(&mut unified, &legacy);

        assert_eq!(unified.len(), 3);
        assert!(unified.contains("Read"));
        assert!(unified.contains("Write"));
        assert!(unified.contains("Grep"));
    }

    #[test]
    fn test_import_from_legacy_preserves_names() {
        let mut legacy = ToolRegistry::new();
        legacy.register(Arc::new(MockOldTool::new("Read", "Read files")));
        legacy.register(Arc::new(MockOldTool::new("Write", "Write files")));

        let mut unified = UnifiedToolRegistry::new();
        import_legacy_tools(&mut unified, &legacy);

        let names = unified.names();
        assert_eq!(names, vec!["Read", "Write"]);
    }

    #[tokio::test]
    async fn test_import_from_legacy_tools_are_executable() {
        let mut legacy = ToolRegistry::new();
        legacy.register(Arc::new(MockOldTool::new("Echo", "Echoes")));

        let mut unified = UnifiedToolRegistry::new();
        import_legacy_tools(&mut unified, &legacy);

        let ctx = make_tool_context();
        let result = unified
            .execute("Echo", &ctx, serde_json::json!({"input": "test"}))
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::String("old-Echo: test".to_string()));
    }

    // ── Bidirectional adapter roundtrip tests ──────────────────────────

    #[tokio::test]
    async fn test_old_to_new_to_old_roundtrip() {
        // Old tool -> ToolAdapter -> UnifiedToolAdapter -> used as old Tool
        let old_tool: Arc<dyn Tool> = Arc::new(MockOldTool::new("Roundtrip", "Test roundtrip"));
        let as_unified: Arc<dyn UnifiedTool> = Arc::new(ToolAdapter::new(old_tool));
        let back_to_old = UnifiedToolAdapter::new(as_unified);

        assert_eq!(back_to_old.name(), "Roundtrip");

        let ctx = make_tool_execution_context();
        let args = serde_json::json!({"input": "rt"});
        let result = back_to_old.execute(&ctx, args).await;
        assert!(result.success);
        assert_eq!(result.output.unwrap(), "old-Roundtrip: rt");
    }

    #[tokio::test]
    async fn test_new_to_old_to_new_roundtrip() {
        // New tool -> UnifiedToolAdapter -> ToolAdapter -> used as UnifiedTool
        let new_tool: Arc<dyn UnifiedTool> = Arc::new(MockNewTool::new("Roundtrip2", "Test roundtrip"));
        let as_old: Arc<dyn Tool> = Arc::new(UnifiedToolAdapter::new(new_tool));
        let back_to_new = ToolAdapter::new(as_old);

        assert_eq!(back_to_new.name(), "Roundtrip2");

        let ctx = make_tool_context();
        let args = serde_json::json!({"input": "rt2"});
        let result = back_to_new.execute(&ctx, args).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::String("new-Roundtrip2: rt2".to_string()));
    }

    // ── Send + Sync tests ──────────────────────────────────────────────

    #[test]
    fn test_adapters_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ToolAdapter>();
        assert_send_sync::<UnifiedToolAdapter>();
    }
}
