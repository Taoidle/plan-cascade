//! Tool Executor
//!
//! Executes tools requested by LLM providers.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

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
    pub fn ok_with_image(output: impl Into<String>, mime_type: String, base64_data: String) -> Self {
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
        ("Read", "file_path") => r#"```tool_call
{"tool": "Read", "arguments": {"file_path": "path/to/file"}}
```"#,
        ("LS", "path") => r#"```tool_call
{"tool": "LS", "arguments": {"path": "."}}
```"#,
        ("Bash", "command") => r#"```tool_call
{"tool": "Bash", "arguments": {"command": "your command here"}}
```"#,
        ("Glob", "pattern") => r#"```tool_call
{"tool": "Glob", "arguments": {"pattern": "**/*.rs"}}
```"#,
        ("Grep", "pattern") => r#"```tool_call
{"tool": "Grep", "arguments": {"pattern": "search_term"}}
```"#,
        ("Write", "file_path") => r#"```tool_call
{"tool": "Write", "arguments": {"file_path": "path/to/file", "content": "file content"}}
```"#,
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

/// Tool executor for running tools locally
pub struct ToolExecutor {
    /// Project root for path validation
    project_root: PathBuf,
    /// Default timeout for bash commands (in milliseconds)
    default_timeout: u64,
    /// Track files that have been read (for read-before-write enforcement)
    read_files: Mutex<HashSet<PathBuf>>,
    /// Persistent working directory for Bash commands
    current_working_dir: Mutex<PathBuf>,
    /// WebFetch service for fetching web pages
    web_fetch: super::web_fetch::WebFetchService,
    /// WebSearch service (None if no search provider configured)
    web_search: Option<super::web_search::WebSearchService>,
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
            web_fetch: super::web_fetch::WebFetchService::new(),
            web_search: None,
        }
    }

    /// Configure the web search provider
    pub fn set_search_provider(&mut self, provider_name: &str, api_key: Option<String>) {
        match super::web_search::WebSearchService::new(provider_name, api_key.as_deref()) {
            Ok(service) => self.web_search = Some(service),
            Err(e) => {
                tracing::warn!("Failed to configure search provider '{}': {}", provider_name, e);
                self.web_search = None;
            }
        }
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
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "pdf" => {
                let pages = args.get("pages").and_then(|v| v.as_str());
                match super::file_parsers::parse_pdf(&path, pages) {
                    Ok(content) => return ToolResult::ok(content),
                    Err(e) => return ToolResult::err(e),
                }
            }
            "ipynb" => {
                match super::file_parsers::parse_jupyter(&path) {
                    Ok(content) => return ToolResult::ok(content),
                    Err(e) => return ToolResult::err(e),
                }
            }
            "docx" => {
                match super::file_parsers::parse_docx(&path) {
                    Ok(content) => return ToolResult::ok(content),
                    Err(e) => return ToolResult::err(e),
                }
            }
            "xlsx" | "xls" | "ods" => {
                match super::file_parsers::parse_xlsx(&path) {
                    Ok(content) => return ToolResult::ok(content),
                    Err(e) => return ToolResult::err(e),
                }
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

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let start = (offset.saturating_sub(1)).min(lines.len());
                let end = (start + limit).min(lines.len());

                let numbered_lines: Vec<String> = lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| {
                        let truncated = if line.len() > 2000 {
                            format!("{}...", &line[..2000])
                        } else {
                            line.to_string()
                        };
                        format!("{:6}\t{}", start + i + 1, truncated)
                    })
                    .collect();

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

        // Enforce read-before-write for existing files
        if path.exists() {
            if let Ok(read_files) = self.read_files.lock() {
                if !read_files.contains(&path) {
                    return ToolResult::err(
                        "You must read a file before writing to it. Use the Read tool first.",
                    );
                }
            }
        }

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

        // Enforce read-before-edit
        if let Ok(read_files) = self.read_files.lock() {
            if !read_files.contains(&path) {
                return ToolResult::err(
                    "You must read a file before editing it. Use the Read tool first.",
                );
            }
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("Failed to read file: {}", e)),
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

        let base_path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project_root.clone());

        // Combine base path with pattern
        let full_pattern = base_path.join(pattern);
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

        let search_path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                self.current_working_dir
                    .lock()
                    .map(|cwd| cwd.clone())
                    .unwrap_or_else(|_| self.project_root.clone())
            });

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
        let head_limit = args
            .get("head_limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

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
                if !entry
                    .file_type()
                    .map(|ft| ft.is_file())
                    .unwrap_or(false)
                {
                    continue;
                }

                let path = entry.path();

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
        let dir_path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

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

                    let is_dir = entry
                        .file_type()
                        .map(|ft| ft.is_dir())
                        .unwrap_or(false);

                    let size = if is_dir {
                        0
                    } else {
                        entry.metadata().map(|m| m.len()).unwrap_or(0)
                    };

                    items.push((name, is_dir, size));
                }

                // Sort: directories first, then alphabetically
                items.sort_by(|a, b| {
                    b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
                });

                if items.is_empty() {
                    return ToolResult::ok(format!("Directory is empty: {}", path.display()));
                }

                let mut output = format!("Directory: {}\n\n", path.display());
                for (name, is_dir, size) in &items {
                    if *is_dir {
                        output.push_str(&format!("  DIR   {:>10}  {}/\n", "-", name));
                    } else {
                        output.push_str(&format!("  FILE  {:>10}  {}\n", format_size(*size), name));
                    }
                }
                output.push_str(&format!(
                    "\n{} entries ({} dirs, {} files)",
                    items.len(),
                    items.iter().filter(|i| i.1).count(),
                    items.iter().filter(|i| !i.1).count(),
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

        match super::notebook_edit::edit_notebook(&path, cell_index, operation, cell_type, new_source) {
            Ok(msg) => ToolResult::ok(msg),
            Err(e) => ToolResult::err(e),
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
            .send(crate::services::streaming::unified::UnifiedStreamEvent::SubAgentStart {
                sub_agent_id: sub_agent_id.clone(),
                prompt: prompt.chars().take(200).collect(),
                task_type: task_type.clone(),
            })
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
            .send(crate::services::streaming::unified::UnifiedStreamEvent::SubAgentEnd {
                sub_agent_id,
                success: result.success,
                usage: serde_json::json!({
                    "input_tokens": result.usage.input_tokens,
                    "output_tokens": result.usage.output_tokens,
                    "iterations": result.iterations,
                }),
            })
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

    /// Get project root (for external access)
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Update the project root (working directory)
    pub fn set_project_root(&mut self, new_root: PathBuf) {
        self.project_root = new_root;
    }
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
}
