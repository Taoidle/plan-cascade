//! Analyze Tool Implementation
//!
//! Provides project analysis via the codebase index, with quick and deep modes.
//! The orchestrator owns the full Analyze pipeline and intercepts Analyze calls
//! in the agentic loop so usage/iteration accounting remains accurate.
//! If this trait implementation is reached directly, return a hard error
//! instead of a fake success result.

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
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("(no query)");
        ToolResult::err(format!(
            "Analyze tool is orchestrator-managed and was invoked through the generic tool path for query '{}'. Retry via orchestrator Analyze flow.",
            query
        ))
        .with_error_code("analyze_orchestrator_required")
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_test_ctx;
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_analyze_tool_basic() {
        let dir = TempDir::new().unwrap();
        let tool = AnalyzeTool::new();
        let ctx = make_test_ctx(dir.path());

        let args = serde_json::json!({"query": "test analysis"});
        let result = tool.execute(&ctx, args).await;
        assert!(result.is_error());
        assert!(result.error_message_owned().unwrap().contains("test analysis"));
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
