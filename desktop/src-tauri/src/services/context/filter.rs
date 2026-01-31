//! Context Filter Service
//!
//! Filters context based on agent type and execution phase.
//! Integrates with DesignDocLoader for architectural context.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use thiserror::Error;

use crate::models::design_doc::{Component, Decision, DesignDoc, Pattern};
use crate::models::prd::Story;
use crate::services::design::DesignDocLoader;
use crate::services::phase::{Phase, PhaseManager};

/// Context tags for filtering findings.md entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ContextTag {
    /// Architecture-related findings
    Arch,
    /// API-related findings
    Api,
    /// Database-related findings
    Db,
    /// UI-related findings
    Ui,
    /// Security-related findings
    Security,
    /// Performance-related findings
    Perf,
    /// Testing-related findings
    Test,
    /// Infrastructure-related findings
    Infra,
    /// Business logic findings
    Logic,
    /// Configuration-related findings
    Config,
}

impl std::fmt::Display for ContextTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextTag::Arch => write!(f, "[ARCH]"),
            ContextTag::Api => write!(f, "[API]"),
            ContextTag::Db => write!(f, "[DB]"),
            ContextTag::Ui => write!(f, "[UI]"),
            ContextTag::Security => write!(f, "[SECURITY]"),
            ContextTag::Perf => write!(f, "[PERF]"),
            ContextTag::Test => write!(f, "[TEST]"),
            ContextTag::Infra => write!(f, "[INFRA]"),
            ContextTag::Logic => write!(f, "[LOGIC]"),
            ContextTag::Config => write!(f, "[CONFIG]"),
        }
    }
}

impl ContextTag {
    /// Parse a tag from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().trim_matches(|c| c == '[' || c == ']') {
            "ARCH" => Some(ContextTag::Arch),
            "API" => Some(ContextTag::Api),
            "DB" | "DATABASE" => Some(ContextTag::Db),
            "UI" => Some(ContextTag::Ui),
            "SECURITY" | "SEC" => Some(ContextTag::Security),
            "PERF" | "PERFORMANCE" => Some(ContextTag::Perf),
            "TEST" | "TESTING" => Some(ContextTag::Test),
            "INFRA" | "INFRASTRUCTURE" => Some(ContextTag::Infra),
            "LOGIC" | "BIZ" | "BUSINESS" => Some(ContextTag::Logic),
            "CONFIG" | "CONFIGURATION" => Some(ContextTag::Config),
            _ => None,
        }
    }

    /// Get all tags as a list
    pub fn all() -> Vec<Self> {
        vec![
            ContextTag::Arch,
            ContextTag::Api,
            ContextTag::Db,
            ContextTag::Ui,
            ContextTag::Security,
            ContextTag::Perf,
            ContextTag::Test,
            ContextTag::Infra,
            ContextTag::Logic,
            ContextTag::Config,
        ]
    }
}

/// Errors that can occur in context filtering
#[derive(Debug, Error)]
pub enum ContextError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Design document error: {0}")]
    DesignDoc(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

/// Configuration for context filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFilterConfig {
    /// Maximum number of components to include
    #[serde(default = "default_max_components")]
    pub max_components: usize,
    /// Maximum number of patterns to include
    #[serde(default = "default_max_patterns")]
    pub max_patterns: usize,
    /// Maximum number of decisions to include
    #[serde(default = "default_max_decisions")]
    pub max_decisions: usize,
    /// Include findings.md content
    #[serde(default = "default_include_findings")]
    pub include_findings: bool,
    /// Agent-specific tag filters (agent_name -> relevant tags)
    #[serde(default)]
    pub agent_tag_filters: HashMap<String, Vec<String>>,
}

fn default_max_components() -> usize {
    10
}

fn default_max_patterns() -> usize {
    5
}

fn default_max_decisions() -> usize {
    5
}

fn default_include_findings() -> bool {
    true
}

impl Default for ContextFilterConfig {
    fn default() -> Self {
        Self {
            max_components: default_max_components(),
            max_patterns: default_max_patterns(),
            max_decisions: default_max_decisions(),
            include_findings: default_include_findings(),
            agent_tag_filters: HashMap::new(),
        }
    }
}

