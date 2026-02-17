//! Plugin Commands
//!
//! Tauri commands for the Claude Code-compatible plugin system.
//! Provides commands for listing, toggling, refreshing, and inspecting plugins.

use std::sync::Arc;
use tokio::sync::RwLock;

use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::plugins::manager::PluginManager;
use crate::services::plugins::models::*;

// ============================================================================
// Plugin State
// ============================================================================

/// Tauri-managed plugin state.
///
/// Uses `Arc<RwLock<Option<PluginManager>>>` for lazy initialization.
/// Initialized during `init_app` or on first access.
pub struct PluginState {
    inner: Arc<RwLock<Option<PluginManager>>>,
}

impl PluginState {
    /// Create a new uninitialized plugin state.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the plugin manager for a project.
    pub async fn initialize(&self, project_root: &str) {
        let mut guard = self.inner.write().await;
        let mut manager = PluginManager::new(project_root);
        manager.discover_and_load();
        *guard = Some(manager);
    }

    /// Get read access to the plugin manager.
    async fn with_manager<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&PluginManager) -> T,
    {
        let guard = self.inner.read().await;
        match &*guard {
            Some(manager) => Ok(f(manager)),
            None => Err("Plugin system not initialized. Call init_app first.".to_string()),
        }
    }

    /// Get write access to the plugin manager.
    async fn with_manager_mut<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&mut PluginManager) -> T,
    {
        let mut guard = self.inner.write().await;
        match &mut *guard {
            Some(manager) => Ok(f(manager)),
            None => Err("Plugin system not initialized. Call init_app first.".to_string()),
        }
    }
}

