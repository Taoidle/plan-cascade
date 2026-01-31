//! PRD (Product Requirements Document) Models
//!
//! Data structures for representing PRD documents with stories and dependencies.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Priority level for stories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Medium
    }
}

/// Type of story for categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StoryType {
    /// New feature implementation
    #[default]
    Feature,
    /// Bug fix
    Bugfix,
    /// Code refactoring
    Refactor,
    /// Test implementation
    Test,
}

impl std::fmt::Display for StoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoryType::Feature => write!(f, "feature"),
            StoryType::Bugfix => write!(f, "bugfix"),
            StoryType::Refactor => write!(f, "refactor"),
            StoryType::Test => write!(f, "test"),
        }
    }
}

/// Status of a story in the PRD
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StoryStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for StoryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoryStatus::Pending => write!(f, "pending"),
            StoryStatus::InProgress => write!(f, "in_progress"),
            StoryStatus::Completed => write!(f, "completed"),
            StoryStatus::Failed => write!(f, "failed"),
            StoryStatus::Skipped => write!(f, "skipped"),
        }
    }
}

/// Acceptance criteria for a story
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriteria {
    /// Unique ID for this criteria
    pub id: String,
    /// Description of the criteria
    pub description: String,
    /// Whether this criteria has been met
    #[serde(default)]
    pub met: bool,
}

/// A single story in the PRD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    /// Unique story identifier (e.g., "S001", "story-1")
    pub id: String,
    /// Story title
    pub title: String,
    /// Detailed description of the story
    #[serde(default)]
    pub description: String,
    /// Priority level
    #[serde(default)]
    pub priority: Priority,
    /// List of story IDs this story depends on
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Acceptance criteria
    #[serde(default)]
    pub acceptance_criteria: Vec<AcceptanceCriteria>,
    /// Current status
    #[serde(default)]
    pub status: StoryStatus,
    /// Estimated complexity (1-5)
    #[serde(default)]
    pub complexity: Option<u8>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Agent to execute this story (e.g., "claude-code", "codex", "aider")
    #[serde(default)]
    pub agent: Option<String>,
    /// Type of story for categorization and agent selection
    #[serde(default)]
    pub story_type: Option<StoryType>,
}

impl Story {
    /// Create a new story with required fields
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: String::new(),
            priority: Priority::default(),
            dependencies: Vec::new(),
            acceptance_criteria: Vec::new(),
            status: StoryStatus::default(),
            complexity: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
            agent: None,
            story_type: None,
        }
    }

    /// Check if this story has all dependencies satisfied
    pub fn dependencies_satisfied(&self, completed: &std::collections::HashSet<String>) -> bool {
        self.dependencies.iter().all(|dep| completed.contains(dep))
    }

    /// Check if this story is ready to execute
    pub fn is_ready(&self, completed: &std::collections::HashSet<String>) -> bool {
        self.status == StoryStatus::Pending && self.dependencies_satisfied(completed)
    }

    /// Get the agent assigned to this story
    pub fn get_agent(&self) -> Option<&str> {
        self.agent.as_deref()
    }

    /// Get the story type
    pub fn get_story_type(&self) -> Option<StoryType> {
        self.story_type
    }

    /// Set the agent for this story (builder pattern)
    pub fn with_agent(mut self, agent: impl Into<String>) -> Self {
        self.agent = Some(agent.into());
        self
    }

    /// Set the story type (builder pattern)
    pub fn with_story_type(mut self, story_type: StoryType) -> Self {
        self.story_type = Some(story_type);
        self
    }

    /// Check if this story is a bugfix
    pub fn is_bugfix(&self) -> bool {
        self.story_type == Some(StoryType::Bugfix)
    }

    /// Check if this story is a refactor
    pub fn is_refactor(&self) -> bool {
        self.story_type == Some(StoryType::Refactor)
    }
}

/// The complete PRD document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prd {
    /// PRD version
    #[serde(default = "default_version")]
    pub version: String,
    /// Task/project name
    pub name: String,
    /// Description of the overall task
    #[serde(default)]
    pub description: String,
    /// Target branch for merging
    #[serde(default = "default_target_branch")]
    pub target_branch: String,
    /// All stories in the PRD
    pub stories: Vec<Story>,
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

