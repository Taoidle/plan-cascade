//! Bash Tool Implementation
//!
//! Executes shell commands with timeout, blocked command checking,
//! and persistent working directory tracking via ToolExecutionContext.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;
use tokio::process::Command;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

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

/// Default timeout in milliseconds
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
/// Maximum timeout in milliseconds (10 minutes)
const MAX_TIMEOUT_MS: u64 = 600_000;

/// Bash command tool — executes shell commands with safety checks.
///
/// Uses `ctx.working_directory` (Arc<Mutex<PathBuf>>) for persistent
/// working directory tracking. When a simple `cd <path>` command succeeds,
/// the shared working directory is updated for all subsequent tool calls.
pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        Self
    }

    /// Detect simple `cd <path>` commands and update the shared working directory
    fn detect_cd_command(command: &str, working_dir: &Path, shared_cwd: &Mutex<PathBuf>) {
        let trimmed = command.trim();
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
            if let Ok(canonical) = target_path.canonicalize() {
                if canonical.is_dir() {
                    if let Ok(mut cwd) = shared_cwd.lock() {
                        *cwd = canonical;
                    }
                }
            }
        }
    }
}

fn missing_param_error() -> String {
    let example = r#"```tool_call
{"tool": "Bash", "arguments": {"command": "your command here"}}
```"#;
    format!("Missing required parameter: command. Correct format:\n{example}")
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command. Returns stdout and stderr. Has a configurable timeout. Some dangerous commands are blocked for safety."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "command".to_string(),
            ParameterSchema::string(Some("The command to execute")),
        );
        properties.insert(
            "timeout".to_string(),
            ParameterSchema::integer(Some(
                "Timeout in milliseconds (default: 120000, max: 600000)",
            )),
        );
        properties.insert(
            "working_dir".to_string(),
            ParameterSchema::string(Some("Working directory for the command")),
        );
        ParameterSchema::object(
            Some("Bash command parameters"),
            properties,
            vec!["command".to_string()],
        )
    }

    fn is_long_running(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let command = match args.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err(missing_param_error()),
        };

        let timeout_ms = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);

        let working_dir = args
            .get("working_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| ctx.working_directory_snapshot());

        // Check for blocked commands
        for blocked in BLOCKED_COMMANDS {
            if command.contains(blocked) {
                return ToolResult::err(format!(
                    "Command blocked for safety: contains '{}'",
                    blocked
                ));
            }
        }

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

        // Use spawn + select! so we can cancel or timeout and kill the child process.
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => return ToolResult::err(format!("Failed to spawn command: {}", e)),
        };

        // Take stdout/stderr handles before awaiting, so `child` is not consumed.
        let child_stdout = child.stdout.take();
        let child_stderr = child.stderr.take();

        let result = tokio::select! {
            status = child.wait() => {
                match status {
                    Ok(status) => {
                        // Read captured output from taken handles
                        let stdout_bytes = if let Some(mut out) = child_stdout {
                            let mut buf = Vec::new();
                            let _ = tokio::io::AsyncReadExt::read_to_end(&mut out, &mut buf).await;
                            buf
                        } else {
                            Vec::new()
                        };
                        let stderr_bytes = if let Some(mut err) = child_stderr {
                            let mut buf = Vec::new();
                            let _ = tokio::io::AsyncReadExt::read_to_end(&mut err, &mut buf).await;
                            buf
                        } else {
                            Vec::new()
                        };
                        let output = std::process::Output {
                            status,
                            stdout: stdout_bytes,
                            stderr: stderr_bytes,
                        };
                        Ok(Ok(output))
                    }
                    Err(e) => Ok(Err(e)),
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(timeout_ms)) => {
                // Timeout: kill the child process
                let _ = child.kill().await;
                Err("timeout")
            }
            _ = ctx.cancellation_token.cancelled() => {
                // Cancelled: kill the child process
                let _ = child.kill().await;
                return ToolResult::err("Command cancelled".to_string());
            }
        };

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
                    Self::detect_cd_command(command, &working_dir, &ctx.working_directory);
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
            Err(_) => ToolResult::err(format!("Command timed out after {} ms", timeout_ms)), // "timeout" sentinel
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_test_ctx;
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_bash_tool_echo() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new();
        let ctx = make_test_ctx(dir.path());

        let args = serde_json::json!({"command": "echo hello"});
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_tool_blocked() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new();
        let ctx = make_test_ctx(dir.path());

        let args = serde_json::json!({"command": "rm -rf /"});
        let result = tool.execute(&ctx, args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("blocked"));
    }

    #[tokio::test]
    async fn test_bash_tool_missing_command() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new();
        let ctx = make_test_ctx(dir.path());

        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("command"));
    }

    #[test]
    fn test_bash_tool_name() {
        let tool = BashTool::new();
        assert_eq!(tool.name(), "Bash");
    }

    #[test]
    fn test_bash_tool_is_long_running() {
        let tool = BashTool::new();
        assert!(tool.is_long_running());
    }

    #[tokio::test]
    async fn test_bash_tool_cd_updates_shared_working_dir() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("sub");
        std::fs::create_dir(&subdir).unwrap();
        let tool = BashTool::new();
        let ctx = make_test_ctx(dir.path());

        // Execute cd command
        let args = serde_json::json!({"command": "cd sub"});
        let result = tool.execute(&ctx, args).await;
        assert!(result.success, "cd failed: {:?}", result.error);

        // Verify shared working directory was updated
        let new_cwd = ctx.working_directory_snapshot();
        assert_eq!(new_cwd, subdir.canonicalize().unwrap());
    }
}