impl Default for PluginState {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginState").finish()
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// List all discovered plugins with their status.
///
/// Returns a list of PluginInfo summaries including name, version,
/// source, enabled state, and component counts.
#[tauri::command]
pub async fn list_plugins(
    state: State<'_, PluginState>,
) -> Result<CommandResponse<Vec<PluginInfo>>, String> {
    match state.with_manager(|m| m.list_plugins()).await {
        Ok(plugins) => Ok(CommandResponse::ok(plugins)),
        Err(e) => Ok(CommandResponse::err(e)),
    }
}

/// Toggle a plugin's enabled/disabled state.
///
/// Returns success if the plugin was found and toggled.
#[tauri::command]
pub async fn toggle_plugin(
    name: String,
    enabled: bool,
    state: State<'_, PluginState>,
) -> Result<CommandResponse<bool>, String> {
    match state
        .with_manager_mut(|m| m.toggle_plugin(&name, enabled))
        .await
    {
        Ok(toggled) => {
            if toggled {
                Ok(CommandResponse::ok(true))
            } else {
                Ok(CommandResponse::err(format!(
                    "Plugin not found: {}",
                    name
                )))
            }
        }
        Err(e) => Ok(CommandResponse::err(e)),
    }
}

/// Refresh plugin discovery (re-scan all sources).
///
/// Preserves enabled/disabled state for plugins that still exist.
/// Returns the updated plugin list.
#[tauri::command]
pub async fn refresh_plugins(
    state: State<'_, PluginState>,
) -> Result<CommandResponse<Vec<PluginInfo>>, String> {
    match state
        .with_manager_mut(|m| {
            m.refresh_plugins();
            m.list_plugins()
        })
        .await
    {
        Ok(plugins) => Ok(CommandResponse::ok(plugins)),
        Err(e) => Ok(CommandResponse::err(e)),
    }
}

/// Get detailed information about a specific plugin.
///
/// Returns the full LoadedPlugin with skills, commands, hooks, and instructions.
#[tauri::command]
pub async fn get_plugin_detail(
    name: String,
    state: State<'_, PluginState>,
) -> Result<CommandResponse<PluginDetail>, String> {
    match state
        .with_manager(|m| {
            m.get_plugin(&name).map(|p| PluginDetail {
                plugin: p.clone(),
                root_path: p.root_path.clone(),
            })
        })
        .await
    {
        Ok(Some(detail)) => Ok(CommandResponse::ok(detail)),
        Ok(None) => Ok(CommandResponse::err(format!(
            "Plugin not found: {}",
            name
        ))),
        Err(e) => Ok(CommandResponse::err(e)),
    }
}

/// Install a plugin from a source directory.
///
/// Copies the plugin directory to ~/.plan-cascade/plugins/ and reloads.
#[tauri::command]
pub async fn install_plugin(
    source_path: String,
    state: State<'_, PluginState>,
) -> Result<CommandResponse<PluginInfo>, String> {
    let source = std::path::Path::new(&source_path);

    // Validate source has a plugin.json
    if !source.join("plugin.json").exists() {
        return Ok(CommandResponse::err(format!(
            "No plugin.json found in {}",
            source_path
        )));
    }

    // Read the manifest to get the plugin name
    let manifest_content = match std::fs::read_to_string(source.join("plugin.json")) {
        Ok(c) => c,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to read plugin.json: {}",
                e
            )))
        }
    };
    let manifest: PluginManifest = match serde_json::from_str(&manifest_content) {
        Ok(m) => m,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Invalid plugin.json: {}",
                e
            )))
        }
    };

    // Determine destination
    let dest = match dirs::home_dir() {
        Some(home) => home
            .join(".plan-cascade")
            .join("plugins")
            .join(&manifest.name),
        None => {
            return Ok(CommandResponse::err(
                "Cannot determine home directory".to_string(),
            ))
        }
    };

    // Create destination and copy files
    if let Err(e) = copy_dir_recursive(source, &dest) {
        return Ok(CommandResponse::err(format!(
            "Failed to install plugin: {}",
            e
        )));
    }

    // Refresh to pick up the new plugin
    let result = state
        .with_manager_mut(|m| {
            m.refresh_plugins();
            m.get_plugin(&manifest.name).map(|p| p.to_info())
        })
        .await;

    match result {
        Ok(Some(info)) => Ok(CommandResponse::ok(info)),
        Ok(None) => Ok(CommandResponse::err(
            "Plugin installed but not found after refresh".to_string(),
        )),
        Err(e) => Ok(CommandResponse::err(e)),
    }
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path)?;
        }
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_plugin(dir: &std::path::Path) {
        fs::write(
            dir.join("plugin.json"),
            r#"{"name": "test-plugin", "version": "1.0.0", "description": "Test"}"#,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_plugin_state_new() {
        let state = PluginState::new();
        let result = state.with_manager(|m| m.plugin_count()).await;
        assert!(result.is_err(), "Should fail when not initialized");
    }

    #[tokio::test]
    async fn test_plugin_state_initialize() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_test_plugin(&plugin_dir);

        let state = PluginState::new();
        state.initialize(dir.path().to_str().unwrap()).await;

        let count = state.with_manager(|m| m.plugin_count()).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_plugin_state_toggle() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_test_plugin(&plugin_dir);

        let state = PluginState::new();
        state.initialize(dir.path().to_str().unwrap()).await;

        // Verify enabled
        let enabled = state
            .with_manager(|m| m.is_plugin_enabled("test-plugin"))
            .await
            .unwrap();
        assert!(enabled);

        // Toggle off
        let toggled = state
            .with_manager_mut(|m| m.toggle_plugin("test-plugin", false))
            .await
            .unwrap();
        assert!(toggled);

        // Verify disabled
        let enabled = state
            .with_manager(|m| m.is_plugin_enabled("test-plugin"))
            .await
            .unwrap();
        assert!(!enabled);
    }

    #[tokio::test]
    async fn test_plugin_state_refresh() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        create_test_plugin(&plugin_dir);

        let state = PluginState::new();
        state.initialize(dir.path().to_str().unwrap()).await;

        let plugins = state.with_manager_mut(|m| {
            m.refresh_plugins();
            m.list_plugins()
        }).await.unwrap();
        assert_eq!(plugins.len(), 1);
    }

    #[test]
    fn test_copy_dir_recursive() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();

        // Create source structure
        fs::write(src.path().join("file.txt"), "hello").unwrap();
        let subdir = src.path().join("sub");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("nested.txt"), "world").unwrap();

        let dest_path = dst.path().join("copy");
        copy_dir_recursive(src.path(), &dest_path).unwrap();

        assert!(dest_path.join("file.txt").exists());
        assert!(dest_path.join("sub").join("nested.txt").exists());
        assert_eq!(
            fs::read_to_string(dest_path.join("file.txt")).unwrap(),
            "hello"
        );
    }
}
