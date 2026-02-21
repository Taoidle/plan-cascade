//! Edit Tool Implementation
//!
//! Performs string replacement in files with uniqueness checking.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

use super::read::validate_path;
use super::text_utils::decode_read_text;

/// Edit file tool — performs string replacement in files.
pub struct EditTool;

impl EditTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn description(&self) -> &str {
        "Perform string replacement in a file. The old_string must be unique in the file unless replace_all is true. Preserves file encoding and line endings."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "file_path".to_string(),
            ParameterSchema::string(Some("The absolute path to the file to edit")),
        );
        properties.insert(
            "old_string".to_string(),
            ParameterSchema::string(Some("The exact string to replace")),
        );
        properties.insert(
            "new_string".to_string(),
            ParameterSchema::string(Some("The string to replace it with")),
        );
        properties.insert(
            "replace_all".to_string(),
            ParameterSchema::boolean(Some("Replace all occurrences (default: false)")),
        );
        ParameterSchema::object(
            Some("Edit file parameters"),
            properties,
            vec![
                "file_path".to_string(),
                "old_string".to_string(),
                "new_string".to_string(),
            ],
        )
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: file_path"),
        };

        let old_string = match args.get("old_string").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::err("Missing required parameter: old_string"),
        };

        let new_string = match args.get("new_string").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::err("Missing required parameter: new_string"),
        };

        let replace_all = args
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let path = match validate_path(file_path, &ctx.working_directory_snapshot(), &ctx.project_root) {
            Ok(p) => p,
            Err(e) => return ToolResult::err(e),
        };

        if !path.exists() {
            return ToolResult::err(format!("File not found: {}", file_path));
        }

        // Enforce read-before-write: the file must have been read first
        if let Ok(read_files) = ctx.read_files.lock() {
            if !read_files.contains(&path) {
                return ToolResult::err(
                    "You must read the file before editing it. Use the Read tool first to view the current contents.",
                );
            }
        }

        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => return ToolResult::err(format!("Failed to read file: {}", e)),
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let (content, _) = match decode_read_text(&bytes, &ext) {
            Some(value) => value,
            None => return ToolResult::err(format!("Cannot edit binary file: {}", path.display())),
        };

        let occurrences = content.matches(old_string).count();
        if occurrences == 0 {
            return ToolResult::err(
                "String not found in file. The old_string must exist in the file.".to_string(),
            );
        }

        if !replace_all && occurrences > 1 {
            return ToolResult::err(format!(
                "The old_string appears {} times in the file. Either provide more context to make it unique, or set replace_all to true.",
                occurrences
            ));
        }

        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        match std::fs::write(&path, &new_content) {
            Ok(_) => {
                ctx.invalidate_read_cache_for_path(&path);
                if replace_all {
                    ToolResult::ok(format!(
                        "Successfully replaced {} occurrences in {}",
                        occurrences,
                        path.display()
                    ))
                } else {
                    ToolResult::ok(format!("Successfully edited {}", path.display()))
                }
            }
            Err(e) => ToolResult::err(format!("Failed to write file: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_helpers::make_test_ctx;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_edit_tool_basic() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line 1\nline 2\nline 3\n").unwrap();
        let tool = EditTool::new();
        let ctx = make_test_ctx(dir.path());
        // Pre-populate read_files to satisfy read-before-write guard
        ctx.read_files.lock().unwrap().insert(file_path.clone());

        let args = serde_json::json!({
            "file_path": file_path.to_string_lossy().to_string(),
            "old_string": "line 2",
            "new_string": "modified line 2"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("modified line 2"));
    }

    #[tokio::test]
    async fn test_edit_tool_non_unique() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("dup.txt");
        std::fs::write(&file_path, "foo foo foo").unwrap();
        let tool = EditTool::new();
        let ctx = make_test_ctx(dir.path());
        ctx.read_files.lock().unwrap().insert(file_path.clone());

        let args = serde_json::json!({
            "file_path": file_path.to_string_lossy().to_string(),
            "old_string": "foo",
            "new_string": "bar"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("appears 3 times"));
    }

    #[tokio::test]
    async fn test_edit_tool_replace_all() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("dup.txt");
        std::fs::write(&file_path, "foo foo foo").unwrap();
        let tool = EditTool::new();
        let ctx = make_test_ctx(dir.path());
        ctx.read_files.lock().unwrap().insert(file_path.clone());

        let args = serde_json::json!({
            "file_path": file_path.to_string_lossy().to_string(),
            "old_string": "foo",
            "new_string": "bar",
            "replace_all": true
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "bar bar bar");
    }

    #[tokio::test]
    async fn test_edit_tool_rejects_unread_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("unread.txt");
        std::fs::write(&file_path, "original content").unwrap();
        let tool = EditTool::new();
        let ctx = make_test_ctx(dir.path());
        // Do NOT insert into read_files — simulate editing without reading first

        let args = serde_json::json!({
            "file_path": file_path.to_string_lossy().to_string(),
            "old_string": "original",
            "new_string": "modified"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("must read the file before editing"));
    }

    #[test]
    fn test_edit_tool_name() {
        let tool = EditTool::new();
        assert_eq!(tool.name(), "Edit");
    }
}
