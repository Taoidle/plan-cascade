//! Plugin Loader
//!
//! Discovery and parsing of Claude Code-compatible plugins from 3 source locations:
//! 1. Project-level: `<project>/.claude-plugin/`
//! 2. User cache: `~/.claude/plugins/cache/`
//! 3. Plan Cascade: `~/.plan-cascade/plugins/`
//!
//! A plugin directory is expected to contain:
//! - `plugin.json` (manifest, required for a directory to be recognized as a plugin)
//! - `skills/*/SKILL.md` (optional skills)
//! - `commands/*.md` (optional commands)
//! - `.claude/settings.json` (optional hooks and permissions)
//! - `hooks/hooks.json` (optional independent hooks)
//! - `CLAUDE.md` (optional instructions)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::services::plugins::models::*;
use crate::utils::error::{AppError, AppResult};

// ============================================================================
// Public API
// ============================================================================

/// Load a single plugin from a directory containing plugin.json.
///
/// The directory must contain a `plugin.json` file. All other files are optional:
/// - `skills/*/SKILL.md` - Skills
/// - `commands/*.md` - Commands
/// - `.claude/settings.json` - Hooks and permissions
/// - `hooks/hooks.json` - Independent hooks
/// - `CLAUDE.md` - Instructions
pub fn load_plugin_from_dir(dir: &Path, source: PluginSource) -> AppResult<LoadedPlugin> {
    let manifest_path = dir.join("plugin.json");
    if !manifest_path.exists() {
        return Err(AppError::not_found(format!(
            "plugin.json not found in {}",
            dir.display()
        )));
    }

    // Read and parse manifest
    let manifest_content = std::fs::read_to_string(&manifest_path)?;
    let manifest: PluginManifest = serde_json::from_str(&manifest_content).map_err(|e| {
        AppError::parse(format!(
            "Invalid plugin.json at {}: {}",
            manifest_path.display(),
            e
        ))
    })?;

    // Discover skills
    let skills = discover_skills(dir);

    // Discover commands
    let commands = discover_commands(dir);

    // Load hooks from .claude/settings.json and hooks/hooks.json
    let mut hooks = Vec::new();
    let settings_path = dir.join(".claude").join("settings.json");
    if settings_path.exists() {
        if let Ok(h) = load_hooks_from_settings(&settings_path) {
            hooks.extend(h);
        }
    }
    let hooks_json_path = dir.join("hooks").join("hooks.json");
    if hooks_json_path.exists() {
        if let Ok(h) = load_hooks_from_hooks_json(&hooks_json_path) {
            hooks.extend(h);
        }
    }

    // Load instructions from CLAUDE.md
    let instructions = load_instructions(dir);

    // Load permissions from .claude/settings.json
    let permissions = if settings_path.exists() {
        load_permissions_from_settings(&settings_path).unwrap_or_default()
    } else {
        PluginPermissions::default()
    };

    Ok(LoadedPlugin {
        manifest,
        source,
        enabled: true,
        root_path: dir.to_string_lossy().to_string(),
        skills,
        commands,
        hooks,
        instructions,
        permissions,
    })
}

/// Discover plugin directories within a parent directory.
///
/// Each subdirectory that contains a `plugin.json` is considered a plugin.
/// Returns paths to the plugin directories.
pub fn discover_plugin_dirs(parent: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if !parent.exists() || !parent.is_dir() {
        return dirs;
    }

    // Check if the parent itself is a plugin directory
    if parent.join("plugin.json").exists() {
        dirs.push(parent.to_path_buf());
        return dirs;
    }

    // Scan subdirectories
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("plugin.json").exists() {
                dirs.push(path);
            }
        }
    }

    dirs
}

