//! Write Tool Implementation
//!
//! Writes content to a file, creating directories as needed.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

use super::read::validate_path;

/// Write file tool — writes content to a file, creating parent directories as needed.
pub struct WriteTool;

impl WriteTool {
    pub fn new() -> Self {
        Self
    }
}

fn missing_param_error(param: &str) -> String {
    match param {
        "file_path" => {
            let example = r#"```tool_call
{"tool": "Write", "arguments": {"file_path": "path/to/file", "content": "file content"}}
```"#;
            format!("Missing required parameter: {param}. Correct format:\n{example}")
        }
        _ => format!("Missing required parameter: {param}"),
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Creates parent directories as needed."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "file_path".to_string(),
            ParameterSchema::string(Some("The absolute path to the file to write")),
        );
        properties.insert(
            "content".to_string(),
            ParameterSchema::string(Some("The content to write to the file")),
        );
        ParameterSchema::object(
            Some("Write file parameters"),
            properties,
            vec!["file_path".to_string(), "content".to_string()],
        )
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err(missing_param_error("file_path")),
        };

        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err(missing_param_error("content")),
        };

        let path = match validate_path(file_path, &ctx.working_directory_snapshot(), &ctx.project_root) {
            Ok(p) => p,
            Err(e) => return ToolResult::err(e),
        };

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return ToolResult::err(format!("Failed to create directories: {}", e));
                }
            }
        }

        match std::fs::write(&path, content) {
            Ok(_) => {
                let line_count = content.lines().count();
                ToolResult::ok(format!(
                    "Successfully wrote {} lines to {}",
                    line_count,
                    path.display()
                ))
            }
            Err(e) => ToolResult::err(format!("Failed to write file: {}", e)),
        }
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
        }
    }

    #[tokio::test]
    async fn test_write_tool_basic() {
        let dir = TempDir::new().unwrap();
        let tool = WriteTool::new();
        let ctx = make_ctx(dir.path());

        let new_file = dir.path().join("new_file.txt");
        let args = serde_json::json!({
            "file_path": new_file.to_string_lossy().to_string(),
            "content": "new content"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        assert!(new_file.exists());
        assert_eq!(std::fs::read_to_string(&new_file).unwrap(), "new content");
    }

    #[tokio::test]
    async fn test_write_tool_creates_directories() {
        let dir = TempDir::new().unwrap();
        let tool = WriteTool::new();
        let ctx = make_ctx(dir.path());

        let nested = dir.path().join("a/b/c/file.txt");
        let args = serde_json::json!({
            "file_path": nested.to_string_lossy().to_string(),
            "content": "deep content"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        assert!(nested.exists());
    }

    #[tokio::test]
    async fn test_write_tool_missing_params() {
        let dir = TempDir::new().unwrap();
        let tool = WriteTool::new();
        let ctx = make_ctx(dir.path());

        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("file_path"));
    }

    #[test]
    fn test_write_tool_name() {
        let tool = WriteTool::new();
        assert_eq!(tool.name(), "Write");
    }
}
