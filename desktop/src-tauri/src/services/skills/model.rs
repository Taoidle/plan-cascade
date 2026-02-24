//! Skill System Data Types
//!
//! Core types for the universal skill system supporting plan-cascade SKILL.md,
//! adk-rust .skills/, and convention file formats.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Source tier for a skill, determining its priority range
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
    /// Bundled with plan-cascade (priority 1-50)
    Builtin,
    /// Community skills from external-skills/ submodules (priority 51-100)
    External { source_name: String },
    /// User-defined skills (priority 101-200)
    User,
    /// Project-local .skills/ directory + convention files (priority 201+)
    ProjectLocal,
    /// Auto-generated from successful sessions (stored in DB)
    Generated,
}

/// Phase in which a skill should be injected
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjectionPhase {
    /// PRD/design generation
    Planning,
    /// Code execution
    Implementation,
    /// Error recovery
    Retry,
    /// All phases (for project-local skills)
    Always,
}

/// Hooks for pre/post tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHooks {
    pub pre_tool_use: Vec<ToolHookRule>,
    pub post_tool_use: Vec<ToolHookRule>,
    pub stop: Vec<HookAction>,
}

/// A rule matching tools and defining hook actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHookRule {
    /// Regex pattern matching tool names (e.g. "Write|Edit|Bash")
    pub matcher: String,
    /// Actions to execute when matched
    pub hooks: Vec<HookAction>,
}

/// A single hook action (shell command)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookAction {
    /// Type of hook action (currently only "command")
    pub hook_type: String,
    /// Shell command to execute
    pub command: String,
}

/// Detection rules from external-skills.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDetection {
    /// Files to check for existence in project root
    pub files: Vec<String>,
    /// Content patterns to search for in detected files
    pub patterns: Vec<String>,
}

/// A parsed and indexed skill document (universal format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDocument {
    /// Unique ID: normalized-name + first 12 chars of SHA-256
    pub id: String,
    /// Skill name
    pub name: String,
    /// When to use this skill
    pub description: String,
    /// Semantic version
    pub version: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Full skill text (markdown body)
    pub body: String,
    /// Source file path
    pub path: PathBuf,
    /// Full SHA-256 hex string of file content
    pub hash: String,
    /// Unix timestamp of last modification
    pub last_modified: Option<i64>,

    // --- Plan Cascade extensions ---
    /// Whether directly callable by user (default: false)
    pub user_invocable: bool,
    /// Restrict tool access (empty = all tools allowed)
    pub allowed_tools: Vec<String>,
    /// License identifier
    pub license: Option<String>,
    /// Arbitrary key-value metadata (author, etc.)
    pub metadata: HashMap<String, String>,
    /// Pre/Post tool hooks
    pub hooks: Option<SkillHooks>,

    // --- Source & priority ---
    /// Which source tier this skill belongs to
    pub source: SkillSource,
    /// Resolved priority (1-200+)
    pub priority: u32,

    // --- Detection (from config) ---
    /// Detection rules (from external-skills.json)
    pub detect: Option<SkillDetection>,
    /// Which phases to inject into
    pub inject_into: Vec<InjectionPhase>,

    // --- Runtime state ---
    /// Whether the user has enabled this skill in the UI
    pub enabled: bool,
}

/// Lightweight summary without body (for UI listings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub tags: Vec<String>,
    pub source: SkillSource,
    pub priority: u32,
    pub enabled: bool,
    /// Whether auto-detection matched this project
    pub detected: bool,
    pub user_invocable: bool,
    pub has_hooks: bool,
    pub inject_into: Vec<InjectionPhase>,
    pub path: PathBuf,
}

/// Reason why a skill was selected
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchReason {
    /// Matched via detect.files + detect.patterns
    AutoDetected,
    /// Matched via lexical scoring against user query
    LexicalMatch { query: String },
    /// User explicitly enabled for this session
    UserForced,
}

/// A matched skill with relevance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMatch {
    /// Relevance score
    pub score: f32,
    /// Why this skill was selected
    pub match_reason: MatchReason,
    /// Skill summary
    pub skill: SkillSummary,
}

/// Policy for skill selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionPolicy {
    /// Max skills to return (default: 3)
    pub top_k: usize,
    /// Minimum score threshold (default: 1.0)
    pub min_score: f32,
    /// Must match one of these tags (OR). Empty = no filter.
    pub include_tags: Vec<String>,
    /// Must not match any of these tags (AND NOT). Empty = no filter.
    pub exclude_tags: Vec<String>,
    /// Max lines per skill body (default: 200)
    pub max_content_lines: usize,
}

impl Default for SelectionPolicy {
    fn default() -> Self {
        Self {
            top_k: 3,
            min_score: 1.0,
            include_tags: vec![],
            exclude_tags: vec![],
            max_content_lines: 200,
        }
    }
}

/// Immutable collection of indexed skills
#[derive(Debug, Clone)]
pub struct SkillIndex {
    skills: Vec<SkillDocument>,
}

