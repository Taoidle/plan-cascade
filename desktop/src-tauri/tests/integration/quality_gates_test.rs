//! Quality Gates Integration Tests
//!
//! Tests for quality gate detection and execution across different project types.
//! These tests use temporary directories to create isolated test environments.

use std::fs;
use tempfile::TempDir;

use plan_cascade_desktop::models::quality_gates::{ProjectType, GateStatus, QualityGate, GatesSummary};
use plan_cascade_desktop::services::quality_gates::{ProjectDetector, detect_project_type};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a temporary Node.js project with full configuration
fn create_nodejs_project(temp: &TempDir) {
    let package_json = r#"{
        "name": "test-nodejs-project",
        "version": "1.0.0",
        "devDependencies": {
            "typescript": "^5.0.0",
            "eslint": "^8.0.0",
            "prettier": "^3.0.0",
            "vitest": "^1.0.0"
        },
        "scripts": {
            "test": "vitest run",
            "lint": "eslint .",
            "typecheck": "tsc --noEmit"
        }
    }"#;

    fs::write(temp.path().join("package.json"), package_json).unwrap();
    fs::write(temp.path().join("tsconfig.json"), r#"{"compilerOptions": {}}"#).unwrap();
    fs::write(temp.path().join(".eslintrc.json"), r#"{"env": {"node": true}}"#).unwrap();
}

/// Create a temporary Rust project
fn create_rust_project(temp: &TempDir) {
    let cargo_toml = r#"
[package]
name = "test-rust-project"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;

    fs::write(temp.path().join("Cargo.toml"), cargo_toml).unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src/lib.rs"), "pub fn hello() -> &'static str { \"hello\" }").unwrap();
}

/// Create a temporary Python project
fn create_python_project(temp: &TempDir) {
    let pyproject = r#"
[project]
name = "test-python-project"
version = "1.0.0"

[tool.pytest]
testpaths = ["tests"]

[tool.mypy]
strict = true

[tool.ruff]
line-length = 88
"#;

    fs::write(temp.path().join("pyproject.toml"), pyproject).unwrap();
    fs::create_dir_all(temp.path().join("tests")).unwrap();
    fs::write(temp.path().join("tests/test_example.py"), "def test_pass(): assert True").unwrap();
}

/// Create a temporary Go project
fn create_go_project(temp: &TempDir) {
    let go_mod = "module github.com/test/go-project\n\ngo 1.21\n";

    fs::write(temp.path().join("go.mod"), go_mod).unwrap();
    fs::write(temp.path().join("main.go"), "package main\n\nfunc main() {}\n").unwrap();
}

// ============================================================================
// Project Type Detection Tests
// ============================================================================

#[test]
fn test_detect_nodejs_project() {
    let temp = tempfile::tempdir().unwrap();
    create_nodejs_project(&temp);

    let result = detect_project_type(temp.path()).unwrap();

    assert_eq!(result.project_type, ProjectType::NodeJs);
    assert!(result.marker_file.is_some());
    assert!(result.marker_file.as_ref().unwrap().contains("package.json"));

    // Check metadata extraction
    assert!(result.metadata.has_typescript);
    assert!(result.metadata.has_eslint);
    assert!(result.metadata.has_tests);
    assert_eq!(result.metadata.name, Some("test-nodejs-project".to_string()));
}

#[test]
fn test_detect_rust_project() {
    let temp = tempfile::tempdir().unwrap();
    create_rust_project(&temp);

    let result = detect_project_type(temp.path()).unwrap();

    assert_eq!(result.project_type, ProjectType::Rust);
    assert!(result.marker_file.is_some());
    assert!(result.marker_file.as_ref().unwrap().contains("Cargo.toml"));

    // Rust projects always have tests via cargo
    assert!(result.metadata.has_tests);
    assert_eq!(result.metadata.name, Some("test-rust-project".to_string()));
    assert_eq!(result.metadata.version, Some("0.1.0".to_string()));
}

#[test]
fn test_detect_python_project() {
    let temp = tempfile::tempdir().unwrap();
    create_python_project(&temp);

    let result = detect_project_type(temp.path()).unwrap();

    assert_eq!(result.project_type, ProjectType::Python);
    assert!(result.marker_file.is_some());
    assert!(result.marker_file.as_ref().unwrap().contains("pyproject.toml"));

    // Check metadata
    assert!(result.metadata.has_tests);
    assert!(result.metadata.has_typescript); // mypy = type checking
    assert!(result.metadata.has_eslint); // ruff = linting
}

#[test]
fn test_detect_go_project() {
    let temp = tempfile::tempdir().unwrap();
    create_go_project(&temp);

    let result = detect_project_type(temp.path()).unwrap();

    assert_eq!(result.project_type, ProjectType::Go);
    assert!(result.marker_file.is_some());
    assert!(result.marker_file.as_ref().unwrap().contains("go.mod"));

    // Go projects always have tests via go test
    assert!(result.metadata.has_tests);
    assert_eq!(result.metadata.name, Some("github.com/test/go-project".to_string()));
}

#[test]
fn test_detect_unknown_project() {
    let temp = tempfile::tempdir().unwrap();
    // Don't create any marker files

    let result = detect_project_type(temp.path()).unwrap();

    assert_eq!(result.project_type, ProjectType::Unknown);
    assert!(result.marker_file.is_none());
    assert!(result.suggested_gates.is_empty());
}

#[test]
fn test_detection_priority_rust_over_nodejs() {
    let temp = tempfile::tempdir().unwrap();

    // Create both markers - Rust should take priority
    fs::write(temp.path().join("package.json"), r#"{"name": "test"}"#).unwrap();
    fs::write(temp.path().join("Cargo.toml"), r#"[package]
name = "test"
version = "0.1.0"
"#).unwrap();

    let result = detect_project_type(temp.path()).unwrap();

    // Rust has higher priority in detection order
    assert_eq!(result.project_type, ProjectType::Rust);
}

// ============================================================================
// Suggested Gates Tests
// ============================================================================

#[test]
fn test_nodejs_suggested_gates() {
    let temp = tempfile::tempdir().unwrap();
    create_nodejs_project(&temp);

    let result = detect_project_type(temp.path()).unwrap();

    // Should suggest TypeScript, ESLint, and test gates
    assert!(result.suggested_gates.contains(&"tsc".to_string()));
    assert!(result.suggested_gates.contains(&"eslint".to_string()));
    assert!(result.suggested_gates.contains(&"test".to_string()));
}

#[test]
fn test_rust_suggested_gates() {
    let temp = tempfile::tempdir().unwrap();
    create_rust_project(&temp);

    let result = detect_project_type(temp.path()).unwrap();

    // Should suggest Rust toolchain gates
    assert!(result.suggested_gates.contains(&"cargo-check".to_string()));
    assert!(result.suggested_gates.contains(&"cargo-clippy".to_string()));
    assert!(result.suggested_gates.contains(&"cargo-fmt".to_string()));
    assert!(result.suggested_gates.contains(&"cargo-test".to_string()));
}

#[test]
fn test_python_suggested_gates() {
    let temp = tempfile::tempdir().unwrap();
    create_python_project(&temp);

    let result = detect_project_type(temp.path()).unwrap();

    // Should suggest Python toolchain gates
    assert!(result.suggested_gates.contains(&"mypy".to_string()));
    assert!(result.suggested_gates.contains(&"ruff".to_string()));
    assert!(result.suggested_gates.contains(&"pytest".to_string()));
}

#[test]
fn test_go_suggested_gates() {
    let temp = tempfile::tempdir().unwrap();
    create_go_project(&temp);

    let result = detect_project_type(temp.path()).unwrap();

    // Should suggest Go toolchain gates
    assert!(result.suggested_gates.contains(&"go-vet".to_string()));
    assert!(result.suggested_gates.contains(&"go-fmt".to_string()));
    assert!(result.suggested_gates.contains(&"go-test".to_string()));
}

// ============================================================================
// Quality Gate Model Tests
// ============================================================================

#[test]
fn test_project_type_markers() {
    assert_eq!(ProjectType::NodeJs.marker_file(), Some("package.json"));
    assert_eq!(ProjectType::Rust.marker_file(), Some("Cargo.toml"));
    assert_eq!(ProjectType::Python.marker_file(), Some("pyproject.toml"));
    assert_eq!(ProjectType::Go.marker_file(), Some("go.mod"));
    assert_eq!(ProjectType::Unknown.marker_file(), None);
}

#[test]
fn test_gate_status_methods() {
    assert!(GateStatus::Passed.is_success());
    assert!(GateStatus::Skipped.is_success());
    assert!(!GateStatus::Failed.is_success());
    assert!(GateStatus::Failed.is_failure());
    assert!(!GateStatus::Passed.is_failure());
}

#[test]
fn test_quality_gate_builder() {
    let gate = QualityGate::new("test-gate", "Test Gate", "npm")
        .with_args(vec!["test".to_string()])
        .with_timeout(120)
        .required(false)
        .for_project_types(vec![ProjectType::NodeJs]);

    assert_eq!(gate.id, "test-gate");
    assert_eq!(gate.command, "npm");
    assert_eq!(gate.args, vec!["test"]);
    assert_eq!(gate.timeout_secs, 120);
    assert!(!gate.required);
    assert_eq!(gate.project_types, vec![ProjectType::NodeJs]);
}

#[test]
fn test_gates_summary_calculation() {
    let mut summary = GatesSummary::new("/test/project", ProjectType::NodeJs);

    let gate = QualityGate::new("test", "Test", "test");

    use plan_cascade_desktop::models::quality_gates::GateResult;

    summary.add_result(GateResult::passed(&gate, "ok".into(), "".into(), 1000));
    summary.add_result(GateResult::failed(&gate, 1, "".into(), "error".into(), 500));
    summary.finalize();

    assert_eq!(summary.total_gates, 2);
    assert_eq!(summary.passed_gates, 1);
    assert_eq!(summary.failed_gates, 1);
    assert_eq!(summary.overall_status, GateStatus::Failed);
}

// ============================================================================
// Metadata Extraction Tests
// ============================================================================

#[test]
fn test_nodejs_minimal_package_json() {
    let temp = tempfile::tempdir().unwrap();
    let minimal = r#"{"name": "minimal", "version": "0.0.1"}"#;
    fs::write(temp.path().join("package.json"), minimal).unwrap();

    let result = detect_project_type(temp.path()).unwrap();

    assert_eq!(result.project_type, ProjectType::NodeJs);
    assert_eq!(result.metadata.name, Some("minimal".to_string()));
    assert_eq!(result.metadata.version, Some("0.0.1".to_string()));
    assert!(!result.metadata.has_typescript);
    assert!(!result.metadata.has_eslint);
    assert!(!result.metadata.has_tests);
}

#[test]
fn test_nodejs_with_eslint_config_file() {
    let temp = tempfile::tempdir().unwrap();
    let package = r#"{"name": "test"}"#;
    fs::write(temp.path().join("package.json"), package).unwrap();
    fs::write(temp.path().join(".eslintrc.json"), r#"{}"#).unwrap();

    let result = detect_project_type(temp.path()).unwrap();

    // ESLint should be detected from config file even without dep
    assert!(result.metadata.has_eslint);
}

#[test]
fn test_python_with_tests_directory() {
    let temp = tempfile::tempdir().unwrap();
    let pyproject = r#"[project]
name = "test"
"#;
    fs::write(temp.path().join("pyproject.toml"), pyproject).unwrap();
    fs::create_dir_all(temp.path().join("tests")).unwrap();
    fs::write(temp.path().join("tests/test_main.py"), "").unwrap();

    let result = detect_project_type(temp.path()).unwrap();

    // Should detect tests from directory
    assert!(result.metadata.has_tests);
}

#[test]
fn test_project_with_github_ci() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("package.json"), r#"{"name": "test"}"#).unwrap();
    fs::create_dir_all(temp.path().join(".github/workflows")).unwrap();
    fs::write(temp.path().join(".github/workflows/ci.yml"), "").unwrap();

    let result = detect_project_type(temp.path()).unwrap();

    assert!(result.metadata.has_ci);
}
