//! Mega Plan Models
//!
//! Data structures for multi-feature orchestration with dependencies.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Status of a feature in the mega plan
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FeatureStatus {
    #[default]
    Pending,
    Creating,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for FeatureStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeatureStatus::Pending => write!(f, "pending"),
            FeatureStatus::Creating => write!(f, "creating"),
            FeatureStatus::InProgress => write!(f, "in_progress"),
            FeatureStatus::Completed => write!(f, "completed"),
            FeatureStatus::Failed => write!(f, "failed"),
            FeatureStatus::Skipped => write!(f, "skipped"),
        }
    }
}

/// A single feature in the mega plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    /// Unique feature identifier
    pub id: String,
    /// Feature name
    pub name: String,
    /// Feature description
    #[serde(default)]
    pub description: String,
    /// List of feature IDs this feature depends on
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Current status
    #[serde(default)]
    pub status: FeatureStatus,
    /// Priority (lower is higher priority)
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// Estimated complexity (1-5)
    #[serde(default)]
    pub complexity: Option<u8>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_priority() -> u32 {
    100
}

impl Feature {
    /// Create a new feature
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            dependencies: Vec::new(),
            status: FeatureStatus::default(),
            priority: default_priority(),
            complexity: None,
            tags: Vec::new(),
        }
    }

    /// Check if dependencies are satisfied
    pub fn dependencies_satisfied(&self, completed: &std::collections::HashSet<String>) -> bool {
        self.dependencies.iter().all(|dep| completed.contains(dep))
    }
}

/// The complete mega plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MegaPlan {
    /// Plan version
    #[serde(default = "default_version")]
    pub version: String,
    /// Project name
    pub name: String,
    /// Project description
    #[serde(default)]
    pub description: String,
    /// Target branch for merging all features
    #[serde(default = "default_target_branch")]
    pub target_branch: String,
    /// All features in the plan
    pub features: Vec<Feature>,
    /// Creation timestamp
    #[serde(default)]
    pub created_at: Option<String>,
    /// Last update timestamp
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_target_branch() -> String {
    "main".to_string()
}

impl MegaPlan {
    /// Create a new mega plan
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: default_version(),
            name: name.into(),
            description: String::new(),
            target_branch: default_target_branch(),
            features: Vec::new(),
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            updated_at: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a feature to the plan
    pub fn add_feature(&mut self, feature: Feature) {
        self.features.push(feature);
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Get a feature by ID
    pub fn get_feature(&self, id: &str) -> Option<&Feature> {
        self.features.iter().find(|f| f.id == id)
    }

    /// Get completed feature IDs
    pub fn completed_feature_ids(&self) -> std::collections::HashSet<String> {
        self.features
            .iter()
            .filter(|f| f.status == FeatureStatus::Completed)
            .map(|f| f.id.clone())
            .collect()
    }

    /// Check if all features are complete
    pub fn is_complete(&self) -> bool {
        self.features.iter().all(|f| f.status == FeatureStatus::Completed)
    }

    /// Load from file
    pub fn from_file(path: &std::path::Path) -> Result<Self, MegaPlanError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| MegaPlanError::IoError(e.to_string()))?;
        serde_json::from_str(&content)
            .map_err(|e| MegaPlanError::ParseError(e.to_string()))
    }

    /// Save to file
    pub fn to_file(&self, path: &std::path::Path) -> Result<(), MegaPlanError> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| MegaPlanError::SerializeError(e.to_string()))?;
        std::fs::write(path, content)
            .map_err(|e| MegaPlanError::IoError(e.to_string()))
    }
}

/// Runtime state of a feature during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureState {
    /// Current status
    pub status: FeatureStatus,
    /// Worktree path if created
    pub worktree: Option<PathBuf>,
    /// Whether PRD has been generated
    pub prd_generated: bool,
    /// Path to the generated PRD
    pub prd_path: Option<PathBuf>,
    /// Number of completed stories
    pub stories_completed: usize,
    /// Total number of stories
    pub stories_total: usize,
    /// Error message if failed
    pub error: Option<String>,
    /// Started timestamp
    pub started_at: Option<String>,
    /// Completed timestamp
    pub completed_at: Option<String>,
}

impl Default for FeatureState {
    fn default() -> Self {
        Self {
            status: FeatureStatus::Pending,
            worktree: None,
            prd_generated: false,
            prd_path: None,
            stories_completed: 0,
            stories_total: 0,
            error: None,
            started_at: None,
            completed_at: None,
        }
    }
}

