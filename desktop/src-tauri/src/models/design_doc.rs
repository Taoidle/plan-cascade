//! Design Document Models
//!
//! Data structures for representing two-level design documents (Project and Feature level).
//! Supports loading, parsing, and querying of architectural design information including
//! components, patterns, decisions (ADRs), and feature mappings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Story S001: Core Design Document Models
// ============================================================================

/// Level of a design document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DesignDocLevel {
    /// Project-level design document (root design_doc.json)
    Project,
    /// Feature-level design document (in worktree directory)
    Feature,
}

impl Default for DesignDocLevel {
    fn default() -> Self {
        DesignDocLevel::Project
    }
}

impl std::fmt::Display for DesignDocLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DesignDocLevel::Project => write!(f, "project"),
            DesignDocLevel::Feature => write!(f, "feature"),
        }
    }
}

/// Metadata for a design document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignDocMetadata {
    /// Creation timestamp
    #[serde(default)]
    pub created_at: Option<String>,
    /// Document version
    #[serde(default)]
    pub version: String,
    /// Source of the document (e.g., "manual", "imported", "generated")
    #[serde(default)]
    pub source: Option<String>,
    /// Document level (project or feature)
    #[serde(default)]
    pub level: DesignDocLevel,
    /// Reference to mega plan (for feature-level docs)
    #[serde(default)]
    pub mega_plan_reference: Option<String>,
}

impl Default for DesignDocMetadata {
    fn default() -> Self {
        Self {
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            version: "1.0.0".to_string(),
            source: None,
            level: DesignDocLevel::Project,
            mega_plan_reference: None,
        }
    }
}

/// Overview section of a design document
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Overview {
    /// Document/project title
    #[serde(default)]
    pub title: String,
    /// Summary description
    #[serde(default)]
    pub summary: String,
    /// List of goals
    #[serde(default)]
    pub goals: Vec<String>,
    /// List of non-goals (explicitly out of scope)
    #[serde(default)]
    pub non_goals: Vec<String>,
}

/// Component definition in the architecture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    /// Component name (unique identifier)
    pub name: String,
    /// Description of the component
    #[serde(default)]
    pub description: String,
    /// List of responsibilities
    #[serde(default)]
    pub responsibilities: Vec<String>,
    /// Dependencies on other components
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Features this component is associated with
    #[serde(default)]
    pub features: Vec<String>,
}

impl Component {
    /// Create a new component with a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            responsibilities: Vec::new(),
            dependencies: Vec::new(),
            features: Vec::new(),
        }
    }
}

/// Design pattern used in the architecture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// Pattern name
    pub name: String,
    /// Description of the pattern
    #[serde(default)]
    pub description: String,
    /// Rationale for using this pattern
    #[serde(default)]
    pub rationale: String,
    /// Components/features this pattern applies to
    #[serde(default)]
    pub applies_to: Vec<String>,
}

impl Pattern {
    /// Create a new pattern with a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            rationale: String::new(),
            applies_to: Vec::new(),
        }
    }
}

/// Status of an architecture decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatus {
    #[default]
    Proposed,
    Accepted,
    Deprecated,
    Superseded,
}

impl std::fmt::Display for DecisionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionStatus::Proposed => write!(f, "proposed"),
            DecisionStatus::Accepted => write!(f, "accepted"),
            DecisionStatus::Deprecated => write!(f, "deprecated"),
            DecisionStatus::Superseded => write!(f, "superseded"),
        }
    }
}

/// Architecture Decision Record (ADR)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// Unique decision ID (e.g., "ADR-001")
    pub id: String,
    /// Decision title
    pub title: String,
    /// Context that led to this decision
    #[serde(default)]
    pub context: String,
    /// The actual decision made
    #[serde(default)]
    pub decision: String,
    /// Rationale behind the decision
    #[serde(default)]
    pub rationale: String,
    /// Alternatives that were considered
    #[serde(default)]
    pub alternatives_considered: Vec<String>,
    /// Current status of the decision
    #[serde(default)]
    pub status: DecisionStatus,
    /// Components/features this decision applies to
    #[serde(default)]
    pub applies_to: Vec<String>,
}

