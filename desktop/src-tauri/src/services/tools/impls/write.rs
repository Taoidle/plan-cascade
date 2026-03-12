//! Write Tool Implementation
//!
//! Writes content to a file, creating directories as needed.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

use super::file_io::atomic_write_bytes;
use super::read::validate_path;
use super::text_utils::{decode_text_with_format, encode_text_with_format};

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
            ParameterSchema::string(Some(
                "Path to the file to write (relative to workspace, or absolute path within workspace)",
            )),
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

        let path = match validate_path(
            file_path,
            &ctx.working_directory_snapshot(),
            &ctx.project_root,
        ) {
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

        // Capture before-state for change tracking
        let before_hash = if let Some(tracker) = &ctx.file_change_tracker {
            if let Ok(t) = tracker.lock() {
                t.capture_before(&path)
            } else {
                None
            }
        } else {
            None
        };

        let write_bytes = if path.exists() {
            let existing_bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => return ToolResult::err(format!("Failed to read existing file: {}", e)),
            };
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let decoded = match decode_text_with_format(&existing_bytes, &ext) {
                Some(d) => d,
                None => {
                    return ToolResult::err(format!(
                        "Cannot overwrite binary/non-text file with Write: {}",
                        path.display()
                    ));
                }
            };
            encode_text_with_format(content, decoded.format)
        } else {
            content.as_bytes().to_vec()
        };

        match atomic_write_bytes(&path, &write_bytes) {
            Ok(_) => {
                ctx.invalidate_read_cache_for_path(&path);
                let line_count = content.lines().count();

                // Record change in tracker
                if let Some(tracker) = &ctx.file_change_tracker {
                    if let Ok(mut t) = tracker.lock() {
                        if let Ok(after_hash) = t.store_content(&write_bytes) {
                            let metadata = ctx.file_change_metadata();
                            let rel_path = path
                                .strip_prefix(&ctx.project_root)
                                .unwrap_or(&path)
                                .to_string_lossy()
                                .to_string();
                            let tool_call_id = format!("write-{}", uuid::Uuid::new_v4());
                            if let Some(turn_index) = ctx.file_change_turn_index {
                                t.record_change_at_with_metadata(
                                    turn_index,
                                    &tool_call_id,
                                    "Write",
                                    &rel_path,
                                    before_hash,
                                    Some(&after_hash),
                                    &format!("Wrote {} lines", line_count),
                                    metadata.as_ref(),
                                );
                            } else {
                                let turn_index = t.turn_index();
                                t.record_change_at_with_metadata(
                                    turn_index,
                                    &tool_call_id,
                                    "Write",
                                    &rel_path,
                                    before_hash,
                                    Some(&after_hash),
                                    &format!("Wrote {} lines", line_count),
                                    metadata.as_ref(),
                                );
                            }
                        }
                    }
                }

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
    use super::super::test_helpers::make_test_ctx;
    use super::super::text_utils::{decode_text_with_format, TextEncoding};
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_write_tool_basic() {
        let dir = TempDir::new().unwrap();
        let tool = WriteTool::new();
        let ctx = make_test_ctx(dir.path());

        let new_file = dir.path().join("new_file.txt");
        let args = serde_json::json!({
            "file_path": new_file.to_string_lossy().to_string(),
            "content": "new content"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.is_success());
        assert!(new_file.exists());
        assert_eq!(std::fs::read_to_string(&new_file).unwrap(), "new content");
    }

    #[tokio::test]
    async fn test_write_tool_creates_directories() {
        let dir = TempDir::new().unwrap();
        let tool = WriteTool::new();
        let ctx = make_test_ctx(dir.path());

        let nested = dir.path().join("a/b/c/file.txt");
        let args = serde_json::json!({
            "file_path": nested.to_string_lossy().to_string(),
            "content": "deep content"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.is_success());
        assert!(nested.exists());
    }

    #[tokio::test]
    async fn test_write_tool_missing_params() {
        let dir = TempDir::new().unwrap();
        let tool = WriteTool::new();
        let ctx = make_test_ctx(dir.path());

        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(result.is_error());
        assert!(result.error_message_owned().unwrap().contains("file_path"));
    }

    #[test]
    fn test_write_tool_name() {
        let tool = WriteTool::new();
        assert_eq!(tool.name(), "Write");
    }

    #[tokio::test]
    async fn test_write_tool_preserves_utf16le_bom_and_crlf_for_existing_file() {
        let dir = TempDir::new().unwrap();
        let tool = WriteTool::new();
        let ctx = make_test_ctx(dir.path());
        let file = dir.path().join("existing.txt");

        let mut original = vec![0xFF, 0xFE];
        for unit in "old\r\ntext\r\n".encode_utf16() {
            original.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(&file, original).unwrap();

        let args = serde_json::json!({
            "file_path": file.to_string_lossy().to_string(),
            "content": "new\ncontent\n"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.is_success());

        let written = std::fs::read(&file).unwrap();
        let decoded = decode_text_with_format(&written, "txt").unwrap();
        assert_eq!(decoded.format.encoding, TextEncoding::Utf16Le);
        assert!(decoded.format.has_bom);
        assert_eq!(decoded.text, "new\r\ncontent\r\n");
    }

    #[tokio::test]
    async fn test_write_tool_rejects_existing_binary_file() {
        let dir = TempDir::new().unwrap();
        let tool = WriteTool::new();
        let ctx = make_test_ctx(dir.path());
        let file = dir.path().join("blob.bin");
        std::fs::write(&file, vec![0x00, 0xFF, 0x80, 0x11]).unwrap();

        let args = serde_json::json!({
            "file_path": file.to_string_lossy().to_string(),
            "content": "text"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.is_error());
        assert!(result
            .error_message_owned()
            .unwrap()
            .contains("Cannot overwrite binary/non-text file"));
    }
}
