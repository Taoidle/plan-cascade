//! Plugin Installer
//!
//! Handles git-based plugin installation and uninstallation.
//! Plugins are installed to `~/.plan-cascade/plugins/<name>/`.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter};

use crate::services::plugins::models::{InstallProgress, PluginManifest};

/// Get the managed plugins directory.
///
/// Returns `~/.plan-cascade/plugins/`.
pub fn managed_plugins_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".plan-cascade").join("plugins"))
}

/// Install a plugin from a git URL.
///
/// 1. Clones the repo (shallow) to a temp directory
/// 2. Validates that plugin.json exists and parses
/// 3. Moves to `~/.plan-cascade/plugins/<name>/`
/// 4. Removes `.git/` directory to save space
/// 5. Emits `plugin:install-progress` events throughout
pub async fn install_from_git(
    git_url: &str,
    app: &AppHandle,
) -> Result<PluginManifest, String> {
    let plugins_dir =
        managed_plugins_dir().ok_or_else(|| "Cannot determine home directory".to_string())?;

    // Ensure plugins directory exists
    std::fs::create_dir_all(&plugins_dir)
        .map_err(|e| format!("Failed to create plugins directory: {}", e))?;

    // Phase 1: Clone to temp directory
    emit_progress(app, "unknown", "cloning", "Cloning repository...", 0.1);

    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;
    let clone_path = temp_dir.path().join("plugin");

    let output = tokio::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            git_url,
            clone_path.to_str().unwrap_or("plugin"),
        ])
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "git is not installed. Please install git to use this feature.".to_string()
            } else {
                format!("Failed to execute git clone: {}", e)
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed: {}", stderr.trim()));
    }

    emit_progress(app, "unknown", "cloning", "Repository cloned", 0.4);

    // Phase 2: Validate plugin.json
    emit_progress(app, "unknown", "validating", "Validating plugin...", 0.5);

    let manifest_path = clone_path.join("plugin.json");
    if !manifest_path.exists() {
        return Err("No plugin.json found in repository root".to_string());
    }

    let manifest_content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read plugin.json: {}", e))?;

    let manifest: PluginManifest = serde_json::from_str(&manifest_content)
        .map_err(|e| format!("Invalid plugin.json: {}", e))?;

    if manifest.name.is_empty() {
        return Err("plugin.json must have a non-empty 'name' field".to_string());
    }

    emit_progress(
        app,
        &manifest.name,
        "validating",
        &format!("Validated: {}", manifest.name),
        0.6,
    );

    // Phase 3: Move to plugins directory
    let dest = plugins_dir.join(&manifest.name);

    emit_progress(
        app,
        &manifest.name,
        "installing",
        "Installing plugin...",
        0.7,
    );

    // Remove existing installation if any
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed to remove existing plugin: {}", e))?;
    }

    // Move clone to final destination
    copy_dir_recursive(&clone_path, &dest)
        .map_err(|e| format!("Failed to install plugin: {}", e))?;

    // Phase 4: Remove .git/ to save space
    let git_dir = dest.join(".git");
    if git_dir.exists() {
        let _ = std::fs::remove_dir_all(&git_dir);
    }

    emit_progress(
        app,
        &manifest.name,
        "complete",
        &format!("Plugin '{}' installed successfully", manifest.name),
        1.0,
    );

    Ok(manifest)
}

/// Uninstall a plugin by removing its directory.
///
/// Only works for plugins in `~/.plan-cascade/plugins/`.
pub fn uninstall_plugin(name: &str) -> Result<(), String> {
    let plugins_dir =
        managed_plugins_dir().ok_or_else(|| "Cannot determine home directory".to_string())?;

    let plugin_dir = plugins_dir.join(name);

    if !plugin_dir.exists() {
        return Err(format!("Plugin '{}' not found in managed plugins", name));
    }

    std::fs::remove_dir_all(&plugin_dir)
        .map_err(|e| format!("Failed to remove plugin '{}': {}", name, e))?;

    eprintln!("[plugins] Uninstalled plugin '{}'", name);
    Ok(())
}

/// Emit a progress event to the frontend.
fn emit_progress(app: &AppHandle, plugin_name: &str, phase: &str, message: &str, progress: f64) {
    let event = InstallProgress {
        plugin_name: plugin_name.to_string(),
        phase: phase.to_string(),
        message: message.to_string(),
        progress,
    };
    let _ = app.emit("plugin:install-progress", &event);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_managed_plugins_dir() {
        let dir = managed_plugins_dir();
        assert!(dir.is_some());
        let path = dir.unwrap();
        assert!(path.to_str().unwrap().contains(".plan-cascade"));
        assert!(path.to_str().unwrap().contains("plugins"));
    }

    #[test]
    fn test_uninstall_nonexistent() {
        let result = uninstall_plugin("definitely-not-a-real-plugin-12345");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
