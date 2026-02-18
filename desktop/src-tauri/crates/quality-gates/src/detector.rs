//! Project Type Detection
//!
//! Automatically detects project type by looking for marker files
//! (package.json, Cargo.toml, pyproject.toml, go.mod).

use std::path::{Path, PathBuf};

use plan_cascade_core::{CoreError, CoreResult};
use crate::models::{ProjectDetectionResult, ProjectMetadata, ProjectType};

/// Project type detector
pub struct ProjectDetector {
    /// Project root path
    project_path: PathBuf,
}

impl ProjectDetector {
    /// Create a new project detector for the given path
    pub fn new(project_path: impl AsRef<Path>) -> Self {
        Self {
            project_path: project_path.as_ref().to_path_buf(),
        }
    }

    /// Detect the project type
    pub fn detect(&self) -> CoreResult<ProjectDetectionResult> {
        let detections = vec![
            (ProjectType::Rust, "Cargo.toml"),
            (ProjectType::NodeJs, "package.json"),
            (ProjectType::Python, "pyproject.toml"),
            (ProjectType::Python, "setup.py"),
            (ProjectType::Python, "requirements.txt"),
            (ProjectType::Go, "go.mod"),
        ];

        for (project_type, marker) in detections {
            let marker_path = self.project_path.join(marker);
            if marker_path.exists() {
                let metadata = self.extract_metadata(project_type, &marker_path)?;
                let suggested_gates = self.get_suggested_gates(project_type, &metadata);

                return Ok(ProjectDetectionResult {
                    project_type,
                    marker_file: Some(marker_path.to_string_lossy().into_owned()),
                    metadata,
                    suggested_gates,
                });
            }
        }

        Ok(ProjectDetectionResult {
            project_type: ProjectType::Unknown,
            marker_file: None,
            metadata: ProjectMetadata::default(),
            suggested_gates: Vec::new(),
        })
    }

    /// Extract metadata from the project configuration file
    fn extract_metadata(
        &self,
        project_type: ProjectType,
        marker_path: &Path,
    ) -> CoreResult<ProjectMetadata> {
        match project_type {
            ProjectType::NodeJs => self.extract_nodejs_metadata(marker_path),
            ProjectType::Rust => self.extract_rust_metadata(marker_path),
            ProjectType::Python => self.extract_python_metadata(marker_path),
            ProjectType::Go => self.extract_go_metadata(marker_path),
            ProjectType::Unknown => Ok(ProjectMetadata::default()),
        }
    }

    /// Extract metadata from package.json
    fn extract_nodejs_metadata(&self, marker_path: &Path) -> CoreResult<ProjectMetadata> {
        let content = std::fs::read_to_string(marker_path)?;
        let json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| CoreError::Parse(format!("Failed to parse package.json: {}", e)))?;

        let mut metadata = ProjectMetadata::default();

