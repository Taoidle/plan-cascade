//! Quality Gates Models
//!
//! Data structures for project type detection and quality gate validation.

use serde::{Deserialize, Serialize};

/// Supported project types for quality gate detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    /// Node.js/JavaScript/TypeScript project (package.json)
    NodeJs,
    /// Rust project (Cargo.toml)
    Rust,
    /// Python project (pyproject.toml, setup.py, requirements.txt)
    Python,
    /// Go project (go.mod)
    Go,
    /// Unknown/unsupported project type
    Unknown,
}

impl ProjectType {
    /// Get the primary marker file for this project type
    pub fn marker_file(&self) -> Option<&'static str> {
        match self {
            ProjectType::NodeJs => Some("package.json"),
            ProjectType::Rust => Some("Cargo.toml"),
            ProjectType::Python => Some("pyproject.toml"),
            ProjectType::Go => Some("go.mod"),
            ProjectType::Unknown => None,
        }
    }

    /// Get all possible marker files for this project type
    pub fn all_markers(&self) -> Vec<&'static str> {
        match self {
            ProjectType::NodeJs => vec!["package.json"],
            ProjectType::Rust => vec!["Cargo.toml"],
            ProjectType::Python => vec!["pyproject.toml", "setup.py", "requirements.txt"],
            ProjectType::Go => vec!["go.mod"],
            ProjectType::Unknown => vec![],
        }
    }

    /// Get human-readable name
    pub fn display_name(&self) -> &'static str {
        match self {
            ProjectType::NodeJs => "Node.js",
            ProjectType::Rust => "Rust",
            ProjectType::Python => "Python",
            ProjectType::Go => "Go",
            ProjectType::Unknown => "Unknown",
        }
    }
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Quality gate status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateStatus {
    /// Gate passed successfully
    Passed,
    /// Gate failed with errors
    Failed,
    /// Gate was skipped (e.g., tool not available)
    Skipped,
    /// Gate is currently running
    Running,
    /// Gate is pending execution
    Pending,
}

impl GateStatus {
    /// Check if this status indicates success
    pub fn is_success(&self) -> bool {
        matches!(self, GateStatus::Passed | GateStatus::Skipped)
    }

    /// Check if this status indicates failure
    pub fn is_failure(&self) -> bool {
        matches!(self, GateStatus::Failed)
    }
}

impl std::fmt::Display for GateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateStatus::Passed => write!(f, "passed"),
            GateStatus::Failed => write!(f, "failed"),
            GateStatus::Skipped => write!(f, "skipped"),
            GateStatus::Running => write!(f, "running"),
            GateStatus::Pending => write!(f, "pending"),
        }
    }
}

/// A single quality gate definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGate {
    /// Unique identifier for this gate
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Arguments for the command
    pub args: Vec<String>,
    /// Working directory (relative to project root)
    pub working_dir: Option<String>,
    /// Environment variables
    pub env: std::collections::HashMap<String, String>,
    /// Whether this gate is required (failure blocks merge)
    pub required: bool,
    /// Timeout in seconds (default 300)
    pub timeout_secs: u64,
    /// Project types this gate applies to
    pub project_types: Vec<ProjectType>,
}

impl QualityGate {
    /// Create a new quality gate
    pub fn new(id: impl Into<String>, name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            command: command.into(),
            args: Vec::new(),
            working_dir: None,
            env: std::collections::HashMap::new(),
            required: true,
            timeout_secs: 300,
            project_types: Vec::new(),
        }
    }

    /// Add arguments
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Set working directory
    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Add environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set required flag
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set applicable project types
    pub fn for_project_types(mut self, types: Vec<ProjectType>) -> Self {
        self.project_types = types;
        self
    }
}

