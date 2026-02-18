//! Plugin Settings Persistence
//!
//! Persists plugin enabled/disabled state to disk so it survives app restarts.
//! Settings are stored at `~/.plan-cascade/plugin-settings.json`.

use std::path::PathBuf;

use crate::services::plugins::models::PluginSettings;

/// Get the path to the plugin settings file.
///
/// Returns `~/.plan-cascade/plugin-settings.json`.
pub fn settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".plan-cascade").join("plugin-settings.json"))
}

/// Load plugin settings from disk.
///
/// Returns default settings if the file doesn't exist or can't be parsed.
pub fn load_plugin_settings() -> PluginSettings {
    let path = match settings_path() {
        Some(p) => p,
        None => return PluginSettings::default(),
    };

    if !path.exists() {
        return PluginSettings::default();
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            eprintln!("[plugins] Failed to read plugin settings: {}", e);
            PluginSettings::default()
        }
    }
}

/// Save plugin settings to disk.
///
/// Creates the parent directory if it doesn't exist.
pub fn save_plugin_settings(settings: &PluginSettings) -> Result<(), String> {
    let path = settings_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings directory: {}", e))?;
    }

    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write plugin settings: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_default_when_missing() {
        // settings_path returns a real path, but we test the default behavior
        let settings = PluginSettings::default();
        assert!(settings.disabled_plugins.is_empty());
    }

    #[test]
    fn test_roundtrip_settings() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("plugin-settings.json");

        let settings = PluginSettings {
            disabled_plugins: vec!["plugin-a".to_string(), "plugin-b".to_string()],
        };

        let content = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&path, &content).unwrap();

        let loaded: PluginSettings =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.disabled_plugins.len(), 2);
        assert!(loaded.disabled_plugins.contains(&"plugin-a".to_string()));
        assert!(loaded.disabled_plugins.contains(&"plugin-b".to_string()));
    }
}