impl FeatureState {
    /// Mark as started
    pub fn start(&mut self) {
        self.status = FeatureStatus::InProgress;
        self.started_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark as completed
    pub fn complete(&mut self) {
        self.status = FeatureStatus::Completed;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = FeatureStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// Overall status of the mega plan execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MegaStatus {
    /// Plan ID/name
    pub plan_id: String,
    /// Current batch number (0-indexed)
    pub current_batch: usize,
    /// Completed batch indices
    pub completed_batches: Vec<usize>,
    /// Feature states by feature ID
    pub features: HashMap<String, FeatureState>,
    /// Overall status
    pub status: MegaExecutionStatus,
    /// Started timestamp
    pub started_at: Option<String>,
    /// Last update timestamp
    pub updated_at: Option<String>,
    /// Completed timestamp
    pub completed_at: Option<String>,
    /// Error if failed
    pub error: Option<String>,
}

/// Overall execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MegaExecutionStatus {
    #[default]
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl MegaStatus {
    /// Create new status for a plan
    pub fn new(plan_id: impl Into<String>) -> Self {
        Self {
            plan_id: plan_id.into(),
            current_batch: 0,
            completed_batches: Vec::new(),
            features: HashMap::new(),
            status: MegaExecutionStatus::Pending,
            started_at: None,
            updated_at: None,
            completed_at: None,
            error: None,
        }
    }

    /// Start execution
    pub fn start(&mut self) {
        self.status = MegaExecutionStatus::Running;
        self.started_at = Some(chrono::Utc::now().to_rfc3339());
        self.updated_at = self.started_at.clone();
    }

    /// Mark as completed
    pub fn complete(&mut self) {
        self.status = MegaExecutionStatus::Completed;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.updated_at = self.completed_at.clone();
    }

    /// Mark as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = MegaExecutionStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.updated_at = self.completed_at.clone();
    }

    /// Get completion percentage
    pub fn completion_percentage(&self) -> f32 {
        if self.features.is_empty() {
            return 0.0;
        }
        let completed = self.features
            .values()
            .filter(|s| s.status == FeatureStatus::Completed)
            .count();
        (completed as f32 / self.features.len() as f32) * 100.0
    }

    /// Save to file
    pub fn to_file(&self, path: &std::path::Path) -> Result<(), MegaPlanError> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| MegaPlanError::SerializeError(e.to_string()))?;
        std::fs::write(path, content)
            .map_err(|e| MegaPlanError::IoError(e.to_string()))
    }

    /// Load from file
    pub fn from_file(path: &std::path::Path) -> Result<Self, MegaPlanError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| MegaPlanError::IoError(e.to_string()))?;
        serde_json::from_str(&content)
            .map_err(|e| MegaPlanError::ParseError(e.to_string()))
    }
}

/// Errors for mega plan operations
#[derive(Debug, thiserror::Error)]
pub enum MegaPlanError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Serialize error: {0}")]
    SerializeError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_creation() {
        let feature = Feature::new("F001", "User Authentication");
        assert_eq!(feature.id, "F001");
        assert_eq!(feature.status, FeatureStatus::Pending);
    }

    #[test]
    fn test_mega_plan_creation() {
        let mut plan = MegaPlan::new("Test Project");
        plan.add_feature(Feature::new("F001", "Feature 1"));
        plan.add_feature(Feature::new("F002", "Feature 2"));

        assert_eq!(plan.features.len(), 2);
        assert!(!plan.is_complete());
    }

    #[test]
    fn test_feature_dependencies() {
        let mut feature = Feature::new("F002", "Feature 2");
        feature.dependencies = vec!["F001".to_string()];

        let mut completed = std::collections::HashSet::new();
        assert!(!feature.dependencies_satisfied(&completed));

        completed.insert("F001".to_string());
        assert!(feature.dependencies_satisfied(&completed));
    }

    #[test]
    fn test_mega_status() {
        let mut status = MegaStatus::new("test-plan");
        assert_eq!(status.status, MegaExecutionStatus::Pending);

        status.start();
        assert_eq!(status.status, MegaExecutionStatus::Running);

        status.complete();
        assert_eq!(status.status, MegaExecutionStatus::Completed);
    }
}