impl SkillIndex {
    /// Create a new SkillIndex from a vector of skill documents
    pub fn new(skills: Vec<SkillDocument>) -> Self {
        Self { skills }
    }

    /// Check if the index contains no skills
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Get the number of skills in the index
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Get all skill documents
    pub fn skills(&self) -> &[SkillDocument] {
        &self.skills
    }

    /// Get lightweight summaries of all skills
    pub fn summaries(&self) -> Vec<SkillSummary> {
        self.skills.iter().map(|s| s.to_summary(false)).collect()
    }

    /// Get only auto-detected applicable skills for this project
    pub fn detected_skills(&self) -> Vec<&SkillDocument> {
        self.skills
            .iter()
            .filter(|s| s.detect.is_some() && s.enabled)
            .collect()
    }

    /// Get skills filtered by source tier
    pub fn skills_by_source(&self, source: &SkillSource) -> Vec<&SkillDocument> {
        self.skills
            .iter()
            .filter(|s| std::mem::discriminant(&s.source) == std::mem::discriminant(source))
            .collect()
    }

    /// Find a skill by ID
    pub fn get_by_id(&self, id: &str) -> Option<&SkillDocument> {
        self.skills.iter().find(|s| s.id == id)
    }
}

impl SkillDocument {
    /// Convert to a lightweight summary
    pub fn to_summary(&self, detected: bool) -> SkillSummary {
        SkillSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            tags: self.tags.clone(),
            source: self.source.clone(),
            priority: self.priority,
            enabled: self.enabled,
            detected,
            user_invocable: self.user_invocable,
            has_hooks: self.hooks.is_some(),
            inject_into: self.inject_into.clone(),
            path: self.path.clone(),
        }
    }
}

/// Statistics about a skill index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndexStats {
    pub total: usize,
    pub builtin_count: usize,
    pub external_count: usize,
    pub user_count: usize,
    pub project_local_count: usize,
    pub generated_count: usize,
    pub enabled_count: usize,
    pub detected_count: usize,
}

/// Overview of skills configuration and state for a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsOverview {
    pub stats: SkillIndexStats,
    pub detected_skills: Vec<SkillSummary>,
    pub sources: Vec<String>,
}

/// A skill discovered during filesystem scanning (before indexing)
#[derive(Debug, Clone)]
pub struct DiscoveredSkill {
    pub path: PathBuf,
    pub content: String,
    pub source: SkillSource,
    pub priority: u32,
    pub detect: Option<SkillDetection>,
    pub inject_into: Vec<InjectionPhase>,
    pub enabled: bool,
}

/// A generated skill stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedSkill {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub body: String,
    pub source_session_ids: Vec<String>,
}

/// A generated skill record from the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedSkillRecord {
    pub id: String,
    pub project_path: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub body: String,
    pub source_type: String,
    pub source_session_ids: Vec<String>,
    pub usage_count: i64,
    pub success_rate: f64,
    pub keywords: Vec<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Result of parsing a skill file (before indexing)
#[derive(Debug, Clone)]
pub struct ParsedSkill {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub tags: Vec<String>,
    pub body: String,
    pub user_invocable: bool,
    pub allowed_tools: Vec<String>,
    pub license: Option<String>,
    pub metadata: HashMap<String, String>,
    pub hooks: Option<SkillHooks>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_skill(name: &str, source: SkillSource, priority: u32) -> SkillDocument {
        SkillDocument {
            id: format!("{}-abc123", name),
            name: name.to_string(),
            description: format!("Description for {}", name),
            version: Some("1.0.0".to_string()),
            tags: vec!["test".to_string()],
            body: "# Test\n\nBody content".to_string(),
            path: PathBuf::from(format!("/test/{}/SKILL.md", name)),
            hash: "abc123def456".to_string(),
            last_modified: Some(1700000000),
            user_invocable: false,
            allowed_tools: vec![],
            license: None,
            metadata: HashMap::new(),
            hooks: None,
            source,
            priority,
            detect: None,
            inject_into: vec![InjectionPhase::Always],
            enabled: true,
        }
    }

    #[test]
    fn test_selection_policy_default() {
        let policy = SelectionPolicy::default();
        assert_eq!(policy.top_k, 3);
        assert_eq!(policy.min_score, 1.0);
        assert_eq!(policy.max_content_lines, 200);
        assert!(policy.include_tags.is_empty());
        assert!(policy.exclude_tags.is_empty());
    }

    #[test]
    fn test_skill_index_empty() {
        let index = SkillIndex::new(vec![]);
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert!(index.skills().is_empty());
        assert!(index.summaries().is_empty());
        assert!(index.detected_skills().is_empty());
    }

    #[test]
    fn test_skill_index_with_skills() {
        let skills = vec![
            make_test_skill("alpha", SkillSource::Builtin, 10),
            make_test_skill(
                "beta",
                SkillSource::External {
                    source_name: "vercel".to_string(),
                },
                80,
            ),
            make_test_skill("gamma", SkillSource::ProjectLocal, 201),
        ];
        let index = SkillIndex::new(skills);

        assert!(!index.is_empty());
        assert_eq!(index.len(), 3);
        assert_eq!(index.skills().len(), 3);
    }

