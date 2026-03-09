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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AttachmentMetadataResult {
    pub name: String,
    pub path: String,
    pub size: usize,
    pub is_binary: bool,
    pub mime_type: String,
    #[serde(rename = "type")]
    pub attachment_type: String,
    pub is_previewable: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceFileResult {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceFileListV2Result {
    pub items: Vec<WorkspaceFileResult>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AttachmentContextInput {
    pub name: String,
    pub path: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub attachment_type: String,
    pub content: Option<String>,
    pub preview: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceReferenceInput {
    pub name: String,
    #[serde(alias = "relativePath")]
    pub relative_path: String,
    #[serde(alias = "absolutePath")]
    pub absolute_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreparedAttachmentSkip {
    pub name: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PrepareAttachmentContextResult {
    pub prepared_prompt: String,
    pub included_files: Vec<String>,
    pub skipped_files: Vec<PreparedAttachmentSkip>,
    pub prompt_tokens: usize,
    pub attachment_tokens: usize,
    pub total_tokens: usize,
    pub budget_tokens: usize,
    pub exceeds_budget: bool,
    pub truncated: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PromptTokenEstimateResult {
    pub estimated_tokens: usize,
    pub prompt_tokens: usize,
    pub attachment_tokens: usize,
    pub attachment_count: usize,
    pub budget_tokens: usize,
    pub remaining_tokens: isize,
    pub exceeds_budget: bool,
}

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_MAX_SIZE: usize = 10_485_760; // 10MB
const BINARY_CHECK_SIZE: usize = 8192;
const DEFAULT_TOKEN_BUDGET: usize = 160_000;
const DEFAULT_ATTACHMENT_BUDGET_RATIO: f64 = 0.4;
const DEFAULT_MAX_ATTACHMENT_TOKENS: usize = 64_000;
const DEFAULT_MAX_TOKENS_PER_FILE: usize = 12_000;
const DEFAULT_NON_TEXT_ATTACHMENT_TOKENS: usize = 48;
const DEFAULT_LIST_V2_MAX_SCAN: usize = 5_000;

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

fn estimate_tokens_rough(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    (text.chars().count() + 3) / 4
}

fn truncate_to_token_budget(text: &str, token_budget: usize) -> (String, bool) {
    let max_chars = token_budget.saturating_mul(4);
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return (text.to_string(), false);
    }

    let mut truncated = String::with_capacity(max_chars + 64);
    for ch in text.chars().take(max_chars) {
        truncated.push(ch);
    }
    truncated.push_str("\n\n[... truncated for context budget ...]");
    (truncated, true)
}

fn read_attachment_file(path: &str, max_size: usize) -> Result<FileContentResult, String> {
    let file_path = PathBuf::from(path);

    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    if !file_path.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    let metadata =
        std::fs::metadata(&file_path).map_err(|e| format!("Failed to read file metadata: {}", e))?;
    let file_size = metadata.len() as usize;

    if file_size > max_size {
        return Err(format!(
            "File too large: {} bytes (max {} bytes)",
            file_size, max_size
        ));
    }

    let data = std::fs::read(&file_path).map_err(|e| format!("Failed to read file: {}", e))?;
    let ext = get_extension(&file_path);
    let is_binary = is_binary_content(&data);
    let mime = mime_type_from_extension(&ext).to_string();

    if is_binary {
        if is_image_extension(&ext) {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
            let data_url = format!("data:{};base64,{}", mime, b64);
            return Ok(FileContentResult {
                content: data_url,
                size: file_size,
                is_binary: true,
                mime_type: mime,
            });
        }

        return Err("Unsupported binary file type".to_string());
    }

    let content =
        String::from_utf8(data).map_err(|_| "File contains invalid UTF-8 encoding".to_string())?;

    Ok(FileContentResult {
        content,
        size: file_size,
        is_binary: false,
        mime_type: mime,
    })
}

fn inspect_attachment_file(path: &str, max_size: usize) -> Result<AttachmentMetadataResult, String> {
    let file_path = PathBuf::from(path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }
    if !file_path.is_file() {
        return Err(format!("Not a file: {}", path));
    }
    let metadata =
        std::fs::metadata(&file_path).map_err(|e| format!("Failed to read file metadata: {}", e))?;
    let file_size = metadata.len() as usize;
    if file_size > max_size {
        return Err(format!(
            "File too large: {} bytes (max {} bytes)",
            file_size, max_size
        ));
    }

    let ext = get_extension(&file_path);
    let mime_type = mime_type_from_extension(&ext).to_string();
    let attachment_type = if is_image_extension(&ext) {
        "image"
    } else if ext == "pdf" {
        "pdf"
    } else {
        "text"
    };

    let is_binary = if attachment_type == "image" {
        true
    } else {
        let data = std::fs::read(&file_path).map_err(|e| format!("Failed to read file: {}", e))?;
        is_binary_content(&data)
    };

    Ok(AttachmentMetadataResult {
        name: file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path)
            .to_string(),
        path: path.to_string(),
        size: file_size,
        is_binary,
        mime_type,
        attachment_type: if is_binary && attachment_type != "image" {
            "unknown".to_string()
        } else {
            attachment_type.to_string()
        },
        is_previewable: attachment_type == "image",
    })
}

fn resolve_attachment_section(
    attachment: &AttachmentContextInput,
    per_file_budget: usize,
) -> Result<Option<(String, bool)>, String> {
    let normalized_type = attachment.attachment_type.to_lowercase();
    if normalized_type == "text" {
        let source_text = if let Some(content) = attachment.content.as_deref() {
            if content.is_empty() {
                None
            } else {
                Some(content.to_string())
            }
        } else if !attachment.path.is_empty() {
            Some(read_attachment_file(&attachment.path, DEFAULT_MAX_SIZE)?.content)
        } else {
            None
        };

        let Some(source_text) = source_text else {
            return Ok(None);
        };

        let (prepared, was_truncated) = truncate_to_token_budget(&source_text, per_file_budget);
        return Ok(Some((
            format!(
                "--- File: {} ---\n{}\n--- End of {} ---",
                attachment.name, prepared, attachment.name
            ),
            was_truncated,
        )));
    }

    if normalized_type == "image" {
        return Ok(Some((
            format!(
                "--- Attached image: {} ({} bytes) ---",
                attachment.name, attachment.size
            ),
            false,
        )));
    }

    if normalized_type == "pdf" {
        return Ok(Some((
            format!(
                "--- Attached PDF: {} ({} bytes) ---",
                attachment.name, attachment.size
            ),
            false,
        )));
    }

    Ok(Some((
        format!(
            "--- Attached file: {} ({}, {} bytes) ---",
            attachment.name, normalized_type, attachment.size
        ),
        false,
    )))
}

fn estimate_attachment_tokens(attachment: &AttachmentContextInput) -> usize {
    if !attachment.attachment_type.eq_ignore_ascii_case("text") {
        return DEFAULT_NON_TEXT_ATTACHMENT_TOKENS;
    }

    if let Some(content) = attachment.content.as_deref() {
        return estimate_tokens_rough(content);
    }

    if attachment.size > 0 {
        return estimate_tokens_rough(&"x".repeat((attachment.size as usize).min(32_000)));
    }

    if attachment.path.is_empty() {
        return DEFAULT_NON_TEXT_ATTACHMENT_TOKENS;
    }

    match read_attachment_file(&attachment.path, DEFAULT_MAX_SIZE) {
        Ok(file) => estimate_tokens_rough(&file.content),
        Err(_) => DEFAULT_NON_TEXT_ATTACHMENT_TOKENS,
    }
}

fn collect_workspace_files(
    dir_path: &Path,
    search_query: Option<&str>,
    max_depth: usize,
    max_results: usize,
) -> Vec<WorkspaceFileResult> {
    let mut results: Vec<WorkspaceFileResult> = Vec::new();

    fn walk_dir(
        dir: &Path,
        base: &Path,
        depth: usize,
        max_depth: usize,
        query: Option<&str>,
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
            if is_hidden(&name) {
                continue;
            }

            let entry_path = entry.path();
            let is_dir = entry_path.is_dir();

            if is_dir && is_ignored_dir(&name) {
                continue;
            }

            if let Some(q) = query {
                if !name.to_lowercase().contains(q) {
                    // Recurse into directories even if the directory name itself does not match.
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

    walk_dir(
        dir_path,
        dir_path,
        0,
        max_depth,
        search_query,
        &mut results,
        max_results,
    );
    results
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
    let max = max_size.unwrap_or(DEFAULT_MAX_SIZE);
    match read_attachment_file(&path, max) {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(error) => Ok(CommandResponse::err(error)),
    }
}

#[tauri::command]
pub async fn inspect_file_for_attachment(
    path: String,
    max_size: Option<usize>,
) -> Result<CommandResponse<AttachmentMetadataResult>, String> {
    let max = max_size.unwrap_or(DEFAULT_MAX_SIZE);
    match inspect_attachment_file(&path, max) {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(error) => Ok(CommandResponse::err(error)),
    }
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

    let results = collect_workspace_files(&dir_path, search_query.as_deref(), 3, max);

    Ok(CommandResponse::ok(results))
}

/// V2 workspace file listing with deterministic ordering and cursor pagination.
#[tauri::command]
pub async fn list_workspace_files_v2(
    path: String,
    query: Option<String>,
    page_size: Option<usize>,
    cursor: Option<String>,
) -> Result<CommandResponse<WorkspaceFileListV2Result>, String> {
    let dir_path = PathBuf::from(&path);
    if !dir_path.exists() {
        return Ok(CommandResponse::err(format!(
            "Directory not found: {}",
            path
        )));
    }
    if !dir_path.is_dir() {
        return Ok(CommandResponse::err(format!("Not a directory: {}", path)));
    }

    let size = page_size.unwrap_or(50).clamp(1, 200);
    let search_query = query.as_deref().map(|q| q.to_lowercase());
    let offset = match cursor.as_deref() {
        Some(raw) => raw
            .parse::<usize>()
            .map_err(|_| "Invalid cursor value".to_string())?,
        None => 0,
    };

    let mut all = collect_workspace_files(
        &dir_path,
        search_query.as_deref(),
        3,
        DEFAULT_LIST_V2_MAX_SCAN,
    );
    all.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));

    let total = all.len();
    let start = offset.min(total);
    let end = (start + size).min(total);

    let items = all[start..end].to_vec();
    let has_more = end < total;
    let next_cursor = has_more.then(|| end.to_string());

    Ok(CommandResponse::ok(WorkspaceFileListV2Result {
        items,
        next_cursor,
        has_more,
        total,
    }))
}

/// Rough token estimation for prompt + attachment payload.
#[tauri::command]
pub async fn estimate_prompt_tokens(
    prompt: String,
    attachments: Option<Vec<AttachmentContextInput>>,
    workspace_references: Option<Vec<WorkspaceReferenceInput>>,
    budget_tokens: Option<usize>,
) -> Result<CommandResponse<PromptTokenEstimateResult>, String> {
    let budget = budget_tokens.unwrap_or(DEFAULT_TOKEN_BUDGET);
    let prompt_tokens = estimate_tokens_rough(&prompt);

    let attachment_tokens: usize = attachments
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(estimate_attachment_tokens)
        .sum();
    let reference_tokens = workspace_references
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|reference| estimate_tokens_rough(&reference.relative_path).max(8))
        .sum::<usize>();

    let estimated_tokens = prompt_tokens + attachment_tokens + reference_tokens;
    let remaining_tokens = budget as isize - estimated_tokens as isize;

    Ok(CommandResponse::ok(PromptTokenEstimateResult {
        estimated_tokens,
        prompt_tokens,
        attachment_tokens: attachment_tokens + reference_tokens,
        attachment_count: attachments.as_ref().map(|a| a.len()).unwrap_or(0)
            + workspace_references.as_ref().map(|r| r.len()).unwrap_or(0),
        budget_tokens: budget,
        remaining_tokens,
        exceeds_budget: estimated_tokens > budget,
    }))
}

/// Prepare attachment context under a fixed token budget and return the final prompt.
#[tauri::command]
pub async fn prepare_attachment_context(
    prompt: String,
    attachments: Vec<AttachmentContextInput>,
    workspace_references: Option<Vec<WorkspaceReferenceInput>>,
    budget_tokens: Option<usize>,
    max_attachment_tokens: Option<usize>,
    max_tokens_per_file: Option<usize>,
) -> Result<CommandResponse<PrepareAttachmentContextResult>, String> {
    let budget = budget_tokens.unwrap_or(DEFAULT_TOKEN_BUDGET);
    let prompt_tokens = estimate_tokens_rough(&prompt);

    let default_attachment_budget = (budget as f64 * DEFAULT_ATTACHMENT_BUDGET_RATIO) as usize;
    let configured_attachment_budget = max_attachment_tokens.unwrap_or(
        default_attachment_budget
            .max(DEFAULT_MAX_ATTACHMENT_TOKENS / 2)
            .min(DEFAULT_MAX_ATTACHMENT_TOKENS),
    );

    let per_file_budget = max_tokens_per_file.unwrap_or(DEFAULT_MAX_TOKENS_PER_FILE);
    let available_for_attachments =
        configured_attachment_budget.min(budget.saturating_sub(prompt_tokens));

    let mut remaining_attachment_budget = available_for_attachments;
    let mut sections: Vec<String> = Vec::new();
    let mut included_files: Vec<String> = Vec::new();
    let mut skipped_files: Vec<PreparedAttachmentSkip> = Vec::new();
    let mut attachment_tokens = 0usize;
    let mut truncated = false;

    for attachment in attachments.iter() {
        let identifier = if !attachment.path.is_empty() {
            attachment.path.clone()
        } else {
            attachment.name.clone()
        };

        let section = match resolve_attachment_section(attachment, per_file_budget) {
            Ok(result) => result,
            Err(error) => {
                skipped_files.push(PreparedAttachmentSkip {
                    name: attachment.name.clone(),
                    path: attachment.path.clone(),
                    reason: error,
                });
                continue;
            }
        };

        let Some((section_text, was_truncated)) = section else {
            skipped_files.push(PreparedAttachmentSkip {
                name: attachment.name.clone(),
                path: attachment.path.clone(),
                reason: "missing_content".to_string(),
            });
            continue;
        };
        truncated |= was_truncated;

        let section_tokens = estimate_tokens_rough(&section_text);
        if section_tokens > remaining_attachment_budget {
            skipped_files.push(PreparedAttachmentSkip {
                name: attachment.name.clone(),
                path: attachment.path.clone(),
                reason: "budget_exceeded".to_string(),
            });
            continue;
        }

        remaining_attachment_budget = remaining_attachment_budget.saturating_sub(section_tokens);
        attachment_tokens += section_tokens;
        included_files.push(identifier);
        sections.push(section_text);
    }

    let prepared_prompt = if sections.is_empty() {
        prompt
    } else {
        format!("{}\n\n{}", sections.join("\n\n"), prompt)
    };
    let prepared_prompt = if let Some(references) = workspace_references.as_ref() {
        if references.is_empty() {
            prepared_prompt
        } else {
            let reference_block = [
                "--- Referenced workspace files ---".to_string(),
                references
                    .iter()
                    .map(|reference| format!("- {} ({})", reference.relative_path, reference.name))
                    .collect::<Vec<_>>()
                    .join("\n"),
                "--- End referenced workspace files ---".to_string(),
            ]
            .join("\n");
            format!("{}\n\n{}", reference_block, prepared_prompt)
        }
    } else {
        prepared_prompt
    };

    let total_tokens = estimate_tokens_rough(&prepared_prompt);

    Ok(CommandResponse::ok(PrepareAttachmentContextResult {
        prepared_prompt,
        included_files,
        skipped_files,
        prompt_tokens,
        attachment_tokens,
        total_tokens,
        budget_tokens: budget,
        exceeds_budget: total_tokens > budget,
        truncated,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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

    #[test]
    fn test_estimate_tokens_rough() {
        assert_eq!(estimate_tokens_rough(""), 0);
        assert_eq!(estimate_tokens_rough("abcd"), 1);
        assert_eq!(estimate_tokens_rough("abcde"), 2);
    }

    #[test]
    fn test_truncate_to_token_budget() {
        let source = "a".repeat(120);
        let (truncated, was_truncated) = truncate_to_token_budget(&source, 10);
        assert!(was_truncated);
        assert!(truncated.contains("truncated for context budget"));
        assert!(truncated.len() < source.len() + 64);
    }

    #[tokio::test]
    async fn test_list_workspace_files_v2_pagination_sorted() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path();
        fs::write(root.join("b.txt"), "b").expect("write b");
        fs::write(root.join("a.txt"), "a").expect("write a");
        fs::write(root.join("c.txt"), "c").expect("write c");
        fs::write(root.join(".hidden"), "hidden").expect("write hidden");

        let first =
            list_workspace_files_v2(root.to_string_lossy().to_string(), None, Some(2), None)
                .await
                .expect("command should resolve");
        assert!(first.success);
        let first_data = first.data.expect("first page data");
        assert_eq!(first_data.total, 3);
        assert_eq!(first_data.items.len(), 2);
        assert!(first_data.has_more);
        assert_eq!(
            first_data
                .items
                .iter()
                .map(|item| item.path.clone())
                .collect::<Vec<_>>(),
            vec!["a.txt".to_string(), "b.txt".to_string()]
        );

        let second = list_workspace_files_v2(
            root.to_string_lossy().to_string(),
            None,
            Some(2),
            first_data.next_cursor.clone(),
        )
        .await
        .expect("command should resolve");
        assert!(second.success);
        let second_data = second.data.expect("second page data");
        assert_eq!(second_data.items.len(), 1);
        assert!(!second_data.has_more);
        assert_eq!(second_data.next_cursor, None);
        assert_eq!(second_data.items[0].path, "c.txt");
    }

    #[tokio::test]
    async fn test_prepare_attachment_context_budget_and_truncation() {
        let prompt = "Please summarize these files".to_string();
        let attachments = vec![
            AttachmentContextInput {
                name: "first.txt".to_string(),
                path: "src/first.txt".to_string(),
                size: 120,
                attachment_type: "text".to_string(),
                content: Some("a".repeat(120)),
                preview: None,
            },
            AttachmentContextInput {
                name: "second.txt".to_string(),
                path: "src/second.txt".to_string(),
                size: 120,
                attachment_type: "text".to_string(),
                content: Some("b".repeat(120)),
                preview: None,
            },
        ];

        let response = prepare_attachment_context(
            prompt.clone(),
            attachments,
            Some(vec![WorkspaceReferenceInput {
                name: "first.txt".to_string(),
                relative_path: "src/first.txt".to_string(),
                absolute_path: "/tmp/src/first.txt".to_string(),
            }]),
            Some(120),
            Some(50),
            Some(10),
        )
                .await
                .expect("command should resolve");
        assert!(response.success);

        let payload = response.data.expect("payload should exist");
        assert!(payload.truncated);
        assert_eq!(payload.included_files, vec!["src/first.txt".to_string()]);
        assert_eq!(payload.skipped_files.len(), 1);
        assert_eq!(payload.skipped_files[0].reason, "budget_exceeded");
        assert!(payload
            .prepared_prompt
            .contains("--- Referenced workspace files ---"));
        assert!(payload.prepared_prompt.contains("--- File: first.txt ---"));
        assert!(payload.prepared_prompt.ends_with(&prompt));
        assert!(payload.attachment_tokens > 0);
    }
}
