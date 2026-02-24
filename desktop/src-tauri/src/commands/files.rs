//! File Commands
//!
//! Tauri commands for reading file contents and listing workspace files.
//! Used by the file attachment system in SimpleMode.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::models::CommandResponse;

// ============================================================================
// Result Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct FileContentResult {
    pub content: String,
    pub size: usize,
    pub is_binary: bool,
    pub mime_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceFileResult {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_MAX_SIZE: usize = 10_485_760; // 10MB
const BINARY_CHECK_SIZE: usize = 8192;

const IGNORE_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    "__pycache__",
    ".next",
    ".nuxt",
    ".turbo",
    "vendor",
    "coverage",
];

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg"];

// ============================================================================
// Helper Functions
// ============================================================================

fn get_extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn mime_type_from_extension(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "json" => "application/json",
        "js" | "mjs" | "cjs" => "text/javascript",
        "ts" | "mts" | "cts" => "text/typescript",
        "jsx" | "tsx" => "text/typescript",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "xml" => "text/xml",
        "yaml" | "yml" => "text/yaml",
        "toml" => "text/toml",
        "rs" => "text/rust",
        "py" => "text/python",
        "go" => "text/go",
        "java" => "text/java",
        "c" | "h" => "text/c",
        "cpp" | "hpp" | "cc" | "cxx" => "text/cpp",
        "sh" | "bash" | "zsh" => "text/x-shellscript",
        "sql" => "text/sql",
        "graphql" | "gql" => "text/graphql",
        "vue" => "text/vue",
        "svelte" => "text/svelte",
        _ => "text/plain",
    }
}

fn is_binary_content(data: &[u8]) -> bool {
    let check_len = data.len().min(BINARY_CHECK_SIZE);
    data[..check_len].contains(&0)
}

fn is_image_extension(ext: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&ext)
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn is_ignored_dir(name: &str) -> bool {
    IGNORE_DIRS.contains(&name)
}

// ============================================================================
// Commands
// ============================================================================

/// Read a file for attachment purposes.
/// Returns file content (text or base64 data URL for images).
#[tauri::command]
pub async fn read_file_for_attachment(
    path: String,
    max_size: Option<usize>,
) -> Result<CommandResponse<FileContentResult>, String> {
    let file_path = PathBuf::from(&path);
    let max = max_size.unwrap_or(DEFAULT_MAX_SIZE);

    // Validate file exists
    if !file_path.exists() {
        return Ok(CommandResponse::err(format!("File not found: {}", path)));
    }

    if !file_path.is_file() {
        return Ok(CommandResponse::err(format!("Not a file: {}", path)));
    }

    // Check file size
    let metadata = std::fs::metadata(&file_path)
        .map_err(|e| format!("Failed to read file metadata: {}", e))?;
    let file_size = metadata.len() as usize;

    if file_size > max {
        return Ok(CommandResponse::err(format!(
            "File too large: {} bytes (max {} bytes)",
            file_size, max
        )));
    }

    // Read file bytes
    let data = std::fs::read(&file_path).map_err(|e| format!("Failed to read file: {}", e))?;

    let ext = get_extension(&file_path);
    let is_binary = is_binary_content(&data);

    if is_binary {
        // Check if it's a supported image type
        if is_image_extension(&ext) {
            let mime = mime_type_from_extension(&ext);
            // For SVG, it might not be binary but we handle it here too
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
            let data_url = format!("data:{};base64,{}", mime, b64);

            return Ok(CommandResponse::ok(FileContentResult {
                content: data_url,
                size: file_size,
                is_binary: true,
                mime_type: mime.to_string(),
            }));
        }

        return Ok(CommandResponse::err(
            "Unsupported binary file type".to_string(),
        ));
    }

    // Text file
    let content =
        String::from_utf8(data).map_err(|_| "File contains invalid UTF-8 encoding".to_string())?;

    let mime = mime_type_from_extension(&ext);

    Ok(CommandResponse::ok(FileContentResult {
        content,
        size: file_size,
        is_binary: false,
        mime_type: mime.to_string(),
    }))
}