impl Prd {
    /// Create a new PRD with a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: default_version(),
            name: name.into(),
            description: String::new(),
            target_branch: default_target_branch(),
            stories: Vec::new(),
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            updated_at: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a story to the PRD
    pub fn add_story(&mut self, story: Story) {
        self.stories.push(story);
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Get a story by ID
    pub fn get_story(&self, id: &str) -> Option<&Story> {
        self.stories.iter().find(|s| s.id == id)
    }

    /// Get a mutable reference to a story by ID
    pub fn get_story_mut(&mut self, id: &str) -> Option<&mut Story> {
        self.stories.iter_mut().find(|s| s.id == id)
    }

    /// Get all story IDs
    pub fn story_ids(&self) -> Vec<String> {
        self.stories.iter().map(|s| s.id.clone()).collect()
    }

    /// Get completed story IDs
    pub fn completed_story_ids(&self) -> std::collections::HashSet<String> {
        self.stories
            .iter()
            .filter(|s| s.status == StoryStatus::Completed)
            .map(|s| s.id.clone())
            .collect()
    }

    /// Get pending stories that are ready to execute
    pub fn ready_stories(&self) -> Vec<&Story> {
        let completed = self.completed_story_ids();
        self.stories
            .iter()
            .filter(|s| s.is_ready(&completed))
            .collect()
    }

    /// Check if all stories are complete
    pub fn is_complete(&self) -> bool {
        self.stories.iter().all(|s| s.status == StoryStatus::Completed)
    }

    /// Get completion percentage
    pub fn completion_percentage(&self) -> f32 {
        if self.stories.is_empty() {
            return 100.0;
        }
        let completed = self.stories
            .iter()
            .filter(|s| s.status == StoryStatus::Completed)
            .count();
        (completed as f32 / self.stories.len() as f32) * 100.0
    }

    /// Load PRD from a JSON file
    pub fn from_file(path: &std::path::Path) -> Result<Self, PrdError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| PrdError::IoError(e.to_string()))?;
        serde_json::from_str(&content)
            .map_err(|e| PrdError::ParseError(e.to_string()))
    }

    /// Save PRD to a JSON file
    pub fn to_file(&self, path: &std::path::Path) -> Result<(), PrdError> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| PrdError::SerializeError(e.to_string()))?;
        std::fs::write(path, content)
            .map_err(|e| PrdError::IoError(e.to_string()))
    }
}

/// Errors that can occur when working with PRDs
#[derive(Debug, thiserror::Error)]
pub enum PrdError {
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
    fn test_story_creation() {
        let story = Story::new("S001", "Implement login");
        assert_eq!(story.id, "S001");
        assert_eq!(story.title, "Implement login");
        assert_eq!(story.status, StoryStatus::Pending);
    }

    #[test]
    fn test_prd_creation() {
        let mut prd = Prd::new("Test Project");
        prd.add_story(Story::new("S001", "Story 1"));
        prd.add_story(Story::new("S002", "Story 2"));

        assert_eq!(prd.stories.len(), 2);
        assert_eq!(prd.completion_percentage(), 0.0);
    }

    #[test]
    fn test_dependencies_satisfied() {
        let mut story = Story::new("S002", "Story 2");
        story.dependencies = vec!["S001".to_string()];

        let mut completed = std::collections::HashSet::new();
        assert!(!story.dependencies_satisfied(&completed));

        completed.insert("S001".to_string());
        assert!(story.dependencies_satisfied(&completed));
    }

