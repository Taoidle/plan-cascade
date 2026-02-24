//! Plugin Data Models
//!
//! Data types for the Claude Code-compatible plugin system.
//! Compatible with the Claude Code plugin.json manifest format.
//!
//! ## Key Types
//!
//! - `PluginManifest` - parsed from plugin.json (name, version, description, etc.)
//! - `LoadedPlugin` - a fully loaded plugin with manifest + skills + commands + hooks
//! - `PluginSource` - where the plugin was loaded from (ClaudeCode, Installed, ProjectLocal)
//! - `HookEvent` - 14 Claude Code lifecycle hook events
//! - `HookType` - Command (shell) or Prompt (LLM) hook
//! - `PluginHook` - a hook definition with event, matcher, type, command/prompt
//! - `PluginSkill` - a skill from a plugin's skills/ directory
//! - `PluginCommand` - a command from a plugin's commands/ directory
//! - `PluginPermissions` - allow/deny tool lists
//! - `ShellResult` - result from executing a shell hook

use serde::{Deserialize, Deserializer, Serialize};

// ============================================================================
// Plugin Source
// ============================================================================

/// Where a plugin was discovered and loaded from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginSource {
    /// Loaded from project-level .claude-plugin/ directory (highest priority)
    ClaudeCode,
    /// Installed in ~/.claude/plugins/cache/ (medium priority)
    Installed,
    /// Stored in ~/.plan-cascade/plugins/ (lowest priority)
    ProjectLocal,
}

impl std::fmt::Display for PluginSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginSource::ClaudeCode => write!(f, "claude-code"),
            PluginSource::Installed => write!(f, "installed"),
            PluginSource::ProjectLocal => write!(f, "project-local"),
        }
    }
}

// ============================================================================
// Plugin Manifest (compatible with Claude Code plugin.json)
// ============================================================================

/// Plugin manifest parsed from plugin.json.
///
/// Compatible with the Claude Code plugin.json format:
/// ```json
/// {
///   "name": "my-plugin",
///   "version": "1.0.0",
///   "description": "A Claude Code plugin",
///   "author": "author-name",
///   "repository": "https://github.com/...",
///   "license": "MIT",
///   "keywords": ["linting", "testing"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin name (unique identifier, kebab-case)
    pub name: String,
    /// Semantic version
    #[serde(default)]
    pub version: String,
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Author name (accepts both `"author": "name"` and `"author": { "name": "...", "url": "..." }`)
    #[serde(default, deserialize_with = "deserialize_author")]
    pub author: Option<String>,
    /// Repository URL
    #[serde(default)]
    pub repository: Option<String>,
    /// License identifier (e.g. "MIT", "Apache-2.0")
    #[serde(default)]
    pub license: Option<String>,
    /// Categorization keywords
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// Deserialize `author` from either a plain string or an object `{ "name": "...", ... }`.
fn deserialize_author<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(serde_json::Value::Object(map)) => Ok(map
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())),
        Some(_) => Ok(None),
    }
}

impl Default for PluginManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: "0.0.0".to_string(),
            description: String::new(),
            author: None,
            repository: None,
            license: None,
            keywords: vec![],
        }
    }
}

// ============================================================================
// Plugin Hook Types
// ============================================================================

/// Claude Code lifecycle hook events.
///
/// These 14 events cover the full lifecycle of a Claude Code session,
/// from session start through tool use, compilation, and session end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    /// Fired when a new session starts
    SessionStart,
    /// Fired when user submits a prompt
    UserPromptSubmit,
    /// Fired before context compaction
    PreCompact,
    /// Fired after context compaction
    PostCompact,
    /// Fired before a tool is executed
    PreToolUse,
    /// Fired after a tool has executed
    PostToolUse,
    /// Fired when the agent stops (end of turn)
    Stop,
    /// Fired on sub-agent spawn
    SubAgentSpawn,
    /// Fired on sub-agent completion
    SubAgentComplete,
    /// Fired before the LLM is called
    PreLlmCall,
    /// Fired after the LLM responds
    PostLlmCall,
    /// Fired on notification events
    Notification,
    /// Fired on error events
    Error,
    /// Fired on session end (cleanup)
    SessionEnd,
}