/// List files in a workspace directory with optional search query.
/// Recurses up to max_depth=3, skips hidden and ignored directories.
#[tauri::command]
pub async fn list_workspace_files(
    path: String,
    query: Option<String>,
    max_results: Option<usize>,
) -> Result<CommandResponse<Vec<WorkspaceFileResult>>, String> {
    let dir_path = PathBuf::from(&path);
    let max = max_results.unwrap_or(50);
    let search_query = query.as_deref().map(|q| q.to_lowercase());

    if !dir_path.exists() {
        return Ok(CommandResponse::err(format!(
            "Directory not found: {}",
            path
        )));
    }

    if !dir_path.is_dir() {
        return Ok(CommandResponse::err(format!("Not a directory: {}", path)));
    }

    let mut results: Vec<WorkspaceFileResult> = Vec::new();

    fn walk_dir(
        dir: &Path,
        base: &Path,
        depth: usize,
        max_depth: usize,
        query: &Option<String>,
        results: &mut Vec<WorkspaceFileResult>,
        max_results: usize,
    ) {
        if depth > max_depth || results.len() >= max_results {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        for entry in entries {
            if results.len() >= max_results {
                break;
            }

            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files/directories
            if is_hidden(&name) {
                continue;
            }

            let entry_path = entry.path();
            let is_dir = entry_path.is_dir();

            // Skip ignored directories
            if is_dir && is_ignored_dir(&name) {
                continue;
            }

            // Apply query filter on file name (case-insensitive substring)
            if let Some(ref q) = query {
                if !name.to_lowercase().contains(q) {
                    // For directories, still recurse even if dir name doesn't match
                    if is_dir && depth < max_depth {
                        walk_dir(
                            &entry_path,
                            base,
                            depth + 1,
                            max_depth,
                            query,
                            results,
                            max_results,
                        );
                    }
                    continue;
                }
            }

            let size = if is_dir {
                0
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            };

            let relative_path = entry_path
                .strip_prefix(base)
                .unwrap_or(&entry_path)
                .to_string_lossy()
                .to_string();

            results.push(WorkspaceFileResult {
                name,
                path: relative_path,
                size,
                is_dir,
            });

            // Recurse into directories
            if is_dir && depth < max_depth {
                walk_dir(
                    &entry_path,
                    base,
                    depth + 1,
                    max_depth,
                    query,
                    results,
                    max_results,
                );
            }
        }
    }

    walk_dir(&dir_path, &dir_path, 0, 3, &search_query, &mut results, max);

    Ok(CommandResponse::ok(results))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension(Path::new("file.txt")), "txt");
        assert_eq!(get_extension(Path::new("file.RS")), "rs");
        assert_eq!(get_extension(Path::new("noext")), "");
    }

    #[test]
    fn test_mime_type_from_extension() {
        assert_eq!(mime_type_from_extension("png"), "image/png");
        assert_eq!(mime_type_from_extension("ts"), "text/typescript");
        assert_eq!(mime_type_from_extension("unknown"), "text/plain");
    }

    #[test]
    fn test_is_binary_content() {
        assert!(!is_binary_content(b"Hello, world!"));
        assert!(is_binary_content(&[0u8, 1, 2, 3]));
        assert!(!is_binary_content(b""));
    }

    #[test]
    fn test_is_image_extension() {
        assert!(is_image_extension("png"));
        assert!(is_image_extension("jpg"));
        assert!(!is_image_extension("txt"));
    }

    #[test]
    fn test_is_hidden() {
        assert!(is_hidden(".git"));
        assert!(!is_hidden("src"));
    }

    #[test]
    fn test_is_ignored_dir() {
        assert!(is_ignored_dir("node_modules"));
        assert!(is_ignored_dir("target"));
        assert!(!is_ignored_dir("src"));
    }
}
