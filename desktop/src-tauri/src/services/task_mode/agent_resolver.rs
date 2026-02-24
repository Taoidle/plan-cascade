//! Agent Priority Chain Resolver
//!
//! Implements a 7-level priority chain for selecting the best agent for a given
//! story and execution phase:
//!
//! 1. Global override
//! 2. Phase-specific override
//! 3. Story-level agent (story.agent field)
//! 4. Story type inference (keywords in title -> StoryType -> type override)
//! 5. Phase defaults
//! 6. Fallback chain traversal
//! 7. Default agent

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// Enums
// ============================================================================

/// Execution phase for agent resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPhase {
    /// Planning phase (PRD generation, design)
    Planning,
    /// Implementation phase (writing code)
    Implementation,
    /// Retry phase (re-attempting after failure)
    Retry,
    /// Review phase (code review, verification)
    Review,
}

impl std::fmt::Display for ExecutionPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionPhase::Planning => write!(f, "planning"),
            ExecutionPhase::Implementation => write!(f, "implementation"),
            ExecutionPhase::Retry => write!(f, "retry"),
            ExecutionPhase::Review => write!(f, "review"),
        }
    }
}

/// Story type for agent selection heuristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryType {
    /// New feature implementation
    Feature,
    /// Bug fix
    Bugfix,
    /// Code refactoring
    Refactor,
    /// Test creation/modification
    Test,
    /// Documentation
    Documentation,
}

impl StoryType {
    /// Infer story type from title and description keywords.
    pub fn infer(title: &str, description: &str) -> StoryType {
        let combined = format!("{} {}", title, description).to_lowercase();

        // Check keywords in priority order
        if combined.contains("bug")
            || combined.contains("fix")
            || combined.contains("patch")
            || combined.contains("hotfix")
            || combined.contains("defect")
            || combined.contains("issue")
        {
            return StoryType::Bugfix;
        }

        if combined.contains("refactor")
            || combined.contains("restructure")
            || combined.contains("reorganize")
            || combined.contains("cleanup")
            || combined.contains("clean up")
            || combined.contains("simplify")
            || combined.contains("optimize")
        {
            return StoryType::Refactor;
        }

        if combined.contains("test")
            || combined.contains("spec")
            || combined.contains("coverage")
            || combined.contains("e2e")
            || combined.contains("integration test")
            || combined.contains("unit test")
        {
            return StoryType::Test;
        }

        if combined.contains("doc")
            || combined.contains("readme")
            || combined.contains("comment")
            || combined.contains("changelog")
            || combined.contains("guide")
            || combined.contains("tutorial")
        {
            return StoryType::Documentation;
        }

        // Default to Feature
        StoryType::Feature
    }
}

impl std::fmt::Display for StoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoryType::Feature => write!(f, "feature"),
            StoryType::Bugfix => write!(f, "bugfix"),
            StoryType::Refactor => write!(f, "refactor"),
            StoryType::Test => write!(f, "test"),
            StoryType::Documentation => write!(f, "documentation"),
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Per-phase agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseConfig {
    /// Default agent for this phase
    pub default_agent: Option<String>,
    /// Fallback chain (ordered list of alternative agents)
    #[serde(default)]
    pub fallback_chain: Vec<String>,
    /// Story type overrides (story_type -> agent_name)
    #[serde(default)]
    pub story_type_overrides: HashMap<String, String>,
}

impl Default for PhaseConfig {
    fn default() -> Self {
        Self {
            default_agent: None,
            fallback_chain: Vec::new(),
            story_type_overrides: HashMap::new(),
        }
    }
}

/// Agent definition for the resolver.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefinition {
    /// Agent name/identifier
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Whether this agent is currently available
    #[serde(default = "default_true")]
    pub available: bool,
    /// Phases this agent is suitable for
    #[serde(default)]
    pub suitable_phases: Vec<ExecutionPhase>,
}

fn default_true() -> bool {
    true
}