impl HookEvent {
    /// Parse a hook event from a string (case-insensitive match).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s {
            "SessionStart" | "session_start" | "sessionStart" => Some(Self::SessionStart),
            "UserPromptSubmit" | "user_prompt_submit" | "userPromptSubmit" => {
                Some(Self::UserPromptSubmit)
            }
            "PreCompact" | "pre_compact" | "preCompact" => Some(Self::PreCompact),
            "PostCompact" | "post_compact" | "postCompact" => Some(Self::PostCompact),
            "PreToolUse" | "pre_tool_use" | "preToolUse" => Some(Self::PreToolUse),
            "PostToolUse" | "post_tool_use" | "postToolUse" => Some(Self::PostToolUse),
            "Stop" | "stop" => Some(Self::Stop),
            "SubAgentSpawn" | "sub_agent_spawn" | "subAgentSpawn" => Some(Self::SubAgentSpawn),
            "SubAgentComplete" | "sub_agent_complete" | "subAgentComplete" => {
                Some(Self::SubAgentComplete)
            }
            "PreLlmCall" | "pre_llm_call" | "preLlmCall" => Some(Self::PreLlmCall),
            "PostLlmCall" | "post_llm_call" | "postLlmCall" => Some(Self::PostLlmCall),
            "Notification" | "notification" => Some(Self::Notification),
            "Error" | "error" => Some(Self::Error),
            "SessionEnd" | "session_end" | "sessionEnd" => Some(Self::SessionEnd),
            _ => None,
        }
    }

    /// Get all 14 hook event variants.
    pub fn all_variants() -> Vec<Self> {
        vec![
            Self::SessionStart,
            Self::UserPromptSubmit,
            Self::PreCompact,
            Self::PostCompact,
            Self::PreToolUse,
            Self::PostToolUse,
            Self::Stop,
            Self::SubAgentSpawn,
            Self::SubAgentComplete,
            Self::PreLlmCall,
            Self::PostLlmCall,
            Self::Notification,
            Self::Error,
            Self::SessionEnd,
        ]
    }
}

/// Type of hook execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookType {
    /// Shell command hook - executes a shell command
    Command,
    /// Prompt hook - evaluates a prompt with an LLM for a single turn
    Prompt,
}

/// A plugin hook definition.
///
/// Hooks are triggered on specific lifecycle events. They can either
/// execute a shell command or evaluate a prompt with an LLM.
///
/// For PreToolUse hooks, an exit code of 2 blocks the tool execution.
/// Exit code 0 continues execution, and stdout is injected as context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHook {
    /// Which lifecycle event triggers this hook
    pub event: HookEvent,
    /// Optional regex pattern to filter by tool name (for PreToolUse/PostToolUse)
    #[serde(default)]
    pub matcher: Option<String>,
    /// Type of hook execution (Command or Prompt)
    pub hook_type: HookType,
    /// Shell command (for Command type) or prompt text (for Prompt type)
    pub command: String,
    /// Timeout in milliseconds (default: 10000)
    #[serde(default = "default_hook_timeout")]
    pub timeout: u64,
    /// If true, hook runs in background without blocking
    #[serde(default)]
    pub async_hook: bool,
}

fn default_hook_timeout() -> u64 {
    10_000
}

// ============================================================================
// Plugin Skill
// ============================================================================

/// A skill within a plugin, parsed from skills/*/SKILL.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSkill {
    /// Skill name
    pub name: String,
    /// Skill description
    pub description: String,
    /// Whether the user can invoke this skill directly
    #[serde(default)]
    pub user_invocable: bool,
    /// Tools this skill is allowed to use (empty = all tools)
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Full skill body (markdown content)
    pub body: String,
    /// Optional hooks defined within the skill
    #[serde(default)]
    pub hooks: Vec<PluginHook>,
}

// ============================================================================
// Plugin Command
// ============================================================================

/// A command within a plugin, parsed from commands/*.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommand {
    /// Command name (derived from filename)
    pub name: String,
    /// Command description (from first heading or filename)
    pub description: String,
    /// Full command body (markdown content with instructions)
    pub body: String,
}

// ============================================================================
// Plugin Permissions
// ============================================================================

/// Permission configuration for a plugin.
///
/// Controls which tools the plugin is allowed to use.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginPermissions {
    /// Explicitly allowed tools (empty = all allowed)
    #[serde(default)]
    pub allow: Vec<String>,
    /// Explicitly denied tools
    #[serde(default)]
    pub deny: Vec<String>,
    /// Tools that are always approved without user confirmation
    #[serde(default)]
    pub always_approve: Vec<String>,
}

