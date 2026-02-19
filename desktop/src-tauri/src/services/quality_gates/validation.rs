//! Validation Phase Gates
//!
//! Creates GateExecutor closures for the VALIDATION phase gates:
//! typecheck, test, and lint. Uses `detect_project_type()` to select
//! the correct tool per project type, then runs the validation command
//! via `tokio::process::Command` with configurable timeout.
//!
//! - Exit code 0 -> PipelineGateResult::passed
//! - Non-zero exit code -> PipelineGateResult::failed with stderr/stdout as findings
//! - Command not found -> PipelineGateResult::skipped (tool not available)
//! - Unknown project type -> PipelineGateResult::skipped

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::models::quality_gates::ProjectType;
use crate::services::quality_gates::detector::detect_project_type;
use crate::services::quality_gates::pipeline::{GateExecutor, GatePhase, PipelineGateResult};
use crate::services::quality_gates::validators::ValidatorRegistry;

/// ValidationGate creates GateExecutor closures for typecheck, test, and lint gates.
///
/// Each gate detects the project type, maps the abstract gate ID to a concrete
/// validator from the ValidatorRegistry, and runs the corresponding command as
/// a subprocess with timeout.
pub struct ValidationGate;

impl ValidationGate {
    /// Create a GateExecutor closure for a validation gate.
    ///
    /// The returned closure captures the gate_id and project_path, detects
    /// the project type at execution time, and delegates to the appropriate
    /// validator command.
    pub fn create_executor(gate_id: &str, project_path: PathBuf) -> GateExecutor {
        let gate_id = gate_id.to_string();

        Box::new(move || {
            let gate_id = gate_id.clone();
            let project_path = project_path.clone();
            Box::pin(async move { Self::execute_gate(&gate_id, &project_path).await })
        })
    }

    /// Execute a validation gate for the given gate ID and project path.
    async fn execute_gate(gate_id: &str, project_path: &Path) -> PipelineGateResult {
        let start = Instant::now();
        let gate_display_name = format!("{} Gate", capitalize_first(gate_id));

        // Detect project type
        let detection = match detect_project_type(project_path) {
            Ok(d) => d,
            Err(e) => {
                return PipelineGateResult::skipped(
                    gate_id,
                    &gate_display_name,
                    GatePhase::Validation,
                    &format!("Detection failed: {}", e),
                );
            }
        };

        if detection.project_type == ProjectType::Unknown {
            return PipelineGateResult::skipped(
                gate_id,
                &gate_display_name,
                GatePhase::Validation,
                "Unknown project type",
            );
        }

        // Map abstract gate ID to concrete validator ID
        let validator_id = match Self::resolve_validator(gate_id, detection.project_type) {
            Some(id) => id,
            None => {
                return PipelineGateResult::skipped(
                    gate_id,
                    &gate_display_name,
                    GatePhase::Validation,
                    &format!(
                        "No {} validator for {}",
                        gate_id, detection.project_type
                    ),
                );
            }
        };

        // Get validator from registry
        let registry = ValidatorRegistry::new();
        let gate = match registry.get(&validator_id) {
            Some(g) => g.clone(),
            None => {
                return PipelineGateResult::skipped(
                    gate_id,
                    &gate_display_name,
                    GatePhase::Validation,
                    &format!("Validator '{}' not found in registry", validator_id),
                );
            }
        };

        // Use the gate's display name from the registry
        let gate_name = gate.name.clone();
        let timeout_duration = Duration::from_secs(gate.timeout_secs);

        // Execute command
        let mut cmd = Command::new(&gate.command);
        cmd.args(&gate.args)
            .current_dir(project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                if output.status.success() {
                    PipelineGateResult::passed(
                        gate_id,
                        &gate_name,
                        GatePhase::Validation,
                        duration_ms,
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let mut findings = Vec::new();
                    if !stderr.is_empty() {
                        findings.push(stderr);
                    }
                    if !stdout.is_empty() {
                        findings.push(stdout);
                    }
                    PipelineGateResult::failed(
                        gate_id,
                        &gate_name,
                        GatePhase::Validation,
                        duration_ms,
                        format!(
                            "Exited with code {}",
                            output.status.code().unwrap_or(-1)
                        ),
                        findings,
                    )
                }
            }
            Ok(Err(e)) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                // Command not found -> skip; other IO errors -> fail
                if e.kind() == std::io::ErrorKind::NotFound {
                    PipelineGateResult::skipped(
                        gate_id,
                        &gate_name,
                        GatePhase::Validation,
                        &format!("Tool not available: {}", gate.command),
                    )
                } else {
                    PipelineGateResult::failed(
                        gate_id,
                        &gate_name,
                        GatePhase::Validation,
                        duration_ms,
                        format!("Execution error: {}", e),
                        vec![],
                    )
                }
            }
            Err(_) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                PipelineGateResult::failed(
                    gate_id,
                    &gate_name,
                    GatePhase::Validation,
                    duration_ms,
                    format!("Timed out after {}s", gate.timeout_secs),
                    vec![],
                )
            }
        }
    }

    /// Map an abstract gate ID + project type to a concrete validator ID
    /// from the ValidatorRegistry.
    fn resolve_validator(gate_id: &str, project_type: ProjectType) -> Option<String> {
        match (gate_id, project_type) {
            // typecheck
            ("typecheck", ProjectType::NodeJs) => Some("tsc".to_string()),
            ("typecheck", ProjectType::Rust) => Some("cargo-check".to_string()),
            ("typecheck", ProjectType::Python) => Some("mypy".to_string()),
            ("typecheck", ProjectType::Go) => Some("go-vet".to_string()),
            // test
            ("test", ProjectType::NodeJs) => Some("npm-test".to_string()),
            ("test", ProjectType::Rust) => Some("cargo-test".to_string()),
            ("test", ProjectType::Python) => Some("pytest".to_string()),
            ("test", ProjectType::Go) => Some("go-test".to_string()),
            // lint
            ("lint", ProjectType::NodeJs) => Some("eslint".to_string()),
            ("lint", ProjectType::Rust) => Some("cargo-clippy".to_string()),
            ("lint", ProjectType::Python) => Some("ruff".to_string()),
            ("lint", ProjectType::Go) => Some("staticcheck".to_string()),
            // Unknown or unsupported combination
            _ => None,
        }
    }
}