/// Global and phase-level overrides.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOverrides {
    /// Global override - applies to all phases
    pub global: Option<String>,
    /// Phase-specific overrides
    #[serde(default)]
    pub phase_overrides: HashMap<String, String>,
}

/// Full agents configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsConfig {
    /// Default agent to use when all other resolution fails
    pub default_agent: String,
    /// Agent definitions
    #[serde(default)]
    pub agents: HashMap<String, AgentDefinition>,
    /// Per-phase defaults and fallback chains
    #[serde(default)]
    pub phase_defaults: HashMap<String, PhaseConfig>,
    /// Active overrides
    #[serde(default)]
    pub overrides: AgentOverrides,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        let mut agents = HashMap::new();
        agents.insert(
            "claude-sonnet".to_string(),
            AgentDefinition {
                name: "claude-sonnet".to_string(),
                description: "Claude Sonnet - fast, capable general-purpose agent".to_string(),
                available: true,
                suitable_phases: vec![
                    ExecutionPhase::Planning,
                    ExecutionPhase::Implementation,
                    ExecutionPhase::Retry,
                    ExecutionPhase::Review,
                ],
            },
        );
        agents.insert(
            "claude-opus".to_string(),
            AgentDefinition {
                name: "claude-opus".to_string(),
                description: "Claude Opus - most capable, best for complex tasks".to_string(),
                available: true,
                suitable_phases: vec![
                    ExecutionPhase::Planning,
                    ExecutionPhase::Implementation,
                    ExecutionPhase::Review,
                ],
            },
        );
        agents.insert(
            "claude-haiku".to_string(),
            AgentDefinition {
                name: "claude-haiku".to_string(),
                description: "Claude Haiku - fastest, best for simple tasks".to_string(),
                available: true,
                suitable_phases: vec![ExecutionPhase::Implementation, ExecutionPhase::Retry],
            },
        );

        let mut phase_defaults = HashMap::new();
        phase_defaults.insert(
            ExecutionPhase::Planning.to_string(),
            PhaseConfig {
                default_agent: Some("claude-opus".to_string()),
                fallback_chain: vec!["claude-sonnet".to_string()],
                story_type_overrides: HashMap::new(),
            },
        );
        phase_defaults.insert(
            ExecutionPhase::Implementation.to_string(),
            PhaseConfig {
                default_agent: Some("claude-sonnet".to_string()),
                fallback_chain: vec!["claude-opus".to_string(), "claude-haiku".to_string()],
                story_type_overrides: {
                    let mut m = HashMap::new();
                    m.insert("documentation".to_string(), "claude-haiku".to_string());
                    m
                },
            },
        );
        phase_defaults.insert(
            ExecutionPhase::Retry.to_string(),
            PhaseConfig {
                default_agent: Some("claude-opus".to_string()),
                fallback_chain: vec!["claude-sonnet".to_string()],
                story_type_overrides: HashMap::new(),
            },
        );
        phase_defaults.insert(
            ExecutionPhase::Review.to_string(),
            PhaseConfig {
                default_agent: Some("claude-opus".to_string()),
                fallback_chain: vec!["claude-sonnet".to_string()],
                story_type_overrides: HashMap::new(),
            },
        );

        Self {
            default_agent: "claude-sonnet".to_string(),
            agents,
            phase_defaults,
            overrides: AgentOverrides::default(),
        }
    }
}

// ============================================================================
// Agent Assignment
// ============================================================================

/// Resolution level indicating which priority level produced the match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionLevel {
    /// Level 1: Global override
    GlobalOverride,
    /// Level 2: Phase-specific override
    PhaseOverride,
    /// Level 3: Story-level agent assignment
    StoryLevel,
    /// Level 4: Story type inference
    StoryTypeInference,
    /// Level 5: Phase default
    PhaseDefault,
    /// Level 6: Fallback chain
    FallbackChain,
    /// Level 7: Default agent
    DefaultAgent,
}