/// Context for executing a story
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoryContext {
    /// Story being executed
    pub story_id: String,
    /// Story title
    pub story_title: String,
    /// Story description
    pub story_description: String,
    /// Current execution phase
    pub phase: String,
    /// Agent being used
    pub agent: String,
    /// Relevant components from design doc
    pub components: Vec<Component>,
    /// Relevant patterns from design doc
    pub patterns: Vec<Pattern>,
    /// Relevant architectural decisions
    pub decisions: Vec<Decision>,
    /// Filtered findings content
    pub findings: String,
    /// Additional context (from feature mapping, etc.)
    pub additional_context: HashMap<String, String>,
}

impl StoryContext {
    /// Create a new StoryContext
    pub fn new(story_id: impl Into<String>, story_title: impl Into<String>) -> Self {
        Self {
            story_id: story_id.into(),
            story_title: story_title.into(),
            ..Default::default()
        }
    }

    /// Build a prompt-ready context string
    pub fn to_prompt_context(&self) -> String {
        let mut context = String::new();

        // Story info
        context.push_str(&format!("## Story: {} - {}\n\n", self.story_id, self.story_title));
        if !self.story_description.is_empty() {
            context.push_str(&format!("{}\n\n", self.story_description));
        }

        // Phase and agent
        context.push_str(&format!("**Phase:** {}\n", self.phase));
        context.push_str(&format!("**Agent:** {}\n\n", self.agent));

        // Components
        if !self.components.is_empty() {
            context.push_str("## Relevant Components\n\n");
            for component in &self.components {
                context.push_str(&format!("### {}\n", component.name));
                context.push_str(&format!("{}\n", component.description));
                if !component.responsibilities.is_empty() {
                    context.push_str("**Responsibilities:**\n");
                    for resp in &component.responsibilities {
                        context.push_str(&format!("- {}\n", resp));
                    }
                }
                context.push('\n');
            }
        }

        // Patterns
        if !self.patterns.is_empty() {
            context.push_str("## Relevant Patterns\n\n");
            for pattern in &self.patterns {
                context.push_str(&format!("### {}\n", pattern.name));
                context.push_str(&format!("{}\n", pattern.description));
                if !pattern.rationale.is_empty() {
                    context.push_str(&format!("**Rationale:** {}\n", pattern.rationale));
                }
                context.push('\n');
            }
        }

        // Decisions
        if !self.decisions.is_empty() {
            context.push_str("## Architectural Decisions\n\n");
            for decision in &self.decisions {
                context.push_str(&format!("### {} - {}\n", decision.id, decision.title));
                context.push_str(&format!("**Context:** {}\n", decision.context));
                context.push_str(&format!("**Decision:** {}\n", decision.decision));
                if !decision.rationale.is_empty() {
                    context.push_str(&format!("**Rationale:** {}\n", decision.rationale));
                }
                context.push('\n');
            }
        }

        // Findings
        if !self.findings.is_empty() {
            context.push_str("## Relevant Findings\n\n");
            context.push_str(&self.findings);
            context.push_str("\n\n");
        }

        // Additional context
        if !self.additional_context.is_empty() {
            context.push_str("## Additional Context\n\n");
            for (key, value) in &self.additional_context {
                context.push_str(&format!("**{}:** {}\n", key, value));
            }
        }

        context
    }
}

/// Context Filter Service
///
/// Filters and provides context for agent execution based on phase and agent type.
pub struct ContextFilter {
    /// Filter configuration
    config: ContextFilterConfig,
    /// Phase manager for phase-specific filtering
    phase_manager: PhaseManager,
    /// Design document loader
    design_loader: DesignDocLoader,
    /// Agent-specific tag relevance
    agent_tags: HashMap<String, HashSet<ContextTag>>,
}

impl Default for ContextFilter {
    fn default() -> Self {
        Self::new(ContextFilterConfig::default())
    }
}

impl ContextFilter {
    /// Create a new ContextFilter with the given configuration
    pub fn new(config: ContextFilterConfig) -> Self {
        let mut agent_tags = HashMap::new();

        // Default tag mappings for known agents
        // claude-code: general purpose, all tags relevant
        agent_tags.insert(
            "claude-code".to_string(),
            ContextTag::all().into_iter().collect(),
        );

        // codex: planning focused
        agent_tags.insert(
            "codex".to_string(),
            [ContextTag::Arch, ContextTag::Api, ContextTag::Logic, ContextTag::Infra]
                .into_iter()
                .collect(),
        );

        // aider: refactoring focused
        agent_tags.insert(
            "aider".to_string(),
            [ContextTag::Arch, ContextTag::Logic, ContextTag::Perf, ContextTag::Config]
                .into_iter()
                .collect(),
        );

        Self {
            config,
            phase_manager: PhaseManager::new(),
            design_loader: DesignDocLoader::new(),
            agent_tags,
        }
    }

