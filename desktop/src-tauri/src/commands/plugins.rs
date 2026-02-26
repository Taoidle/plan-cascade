//! Plugin Commands
//!
//! Tauri commands for the Claude Code-compatible plugin system.
//! Provides commands for listing, toggling, refreshing, and inspecting plugins.

use std::sync::Arc;
use tokio::sync::RwLock;

use tauri::{AppHandle, State};

use crate::models::response::CommandResponse;
use crate::services::plugins::installer;
use crate::services::plugins::manager::PluginManager;
use crate::services::plugins::marketplace;
use crate::services::plugins::models::*;
use crate::services::plugins::registry;
use crate::services::plugins::settings as plugin_settings;

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

    /// Wire plugin context into an OrchestratorService.
    ///
    /// Acquires a brief read lock on the plugin manager, extracts all plugin
    /// data (instructions, skills, commands, hooks, permissions), and returns the wired orchestrator.
    /// Returns the orchestrator unchanged if the plugin system is not initialized.
    ///
    /// - `event_tx`: Optional stream event sender for reporting hook failures to the frontend.
    pub async fn wire_orchestrator(
        &self,
        orchestrator: crate::services::orchestrator::OrchestratorService,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::services::streaming::UnifiedStreamEvent>>,
    ) -> crate::services::orchestrator::OrchestratorService {
        let guard = self.inner.read().await;
        match &*guard {
            Some(manager) => orchestrator.with_plugin_context(manager, event_tx),
            None => {
                eprintln!("[plugins] Plugin system not initialized, skipping plugin wiring");
                orchestrator
            }
        }
    }

    /// Collect quality gate definitions from all enabled plugins.
    pub async fn collect_quality_gates(
        &self,
    ) -> Vec<crate::services::plugins::models::PluginQualityGate> {
        let guard = self.inner.read().await;
        match &*guard {
            Some(manager) => manager.collect_quality_gates(),
            None => vec![],
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

/// List user-invocable plugin skills.
///
/// Returns skills from enabled plugins where `user_invocable: true`.
/// These can be invoked from the chat input via `/<skill-name>`.
#[tauri::command]
pub async fn list_invocable_plugin_skills(
    state: State<'_, PluginState>,
) -> Result<CommandResponse<Vec<PluginSkill>>, String> {
    match state
        .with_manager(|m| {
            m.collect_skills()
                .into_iter()
                .filter(|s| s.user_invocable)
                .collect::<Vec<_>>()
        })
        .await
    {
        Ok(skills) => Ok(CommandResponse::ok(skills)),
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
                Ok(CommandResponse::err(format!("Plugin not found: {}", name)))
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
        Ok(None) => Ok(CommandResponse::err(format!("Plugin not found: {}", name))),
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
        Err(e) => return Ok(CommandResponse::err(format!("Invalid plugin.json: {}", e))),
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

/// Fetch marketplace plugins from all configured marketplaces.
///
/// Fetches from all enabled marketplace sources and aggregates results.
/// Each plugin is enriched with local install/enable status.
#[tauri::command]
pub async fn fetch_marketplace(
    registry_url: Option<String>,
    state: State<'_, PluginState>,
) -> Result<CommandResponse<Vec<MarketplacePlugin>>, String> {
    let settings = plugin_settings::load_plugin_settings();
    let installed_plugins = state
        .with_manager(|m| m.list_plugins())
        .await
        .unwrap_or_default();

    // Fetch from all configured marketplaces
    let manifests = marketplace::fetch_all_marketplaces(&settings.marketplaces).await;

    let mut all_plugins: Vec<MarketplacePlugin> = Vec::new();

    for (marketplace_name, manifest) in &manifests {
        let marketplace_config = settings
            .marketplaces
            .iter()
            .find(|m| &m.name == marketplace_name);

        for entry in &manifest.plugins {
            let local = installed_plugins.iter().find(|p| p.name == entry.name);

            let source_spec = marketplace_config
                .and_then(|cfg| marketplace::resolve_install_source(entry, cfg).ok())
                .map(|s| s.to_spec_string())
                .unwrap_or_default();

            all_plugins.push(MarketplacePlugin {
                name: entry.name.clone(),
                version: entry.version.clone().unwrap_or_default(),
                description: entry.description.clone().unwrap_or_default(),
                author: entry.author_string(),
                repository: entry.repository.clone(),
                license: entry.license.clone(),
                keywords: entry.keywords.clone(),
                category: entry.category.clone(),
                marketplace_name: marketplace_name.clone(),
                source_spec,
                installed: local.is_some(),
                enabled: local.is_some_and(|p| p.enabled),
            });
        }
    }

    // If no marketplace plugins found, fall back to legacy registry
    if all_plugins.is_empty() {
        let reg = registry::fetch_registry(registry_url.as_deref()).await;
        for entry in reg.plugins {
            let local = installed_plugins.iter().find(|p| p.name == entry.name);
            all_plugins.push(MarketplacePlugin {
                name: entry.name.clone(),
                version: entry.version.clone(),
                description: entry.description.clone(),
                author: entry.author.clone(),
                repository: entry.repository.clone(),
                license: entry.license.clone(),
                keywords: entry.keywords.clone(),
                category: entry.category.clone(),
                marketplace_name: "legacy-registry".to_string(),
                source_spec: format!("git:{}", entry.git_url),
                installed: local.is_some(),
                enabled: local.is_some_and(|p| p.enabled),
            });
        }
    }

    Ok(CommandResponse::ok(all_plugins))
}

/// Install a plugin from a git URL.
///
/// Clones the repository, validates plugin.json, installs to managed
/// plugins directory, and refreshes the plugin manager.
#[tauri::command]
pub async fn install_plugin_from_git(
    git_url: String,
    app: AppHandle,
    state: State<'_, PluginState>,
) -> Result<CommandResponse<PluginInfo>, String> {
    // Do the heavy git clone work OUTSIDE the PluginState lock
    let manifest = match installer::install_from_git(&git_url, &app).await {
        Ok(m) => m,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    // Brief lock to refresh and get the installed plugin info
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

/// Uninstall a plugin by name.
///
/// Only plugins with `ProjectLocal` source can be uninstalled.
/// Removes the plugin directory and refreshes the plugin manager.
#[tauri::command]
pub async fn uninstall_plugin(
    name: String,
    state: State<'_, PluginState>,
) -> Result<CommandResponse<bool>, String> {
    // Verify the plugin exists and is ProjectLocal source
    let is_project_local = state
        .with_manager(|m| {
            m.get_plugin(&name)
                .map(|p| p.source == PluginSource::ProjectLocal)
        })
        .await;

    match is_project_local {
        Ok(Some(true)) => {}
        Ok(Some(false)) => {
            return Ok(CommandResponse::err(format!(
                "Only project-local plugins can be uninstalled. Plugin '{}' has a different source.",
                name
            )));
        }
        Ok(None) => {
            return Ok(CommandResponse::err(format!("Plugin not found: {}", name)));
        }
        Err(e) => return Ok(CommandResponse::err(e)),
    }

    // Remove the plugin directory
    if let Err(e) = installer::uninstall_plugin(&name) {
        return Ok(CommandResponse::err(e));
    }

    // Refresh to remove the plugin from the manager
    let _ = state
        .with_manager_mut(|m| {
            m.refresh_plugins();
        })
        .await;

    Ok(CommandResponse::ok(true))
}

// ============================================================================
// Marketplace Management Commands
// ============================================================================

/// List all configured marketplaces with status.
#[tauri::command]
pub async fn list_marketplaces(
    _state: State<'_, PluginState>,
) -> Result<CommandResponse<Vec<MarketplaceInfo>>, String> {
    let settings = plugin_settings::load_plugin_settings();

    let mut infos = Vec::new();
    for config in &settings.marketplaces {
        infos.push(MarketplaceInfo {
            name: config.name.clone(),
            source_display: config.source.display(),
            enabled: config.enabled,
            plugin_count: 0, // Will be populated by frontend after fetch
            description: None,
            is_official: config.name == "claude-plugins-official",
        });
    }

    Ok(CommandResponse::ok(infos))
}

/// Add a new marketplace source.
///
/// Auto-detects source type from the input string:
/// - "owner/repo" → GitHub
/// - "https://..." or "git@..." → Git URL
/// - Path → Local
///
/// Validates by fetching marketplace.json before saving.
#[tauri::command]
pub async fn add_marketplace(
    source: String,
    _state: State<'_, PluginState>,
) -> Result<CommandResponse<MarketplaceInfo>, String> {
    let trimmed = source.trim();

    // Auto-detect source type
    let (source_type, name) = if trimmed.starts_with("https://")
        || trimmed.starts_with("http://")
        || trimmed.starts_with("git@")
    {
        // Git URL
        let name = trimmed
            .rsplit('/')
            .next()
            .unwrap_or("custom")
            .trim_end_matches(".git")
            .to_string();
        (
            MarketplaceSourceType::GitUrl {
                url: trimmed.to_string(),
            },
            name,
        )
    } else if trimmed.contains('/')
        && !trimmed.contains(' ')
        && !std::path::Path::new(trimmed).exists()
    {
        // GitHub shorthand (owner/repo)
        let name = trimmed.rsplit('/').next().unwrap_or("custom").to_string();
        (
            MarketplaceSourceType::Github {
                repo: trimmed.to_string(),
            },
            name,
        )
    } else {
        // Local path
        let name = std::path::Path::new(trimmed)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("local")
            .to_string();
        (
            MarketplaceSourceType::LocalPath {
                path: trimmed.to_string(),
            },
            name,
        )
    };

    let config = MarketplaceConfig {
        name: name.clone(),
        source: source_type.clone(),
        enabled: true,
    };

    // Validate by fetching the manifest
    if let Err(e) = marketplace::fetch_marketplace_manifest(&config).await {
        return Ok(CommandResponse::err(format!(
            "Could not find a valid marketplace.json: {}",
            e
        )));
    }

    // Save to settings
    if let Err(e) = plugin_settings::add_marketplace(config.clone()) {
        return Ok(CommandResponse::err(e));
    }

    Ok(CommandResponse::ok(MarketplaceInfo {
        name,
        source_display: source_type.display(),
        enabled: true,
        plugin_count: 0,
        description: None,
        is_official: false,
    }))
}

/// Remove a marketplace source.
///
/// Cannot remove the official marketplace.
#[tauri::command]
pub async fn remove_marketplace(
    name: String,
    _state: State<'_, PluginState>,
) -> Result<CommandResponse<bool>, String> {
    if let Err(e) = plugin_settings::remove_marketplace(&name) {
        return Ok(CommandResponse::err(e));
    }

    marketplace::remove_cached_manifest(&name);
    Ok(CommandResponse::ok(true))
}

/// Toggle a marketplace's enabled/disabled state.
#[tauri::command]
pub async fn toggle_marketplace(
    name: String,
    enabled: bool,
    _state: State<'_, PluginState>,
) -> Result<CommandResponse<bool>, String> {
    if let Err(e) = plugin_settings::toggle_marketplace(&name, enabled) {
        return Ok(CommandResponse::err(e));
    }
    Ok(CommandResponse::ok(true))
}

/// Install a plugin from a specific marketplace.
///
/// Finds the plugin entry in the specified marketplace, resolves the source,
/// and installs it.
#[tauri::command]
pub async fn install_marketplace_plugin(
    plugin_name: String,
    marketplace_name: String,
    app: AppHandle,
    state: State<'_, PluginState>,
) -> Result<CommandResponse<PluginInfo>, String> {
    let settings = plugin_settings::load_plugin_settings();

    // Find the marketplace config
    let marketplace_config = match settings
        .marketplaces
        .iter()
        .find(|m| m.name == marketplace_name)
    {
        Some(c) => c.clone(),
        None => {
            return Ok(CommandResponse::err(format!(
                "Marketplace '{}' not found",
                marketplace_name
            )))
        }
    };

    // Fetch the marketplace manifest to find the plugin entry
    let manifest = match marketplace::fetch_marketplace_manifest(&marketplace_config).await {
        Ok(m) => m,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to fetch marketplace: {}",
                e
            )))
        }
    };

    let entry = match manifest.plugins.iter().find(|p| p.name == plugin_name) {
        Some(e) => e.clone(),
        None => {
            return Ok(CommandResponse::err(format!(
                "Plugin '{}' not found in marketplace '{}'",
                plugin_name, marketplace_name
            )))
        }
    };

    // Install
    let manifest =
        match installer::install_from_marketplace(&entry, &marketplace_config, &app).await {
            Ok(m) => m,
            Err(e) => return Ok(CommandResponse::err(e)),
        };

    // Refresh plugin manager
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
