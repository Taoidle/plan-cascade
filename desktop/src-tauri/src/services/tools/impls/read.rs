//! Read Tool Implementation
//!
//! Reads file contents with line numbers, offset/limit support,
//! and rich format parsing (PDF, DOCX, XLSX, Jupyter, images).
//! Includes content-aware deduplication via the shared read cache.

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::{ReadCacheEntry, ToolResult};
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::text_utils::decode_read_text;

/// Read file tool — reads file contents with line numbers and caching.
pub struct ReadTool;

impl ReadTool {
    pub fn new() -> Self {
        Self
    }
}

fn missing_param_error(param: &str) -> String {
    match param {
        "file_path" => {
            let example = r#"```tool_call
{"tool": "Read", "arguments": {"file_path": "path/to/file"}}
```"#;
            format!("Missing required parameter: {param}. Correct format:\n{example}")
        }
        _ => format!("Missing required parameter: {param}"),
    }
}

/// Validate and resolve a file path relative to a working directory and project root.
pub(crate) fn validate_path(
    path_str: &str,
    working_dir: &Path,
    _project_root: &Path,
) -> Result<PathBuf, String> {
    let path = Path::new(path_str);
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    };

    let check_path = if abs_path.exists() {
        abs_path.clone()
    } else if let Some(parent) = abs_path.parent() {
        if parent.exists() {
            parent.to_path_buf()
        } else {
            return Ok(abs_path);
        }
    } else {
        return Ok(abs_path);
    };

    match check_path.canonicalize() {
        Ok(_canonical) => Ok(abs_path),
        Err(e) => Err(format!("Invalid path: {}", e)),
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Returns the file contents with line numbers. Supports optional offset and limit for reading specific portions of large files. Also reads PDF, DOCX, XLSX, Jupyter notebooks (.ipynb), and images (returns metadata)."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "file_path".to_string(),
            ParameterSchema::string(Some("The absolute path to the file to read")),
        );
        properties.insert(
            "offset".to_string(),
            ParameterSchema::integer(Some("The line number to start reading from (1-indexed)")),
        );
        properties.insert(
            "limit".to_string(),
            ParameterSchema::integer(Some("Maximum number of lines to read")),
        );
        properties.insert(
            "pages".to_string(),
            ParameterSchema::string(Some("Page range for PDF files (e.g., '1-5', '3', '10-20'). Only for PDFs. Max 20 pages per request.")),
        );
        ParameterSchema::object(
            Some("Read file parameters"),
            properties,
            vec!["file_path".to_string()],
        )
    }

    fn is_parallel_safe(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err(missing_param_error("file_path")),
        };

        let path = match validate_path(
            file_path,
            &ctx.working_directory_snapshot(),
            &ctx.project_root,
        ) {
            Ok(p) => p,
            Err(e) => return ToolResult::err(e),
        };

        if !path.exists() {
            return ToolResult::err(format!("File not found: {}", file_path));
        }

        // Track this file as read (for read-before-write enforcement)
        if let Ok(mut read_files) = ctx.read_files.lock() {
            read_files.insert(path.clone());
        }

        // Extension-based dispatch for rich file formats
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let parser_error_result = |err: String| {
            let lower = err.to_ascii_lowercase();
            if lower.contains("utf-8") || lower.contains("utf8") {
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                ToolResult::ok(format!(
                    "[binary/non-utf8 file skipped] {} ({} bytes).",
                    path.display(),
                    size
                ))
            } else {
                ToolResult::err(err)
            }
        };
        match ext.as_str() {
            "pdf" => {
                let pages = args.get("pages").and_then(|v| v.as_str());
                match crate::services::tools::file_parsers::parse_pdf(&path, pages) {
                    Ok(content) => return ToolResult::ok(content),
                    Err(e) => return ToolResult::err(e),
                }
            }
            "ipynb" => match crate::services::tools::file_parsers::parse_jupyter(&path) {
                Ok(content) => return ToolResult::ok(content),
                Err(e) => return parser_error_result(e),
            },
            "docx" => match crate::services::tools::file_parsers::parse_docx(&path) {
                Ok(content) => return ToolResult::ok(content),
                Err(e) => return parser_error_result(e),
            },
            "xlsx" | "xls" | "ods" => match crate::services::tools::file_parsers::parse_xlsx(&path)
            {
                Ok(content) => return ToolResult::ok(content),
                Err(e) => return parser_error_result(e),
            },
            "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" | "jar" | "war" | "class"
            | "woff" | "woff2" | "ttf" | "otf" | "eot" | "ico" | "mp3" | "wav" | "ogg" | "mp4"
            | "mov" | "avi" | "webm" | "exe" | "dll" | "so" | "dylib" | "bin" => {
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                return ToolResult::ok(format!(
                    "[binary file skipped] {} ({} bytes). Use parser-specific tools for binary/document formats.",
                    path.display(),
                    size
                ));
            }
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" => {
                let metadata =
                    match crate::services::tools::file_parsers::read_image_metadata(&path) {
                        Ok(m) => m,
                        Err(e) => return ToolResult::err(e),
                    };
                match crate::services::tools::file_parsers::encode_image_base64(&path) {
                    Ok((mime, b64)) => return ToolResult::ok_with_image(metadata, mime, b64),
                    Err(_) => return ToolResult::ok(metadata),
                }
            }
            _ => { /* fall through to regular text reading */ }
        }

        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;

        // --- Read cache deduplication check ---
        let current_mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());

        let cache_key = (path.clone(), offset, limit);

        if let Some(mtime) = current_mtime {
            if let Ok(cache) = ctx.read_cache.lock() {
                if let Some(entry) = cache.get(&cache_key) {
                    if entry.modified_time == mtime {
                        return ToolResult::ok_dedup(format!(
                            "[DEDUP] {} ({} lines) already read. Content unchanged.",
                            path.display(),
                            entry.line_count,
                        ));
                    }
                }
            }
        }

        // If mtime changed, clear any stale cache entry
        if let Ok(mut cache) = ctx.read_cache.lock() {
            cache.remove(&cache_key);
        }

        match std::fs::read(&path) {
            Ok(bytes) => {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let decoded = decode_read_text(&bytes, &ext);
                let (content, lossy_decoded) = match decoded {
                    Some(tuple) => tuple,
                    None => {
                        return ToolResult::ok(format!(
                            "[binary file skipped] {} ({} bytes). Use parser-specific tools for binary/document formats.",
                            path.display(),
                            bytes.len()
                        ));
                    }
                };

                let all_lines: Vec<&str> = content.lines().collect();
                let start = (offset.saturating_sub(1)).min(all_lines.len());
                let end = (start + limit).min(all_lines.len());

                let mut numbered_lines: Vec<String> = all_lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| {
                        let truncated = if line.len() > 2000 {
                            let mut end = 2000;
                            while end > 0 && !line.is_char_boundary(end) {
                                end -= 1;
                            }
                            format!("{}...", &line[..end])
                        } else {
                            line.to_string()
                        };
                        format!("{:6}\t{}", start + i + 1, truncated)
                    })
                    .collect();

                if lossy_decoded {
                    numbered_lines.insert(
                        0,
                        format!("[non-utf8 decoded with replacement] {}", path.display()),
                    );
                }

                // Populate the read cache after a successful read
                if let Some(mtime) = current_mtime {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    bytes.hash(&mut hasher);
                    let content_hash = hasher.finish();

                    let first_lines_preview: String = all_lines
                        .iter()
                        .take(5)
                        .map(|l| {
                            if l.len() > 120 {
                                let mut end = 120;
                                while end > 0 && !l.is_char_boundary(end) {
                                    end -= 1;
                                }
                                format!("{}...", &l[..end])
                            } else {
                                l.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    let entry = ReadCacheEntry {
                        path: path.clone(),
                        modified_time: mtime,
                        line_count: all_lines.len(),
                        size_bytes: bytes.len() as u64,
                        content_hash,
                        offset,
                        limit,
                        extension: ext.clone(),
                        first_lines_preview,
                    };

                    if let Ok(mut cache) = ctx.read_cache.lock() {
                        cache.insert(cache_key, entry);
                    }
                }

                ToolResult::ok(numbered_lines.join("\n"))
            }
            Err(e) => ToolResult::err(format!("Failed to read file: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_test_ctx;
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_tool_basic() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "line 1\nline 2\nline 3\n").unwrap();
        let tool = ReadTool::new();
        let ctx = make_test_ctx(dir.path());

        let args = serde_json::json!({
            "file_path": dir.path().join("test.txt").to_string_lossy().to_string()
        });
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("line 1"));
    }

    #[tokio::test]
    async fn test_read_tool_not_found() {
        let dir = TempDir::new().unwrap();
        let tool = ReadTool::new();
        let ctx = make_test_ctx(dir.path());

        let args = serde_json::json!({
            "file_path": dir.path().join("nonexistent.txt").to_string_lossy().to_string()
        });
        let result = tool.execute(&ctx, args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_read_tool_dedup() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "content\n").unwrap();
        let tool = ReadTool::new();
        let ctx = make_test_ctx(dir.path());

        let args = serde_json::json!({
            "file_path": dir.path().join("test.txt").to_string_lossy().to_string()
        });

        // First read
        let r1 = tool.execute(&ctx, args.clone()).await;
        assert!(r1.success);
        assert!(!r1.is_dedup);

        // Second read - should dedup
        let r2 = tool.execute(&ctx, args).await;
        assert!(r2.success);
        assert!(r2.is_dedup);
        assert!(r2.output.unwrap().contains("[DEDUP]"));
    }

    #[tokio::test]
    async fn test_read_tool_missing_param() {
        let dir = TempDir::new().unwrap();
        let tool = ReadTool::new();
        let ctx = make_test_ctx(dir.path());

        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("file_path"));
    }

    #[test]
    fn test_read_tool_name() {
        let tool = ReadTool::new();
        assert_eq!(tool.name(), "Read");
    }

    #[test]
    fn test_read_tool_not_long_running() {
        let tool = ReadTool::new();
        assert!(!tool.is_long_running());
    }
}