        metadata.name = json.get("name").and_then(|v| v.as_str()).map(String::from);
        metadata.version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from);

        let has_ts_dep = json
            .get("devDependencies")
            .and_then(|deps| deps.get("typescript"))
            .is_some();
        let tsconfig_exists = self.project_path.join("tsconfig.json").exists();
        metadata.has_typescript = has_ts_dep || tsconfig_exists;

        let has_eslint_dep = json
            .get("devDependencies")
            .and_then(|deps| deps.get("eslint"))
            .is_some();
        let eslint_config_exists = self.project_path.join(".eslintrc.js").exists()
            || self.project_path.join(".eslintrc.json").exists()
            || self.project_path.join(".eslintrc.yml").exists()
            || self.project_path.join("eslint.config.js").exists();
        metadata.has_eslint = has_eslint_dep || eslint_config_exists;

        let has_prettier_dep = json
            .get("devDependencies")
            .and_then(|deps| deps.get("prettier"))
            .is_some();
        let prettier_config_exists = self.project_path.join(".prettierrc").exists()
            || self.project_path.join(".prettierrc.json").exists()
            || self.project_path.join("prettier.config.js").exists();
        metadata.has_prettier = has_prettier_dep || prettier_config_exists;

        let scripts = json.get("scripts");
        metadata.has_tests = scripts.and_then(|s| s.get("test")).is_some();

        if json
            .get("devDependencies")
            .and_then(|d| d.get("jest"))
            .is_some()
        {
            metadata.test_framework = Some("jest".to_string());
        } else if json
            .get("devDependencies")
            .and_then(|d| d.get("vitest"))
            .is_some()
        {
            metadata.test_framework = Some("vitest".to_string());
        } else if json
            .get("devDependencies")
            .and_then(|d| d.get("mocha"))
            .is_some()
        {
            metadata.test_framework = Some("mocha".to_string());
        }

        metadata.has_ci = self.project_path.join(".github/workflows").exists()
            || self.project_path.join(".gitlab-ci.yml").exists()
            || self.project_path.join(".circleci").exists();

        Ok(metadata)
    }

    /// Extract metadata from Cargo.toml
    fn extract_rust_metadata(&self, marker_path: &Path) -> CoreResult<ProjectMetadata> {
        let content = std::fs::read_to_string(marker_path)?;
        let toml: toml::Value = content
            .parse()
            .map_err(|e| CoreError::Parse(format!("Failed to parse Cargo.toml: {}", e)))?;

        let mut metadata = ProjectMetadata::default();

        if let Some(package) = toml.get("package") {
            metadata.name = package
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from);
            metadata.version = package
                .get("version")
                .and_then(|v| v.as_str())
                .map(String::from);
        }

        metadata.has_tests = true;
        metadata.test_framework = Some("cargo test".to_string());
        metadata.has_eslint = true; // Clippy is the Rust equivalent

        metadata.has_ci = self.project_path.join(".github/workflows").exists()
            || self.project_path.join(".gitlab-ci.yml").exists();

        Ok(metadata)
    }

    /// Extract metadata from Python project files
    fn extract_python_metadata(&self, marker_path: &Path) -> CoreResult<ProjectMetadata> {
        let mut metadata = ProjectMetadata::default();

        let marker_name = marker_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if marker_name == "pyproject.toml" {
            let content = std::fs::read_to_string(marker_path)?;
            let toml: toml::Value = content
                .parse()
                .map_err(|e| CoreError::Parse(format!("Failed to parse pyproject.toml: {}", e)))?;

            if let Some(project) = toml.get("project") {
                metadata.name = project
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                metadata.version = project
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(String::from);
            } else if let Some(poetry) = toml.get("tool").and_then(|t| t.get("poetry")) {
                metadata.name = poetry
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                metadata.version = poetry
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(String::from);
            }

            let has_mypy = toml.get("tool").and_then(|t| t.get("mypy")).is_some();
            metadata.has_typescript = has_mypy;

            let has_pytest = toml.get("tool").and_then(|t| t.get("pytest")).is_some();
            if has_pytest {
                metadata.has_tests = true;
                metadata.test_framework = Some("pytest".to_string());
            }

            let has_ruff = toml.get("tool").and_then(|t| t.get("ruff")).is_some();
            let has_flake8 = toml.get("tool").and_then(|t| t.get("flake8")).is_some();
            metadata.has_eslint = has_ruff || has_flake8;

            let has_black = toml.get("tool").and_then(|t| t.get("black")).is_some();
            metadata.has_prettier = has_black;
        }

        let tests_dir = self.project_path.join("tests");
        if tests_dir.exists() {
            metadata.has_tests = true;
            if metadata.test_framework.is_none() {
                metadata.test_framework = Some("pytest".to_string());
            }
        }

        metadata.has_ci = self.project_path.join(".github/workflows").exists()
            || self.project_path.join(".gitlab-ci.yml").exists();

        Ok(metadata)
    }

    /// Extract metadata from go.mod
    fn extract_go_metadata(&self, marker_path: &Path) -> CoreResult<ProjectMetadata> {
        let content = std::fs::read_to_string(marker_path)?;
        let mut metadata = ProjectMetadata::default();

        for line in content.lines() {
            if line.starts_with("module ") {
                metadata.name = Some(line.trim_start_matches("module ").trim().to_string());
                break;
            }
        }

        metadata.has_tests = true;
        metadata.test_framework = Some("go test".to_string());
        metadata.has_eslint = true;

        metadata.has_ci = self.project_path.join(".github/workflows").exists()
            || self.project_path.join(".gitlab-ci.yml").exists();

        Ok(metadata)
    }

    /// Get suggested quality gates based on project type and metadata
    fn get_suggested_gates(
        &self,
        project_type: ProjectType,
        metadata: &ProjectMetadata,
    ) -> Vec<String> {
        let mut gates = Vec::new();

        match project_type {
            ProjectType::NodeJs => {
                if metadata.has_typescript {
                    gates.push("tsc".to_string());
                }
                if metadata.has_eslint {
                    gates.push("eslint".to_string());
                }
                if metadata.has_prettier {
                    gates.push("prettier".to_string());
                }
                if metadata.has_tests {
                    gates.push("test".to_string());
                }
            }
            ProjectType::Rust => {
                gates.push("cargo-check".to_string());
                gates.push("cargo-clippy".to_string());
                gates.push("cargo-fmt".to_string());
                gates.push("cargo-test".to_string());
            }
            ProjectType::Python => {
                if metadata.has_typescript {
                    gates.push("mypy".to_string());
                }
                if metadata.has_eslint {
                    gates.push("ruff".to_string());
                }
                if metadata.has_prettier {
                    gates.push("black".to_string());
                }
                if metadata.has_tests {
                    gates.push("pytest".to_string());
                }
            }
            ProjectType::Go => {
                gates.push("go-vet".to_string());
                gates.push("go-fmt".to_string());
                gates.push("go-test".to_string());
            }
            ProjectType::Unknown => {}
        }

        gates
    }
}

/// Detect project type for a given path
pub fn detect_project_type(project_path: impl AsRef<Path>) -> CoreResult<ProjectDetectionResult> {
    let detector = ProjectDetector::new(project_path);
    detector.detect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_detect_rust_project() {
        let temp = create_temp_dir();
        let cargo_toml = r#"
[package]
name = "test-crate"
version = "0.1.0"
edition = "2021"
"#;
        fs::write(temp.path().join("Cargo.toml"), cargo_toml).unwrap();

        let result = detect_project_type(temp.path()).unwrap();
        assert_eq!(result.project_type, ProjectType::Rust);
        assert_eq!(result.metadata.name, Some("test-crate".to_string()));
        assert_eq!(result.metadata.version, Some("0.1.0".to_string()));
    }

    #[test]
    fn test_detect_unknown_project() {
        let temp = create_temp_dir();

        let result = detect_project_type(temp.path()).unwrap();
        assert_eq!(result.project_type, ProjectType::Unknown);
        assert!(result.marker_file.is_none());
    }

    #[test]
    fn test_detect_go_project() {
        let temp = create_temp_dir();
        let go_mod = "module github.com/test/project\n\ngo 1.21\n";
        fs::write(temp.path().join("go.mod"), go_mod).unwrap();

        let result = detect_project_type(temp.path()).unwrap();
        assert_eq!(result.project_type, ProjectType::Go);
        assert_eq!(
            result.metadata.name,
            Some("github.com/test/project".to_string())
        );
    }
}