    /// Create with a custom phase manager
    pub fn with_phase_manager(mut self, phase_manager: PhaseManager) -> Self {
        self.phase_manager = phase_manager;
        self
    }

    /// Create with a custom design loader
    pub fn with_design_loader(mut self, design_loader: DesignDocLoader) -> Self {
        self.design_loader = design_loader;
        self
    }

    /// Load project context from a directory
    pub async fn load_project(&self, project_root: &Path) -> Result<(), ContextError> {
        self.design_loader
            .load_project_doc(project_root)
            .await
            .map_err(|e| ContextError::DesignDoc(e.to_string()))
    }

    /// Load feature context from a worktree
    pub async fn load_feature(&self, worktree_path: &Path) -> Result<(), ContextError> {
        self.design_loader
            .load_feature_doc(worktree_path)
            .await
            .map_err(|e| ContextError::DesignDoc(e.to_string()))
    }

    /// Filter context for a specific agent
    ///
    /// Returns tags that are relevant to this agent
    pub fn filter_for_agent(&self, agent: &str) -> HashSet<ContextTag> {
        self.agent_tags
            .get(agent)
            .cloned()
            .unwrap_or_else(|| ContextTag::all().into_iter().collect())
    }

    /// Filter context for a specific phase
    ///
    /// Returns tags that are relevant to this phase
    pub fn filter_for_phase(&self, phase: Phase) -> HashSet<ContextTag> {
        match phase {
            Phase::Planning => {
                [ContextTag::Arch, ContextTag::Api, ContextTag::Logic, ContextTag::Infra]
                    .into_iter()
                    .collect()
            }
            Phase::Implementation => ContextTag::all().into_iter().collect(),
            Phase::Retry => ContextTag::all().into_iter().collect(),
            Phase::Refactor => {
                [ContextTag::Arch, ContextTag::Logic, ContextTag::Perf, ContextTag::Config]
                    .into_iter()
                    .collect()
            }
            Phase::Review => {
                [ContextTag::Security, ContextTag::Perf, ContextTag::Test, ContextTag::Api]
                    .into_iter()
                    .collect()
            }
        }
    }

    /// Inject design context into a story context
    ///
    /// Fetches relevant components, patterns, and decisions from the design document
    pub async fn inject_design_context(
        &self,
        context: &mut StoryContext,
        feature_id: Option<&str>,
    ) -> Result<(), ContextError> {
        // Get components for the feature
        if let Some(feature_id) = feature_id {
            let components = self
                .design_loader
                .get_components_for_feature(feature_id)
                .await;
            context.components = components
                .into_iter()
                .take(self.config.max_components)
                .collect();

            let patterns = self
                .design_loader
                .get_patterns_for_feature(feature_id)
                .await;
            context.patterns = patterns
                .into_iter()
                .take(self.config.max_patterns)
                .collect();

            let decisions = self
                .design_loader
                .get_decisions_for_feature(feature_id)
                .await;
            context.decisions = decisions
                .into_iter()
                .take(self.config.max_decisions)
                .collect();
        } else {
            // No feature, get all from project
            let components = self.design_loader.list_all_components().await;
            context.components = components
                .into_iter()
                .take(self.config.max_components)
                .collect();

            let patterns = self.design_loader.list_all_patterns().await;
            context.patterns = patterns
                .into_iter()
                .take(self.config.max_patterns)
                .collect();

            let decisions = self.design_loader.list_all_decisions().await;
            context.decisions = decisions
                .into_iter()
                .take(self.config.max_decisions)
                .collect();
        }

        Ok(())
    }

