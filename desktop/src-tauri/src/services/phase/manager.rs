//! Phase Manager Service
//!
//! Manages Phase to Agent mapping and provides agent selection based on phase and story type.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

use crate::models::prd::StoryType;

/// Execution phases in the Plan Cascade workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Initial planning and PRD generation
    Planning,
    /// Story implementation
    Implementation,
    /// Retry after failure
    Retry,
    /// Code refactoring
    Refactor,
    /// Code review
    Review,
}

impl Default for Phase {
    fn default() -> Self {
        Phase::Implementation
    }
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Phase::Planning => write!(f, "planning"),
            Phase::Implementation => write!(f, "implementation"),
            Phase::Retry => write!(f, "retry"),
            Phase::Refactor => write!(f, "refactor"),
            Phase::Review => write!(f, "review"),
        }
    }
}

impl Phase {
    /// Parse phase from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "planning" => Some(Phase::Planning),
            "implementation" => Some(Phase::Implementation),
            "retry" => Some(Phase::Retry),
            "refactor" => Some(Phase::Refactor),
            "review" => Some(Phase::Review),
            _ => None,
        }
    }
}

/// Configuration for a specific phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseConfig {
    /// Default agent for this phase
    #[serde(default = "default_agent")]
    pub default_agent: String,
    /// Fallback chain when default agent fails
    #[serde(default)]
    pub fallback_chain: Vec<String>,
    /// Story type overrides - maps story type to specific agent
    #[serde(default)]
    pub story_type_overrides: HashMap<String, String>,
    /// Timeout in seconds for this phase
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_agent() -> String {
    "claude-code".to_string()
}

fn default_timeout() -> u64 {
    600 // 10 minutes
}

fn default_max_retries() -> u32 {
    3
}

impl Default for PhaseConfig {
    fn default() -> Self {
        Self {
            default_agent: default_agent(),
            fallback_chain: vec!["claude-code".to_string()],
            story_type_overrides: HashMap::new(),
            timeout_seconds: default_timeout(),
            max_retries: default_max_retries(),
        }
    }
}

impl PhaseConfig {
    /// Create a new phase config with the given default agent
    pub fn new(default_agent: impl Into<String>) -> Self {
        Self {
            default_agent: default_agent.into(),
            ..Default::default()
        }
    }

    /// Add a fallback agent
    pub fn with_fallback(mut self, agent: impl Into<String>) -> Self {
        self.fallback_chain.push(agent.into());
        self
    }

    /// Add a story type override
    pub fn with_story_override(mut self, story_type: StoryType, agent: impl Into<String>) -> Self {
        self.story_type_overrides.insert(story_type.to_string(), agent.into());
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }
}

/// Errors that can occur in phase management
#[derive(Debug, Error)]
pub enum PhaseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("Unknown phase: {0}")]
    UnknownPhase(String),

    #[error("Configuration not found")]
    ConfigNotFound,
}

/// Phase defaults configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PhaseDefaults {
    /// Version of the config format
    #[serde(default = "default_version")]
    pub version: String,
    /// Phase configurations
    #[serde(default)]
    pub phases: HashMap<String, PhaseConfig>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// Phase Manager Service
///
/// Manages Phase to Agent mapping and provides agent selection based on phase and story type.
#[derive(Debug, Clone)]
pub struct PhaseManager {
    /// Phase configurations
    configs: HashMap<Phase, PhaseConfig>,
}

impl Default for PhaseManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseManager {
    /// Create a new PhaseManager with default configurations
    ///
    /// Default mappings:
    /// - Planning → codex
    /// - Implementation → claude-code
    /// - Retry → claude-code
    /// - Refactor → aider
    /// - Review → claude-code
    pub fn new() -> Self {
        let mut configs = HashMap::new();

        // Planning phase - codex for PRD generation
        configs.insert(
            Phase::Planning,
            PhaseConfig::new("codex")
                .with_fallback("claude-code"),
        );

        // Implementation phase - claude-code for coding
        configs.insert(
            Phase::Implementation,
            PhaseConfig::new("claude-code")
                .with_story_override(StoryType::Refactor, "aider")
                .with_story_override(StoryType::Bugfix, "claude-code"),
        );

        // Retry phase - claude-code with more context
        configs.insert(
            Phase::Retry,
            PhaseConfig::new("claude-code")
                .with_timeout(900), // 15 minutes for retries
        );

        // Refactor phase - aider for code restructuring
        configs.insert(
            Phase::Refactor,
            PhaseConfig::new("aider")
                .with_fallback("claude-code"),
        );

        // Review phase - claude-code for code review
        configs.insert(
            Phase::Review,
            PhaseConfig::new("claude-code"),
        );

        Self { configs }
    }

