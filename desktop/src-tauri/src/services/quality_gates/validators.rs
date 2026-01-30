//! Validation Runners
//!
//! Provides pre-configured validators for different project types:
//! - Node.js: tsc, eslint, prettier, npm test
//! - Rust: cargo check, cargo clippy, cargo fmt, cargo test
//! - Python: mypy, ruff/flake8, black, pytest
//! - Go: go vet, gofmt, go test

use std::collections::HashMap;

use crate::models::quality_gates::{ProjectType, QualityGate};

/// Validator registry containing pre-configured quality gates
pub struct ValidatorRegistry {
    /// All registered validators
    validators: HashMap<String, QualityGate>,
}

impl ValidatorRegistry {
    /// Create a new validator registry with default validators
    pub fn new() -> Self {
        let mut registry = Self {
            validators: HashMap::new(),
        };
        registry.register_default_validators();
        registry
    }

    /// Register all default validators
    fn register_default_validators(&mut self) {
        // Node.js validators
        self.register_nodejs_validators();
        // Rust validators
        self.register_rust_validators();
        // Python validators
        self.register_python_validators();
        // Go validators
        self.register_go_validators();
    }

    /// Register Node.js validators
    fn register_nodejs_validators(&mut self) {
        // TypeScript compiler
        self.register(
            QualityGate::new("tsc", "TypeScript Compiler", "npx")
                .with_args(vec!["tsc".to_string(), "--noEmit".to_string()])
                .for_project_types(vec![ProjectType::NodeJs])
                .with_timeout(120),
        );

        // ESLint
        self.register(
            QualityGate::new("eslint", "ESLint", "npx")
                .with_args(vec!["eslint".to_string(), ".".to_string()])
                .for_project_types(vec![ProjectType::NodeJs])
                .with_timeout(180),
        );

        // Prettier check
        self.register(
            QualityGate::new("prettier", "Prettier Check", "npx")
                .with_args(vec!["prettier".to_string(), "--check".to_string(), ".".to_string()])
                .for_project_types(vec![ProjectType::NodeJs])
                .required(false)
                .with_timeout(60),
        );

        // npm test
        self.register(
            QualityGate::new("npm-test", "npm test", "npm")
                .with_args(vec!["test".to_string()])
                .for_project_types(vec![ProjectType::NodeJs])
                .with_timeout(300),
        );

        // npm run build
        self.register(
            QualityGate::new("npm-build", "npm build", "npm")
                .with_args(vec!["run".to_string(), "build".to_string()])
                .for_project_types(vec![ProjectType::NodeJs])
                .required(false)
                .with_timeout(300),
        );
    }

    /// Register Rust validators
    fn register_rust_validators(&mut self) {
        // cargo check
        self.register(
            QualityGate::new("cargo-check", "Cargo Check", "cargo")
                .with_args(vec!["check".to_string()])
                .for_project_types(vec![ProjectType::Rust])
                .with_timeout(300),
        );

        // cargo clippy
        self.register(
            QualityGate::new("cargo-clippy", "Cargo Clippy", "cargo")
                .with_args(vec![
                    "clippy".to_string(),
                    "--".to_string(),
                    "-D".to_string(),
                    "warnings".to_string(),
                ])
                .for_project_types(vec![ProjectType::Rust])
                .with_timeout(300),
        );

        // cargo fmt check
        self.register(
            QualityGate::new("cargo-fmt", "Cargo Format Check", "cargo")
                .with_args(vec!["fmt".to_string(), "--".to_string(), "--check".to_string()])
                .for_project_types(vec![ProjectType::Rust])
                .required(false)
                .with_timeout(60),
        );

        // cargo test
        self.register(
            QualityGate::new("cargo-test", "Cargo Test", "cargo")
                .with_args(vec!["test".to_string()])
                .for_project_types(vec![ProjectType::Rust])
                .with_timeout(600),
        );

        // cargo build
        self.register(
            QualityGate::new("cargo-build", "Cargo Build", "cargo")
                .with_args(vec!["build".to_string()])
                .for_project_types(vec![ProjectType::Rust])
                .required(false)
                .with_timeout(600),
        );
    }

    /// Register Python validators
    fn register_python_validators(&mut self) {
        // mypy type checker
        self.register(
            QualityGate::new("mypy", "MyPy Type Checker", "mypy")
                .with_args(vec![".".to_string()])
                .for_project_types(vec![ProjectType::Python])
                .with_timeout(180),
        );

        // ruff linter (modern, fast)
        self.register(
            QualityGate::new("ruff", "Ruff Linter", "ruff")
                .with_args(vec!["check".to_string(), ".".to_string()])
                .for_project_types(vec![ProjectType::Python])
                .with_timeout(60),
        );

        // flake8 linter (legacy)
        self.register(
            QualityGate::new("flake8", "Flake8 Linter", "flake8")
                .with_args(vec![".".to_string()])
                .for_project_types(vec![ProjectType::Python])
                .required(false)
                .with_timeout(120),
        );

        // black formatter check
        self.register(
            QualityGate::new("black", "Black Format Check", "black")
                .with_args(vec!["--check".to_string(), ".".to_string()])
                .for_project_types(vec![ProjectType::Python])
                .required(false)
                .with_timeout(60),
        );

        // pytest
        self.register(
            QualityGate::new("pytest", "Pytest", "pytest")
                .with_args(vec![])
                .for_project_types(vec![ProjectType::Python])
                .with_timeout(600),
        );

        // pip check (dependency validation)
        self.register(
            QualityGate::new("pip-check", "Pip Dependency Check", "pip")
                .with_args(vec!["check".to_string()])
                .for_project_types(vec![ProjectType::Python])
                .required(false)
                .with_timeout(30),
        );
    }

