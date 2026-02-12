//! Tool Executor
//!
//! Executes tools requested by LLM providers.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::process::Command;
use tokio::time::timeout;

use crate::services::orchestrator::embedding_service::EmbeddingService;
use crate::services::orchestrator::index_store::IndexStore;

/// Cache entry for a previously read file, used for deduplication.
///
/// ADR-F001: Uses `Mutex<HashMap>` over `mini-moka` for deterministic behavior,
/// low cardinality (<100 files), and zero additional dependencies.
#[derive(Debug, Clone)]
pub struct ReadCacheEntry {
    /// Canonical path of the cached file
    pub path: PathBuf,
    /// File modification time at the time of caching
    pub modified_time: SystemTime,
    /// Number of lines in the file
    pub line_count: usize,
    /// Size of the file in bytes
    pub size_bytes: u64,
    /// Hash of the file content (using std DefaultHasher for speed, not crypto)
    pub content_hash: u64,
    /// Offset (1-based line number) used when the file was read
    pub offset: usize,
    /// Line limit used when the file was read
    pub limit: usize,
    /// File extension (e.g. "rs", "py", "ts") for the enhanced dedup message
    pub extension: String,
    /// First ~5 lines of the file content for the enhanced dedup message
    pub first_lines_preview: String,
}

/// Result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the execution was successful
    pub success: bool,
    /// Output from the tool (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Optional image data for multimodal responses: (mime_type, base64_data)
    #[serde(skip)]
    pub image_data: Option<(String, String)>,
}

impl ToolResult {
    /// Create a successful result
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            image_data: None,
        }
    }

    /// Create an error result
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error.into()),
            image_data: None,
        }
    }

    /// Create a successful result with image data for multimodal support
    pub fn ok_with_image(
        output: impl Into<String>,
        mime_type: String,
        base64_data: String,
    ) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            image_data: Some((mime_type, base64_data)),
        }
    }

    /// Convert to string for LLM consumption
    pub fn to_content(&self) -> String {
        if self.success {
            self.output.clone().unwrap_or_default()
        } else {
            format!(
                "Error: {}",
                self.error.as_deref().unwrap_or("Unknown error")
            )
        }
    }
}

