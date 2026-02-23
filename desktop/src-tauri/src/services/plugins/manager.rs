//! Plugin Manager
//!
//! Unified entry point for the plugin system. Manages plugin lifecycle:
//! discovery, loading, hook registration, and runtime toggling.
//!
//! The PluginManager discovers plugins from 3 source locations in priority order:
//! 1. Project-level: `<project>/.claude-plugin/` (highest)
//! 2. User cache: `~/.claude/plugins/cache/`
//! 3. Plan Cascade: `~/.plan-cascade/plugins/` (lowest)

use std::path::{Path, PathBuf};

use crate::services::orchestrator::hooks::AgenticHooks;
use crate::services::plugins::dispatcher::register_plugin_hooks;
use crate::services::plugins::loader::{discover_all_plugins, discover_all_plugins_with_home};
use crate::services::plugins::models::*;
use crate::services::plugins::settings::{load_plugin_settings, save_plugin_settings};

/// Unified plugin manager.
///
/// Manages the full lifecycle of Claude Code-compatible plugins:
/// - Discovery and loading from 3 source locations
/// - Hook registration into AgenticHooks
/// - Skill and command collection for system prompt injection
/// - Runtime enable/disable toggling
#[derive(Debug)]
pub struct PluginManager {
    /// Project root path for plugin discovery
    project_root: PathBuf,
    /// Optional home directory override (None = use system default)
    home_override: Option<PathBuf>,
    /// All loaded plugins
    plugins: Vec<LoadedPlugin>,
}

impl PluginManager {
    /// Create a new PluginManager for the given project root.
    ///
    /// Does not automatically discover plugins - call `discover_and_load()` to populate.
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            home_override: None,
            plugins: Vec::new(),
        }
    }

    /// Create a new PluginManager with an explicit home directory.
    ///
    /// Used for testing to avoid reading the real user's installed plugins.
    #[cfg(test)]
    pub fn new_with_home(project_root: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            home_override: Some(home.into()),
            plugins: Vec::new(),
        }
    }

    /// Get the project root path.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Discover and load all plugins from the 3 source locations.
    ///
    /// This replaces any previously loaded plugins. Plugins are loaded
    /// in priority order (project > installed > plan-cascade), with
    /// higher-priority plugins overriding lower-priority ones by name.
    pub fn discover_and_load(&mut self) {
        self.plugins = match &self.home_override {
            Some(home) => discover_all_plugins_with_home(&self.project_root, Some(home.clone())),
            None => discover_all_plugins(&self.project_root),
        };

        // Apply persisted disabled state
        let settings = load_plugin_settings();
        for plugin in &mut self.plugins {
            if settings.disabled_plugins.contains(&plugin.manifest.name) {
                plugin.enabled = false;
            }
        }

        eprintln!(
            "[plugins] Discovered {} plugins for project {}",
            self.plugins.len(),
            self.project_root.display()
        );
        for plugin in &self.plugins {
            eprintln!(
                "[plugins]   - {} v{} ({}) [{}]",
                plugin.manifest.name,
                plugin.manifest.version,
                plugin.source,
                if plugin.enabled { "enabled" } else { "disabled" }
            );
        }
    }

    /// Refresh plugins by re-discovering and reloading.
    ///
    /// Preserves enabled/disabled state for plugins that still exist.
    pub fn refresh_plugins(&mut self) {
        // Save current in-memory enabled state
        let enabled_state: std::collections::HashMap<String, bool> = self
            .plugins
            .iter()
            .map(|p| (p.manifest.name.clone(), p.enabled))
            .collect();

        // Re-discover
        self.plugins = match &self.home_override {
            Some(home) => discover_all_plugins_with_home(&self.project_root, Some(home.clone())),
            None => discover_all_plugins(&self.project_root),
        };

        // Apply persisted settings first, then overlay in-memory state
        let settings = load_plugin_settings();
        for plugin in &mut self.plugins {
            if let Some(&was_enabled) = enabled_state.get(&plugin.manifest.name) {
                plugin.enabled = was_enabled;
            } else if settings.disabled_plugins.contains(&plugin.manifest.name) {
                plugin.enabled = false;
            }
        }

        eprintln!(
            "[plugins] Refreshed: {} plugins found",
            self.plugins.len()
        );
    }

    /// Register hooks from all enabled plugins into AgenticHooks.
    ///
    /// This should be called after `discover_and_load()` and whenever
    /// plugins are toggled. Creates hook closures that execute shell
    /// commands or LLM prompts when triggered.
    pub fn register_hooks(&self, hooks: &mut AgenticHooks) {
        for plugin in &self.plugins {
            if !plugin.enabled {
                continue;
            }
            if plugin.hooks.is_empty() {
                continue;
            }

            register_plugin_hooks(
                hooks,
                plugin.hooks.clone(),
                plugin.manifest.name.clone(),
                plugin.root_path.clone(),
            );

            eprintln!(
                "[plugins] Registered {} hooks from plugin '{}'",
                plugin.hooks.len(),
                plugin.manifest.name
            );
        }
    }

    /// Collect all skills from enabled plugins.
    ///
    /// Returns a flat list of all plugin skills, suitable for
    /// merging into the SkillIndex.
    pub fn collect_skills(&self) -> Vec<PluginSkill> {
        self.plugins
            .iter()
            .filter(|p| p.enabled)
            .flat_map(|p| p.skills.clone())
            .collect()
    }

    /// Collect all commands from enabled plugins.
    ///
    /// Returns a flat list of all plugin commands.
    pub fn collect_commands(&self) -> Vec<PluginCommand> {
        self.plugins
            .iter()
            .filter(|p| p.enabled)
            .flat_map(|p| p.commands.clone())
            .collect()
    }

    /// Collect all instructions from enabled plugins.
    ///
    /// Returns the concatenated CLAUDE.md contents from all enabled plugins.
    /// Each plugin's instructions are prefixed with a comment showing
    /// the plugin name.
    pub fn collect_instructions(&self) -> String {
        let mut instructions = Vec::new();

        for plugin in &self.plugins {
            if !plugin.enabled {
                continue;
            }
            if let Some(ref inst) = plugin.instructions {
                instructions.push(format!(
                    "<!-- Plugin: {} -->\n{}",
                    plugin.manifest.name, inst
                ));
            }
        }

        instructions.join("\n\n")
    }

    /// List all plugins with lightweight info summaries.
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins.iter().map(|p| p.to_info()).collect()
    }

    /// Get full details for a plugin by name.
    pub fn get_plugin(&self, name: &str) -> Option<&LoadedPlugin> {
        self.plugins.iter().find(|p| p.manifest.name == name)
    }

    /// Toggle a plugin's enabled state.
    ///
    /// Returns true if the plugin was found and toggled.
    pub fn toggle_plugin(&mut self, name: &str, enabled: bool) -> bool {
        if let Some(plugin) = self.plugins.iter_mut().find(|p| p.manifest.name == name) {
            plugin.enabled = enabled;
            eprintln!(
                "[plugins] Plugin '{}' {}",
                name,
                if enabled { "enabled" } else { "disabled" }
            );

            // Persist the toggle state
            let mut settings = load_plugin_settings();
            if enabled {
                settings.disabled_plugins.retain(|n| n != name);
            } else if !settings.disabled_plugins.contains(&name.to_string()) {
                settings.disabled_plugins.push(name.to_string());
            }
            if let Err(e) = save_plugin_settings(&settings) {
                eprintln!("[plugins] Failed to persist toggle state: {}", e);
            }

            true
        } else {
            false
        }
    }

    /// Get the number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get the number of enabled plugins.
    pub fn enabled_count(&self) -> usize {
        self.plugins.iter().filter(|p| p.enabled).count()
    }

    /// Check if a specific plugin is loaded and enabled.
    pub fn is_plugin_enabled(&self, name: &str) -> bool {
        self.plugins
            .iter()
            .any(|p| p.manifest.name == name && p.enabled)
    }
}