/// Result of running a quality gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Gate ID
    pub gate_id: String,
    /// Gate name
    pub gate_name: String,
    /// Status of the gate
    pub status: GateStatus,
    /// Exit code from the command (if executed)
    pub exit_code: Option<i32>,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Timestamp when the gate started
    pub started_at: i64,
    /// Timestamp when the gate finished
    pub finished_at: Option<i64>,
    /// Error message if gate failed to run
    pub error: Option<String>,
}

impl GateResult {
    /// Create a new gate result for a pending gate
    pub fn pending(gate: &QualityGate) -> Self {
        Self {
            gate_id: gate.id.clone(),
            gate_name: gate.name.clone(),
            status: GateStatus::Pending,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
            started_at: chrono::Utc::now().timestamp(),
            finished_at: None,
            error: None,
        }
    }

    /// Create a result for a passed gate
    pub fn passed(gate: &QualityGate, stdout: String, stderr: String, duration_ms: u64) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            gate_id: gate.id.clone(),
            gate_name: gate.name.clone(),
            status: GateStatus::Passed,
            exit_code: Some(0),
            stdout,
            stderr,
            duration_ms,
            started_at: now - (duration_ms as i64 / 1000),
            finished_at: Some(now),
            error: None,
        }
    }

    /// Create a result for a failed gate
    pub fn failed(
        gate: &QualityGate,
        exit_code: i32,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            gate_id: gate.id.clone(),
            gate_name: gate.name.clone(),
            status: GateStatus::Failed,
            exit_code: Some(exit_code),
            stdout,
            stderr,
            duration_ms,
            started_at: now - (duration_ms as i64 / 1000),
            finished_at: Some(now),
            error: None,
        }
    }

    /// Create a result for a skipped gate
    pub fn skipped(gate: &QualityGate, reason: impl Into<String>) -> Self {
        Self {
            gate_id: gate.id.clone(),
            gate_name: gate.name.clone(),
            status: GateStatus::Skipped,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
            started_at: chrono::Utc::now().timestamp(),
            finished_at: Some(chrono::Utc::now().timestamp()),
            error: Some(reason.into()),
        }
    }

    /// Create a result for an error during gate execution
    pub fn error(gate: &QualityGate, error: impl Into<String>) -> Self {
        Self {
            gate_id: gate.id.clone(),
            gate_name: gate.name.clone(),
            status: GateStatus::Failed,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
            started_at: chrono::Utc::now().timestamp(),
            finished_at: Some(chrono::Utc::now().timestamp()),
            error: Some(error.into()),
        }
    }
}

/// Summary of all quality gate results for a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatesSummary {
    /// Project path
    pub project_path: String,
    /// Detected project type
    pub project_type: ProjectType,
    /// Overall status (passed if all required gates passed)
    pub overall_status: GateStatus,
    /// Total number of gates
    pub total_gates: usize,
    /// Number of passed gates
    pub passed_gates: usize,
    /// Number of failed gates
    pub failed_gates: usize,
    /// Number of skipped gates
    pub skipped_gates: usize,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Individual gate results
    pub results: Vec<GateResult>,
    /// Timestamp when gates started running
    pub started_at: i64,
    /// Timestamp when all gates finished
    pub finished_at: Option<i64>,
}

impl GatesSummary {
    /// Create a new empty summary
    pub fn new(project_path: impl Into<String>, project_type: ProjectType) -> Self {
        Self {
            project_path: project_path.into(),
            project_type,
            overall_status: GateStatus::Pending,
            total_gates: 0,
            passed_gates: 0,
            failed_gates: 0,
            skipped_gates: 0,
            total_duration_ms: 0,
            results: Vec::new(),
            started_at: chrono::Utc::now().timestamp(),
            finished_at: None,
        }
    }

    /// Add a gate result and update counts
    pub fn add_result(&mut self, result: GateResult) {
        self.total_duration_ms += result.duration_ms;
        match result.status {
            GateStatus::Passed => self.passed_gates += 1,
            GateStatus::Failed => self.failed_gates += 1,
            GateStatus::Skipped => self.skipped_gates += 1,
            _ => {}
        }
        self.results.push(result);
        self.total_gates = self.results.len();
    }