    #[test]
    fn test_ready_stories() {
        let mut prd = Prd::new("Test");

        let mut s1 = Story::new("S001", "Story 1");
        s1.status = StoryStatus::Completed;
        prd.add_story(s1);

        let mut s2 = Story::new("S002", "Story 2");
        s2.dependencies = vec!["S001".to_string()];
        prd.add_story(s2);

        let mut s3 = Story::new("S003", "Story 3");
        s3.dependencies = vec!["S002".to_string()];
        prd.add_story(s3);

        let ready = prd.ready_stories();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "S002");
    }

    #[test]
    fn test_story_type_serialization() {
        // Test that StoryType serializes to lowercase strings
        assert_eq!(
            serde_json::to_string(&StoryType::Feature).unwrap(),
            "\"feature\""
        );
        assert_eq!(
            serde_json::to_string(&StoryType::Bugfix).unwrap(),
            "\"bugfix\""
        );
        assert_eq!(
            serde_json::to_string(&StoryType::Refactor).unwrap(),
            "\"refactor\""
        );
        assert_eq!(
            serde_json::to_string(&StoryType::Test).unwrap(),
            "\"test\""
        );
    }

    #[test]
    fn test_story_type_deserialization() {
        // Test that StoryType deserializes from lowercase strings
        assert_eq!(
            serde_json::from_str::<StoryType>("\"feature\"").unwrap(),
            StoryType::Feature
        );
        assert_eq!(
            serde_json::from_str::<StoryType>("\"bugfix\"").unwrap(),
            StoryType::Bugfix
        );
        assert_eq!(
            serde_json::from_str::<StoryType>("\"refactor\"").unwrap(),
            StoryType::Refactor
        );
        assert_eq!(
            serde_json::from_str::<StoryType>("\"test\"").unwrap(),
            StoryType::Test
        );
    }

    #[test]
    fn test_story_type_default() {
        // Test that StoryType defaults to Feature
        assert_eq!(StoryType::default(), StoryType::Feature);
    }

    #[test]
    fn test_story_type_display() {
        // Test Display implementation
        assert_eq!(format!("{}", StoryType::Feature), "feature");
        assert_eq!(format!("{}", StoryType::Bugfix), "bugfix");
        assert_eq!(format!("{}", StoryType::Refactor), "refactor");
        assert_eq!(format!("{}", StoryType::Test), "test");
    }

    #[test]
    fn test_story_backward_compatibility() {
        // Test that JSON without agent/story_type fields can be deserialized
        let json = r#"{
            "id": "S001",
            "title": "Test Story",
            "description": "A test",
            "priority": "high",
            "dependencies": [],
            "acceptance_criteria": [],
            "status": "pending",
            "complexity": 2,
            "tags": ["test"],
            "metadata": {}
        }"#;

        let story: Story = serde_json::from_str(json).unwrap();
        assert_eq!(story.id, "S001");
        assert_eq!(story.title, "Test Story");
        assert_eq!(story.agent, None);
        assert_eq!(story.story_type, None);
    }

    #[test]
    fn test_story_with_new_fields() {
        // Test JSON with agent and story_type fields
        let json = r#"{
            "id": "S002",
            "title": "Bugfix Story",
            "description": "Fix a bug",
            "priority": "critical",
            "dependencies": ["S001"],
            "acceptance_criteria": [],
            "status": "pending",
            "complexity": 1,
            "tags": ["bugfix"],
            "metadata": {},
            "agent": "codex",
            "story_type": "bugfix"
        }"#;

        let story: Story = serde_json::from_str(json).unwrap();
        assert_eq!(story.id, "S002");
        assert_eq!(story.agent, Some("codex".to_string()));
        assert_eq!(story.story_type, Some(StoryType::Bugfix));

        // Test serialization roundtrip
        let serialized = serde_json::to_string(&story).unwrap();
        let deserialized: Story = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.agent, Some("codex".to_string()));
        assert_eq!(deserialized.story_type, Some(StoryType::Bugfix));
    }

    #[test]
    fn test_story_helper_methods() {
        // Test get_agent and get_story_type
        let story = Story::new("S001", "Test");
        assert_eq!(story.get_agent(), None);
        assert_eq!(story.get_story_type(), None);

        // Test with_agent builder
        let story = Story::new("S002", "With Agent")
            .with_agent("claude-code");
        assert_eq!(story.get_agent(), Some("claude-code"));

        // Test with_story_type builder
        let story = Story::new("S003", "With Type")
            .with_story_type(StoryType::Refactor);
        assert_eq!(story.get_story_type(), Some(StoryType::Refactor));

        // Test chained builders
        let story = Story::new("S004", "Chained")
            .with_agent("aider")
            .with_story_type(StoryType::Bugfix);
        assert_eq!(story.get_agent(), Some("aider"));
        assert_eq!(story.get_story_type(), Some(StoryType::Bugfix));

        // Test is_bugfix and is_refactor
        let bugfix_story = Story::new("S005", "Bugfix")
            .with_story_type(StoryType::Bugfix);
        assert!(bugfix_story.is_bugfix());
        assert!(!bugfix_story.is_refactor());

        let refactor_story = Story::new("S006", "Refactor")
            .with_story_type(StoryType::Refactor);
        assert!(!refactor_story.is_bugfix());
        assert!(refactor_story.is_refactor());

        // Test feature story (neither bugfix nor refactor)
        let feature_story = Story::new("S007", "Feature")
            .with_story_type(StoryType::Feature);
        assert!(!feature_story.is_bugfix());
        assert!(!feature_story.is_refactor());

        // Test story with no type
        let no_type_story = Story::new("S008", "No Type");
        assert!(!no_type_story.is_bugfix());
        assert!(!no_type_story.is_refactor());
    }
}