/// Load a plugin from a Claude Code cache install path.
///
/// In the Claude Code cache, the directory structure is:
/// ```text
/// <install_path>/              (version root, e.g. ~/.claude/plugins/cache/org/name/1.0.0/)
/// ├── .claude-plugin/
/// │   └── plugin.json          ← manifest lives here
/// ├── skills/                  ← skills at version root level
/// ├── commands/                ← commands at version root level
/// ├── CLAUDE.md
/// └── .claude/
///     └── settings.json
/// ```
///
/// The `install_path` is the version root directory. The manifest is read from
/// `.claude-plugin/plugin.json`, while skills, commands, hooks, and instructions
/// are discovered from the version root.
pub fn load_plugin_from_install_path(
    install_path: &Path,
    source: PluginSource,
) -> AppResult<LoadedPlugin> {
    let manifest_path = install_path.join(".claude-plugin").join("plugin.json");
    if !manifest_path.exists() {
        return Err(AppError::not_found(format!(
            ".claude-plugin/plugin.json not found in {}",
            install_path.display()
        )));
    }

    // Read and parse manifest
    let manifest_content = std::fs::read_to_string(&manifest_path)?;
    let manifest: PluginManifest = serde_json::from_str(&manifest_content).map_err(|e| {
        AppError::parse(format!(
            "Invalid plugin.json at {}: {}",
            manifest_path.display(),
            e
        ))
    })?;

    // Discover skills, commands, hooks, instructions from the version root
    let skills = discover_skills(install_path);
    let commands = discover_commands(install_path);

    let mut hooks = Vec::new();
    let settings_path = install_path.join(".claude").join("settings.json");
    if settings_path.exists() {
        if let Ok(h) = load_hooks_from_settings(&settings_path) {
            hooks.extend(h);
        }
    }
    let hooks_json_path = install_path.join("hooks").join("hooks.json");
    if hooks_json_path.exists() {
        if let Ok(h) = load_hooks_from_hooks_json(&hooks_json_path) {
            hooks.extend(h);
        }
    }

    let instructions = load_instructions(install_path);

    let permissions = if settings_path.exists() {
        load_permissions_from_settings(&settings_path).unwrap_or_default()
    } else {
        PluginPermissions::default()
    };

    Ok(LoadedPlugin {
        manifest,
        source,
        enabled: true,
        root_path: install_path.to_string_lossy().to_string(),
        skills,
        commands,
        hooks,
        instructions,
        permissions,
    })
}

/// Discover installed plugins by reading `~/.claude/plugins/installed_plugins.json`.
///
/// This file is maintained by Claude Code with the structure:
/// ```json
/// {
///   "version": 2,
///   "plugins": {
///     "plan-cascade@plan-cascade": [
///       {
///         "scope": "user",
///         "installPath": "/Users/.../.claude/plugins/cache/org/name/1.0.0",
///         "version": "4.4.0",
///         ...
///       }
///     ]
///   }
/// }
/// ```
///
/// For each plugin entry, we use the first (latest) install record's `installPath`
/// and look for `.claude-plugin/plugin.json` inside it.
pub fn discover_installed_plugins() -> Vec<LoadedPlugin> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };
    let installed_json_path = home
        .join(".claude")
        .join("plugins")
        .join("installed_plugins.json");
    discover_installed_plugins_from(&installed_json_path)
}

/// Internal helper that reads from a specific installed_plugins.json path.
/// Separated from `discover_installed_plugins` for testability.
fn discover_installed_plugins_from(installed_json_path: &Path) -> Vec<LoadedPlugin> {
    let mut plugins = Vec::new();
    if !installed_json_path.exists() {
        return plugins;
    }

    let content = match std::fs::read_to_string(&installed_json_path) {
        Ok(c) => c,
        Err(_) => return plugins,
    };

    let root: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return plugins,
    };

    // Extract the "plugins" object: { "<key>": [ { "installPath": "..." }, ... ] }
    let plugins_obj = match root.get("plugins").and_then(|v| v.as_object()) {
        Some(obj) => obj,
        None => return plugins,
    };

    for (_key, entries) in plugins_obj {
        let entries_arr = match entries.as_array() {
            Some(arr) => arr,
            None => continue,
        };

        // Use the first (latest) install entry
        let entry = match entries_arr.first() {
            Some(e) => e,
            None => continue,
        };

        let install_path_str = match entry.get("installPath").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => continue,
        };

        let install_path = PathBuf::from(install_path_str);
        if !install_path.exists() {
            continue;
        }

        if let Ok(plugin) = load_plugin_from_install_path(&install_path, PluginSource::Installed) {
            plugins.push(plugin);
        }
    }

    plugins
}

/// Discover and load all plugins from the three source locations.
///
/// Priority order (last loaded wins for name conflicts):
/// 1. `~/.plan-cascade/plugins/` (lowest)
/// 2. `~/.claude/plugins/cache/` via `installed_plugins.json` (medium)
/// 3. `<project>/.claude-plugin/` (highest)
pub fn discover_all_plugins(project_root: &Path) -> Vec<LoadedPlugin> {
    discover_all_plugins_with_home(project_root, dirs::home_dir())
}