// ============================================================================
// Loaded Plugin
// ============================================================================

/// A fully loaded and parsed plugin.
///
/// Contains the manifest plus all discovered components:
/// skills, commands, hooks, instructions, and permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedPlugin {
    /// Plugin manifest from plugin.json
    pub manifest: PluginManifest,
    /// Where the plugin was loaded from
    pub source: PluginSource,
    /// Whether the plugin is currently enabled
    pub enabled: bool,
    /// Root path of the plugin directory
    pub root_path: String,
    /// Discovered skills (from skills/*/SKILL.md)
    pub skills: Vec<PluginSkill>,
    /// Discovered commands (from commands/*.md)
    pub commands: Vec<PluginCommand>,
    /// Registered hooks
    pub hooks: Vec<PluginHook>,
    /// Instructions text (from CLAUDE.md)
    #[serde(default)]
    pub instructions: Option<String>,
    /// Permission configuration
    #[serde(default)]
    pub permissions: PluginPermissions,
}

// ============================================================================
// Shell Result
// ============================================================================

/// Result from executing a shell hook command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellResult {
    /// Process exit code
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
}

impl ShellResult {
    /// Check if the hook execution was successful (exit code 0).
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }

    /// Check if the hook requested blocking the tool (exit code 2).
    /// Only meaningful for PreToolUse hooks.
    pub fn is_block(&self) -> bool {
        self.exit_code == 2
    }
}

// ============================================================================
// Frontend Response Types
// ============================================================================

/// Lightweight plugin info for listing in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Where it was loaded from
    pub source: PluginSource,
    /// Whether enabled
    pub enabled: bool,
    /// Number of skills
    pub skill_count: usize,
    /// Number of commands
    pub command_count: usize,
    /// Number of hooks
    pub hook_count: usize,
    /// Whether it has instructions
    pub has_instructions: bool,
    /// Author name
    pub author: Option<String>,
}

/// Detailed plugin info for the detail view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDetail {
    /// Full loaded plugin
    pub plugin: LoadedPlugin,
    /// Plugin root path
    pub root_path: String,
}

impl LoadedPlugin {
    /// Convert to a lightweight PluginInfo for listing.
    pub fn to_info(&self) -> PluginInfo {
        PluginInfo {
            name: self.manifest.name.clone(),
            version: self.manifest.version.clone(),
            description: self.manifest.description.clone(),
            source: self.source.clone(),
            enabled: self.enabled,
            skill_count: self.skills.len(),
            command_count: self.commands.len(),
            hook_count: self.hooks.len(),
            has_instructions: self.instructions.is_some(),
            author: self.manifest.author.clone(),
        }
    }
}

// ============================================================================
// Registry & Marketplace Types
// ============================================================================

/// A plugin entry in the remote registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Plugin name (unique identifier)
    pub name: String,
    /// Semantic version
    #[serde(default)]
    pub version: String,
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Author name
    #[serde(default)]
    pub author: Option<String>,
    /// Repository URL (informational)
    #[serde(default)]
    pub repository: Option<String>,
    /// License identifier
    #[serde(default)]
    pub license: Option<String>,
    /// Categorization keywords
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Category identifier
    #[serde(default)]
    pub category: Option<String>,
    /// Git URL for cloning
    pub git_url: String,
    /// GitHub stars count
    #[serde(default)]
    pub stars: u64,
    /// Download count
    #[serde(default)]
    pub downloads: u64,
}

/// A category in the plugin registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryCategory {
    /// Category identifier
    pub id: String,
    /// Human-readable label
    pub label: String,
}

/// The full plugin registry fetched from a remote URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRegistry {
    /// Registry format version
    pub version: String,
    /// Last updated timestamp
    pub updated_at: String,
    /// All available plugins
    pub plugins: Vec<RegistryEntry>,
    /// Available categories
    pub categories: Vec<RegistryCategory>,
}

/// A marketplace plugin enriched with local install/enable status.
///
/// Used by the frontend to display plugins from all marketplace sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacePlugin {
    /// Plugin name
    pub name: String,
    /// Semantic version
    #[serde(default)]
    pub version: String,
    /// Description
    #[serde(default)]
    pub description: String,
    /// Author name
    #[serde(default)]
    pub author: Option<String>,
    /// Repository URL
    #[serde(default)]
    pub repository: Option<String>,
    /// License
    #[serde(default)]
    pub license: Option<String>,
    /// Keywords
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Category
    #[serde(default)]
    pub category: Option<String>,
    /// Which marketplace this plugin came from
    pub marketplace_name: String,
    /// Serialized source spec for installation
    #[serde(default)]
    pub source_spec: String,
    /// Whether installed locally
    pub installed: bool,
    /// Whether currently enabled
    pub enabled: bool,
}