impl Decision {
    /// Create a new decision with ID and title
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            context: String::new(),
            decision: String::new(),
            rationale: String::new(),
            alternatives_considered: Vec::new(),
            status: DecisionStatus::default(),
            applies_to: Vec::new(),
        }
    }
}

// ============================================================================
// Story S002: Architecture and Interfaces Sub-structures
// ============================================================================

/// Infrastructure information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Infrastructure {
    /// List of existing services
    #[serde(default)]
    pub existing_services: Vec<String>,
    /// List of new services to be added
    #[serde(default)]
    pub new_services: Vec<String>,
}

/// Architecture section of the design document
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Architecture {
    /// High-level system overview
    #[serde(default)]
    pub system_overview: String,
    /// List of components
    #[serde(default)]
    pub components: Vec<Component>,
    /// Data flow description
    #[serde(default)]
    pub data_flow: String,
    /// Design patterns used
    #[serde(default)]
    pub patterns: Vec<Pattern>,
    /// Infrastructure information
    #[serde(default)]
    pub infrastructure: Infrastructure,
}

/// API standards for interfaces
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiStandards {
    /// API style (e.g., "REST", "GraphQL", "Rust trait-based")
    #[serde(default)]
    pub style: String,
    /// Error handling approach
    #[serde(default)]
    pub error_handling: String,
    /// Async pattern used
    #[serde(default)]
    pub async_pattern: String,
}

/// Shared data model definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedDataModel {
    /// Model name
    pub name: String,
    /// File location
    #[serde(default)]
    pub location: String,
    /// Description of the model
    #[serde(default)]
    pub description: Option<String>,
    /// Changes or modifications
    #[serde(default)]
    pub changes: Option<String>,
}

impl SharedDataModel {
    /// Create a new shared data model
    pub fn new(name: impl Into<String>, location: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            location: location.into(),
            description: None,
            changes: None,
        }
    }
}

/// Interfaces section of the design document
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Interfaces {
    /// API standards
    #[serde(default)]
    pub api_standards: ApiStandards,
    /// Shared data models
    #[serde(default)]
    pub shared_data_models: Vec<SharedDataModel>,
}

// ============================================================================
// Story S003: FeatureMapping and DesignDocError
// ============================================================================

/// Mapping between a feature and design elements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureMapping {
    /// Associated component names
    #[serde(default)]
    pub components: Vec<String>,
    /// Design patterns used
    #[serde(default)]
    pub patterns: Vec<String>,
    /// Related ADR decision IDs
    #[serde(default)]
    pub decisions: Vec<String>,
    /// Feature description
    #[serde(default)]
    pub description: String,
}

impl FeatureMapping {
    /// Create a new feature mapping with description
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            components: Vec::new(),
            patterns: Vec::new(),
            decisions: Vec::new(),
            description: description.into(),
        }
    }

    /// Add a component to this mapping
    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.components.push(component.into());
        self
    }

    /// Add a pattern to this mapping
    pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.patterns.push(pattern.into());
        self
    }

    /// Add a decision to this mapping
    pub fn with_decision(mut self, decision: impl Into<String>) -> Self {
        self.decisions.push(decision.into());
        self
    }
}

/// Errors that can occur when working with design documents
#[derive(Debug, thiserror::Error)]
pub enum DesignDocError {
    /// IO error during file operations
    #[error("IO error: {0}")]
    IoError(String),
    /// JSON parsing error
    #[error("Parse error: {0}")]
    ParseError(String),
    /// Design document not found
    #[error("Design document not found: {0}")]
    NotFound(String),
    /// Invalid document level
    #[error("Invalid document level: {0}")]
    InvalidLevel(String),
    /// Validation error
    #[error("Validation error: {0}")]
    ValidationError(String),
}