impl std::fmt::Display for ResolutionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolutionLevel::GlobalOverride => write!(f, "global_override"),
            ResolutionLevel::PhaseOverride => write!(f, "phase_override"),
            ResolutionLevel::StoryLevel => write!(f, "story_level"),
            ResolutionLevel::StoryTypeInference => write!(f, "story_type_inference"),
            ResolutionLevel::PhaseDefault => write!(f, "phase_default"),
            ResolutionLevel::FallbackChain => write!(f, "fallback_chain"),
            ResolutionLevel::DefaultAgent => write!(f, "default_agent"),
        }
    }
}

/// Result of agent resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAssignment {
    /// Selected agent name
    pub agent_name: String,
    /// Phase the agent was resolved for
    pub phase: ExecutionPhase,
    /// Which priority level matched
    pub resolution_level: ResolutionLevel,
    /// Human-readable reasoning
    pub reasoning: String,
}

// ============================================================================
// Story Info (input to resolver)
// ============================================================================

/// Minimal story information needed for agent resolution.
#[derive(Debug, Clone)]
pub struct StoryInfo {
    /// Story title
    pub title: String,
    /// Story description
    pub description: String,
    /// Explicitly assigned agent (from story.agent field in PRD)
    pub agent: Option<String>,
}

// ============================================================================
// Agent Resolver
// ============================================================================

/// Agent resolver with 7-level priority chain.
pub struct AgentResolver {
    config: AgentsConfig,
}

impl AgentResolver {
    /// Create a new resolver with the given configuration.
    pub fn new(config: AgentsConfig) -> Self {
        Self { config }
    }

    /// Create a resolver with default configuration.
    pub fn with_defaults() -> Self {
        Self {
            config: AgentsConfig::default(),
        }
    }

    /// Resolve the best agent for a story and phase.
    ///
    /// Implements the 7-level priority chain:
    /// 1. Global override
    /// 2. Phase-specific override
    /// 3. Story-level agent
    /// 4. Story type inference
    /// 5. Phase defaults
    /// 6. Fallback chain traversal
    /// 7. Default agent
    pub fn resolve(&self, story: &StoryInfo, phase: ExecutionPhase) -> AgentAssignment {
        // Level 1: Global override
        if let Some(ref global) = self.config.overrides.global {
            if self.is_available(global) {
                return AgentAssignment {
                    agent_name: global.clone(),
                    phase,
                    resolution_level: ResolutionLevel::GlobalOverride,
                    reasoning: format!("Global override to '{}'", global),
                };
            }
        }

        // Level 2: Phase-specific override
        if let Some(phase_agent) = self
            .config
            .overrides
            .phase_overrides
            .get(&phase.to_string())
        {
            if self.is_available(phase_agent) {
                return AgentAssignment {
                    agent_name: phase_agent.clone(),
                    phase,
                    resolution_level: ResolutionLevel::PhaseOverride,
                    reasoning: format!("Phase override for {} to '{}'", phase, phase_agent),
                };
            }
        }

        // Level 3: Story-level agent
        if let Some(ref story_agent) = story.agent {
            if self.is_available(story_agent) {
                return AgentAssignment {
                    agent_name: story_agent.clone(),
                    phase,
                    resolution_level: ResolutionLevel::StoryLevel,
                    reasoning: format!("Story-level agent assignment to '{}'", story_agent),
                };
            }
        }

        // Level 4: Story type inference
        let story_type = StoryType::infer(&story.title, &story.description);
        if let Some(phase_config) = self.config.phase_defaults.get(&phase.to_string()) {
            if let Some(type_agent) = phase_config
                .story_type_overrides
                .get(&story_type.to_string())
            {
                if self.is_available(type_agent) {
                    return AgentAssignment {
                        agent_name: type_agent.clone(),
                        phase,
                        resolution_level: ResolutionLevel::StoryTypeInference,
                        reasoning: format!(
                            "Story type '{}' maps to '{}' for {} phase",
                            story_type, type_agent, phase
                        ),
                    };
                }
            }
        }

        // Level 5: Phase defaults
        if let Some(phase_config) = self.config.phase_defaults.get(&phase.to_string()) {
            if let Some(ref default_agent) = phase_config.default_agent {
                if self.is_available(default_agent) {
                    return AgentAssignment {
                        agent_name: default_agent.clone(),
                        phase,
                        resolution_level: ResolutionLevel::PhaseDefault,
                        reasoning: format!("Phase default for {} is '{}'", phase, default_agent),
                    };
                }
            }

            // Level 6: Fallback chain traversal
            for fallback in &phase_config.fallback_chain {
                if self.is_available(fallback) {
                    return AgentAssignment {
                        agent_name: fallback.clone(),
                        phase,
                        resolution_level: ResolutionLevel::FallbackChain,
                        reasoning: format!(
                            "Fallback chain for {} phase selected '{}'",
                            phase, fallback
                        ),
                    };
                }
            }
        }

        // Level 7: Default agent
        AgentAssignment {
            agent_name: self.config.default_agent.clone(),
            phase,
            resolution_level: ResolutionLevel::DefaultAgent,
            reasoning: format!("Using default agent '{}'", self.config.default_agent),
        }
    }