    /// Finalize the summary and calculate overall status
    pub fn finalize(&mut self) {
        self.finished_at = Some(chrono::Utc::now().timestamp());
        self.overall_status = if self.failed_gates > 0 {
            GateStatus::Failed
        } else if self.passed_gates > 0 {
            GateStatus::Passed
        } else if self.skipped_gates > 0 && self.total_gates == self.skipped_gates {
            GateStatus::Skipped
        } else {
            GateStatus::Passed
        };
    }
}

/// Custom quality gate configuration from prd.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomGateConfig {
    /// Gate ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Command to run
    pub command: String,
    /// Arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Whether the gate is required
    #[serde(default = "default_required")]
    pub required: bool,
    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_required() -> bool {
    true
}

fn default_timeout() -> u64 {
    300
}

impl From<CustomGateConfig> for QualityGate {
    fn from(config: CustomGateConfig) -> Self {
        QualityGate::new(config.id, config.name, config.command)
            .with_args(config.args)
            .required(config.required)
            .with_timeout(config.timeout_secs)
    }
}

/// Project detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetectionResult {
    /// Detected project type
    pub project_type: ProjectType,
    /// Path to the detected marker file
    pub marker_file: Option<String>,
    /// Additional project metadata extracted from marker file
    pub metadata: ProjectMetadata,
    /// Suggested quality gates for this project
    pub suggested_gates: Vec<String>,
}

/// Project metadata extracted from configuration files
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// Project name
    pub name: Option<String>,
    /// Project version
    pub version: Option<String>,
    /// Has TypeScript configuration
    pub has_typescript: bool,
    /// Has ESLint configuration
    pub has_eslint: bool,
    /// Has Prettier configuration
    pub has_prettier: bool,
    /// Has test framework configured
    pub has_tests: bool,
    /// Detected test framework
    pub test_framework: Option<String>,
    /// Has CI/CD configuration
    pub has_ci: bool,
}

/// Stored quality gate result in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredGateResult {
    /// Database ID
    pub id: i64,
    /// Project path
    pub project_path: String,
    /// Session ID (if applicable)
    pub session_id: Option<String>,
    /// Gate ID
    pub gate_id: String,
    /// Gate name
    pub gate_name: String,
    /// Status
    pub status: String,
    /// Exit code
    pub exit_code: Option<i32>,
    /// Stdout (truncated if too long)
    pub stdout: String,
    /// Stderr (truncated if too long)
    pub stderr: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// When the gate was run
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_type_markers() {
        assert_eq!(ProjectType::NodeJs.marker_file(), Some("package.json"));
        assert_eq!(ProjectType::Rust.marker_file(), Some("Cargo.toml"));
        assert_eq!(ProjectType::Python.marker_file(), Some("pyproject.toml"));
        assert_eq!(ProjectType::Go.marker_file(), Some("go.mod"));
        assert_eq!(ProjectType::Unknown.marker_file(), None);
    }

    #[test]
    fn test_gate_status() {
        assert!(GateStatus::Passed.is_success());
        assert!(GateStatus::Skipped.is_success());
        assert!(!GateStatus::Failed.is_success());
        assert!(GateStatus::Failed.is_failure());
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
    fn test_gates_summary() {
        let mut summary = GatesSummary::new("/test/project", ProjectType::NodeJs);

        let gate = QualityGate::new("test", "Test", "test");
        summary.add_result(GateResult::passed(&gate, "ok".into(), "".into(), 1000));
        summary.add_result(GateResult::failed(&gate, 1, "".into(), "error".into(), 500));
        summary.finalize();

        assert_eq!(summary.total_gates, 2);
        assert_eq!(summary.passed_gates, 1);
        assert_eq!(summary.failed_gates, 1);
        assert_eq!(summary.overall_status, GateStatus::Failed);
    }
}
