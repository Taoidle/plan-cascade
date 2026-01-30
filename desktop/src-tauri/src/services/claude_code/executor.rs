//! Claude Code CLI Executor
//!
//! Handles spawning and managing the Claude Code CLI process.
//! Provides async process lifecycle management with cancellation support.

use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::utils::error::{AppError, AppResult};

/// Handle to a running Claude Code CLI process
pub struct ClaudeCodeProcess {
    /// The child process
    child: Child,
    /// Process ID for identification
    pid: u32,
}

impl ClaudeCodeProcess {
    /// Get the process ID
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Check if the process is still running
    pub fn is_running(&self) -> bool {
        // Try to get the exit status without waiting
        self.child.id().is_some()
    }

    /// Kill the process
    pub async fn kill(&mut self) -> AppResult<()> {
        self.child
            .kill()
            .await
            .map_err(|e| AppError::command(format!("Failed to kill process: {}", e)))
    }

    /// Wait for the process to exit and return the exit status
    pub async fn wait(&mut self) -> AppResult<Option<i32>> {
        let status = self
            .child
            .wait()
            .await
            .map_err(|e| AppError::command(format!("Failed to wait for process: {}", e)))?;

        Ok(status.code())
    }

    /// Take the stdout handle (can only be called once)
    pub fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.child.stdout.take()
    }

    /// Take the stdin handle (can only be called once)
    pub fn take_stdin(&mut self) -> Option<tokio::process::ChildStdin> {
        self.child.stdin.take()
    }

    /// Take the stderr handle (can only be called once)
    pub fn take_stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.child.stderr.take()
    }
}

impl Drop for ClaudeCodeProcess {
    fn drop(&mut self) {
        // Attempt to kill the process on drop to prevent zombies
        // Use start_kill which is non-async
        let _ = self.child.start_kill();
    }
}

/// Configuration for spawning a Claude Code process
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    /// Working directory for the process
    pub working_dir: String,
    /// Additional CLI arguments
    pub extra_args: Vec<String>,
    /// Session ID for resuming (optional)
    pub resume_session_id: Option<String>,
    /// Model to use (optional, uses default if not set)
    pub model: Option<String>,
}

impl SpawnConfig {
    /// Create a new spawn configuration with minimal options
    pub fn new(working_dir: impl Into<String>) -> Self {
        Self {
            working_dir: working_dir.into(),
            extra_args: Vec::new(),
            resume_session_id: None,
            model: None,
        }
    }

    /// Set the session ID to resume
    pub fn with_resume(mut self, session_id: impl Into<String>) -> Self {
        self.resume_session_id = Some(session_id.into());
        self
    }

    /// Set the model to use
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Add extra CLI arguments
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }
}

/// Claude Code CLI Executor Service
///
/// Manages spawning and lifecycle of Claude Code CLI processes.
#[derive(Debug, Default)]
pub struct ClaudeCodeExecutor;

impl ClaudeCodeExecutor {
    /// Create a new executor instance
    pub fn new() -> Self {
        Self
    }

    /// Spawn a new Claude Code CLI process
    ///
    /// The process is started with `--output-format stream-json` for machine-readable output.
    /// Returns a handle that can be used to interact with the process.
    pub async fn spawn(&self, config: &SpawnConfig) -> AppResult<ClaudeCodeProcess> {
        let mut cmd = Command::new("claude");

        // Set working directory
        cmd.current_dir(&config.working_dir);

        // Always use stream-json format for machine parsing
        cmd.arg("--output-format").arg("stream-json");

        // Add resume flag if session ID provided
        if let Some(ref session_id) = config.resume_session_id {
            cmd.arg("--resume").arg(session_id);
        }

        // Add model if specified
        if let Some(ref model) = config.model {
            cmd.arg("--model").arg(model);
        }

        // Add any extra arguments
        for arg in &config.extra_args {
            cmd.arg(arg);
        }

        // Configure stdio
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Spawn the process
        let child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::command("Claude Code CLI not found. Please install it with: npm install -g @anthropic-ai/claude-code")
            } else {
                AppError::command(format!("Failed to spawn Claude Code CLI: {}", e))
            }
        })?;

        let pid = child.id().unwrap_or(0);

        Ok(ClaudeCodeProcess { child, pid })
    }

    /// Spawn a process and set up a line reader for stdout
    ///
    /// Returns both the process handle and a channel receiver that yields lines from stdout.
    pub async fn spawn_with_reader(
        &self,
        config: &SpawnConfig,
    ) -> AppResult<(ClaudeCodeProcess, mpsc::Receiver<String>)> {
        let mut process = self.spawn(config).await?;

        let stdout = process.take_stdout().ok_or_else(|| {
            AppError::command("Failed to capture stdout from Claude Code process")
        })?;

        let (tx, rx) = mpsc::channel(100);

        // Spawn a task to read lines from stdout
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(line).await.is_err() {
                    // Receiver dropped, stop reading
                    break;
                }
            }
        });

        Ok((process, rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_config_creation() {
        let config = SpawnConfig::new("/path/to/project");
        assert_eq!(config.working_dir, "/path/to/project");
        assert!(config.resume_session_id.is_none());
        assert!(config.model.is_none());
        assert!(config.extra_args.is_empty());
    }

    #[test]
    fn test_spawn_config_builder() {
        let config = SpawnConfig::new("/project")
            .with_resume("session-123")
            .with_model("claude-sonnet-4-20250514")
            .with_args(vec!["--verbose".to_string()]);

        assert_eq!(config.working_dir, "/project");
        assert_eq!(config.resume_session_id, Some("session-123".to_string()));
        assert_eq!(config.model, Some("claude-sonnet-4-20250514".to_string()));
        assert_eq!(config.extra_args, vec!["--verbose".to_string()]);
    }

    #[test]
    fn test_executor_creation() {
        let executor = ClaudeCodeExecutor::new();
        let _ = executor;
    }
}