    /// Check if an agent is available.
    fn is_available(&self, agent_name: &str) -> bool {
        self.config
            .agents
            .get(agent_name)
            .map(|a| a.available)
            .unwrap_or(false)
    }

    /// Get the current configuration.
    pub fn config(&self) -> &AgentsConfig {
        &self.config
    }

    /// Update the configuration.
    pub fn set_config(&mut self, config: AgentsConfig) {
        self.config = config;
    }

    /// Set a global override.
    pub fn set_global_override(&mut self, agent: Option<String>) {
        self.config.overrides.global = agent;
    }

    /// Set a phase-level override.
    pub fn set_phase_override(&mut self, phase: ExecutionPhase, agent: Option<String>) {
        match agent {
            Some(a) => {
                self.config
                    .overrides
                    .phase_overrides
                    .insert(phase.to_string(), a);
            }
            None => {
                self.config
                    .overrides
                    .phase_overrides
                    .remove(&phase.to_string());
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_story() -> StoryInfo {
        StoryInfo {
            title: "Implement user authentication".to_string(),
            description: "Build OAuth2 authentication flow".to_string(),
            agent: None,
        }
    }

    fn test_bugfix_story() -> StoryInfo {
        StoryInfo {
            title: "Fix login bug".to_string(),
            description: "Users cannot log in due to a bug in the auth flow".to_string(),
            agent: None,
        }
    }

    fn test_doc_story() -> StoryInfo {
        StoryInfo {
            title: "Write API documentation".to_string(),
            description: "Create comprehensive API docs and guide".to_string(),
            agent: None,
        }
    }

    // ========================================================================
    // Story Type Inference Tests
    // ========================================================================

    #[test]
    fn test_story_type_infer_feature() {
        let st = StoryType::infer("Implement user authentication", "Build OAuth2 flow");
        assert_eq!(st, StoryType::Feature);
    }

    #[test]
    fn test_story_type_infer_bugfix() {
        let st = StoryType::infer("Fix login bug", "Users cannot login");
        assert_eq!(st, StoryType::Bugfix);
    }

    #[test]
    fn test_story_type_infer_refactor() {
        let st = StoryType::infer("Refactor database module", "Simplify the query layer");
        assert_eq!(st, StoryType::Refactor);
    }

    #[test]
    fn test_story_type_infer_test() {
        let st = StoryType::infer("Add unit tests for auth", "Improve coverage");
        assert_eq!(st, StoryType::Test);
    }

    #[test]
    fn test_story_type_infer_documentation() {
        let st = StoryType::infer("Write API documentation", "Create API docs");
        assert_eq!(st, StoryType::Documentation);
    }

    // ========================================================================
    // Agent Resolution Tests
    // ========================================================================

    #[test]
    fn test_default_agent_as_last_resort() {
        let mut config = AgentsConfig::default();
        // Make all agents unavailable except default
        for (_, agent) in config.agents.iter_mut() {
            agent.available = false;
        }
        // But keep the default available
        config.agents.get_mut("claude-sonnet").unwrap().available = true;

        // Clear phase defaults
        config.phase_defaults.clear();

        let resolver = AgentResolver::new(config);
        let assignment = resolver.resolve(&test_story(), ExecutionPhase::Implementation);
        assert_eq!(assignment.agent_name, "claude-sonnet");
        assert_eq!(assignment.resolution_level, ResolutionLevel::DefaultAgent);
    }

    #[test]
    fn test_global_override_takes_precedence() {
        let mut config = AgentsConfig::default();
        config.overrides.global = Some("claude-opus".to_string());

        let resolver = AgentResolver::new(config);
        let assignment = resolver.resolve(&test_story(), ExecutionPhase::Implementation);
        assert_eq!(assignment.agent_name, "claude-opus");
        assert_eq!(assignment.resolution_level, ResolutionLevel::GlobalOverride);
    }

    #[test]
    fn test_phase_override() {
        let mut config = AgentsConfig::default();
        config
            .overrides
            .phase_overrides
            .insert("implementation".to_string(), "claude-haiku".to_string());

        let resolver = AgentResolver::new(config);
        let assignment = resolver.resolve(&test_story(), ExecutionPhase::Implementation);
        assert_eq!(assignment.agent_name, "claude-haiku");
        assert_eq!(assignment.resolution_level, ResolutionLevel::PhaseOverride);
    }

    #[test]
    fn test_story_level_assignment() {
        let resolver = AgentResolver::with_defaults();
        let story = StoryInfo {
            title: "Test".to_string(),
            description: "Test".to_string(),
            agent: Some("claude-opus".to_string()),
        };
        let assignment = resolver.resolve(&story, ExecutionPhase::Implementation);
        assert_eq!(assignment.agent_name, "claude-opus");
        assert_eq!(assignment.resolution_level, ResolutionLevel::StoryLevel);
    }

    #[test]
    fn test_story_type_inference_for_docs() {
        let resolver = AgentResolver::with_defaults();
        let assignment = resolver.resolve(&test_doc_story(), ExecutionPhase::Implementation);
        // Documentation stories should get claude-haiku in implementation phase
        assert_eq!(assignment.agent_name, "claude-haiku");
        assert_eq!(
            assignment.resolution_level,
            ResolutionLevel::StoryTypeInference
        );
    }

    #[test]
    fn test_phase_default() {
        let resolver = AgentResolver::with_defaults();
        let assignment = resolver.resolve(&test_story(), ExecutionPhase::Planning);
        assert_eq!(assignment.agent_name, "claude-opus");
        assert_eq!(assignment.resolution_level, ResolutionLevel::PhaseDefault);
    }

    #[test]
    fn test_fallback_chain() {
        let mut config = AgentsConfig::default();
        // Make the default planning agent unavailable
        config.agents.get_mut("claude-opus").unwrap().available = false;

        let resolver = AgentResolver::new(config);
        let assignment = resolver.resolve(&test_story(), ExecutionPhase::Planning);
        // Should fall back to claude-sonnet
        assert_eq!(assignment.agent_name, "claude-sonnet");
        assert_eq!(assignment.resolution_level, ResolutionLevel::FallbackChain);
    }

    #[test]
    fn test_unavailable_global_override_skips_to_next_level() {
        let mut config = AgentsConfig::default();
        config.overrides.global = Some("nonexistent-agent".to_string());

        let resolver = AgentResolver::new(config);
        let assignment = resolver.resolve(&test_story(), ExecutionPhase::Implementation);
        // Should skip the unavailable global override and resolve normally
        assert_ne!(assignment.agent_name, "nonexistent-agent");
        assert_ne!(assignment.resolution_level, ResolutionLevel::GlobalOverride);
    }

    #[test]
    fn test_unavailable_story_agent_skips_to_next_level() {
        let resolver = AgentResolver::with_defaults();
        let story = StoryInfo {
            title: "Implement feature".to_string(),
            description: "Build something".to_string(),
            agent: Some("nonexistent-agent".to_string()),
        };
        let assignment = resolver.resolve(&story, ExecutionPhase::Implementation);
        assert_ne!(assignment.agent_name, "nonexistent-agent");
        assert_ne!(assignment.resolution_level, ResolutionLevel::StoryLevel);
    }

    #[test]
    fn test_retry_phase_uses_different_agent() {
        let resolver = AgentResolver::with_defaults();
        let impl_assignment = resolver.resolve(&test_story(), ExecutionPhase::Implementation);
        let retry_assignment = resolver.resolve(&test_story(), ExecutionPhase::Retry);

        // By default, implementation uses sonnet, retry uses opus
        assert_eq!(impl_assignment.agent_name, "claude-sonnet");
        assert_eq!(retry_assignment.agent_name, "claude-opus");
    }

    // ========================================================================
    // Serialization Tests
    // ========================================================================

    #[test]
    fn test_execution_phase_serialization() {
        let json = serde_json::to_string(&ExecutionPhase::Planning).unwrap();
        assert_eq!(json, "\"planning\"");
        let json = serde_json::to_string(&ExecutionPhase::Implementation).unwrap();
        assert_eq!(json, "\"implementation\"");
        let json = serde_json::to_string(&ExecutionPhase::Retry).unwrap();
        assert_eq!(json, "\"retry\"");
        let json = serde_json::to_string(&ExecutionPhase::Review).unwrap();
        assert_eq!(json, "\"review\"");
    }

    #[test]
    fn test_story_type_serialization() {
        let json = serde_json::to_string(&StoryType::Feature).unwrap();
        assert_eq!(json, "\"feature\"");
        let json = serde_json::to_string(&StoryType::Bugfix).unwrap();
        assert_eq!(json, "\"bugfix\"");
    }

    #[test]
    fn test_agents_config_serialization() {
        let config = AgentsConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("defaultAgent"));
        assert!(json.contains("phaseDefaults"));

        // Round-trip
        let parsed: AgentsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.default_agent, "claude-sonnet");
        assert!(parsed.agents.contains_key("claude-opus"));
    }

    #[test]
    fn test_agent_assignment_serialization() {
        let assignment = AgentAssignment {
            agent_name: "claude-sonnet".to_string(),
            phase: ExecutionPhase::Implementation,
            resolution_level: ResolutionLevel::PhaseDefault,
            reasoning: "Phase default".to_string(),
        };
        let json = serde_json::to_string(&assignment).unwrap();
        assert!(json.contains("\"agentName\""));
        assert!(json.contains("\"resolutionLevel\""));
    }

    // ========================================================================
    // Set Override Tests
    // ========================================================================

    #[test]
    fn test_set_global_override() {
        let mut resolver = AgentResolver::with_defaults();
        resolver.set_global_override(Some("claude-opus".to_string()));
        let assignment = resolver.resolve(&test_story(), ExecutionPhase::Implementation);
        assert_eq!(assignment.agent_name, "claude-opus");
        assert_eq!(assignment.resolution_level, ResolutionLevel::GlobalOverride);

        // Clear override
        resolver.set_global_override(None);
        let assignment = resolver.resolve(&test_story(), ExecutionPhase::Implementation);
        assert_ne!(assignment.resolution_level, ResolutionLevel::GlobalOverride);
    }

    #[test]
    fn test_set_phase_override() {
        let mut resolver = AgentResolver::with_defaults();
        resolver.set_phase_override(
            ExecutionPhase::Implementation,
            Some("claude-haiku".to_string()),
        );
        let assignment = resolver.resolve(&test_story(), ExecutionPhase::Implementation);
        assert_eq!(assignment.agent_name, "claude-haiku");
        assert_eq!(assignment.resolution_level, ResolutionLevel::PhaseOverride);

        // Clear override
        resolver.set_phase_override(ExecutionPhase::Implementation, None);
        let assignment = resolver.resolve(&test_story(), ExecutionPhase::Implementation);
        assert_ne!(assignment.resolution_level, ResolutionLevel::PhaseOverride);
    }
}