    #[test]
    fn test_skill_index_summaries() {
        let skills = vec![make_test_skill("alpha", SkillSource::Builtin, 10)];
        let index = SkillIndex::new(skills);
        let summaries = index.summaries();

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "alpha");
        assert_eq!(summaries[0].priority, 10);
        assert!(!summaries[0].has_hooks);
        assert!(summaries[0].enabled);
    }

    #[test]
    fn test_skill_index_detected_skills() {
        let mut skill_with_detect = make_test_skill(
            "react",
            SkillSource::External {
                source_name: "vercel".to_string(),
            },
            100,
        );
        skill_with_detect.detect = Some(SkillDetection {
            files: vec!["package.json".to_string()],
            patterns: vec!["\"react\"".to_string()],
        });

        let skill_without_detect = make_test_skill("generic", SkillSource::Builtin, 10);

        let index = SkillIndex::new(vec![skill_with_detect, skill_without_detect]);
        let detected = index.detected_skills();

        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].name, "react");
    }

    #[test]
    fn test_skill_index_detected_skills_disabled() {
        let mut skill = make_test_skill(
            "react",
            SkillSource::External {
                source_name: "vercel".to_string(),
            },
            100,
        );
        skill.detect = Some(SkillDetection {
            files: vec!["package.json".to_string()],
            patterns: vec!["\"react\"".to_string()],
        });
        skill.enabled = false;

        let index = SkillIndex::new(vec![skill]);
        assert!(index.detected_skills().is_empty());
    }

    #[test]
    fn test_skill_index_by_source() {
        let skills = vec![
            make_test_skill("alpha", SkillSource::Builtin, 10),
            make_test_skill("beta", SkillSource::Builtin, 20),
            make_test_skill(
                "gamma",
                SkillSource::External {
                    source_name: "vercel".to_string(),
                },
                80,
            ),
            make_test_skill("delta", SkillSource::ProjectLocal, 201),
        ];
        let index = SkillIndex::new(skills);

        assert_eq!(index.skills_by_source(&SkillSource::Builtin).len(), 2);
        assert_eq!(
            index
                .skills_by_source(&SkillSource::External {
                    source_name: String::new()
                })
                .len(),
            1
        );
        assert_eq!(index.skills_by_source(&SkillSource::ProjectLocal).len(), 1);
        assert_eq!(index.skills_by_source(&SkillSource::User).len(), 0);
    }

    #[test]
    fn test_skill_index_get_by_id() {
        let skills = vec![make_test_skill("alpha", SkillSource::Builtin, 10)];
        let index = SkillIndex::new(skills);

        assert!(index.get_by_id("alpha-abc123").is_some());
        assert!(index.get_by_id("nonexistent").is_none());
    }

    #[test]
    fn test_skill_document_to_summary() {
        let skill = make_test_skill("test", SkillSource::Builtin, 10);
        let summary = skill.to_summary(true);

        assert_eq!(summary.id, "test-abc123");
        assert_eq!(summary.name, "test");
        assert!(summary.detected);
        assert!(!summary.has_hooks);
    }

    #[test]
    fn test_skill_source_serialization() {
        let builtin = SkillSource::Builtin;
        let json = serde_json::to_string(&builtin).unwrap();
        assert_eq!(json, "\"builtin\"");

        let external = SkillSource::External {
            source_name: "vercel".to_string(),
        };
        let json = serde_json::to_string(&external).unwrap();
        assert!(json.contains("vercel"));

        let project = SkillSource::ProjectLocal;
        let json = serde_json::to_string(&project).unwrap();
        assert_eq!(json, "\"project_local\"");
    }

    #[test]
    fn test_injection_phase_serialization() {
        let phase = InjectionPhase::Implementation;
        let json = serde_json::to_string(&phase).unwrap();
        assert_eq!(json, "\"implementation\"");

        let phase = InjectionPhase::Always;
        let json = serde_json::to_string(&phase).unwrap();
        assert_eq!(json, "\"always\"");
    }

    #[test]
    fn test_match_reason_serialization() {
        let auto = MatchReason::AutoDetected;
        let json = serde_json::to_string(&auto).unwrap();
        assert_eq!(json, "\"auto_detected\"");

        let lexical = MatchReason::LexicalMatch {
            query: "react hooks".to_string(),
        };
        let json = serde_json::to_string(&lexical).unwrap();
        assert!(json.contains("react hooks"));
    }

    #[test]
    fn test_skill_index_stats() {
        let stats = SkillIndexStats {
            total: 10,
            builtin_count: 3,
            external_count: 4,
            user_count: 1,
            project_local_count: 2,
            generated_count: 0,
            enabled_count: 8,
            detected_count: 3,
        };
        assert_eq!(stats.total, 10);
        assert_eq!(stats.enabled_count, 8);
    }
}
