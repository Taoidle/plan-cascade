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
        ParameterSchema::object(
            Some("No parameters required"),
            HashMap::new(),
            vec![],
        )
    }

    async fn execute(&self, ctx: &ToolExecutionContext, _args: Value) -> ToolResult {
        ToolResult::ok(ctx.working_directory_snapshot().to_string_lossy().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    fn make_ctx(dir: &Path) -> ToolExecutionContext {
        ToolExecutionContext {
            session_id: "test".to_string(),
            project_root: dir.to_path_buf(),
            working_directory: Arc::new(Mutex::new(dir.to_path_buf())),
            read_cache: Arc::new(Mutex::new(HashMap::new())),
            read_files: Arc::new(Mutex::new(HashSet::new())),
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

    #[tokio::test]
    async fn test_cwd_tool() {
        let dir = TempDir::new().unwrap();
        let tool = CwdTool::new();
        let ctx = make_ctx(dir.path());

        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(result.success);
        assert_eq!(result.output.unwrap(), dir.path().to_string_lossy().to_string());
    }

    #[test]
    fn test_cwd_tool_name() {
        let tool = CwdTool::new();
        assert_eq!(tool.name(), "Cwd");
    }
}
