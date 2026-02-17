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

fn is_likely_text_extension(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "md" | "markdown" | "rst" | "json" | "jsonl" | "yaml" | "yml"
            | "toml" | "ini" | "cfg" | "conf" | "lock" | "env" | "gitignore"
            | "gitattributes" | "py" | "rs" | "ts" | "tsx" | "js" | "jsx"
            | "java" | "kt" | "go" | "c" | "h" | "cpp" | "hpp" | "cs"
            | "rb" | "php" | "swift" | "scala" | "sql" | "sh" | "bash"
            | "ps1" | "zsh" | "fish" | "xml" | "html" | "htm" | "css"
            | "scss" | "less" | "svg" | "vue" | "svelte"
    )
}

fn is_probably_binary(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let sample_len = bytes.len().min(4096);
    let sample = &bytes[..sample_len];
    if sample.contains(&0) {
        return true;
    }
    let mut suspicious = 0usize;
    for b in sample {
        let is_text_like = matches!(*b, 0x09 | 0x0A | 0x0D | 0x20..=0x7E);
        if !is_text_like {
            suspicious += 1;
        }
    }
    (suspicious as f64 / sample_len as f64) > 0.30
}

fn decode_read_text(bytes: &[u8], ext: &str) -> Option<(String, bool)> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Some((text.to_string(), false)),
        Err(_) => {
            if is_likely_text_extension(ext) || !is_probably_binary(bytes) {
                Some((String::from_utf8_lossy(bytes).into_owned(), true))
            } else {
                None
            }
        }
    }
}

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
    async fn test_edit_tool_basic() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "line 1\nline 2\nline 3\n").unwrap();
        let tool = EditTool::new();
        let ctx = make_ctx(dir.path());

        let args = serde_json::json!({
            "file_path": dir.path().join("test.txt").to_string_lossy().to_string(),
            "old_string": "line 2",
            "new_string": "modified line 2"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);

        let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert!(content.contains("modified line 2"));
    }

    #[tokio::test]
    async fn test_edit_tool_non_unique() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("dup.txt"), "foo foo foo").unwrap();
        let tool = EditTool::new();
        let ctx = make_ctx(dir.path());

        let args = serde_json::json!({
            "file_path": dir.path().join("dup.txt").to_string_lossy().to_string(),
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
        std::fs::write(dir.path().join("dup.txt"), "foo foo foo").unwrap();
        let tool = EditTool::new();
        let ctx = make_ctx(dir.path());

        let args = serde_json::json!({
            "file_path": dir.path().join("dup.txt").to_string_lossy().to_string(),
            "old_string": "foo",
            "new_string": "bar",
            "replace_all": true
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        let content = std::fs::read_to_string(dir.path().join("dup.txt")).unwrap();
        assert_eq!(content, "bar bar bar");
    }

    #[test]
    fn test_edit_tool_name() {
        let tool = EditTool::new();
        assert_eq!(tool.name(), "Edit");
    }
}