    /// Filter findings.md content by tags
    ///
    /// Extracts entries that match the given tags
    pub fn filter_findings(&self, findings_content: &str, tags: &HashSet<ContextTag>) -> String {
        let mut filtered_lines = Vec::new();
        let mut current_section: Option<ContextTag> = None;
        let mut section_lines = Vec::new();

        for line in findings_content.lines() {
            // Check for tag at start of line
            let tag = ContextTag::all().into_iter().find(|t| {
                line.to_uppercase().starts_with(&t.to_string())
            });

            if let Some(new_tag) = tag {
                // Save previous section if relevant
                if let Some(prev_tag) = current_section {
                    if tags.contains(&prev_tag) && !section_lines.is_empty() {
                        filtered_lines.push(format!("{}", prev_tag));
                        filtered_lines.extend(section_lines.drain(..).map(|s: &str| s.to_string()));
                        filtered_lines.push(String::new());
                    }
                }
                section_lines.clear();

                current_section = Some(new_tag);
                section_lines.push(line);
            } else if current_section.is_some() {
                section_lines.push(line);
            } else if tags.is_empty() || line.trim().is_empty() {
                // Include untagged content if no specific tags requested
                filtered_lines.push(line.to_string());
            }
        }

        // Handle last section
        if let Some(prev_tag) = current_section {
            if tags.contains(&prev_tag) && !section_lines.is_empty() {
                filtered_lines.push(format!("{}", prev_tag));
                filtered_lines.extend(section_lines.into_iter().map(|s| s.to_string()));
            }
        }

        filtered_lines.join("\n")
    }

    /// Get complete story context for execution
    ///
    /// Combines:
    /// - Story information
    /// - Phase and agent context
    /// - Design document context (filtered by agent/phase)
    /// - Findings (filtered by tags)
    pub async fn get_story_context(
        &self,
        story: &Story,
        phase: Phase,
        feature_id: Option<&str>,
        findings_content: Option<&str>,
    ) -> Result<StoryContext, ContextError> {
        let agent = self.phase_manager.get_agent_for_story(
            phase,
            story.story_type,
            story.agent.as_deref(),
        );

        let mut context = StoryContext::new(&story.id, &story.title);
        context.story_description = story.description.clone();
        context.phase = phase.to_string();
        context.agent = agent.clone();

        // Inject design context
        self.inject_design_context(&mut context, feature_id).await?;

        // Filter findings by agent and phase
        if self.config.include_findings {
            if let Some(findings) = findings_content {
                let agent_tags = self.filter_for_agent(&agent);
                let phase_tags = self.filter_for_phase(phase);
                let combined_tags: HashSet<_> = agent_tags.intersection(&phase_tags).copied().collect();
                context.findings = self.filter_findings(findings, &combined_tags);
            }
        }

        // Add feature mapping description if available
        if let Some(feature_id) = feature_id {
            if let Some(mapping) = self.design_loader.get_feature_mapping(feature_id).await {
                context
                    .additional_context
                    .insert("feature_mapping".to_string(), mapping.description.clone());
            }
        }

        Ok(context)
    }

    /// Get the phase manager
    pub fn phase_manager(&self) -> &PhaseManager {
        &self.phase_manager
    }

    /// Get the design loader
    pub fn design_loader(&self) -> &DesignDocLoader {
        &self.design_loader
    }