/// Marketplace info for UI listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceInfo {
    /// Marketplace name
    pub name: String,
    /// Human-readable source (e.g. "github:anthropics/claude-plugins-official")
    pub source_display: String,
    /// Whether enabled
    pub enabled: bool,
    /// Number of plugins in this marketplace
    pub plugin_count: usize,
    /// Description from manifest metadata
    pub description: Option<String>,
    /// Whether this is the official marketplace
    pub is_official: bool,
}

/// Progress update during plugin installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallProgress {
    /// Plugin being installed
    pub plugin_name: String,
    /// Current phase (cloning, validating, installing, complete)
    pub phase: String,
    /// Human-readable message
    pub message: String,
    /// Progress percentage (0.0 to 1.0)
    pub progress: f64,
}

/// Persistent plugin settings (enabled/disabled state + marketplace configs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSettings {
    /// List of plugin names that are disabled
    #[serde(default)]
    pub disabled_plugins: Vec<String>,
    /// Configured marketplace sources
    #[serde(default = "default_marketplaces")]
    pub marketplaces: Vec<MarketplaceConfig>,
}

impl Default for PluginSettings {
    fn default() -> Self {
        Self {
            disabled_plugins: vec![],
            marketplaces: default_marketplaces(),
        }
    }
}

/// Returns the default marketplace list (official Claude plugins marketplace).
pub fn default_marketplaces() -> Vec<MarketplaceConfig> {
    vec![MarketplaceConfig {
        name: "claude-plugins-official".to_string(),
        source: MarketplaceSourceType::Github {
            repo: "anthropics/claude-plugins-official".to_string(),
        },
        enabled: true,
    }]
}

// ============================================================================
// Marketplace Source Types
// ============================================================================

/// How to find a marketplace repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MarketplaceSourceType {
    /// GitHub repository shorthand (e.g. "anthropics/claude-plugins-official")
    Github { repo: String },
    /// Full git URL (e.g. "https://gitlab.com/org/plugins.git")
    GitUrl { url: String },
    /// Local filesystem path
    LocalPath { path: String },
}

impl MarketplaceSourceType {
    /// Human-readable display string for the source.
    pub fn display(&self) -> String {
        match self {
            Self::Github { repo } => format!("github:{}", repo),
            Self::GitUrl { url } => url.clone(),
            Self::LocalPath { path } => format!("local:{}", path),
        }
    }
}

/// A configured marketplace source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceConfig {
    /// Unique name for this marketplace
    pub name: String,
    /// Source location
    pub source: MarketplaceSourceType,
    /// Whether this marketplace is enabled
    pub enabled: bool,
}

// ============================================================================
// Marketplace Manifest (Claude Code marketplace.json format)
// ============================================================================

/// Parsed `.claude-plugin/marketplace.json` from a marketplace repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceManifest {
    /// Marketplace name
    pub name: String,
    /// Marketplace owner
    #[serde(default)]
    pub owner: Option<MarketplaceOwner>,
    /// Marketplace metadata
    #[serde(default)]
    pub metadata: Option<MarketplaceMetadata>,
    /// Plugin entries
    #[serde(default)]
    pub plugins: Vec<MarketplacePluginEntry>,
}

/// Marketplace owner info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceOwner {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
}

/// Marketplace metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceMetadata {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    /// Root directory for relative plugin paths
    #[serde(default)]
    pub plugin_root: Option<String>,
}

/// A plugin entry within a marketplace.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacePluginEntry {
    /// Plugin name
    pub name: String,
    /// Plugin source (flexible: string path or object with repo/url/etc.)
    #[serde(default)]
    pub source: Option<serde_json::Value>,
    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
    /// Semantic version
    #[serde(default)]
    pub version: Option<String>,
    /// Author (string or object)
    #[serde(default)]
    pub author: Option<serde_json::Value>,
    /// Category
    #[serde(default)]
    pub category: Option<String>,
    /// Keywords for search
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Plugin homepage URL
    #[serde(default)]
    pub homepage: Option<String>,
    /// Repository URL
    #[serde(default)]
    pub repository: Option<String>,
    /// License identifier
    #[serde(default)]
    pub license: Option<String>,
}