/// Internal implementation that accepts an explicit home dir for testability.
pub(crate) fn discover_all_plugins_with_home(
    project_root: &Path,
    home: Option<PathBuf>,
) -> Vec<LoadedPlugin> {
    let mut plugins_by_name: HashMap<String, LoadedPlugin> = HashMap::new();

    // Source 3 (lowest priority): ~/.plan-cascade/plugins/
    if let Some(ref home) = home {
        let plan_cascade_plugins = home.join(".plan-cascade").join("plugins");
        for dir in discover_plugin_dirs(&plan_cascade_plugins) {
            if let Ok(plugin) = load_plugin_from_dir(&dir, PluginSource::ProjectLocal) {
                plugins_by_name.insert(plugin.manifest.name.clone(), plugin);
            }
        }
    }

    // Source 2 (medium priority): ~/.claude/plugins/ via installed_plugins.json
    if let Some(ref home) = home {
        let installed_json = home
            .join(".claude")
            .join("plugins")
            .join("installed_plugins.json");
        for plugin in discover_installed_plugins_from(&installed_json) {
            plugins_by_name.insert(plugin.manifest.name.clone(), plugin);
        }
    }

    // Source 1 (highest priority): <project>/.claude-plugin/
    let project_plugin = project_root.join(".claude-plugin");
    for dir in discover_plugin_dirs(&project_plugin) {
        if let Ok(plugin) = load_plugin_from_dir(&dir, PluginSource::ClaudeCode) {
            plugins_by_name.insert(plugin.manifest.name.clone(), plugin);
        }
    }

    plugins_by_name.into_values().collect()
}

// ============================================================================
// Internal Helpers
// ============================================================================

/// Discover skills from skills/*/SKILL.md files within a plugin directory.
fn discover_skills(plugin_dir: &Path) -> Vec<PluginSkill> {
    let skills_dir = plugin_dir.join("skills");
    if !skills_dir.exists() || !skills_dir.is_dir() {
        return vec![];
    }

    let mut skills = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    if let Ok(content) = std::fs::read_to_string(&skill_file) {
                        if let Ok(parsed) =
                            crate::services::skills::parser::parse_skill_file(&skill_file, &content)
                        {
                            skills.push(PluginSkill {
                                name: parsed.name,
                                description: parsed.description,
                                user_invocable: parsed.user_invocable,
                                allowed_tools: parsed.allowed_tools,
                                body: parsed.body,
                                hooks: vec![],
                            });
                        }
                    }
                }
            }
        }
    }

    // Also check for flat .md files in skills/ (skills/foo.md)
    if let Ok(entries) = std::fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "md") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(parsed) =
                        crate::services::skills::parser::parse_skill_file(&path, &content)
                    {
                        skills.push(PluginSkill {
                            name: parsed.name,
                            description: parsed.description,
                            user_invocable: parsed.user_invocable,
                            allowed_tools: parsed.allowed_tools,
                            body: parsed.body,
                            hooks: vec![],
                        });
                    }
                }
            }
        }
    }

    skills
}

/// Discover commands from commands/*.md files within a plugin directory.
fn discover_commands(plugin_dir: &Path) -> Vec<PluginCommand> {
    let commands_dir = plugin_dir.join("commands");
    if !commands_dir.exists() || !commands_dir.is_dir() {
        return vec![];
    }

    let mut commands = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&commands_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "md") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    // Extract description from first heading or first non-empty line
                    let description = content
                        .lines()
                        .find(|line| !line.trim().is_empty())
                        .map(|line| {
                            let trimmed = line.trim();
                            if trimmed.starts_with('#') {
                                trimmed.trim_start_matches('#').trim().to_string()
                            } else {
                                trimmed.to_string()
                            }
                        })
                        .unwrap_or_else(|| format!("Command: {}", name));

                    commands.push(PluginCommand {
                        name,
                        description,
                        body: content,
                    });
                }
            }
        }
    }

    commands
}

/// Load hooks from a .claude/settings.json file.
///
/// Claude Code settings.json hooks format:
/// ```json
/// {
///   "hooks": {
///     "PreToolUse": [
///       { "matcher": "Bash", "command": "echo check", "type": "command" },
///       { "command": "echo always", "type": "command" }
///     ],
///     "PostToolUse": [...],
///     "Stop": [...]
///   }
/// }
/// ```
pub fn load_hooks_from_settings(path: &Path) -> AppResult<Vec<PluginHook>> {
    let content = std::fs::read_to_string(path)?;
    let settings: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::parse(format!("Invalid settings.json: {}", e)))?;

    parse_hooks_from_value(&settings)
}