    /// Register Go validators
    fn register_go_validators(&mut self) {
        // go vet
        self.register(
            QualityGate::new("go-vet", "Go Vet", "go")
                .with_args(vec!["vet".to_string(), "./...".to_string()])
                .for_project_types(vec![ProjectType::Go])
                .with_timeout(180),
        );

        // gofmt check
        self.register(
            QualityGate::new("go-fmt", "Go Format Check", "gofmt")
                .with_args(vec!["-l".to_string(), ".".to_string()])
                .for_project_types(vec![ProjectType::Go])
                .required(false)
                .with_timeout(60),
        );

        // go build
        self.register(
            QualityGate::new("go-build", "Go Build", "go")
                .with_args(vec!["build".to_string(), "./...".to_string()])
                .for_project_types(vec![ProjectType::Go])
                .with_timeout(300),
        );

        // go test
        self.register(
            QualityGate::new("go-test", "Go Test", "go")
                .with_args(vec!["test".to_string(), "./...".to_string()])
                .for_project_types(vec![ProjectType::Go])
                .with_timeout(600),
        );

        // staticcheck (if available)
        self.register(
            QualityGate::new("staticcheck", "Staticcheck", "staticcheck")
                .with_args(vec!["./...".to_string()])
                .for_project_types(vec![ProjectType::Go])
                .required(false)
                .with_timeout(180),
        );
    }

    /// Register a quality gate
    pub fn register(&mut self, gate: QualityGate) {
        self.validators.insert(gate.id.clone(), gate);
    }

    /// Get a validator by ID
    pub fn get(&self, id: &str) -> Option<&QualityGate> {
        self.validators.get(id)
    }

    /// Get all validators for a project type
    pub fn get_for_project_type(&self, project_type: ProjectType) -> Vec<&QualityGate> {
        self.validators
            .values()
            .filter(|gate| gate.project_types.contains(&project_type))
            .collect()
    }

    /// Get validators by IDs
    pub fn get_by_ids(&self, ids: &[String]) -> Vec<&QualityGate> {
        ids.iter()
            .filter_map(|id| self.validators.get(id))
            .collect()
    }

    /// Get all registered validators
    pub fn all(&self) -> Vec<&QualityGate> {
        self.validators.values().collect()
    }

    /// Check if a validator exists
    pub fn contains(&self, id: &str) -> bool {
        self.validators.contains_key(id)
    }
}

impl Default for ValidatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the default quality gates for a project type
pub fn get_default_gates(project_type: ProjectType) -> Vec<QualityGate> {
    let registry = ValidatorRegistry::new();
    registry
        .get_for_project_type(project_type)
        .into_iter()
        .cloned()
        .collect()
}

/// Get quality gates by their IDs
pub fn get_gates_by_ids(ids: &[String]) -> Vec<QualityGate> {
    let registry = ValidatorRegistry::new();
    registry
        .get_by_ids(ids)
        .into_iter()
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = ValidatorRegistry::new();
        assert!(registry.contains("tsc"));
        assert!(registry.contains("cargo-check"));
        assert!(registry.contains("mypy"));
        assert!(registry.contains("go-vet"));
    }

    #[test]
    fn test_get_nodejs_validators() {
        let registry = ValidatorRegistry::new();
        let validators = registry.get_for_project_type(ProjectType::NodeJs);

        let ids: Vec<&str> = validators.iter().map(|v| v.id.as_str()).collect();
        assert!(ids.contains(&"tsc"));
        assert!(ids.contains(&"eslint"));
        assert!(ids.contains(&"npm-test"));
    }

    #[test]
    fn test_get_rust_validators() {
        let registry = ValidatorRegistry::new();
        let validators = registry.get_for_project_type(ProjectType::Rust);

        let ids: Vec<&str> = validators.iter().map(|v| v.id.as_str()).collect();
        assert!(ids.contains(&"cargo-check"));
        assert!(ids.contains(&"cargo-clippy"));
        assert!(ids.contains(&"cargo-test"));
    }

    #[test]
    fn test_get_python_validators() {
        let registry = ValidatorRegistry::new();
        let validators = registry.get_for_project_type(ProjectType::Python);

        let ids: Vec<&str> = validators.iter().map(|v| v.id.as_str()).collect();
        assert!(ids.contains(&"mypy"));
        assert!(ids.contains(&"pytest"));
    }

    #[test]
    fn test_get_go_validators() {
        let registry = ValidatorRegistry::new();
        let validators = registry.get_for_project_type(ProjectType::Go);

        let ids: Vec<&str> = validators.iter().map(|v| v.id.as_str()).collect();
        assert!(ids.contains(&"go-vet"));
        assert!(ids.contains(&"go-test"));
    }

    #[test]
    fn test_get_by_ids() {
        let registry = ValidatorRegistry::new();
        let validators = registry.get_by_ids(&[
            "tsc".to_string(),
            "cargo-check".to_string(),
            "nonexistent".to_string(),
        ]);

        assert_eq!(validators.len(), 2);
    }

    #[test]
    fn test_custom_validator() {
        let mut registry = ValidatorRegistry::new();
        registry.register(
            QualityGate::new("custom-gate", "Custom Gate", "custom-command")
                .with_args(vec!["--flag".to_string()])
        );

        assert!(registry.contains("custom-gate"));
        let gate = registry.get("custom-gate").unwrap();
        assert_eq!(gate.command, "custom-command");
    }
}
