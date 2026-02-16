//! Analyze Tool Implementation
//!
//! Provides project analysis via the codebase index, with quick and deep modes.
//! This is a proxy tool — it delegates to the Analyze command in the executor
//! which manages the index store. Since the Analyze tool requires access to
//! the IndexStore (which is not in ToolExecutionContext), this implementation
//! serves as a placeholder that returns a not-available message when called
//! directly through the trait. The actual Analyze execution goes through
//! ToolExecutor which has access to the IndexStore.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// Analyze tool — project analysis via codebase index.
pub struct AnalyzeTool;

impl AnalyzeTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for AnalyzeTool {
    fn name(&self) -> &str {
        "Analyze"
    }

    fn description(&self) -> &str {
        "Gather project context for informed decisions. Defaults to quick mode: returns a concise project brief from file inventory (relevant files, components, test coverage). Use mode='deep' ONLY when the user explicitly requests comprehensive architectural analysis, cross-module dependency tracing, or full codebase review. Do NOT use this tool for simple questions — use Cwd, LS, Read, Glob, or Grep instead."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "query".to_string(),
            ParameterSchema::string(Some(
                "What to analyze. Use concise objective language (e.g., 'analyze architecture and test strategy').",
            )),
        );
        properties.insert(
            "mode".to_string(),
            ParameterSchema::string(Some(
                "Analysis mode: 'quick' (default — lightweight file inventory brief), 'deep' (full multi-phase analysis pipeline, use only when explicitly needed), or 'local' (focused on specific paths).",
            )),
        );
        properties.insert(
            "path_hint".to_string(),
            ParameterSchema::string(Some(
                "Optional path/file hint to focus the analysis scope (e.g., 'src/plan_cascade/core').",
            )),
        );
        ParameterSchema::object(
            Some("Analyze parameters"),
            properties,
            vec!["query".to_string()],
        )
    }

    async fn execute(&self, _ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        // The Analyze tool requires IndexStore access which is managed by ToolExecutor.
        // When called through the registry, we return a helpful message.
        // In practice, ToolExecutor intercepts "Analyze" calls before they reach here.
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("(no query)");
        ToolResult::ok(format!(
            "Analyze tool received query '{}'. Note: Analysis requires the codebase index. Use Grep, Glob, or LS for direct file exploration.",
            query
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    fn make_ctx(dir: &Path) -> ToolExecutionContext {
        ToolExecutionContext {
            session_id: "test".to_string(),
            project_root: dir.to_path_buf(),
            working_directory: dir.to_path_buf(),
            read_cache: Arc::new(Mutex::new(HashMap::new())),
            read_files: Arc::new(Mutex::new(HashSet::new())),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    #[tokio::test]
    async fn test_analyze_tool_basic() {
        let dir = TempDir::new().unwrap();
        let tool = AnalyzeTool::new();
        let ctx = make_ctx(dir.path());

        let args = serde_json::json!({"query": "test analysis"});
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("test analysis"));
    }

    #[test]
    fn test_analyze_tool_name() {
        let tool = AnalyzeTool::new();
        assert_eq!(tool.name(), "Analyze");
    }

    #[test]
    fn test_analyze_tool_schema_has_quick_deep() {
        let tool = AnalyzeTool::new();
        let desc = tool.description();
        assert!(desc.contains("quick mode"));
        assert!(desc.contains("deep"));
    }
}