/// Load hooks from a hooks/hooks.json file (same format as settings.json hooks section).
fn load_hooks_from_hooks_json(path: &Path) -> AppResult<Vec<PluginHook>> {
    let content = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::parse(format!("Invalid hooks.json: {}", e)))?;

    // hooks.json may be either { "hooks": { ... } } or directly { "PreToolUse": [...] }
    if value.get("hooks").is_some() {
        parse_hooks_from_value(&value)
    } else {
        // Treat the root as the hooks object
        let wrapper = serde_json::json!({ "hooks": value });
        parse_hooks_from_value(&wrapper)
    }
}

/// Parse hooks from a JSON value containing a "hooks" key.
fn parse_hooks_from_value(value: &serde_json::Value) -> AppResult<Vec<PluginHook>> {
    let mut hooks = Vec::new();

    let hooks_obj = match value.get("hooks") {
        Some(h) if h.is_object() => h,
        _ => return Ok(hooks),
    };

    if let Some(obj) = hooks_obj.as_object() {
        for (event_name, entries) in obj {
            let event = match HookEvent::from_str_loose(event_name) {
                Some(e) => e,
                None => continue, // Skip unknown events
            };

            if let Some(arr) = entries.as_array() {
                for entry in arr {
                    let command = entry
                        .get("command")
                        .or_else(|| entry.get("prompt"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if command.is_empty() {
                        continue;
                    }

                    let hook_type_str = entry
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("command");

                    let hook_type = match hook_type_str {
                        "prompt" => HookType::Prompt,
                        _ => HookType::Command,
                    };

                    let matcher = entry
                        .get("matcher")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let timeout = entry
                        .get("timeout")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10_000);

                    let async_hook = entry
                        .get("async")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    hooks.push(PluginHook {
                        event: event.clone(),
                        matcher,
                        hook_type,
                        command,
                        timeout,
                        async_hook,
                    });
                }
            }
        }
    }

    Ok(hooks)
}

/// Load instructions from CLAUDE.md in the plugin directory.
fn load_instructions(plugin_dir: &Path) -> Option<String> {
    let claude_md = plugin_dir.join("CLAUDE.md");
    if claude_md.exists() {
        std::fs::read_to_string(&claude_md).ok()
    } else {
        None
    }
}

/// Load permissions from .claude/settings.json.
///
/// Expected format:
/// ```json
/// {
///   "permissions": {
///     "allow": ["Read", "Write"],
///     "deny": ["Bash"],
///     "always_approve": ["Grep"]
///   }
/// }
/// ```
fn load_permissions_from_settings(path: &Path) -> AppResult<PluginPermissions> {
    let content = std::fs::read_to_string(path)?;
    let settings: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::parse(format!("Invalid settings.json: {}", e)))?;

    let perms_val = match settings.get("permissions") {
        Some(p) => p,
        None => return Ok(PluginPermissions::default()),
    };

    fn extract_string_array(val: &serde_json::Value, key: &str) -> Vec<String> {
        val.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    Ok(PluginPermissions {
        allow: extract_string_array(perms_val, "allow"),
        deny: extract_string_array(perms_val, "deny"),
        always_approve: extract_string_array(perms_val, "always_approve"),
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create a minimal plugin directory structure for testing.
    fn create_test_plugin(dir: &Path, name: &str) {
        let plugin_dir = dir.join(name);
        fs::create_dir_all(&plugin_dir).unwrap();

        // plugin.json
        let manifest = serde_json::json!({
            "name": name,
            "version": "1.0.0",
            "description": format!("Test plugin: {}", name),
            "author": "tester",
            "keywords": ["test"]
        });
        fs::write(
            plugin_dir.join("plugin.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    /// Create a full plugin directory with all optional files.
    fn create_full_test_plugin(dir: &Path) {
        // plugin.json
        let manifest = serde_json::json!({
            "name": "full-plugin",
            "version": "2.0.0",
            "description": "A full test plugin",
            "author": "tester",
            "license": "MIT",
            "keywords": ["test", "full"]
        });
        fs::write(
            dir.join("plugin.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        // skills/lint-check/SKILL.md
        let skills_dir = dir.join("skills").join("lint-check");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("SKILL.md"),
            "---\nname: lint-check\ndescription: Run lint checks on code\nuser-invocable: true\nallowed-tools:\n  - Bash\n---\n\n# Lint Check\n\nRun eslint and fix errors.\n",
        ).unwrap();

        // commands/deploy.md
        let commands_dir = dir.join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(
            commands_dir.join("deploy.md"),
            "# Deploy\n\nDeploy the application to staging.\n\n1. Build\n2. Push\n3. Verify\n",
        )
        .unwrap();

        // .claude/settings.json with hooks and permissions
        let claude_dir = dir.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "command": "echo 'checking bash'",
                        "type": "command"
                    }
                ],
                "PostToolUse": [
                    {
                        "command": "echo 'tool done'",
                        "type": "command",
                        "async": true
                    }
                ],
                "Stop": [
                    {
                        "command": "echo 'session ending'",
                        "type": "command"
                    }
                ]
            },
            "permissions": {
                "allow": ["Read", "Write", "Bash"],
                "deny": ["rm -rf"],
                "always_approve": ["Grep"]
            }
        });
        fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        // CLAUDE.md
        fs::write(
            dir.join("CLAUDE.md"),
            "# Plugin Instructions\n\nAlways run lint before committing.\n",
        )
        .unwrap();
    }

    #[test]
    fn test_load_plugin_missing_manifest() {
        let dir = TempDir::new().unwrap();
        let result = load_plugin_from_dir(dir.path(), PluginSource::ClaudeCode);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("plugin.json not found"));
    }

    #[test]
    fn test_load_plugin_minimal() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path();

        // Create minimal plugin.json
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "minimal-plugin", "version": "0.1.0"}"#,
        )
        .unwrap();

        let plugin = load_plugin_from_dir(plugin_dir, PluginSource::ClaudeCode).unwrap();
        assert_eq!(plugin.manifest.name, "minimal-plugin");
        assert_eq!(plugin.manifest.version, "0.1.0");
        assert_eq!(plugin.source, PluginSource::ClaudeCode);
        assert!(plugin.enabled);
        assert!(plugin.skills.is_empty());
        assert!(plugin.commands.is_empty());
        assert!(plugin.hooks.is_empty());
        assert!(plugin.instructions.is_none());
    }

    #[test]
    fn test_load_plugin_full() {
        let dir = TempDir::new().unwrap();
        create_full_test_plugin(dir.path());

        let plugin = load_plugin_from_dir(dir.path(), PluginSource::Installed).unwrap();
        assert_eq!(plugin.manifest.name, "full-plugin");
        assert_eq!(plugin.manifest.version, "2.0.0");
        assert_eq!(plugin.source, PluginSource::Installed);

        // Skills
        assert_eq!(plugin.skills.len(), 1);
        assert_eq!(plugin.skills[0].name, "lint-check");
        assert!(plugin.skills[0].user_invocable);
        assert_eq!(plugin.skills[0].allowed_tools, vec!["Bash"]);

        // Commands
        assert_eq!(plugin.commands.len(), 1);
        assert_eq!(plugin.commands[0].name, "deploy");
        assert!(plugin.commands[0].body.contains("Deploy the application"));

        // Hooks
        assert_eq!(plugin.hooks.len(), 3);
        // Find the PreToolUse hook
        let pre_tool = plugin
            .hooks
            .iter()
            .find(|h| h.event == HookEvent::PreToolUse)
            .unwrap();
        assert_eq!(pre_tool.matcher.as_deref(), Some("Bash"));
        assert_eq!(pre_tool.command, "echo 'checking bash'");
        // Check async hook
        let post_tool = plugin
            .hooks
            .iter()
            .find(|h| h.event == HookEvent::PostToolUse)
            .unwrap();
        assert!(post_tool.async_hook);

        // Instructions
        assert!(plugin.instructions.is_some());
        assert!(plugin
            .instructions
            .as_ref()
            .unwrap()
            .contains("lint before committing"));

        // Permissions
        assert_eq!(plugin.permissions.allow, vec!["Read", "Write", "Bash"]);
        assert_eq!(plugin.permissions.deny, vec!["rm -rf"]);
        assert_eq!(plugin.permissions.always_approve, vec!["Grep"]);
    }

    #[test]
    fn test_discover_plugin_dirs_empty() {
        let dir = TempDir::new().unwrap();
        let dirs = discover_plugin_dirs(dir.path());
        assert!(dirs.is_empty());
    }

    #[test]
    fn test_discover_plugin_dirs_with_plugins() {
        let dir = TempDir::new().unwrap();
        create_test_plugin(dir.path(), "plugin-a");
        create_test_plugin(dir.path(), "plugin-b");

        // Also create a non-plugin directory
        fs::create_dir(dir.path().join("not-a-plugin")).unwrap();

        let dirs = discover_plugin_dirs(dir.path());
        assert_eq!(dirs.len(), 2);
    }

    #[test]
    fn test_discover_plugin_dirs_self_is_plugin() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("plugin.json"), r#"{"name": "self-plugin"}"#).unwrap();

        let dirs = discover_plugin_dirs(dir.path());
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0], dir.path());
    }

    #[test]
    fn test_discover_plugin_dirs_nonexistent() {
        let dirs = discover_plugin_dirs(Path::new("/nonexistent/path"));
        assert!(dirs.is_empty());
    }

    #[test]
    fn test_load_hooks_from_settings() {
        let dir = TempDir::new().unwrap();
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {"matcher": "Write|Edit", "command": "npm run lint", "type": "command", "timeout": 5000},
                    {"command": "echo checking", "type": "command"}
                ],
                "SessionStart": [
                    {"command": "echo starting", "type": "command"}
                ],
                "Stop": [
                    {"command": "echo stopping", "type": "command", "async": true}
                ]
            }
        });
        let path = dir.path().join("settings.json");
        fs::write(&path, serde_json::to_string(&settings).unwrap()).unwrap();

        let hooks = load_hooks_from_settings(&path).unwrap();
        assert_eq!(hooks.len(), 4);

        let pre_tool_hooks: Vec<_> = hooks
            .iter()
            .filter(|h| h.event == HookEvent::PreToolUse)
            .collect();
        assert_eq!(pre_tool_hooks.len(), 2);
        assert_eq!(pre_tool_hooks[0].matcher.as_deref(), Some("Write|Edit"));
        assert_eq!(pre_tool_hooks[0].timeout, 5000);

        let stop_hooks: Vec<_> = hooks
            .iter()
            .filter(|h| h.event == HookEvent::Stop)
            .collect();
        assert_eq!(stop_hooks.len(), 1);
        assert!(stop_hooks[0].async_hook);
    }

    #[test]
    fn test_load_hooks_from_settings_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{}").unwrap();

        let hooks = load_hooks_from_settings(&path).unwrap();
        assert!(hooks.is_empty());
    }

    #[test]
    fn test_load_hooks_from_settings_unknown_event_skipped() {
        let dir = TempDir::new().unwrap();
        let settings = serde_json::json!({
            "hooks": {
                "UnknownEvent": [
                    {"command": "echo test", "type": "command"}
                ],
                "PreToolUse": [
                    {"command": "echo valid", "type": "command"}
                ]
            }
        });
        let path = dir.path().join("settings.json");
        fs::write(&path, serde_json::to_string(&settings).unwrap()).unwrap();

        let hooks = load_hooks_from_settings(&path).unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].event, HookEvent::PreToolUse);
    }

    #[test]
    fn test_load_hooks_prompt_type() {
        let dir = TempDir::new().unwrap();
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {"command": "Is this tool use safe?", "type": "prompt"}
                ]
            }
        });
        let path = dir.path().join("settings.json");
        fs::write(&path, serde_json::to_string(&settings).unwrap()).unwrap();

        let hooks = load_hooks_from_settings(&path).unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].hook_type, HookType::Prompt);
    }

    #[test]
    fn test_discover_commands() {
        let dir = TempDir::new().unwrap();
        let commands_dir = dir.path().join("commands");
        fs::create_dir(&commands_dir).unwrap();

        fs::write(
            commands_dir.join("build.md"),
            "# Build\n\nRun the build process.\n",
        )
        .unwrap();
        fs::write(commands_dir.join("test.md"), "# Test\n\nRun all tests.\n").unwrap();
        // Non-md file should be ignored
        fs::write(commands_dir.join("readme.txt"), "Not a command").unwrap();

        let commands = discover_commands(dir.path());
        assert_eq!(commands.len(), 2);

        let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"build"));
        assert!(names.contains(&"test"));
    }

    #[test]
    fn test_discover_skills_with_subdirs() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join("skills");
        let skill_subdir = skills_dir.join("my-skill");
        fs::create_dir_all(&skill_subdir).unwrap();

        fs::write(
            skill_subdir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A test skill\n---\n\n# My Skill\n\nDo stuff.\n",
        )
        .unwrap();

        let skills = discover_skills(dir.path());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
        assert_eq!(skills[0].description, "A test skill");
    }

    #[test]
    fn test_load_permissions() {
        let dir = TempDir::new().unwrap();
        let settings = serde_json::json!({
            "permissions": {
                "allow": ["Read", "Write"],
                "deny": ["Bash"],
                "always_approve": ["Grep", "Glob"]
            }
        });
        let path = dir.path().join("settings.json");
        fs::write(&path, serde_json::to_string(&settings).unwrap()).unwrap();

        let perms = load_permissions_from_settings(&path).unwrap();
        assert_eq!(perms.allow, vec!["Read", "Write"]);
        assert_eq!(perms.deny, vec!["Bash"]);
        assert_eq!(perms.always_approve, vec!["Grep", "Glob"]);
    }

    #[test]
    fn test_load_permissions_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{}").unwrap();

        let perms = load_permissions_from_settings(&path).unwrap();
        assert!(perms.allow.is_empty());
        assert!(perms.deny.is_empty());
    }

    #[test]
    fn test_load_instructions() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("CLAUDE.md"),
            "# Instructions\n\nDo the thing.\n",
        )
        .unwrap();

        let instructions = load_instructions(dir.path());
        assert!(instructions.is_some());
        assert!(instructions.unwrap().contains("Do the thing"));
    }

    #[test]
    fn test_load_instructions_missing() {
        let dir = TempDir::new().unwrap();
        let instructions = load_instructions(dir.path());
        assert!(instructions.is_none());
    }

    #[test]
    fn test_discover_all_plugins_from_project() {
        let dir = TempDir::new().unwrap();
        let project_root = dir.path();

        // Create .claude-plugin/ with a plugin
        let claude_plugin = project_root.join(".claude-plugin");
        fs::create_dir_all(&claude_plugin).unwrap();
        fs::write(
            claude_plugin.join("plugin.json"),
            r#"{"name": "project-plugin", "version": "1.0.0", "description": "Project plugin"}"#,
        )
        .unwrap();

        // Use temp dir as home to avoid picking up real user plugins
        let plugins = discover_all_plugins_with_home(project_root, Some(dir.path().to_path_buf()));
        assert!(!plugins.is_empty());

        let project_plugin = plugins.iter().find(|p| p.manifest.name == "project-plugin");
        assert!(project_plugin.is_some());
        assert_eq!(project_plugin.unwrap().source, PluginSource::ClaudeCode);
    }

    #[test]
    fn test_missing_optional_files_no_error() {
        let dir = TempDir::new().unwrap();
        // Only plugin.json, no skills, commands, hooks, or instructions
        fs::write(
            dir.path().join("plugin.json"),
            r#"{"name": "bare-plugin", "version": "0.1.0"}"#,
        )
        .unwrap();

        let plugin = load_plugin_from_dir(dir.path(), PluginSource::ProjectLocal).unwrap();
        assert_eq!(plugin.manifest.name, "bare-plugin");
        assert!(plugin.skills.is_empty());
        assert!(plugin.commands.is_empty());
        assert!(plugin.hooks.is_empty());
        assert!(plugin.instructions.is_none());
    }

    #[test]
    fn test_hooks_json_standalone() {
        let dir = TempDir::new().unwrap();
        let hooks_dir = dir.path().join("hooks");
        fs::create_dir(&hooks_dir).unwrap();

        // hooks/hooks.json without a wrapping "hooks" key
        let hooks_json = serde_json::json!({
            "PreToolUse": [
                {"command": "echo pre", "type": "command"}
            ]
        });
        fs::write(
            hooks_dir.join("hooks.json"),
            serde_json::to_string(&hooks_json).unwrap(),
        )
        .unwrap();

        // Also need plugin.json
        fs::write(dir.path().join("plugin.json"), r#"{"name": "hooks-test"}"#).unwrap();

        let plugin = load_plugin_from_dir(dir.path(), PluginSource::ClaudeCode).unwrap();
        assert_eq!(plugin.hooks.len(), 1);
        assert_eq!(plugin.hooks[0].event, HookEvent::PreToolUse);
    }

    #[test]
    fn test_load_plugin_from_install_path() {
        let dir = TempDir::new().unwrap();
        let install_path = dir.path();

        // Create .claude-plugin/plugin.json (Claude Code cache structure)
        let claude_plugin_dir = install_path.join(".claude-plugin");
        fs::create_dir_all(&claude_plugin_dir).unwrap();
        fs::write(
            claude_plugin_dir.join("plugin.json"),
            r#"{"name": "cached-plugin", "version": "2.0.0", "description": "A cached plugin"}"#,
        )
        .unwrap();

        // Create skills at version root level
        let skill_dir = install_path.join("skills").join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A test skill\n---\n\n# My Skill\n\nDo stuff.\n",
        )
        .unwrap();

        // Create commands at version root level
        let commands_dir = install_path.join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(
            commands_dir.join("deploy.md"),
            "# Deploy\n\nDeploy the app.\n",
        )
        .unwrap();

        // Create CLAUDE.md at version root
        fs::write(
            install_path.join("CLAUDE.md"),
            "# Instructions\n\nFollow these rules.\n",
        )
        .unwrap();

        let plugin = load_plugin_from_install_path(install_path, PluginSource::Installed).unwrap();
        assert_eq!(plugin.manifest.name, "cached-plugin");
        assert_eq!(plugin.manifest.version, "2.0.0");
        assert_eq!(plugin.source, PluginSource::Installed);
        assert_eq!(plugin.skills.len(), 1);
        assert_eq!(plugin.skills[0].name, "my-skill");
        assert_eq!(plugin.commands.len(), 1);
        assert_eq!(plugin.commands[0].name, "deploy");
        assert!(plugin.instructions.is_some());
        assert!(plugin
            .instructions
            .as_ref()
            .unwrap()
            .contains("Follow these rules"));
    }

    #[test]
    fn test_load_plugin_from_install_path_missing_manifest() {
        let dir = TempDir::new().unwrap();
        let result = load_plugin_from_install_path(dir.path(), PluginSource::Installed);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains(".claude-plugin/plugin.json not found"));
    }

    #[test]
    fn test_load_plugin_from_install_path_author_object() {
        let dir = TempDir::new().unwrap();
        let claude_plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&claude_plugin_dir).unwrap();
        fs::write(
            claude_plugin_dir.join("plugin.json"),
            r#"{"name": "author-test", "author": {"name": "Jane Doe", "url": "https://example.com"}}"#,
        )
        .unwrap();

        let plugin = load_plugin_from_install_path(dir.path(), PluginSource::Installed).unwrap();
        assert_eq!(plugin.manifest.author.as_deref(), Some("Jane Doe"));
    }

    #[test]
    fn test_discover_installed_plugins_from_v2_format() {
        let dir = TempDir::new().unwrap();

        // Create a fake plugin at a fake installPath
        let install_path = dir
            .path()
            .join("cache")
            .join("org")
            .join("my-plugin")
            .join("1.0.0");
        fs::create_dir_all(install_path.join(".claude-plugin")).unwrap();
        fs::write(
            install_path.join(".claude-plugin").join("plugin.json"),
            r#"{"name": "my-plugin", "version": "1.0.0", "description": "Test"}"#,
        )
        .unwrap();

        // Create installed_plugins.json in v2 format
        let installed_json = dir.path().join("installed_plugins.json");
        let json_content = serde_json::json!({
            "version": 2,
            "plugins": {
                "my-plugin@org": [
                    {
                        "scope": "user",
                        "installPath": install_path.to_str().unwrap(),
                        "version": "1.0.0"
                    }
                ]
            }
        });
        fs::write(
            &installed_json,
            serde_json::to_string_pretty(&json_content).unwrap(),
        )
        .unwrap();

        let plugins = discover_installed_plugins_from(&installed_json);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].manifest.name, "my-plugin");
        assert_eq!(plugins[0].manifest.version, "1.0.0");
        assert_eq!(plugins[0].source, PluginSource::Installed);
    }

    #[test]
    fn test_discover_installed_plugins_from_nonexistent() {
        let plugins =
            discover_installed_plugins_from(Path::new("/nonexistent/installed_plugins.json"));
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_discover_installed_plugins_from_empty_plugins() {
        let dir = TempDir::new().unwrap();
        let installed_json = dir.path().join("installed_plugins.json");
        fs::write(&installed_json, r#"{"version": 2, "plugins": {}}"#).unwrap();

        let plugins = discover_installed_plugins_from(&installed_json);
        assert!(plugins.is_empty());
    }
}
