//! Glob Tool Implementation
//!
//! Finds files matching glob patterns, sorted by modification time.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

use super::read::validate_path;

/// Directories excluded from default full-workspace scans.
const DEFAULT_SCAN_EXCLUDES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
    ".venv",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".plan-cascade",
    "builtin-skills",
    "external-skills",
    "claude-code",
    "codex",
];

fn is_default_scan_excluded(base: &Path, candidate: &Path) -> bool {
    if let Ok(relative) = candidate.strip_prefix(base) {
        if let Some(first) = relative.components().next() {
            let root = first.as_os_str().to_string_lossy();
            return DEFAULT_SCAN_EXCLUDES.contains(&root.as_ref());
        }
    }
    false
}

fn missing_param_error() -> String {
    let example = r#"```tool_call
{"tool": "Glob", "arguments": {"pattern": "**/*.rs"}}
```"#;
    format!("Missing required parameter: pattern. Correct format:\n{example}")
}

/// Glob file matching tool — finds files by glob pattern.
pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. Returns a list of matching file paths sorted by modification time."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "pattern".to_string(),
            ParameterSchema::string(Some("The glob pattern to match (e.g., '**/*.rs', 'src/**/*.ts')")),
        );
        properties.insert(
            "path".to_string(),
            ParameterSchema::string(Some("The directory to search in (defaults to current working directory)")),
        );
        ParameterSchema::object(
            Some("Glob parameters"),
            properties,
            vec!["pattern".to_string()],
        )
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err(missing_param_error()),
        };
        let head_limit = args.get("head_limit").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        let working_dir = ctx.working_directory_snapshot();
        let explicit_base_path = args.get("path").and_then(|v| v.as_str());
        let base_path = match explicit_base_path {
            Some(path) => match validate_path(path, &working_dir, &ctx.project_root) {
                Ok(resolved) => resolved,
                Err(err) => return ToolResult::err(err),
            },
            None => working_dir,
        };
        let apply_default_excludes = explicit_base_path
            .map(|p| {
                let normalized = p.trim().replace('\\', "/");
                normalized == "." || normalized == "./"
            })
            .unwrap_or(true);

        let pattern_path = Path::new(pattern);
        let full_pattern = if pattern_path.is_absolute() {
            pattern_path.to_path_buf()
        } else {
            base_path.join(pattern)
        };
        let pattern_str = full_pattern.to_string_lossy();

        match glob::glob(&pattern_str) {
            Ok(paths) => {
                let mut matches: Vec<(PathBuf, std::time::SystemTime)> = paths
                    .filter_map(|r| r.ok())
                    .filter_map(|p| {
                        p.metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .map(|t| (p, t))
                    })
                    .collect();

                matches.sort_by(|a, b| b.1.cmp(&a.1));

                let result: Vec<String> = matches
                    .iter()
                    .filter(|(p, _)| {
                        !apply_default_excludes || !is_default_scan_excluded(&base_path, p)
                    })
                    .take(if head_limit > 0 { head_limit } else { usize::MAX })
                    .map(|(p, _)| p.to_string_lossy().to_string())
                    .collect();

                if result.is_empty() {
                    ToolResult::ok("No files matched the pattern")
                } else {
                    ToolResult::ok(result.join("\n"))
                }
            }
            Err(e) => ToolResult::err(format!("Invalid glob pattern: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
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
    async fn test_glob_tool_basic() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "content").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/nested.txt"), "nested").unwrap();

        let tool = GlobTool::new();
        let ctx = make_ctx(dir.path());

        let args = serde_json::json!({
            "pattern": "**/*.txt",
            "path": dir.path().to_string_lossy().to_string()
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("test.txt"));
        assert!(output.contains("nested.txt"));
    }

    #[tokio::test]
    async fn test_glob_tool_missing_param() {
        let dir = TempDir::new().unwrap();
        let tool = GlobTool::new();
        let ctx = make_ctx(dir.path());

        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(!result.success);
    }

    #[test]
    fn test_glob_tool_name() {
        let tool = GlobTool::new();
        assert_eq!(tool.name(), "Glob");
    }
}