impl From<std::io::Error> for DesignDocError {
    fn from(err: std::io::Error) -> Self {
        DesignDocError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for DesignDocError {
    fn from(err: serde_json::Error) -> Self {
        DesignDocError::ParseError(err.to_string())
    }
}

// ============================================================================
// Top-level DesignDoc structure
// ============================================================================

/// Complete design document structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DesignDoc {
    /// Document metadata
    #[serde(default)]
    pub metadata: DesignDocMetadata,
    /// Overview section
    #[serde(default)]
    pub overview: Overview,
    /// Architecture section
    #[serde(default)]
    pub architecture: Architecture,
    /// Interfaces section
    #[serde(default)]
    pub interfaces: Interfaces,
    /// Architecture decisions (ADRs)
    #[serde(default)]
    pub decisions: Vec<Decision>,
    /// Feature mappings (feature_id -> mapping)
    #[serde(default)]
    pub feature_mappings: HashMap<String, FeatureMapping>,
}

impl DesignDoc {
    /// Create a new empty design document
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new design document with title
    pub fn with_title(title: impl Into<String>) -> Self {
        let mut doc = Self::new();
        doc.overview.title = title.into();
        doc
    }

    /// Load a design document from a JSON file
    pub fn from_file(path: &Path) -> Result<Self, DesignDocError> {
        if !path.exists() {
            return Err(DesignDocError::NotFound(path.display().to_string()));
        }

        let content = std::fs::read_to_string(path)?;
        let doc: DesignDoc = serde_json::from_str(&content)?;
        Ok(doc)
    }

    /// Save the design document to a JSON file
    pub fn to_file(&self, path: &Path) -> Result<(), DesignDocError> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get the level of this design document
    pub fn level(&self) -> DesignDocLevel {
        self.metadata.level
    }

    /// Set the level of this design document
    pub fn set_level(&mut self, level: DesignDocLevel) {
        self.metadata.level = level;
    }

    /// Get a component by name
    pub fn get_component(&self, name: &str) -> Option<&Component> {
        self.architecture.components.iter().find(|c| c.name == name)
    }

    /// Get a pattern by name
    pub fn get_pattern(&self, name: &str) -> Option<&Pattern> {
        self.architecture.patterns.iter().find(|p| p.name == name)
    }

    /// Get a decision by ID
    pub fn get_decision(&self, id: &str) -> Option<&Decision> {
        self.decisions.iter().find(|d| d.id == id)
    }

    /// Get a feature mapping by feature ID
    pub fn get_feature_mapping(&self, feature_id: &str) -> Option<&FeatureMapping> {
        self.feature_mappings.get(feature_id)
    }

    /// Add a component to the architecture
    pub fn add_component(&mut self, component: Component) {
        self.architecture.components.push(component);
    }

    /// Add a pattern to the architecture
    pub fn add_pattern(&mut self, pattern: Pattern) {
        self.architecture.patterns.push(pattern);
    }

    /// Add a decision
    pub fn add_decision(&mut self, decision: Decision) {
        self.decisions.push(decision);
    }

