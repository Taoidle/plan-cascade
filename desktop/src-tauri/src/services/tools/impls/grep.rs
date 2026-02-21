//! Grep Tool Implementation
//!
//! Searches file contents using regex patterns with .gitignore-aware traversal.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

use super::read::validate_path;
use super::scan_utils::is_default_scan_excluded;

fn missing_param_error() -> String {
    let example = r#"```tool_call
{"tool": "Grep", "arguments": {"pattern": "search_term"}}
```"#;
    format!("Missing required parameter: pattern. Correct format:\n{example}")
}

/// Grep content search tool — searches for regex patterns in files.
pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Self {
        Self
    }

    /// Search a single file for grep matches
    fn grep_file(
        &self,
        path: &Path,
        regex: &regex::Regex,
        output_mode: &str,
        context_lines: usize,
        head_limit: usize,
        results: &mut Vec<String>,
        total_output_len: &mut usize,
        max_output: usize,
        result_count: &mut usize,
    ) {
        if *total_output_len >= max_output {
            return;
        }
        if head_limit > 0 && *result_count >= head_limit {
            return;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut file_match_count = 0usize;
        let mut file_matched = false;

        for (line_num, line) in lines.iter().enumerate() {
            if !regex.is_match(line) {
                continue;
            }
            file_match_count += 1;
            file_matched = true;

            match output_mode {
                "files_with_matches" => {
                    let entry = path.display().to_string();
                    *total_output_len += entry.len() + 1;
                    results.push(entry);
                    *result_count += 1;
                    return;
                }
                "count" => {}
                _ => {
                    let start = line_num.saturating_sub(context_lines);
                    let end = (line_num + context_lines + 1).min(lines.len());

                    let context: Vec<String> = lines[start..end]
                        .iter()
                        .enumerate()
                        .map(|(i, l)| {
                            let num = start + i + 1;
                            let marker = if start + i == line_num { ">" } else { " " };
                            format!("{}{}:{}", marker, num, l)
                        })
                        .collect();

                    let entry = format!("{}:\n{}", path.display(), context.join("\n"));
                    *total_output_len += entry.len() + 2;
                    results.push(entry);
                    *result_count += 1;

                    if *total_output_len >= max_output {
                        return;
                    }
                    if head_limit > 0 && *result_count >= head_limit {
                        return;
                    }
                }
            }
        }

        if output_mode == "count" && file_matched {
            let entry = format!("{}:{}", path.display(), file_match_count);
            *total_output_len += entry.len() + 1;
            results.push(entry);
            *result_count += 1;
        }
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        "Search for content matching a regex pattern in files. Respects .gitignore and skips binary/hidden files. Returns matching lines with file paths and line numbers."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "pattern".to_string(),
            ParameterSchema::string(Some("Regular expression pattern to search for")),
        );
        properties.insert(
            "path".to_string(),
            ParameterSchema::string(Some("File or directory to search in")),
        );
        properties.insert(
            "glob".to_string(),
            ParameterSchema::string(Some("Glob pattern to filter files (e.g., '*.rs')")),
        );
        properties.insert(
            "case_insensitive".to_string(),
            ParameterSchema::boolean(Some("Case insensitive search")),
        );
        properties.insert(
            "context_lines".to_string(),
            ParameterSchema::integer(Some("Number of context lines before and after matches")),
        );
        properties.insert(
            "output_mode".to_string(),
            ParameterSchema::string(Some(
                "Output mode: 'files_with_matches' (file paths only), 'content' (matching lines), 'count' (match counts). Default: 'files_with_matches'",
            )),
        );
        properties.insert(
            "head_limit".to_string(),
            ParameterSchema::integer(Some("Limit output to first N results (0 = unlimited)")),
        );
        ParameterSchema::object(
            Some("Grep parameters"),
            properties,
            vec!["pattern".to_string()],
        )
    }

    fn is_parallel_safe(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err(missing_param_error()),
        };

        let working_dir = ctx.working_directory_snapshot();
        let explicit_search_path = args.get("path").and_then(|v| v.as_str());
        let search_path = match explicit_search_path {
            Some(path) => match validate_path(path, &working_dir, &ctx.project_root) {
                Ok(resolved) => resolved,
                Err(err) => return ToolResult::err(err),
            },
            None => working_dir,
        };
        let apply_default_excludes = explicit_search_path
            .map(|p| {
                let normalized = p.trim().replace('\\', "/");
                normalized == "." || normalized == "./"
            })
            .unwrap_or(true);

        if !search_path.exists() {
            return ToolResult::err(format!("Path not found: {}", search_path.display()));
        }

        let file_glob = args.get("glob").and_then(|v| v.as_str());
        let case_insensitive = args
            .get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let context_lines = args
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let output_mode = args
            .get("output_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("files_with_matches");
        let head_limit = args.get("head_limit").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        let regex = match regex::RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .build()
        {
            Ok(r) => r,
            Err(e) => return ToolResult::err(format!("Invalid regex pattern: {}", e)),
        };

        let glob_matcher = file_glob.and_then(|g| {
            ignore::overrides::OverrideBuilder::new(&search_path)
                .add(g)
                .ok()
                .and_then(|b| b.build().ok())
        });

        let mut results = Vec::new();
        let mut total_output_len = 0usize;
        let max_output = 30_000;
        let mut result_count = 0usize;

        if search_path.is_file() {
            self.grep_file(
                &search_path,
                &regex,
                output_mode,
                context_lines,
                head_limit,
                &mut results,
                &mut total_output_len,
                max_output,
                &mut result_count,
            );
        } else {
            let walker = ignore::WalkBuilder::new(&search_path)
                .hidden(true)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .build();

            for entry in walker.flatten() {
                if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    continue;
                }

                let path = entry.path();

                if apply_default_excludes && is_default_scan_excluded(&search_path, path) {
                    continue;
                }

                if let Some(ref overrides) = glob_matcher {
                    match overrides.matched(path, false) {
                        ignore::Match::None | ignore::Match::Ignore(..) => continue,
                        ignore::Match::Whitelist(..) => {}
                    }
                }

                self.grep_file(
                    path,
                    &regex,
                    output_mode,
                    context_lines,
                    head_limit,
                    &mut results,
                    &mut total_output_len,
                    max_output,
                    &mut result_count,
                );

                if total_output_len >= max_output {
                    break;
                }
                if head_limit > 0 && result_count >= head_limit {
                    break;
                }
            }
        }

        if results.is_empty() {
            ToolResult::ok("No matches found")
        } else {
            let output = results.join("\n");
            if total_output_len >= max_output {
                ToolResult::ok(format!("{}\n\n... (output truncated)", output))
            } else {
                ToolResult::ok(output)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_helpers::make_test_ctx;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_grep_tool_basic() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "line 1\nline 2\nline 3\n").unwrap();

        let tool = GrepTool::new();
        let ctx = make_test_ctx(dir.path());

        let args = serde_json::json!({
            "pattern": "line",
            "path": dir.path().to_string_lossy().to_string()
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("test.txt"));
    }

    #[tokio::test]
    async fn test_grep_tool_content_mode() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "line 1\nline 2\nline 3\n").unwrap();

        let tool = GrepTool::new();
        let ctx = make_test_ctx(dir.path());

        let args = serde_json::json!({
            "pattern": "line",
            "path": dir.path().to_string_lossy().to_string(),
            "output_mode": "content"
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("line 1"));
    }

    #[tokio::test]
    async fn test_grep_tool_missing_param() {
        let dir = TempDir::new().unwrap();
        let tool = GrepTool::new();
        let ctx = make_test_ctx(dir.path());

        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(!result.success);
    }

    #[test]
    fn test_grep_tool_name() {
        let tool = GrepTool::new();
        assert_eq!(tool.name(), "Grep");
    }
}