    /// Load phase defaults from a configuration file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, PhaseError> {
        let content = std::fs::read_to_string(path)?;
        let defaults: PhaseDefaults = serde_json::from_str(&content)?;

        let mut configs = HashMap::new();
        for (phase_str, config) in defaults.phases {
            if let Some(phase) = Phase::from_str(&phase_str) {
                configs.insert(phase, config);
            }
        }

        // Fill in missing phases with defaults
        let default_manager = Self::new();
        for phase in [Phase::Planning, Phase::Implementation, Phase::Retry, Phase::Refactor, Phase::Review] {
            if !configs.contains_key(&phase) {
                if let Some(default_config) = default_manager.configs.get(&phase) {
                    configs.insert(phase, default_config.clone());
                }
            }
        }

        Ok(Self { configs })
    }

    /// Try to load from phase_defaults.json in the project root, or use defaults
    pub fn load_or_default(project_root: impl AsRef<Path>) -> Self {
        let config_path = project_root.as_ref().join("phase_defaults.json");
        if config_path.exists() {
            Self::from_file(&config_path).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Get the default agent for a specific phase
    pub fn get_agent_for_phase(&self, phase: Phase) -> &str {
        self.configs
            .get(&phase)
            .map(|c| c.default_agent.as_str())
            .unwrap_or("claude-code")
    }

    /// Get the agent for a story based on its type and the current phase
    ///
    /// Priority:
    /// 1. Story's explicit agent field (if set)
    /// 2. Phase's story_type_override for this story type
    /// 3. Phase's default agent
    ///
    /// Returns owned String because the agent may come from different sources
    pub fn get_agent_for_story(
        &self,
        phase: Phase,
        story_type: Option<StoryType>,
        explicit_agent: Option<&str>,
    ) -> String {
        // 1. Explicit agent on story takes highest priority
        if let Some(agent) = explicit_agent {
            return agent.to_string();
        }

        // 2. Check story type overrides
        if let Some(story_type) = story_type {
            if let Some(config) = self.configs.get(&phase) {
                if let Some(agent) = config.story_type_overrides.get(&story_type.to_string()) {
                    return agent.clone();
                }
            }
        }

        // 3. Fall back to phase default
        self.get_agent_for_phase(phase).to_string()
    }

    /// Get the phase configuration
    pub fn get_config(&self, phase: Phase) -> Option<&PhaseConfig> {
        self.configs.get(&phase)
    }

    /// Get the fallback chain for a phase
    pub fn get_fallback_chain(&self, phase: Phase) -> Vec<&str> {
        self.configs
            .get(&phase)
            .map(|c| c.fallback_chain.iter().map(|s| s.as_str()).collect())
            .unwrap_or_else(|| vec!["claude-code"])
    }

    /// Get timeout for a phase in seconds
    pub fn get_timeout(&self, phase: Phase) -> u64 {
        self.configs
            .get(&phase)
            .map(|c| c.timeout_seconds)
            .unwrap_or(default_timeout())
    }

    /// Get max retries for a phase
    pub fn get_max_retries(&self, phase: Phase) -> u32 {
        self.configs
            .get(&phase)
            .map(|c| c.max_retries)
            .unwrap_or(default_max_retries())
    }

    /// Update a phase configuration
    pub fn set_config(&mut self, phase: Phase, config: PhaseConfig) {
        self.configs.insert(phase, config);
    }

    /// Save current configuration to a file
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), PhaseError> {
        let mut phases = HashMap::new();
        for (phase, config) in &self.configs {
            phases.insert(phase.to_string(), config.clone());
        }

        let defaults = PhaseDefaults {
            version: default_version(),
            phases,
        };

        let content = serde_json::to_string_pretty(&defaults)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_display() {
        assert_eq!(Phase::Planning.to_string(), "planning");
        assert_eq!(Phase::Implementation.to_string(), "implementation");
        assert_eq!(Phase::Retry.to_string(), "retry");
        assert_eq!(Phase::Refactor.to_string(), "refactor");
        assert_eq!(Phase::Review.to_string(), "review");
    }

    #[test]
    fn test_phase_from_str() {
        assert_eq!(Phase::from_str("planning"), Some(Phase::Planning));
        assert_eq!(Phase::from_str("IMPLEMENTATION"), Some(Phase::Implementation));
        assert_eq!(Phase::from_str("unknown"), None);
    }

    #[test]
    fn test_default_agent_mappings() {
        let manager = PhaseManager::new();

        assert_eq!(manager.get_agent_for_phase(Phase::Planning), "codex");
        assert_eq!(manager.get_agent_for_phase(Phase::Implementation), "claude-code");
        assert_eq!(manager.get_agent_for_phase(Phase::Retry), "claude-code");
        assert_eq!(manager.get_agent_for_phase(Phase::Refactor), "aider");
        assert_eq!(manager.get_agent_for_phase(Phase::Review), "claude-code");
    }

    #[test]
    fn test_story_type_override() {
        let manager = PhaseManager::new();

        // Refactor story type should use aider even in implementation phase
        let agent = manager.get_agent_for_story(
            Phase::Implementation,
            Some(StoryType::Refactor),
            None,
        );
        assert_eq!(agent, "aider");

        // Feature story type uses default implementation agent
        let agent = manager.get_agent_for_story(
            Phase::Implementation,
            Some(StoryType::Feature),
            None,
        );
        assert_eq!(agent, "claude-code");
    }

    #[test]
    fn test_explicit_agent_priority() {
        let manager = PhaseManager::new();

        // Explicit agent takes priority over everything
        let agent = manager.get_agent_for_story(
            Phase::Implementation,
            Some(StoryType::Refactor),
            Some("custom-agent"),
        );
        assert_eq!(agent.as_str(), "custom-agent");
    }

    #[test]
    fn test_fallback_chain() {
        let manager = PhaseManager::new();

        let fallbacks = manager.get_fallback_chain(Phase::Planning);
        assert!(fallbacks.contains(&"claude-code"));
    }

    #[test]
    fn test_phase_config_builder() {
        let config = PhaseConfig::new("test-agent")
            .with_fallback("fallback-1")
            .with_fallback("fallback-2")
            .with_story_override(StoryType::Bugfix, "bugfix-agent")
            .with_timeout(1200);

        assert_eq!(config.default_agent, "test-agent");
        assert_eq!(config.fallback_chain.len(), 3); // default + 2 added
        assert_eq!(config.story_type_overrides.get("bugfix"), Some(&"bugfix-agent".to_string()));
        assert_eq!(config.timeout_seconds, 1200);
    }

    #[test]
    fn test_timeout_and_retries() {
        let manager = PhaseManager::new();

        assert_eq!(manager.get_timeout(Phase::Retry), 900); // Custom timeout
        assert_eq!(manager.get_timeout(Phase::Implementation), 600); // Default

        assert_eq!(manager.get_max_retries(Phase::Implementation), 3);
    }

    #[test]
    fn test_phase_defaults_serialization() {
        let manager = PhaseManager::new();
        let temp_dir = std::env::temp_dir();
        let config_path = temp_dir.join("test_phase_defaults.json");

        // Save
        manager.save_to_file(&config_path).unwrap();

        // Load
        let loaded = PhaseManager::from_file(&config_path).unwrap();
        assert_eq!(
            loaded.get_agent_for_phase(Phase::Planning),
            manager.get_agent_for_phase(Phase::Planning)
        );

        // Cleanup
        std::fs::remove_file(config_path).ok();
    }

    #[test]
    fn test_load_or_default() {
        let manager = PhaseManager::load_or_default("/nonexistent/path");

        // Should use defaults when file doesn't exist
        assert_eq!(manager.get_agent_for_phase(Phase::Planning), "codex");
    }
}