    /// Add a feature mapping
    pub fn add_feature_mapping(&mut self, feature_id: impl Into<String>, mapping: FeatureMapping) {
        self.feature_mappings.insert(feature_id.into(), mapping);
    }
}

// ============================================================================
// Story S006: Model Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Test S006-1: test_design_doc_serialization
    #[test]
    fn test_design_doc_serialization() {
        // Create a design document with all fields
        let mut doc = DesignDoc::with_title("Test Project");
        doc.metadata.version = "2.0.0".to_string();
        doc.metadata.level = DesignDocLevel::Project;
        doc.overview.summary = "A test project".to_string();
        doc.overview.goals = vec!["Goal 1".to_string(), "Goal 2".to_string()];
        doc.overview.non_goals = vec!["Non-goal 1".to_string()];

        // Add a component
        let mut component = Component::new("TestComponent");
        component.description = "A test component".to_string();
        component.responsibilities = vec!["Do stuff".to_string()];
        component.dependencies = vec!["OtherComponent".to_string()];
        component.features = vec!["feature-001".to_string()];
        doc.add_component(component);

        // Add a pattern
        let mut pattern = Pattern::new("Repository");
        pattern.description = "Repository pattern for data access".to_string();
        pattern.rationale = "Separation of concerns".to_string();
        pattern.applies_to = vec!["TestComponent".to_string()];
        doc.add_pattern(pattern);

        // Add a decision
        let mut decision = Decision::new("ADR-001", "Use Rust");
        decision.context = "Need a fast and safe language".to_string();
        decision.decision = "Use Rust for backend".to_string();
        decision.rationale = "Memory safety without GC".to_string();
        decision.alternatives_considered = vec!["Go".to_string(), "C++".to_string()];
        decision.status = DecisionStatus::Accepted;
        decision.applies_to = vec!["TestComponent".to_string()];
        doc.add_decision(decision);

        // Add a feature mapping
        let mapping = FeatureMapping::new("Test feature implementation")
            .with_component("TestComponent")
            .with_pattern("Repository")
            .with_decision("ADR-001");
        doc.add_feature_mapping("feature-001", mapping);

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&doc).expect("Failed to serialize");
        assert!(json.contains("Test Project"));
        assert!(json.contains("TestComponent"));
        assert!(json.contains("Repository"));
        assert!(json.contains("ADR-001"));
        assert!(json.contains("feature-001"));

