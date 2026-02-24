//! Cwd Tool Implementation
//!
//! Returns the current working directory.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// Cwd tool — returns the current working directory.
pub struct CwdTool;

impl CwdTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CwdTool {
    fn name(&self) -> &str {
        "Cwd"
    }

    fn description(&self) -> &str {
        "Get the current working directory (project root). Returns the absolute path of the current working directory."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        ParameterSchema::object(Some("No parameters required"), HashMap::new(), vec![])
    }

    fn is_parallel_safe(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolExecutionContext, _args: Value) -> ToolResult {
        ToolResult::ok(
            ctx.working_directory_snapshot()
                .to_string_lossy()
                .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_test_ctx;
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cwd_tool() {
        let dir = TempDir::new().unwrap();
        let tool = CwdTool::new();
        let ctx = make_test_ctx(dir.path());

        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(result.success);
        assert_eq!(
            result.output.unwrap(),
            dir.path().to_string_lossy().to_string()
        );
    }

    #[test]
    fn test_cwd_tool_name() {
        let tool = CwdTool::new();
        assert_eq!(tool.name(), "Cwd");
    }
}