impl MarketplacePluginEntry {
    /// Extract author as a plain string (handles both string and object formats).
    pub fn author_string(&self) -> Option<String> {
        match &self.author {
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            Some(serde_json::Value::Object(map)) => map
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            _ => None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_source_display() {
        assert_eq!(PluginSource::ClaudeCode.to_string(), "claude-code");
        assert_eq!(PluginSource::Installed.to_string(), "installed");
        assert_eq!(PluginSource::ProjectLocal.to_string(), "project-local");
    }

    #[test]
    fn test_plugin_source_serialization() {
        let source = PluginSource::ClaudeCode;
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, "\"claude_code\"");

        let deserialized: PluginSource = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PluginSource::ClaudeCode);
    }

    #[test]
    fn test_plugin_manifest_default() {
        let manifest = PluginManifest::default();
        assert!(manifest.name.is_empty());
        assert_eq!(manifest.version, "0.0.0");
        assert!(manifest.description.is_empty());
        assert!(manifest.author.is_none());
        assert!(manifest.keywords.is_empty());
    }

    #[test]
    fn test_plugin_manifest_deserialize() {
        let json = r#"{
            "name": "my-plugin",
            "version": "1.0.0",
            "description": "A test plugin",
            "author": "test-author",
            "repository": "https://github.com/test/plugin",
            "license": "MIT",
            "keywords": ["testing", "lint"]
        }"#;

        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "my-plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, "A test plugin");
        assert_eq!(manifest.author.as_deref(), Some("test-author"));
        assert_eq!(manifest.license.as_deref(), Some("MIT"));
        assert_eq!(manifest.keywords, vec!["testing", "lint"]);
    }

    #[test]
    fn test_plugin_manifest_minimal_deserialize() {
        let json = r#"{"name": "minimal"}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "minimal");
        assert!(manifest.version.is_empty());
        assert!(manifest.description.is_empty());
    }

    #[test]
    fn test_hook_event_all_14_variants() {
        let all = HookEvent::all_variants();
        assert_eq!(all.len(), 14, "Should have exactly 14 hook event variants");
    }

    #[test]
    fn test_hook_event_from_str_loose() {
        assert_eq!(
            HookEvent::from_str_loose("PreToolUse"),
            Some(HookEvent::PreToolUse)
        );
        assert_eq!(
            HookEvent::from_str_loose("pre_tool_use"),
            Some(HookEvent::PreToolUse)
        );
        assert_eq!(
            HookEvent::from_str_loose("preToolUse"),
            Some(HookEvent::PreToolUse)
        );
        assert_eq!(
            HookEvent::from_str_loose("SessionStart"),
            Some(HookEvent::SessionStart)
        );
        assert_eq!(HookEvent::from_str_loose("Stop"), Some(HookEvent::Stop));
        assert_eq!(HookEvent::from_str_loose("invalid"), None);
    }

    #[test]
    fn test_hook_event_serialization() {
        let event = HookEvent::PreToolUse;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "\"PreToolUse\"");

        let deserialized: HookEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, HookEvent::PreToolUse);
    }

    #[test]
    fn test_hook_type_serialization() {
        let cmd = HookType::Command;
        let json = serde_json::to_string(&cmd).unwrap();
        assert_eq!(json, "\"command\"");

        let prompt = HookType::Prompt;
        let json = serde_json::to_string(&prompt).unwrap();
        assert_eq!(json, "\"prompt\"");
    }

    #[test]
    fn test_plugin_hook_default_timeout() {
        let json = r#"{
            "event": "PreToolUse",
            "hook_type": "command",
            "command": "echo hello"
        }"#;
        let hook: PluginHook = serde_json::from_str(json).unwrap();
        assert_eq!(hook.timeout, 10_000);
        assert!(!hook.async_hook);
        assert!(hook.matcher.is_none());
    }

    #[test]
    fn test_plugin_hook_with_matcher() {
        let json = r#"{
            "event": "PreToolUse",
            "matcher": "Bash|Write",
            "hook_type": "command",
            "command": "echo 'checking tool'",
            "timeout": 5000,
            "async_hook": true
        }"#;
        let hook: PluginHook = serde_json::from_str(json).unwrap();
        assert_eq!(hook.matcher.as_deref(), Some("Bash|Write"));
        assert_eq!(hook.timeout, 5000);
        assert!(hook.async_hook);
    }

    #[test]
    fn test_shell_result_success() {
        let result = ShellResult {
            exit_code: 0,
            stdout: "all good".to_string(),
            stderr: String::new(),
        };
        assert!(result.is_success());
        assert!(!result.is_block());
    }

    #[test]
    fn test_shell_result_block() {
        let result = ShellResult {
            exit_code: 2,
            stdout: String::new(),
            stderr: "blocked".to_string(),
        };
        assert!(!result.is_success());
        assert!(result.is_block());
    }

    #[test]
    fn test_shell_result_error() {
        let result = ShellResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: "error occurred".to_string(),
        };
        assert!(!result.is_success());
        assert!(!result.is_block());
    }

    #[test]
    fn test_plugin_permissions_default() {
        let perms = PluginPermissions::default();
        assert!(perms.allow.is_empty());
        assert!(perms.deny.is_empty());
        assert!(perms.always_approve.is_empty());
    }

    #[test]
    fn test_loaded_plugin_to_info() {
        let plugin = LoadedPlugin {
            manifest: PluginManifest {
                name: "test-plugin".to_string(),
                version: "1.0.0".to_string(),
                description: "Test".to_string(),
                author: Some("Author".to_string()),
                repository: None,
                license: None,
                keywords: vec![],
            },
            source: PluginSource::ClaudeCode,
            enabled: true,
            root_path: "/test".to_string(),
            skills: vec![PluginSkill {
                name: "skill1".to_string(),
                description: "A skill".to_string(),
                user_invocable: false,
                allowed_tools: vec![],
                body: "# Skill".to_string(),
                hooks: vec![],
            }],
            commands: vec![],
            hooks: vec![PluginHook {
                event: HookEvent::PreToolUse,
                matcher: None,
                hook_type: HookType::Command,
                command: "echo test".to_string(),
                timeout: 10_000,
                async_hook: false,
            }],
            instructions: Some("Do the thing".to_string()),
            permissions: PluginPermissions::default(),
        };

        let info = plugin.to_info();
        assert_eq!(info.name, "test-plugin");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.skill_count, 1);
        assert_eq!(info.command_count, 0);
        assert_eq!(info.hook_count, 1);
        assert!(info.has_instructions);
        assert!(info.enabled);
        assert_eq!(info.author.as_deref(), Some("Author"));
    }

    #[test]
    fn test_plugin_skill_serialization() {
        let skill = PluginSkill {
            name: "my-skill".to_string(),
            description: "Does things".to_string(),
            user_invocable: true,
            allowed_tools: vec!["Read".to_string(), "Write".to_string()],
            body: "# My Skill\n\nInstructions here.".to_string(),
            hooks: vec![],
        };

        let json = serde_json::to_string(&skill).unwrap();
        let deserialized: PluginSkill = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "my-skill");
        assert!(deserialized.user_invocable);
        assert_eq!(deserialized.allowed_tools.len(), 2);
    }

    #[test]
    fn test_plugin_command_serialization() {
        let cmd = PluginCommand {
            name: "lint-check".to_string(),
            description: "Run lint checks".to_string(),
            body: "# Lint Check\n\n1. Run eslint\n2. Fix errors".to_string(),
        };

        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: PluginCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "lint-check");
        assert!(deserialized.body.contains("eslint"));
    }

    #[test]
    fn test_plugin_info_serialization() {
        let info = PluginInfo {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "desc".to_string(),
            source: PluginSource::Installed,
            enabled: true,
            skill_count: 2,
            command_count: 1,
            hook_count: 3,
            has_instructions: false,
            author: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"installed\""));
        assert!(json.contains("\"skill_count\":2"));
    }

    #[test]
    fn test_plugin_manifest_author_string() {
        let json = r#"{"name": "test", "author": "John Doe"}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.author.as_deref(), Some("John Doe"));
    }

    #[test]
    fn test_plugin_manifest_author_object() {
        let json =
            r#"{"name": "test", "author": {"name": "John Doe", "url": "https://example.com"}}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.author.as_deref(), Some("John Doe"));
    }

    #[test]
    fn test_plugin_manifest_author_null() {
        let json = r#"{"name": "test", "author": null}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(manifest.author.is_none());
    }

    #[test]
    fn test_plugin_manifest_author_missing() {
        let json = r#"{"name": "test"}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(manifest.author.is_none());
    }
}
