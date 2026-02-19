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
use std::sync::Arc;
use std::time::Instant;

use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::models::quality_gates::ProjectType;
use crate::services::quality_gates::cache::GateCache;
use crate::services::quality_gates::detector::detect_project_type;
use crate::services::quality_gates::pipeline::{GatePhase, PipelineGateResult};

/// FormatGate runs the appropriate formatter for the detected project type.
pub struct FormatGate {
    /// Project root path
    project_path: PathBuf,
    /// Timeout for formatter commands (default 60s)
    timeout_secs: u64,
    /// Optional gate cache to invalidate after successful formatting
    cache: Option<Arc<GateCache>>,
}

impl FormatGate {
    /// Create a new FormatGate for the given project path.
    pub fn new(project_path: impl AsRef<Path>) -> Self {
        Self {
            project_path: project_path.as_ref().to_path_buf(),
            timeout_secs: 60,
            cache: None,
        }
    }

    /// Set the timeout for formatter commands.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set an optional gate cache to invalidate after successful formatting.
    ///
    /// After the formatter runs successfully and may have modified files,
    /// `invalidate_all()` is called on the cache so that subsequent quality
    /// gates (typecheck, test, lint) do not return stale cached results.
    pub fn with_cache(mut self, cache: Arc<GateCache>) -> Self {
        self.cache = Some(cache);
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
                    // Invalidate the gate cache after successful formatting
                    // because the formatter may have modified files, making
                    // cached results for typecheck/test/lint stale.
                    if let Some(ref cache) = self.cache {
                        if let Err(e) = cache.invalidate_all() {
                            tracing::warn!(
                                "FormatGate: failed to invalidate cache: {}",
                                e
                            );
                        }
                    }

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
    use r2d2::Pool;
    use r2d2_sqlite::SqliteConnectionManager;
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

    #[test]
    fn test_format_gate_with_cache_builder() {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).unwrap();
        let cache = Arc::new(GateCache::new(pool).unwrap());

        let gate = FormatGate::new("/test/path").with_cache(cache.clone());
        assert!(gate.cache.is_some());
    }

    #[test]
    fn test_format_gate_default_cache_is_none() {
        let gate = FormatGate::new("/test/path");
        assert!(gate.cache.is_none());
    }

    /// Verifies that `invalidate_all()` is called on the cache after a
    /// successful formatting run. Uses a real Rust project so that
    /// `cargo fmt` succeeds, and checks that pre-populated cache entries
    /// are cleared.
    #[tokio::test]
    async fn test_format_gate_invalidates_cache_after_success() {
        use crate::services::quality_gates::cache::GateCacheKey;

        // Create a minimal Rust project that `cargo fmt` can run on
        let temp = create_rust_project();
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("lib.rs"), "fn main() {}\n").unwrap();

        // Set up an in-memory gate cache with some pre-existing entries
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).unwrap();
        let cache = Arc::new(GateCache::new(pool).unwrap());

        let key = GateCacheKey {
            gate_id: "typecheck".to_string(),
            commit_hash: "abc123".to_string(),
            tree_hash: "def456".to_string(),
        };
        let result = PipelineGateResult::passed("typecheck", "TypeCheck", GatePhase::Validation, 100);
        cache.put(&key, &result).unwrap();
        assert_eq!(cache.count().unwrap(), 1, "Cache should have 1 entry before formatting");

        // Run FormatGate with the cache attached
        let gate = FormatGate::new(temp.path()).with_cache(cache.clone());
        let gate_result = gate.run().await;

        if gate_result.passed
            && gate_result.status != crate::models::quality_gates::GateStatus::Skipped
        {
            // Formatting succeeded (not skipped) -- cache should be invalidated
            assert_eq!(
                cache.count().unwrap(),
                0,
                "Cache should be empty after successful formatting"
            );
        }
        // If cargo fmt is not available, the gate may fail or be skipped;
        // in that case the cache should remain untouched.
    }
}