    /// Register custom agent tag filters
    pub fn register_agent_tags(&mut self, agent: impl Into<String>, tags: Vec<ContextTag>) {
        self.agent_tags.insert(agent.into(), tags.into_iter().collect());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::prd::StoryType;

    #[test]
    fn test_context_tag_display() {
        assert_eq!(ContextTag::Arch.to_string(), "[ARCH]");
        assert_eq!(ContextTag::Api.to_string(), "[API]");
        assert_eq!(ContextTag::Db.to_string(), "[DB]");
    }

    #[test]
    fn test_context_tag_from_str() {
        assert_eq!(ContextTag::from_str("[ARCH]"), Some(ContextTag::Arch));
        assert_eq!(ContextTag::from_str("API"), Some(ContextTag::Api));
        assert_eq!(ContextTag::from_str("database"), Some(ContextTag::Db));
        assert_eq!(ContextTag::from_str("unknown"), None);
    }

    #[test]
    fn test_filter_for_agent() {
        let filter = ContextFilter::default();

        // claude-code should have all tags
        let claude_tags = filter.filter_for_agent("claude-code");
        assert!(claude_tags.contains(&ContextTag::Arch));
        assert!(claude_tags.contains(&ContextTag::Api));
        assert!(claude_tags.contains(&ContextTag::Security));

        // codex should have planning-focused tags
        let codex_tags = filter.filter_for_agent("codex");
        assert!(codex_tags.contains(&ContextTag::Arch));
        assert!(!codex_tags.contains(&ContextTag::Ui));

        // Unknown agent gets all tags
        let unknown_tags = filter.filter_for_agent("unknown-agent");
        assert_eq!(unknown_tags.len(), ContextTag::all().len());
    }

    #[test]
    fn test_filter_for_phase() {
        let filter = ContextFilter::default();

        // Planning phase is architecture focused
        let planning_tags = filter.filter_for_phase(Phase::Planning);
        assert!(planning_tags.contains(&ContextTag::Arch));
        assert!(!planning_tags.contains(&ContextTag::Ui));

        // Implementation phase has all tags
        let impl_tags = filter.filter_for_phase(Phase::Implementation);
        assert_eq!(impl_tags.len(), ContextTag::all().len());

        // Review phase is quality focused
        let review_tags = filter.filter_for_phase(Phase::Review);
        assert!(review_tags.contains(&ContextTag::Security));
        assert!(review_tags.contains(&ContextTag::Test));
    }

    #[test]
    fn test_filter_findings() {
        let filter = ContextFilter::default();

        let findings = r#"
[ARCH] This is architecture related
- Point 1
- Point 2

[API] This is API related
- Endpoint info

[DB] Database stuff
- Schema details
"#;

        let arch_only: HashSet<_> = [ContextTag::Arch].into_iter().collect();
        let filtered = filter.filter_findings(findings, &arch_only);
        assert!(filtered.contains("[ARCH]"));
        assert!(filtered.contains("Point 1"));
        assert!(!filtered.contains("[API]"));
        assert!(!filtered.contains("[DB]"));
    }

    #[test]
    fn test_story_context_new() {
        let context = StoryContext::new("S001", "Test Story");
        assert_eq!(context.story_id, "S001");
        assert_eq!(context.story_title, "Test Story");
    }

    #[test]
    fn test_story_context_to_prompt() {
        let mut context = StoryContext::new("S001", "Implement Feature");
        context.story_description = "This story implements a new feature.".to_string();
        context.phase = "implementation".to_string();
        context.agent = "claude-code".to_string();
        context.components.push(Component {
            name: "TestComponent".to_string(),
            description: "A test component".to_string(),
            responsibilities: vec!["Do things".to_string()],
            dependencies: vec![],
            features: vec![],
        });

        let prompt = context.to_prompt_context();
        assert!(prompt.contains("## Story: S001 - Implement Feature"));
        assert!(prompt.contains("**Phase:** implementation"));
        assert!(prompt.contains("### TestComponent"));
    }

    #[test]
    fn test_register_agent_tags() {
        let mut filter = ContextFilter::default();

        filter.register_agent_tags("custom-agent", vec![ContextTag::Db, ContextTag::Api]);

        let tags = filter.filter_for_agent("custom-agent");
        assert!(tags.contains(&ContextTag::Db));
        assert!(tags.contains(&ContextTag::Api));
        assert!(!tags.contains(&ContextTag::Ui));
    }

    #[tokio::test]
    async fn test_get_story_context() {
        let filter = ContextFilter::default();

        let story = Story {
            id: "S001".to_string(),
            title: "Test Story".to_string(),
            description: "Test description".to_string(),
            priority: crate::models::prd::Priority::High,
            dependencies: vec![],
            acceptance_criteria: vec![],
            status: crate::models::prd::StoryStatus::Pending,
            complexity: None,
            tags: vec![],
            metadata: std::collections::HashMap::new(),
            agent: None,
            story_type: Some(StoryType::Feature),
        };

        let context = filter
            .get_story_context(&story, Phase::Implementation, None, None)
            .await
            .unwrap();

        assert_eq!(context.story_id, "S001");
        assert_eq!(context.phase, "implementation");
        assert_eq!(context.agent, "claude-code");
    }

    #[test]
    fn test_context_filter_config_defaults() {
        let config = ContextFilterConfig::default();
        assert_eq!(config.max_components, 10);
        assert_eq!(config.max_patterns, 5);
        assert_eq!(config.max_decisions, 5);
        assert!(config.include_findings);
    }
}