/// Register plugin hooks into AgenticHooks.
///
/// This is a convenience function for wiring plugin hooks into the
/// OrchestratorService's hook system alongside skill and memory hooks.
pub fn register_plugin_hooks_on_agentic_hooks(
    hooks: &mut AgenticHooks,
    manager: &PluginManager,
) {
    manager.register_hooks(hooks);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create a plugin directory with given properties.
    fn create_plugin(dir: &Path, name: &str, with_skills: bool, with_hooks: bool, with_instructions: bool) {
        fs::write(
            dir.join("plugin.json"),
            serde_json::json!({
                "name": name,
                "version": "1.0.0",
                "description": format!("Plugin {}", name),
                "author": "tester",
            })
            .to_string(),
        )
        .unwrap();

        if with_skills {
            let skills_dir = dir.join("skills").join("my-skill");
            fs::create_dir_all(&skills_dir).unwrap();
            fs::write(
                skills_dir.join("SKILL.md"),
                format!(
                    "---\nname: {}-skill\ndescription: A skill from {}\n---\n\n# Skill body\n",
                    name, name
                ),
            )
            .unwrap();
        }

        if with_hooks {
            let claude_dir = dir.join(".claude");
            fs::create_dir_all(&claude_dir).unwrap();
            fs::write(
                claude_dir.join("settings.json"),
                serde_json::json!({
                    "hooks": {
                        "PreToolUse": [
                            {"command": "echo hook", "type": "command"}
                        ]
                    }
                })
                .to_string(),
            )
            .unwrap();
        }

        if with_instructions {
            fs::write(
                dir.join("CLAUDE.md"),
                format!("# {} Instructions\n\nAlways be careful.\n", name),
            )
            .unwrap();
        }
    }

    #[test]
    fn test_plugin_manager_new() {
        let manager = PluginManager::new("/tmp/project");
        assert_eq!(manager.project_root(), Path::new("/tmp/project"));
        assert_eq!(manager.plugin_count(), 0);
        assert_eq!(manager.enabled_count(), 0);
    }

    #[test]
    fn test_plugin_manager_discover_from_project() {
        let dir = TempDir::new().unwrap();
        let project_root = dir.path();

        // Create .claude-plugin/ with a plugin
        let plugin_dir = project_root.join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "project-plugin", true, true, true);

        let mut manager = PluginManager::new_with_home(project_root, dir.path());
        manager.discover_and_load();

        assert_eq!(manager.plugin_count(), 1);
        assert_eq!(manager.enabled_count(), 1);
        assert!(manager.is_plugin_enabled("project-plugin"));
    }

    #[test]
    fn test_plugin_manager_toggle() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "toggle-test", false, false, false);

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();

        assert!(manager.is_plugin_enabled("toggle-test"));

        // Disable
        assert!(manager.toggle_plugin("toggle-test", false));
        assert!(!manager.is_plugin_enabled("toggle-test"));
        assert_eq!(manager.enabled_count(), 0);

        // Re-enable
        assert!(manager.toggle_plugin("toggle-test", true));
        assert!(manager.is_plugin_enabled("toggle-test"));
        assert_eq!(manager.enabled_count(), 1);

        // Toggle non-existent
        assert!(!manager.toggle_plugin("nonexistent", true));
    }

    #[test]
    fn test_plugin_manager_collect_skills() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "skill-plugin", true, false, false);

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();

        let skills = manager.collect_skills();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "skill-plugin-skill");
    }

    #[test]
    fn test_plugin_manager_collect_skills_disabled() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "disabled-plugin", true, false, false);

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();
        manager.toggle_plugin("disabled-plugin", false);

        let skills = manager.collect_skills();
        assert!(skills.is_empty(), "Disabled plugins should not contribute skills");
    }

    #[test]
    fn test_plugin_manager_collect_commands() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "cmd-plugin", false, false, false);

        // Add a command
        let commands_dir = plugin_dir.join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(
            commands_dir.join("deploy.md"),
            "# Deploy\n\nDeploy instructions.\n",
        )
        .unwrap();

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();

        let commands = manager.collect_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "deploy");
    }

    #[test]
    fn test_plugin_manager_collect_instructions() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "inst-plugin", false, false, true);

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();

        let instructions = manager.collect_instructions();
        assert!(instructions.contains("inst-plugin Instructions"));
        assert!(instructions.contains("<!-- Plugin: inst-plugin -->"));
    }

    #[test]
    fn test_plugin_manager_collect_instructions_disabled() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "disabled-inst", false, false, true);

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();
        manager.toggle_plugin("disabled-inst", false);

        let instructions = manager.collect_instructions();
        assert!(instructions.is_empty(), "Disabled plugin instructions should not be collected");
    }

    #[test]
    fn test_plugin_manager_register_hooks() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "hook-plugin", false, true, false);

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();

        let mut hooks = AgenticHooks::new();
        manager.register_hooks(&mut hooks);

        // We registered 1 PreToolUse hook
        assert_eq!(hooks.total_hooks(), 1);
    }

    #[test]
    fn test_plugin_manager_register_hooks_disabled_skipped() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "disabled-hook", false, true, false);

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();
        manager.toggle_plugin("disabled-hook", false);

        let mut hooks = AgenticHooks::new();
        manager.register_hooks(&mut hooks);

        assert_eq!(hooks.total_hooks(), 0, "Disabled plugin hooks should not be registered");
    }

    #[test]
    fn test_plugin_manager_get_plugin() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "detail-plugin", true, true, true);

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();

        let plugin = manager.get_plugin("detail-plugin");
        assert!(plugin.is_some());
        let p = plugin.unwrap();
        assert_eq!(p.manifest.name, "detail-plugin");
        assert!(!p.skills.is_empty());
        assert!(!p.hooks.is_empty());
        assert!(p.instructions.is_some());

        // Non-existent
        assert!(manager.get_plugin("nonexistent").is_none());
    }

    #[test]
    fn test_plugin_manager_refresh_preserves_state() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "refresh-plugin", false, false, false);

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();

        // Disable the plugin
        manager.toggle_plugin("refresh-plugin", false);
        assert!(!manager.is_plugin_enabled("refresh-plugin"));

        // Refresh
        manager.refresh_plugins();

        // State should be preserved
        assert!(!manager.is_plugin_enabled("refresh-plugin"));
        assert_eq!(manager.plugin_count(), 1);
    }

    #[test]
    fn test_plugin_manager_empty_project() {
        let dir = TempDir::new().unwrap();
        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();

        assert_eq!(manager.plugin_count(), 0);
        assert!(manager.list_plugins().is_empty());
        assert!(manager.collect_skills().is_empty());
        assert!(manager.collect_commands().is_empty());
        assert!(manager.collect_instructions().is_empty());
    }

    #[test]
    fn test_register_plugin_hooks_on_agentic_hooks_convenience() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_plugin(&plugin_dir, "convenience-plugin", false, true, false);

        let mut manager = PluginManager::new_with_home(dir.path(), dir.path());
        manager.discover_and_load();

        let mut hooks = AgenticHooks::new();
        register_plugin_hooks_on_agentic_hooks(&mut hooks, &manager);

        assert_eq!(hooks.total_hooks(), 1);
    }
}