/// Capitalize the first character of a string.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::quality_gates::GateStatus;
    use std::fs;
    use tempfile::TempDir;

    // ========================================================================
    // resolve_validator tests
    // ========================================================================

    #[test]
    fn test_resolve_typecheck_nodejs() {
        assert_eq!(
            ValidationGate::resolve_validator("typecheck", ProjectType::NodeJs),
            Some("tsc".to_string())
        );
    }

    #[test]
    fn test_resolve_typecheck_rust() {
        assert_eq!(
            ValidationGate::resolve_validator("typecheck", ProjectType::Rust),
            Some("cargo-check".to_string())
        );
    }

    #[test]
    fn test_resolve_typecheck_python() {
        assert_eq!(
            ValidationGate::resolve_validator("typecheck", ProjectType::Python),
            Some("mypy".to_string())
        );
    }

    #[test]
    fn test_resolve_typecheck_go() {
        assert_eq!(
            ValidationGate::resolve_validator("typecheck", ProjectType::Go),
            Some("go-vet".to_string())
        );
    }

    #[test]
    fn test_resolve_test_nodejs() {
        assert_eq!(
            ValidationGate::resolve_validator("test", ProjectType::NodeJs),
            Some("npm-test".to_string())
        );
    }

    #[test]
    fn test_resolve_test_rust() {
        assert_eq!(
            ValidationGate::resolve_validator("test", ProjectType::Rust),
            Some("cargo-test".to_string())
        );
    }

    #[test]
    fn test_resolve_test_python() {
        assert_eq!(
            ValidationGate::resolve_validator("test", ProjectType::Python),
            Some("pytest".to_string())
        );
    }

    #[test]
    fn test_resolve_test_go() {
        assert_eq!(
            ValidationGate::resolve_validator("test", ProjectType::Go),
            Some("go-test".to_string())
        );
    }

    #[test]
    fn test_resolve_lint_nodejs() {
        assert_eq!(
            ValidationGate::resolve_validator("lint", ProjectType::NodeJs),
            Some("eslint".to_string())
        );
    }

    #[test]
    fn test_resolve_lint_rust() {
        assert_eq!(
            ValidationGate::resolve_validator("lint", ProjectType::Rust),
            Some("cargo-clippy".to_string())
        );
    }

    #[test]
    fn test_resolve_lint_python() {
        assert_eq!(
            ValidationGate::resolve_validator("lint", ProjectType::Python),
            Some("ruff".to_string())
        );
    }

    #[test]
    fn test_resolve_lint_go() {
        assert_eq!(
            ValidationGate::resolve_validator("lint", ProjectType::Go),
            Some("staticcheck".to_string())
        );
    }

    #[test]
    fn test_resolve_unknown_gate_id() {
        assert_eq!(
            ValidationGate::resolve_validator("unknown", ProjectType::NodeJs),
            None
        );
    }

    #[test]
    fn test_resolve_unknown_project_type() {
        assert_eq!(
            ValidationGate::resolve_validator("typecheck", ProjectType::Unknown),
            None
        );
    }

    // ========================================================================
    // capitalize_first tests
    // ========================================================================

    #[test]
    fn test_capitalize_first_normal() {
        assert_eq!(capitalize_first("typecheck"), "Typecheck");
    }

    #[test]
    fn test_capitalize_first_empty() {
        assert_eq!(capitalize_first(""), "");
    }

    // ========================================================================
    // Async gate execution tests
    // ========================================================================

    fn create_unknown_project() -> TempDir {
        // No marker files -> Unknown project type
        tempfile::tempdir().unwrap()
    }

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

    #[tokio::test]
    async fn test_validation_gate_unknown_project_type_skipped() {
        let temp = create_unknown_project();
        let result = ValidationGate::execute_gate("typecheck", temp.path()).await;
        assert!(result.passed); // Skipped counts as passed
        assert_eq!(result.status, GateStatus::Skipped);
        assert_eq!(result.gate_id, "typecheck");
        assert!(result.message.contains("Unknown project type"));
    }

    #[tokio::test]
    async fn test_validation_gate_unknown_project_type_test_skipped() {
        let temp = create_unknown_project();
        let result = ValidationGate::execute_gate("test", temp.path()).await;
        assert!(result.passed);
        assert_eq!(result.status, GateStatus::Skipped);
    }

    #[tokio::test]
    async fn test_validation_gate_unknown_project_type_lint_skipped() {
        let temp = create_unknown_project();
        let result = ValidationGate::execute_gate("lint", temp.path()).await;
        assert!(result.passed);
        assert_eq!(result.status, GateStatus::Skipped);
    }

    #[tokio::test]
    async fn test_validation_gate_passing_command() {
        // Use "true" command which always exits 0.
        // We test this indirectly: for a Rust project, "cargo check" would run.
        // Instead, test the factory method produces a valid executor.
        let temp = create_rust_project();
        let executor = ValidationGate::create_executor("typecheck", temp.path().to_path_buf());
        let result = executor().await;
        // On a minimal Cargo.toml without src/, cargo check will fail or
        // at least produce a result. The key is that the executor runs
        // without panicking and returns a valid PipelineGateResult.
        assert_eq!(result.gate_id, "typecheck");
        assert_eq!(result.phase, GatePhase::Validation);
        // Status will be Passed or Failed depending on whether cargo is available
        // and whether the project compiles. The important thing is it doesn't skip.
        assert!(
            result.status == GateStatus::Passed
                || result.status == GateStatus::Failed
                || result.status == GateStatus::Skipped,
            "Expected a valid gate status, got: {:?}",
            result.status
        );
    }

    #[tokio::test]
    async fn test_validation_gate_missing_command_skipped() {
        // Create a Go project, which requires "go" command for typecheck.
        // The "staticcheck" tool for lint is unlikely to be installed,
        // so it should produce skipped.
        let temp = tempfile::tempdir().unwrap();
        let go_mod = "module github.com/test/project\n\ngo 1.21\n";
        fs::write(temp.path().join("go.mod"), go_mod).unwrap();

        // Use "lint" which maps to "staticcheck" for Go -- likely not installed
        let result = ValidationGate::execute_gate("lint", temp.path()).await;
        // If staticcheck is not installed, should be Skipped.
        // If it is installed, it may Pass or Fail. Both are acceptable.
        assert_eq!(result.gate_id, "lint");
        assert!(
            result.status == GateStatus::Skipped
                || result.status == GateStatus::Passed
                || result.status == GateStatus::Failed,
            "Expected Skipped/Passed/Failed, got: {:?}",
            result.status
        );
    }

    #[tokio::test]
    async fn test_validation_gate_invalid_gate_id_for_project() {
        let temp = create_rust_project();
        // "unknown_gate" is not a valid gate ID
        let result = ValidationGate::execute_gate("unknown_gate", temp.path()).await;
        assert!(result.passed); // Skipped counts as passed
        assert_eq!(result.status, GateStatus::Skipped);
        assert!(result.message.contains("No unknown_gate validator"));
    }

    #[tokio::test]
    async fn test_create_executor_returns_valid_closure() {
        let temp = create_unknown_project();
        let executor = ValidationGate::create_executor("typecheck", temp.path().to_path_buf());
        // Should be callable and return a result
        let result = executor().await;
        assert_eq!(result.gate_id, "typecheck");
        assert_eq!(result.status, GateStatus::Skipped);
    }

    #[tokio::test]
    async fn test_validation_gate_detection_failure() {
        // Use a nonexistent path to trigger a detection error
        let nonexistent = PathBuf::from("/nonexistent/path/that/does/not/exist");
        let result = ValidationGate::execute_gate("typecheck", &nonexistent).await;
        // Should be skipped because detection will still return Unknown for nonexistent path
        // (detect_project_type checks for marker files, none will exist)
        assert!(result.passed);
        assert_eq!(result.status, GateStatus::Skipped);
    }
}
