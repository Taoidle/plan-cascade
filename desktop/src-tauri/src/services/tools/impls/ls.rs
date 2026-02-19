//! LS Tool Implementation
//!
//! Lists directory contents with type indicators and file sizes.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

use super::read::validate_path;

/// Maximum number of entries to display in LS output.
const LS_MAX_ENTRIES: usize = 200;

/// Format a file size into a human-readable string
fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn missing_param_error() -> String {
    let example = r#"```tool_call
{"tool": "LS", "arguments": {"path": "."}}
```"#;
    format!("Missing required parameter: path. Correct format:\n{example}")
}

/// LS directory listing tool — lists files and directories.
pub struct LsTool;

impl LsTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "LS"
    }

    fn description(&self) -> &str {
        "List files and directories at the given path. Returns a formatted listing with type indicators (DIR/FILE), file sizes, and names."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "path".to_string(),
            ParameterSchema::string(Some("The directory path to list. Absolute or relative to working directory.")),
        );
        properties.insert(
            "show_hidden".to_string(),
            ParameterSchema::boolean(Some("Show hidden files (starting with '.'), default false")),
        );
        ParameterSchema::object(
            Some("List directory parameters"),
            properties,
            vec!["path".to_string()],
        )
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let dir_path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err(missing_param_error()),
        };

        let show_hidden = args
            .get("show_hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let path = match validate_path(dir_path, &ctx.working_directory_snapshot(), &ctx.project_root) {
            Ok(p) => p,
            Err(e) => return ToolResult::err(e),
        };

        if !path.exists() {
            return ToolResult::err(format!("Directory not found: {}", dir_path));
        }

        if !path.is_dir() {
            return ToolResult::err(format!("Not a directory: {}", dir_path));
        }

        match std::fs::read_dir(&path) {
            Ok(entries) => {
                let mut items: Vec<(String, bool, u64)> = Vec::new();

                for entry in entries {
                    let entry = match entry {
                        Ok(e) => e,
                        Err(_) => continue,
                    };

                    let name = entry.file_name().to_string_lossy().to_string();

                    if !show_hidden && name.starts_with('.') {
                        continue;
                    }

                    let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                    let size = if is_dir {
                        0
                    } else {
                        entry.metadata().map(|m| m.len()).unwrap_or(0)
                    };

                    items.push((name, is_dir, size));
                }

                items.sort_by(|a, b| {
                    b.1.cmp(&a.1)
                        .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
                });

                if items.is_empty() {
                    return ToolResult::ok(format!("Directory is empty: {}", path.display()));
                }

                let total_count = items.len();
                let total_dirs = items.iter().filter(|i| i.1).count();
                let total_files = items.iter().filter(|i| !i.1).count();

                let truncated = total_count > LS_MAX_ENTRIES;
                if truncated {
                    items.truncate(LS_MAX_ENTRIES);
                }

                let mut output = format!("Directory: {}\n\n", path.display());
                for (name, is_dir, size) in &items {
                    if *is_dir {
                        output.push_str(&format!("  DIR   {:>10}  {}/\n", "-", name));
                    } else {
                        output.push_str(&format!("  FILE  {:>10}  {}\n", format_size(*size), name));
                    }
                }

                if truncated {
                    let omitted = total_count - LS_MAX_ENTRIES;
                    output.push_str(&format!(
                        "\n... ({} more entries not shown. Use Glob for targeted file discovery.)",
                        omitted
                    ));
                }

                output.push_str(&format!(
                    "\n{} entries ({} dirs, {} files)",
                    total_count, total_dirs, total_files,
                ));

                ToolResult::ok(output)
            }
            Err(e) => ToolResult::err(format!("Failed to read directory: {}", e)),
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
            core_context: None,
        }
    }

    #[tokio::test]
    async fn test_ls_tool_basic() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "content").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();

        let tool = LsTool::new();
        let ctx = make_ctx(dir.path());

        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("DIR"));
        assert!(output.contains("subdir"));
        assert!(output.contains("test.txt"));
    }

    #[tokio::test]
    async fn test_ls_tool_not_a_directory() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "content").unwrap();

        let tool = LsTool::new();
        let ctx = make_ctx(dir.path());

        let args = serde_json::json!({
            "path": dir.path().join("test.txt").to_string_lossy().to_string()
        });
        let result = tool.execute(&ctx, args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Not a directory"));
    }

    #[test]
    fn test_ls_tool_name() {
        let tool = LsTool::new();
        assert_eq!(tool.name(), "LS");
    }

    #[test]
    fn test_format_size_helper() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(100), "100 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
    }
}