        // Deserialize back
        let parsed: DesignDoc = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(parsed.overview.title, "Test Project");
        assert_eq!(parsed.architecture.components.len(), 1);
        assert_eq!(parsed.architecture.patterns.len(), 1);
        assert_eq!(parsed.decisions.len(), 1);
        assert_eq!(parsed.feature_mappings.len(), 1);
    }

    /// Test S006-2: test_design_doc_from_file
    #[test]
    fn test_design_doc_from_file() {
        // Create a temporary file with design doc content
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let content = r#"{
            "metadata": {
                "version": "1.0.0",
                "level": "project"
            },
            "overview": {
                "title": "File Test Project",
                "summary": "Testing file loading"
            },
            "architecture": {
                "components": [
                    {
                        "name": "FileComponent",
                        "description": "Component from file"
                    }
                ]
            }
        }"#;
        temp_file
            .write_all(content.as_bytes())
            .expect("Failed to write");

        // Load from file
        let doc = DesignDoc::from_file(temp_file.path()).expect("Failed to load");
        assert_eq!(doc.overview.title, "File Test Project");
        assert_eq!(doc.overview.summary, "Testing file loading");
        assert_eq!(doc.architecture.components.len(), 1);
        assert_eq!(doc.architecture.components[0].name, "FileComponent");
    }

    /// Test S006-3: test_design_doc_level
    #[test]
    fn test_design_doc_level() {
        // Test project level
        let mut doc = DesignDoc::new();
        assert_eq!(doc.level(), DesignDocLevel::Project);

        // Test feature level
        doc.set_level(DesignDocLevel::Feature);
        assert_eq!(doc.level(), DesignDocLevel::Feature);

        // Test serialization of level
        let json = serde_json::to_string(&doc.metadata).expect("Failed to serialize");
        assert!(json.contains("\"level\":\"feature\""));

        // Test deserialization
        let metadata: DesignDocMetadata =
            serde_json::from_str(r#"{"level": "feature"}"#).expect("Failed to parse");
        assert_eq!(metadata.level, DesignDocLevel::Feature);

        let metadata: DesignDocMetadata =
            serde_json::from_str(r#"{"level": "project"}"#).expect("Failed to parse");
        assert_eq!(metadata.level, DesignDocLevel::Project);
    }

    /// Test S006-4: test_feature_mapping_lookup
    #[test]
    fn test_feature_mapping_lookup() {
        let mut doc = DesignDoc::new();

        // Add components
        let mut comp1 = Component::new("Auth");
        comp1.features = vec!["feature-auth".to_string()];
        doc.add_component(comp1);

        let mut comp2 = Component::new("Database");
        comp2.features = vec!["feature-auth".to_string(), "feature-data".to_string()];
        doc.add_component(comp2);

        // Add patterns
        let mut pattern = Pattern::new("Repository");
        pattern.applies_to = vec!["feature-data".to_string()];
        doc.add_pattern(pattern);

        // Add decisions
        let mut decision = Decision::new("ADR-001", "Use JWT");
        decision.applies_to = vec!["feature-auth".to_string()];
        doc.add_decision(decision);

        // Add feature mappings
        let auth_mapping = FeatureMapping::new("Authentication feature")
            .with_component("Auth")
            .with_component("Database")
            .with_decision("ADR-001");
        doc.add_feature_mapping("feature-auth", auth_mapping);

        let data_mapping = FeatureMapping::new("Data feature")
            .with_component("Database")
            .with_pattern("Repository");
        doc.add_feature_mapping("feature-data", data_mapping);

        // Test lookups
        let auth = doc
            .get_feature_mapping("feature-auth")
            .expect("Should find auth");
        assert_eq!(auth.components.len(), 2);
        assert!(auth.components.contains(&"Auth".to_string()));
        assert!(auth.components.contains(&"Database".to_string()));
        assert_eq!(auth.decisions.len(), 1);
        assert!(auth.decisions.contains(&"ADR-001".to_string()));

        let data = doc
            .get_feature_mapping("feature-data")
            .expect("Should find data");
        assert_eq!(data.components.len(), 1);
        assert!(data.components.contains(&"Database".to_string()));
        assert_eq!(data.patterns.len(), 1);
        assert!(data.patterns.contains(&"Repository".to_string()));

        // Test non-existent mapping
        assert!(doc.get_feature_mapping("feature-none").is_none());

        // Test component lookup
        assert!(doc.get_component("Auth").is_some());
        assert!(doc.get_component("Database").is_some());
        assert!(doc.get_component("NonExistent").is_none());

        // Test pattern lookup
        assert!(doc.get_pattern("Repository").is_some());
        assert!(doc.get_pattern("NonExistent").is_none());

        // Test decision lookup
        assert!(doc.get_decision("ADR-001").is_some());
        assert!(doc.get_decision("ADR-999").is_none());
    }

    #[test]
    fn test_design_doc_to_file() {
        let mut doc = DesignDoc::with_title("Save Test");
        doc.add_component(Component::new("SavedComponent"));

        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        doc.to_file(temp_file.path()).expect("Failed to save");

        // Reload and verify
        let loaded = DesignDoc::from_file(temp_file.path()).expect("Failed to reload");
        assert_eq!(loaded.overview.title, "Save Test");
        assert_eq!(loaded.architecture.components[0].name, "SavedComponent");
    }

    #[test]
    fn test_design_doc_not_found() {
        let result = DesignDoc::from_file(Path::new("/nonexistent/path/design_doc.json"));
        assert!(matches!(result, Err(DesignDocError::NotFound(_))));
    }

    #[test]
    fn test_design_doc_parse_error() {
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(b"not valid json")
            .expect("Failed to write");

        let result = DesignDoc::from_file(temp_file.path());
        assert!(matches!(result, Err(DesignDocError::ParseError(_))));
    }

    #[test]
    fn test_default_values() {
        // Test that default values work correctly
        let json = "{}";
        let doc: DesignDoc = serde_json::from_str(json).expect("Failed to parse empty doc");

        assert!(doc.overview.title.is_empty());
        assert!(doc.overview.goals.is_empty());
        assert!(doc.architecture.components.is_empty());
        assert!(doc.decisions.is_empty());
        assert!(doc.feature_mappings.is_empty());
        assert_eq!(doc.level(), DesignDocLevel::Project);
    }

    #[test]
    fn test_decision_status() {
        assert_eq!(DecisionStatus::default(), DecisionStatus::Proposed);

        let statuses = [
            (DecisionStatus::Proposed, "proposed"),
            (DecisionStatus::Accepted, "accepted"),
            (DecisionStatus::Deprecated, "deprecated"),
            (DecisionStatus::Superseded, "superseded"),
        ];

        for (status, expected) in statuses {
            assert_eq!(status.to_string(), expected);
        }
    }
}
