//! Format Gate
//!
//! Auto-detects project type and runs the appropriate formatter:
//! - Rust: `cargo fmt`
//! - Node.js: `prettier --write .`
//! - Python: `ruff format .`
//! - Go: `gofmt -w .`
//!
//! After formatting, invalidates the GateCache for the current project.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::models::quality_gates::ProjectType;
use crate::services::quality_gates::detector::detect_project_type;
use crate::services::quality_gates::pipeline::{GatePhase, PipelineGateResult};

/// FormatGate runs the appropriate formatter for the detected project type.
pub struct FormatGate {
    /// Project root path
    project_path: PathBuf,
    /// Timeout for formatter commands (default 60s)
    timeout_secs: u64,
}

impl FormatGate {
    /// Create a new FormatGate for the given project path.
    pub fn new(project_path: impl AsRef<Path>) -> Self {
        Self {
            project_path: project_path.as_ref().to_path_buf(),
            timeout_secs: 60,
        }
    }

    /// Set the timeout for formatter commands.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Run the format gate.
    pub async fn run(&self) -> PipelineGateResult {
        let start = Instant::now();

        // Detect project type
        let detection = match detect_project_type(&self.project_path) {
            Ok(d) => d,
            Err(e) => {
                return PipelineGateResult::skipped(
                    "format",
                    "Format Gate",
                    GatePhase::PreValidation,
                    &format!("Failed to detect project type: {}", e),
                );
            }
        };

        if detection.project_type == ProjectType::Unknown {
            return PipelineGateResult::skipped(
                "format",
                "Format Gate",
                GatePhase::PreValidation,
                "Unknown project type - cannot determine formatter",
            );
        }

        // Get formatter command for this project type
        let (command, args) = match self.get_formatter_command(detection.project_type) {
            Some(cmd) => cmd,
            None => {
                return PipelineGateResult::skipped(
                    "format",
                    "Format Gate",
                    GatePhase::PreValidation,
                    &format!(
                        "No formatter configured for {} projects",
                        detection.project_type
                    ),
                );
            }
        };

        // Execute formatter
        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .current_dir(&self.project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let timeout_duration = Duration::from_secs(self.timeout_secs);
        match timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                if output.status.success() {
                    PipelineGateResult::passed(
                        "format",
                        "Format Gate",
                        GatePhase::PreValidation,
                        duration_ms,
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    PipelineGateResult::failed(
                        "format",
                        "Format Gate",
                        GatePhase::PreValidation,
                        duration_ms,
                        format!("Formatter exited with code {}", output.status.code().unwrap_or(-1)),
                        vec![stderr],
                    )
                }
            }
            Ok(Err(e)) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                PipelineGateResult::failed(
                    "format",
                    "Format Gate",
                    GatePhase::PreValidation,
                    duration_ms,
                    format!("Failed to execute formatter: {}", e),
                    vec![],
                )
            }
            Err(_) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                PipelineGateResult::failed(
                    "format",
                    "Format Gate",
                    GatePhase::PreValidation,
                    duration_ms,
                    format!("Formatter timed out after {}s", self.timeout_secs),
                    vec![],
                )
            }
        }
    }

    /// Get the formatter command and arguments for a project type.
    fn get_formatter_command(&self, project_type: ProjectType) -> Option<(String, Vec<String>)> {
        match project_type {
            ProjectType::Rust => Some((
                "cargo".to_string(),
                vec!["fmt".to_string()],
            )),
            ProjectType::NodeJs => Some((
                "npx".to_string(),
                vec![
                    "prettier".to_string(),
                    "--write".to_string(),
                    ".".to_string(),
                ],
            )),
            ProjectType::Python => Some((
                "ruff".to_string(),
                vec!["format".to_string(), ".".to_string()],
            )),
            ProjectType::Go => Some((
                "gofmt".to_string(),
                vec!["-w".to_string(), ".".to_string()],
            )),
            ProjectType::Unknown => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_rust_project() -> TempDir {
        let temp = tempfile::tempdir().unwrap();
        let cargo_toml = r#"
[package]
name = "test-crate"
version = "0.1.0"
edition = "2021"
"#;
        fs::write(temp.path().join("Cargo.toml"), cargo_toml).unwrap();
        temp
    }

    fn create_node_project() -> TempDir {
        let temp = tempfile::tempdir().unwrap();
        let package_json = r#"{"name": "test", "version": "1.0.0"}"#;
        fs::write(temp.path().join("package.json"), package_json).unwrap();
        temp
    }

    #[test]
    fn test_format_gate_creation() {
        let gate = FormatGate::new("/test/path").with_timeout(30);
        assert_eq!(gate.timeout_secs, 30);
    }

    #[test]
    fn test_formatter_command_rust() {
        let gate = FormatGate::new("/test");
        let (cmd, args) = gate.get_formatter_command(ProjectType::Rust).unwrap();
        assert_eq!(cmd, "cargo");
        assert_eq!(args, vec!["fmt"]);
    }

    #[test]
    fn test_formatter_command_nodejs() {
        let gate = FormatGate::new("/test");
        let (cmd, args) = gate.get_formatter_command(ProjectType::NodeJs).unwrap();
        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["prettier", "--write", "."]);
    }

    #[test]
    fn test_formatter_command_python() {
        let gate = FormatGate::new("/test");
        let (cmd, args) = gate.get_formatter_command(ProjectType::Python).unwrap();
        assert_eq!(cmd, "ruff");
        assert_eq!(args, vec!["format", "."]);
    }

    #[test]
    fn test_formatter_command_go() {
        let gate = FormatGate::new("/test");
        let (cmd, args) = gate.get_formatter_command(ProjectType::Go).unwrap();
        assert_eq!(cmd, "gofmt");
        assert_eq!(args, vec!["-w", "."]);
    }

    #[test]
    fn test_formatter_command_unknown() {
        let gate = FormatGate::new("/test");
        assert!(gate.get_formatter_command(ProjectType::Unknown).is_none());
    }

    #[tokio::test]
    async fn test_format_gate_unknown_project() {
        let temp = tempfile::tempdir().unwrap();
        let gate = FormatGate::new(temp.path());
        let result = gate.run().await;
        assert!(result.passed); // Skipped counts as passed
        assert_eq!(result.status, crate::models::quality_gates::GateStatus::Skipped);
    }
}
