//! Tool Executor
//!
//! Executes tools requested by LLM providers.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use serde::{Deserialize, Serialize};
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
}

impl ToolResult {
    /// Create a successful result
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
        }
    }

    /// Create an error result
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error.into()),
        }
    }

    /// Convert to string for LLM consumption
    pub fn to_content(&self) -> String {
        if self.success {
            self.output.clone().unwrap_or_default()
        } else {
            format!("Error: {}", self.error.as_deref().unwrap_or("Unknown error"))
        }
    }
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
}

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            default_timeout: 120_000, // 2 minutes
        }
    }

    /// Execute a tool by name with given arguments
    pub async fn execute(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> ToolResult {
        match tool_name {
            "Read" => self.execute_read(arguments).await,
            "Write" => self.execute_write(arguments).await,
            "Edit" => self.execute_edit(arguments).await,
            "Bash" => self.execute_bash(arguments).await,
            "Glob" => self.execute_glob(arguments).await,
            "Grep" => self.execute_grep(arguments).await,
            _ => ToolResult::err(format!("Unknown tool: {}", tool_name)),
        }
    }

    /// Validate and resolve a file path
    fn validate_path(&self, path: &str) -> Result<PathBuf, String> {
        let path = Path::new(path);

        // Convert to absolute path if relative
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_root.join(path)
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
            Ok(canonical) => {
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
            None => return ToolResult::err("Missing required parameter: file_path"),
        };

        let path = match self.validate_path(file_path) {
            Ok(p) => p,
            Err(e) => return ToolResult::err(e),
        };

        if !path.exists() {
            return ToolResult::err(format!("File not found: {}", file_path));
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
                    .map(|(i, line)| format!("{:6}\t{}", start + i + 1, line))
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
            None => return ToolResult::err("Missing required parameter: file_path"),
        };

        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err("Missing required parameter: content"),
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

        let replace_all = args.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);

        let path = match self.validate_path(file_path) {
            Ok(p) => p,
            Err(e) => return ToolResult::err(e),
        };

        if !path.exists() {
            return ToolResult::err(format!("File not found: {}", file_path));
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
                    ToolResult::ok(format!(
                        "Successfully edited {}",
                        path.display()
                    ))
                }
            }
            Err(e) => ToolResult::err(format!("Failed to write file: {}", e)),
        }
    }

    /// Execute Bash tool
    async fn execute_bash(&self, args: &serde_json::Value) -> ToolResult {
        let command = match args.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err("Missing required parameter: command"),
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
            .unwrap_or_else(|| self.project_root.clone());

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
            Err(_) => ToolResult::err(format!(
                "Command timed out after {} ms",
                timeout_ms
            )),
        }
    }

    /// Execute Glob tool
    async fn execute_glob(&self, args: &serde_json::Value) -> ToolResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: pattern"),
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

    /// Execute Grep tool
    async fn execute_grep(&self, args: &serde_json::Value) -> ToolResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: pattern"),
        };

        let search_path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project_root.clone());

        let file_glob = args.get("glob").and_then(|v| v.as_str());
        let case_insensitive = args
            .get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let context_lines = args
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        // Build regex
        let regex = match if case_insensitive {
            regex::RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
        } else {
            regex::Regex::new(pattern)
        } {
            Ok(r) => r,
            Err(e) => return ToolResult::err(format!("Invalid regex pattern: {}", e)),
        };

        let mut results = Vec::new();

        // Get files to search
        let files: Vec<PathBuf> = if search_path.is_file() {
            vec![search_path.clone()]
        } else {
            let glob_pattern = if let Some(g) = file_glob {
                search_path.join("**").join(g)
            } else {
                search_path.join("**").join("*")
            };

            glob::glob(&glob_pattern.to_string_lossy())
                .map(|paths| paths.filter_map(|r| r.ok()).filter(|p| p.is_file()).collect())
                .unwrap_or_default()
        };

        // Limit number of files to search
        let max_files = 1000;
        let files: Vec<_> = files.into_iter().take(max_files).collect();

        for file in files {
            if let Ok(content) = std::fs::read_to_string(&file) {
                let lines: Vec<&str> = content.lines().collect();

                for (line_num, line) in lines.iter().enumerate() {
                    if regex.is_match(line) {
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

                        results.push(format!(
                            "{}:\n{}",
                            file.display(),
                            context.join("\n")
                        ));
                    }
                }
            }
        }

        if results.is_empty() {
            ToolResult::ok("No matches found")
        } else {
            // Limit output size
            let output = results.join("\n\n");
            if output.len() > 50000 {
                ToolResult::ok(format!(
                    "{}\n\n... (output truncated, {} total matches)",
                    &output[..50000],
                    results.len()
                ))
            } else {
                ToolResult::ok(output)
            }
        }
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

        let args = serde_json::json!({
            "pattern": "line",
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("Grep", &args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("line"));
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