/// Generate a "missing parameter" error with a format hint.
///
/// When an LLM uses prompt-fallback tool calling and gets the format wrong,
/// this error message teaches it the correct format for the next retry.
fn missing_param_error(tool: &str, param: &str) -> String {
    let example = match (tool, param) {
        ("Read", "file_path") => {
            r#"```tool_call
{"tool": "Read", "arguments": {"file_path": "path/to/file"}}
```"#
        }
        ("LS", "path") => {
            r#"```tool_call
{"tool": "LS", "arguments": {"path": "."}}
```"#
        }
        ("Bash", "command") => {
            r#"```tool_call
{"tool": "Bash", "arguments": {"command": "your command here"}}
```"#
        }
        ("Glob", "pattern") => {
            r#"```tool_call
{"tool": "Glob", "arguments": {"pattern": "**/*.rs"}}
```"#
        }
        ("Grep", "pattern") => {
            r#"```tool_call
{"tool": "Grep", "arguments": {"pattern": "search_term"}}
```"#
        }
        ("Write", "file_path") => {
            r#"```tool_call
{"tool": "Write", "arguments": {"file_path": "path/to/file", "content": "file content"}}
```"#
        }
        _ => return format!("Missing required parameter: {param}"),
    };
    format!("Missing required parameter: {param}. Correct format:\n{example}")
}

/// Blocked bash commands for security
const BLOCKED_COMMANDS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    "rm -rf ~/",
    "> /dev/sda",
    "dd if=/dev/zero",
    "mkfs.",
    ":(){ :|:& };:",
    "chmod -R 777 /",
    "chown -R",
];

/// Directories excluded from default full-workspace scans.
/// These are skipped only when callers do not provide an explicit search path.
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

/// Maximum number of entries to display in LS output.
/// Directories exceeding this limit are truncated with a note suggesting Glob.
const LS_MAX_ENTRIES: usize = 200;

fn is_likely_text_extension(ext: &str) -> bool {
    matches!(
        ext,
        "txt"
            | "md"
            | "markdown"
            | "rst"
            | "json"
            | "jsonl"
            | "yaml"
            | "yml"
            | "toml"
            | "ini"
            | "cfg"
            | "conf"
            | "lock"
            | "env"
            | "gitignore"
            | "gitattributes"
            | "py"
            | "rs"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "java"
            | "kt"
            | "go"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "swift"
            | "scala"
            | "sql"
            | "sh"
            | "bash"
            | "ps1"
            | "zsh"
            | "fish"
            | "xml"
            | "html"
            | "htm"
            | "css"
            | "scss"
            | "less"
            | "svg"
            | "vue"
            | "svelte"
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

/// Tool executor for running tools locally
pub struct ToolExecutor {
    /// Project root for path validation
    project_root: PathBuf,
    /// Default timeout for bash commands (in milliseconds)
    default_timeout: u64,
    /// Track files that have been read (for read-before-write enforcement)
    read_files: Mutex<HashSet<PathBuf>>,
    /// Content-aware deduplication cache for file reads.
    /// Key: (canonical path, offset, limit) -> cache entry.
    /// If the same file is read with the same offset/limit and the mtime
    /// has not changed, a short dedup message is returned instead of re-reading.
    /// ADR-F001: Mutex<HashMap> chosen over mini-moka for determinism and zero eviction.
    read_cache: Mutex<HashMap<(PathBuf, usize, usize), ReadCacheEntry>>,
    /// Persistent working directory for Bash commands
    current_working_dir: Mutex<PathBuf>,
    /// WebFetch service for fetching web pages
    web_fetch: super::web_fetch::WebFetchService,
    /// WebSearch service (None if no search provider configured)
    web_search: Option<super::web_search::WebSearchService>,
    /// Optional index store for CodebaseSearch tool
    index_store: Option<Arc<IndexStore>>,
    /// Optional embedding service for semantic search in CodebaseSearch
    embedding_service: Option<Arc<EmbeddingService>>,
}

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = project_root.into();
        Self {
            current_working_dir: Mutex::new(root.clone()),
            project_root: root,
            default_timeout: 120_000, // 2 minutes
            read_files: Mutex::new(HashSet::new()),
            read_cache: Mutex::new(HashMap::new()),
            web_fetch: super::web_fetch::WebFetchService::new(),
            web_search: None,
            index_store: None,
            embedding_service: None,
        }
    }

    /// Configure the web search provider
    pub fn set_search_provider(&mut self, provider_name: &str, api_key: Option<String>) {
        match super::web_search::WebSearchService::new(provider_name, api_key.as_deref()) {
            Ok(service) => self.web_search = Some(service),
            Err(e) => {
                tracing::warn!(
                    "Failed to configure search provider '{}': {}",
                    provider_name,
                    e
                );
                self.web_search = None;
            }
        }
    }

    /// Set the index store for CodebaseSearch tool
    pub fn set_index_store(&mut self, store: Arc<IndexStore>) {
        self.index_store = Some(store);
    }

    /// Set the embedding service for semantic search in CodebaseSearch
    pub fn set_embedding_service(&mut self, svc: Arc<EmbeddingService>) {
        self.embedding_service = Some(svc);
    }

    /// Execute a tool by name with given arguments
    pub async fn execute(&self, tool_name: &str, arguments: &serde_json::Value) -> ToolResult {
        match tool_name {
            "Read" => self.execute_read(arguments).await,
            "Write" => self.execute_write(arguments).await,
            "Edit" => self.execute_edit(arguments).await,
            "Bash" => self.execute_bash(arguments).await,
            "Glob" => self.execute_glob(arguments).await,
            "Grep" => self.execute_grep(arguments).await,
            "LS" => self.execute_ls(arguments).await,
            "Cwd" => self.execute_cwd(arguments).await,
            "WebFetch" => self.execute_web_fetch(arguments).await,
            "WebSearch" => self.execute_web_search(arguments).await,
            "NotebookEdit" => self.execute_notebook_edit(arguments).await,
            "CodebaseSearch" => self.execute_codebase_search(arguments).await,
            _ => ToolResult::err(format!("Unknown tool: {}", tool_name)),
        }
    }

    /// Validate and resolve a file path
    fn validate_path(&self, path: &str) -> Result<PathBuf, String> {
        let path = Path::new(path);

        // Convert to absolute path if relative (use current_working_dir for resolution)
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let base = self
                .current_working_dir
                .lock()
                .map(|cwd| cwd.clone())
                .unwrap_or_else(|_| self.project_root.clone());
            base.join(path)
        };

        // Canonicalize to resolve symlinks and .. components
        // Note: File must exist for canonicalize, so we check parent for new files
        let check_path = if abs_path.exists() {
            abs_path.clone()
        } else if let Some(parent) = abs_path.parent() {
            if parent.exists() {
                parent.to_path_buf()
            } else {
                // Parent doesn't exist either, allow it (Write will create directories)
                return Ok(abs_path);
            }
        } else {
            return Ok(abs_path);
        };

        // Check for path traversal
        match check_path.canonicalize() {
            Ok(_canonical) => {
                // Verify the path is within project root (optional - can be removed if too restrictive)
                // For now, just return the path
                Ok(abs_path)
            }
            Err(e) => Err(format!("Invalid path: {}", e)),
        }
    }

    /// Execute Read tool
    async fn execute_read(&self, args: &serde_json::Value) -> ToolResult {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err(missing_param_error("Read", "file_path")),
        };

        let path = match self.validate_path(file_path) {
            Ok(p) => p,
            Err(e) => return ToolResult::err(e),
        };

        if !path.exists() {
            return ToolResult::err(format!("File not found: {}", file_path));
        }

        // Track this file as read (for all file types)
        if let Ok(mut read_files) = self.read_files.lock() {
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
                match super::file_parsers::parse_pdf(&path, pages) {
                    Ok(content) => return ToolResult::ok(content),
                    Err(e) => return ToolResult::err(e),
                }
            }
            "ipynb" => match super::file_parsers::parse_jupyter(&path) {
                Ok(content) => return ToolResult::ok(content),
                Err(e) => return parser_error_result(e),
            },
            "docx" => match super::file_parsers::parse_docx(&path) {
                Ok(content) => return ToolResult::ok(content),
                Err(e) => return parser_error_result(e),
            },
            "xlsx" | "xls" | "ods" => match super::file_parsers::parse_xlsx(&path) {
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
                let metadata = match super::file_parsers::read_image_metadata(&path) {
                    Ok(m) => m,
                    Err(e) => return ToolResult::err(e),
                };
                // Try to encode as base64 for multimodal support
                match super::file_parsers::encode_image_base64(&path) {
                    Ok((mime, b64)) => return ToolResult::ok_with_image(metadata, mime, b64),
                    Err(_) => return ToolResult::ok(metadata),
                }
            }
            _ => { /* fall through to regular text reading */ }
        }

        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;

        // --- Read cache deduplication check ---
        // If we have a cached entry for this exact (path, offset, limit) and
        // the file modification time has not changed, return a short dedup message.
        let current_mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());

        let cache_key = (path.clone(), offset, limit);

        if let Some(mtime) = current_mtime {
            if let Ok(cache) = self.read_cache.lock() {
                if let Some(entry) = cache.get(&cache_key) {
                    if entry.modified_time == mtime {
                        // File unchanged with same offset/limit — return enhanced dedup message
                        // that includes file type, key symbols, and a preview so weak LLMs
                        // have enough context to proceed without re-reading.
                        let file_type = if entry.extension.is_empty() {
                            "unknown".to_string()
                        } else {
                            entry.extension.clone()
                        };

                        // Query IndexStore for key symbols if available
                        let symbols_section = self.get_dedup_symbols_summary(&path);

                        let preview_section = if entry.first_lines_preview.is_empty() {
                            String::new()
                        } else {
                            format!("\nPreview (first lines):\n{}", entry.first_lines_preview)
                        };

                        return ToolResult::ok(format!(
                            "[File previously read in this session, unchanged] Path: {} ({} lines, {} bytes, type: {}){}{}\n\
                             Do NOT re-read this file. Use the information above to proceed.",
                            path.display(),
                            entry.line_count,
                            entry.size_bytes,
                            file_type,
                            symbols_section,
                            preview_section,
                        ));
                    }
                    // mtime differs — fall through to re-read (entry will be replaced below)
                }
            }
        }

        // If mtime changed, clear any stale cache entry for this key before re-reading
        if let Ok(mut cache) = self.read_cache.lock() {
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
                            while end > 0 && !line.is_char_boundary(end) { end -= 1; }
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

                    // Build first ~5 lines preview for enhanced dedup messages
                    let first_lines_preview: String = all_lines
                        .iter()
                        .take(5)
                        .map(|l| {
                            if l.len() > 120 { let mut end = 120; while end > 0 && !l.is_char_boundary(end) { end -= 1; } format!("{}...", &l[..end]) } else { l.to_string() }
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

                    if let Ok(mut cache) = self.read_cache.lock() {
                        cache.insert(cache_key, entry);
                    }
                }

                ToolResult::ok(numbered_lines.join("\n"))
            }
            Err(e) => ToolResult::err(format!("Failed to read file: {}", e)),
        }
    }

    /// Execute Write tool
    async fn execute_write(&self, args: &serde_json::Value) -> ToolResult {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err(missing_param_error("Write", "file_path")),
        };

        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err(missing_param_error("Write", "content")),
        };

        let path = match self.validate_path(file_path) {
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

    /// Execute Edit tool
    async fn execute_edit(&self, args: &serde_json::Value) -> ToolResult {
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

        let path = match self.validate_path(file_path) {
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
            None => {
                return ToolResult::err(format!(
                    "Cannot edit binary file: {}",
                    path.display()
                ))
            }
        };

        // Check if old_string exists
        let occurrences = content.matches(old_string).count();
        if occurrences == 0 {
            return ToolResult::err(format!(
                "String not found in file. The old_string must exist in the file."
            ));
        }

        // Check uniqueness if not replace_all
        if !replace_all && occurrences > 1 {
            return ToolResult::err(format!(
                "The old_string appears {} times in the file. Either provide more context to make it unique, or set replace_all to true.",
                occurrences
            ));
        }

        // Perform replacement
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

    /// Execute Bash tool
    async fn execute_bash(&self, args: &serde_json::Value) -> ToolResult {
        let command = match args.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err(missing_param_error("Bash", "command")),
        };

        let timeout_ms = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.default_timeout)
            .min(600_000); // Max 10 minutes

        let working_dir = args
            .get("working_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                self.current_working_dir
                    .lock()
                    .map(|cwd| cwd.clone())
                    .unwrap_or_else(|_| self.project_root.clone())
            });

        // Check for blocked commands
        for blocked in BLOCKED_COMMANDS {
            if command.contains(blocked) {
                return ToolResult::err(format!(
                    "Command blocked for safety: contains '{}'",
                    blocked
                ));
            }
        }

        // Determine shell based on platform
        #[cfg(windows)]
        let (shell, shell_arg) = ("cmd", "/C");
        #[cfg(not(windows))]
        let (shell, shell_arg) = ("sh", "-c");

        let mut cmd = Command::new(shell);
        cmd.arg(shell_arg)
            .arg(command)
            .current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let result = timeout(Duration::from_millis(timeout_ms), cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                let mut result_text = String::new();

                if !stdout.is_empty() {
                    result_text.push_str(&stdout);
                }

                if !stderr.is_empty() {
                    if !result_text.is_empty() {
                        result_text.push_str("\n\n--- stderr ---\n");
                    }
                    result_text.push_str(&stderr);
                }

                // Truncate at 30,000 chars
                if result_text.len() > 30_000 {
                    result_text.truncate(30_000);
                    result_text.push_str("\n\n... (output truncated)");
                }

                // Detect simple `cd <path>` and update persistent working directory
                if output.status.success() {
                    self.detect_cd_command(command, &working_dir);
                }

                if output.status.success() {
                    ToolResult::ok(if result_text.is_empty() {
                        "Command completed successfully with no output".to_string()
                    } else {
                        result_text
                    })
                } else {
                    let exit_code = output.status.code().unwrap_or(-1);
                    ToolResult::err(format!(
                        "Command failed with exit code {}\n{}",
                        exit_code, result_text
                    ))
                }
            }
            Ok(Err(e)) => ToolResult::err(format!("Failed to execute command: {}", e)),
            Err(_) => ToolResult::err(format!("Command timed out after {} ms", timeout_ms)),
        }
    }

    /// Execute Glob tool
    async fn execute_glob(&self, args: &serde_json::Value) -> ToolResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err(missing_param_error("Glob", "pattern")),
        };
        let head_limit = args.get("head_limit").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        let explicit_base_path = args.get("path").and_then(|v| v.as_str());
        let base_path = match explicit_base_path {
            Some(path) => match self.validate_path(path) {
                Ok(resolved) => resolved,
                Err(err) => return ToolResult::err(err),
            },
            None => self
                .current_working_dir
                .lock()
                .map(|cwd| cwd.clone())
                .unwrap_or_else(|_| self.project_root.clone()),
        };
        let apply_default_excludes = explicit_base_path
            .map(|p| {
                let normalized = p.trim().replace('\\', "/");
                normalized == "." || normalized == "./"
            })
            .unwrap_or(true);

        // Combine base path with pattern
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

                // Sort by modification time (newest first)
                matches.sort_by(|a, b| b.1.cmp(&a.1));

                let result: Vec<String> = matches
                    .iter()
                    .filter(|(p, _)| {
                        !apply_default_excludes || !is_default_scan_excluded(&base_path, p)
                    })
                    .take(if head_limit > 0 {
                        head_limit
                    } else {
                        usize::MAX
                    })
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

    /// Execute Grep tool using ignore crate for .gitignore-aware file walking
    async fn execute_grep(&self, args: &serde_json::Value) -> ToolResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err(missing_param_error("Grep", "pattern")),
        };

        let explicit_search_path = args.get("path").and_then(|v| v.as_str());
        let search_path = match explicit_search_path {
            Some(path) => match self.validate_path(path) {
                Ok(resolved) => resolved,
                Err(err) => return ToolResult::err(err),
            },
            None => self
                .current_working_dir
                .lock()
                .map(|cwd| cwd.clone())
                .unwrap_or_else(|_| self.project_root.clone()),
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

        // Build regex
        let regex = match regex::RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .build()
        {
            Ok(r) => r,
            Err(e) => return ToolResult::err(format!("Invalid regex pattern: {}", e)),
        };

        // Build glob matcher for file filtering
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

        // Use ignore crate walker for .gitignore-aware traversal
        if search_path.is_file() {
            // Search single file
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
                .hidden(true) // skip hidden files
                .git_ignore(true) // respect .gitignore
                .git_global(true) // respect global gitignore
                .git_exclude(true) // respect .git/info/exclude
                .build();

            for entry in walker.flatten() {
                if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    continue;
                }

                let path = entry.path();

                if apply_default_excludes && is_default_scan_excluded(&search_path, path) {
                    continue;
                }

                // Apply glob filter if provided
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

                // Stop if we've hit output limit
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
            Err(_) => return, // Skip files that can't be read (binary, permission denied, etc.)
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
                    // Only need to know the file matches — emit once and break
                    let entry = path.display().to_string();
                    *total_output_len += entry.len() + 1;
                    results.push(entry);
                    *result_count += 1;
                    return;
                }
                "count" => {
                    // Count matches per file — continue counting, emit at end
                }
                _ => {
                    // "content" mode — emit matching lines with context
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

        // Emit count result
        if output_mode == "count" && file_matched {
            let entry = format!("{}:{}", path.display(), file_match_count);
            *total_output_len += entry.len() + 1;
            results.push(entry);
            *result_count += 1;
        }
    }

    /// Execute LS tool - list directory contents
    async fn execute_ls(&self, args: &serde_json::Value) -> ToolResult {
        let dir_path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err(missing_param_error("LS", "path")),
        };

        let show_hidden = args
            .get("show_hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let path = match self.validate_path(dir_path) {
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

                    // Skip hidden files unless show_hidden is true
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

                // Sort: directories first, then alphabetically
                items.sort_by(|a, b| {
                    b.1.cmp(&a.1)
                        .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
                });

                if items.is_empty() {
                    return ToolResult::ok(format!("Directory is empty: {}", path.display()));
                }

                // Compute counts before potential truncation
                let total_count = items.len();
                let total_dirs = items.iter().filter(|i| i.1).count();
                let total_files = items.iter().filter(|i| !i.1).count();

                // Truncate if directory has too many entries
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

    /// Execute Cwd tool - return current working directory
    async fn execute_cwd(&self, _args: &serde_json::Value) -> ToolResult {
        let cwd = self
            .current_working_dir
            .lock()
            .map(|cwd| cwd.to_string_lossy().to_string())
            .unwrap_or_else(|_| self.project_root.to_string_lossy().to_string());
        ToolResult::ok(cwd)
    }

    /// Detect simple `cd <path>` commands and update persistent working directory
    fn detect_cd_command(&self, command: &str, working_dir: &Path) {
        let trimmed = command.trim();

        // Only handle simple `cd <path>` — not chained commands with && or ;
        if trimmed.contains("&&") || trimmed.contains(';') || trimmed.contains('|') {
            return;
        }

        if let Some(target) = trimmed.strip_prefix("cd ") {
            let target = target.trim().trim_matches('"').trim_matches('\'');
            if target.is_empty() {
                return;
            }

            let target_path = if Path::new(target).is_absolute() {
                PathBuf::from(target)
            } else {
                working_dir.join(target)
            };

            // Only update if the resolved directory exists
            if let Ok(canonical) = target_path.canonicalize() {
                if canonical.is_dir() {
                    if let Ok(mut cwd) = self.current_working_dir.lock() {
                        *cwd = canonical;
                    }
                }
            }
        }
    }

    /// Execute WebFetch tool
    async fn execute_web_fetch(&self, args: &serde_json::Value) -> ToolResult {
        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => return ToolResult::err("Missing required parameter: url"),
        };

        let prompt = args.get("prompt").and_then(|v| v.as_str());

        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30)
            .min(60);

        match self.web_fetch.fetch(url, Some(timeout_secs)).await {
            Ok(content) => {
                let mut output = String::new();
                if let Some(p) = prompt {
                    output.push_str(&format!("## Fetched: {}\n### Context: {}\n\n", url, p));
                } else {
                    output.push_str(&format!("## Fetched: {}\n\n", url));
                }
                output.push_str(&content);
                ToolResult::ok(output)
            }
            Err(e) => ToolResult::err(e),
        }
    }

    /// Execute WebSearch tool
    async fn execute_web_search(&self, args: &serde_json::Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return ToolResult::err("Missing required parameter: query"),
        };

        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .min(10) as u32;

        match &self.web_search {
            Some(service) => match service.search(query, Some(max_results)).await {
                Ok(content) => ToolResult::ok(content),
                Err(e) => ToolResult::err(e),
            },
            None => ToolResult::err(
                "WebSearch is not configured. Set a search provider (tavily, brave, or duckduckgo) in Settings > LLM Backend > Search Provider, and provide an API key if required."
            ),
        }
    }

    /// Execute NotebookEdit tool
    async fn execute_notebook_edit(&self, args: &serde_json::Value) -> ToolResult {
        let notebook_path = match args.get("notebook_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: notebook_path"),
        };

        let cell_index = match args.get("cell_index").and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => return ToolResult::err("Missing required parameter: cell_index"),
        };

        let operation = match args.get("operation").and_then(|v| v.as_str()) {
            Some(o) => o,
            None => return ToolResult::err("Missing required parameter: operation"),
        };

        let cell_type = args.get("cell_type").and_then(|v| v.as_str());
        let new_source = args.get("new_source").and_then(|v| v.as_str());

        let path = match self.validate_path(notebook_path) {
            Ok(p) => p,
            Err(e) => return ToolResult::err(e),
        };

        // Enforce read-before-write for existing notebooks
        if path.exists() {
            if let Ok(read_files) = self.read_files.lock() {
                if !read_files.contains(&path) {
                    return ToolResult::err(
                        "You must read a notebook before editing it. Use the Read tool first.",
                    );
                }
            }
        }

        match super::notebook_edit::edit_notebook(
            &path, cell_index, operation, cell_type, new_source,
        ) {
            Ok(msg) => ToolResult::ok(msg),
            Err(e) => ToolResult::err(e),
        }
    }

    /// Execute CodebaseSearch tool — query the SQLite index for symbols, files, or both
    async fn execute_codebase_search(&self, args: &serde_json::Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return ToolResult::err("Missing required parameter: query"),
        };

        let scope = args
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let component = args.get("component").and_then(|v| v.as_str());

        let index_store = match &self.index_store {
            Some(store) => store,
            None => {
                return ToolResult::ok(
                    "Codebase index not available. The project has not been indexed yet. \
                     Use Grep for content search or Glob/LS for file discovery instead."
                );
            }
        };

        let project_path = self.project_root.to_string_lossy().to_string();
        let mut output_sections: Vec<String> = Vec::new();

        // --- Symbol search ---
        if scope == "symbols" || scope == "all" {
            let pattern = format!("%{}%", query);
            match index_store.query_symbols(&pattern) {
                Ok(symbols) => {
                    // If component filter is specified, filter results
                    let filtered: Vec<_> = if let Some(comp) = component {
                        symbols.into_iter().filter(|s| s.file_path.contains(comp) || s.project_path == project_path).collect()
                    } else {
                        symbols
                    };

                    if !filtered.is_empty() {
                        let mut section = format!("## Symbols matching '{}' ({} results)\n", query, filtered.len());
                        for sym in filtered.iter().take(50) {
                            // Base info: name, kind, file, line
                            let mut line = format!(
                                "  {} ({}) — {}:{}",
                                sym.symbol_name, sym.symbol_kind, sym.file_path, sym.line_number
                            );
                            // Add end line range if available
                            if sym.end_line > 0 && sym.end_line != sym.line_number {
                                line.push_str(&format!("-{}", sym.end_line));
                            }
                            // Add parent context if available
                            if let Some(ref parent) = sym.parent_symbol {
                                line.push_str(&format!(" [in {}]", parent));
                            }
                            line.push('\n');
                            section.push_str(&line);
                            // Add signature on its own line if available
                            if let Some(ref sig) = sym.signature {
                                section.push_str(&format!("    sig: {}\n", sig));
                            }
                            // Add doc comment (truncated) if available
                            if let Some(ref doc) = sym.doc_comment {
                                let truncated = if doc.len() > 100 {
                                    let mut end = 100;
                                    while end > 0 && !doc.is_char_boundary(end) { end -= 1; }
                                    format!("{}...", &doc[..end])
                                } else {
                                    doc.clone()
                                };
                                section.push_str(&format!("    doc: {}\n", truncated));
                            }
                        }
                        if filtered.len() > 50 {
                            section.push_str(&format!("  ... and {} more\n", filtered.len() - 50));
                        }
                        output_sections.push(section);
                    } else if scope == "symbols" {
                        output_sections.push(format!("No symbols matching '{}'.", query));
                    }
                }
                Err(e) => {
                    output_sections.push(format!("Symbol search error: {}", e));
                }
            }
        }

        // --- File search ---
        if scope == "files" || scope == "all" {
            if let Some(comp) = component {
                // Search files by component
                match index_store.query_files_by_component(&project_path, comp) {
                    Ok(files) => {
                        // Filter by query pattern (case-insensitive substring match on path)
                        let query_lower = query.to_lowercase();
                        let filtered: Vec<_> = files
                            .into_iter()
                            .filter(|f| f.file_path.to_lowercase().contains(&query_lower))
                            .collect();

                        if !filtered.is_empty() {
                            let mut section = format!(
                                "## Files matching '{}' in component '{}' ({} results)\n",
                                query, comp, filtered.len()
                            );
                            for file in filtered.iter().take(50) {
                                section.push_str(&format!(
                                    "  {} ({}, {} lines)\n",
                                    file.file_path, file.language, file.line_count
                                ));
                            }
                            if filtered.len() > 50 {
                                section.push_str(&format!("  ... and {} more\n", filtered.len() - 50));
                            }
                            output_sections.push(section);
                        } else if scope == "files" {
                            output_sections.push(format!(
                                "No files matching '{}' in component '{}'.",
                                query, comp
                            ));
                        }
                    }
                    Err(e) => {
                        output_sections.push(format!("File search error: {}", e));
                    }
                }
            } else {
                // No component filter — search all components via project summary,
                // then query symbols to find files that match the query pattern.
                // We use query_symbols as a proxy to discover files matching the query.
                match index_store.get_project_summary(&project_path) {
                    Ok(summary) => {
                        let query_lower = query.to_lowercase();
                        let mut matching_files: Vec<String> = Vec::new();

                        // Search each component for files matching the query
                        for comp_summary in &summary.components {
                            if let Ok(files) = index_store.query_files_by_component(&project_path, &comp_summary.name) {
                                for file in files {
                                    if file.file_path.to_lowercase().contains(&query_lower) {
                                        matching_files.push(format!(
                                            "  {} [{}] ({}, {} lines)",
                                            file.file_path, file.component, file.language, file.line_count
                                        ));
                                    }
                                }
                            }
                        }

                        if !matching_files.is_empty() {
                            let count = matching_files.len();
                            let mut section = format!(
                                "## Files matching '{}' ({} results)\n",
                                query, count
                            );
                            for line in matching_files.iter().take(50) {
                                section.push_str(line);
                                section.push('\n');
                            }
                            if count > 50 {
                                section.push_str(&format!("  ... and {} more\n", count - 50));
                            }
                            output_sections.push(section);
                        } else if scope == "files" {
                            output_sections.push(format!("No files matching '{}'.", query));
                        }
                    }
                    Err(e) => {
                        output_sections.push(format!("File search error: {}", e));
                    }
                }
            }
        }

        // --- Semantic search ---
        if scope == "semantic" {
            match &self.embedding_service {
                Some(emb_svc) if emb_svc.is_ready() => {
                    let query_embedding = emb_svc.embed_text(query);
                    if !query_embedding.is_empty() {
                        match index_store.semantic_search(&query_embedding, &project_path, 10) {
                            Ok(results) if !results.is_empty() => {
                                let mut section = format!(
                                    "## Semantic search for '{}' ({} results)\n",
                                    query,
                                    results.len()
                                );
                                for result in &results {
                                    // Truncate chunk text for display (char-boundary safe)
                                    let display_text = if result.chunk_text.len() > 200 {
                                        let mut end = 200;
                                        while end > 0 && !result.chunk_text.is_char_boundary(end) { end -= 1; }
                                        format!("{}...", &result.chunk_text[..end])
                                    } else {
                                        result.chunk_text.clone()
                                    };
                                    let display_text = display_text.replace('\n', " ");
                                    section.push_str(&format!(
                                        "  {} (chunk {}, similarity: {:.3})\n    {}\n",
                                        result.file_path,
                                        result.chunk_index,
                                        result.similarity,
                                        display_text
                                    ));
                                }
                                output_sections.push(section);
                            }
                            Ok(_) => {
                                output_sections.push(format!(
                                    "No semantic matches found for '{}'.",
                                    query
                                ));
                            }
                            Err(e) => {
                                output_sections.push(format!("Semantic search error: {}", e));
                            }
                        }
                    } else {
                        output_sections.push(
                            "Semantic search: embedding service produced empty vector. \
                             The vocabulary may not cover the query terms."
                                .to_string(),
                        );
                    }
                }
                Some(_) => {
                    output_sections.push(
                        "Semantic search not available: embedding vocabulary has not been built yet. \
                         The project needs to be re-indexed with embedding generation enabled. \
                         Use 'symbols' or 'files' scope instead."
                            .to_string(),
                    );
                }
                None => {
                    output_sections.push(
                        "Semantic search not available: no embedding service configured. \
                         Use 'symbols' or 'files' scope instead."
                            .to_string(),
                    );
                }
            }
        }

        if output_sections.is_empty() {
            ToolResult::ok(format!("No results found for '{}' (scope: {}).", query, scope))
        } else {
            ToolResult::ok(output_sections.join("\n"))
        }
    }

    /// Execute a tool by name with optional TaskContext for sub-agent support
    ///
    /// When `task_ctx` is provided, the Task tool becomes available.
    /// When `task_ctx` is None, the Task tool returns an error (sub-agents).
    pub async fn execute_with_context(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
        task_ctx: Option<&super::task_spawner::TaskContext>,
    ) -> ToolResult {
        match tool_name {
            "Task" => match task_ctx {
                Some(ctx) => self.execute_task(arguments, ctx).await,
                None => ToolResult::err("Task tool is not available at this depth. Sub-agents cannot spawn further sub-agents."),
            },
            _ => self.execute(tool_name, arguments).await,
        }
    }

    /// Execute Task tool — spawn a sub-agent
    async fn execute_task(
        &self,
        args: &serde_json::Value,
        ctx: &super::task_spawner::TaskContext,
    ) -> ToolResult {
        let prompt = match args.get("prompt").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolResult::err("Missing required parameter: prompt"),
        };

        let task_type = args
            .get("task_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let sub_agent_id = uuid::Uuid::new_v4().to_string();

        // Emit SubAgentStart event
        let _ = ctx
            .tx
            .send(
                crate::services::streaming::unified::UnifiedStreamEvent::SubAgentStart {
                    sub_agent_id: sub_agent_id.clone(),
                    prompt: prompt.chars().take(200).collect(),
                    task_type: task_type.clone(),
                },
            )
            .await;

        // Spawn the sub-agent task
        let result = ctx
            .spawner
            .spawn_task(
                prompt,
                task_type,
                ctx.tx.clone(),
                ctx.cancellation_token.clone(),
            )
            .await;

        // Emit SubAgentEnd event
        let _ = ctx
            .tx
            .send(
                crate::services::streaming::unified::UnifiedStreamEvent::SubAgentEnd {
                    sub_agent_id,
                    success: result.success,
                    usage: serde_json::json!({
                        "input_tokens": result.usage.input_tokens,
                        "output_tokens": result.usage.output_tokens,
                        "iterations": result.iterations,
                    }),
                },
            )
            .await;

        if result.success {
            ToolResult::ok(
                result
                    .response
                    .unwrap_or_else(|| "Task completed with no output".to_string()),
            )
        } else {
            ToolResult::err(
                result
                    .error
                    .unwrap_or_else(|| "Task failed with unknown error".to_string()),
            )
        }
    }

    /// Return a summary of all files currently in the read cache.
    ///
    /// Each entry is `(display_path, line_count, size_bytes)`, useful for
    /// session-level memory ("which files has the agent already seen?").
    pub fn get_read_file_summary(&self) -> Vec<(String, usize, u64)> {
        let cache = match self.read_cache.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        // Deduplicate by path (there may be multiple entries for different
        // offset/limit combinations). We keep the entry with the largest
        // line_count for each path so the summary reflects the fullest read.
        let mut by_path: HashMap<PathBuf, &ReadCacheEntry> = HashMap::new();
        for entry in cache.values() {
            by_path
                .entry(entry.path.clone())
                .and_modify(|existing| {
                    if entry.line_count > existing.line_count {
                        *existing = entry;
                    }
                })
                .or_insert(entry);
        }
        let mut result: Vec<(String, usize, u64)> = by_path
            .values()
            .map(|e| (e.path.display().to_string(), e.line_count, e.size_bytes))
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Clear the entire read deduplication cache.
    ///
    /// This is useful when starting a new logical session or after bulk
    /// file modifications where stale cache entries could cause confusion.
    /// ADR-004: Called after compaction to prevent stale dedup entries from
    /// causing infinite read loops with weak LLM providers.
    pub fn clear_read_cache(&self) {
        if let Ok(mut cache) = self.read_cache.lock() {
            cache.clear();
        }
    }

    /// Build a symbols summary for enhanced dedup messages.
    ///
    /// Queries the IndexStore for key symbols in the given file and returns
    /// a formatted string section. Returns an empty string if the IndexStore
    /// is not available or has no symbols for the file.
    fn get_dedup_symbols_summary(&self, file_path: &Path) -> String {
        let index_store = match &self.index_store {
            Some(store) => store,
            None => return String::new(),
        };

        let project_path = self.project_root.to_string_lossy().to_string();
        // Convert absolute file path to relative for IndexStore lookup
        let relative_path = file_path
            .strip_prefix(&self.project_root)
            .map(|p| p.to_string_lossy().to_string().replace('\\', "/"))
            .unwrap_or_else(|_| file_path.to_string_lossy().to_string());

        match index_store.get_file_symbols(&project_path, &relative_path) {
            Ok(symbols) if !symbols.is_empty() => {
                let symbol_list: Vec<String> = symbols
                    .iter()
                    .take(10) // Limit to 10 key symbols
                    .map(|s| format!("{}:{} (line {})", s.kind.short_name(), s.name, s.line))
                    .collect();
                format!("\nKey symbols: {}", symbol_list.join(", "))
            }
            _ => String::new(),
        }
    }

    /// Get project root (for external access)
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Update the project root (working directory)
    pub fn set_project_root(&mut self, new_root: PathBuf) {
        self.project_root = new_root;
    }
}

fn is_default_scan_excluded(base: &Path, candidate: &Path) -> bool {
    if let Ok(relative) = candidate.strip_prefix(base) {
        if let Some(first) = relative.components().next() {
            let root = first.as_os_str().to_string_lossy();
            return DEFAULT_SCAN_EXCLUDES.contains(&root.as_ref());
        }
    }
    false
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "line 1\nline 2\nline 3\n").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        std::fs::write(dir.path().join("subdir/nested.txt"), "nested content").unwrap();
        dir
    }

    #[tokio::test]
    async fn test_read_file() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "file_path": dir.path().join("test.txt").to_string_lossy().to_string()
        });

        let result = executor.execute("Read", &args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("line 1"));
    }

    #[tokio::test]
    async fn test_read_non_utf8_text_file_uses_lossy_decode() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file = dir.path().join("latin1.txt");
        std::fs::write(
            &file,
            vec![0x66, 0x6f, 0x6f, 0x20, 0x80, 0x20, 0x62, 0x61, 0x72],
        )
        .unwrap();

        let args = serde_json::json!({
            "file_path": file.to_string_lossy().to_string()
        });

        let result = executor.execute("Read", &args).await;
        assert!(result.success);
        let output = result.output.unwrap_or_default();
        assert!(output.contains("[non-utf8 decoded with replacement]"));
        assert!(output.contains("foo"));
    }

    #[tokio::test]
    async fn test_read_binary_file_returns_skip_message_not_error() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file = dir.path().join("archive.zip");
        std::fs::write(&file, vec![0x50, 0x4b, 0x03, 0x04, 0x00, 0x00, 0xff, 0x00]).unwrap();

        let args = serde_json::json!({
            "file_path": file.to_string_lossy().to_string()
        });

        let result = executor.execute("Read", &args).await;
        assert!(result.success);
        let output = result.output.unwrap_or_default();
        assert!(output.contains("[binary file skipped]"));
        assert!(output.contains("archive.zip"));
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "file_path": dir.path().join("nonexistent.txt").to_string_lossy().to_string()
        });

        let result = executor.execute("Read", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_write_file() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let new_file = dir.path().join("new_file.txt");
        let args = serde_json::json!({
            "file_path": new_file.to_string_lossy().to_string(),
            "content": "new content"
        });

        let result = executor.execute("Write", &args).await;
        assert!(result.success);
        assert!(new_file.exists());
        assert_eq!(std::fs::read_to_string(&new_file).unwrap(), "new content");
    }

    #[tokio::test]
    async fn test_edit_file() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        // Must read before editing (read-before-write enforcement)
        let read_args = serde_json::json!({
            "file_path": dir.path().join("test.txt").to_string_lossy().to_string()
        });
        executor.execute("Read", &read_args).await;

        let args = serde_json::json!({
            "file_path": dir.path().join("test.txt").to_string_lossy().to_string(),
            "old_string": "line 2",
            "new_string": "modified line 2"
        });

        let result = executor.execute("Edit", &args).await;
        assert!(result.success);

        let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert!(content.contains("modified line 2"));
    }

    #[tokio::test]
    async fn test_edit_non_unique() {
        let dir = setup_test_dir();
        std::fs::write(dir.path().join("dup.txt"), "foo foo foo").unwrap();
        let executor = ToolExecutor::new(dir.path());

        // Must read before editing (read-before-write enforcement)
        let read_args = serde_json::json!({
            "file_path": dir.path().join("dup.txt").to_string_lossy().to_string()
        });
        executor.execute("Read", &read_args).await;

        let args = serde_json::json!({
            "file_path": dir.path().join("dup.txt").to_string_lossy().to_string(),
            "old_string": "foo",
            "new_string": "bar"
        });

        let result = executor.execute("Edit", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("appears 3 times"));
    }

    #[tokio::test]
    async fn test_bash_simple() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        #[cfg(windows)]
        let args = serde_json::json!({
            "command": "echo hello"
        });
        #[cfg(not(windows))]
        let args = serde_json::json!({
            "command": "echo hello"
        });

        let result = executor.execute("Bash", &args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_blocked_command() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "command": "rm -rf /"
        });

        let result = executor.execute("Bash", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("blocked"));
    }

    #[tokio::test]
    async fn test_glob() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "pattern": "**/*.txt",
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("Glob", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("test.txt"));
        assert!(output.contains("nested.txt"));
    }

    #[tokio::test]
    async fn test_glob_relative_dot_uses_executor_working_dir() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "pattern": "test.txt",
            "path": "."
        });

        let result = executor.execute("Glob", &args).await;
        assert!(result.success, "glob should succeed: {:?}", result.error);
        let output = result.output.unwrap_or_default();
        assert!(
            output.contains("test.txt"),
            "expected test.txt in output, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_glob_head_limit_caps_results() {
        let dir = setup_test_dir();
        std::fs::write(dir.path().join("another.txt"), "x").unwrap();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "pattern": "**/*.txt",
            "path": dir.path().to_string_lossy().to_string(),
            "head_limit": 1
        });

        let result = executor.execute("Glob", &args).await;
        assert!(result.success);
        let output = result.output.unwrap_or_default();
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 1, "expected one line, got: {}", output);
    }

    #[tokio::test]
    async fn test_grep() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        // Default output_mode is files_with_matches — returns file paths
        let args = serde_json::json!({
            "pattern": "line",
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("Grep", &args).await;
        assert!(result.success);
        assert!(result.output.as_ref().unwrap().contains("test.txt"));

        // Test content mode — returns matching lines
        let args = serde_json::json!({
            "pattern": "line",
            "path": dir.path().to_string_lossy().to_string(),
            "output_mode": "content"
        });

        let result = executor.execute("Grep", &args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("line 1"));
    }

    #[tokio::test]
    async fn test_grep_relative_dot_uses_executor_working_dir() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "pattern": "nested",
            "path": "."
        });

        let result = executor.execute("Grep", &args).await;
        assert!(result.success, "grep should succeed: {:?}", result.error);
        let output = result.output.unwrap_or_default();
        assert!(
            output.contains("nested.txt"),
            "expected nested.txt in output, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_ls_directory() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("DIR"));
        assert!(output.contains("subdir"));
        assert!(output.contains("FILE"));
        assert!(output.contains("test.txt"));
    }

    #[tokio::test]
    async fn test_ls_hidden_files() {
        let dir = setup_test_dir();
        std::fs::write(dir.path().join(".hidden"), "hidden content").unwrap();
        let executor = ToolExecutor::new(dir.path());

        // Without show_hidden
        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });
        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        assert!(!result.output.unwrap().contains(".hidden"));

        // With show_hidden
        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string(),
            "show_hidden": true
        });
        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains(".hidden"));
    }

    #[tokio::test]
    async fn test_ls_not_a_directory() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().join("test.txt").to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Not a directory"));
    }

    #[tokio::test]
    async fn test_ls_not_found() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().join("nonexistent").to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_ls_truncation_large_directory() {
        let dir = TempDir::new().unwrap();
        // Create 250 files (exceeding LS_MAX_ENTRIES of 200)
        for i in 0..250 {
            std::fs::write(dir.path().join(format!("file_{:04}.txt", i)), "content").unwrap();
        }
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // Should show exactly LS_MAX_ENTRIES (200) file entries in the listing
        let file_lines: Vec<&str> = output
            .lines()
            .filter(|l| l.trim_start().starts_with("FILE") || l.trim_start().starts_with("DIR"))
            .collect();
        assert_eq!(
            file_lines.len(),
            LS_MAX_ENTRIES,
            "expected {} file entries, got {}",
            LS_MAX_ENTRIES,
            file_lines.len()
        );

        // Should contain truncation note with count of omitted entries
        assert!(
            output.contains("50 more entries not shown"),
            "expected truncation note with 50 omitted entries, got: {}",
            output
        );

        // Should suggest Glob
        assert!(
            output.contains("Glob"),
            "expected Glob suggestion in truncation note, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_ls_no_truncation_within_limit() {
        let dir = TempDir::new().unwrap();
        // Create exactly 200 files (at the limit, should NOT truncate)
        for i in 0..200 {
            std::fs::write(dir.path().join(format!("file_{:04}.txt", i)), "content").unwrap();
        }
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // Should show all 200 entries
        let file_lines: Vec<&str> = output
            .lines()
            .filter(|l| l.trim_start().starts_with("FILE") || l.trim_start().starts_with("DIR"))
            .collect();
        assert_eq!(file_lines.len(), 200, "expected 200 entries, got {}", file_lines.len());

        // Should NOT contain truncation note
        assert!(
            !output.contains("more entries not shown"),
            "should not have truncation note for directories at the limit"
        );
    }

    #[tokio::test]
    async fn test_ls_no_truncation_small_directory() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // Small directory should NOT have truncation note
        assert!(
            !output.contains("more entries not shown"),
            "small directory should not have truncation note"
        );
    }

    #[tokio::test]
    async fn test_cwd() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({});

        let result = executor.execute("Cwd", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert_eq!(output, dir.path().to_string_lossy().to_string());
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(100), "100 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
    }

    #[test]
    fn test_tool_result() {
        let ok = ToolResult::ok("success");
        assert!(ok.success);
        assert_eq!(ok.to_content(), "success");

        let err = ToolResult::err("failed");
        assert!(!err.success);
        assert!(err.to_content().contains("Error"));
    }

    // =========================================================================
    // Read cache deduplication tests (story-001)
    // =========================================================================

    #[tokio::test]
    async fn test_read_cache_dedup_unchanged_file() {
        // Reading the same unchanged file twice should return a short dedup
        // message on the second read.
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });

        // First read: full content
        let result1 = executor.execute("Read", &args).await;
        assert!(result1.success);
        let output1 = result1.output.unwrap();
        assert!(output1.contains("line 1"), "first read should have content");

        // Second read: dedup message
        let result2 = executor.execute("Read", &args).await;
        assert!(result2.success);
        let output2 = result2.output.unwrap();
        assert!(
            output2.contains("[File previously read in this session, unchanged]"),
            "second read should be dedup message, got: {}",
            output2
        );
        assert!(output2.contains("test.txt"));
        assert!(output2.contains("lines"));
        assert!(output2.contains("bytes"));
        // Enhanced dedup message should include file type
        assert!(
            output2.contains("type: txt"),
            "dedup message should include file type, got: {}",
            output2
        );
        // Enhanced dedup message should include a preview
        assert!(
            output2.contains("Preview (first lines)"),
            "dedup message should include first lines preview, got: {}",
            output2
        );
        assert!(
            output2.contains("line 1"),
            "preview should contain first line content, got: {}",
            output2
        );
        // Enhanced dedup message should include do-not-reread instruction
        assert!(
            output2.contains("Do NOT re-read"),
            "dedup message should instruct LLM not to re-read, got: {}",
            output2
        );
    }

    #[tokio::test]
    async fn test_dedup_message_includes_extension_for_rust_file() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file = dir.path().join("test_module.rs");
        std::fs::write(&file, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        let file_path = file.to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });

        // First read
        executor.execute("Read", &args).await;
        // Second read should return enhanced dedup
        let result = executor.execute("Read", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("type: rs"),
            "should show rs file type, got: {}",
            output
        );
        assert!(
            output.contains("fn main()"),
            "preview should contain first line, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_read_cache_modified_file_returns_full_content() {
        // If the file is modified between reads, the second read should
        // return the full (new) content, not a dedup message.
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file = dir.path().join("mtime_test.txt");
        std::fs::write(&file, "original line 1\noriginal line 2\n").unwrap();

        let file_path = file.to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });

        // First read
        let result1 = executor.execute("Read", &args).await;
        assert!(result1.success);
        assert!(result1.output.unwrap().contains("original line 1"));

        // Modify the file and force a different mtime.
        // We sleep briefly to guarantee mtime changes even on file systems
        // with coarse (1-second) mtime granularity.
        std::thread::sleep(std::time::Duration::from_secs(1));
        std::fs::write(&file, "changed line 1\nchanged line 2\n").unwrap();

        // Second read after modification
        let result2 = executor.execute("Read", &args).await;
        assert!(result2.success);
        let output2 = result2.output.unwrap();
        assert!(
            !output2.contains("[File previously read in this session, unchanged]"),
            "modified file should NOT return dedup message, got: {}",
            output2
        );
        assert!(
            output2.contains("changed line 1"),
            "should see new content, got: {}",
            output2
        );
    }

    #[tokio::test]
    async fn test_read_cache_different_offset_returns_full_content() {
        // Reading the same file with a different offset/limit should return
        // full content even if the file hasn't changed, because the cache key
        // includes offset and limit.
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();

        // First read with default offset/limit
        let args1 = serde_json::json!({ "file_path": &file_path });
        let result1 = executor.execute("Read", &args1).await;
        assert!(result1.success);
        assert!(result1.output.unwrap().contains("line 1"));

        // Second read with different offset — should NOT be dedup
        let args2 = serde_json::json!({ "file_path": &file_path, "offset": 2 });
        let result2 = executor.execute("Read", &args2).await;
        assert!(result2.success);
        let output2 = result2.output.unwrap();
        assert!(
            !output2.contains("[File previously read in this session, unchanged]"),
            "different offset should return full content, got: {}",
            output2
        );
        assert!(output2.contains("line 2"));

        // Third read with different limit — should NOT be dedup
        let args3 = serde_json::json!({ "file_path": &file_path, "limit": 1 });
        let result3 = executor.execute("Read", &args3).await;
        assert!(result3.success);
        let output3 = result3.output.unwrap();
        assert!(
            !output3.contains("[File previously read in this session, unchanged]"),
            "different limit should return full content, got: {}",
            output3
        );
    }

    #[tokio::test]
    async fn test_read_cache_same_offset_limit_dedup() {
        // Reading with explicitly the same offset/limit should dedup on second read
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();

        let args = serde_json::json!({ "file_path": &file_path, "offset": 2, "limit": 1 });

        // First read
        let result1 = executor.execute("Read", &args).await;
        assert!(result1.success);
        assert!(result1.output.unwrap().contains("line 2"));

        // Second read with identical args — should dedup
        let result2 = executor.execute("Read", &args).await;
        assert!(result2.success);
        assert!(
            result2
                .output
                .unwrap()
                .contains("[File previously read in this session, unchanged]"),
            "same offset/limit should dedup"
        );
    }

    #[tokio::test]
    async fn test_get_read_file_summary() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        // Initially empty
        assert!(
            executor.get_read_file_summary().is_empty(),
            "summary should be empty before any reads"
        );

        // Read a file
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });
        executor.execute("Read", &args).await;

        let summary = executor.get_read_file_summary();
        assert_eq!(summary.len(), 1, "should have one cached file");
        assert!(summary[0].0.contains("test.txt"));
        assert!(summary[0].1 > 0, "line_count should be > 0");
        assert!(summary[0].2 > 0, "size_bytes should be > 0");

        // Read another file
        let nested_path = dir
            .path()
            .join("subdir")
            .join("nested.txt")
            .to_string_lossy()
            .to_string();
        let args2 = serde_json::json!({ "file_path": &nested_path });
        executor.execute("Read", &args2).await;

        let summary2 = executor.get_read_file_summary();
        assert_eq!(summary2.len(), 2, "should have two cached files");
    }

    #[tokio::test]
    async fn test_get_read_file_summary_deduplicates_by_path() {
        // Reading the same file with different offset/limit creates multiple
        // cache entries, but the summary should deduplicate by path.
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();

        // Read with default params
        executor
            .execute("Read", &serde_json::json!({ "file_path": &file_path }))
            .await;
        // Read with different offset
        executor
            .execute(
                "Read",
                &serde_json::json!({ "file_path": &file_path, "offset": 2 }),
            )
            .await;

        let summary = executor.get_read_file_summary();
        assert_eq!(
            summary.len(),
            1,
            "summary should deduplicate by path, got: {:?}",
            summary
        );
    }

    #[tokio::test]
    async fn test_clear_read_cache() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        // Read a file to populate cache
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });
        executor.execute("Read", &args).await;

        assert!(
            !executor.get_read_file_summary().is_empty(),
            "cache should be populated"
        );

        // Clear cache
        executor.clear_read_cache();
        assert!(
            executor.get_read_file_summary().is_empty(),
            "cache should be empty after clear"
        );

        // Reading again after clear should return full content, not dedup
        let result = executor.execute("Read", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            !output.contains("[File previously read in this session, unchanged]"),
            "after clear, should get full content, got: {}",
            output
        );
        assert!(output.contains("line 1"));
    }

    // =========================================================================
    // CodebaseSearch tests (story-009)
    // =========================================================================

    fn create_test_executor_with_index() -> (TempDir, ToolExecutor) {
        use crate::services::orchestrator::index_store::IndexStore;
        use crate::storage::database::Database;

        let dir = setup_test_dir();
        let mut executor = ToolExecutor::new(dir.path());

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        // Populate the index with test data
        use crate::services::orchestrator::analysis_index::{FileInventoryItem, SymbolInfo, SymbolKind};

        let project_path = dir.path().to_string_lossy().to_string();

        let item1 = FileInventoryItem {
            path: "src/main.rs".to_string(),
            component: "desktop-rust".to_string(),
            language: "rust".to_string(),
            extension: Some("rs".to_string()),
            size_bytes: 1024,
            line_count: 50,
            is_test: false,
            symbols: vec![
                SymbolInfo::basic("main".to_string(), SymbolKind::Function, 1),
                SymbolInfo::basic("AppConfig".to_string(), SymbolKind::Struct, 10),
            ],
        };

        let item2 = FileInventoryItem {
            path: "src/lib.rs".to_string(),
            component: "desktop-rust".to_string(),
            language: "rust".to_string(),
            extension: Some("rs".to_string()),
            size_bytes: 2048,
            line_count: 100,
            is_test: false,
            symbols: vec![
                SymbolInfo::basic("init_app".to_string(), SymbolKind::Function, 5),
            ],
        };

        let item3 = FileInventoryItem {
            path: "src/components/App.tsx".to_string(),
            component: "desktop-web".to_string(),
            language: "typescript".to_string(),
            extension: Some("tsx".to_string()),
            size_bytes: 512,
            line_count: 30,
            is_test: false,
            symbols: vec![
                SymbolInfo::basic("App".to_string(), SymbolKind::Function, 1),
                SymbolInfo::basic("AppProps".to_string(), SymbolKind::Interface, 5),
            ],
        };

        store.upsert_file_index(&project_path, &item1, "h1").unwrap();
        store.upsert_file_index(&project_path, &item2, "h2").unwrap();
        store.upsert_file_index(&project_path, &item3, "h3").unwrap();

        executor.set_index_store(store);

        (dir, executor)
    }

    #[tokio::test]
    async fn test_codebase_search_index_unavailable() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({ "query": "main" });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("index not available"),
            "should indicate index is unavailable, got: {}",
            output
        );
        assert!(
            output.contains("Grep"),
            "should suggest Grep as alternative, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_missing_query() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({});
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("query"));
    }

    #[tokio::test]
    async fn test_codebase_search_symbols_scope() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "App",
            "scope": "symbols"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("AppConfig"),
            "should find AppConfig symbol, got: {}",
            output
        );
        assert!(
            output.contains("AppProps"),
            "should find AppProps symbol, got: {}",
            output
        );
        assert!(
            output.contains("Symbols matching"),
            "should have symbols section header, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_files_scope_with_component() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "main",
            "scope": "files",
            "component": "desktop-rust"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("src/main.rs"),
            "should find main.rs, got: {}",
            output
        );
        assert!(
            output.contains("Files matching"),
            "should have files section header, got: {}",
            output
        );
        // Should NOT include web component files
        assert!(
            !output.contains("App.tsx"),
            "should not include web files when filtering by desktop-rust, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_files_scope_without_component() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "lib",
            "scope": "files"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("src/lib.rs"),
            "should find lib.rs, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_all_scope() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "App",
            "scope": "all"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // Should have both symbol and file results
        assert!(
            output.contains("Symbols matching"),
            "should have symbols section, got: {}",
            output
        );
        // AppConfig or App should appear in symbols
        assert!(
            output.contains("AppConfig") || output.contains("AppProps"),
            "should find App-related symbols, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_default_scope_is_all() {
        let (_dir, executor) = create_test_executor_with_index();

        // No scope parameter — should default to "all"
        let args = serde_json::json!({ "query": "App" });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("Symbols matching"),
            "default scope should search symbols, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_component_filter_narrows_symbols() {
        let (_dir, executor) = create_test_executor_with_index();

        // Search for "App" with symbols scope — should find both AppConfig and AppProps
        let args_all = serde_json::json!({
            "query": "App",
            "scope": "symbols"
        });
        let result_all = executor.execute("CodebaseSearch", &args_all).await;
        let output_all = result_all.output.unwrap();
        assert!(output_all.contains("AppConfig"));
        assert!(output_all.contains("AppProps"));
    }

    #[tokio::test]
    async fn test_codebase_search_no_results() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "NonExistentThing",
            "scope": "symbols"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("No symbols matching"),
            "should indicate no results found, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_output_format() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "main",
            "scope": "symbols"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // Output should include file path, line number, and kind
        assert!(
            output.contains("main") && output.contains("Function") && output.contains("src/main.rs"),
            "output should contain symbol name, kind, and file path, got: {}",
            output
        );
    }
}
