//! Marketplace Fetcher
//!
//! Fetches marketplace manifests from Claude Code-compatible marketplace
//! repositories. Supports GitHub, generic Git URLs, and local paths.
//!
//! ## Fetch Strategy
//! - **Browsing**: HTTP-fetch `marketplace.json` from raw GitHub URLs (fast)
//! - **Installing**: Git clone on demand (only when user clicks Install)
//! - **Fallback**: For non-GitHub URLs, git clone to temp, extract marketplace.json

use std::path::PathBuf;

use crate::services::plugins::models::{
    MarketplaceConfig, MarketplaceManifest, MarketplacePluginEntry, MarketplaceSourceType,
};

/// Cache directory for marketplace manifests.
fn cache_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".plan-cascade").join("marketplace-cache"))
}

/// Fetch a marketplace manifest from its source.
///
/// - GitHub: HTTP GET raw.githubusercontent.com
/// - GitUrl: clone to temp, read .claude-plugin/marketplace.json
/// - LocalPath: read directly from disk
pub async fn fetch_marketplace_manifest(
    config: &MarketplaceConfig,
) -> Result<MarketplaceManifest, String> {
    match &config.source {
        MarketplaceSourceType::Github { repo } => fetch_github_marketplace(repo).await,
        MarketplaceSourceType::GitUrl { url } => fetch_git_marketplace(url).await,
        MarketplaceSourceType::LocalPath { path } => fetch_local_marketplace(path),
    }
}

/// Fetch marketplace.json from a GitHub repo via raw.githubusercontent.com.
///
/// Tries `main` branch first, then `master`.
async fn fetch_github_marketplace(repo: &str) -> Result<MarketplaceManifest, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("plan-cascade-desktop")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Try main branch first, then master
    for branch in &["main", "master"] {
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}/marketplace.json",
            repo, branch, ".claude-plugin"
        );

        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let manifest: MarketplaceManifest = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse marketplace.json: {}", e))?;
                // Cache the result
                let _ = cache_manifest(&manifest, repo);
                return Ok(manifest);
            }
            _ => continue,
        }
    }

    // Try loading from cache
    if let Some(cached) = load_cached_manifest(repo) {
        eprintln!(
            "[marketplace] Using cached manifest for {} (network unavailable)",
            repo
        );
        return Ok(cached);
    }

    Err(format!(
        "Failed to fetch marketplace.json from github:{} (tried main and master branches)",
        repo
    ))
}

/// Fetch marketplace.json from a generic git URL by cloning to temp.
async fn fetch_git_marketplace(url: &str) -> Result<MarketplaceManifest, String> {
    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;
    let clone_path = temp_dir.path().join("marketplace");

    let output = tokio::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--filter=blob:none",
            "--sparse",
            url,
            clone_path.to_str().unwrap_or("marketplace"),
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to execute git clone: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed for {}: {}", url, stderr.trim()));
    }

    // Try to sparse checkout just .claude-plugin/
    let _ = tokio::process::Command::new("git")
        .args(["sparse-checkout", "set", ".claude-plugin"])
        .current_dir(&clone_path)
        .output()
        .await;

    let manifest_path = clone_path.join(".claude-plugin").join("marketplace.json");
    if !manifest_path.exists() {
        return Err(format!(
            "No .claude-plugin/marketplace.json found in {}",
            url
        ));
    }

    let content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read marketplace.json: {}", e))?;

    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse marketplace.json: {}", e))
}

/// Fetch marketplace.json from a local path.
fn fetch_local_marketplace(path: &str) -> Result<MarketplaceManifest, String> {
    let base = std::path::Path::new(path);
    let manifest_path = base.join(".claude-plugin").join("marketplace.json");

    if !manifest_path.exists() {
        return Err(format!(
            "No .claude-plugin/marketplace.json found at {}",
            path
        ));
    }

    let content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read marketplace.json: {}", e))?;

    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse marketplace.json: {}", e))
}

/// Fetch manifests from all enabled marketplaces.
///
/// Fetches in parallel. Logs failures but returns successes.
pub async fn fetch_all_marketplaces(
    configs: &[MarketplaceConfig],
) -> Vec<(String, MarketplaceManifest)> {
    let mut handles = Vec::new();

    for config in configs {
        if !config.enabled {
            continue;
        }
        let config = config.clone();
        handles.push(tokio::spawn(async move {
            match fetch_marketplace_manifest(&config).await {
                Ok(manifest) => {
                    eprintln!(
                        "[marketplace] Fetched {}: {} plugins",
                        config.name,
                        manifest.plugins.len()
                    );
                    Some((config.name.clone(), manifest))
                }
                Err(e) => {
                    eprintln!("[marketplace] Failed to fetch {}: {}", config.name, e);
                    None
                }
            }
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(Some(result)) = handle.await {
            results.push(result);
        }
    }
    results
}

/// Resolve the install source for a marketplace plugin entry.
///
/// Converts the plugin's source field into a git URL or local path.
pub fn resolve_install_source(
    entry: &MarketplacePluginEntry,
    marketplace: &MarketplaceConfig,
) -> Result<InstallSource, String> {
    // If the plugin has an explicit source
    if let Some(source) = &entry.source {
        match source {
            // String source: relative path or URL
            serde_json::Value::String(s) => {
                if s.starts_with("./") || s.starts_with("../") {
                    // Relative path within marketplace repo
                    return Ok(InstallSource::RelativeInMarketplace {
                        subdir: s.clone(),
                        marketplace: marketplace.clone(),
                    });
                }
                if s.starts_with("https://") || s.starts_with("http://") || s.starts_with("git@")
                {
                    return Ok(InstallSource::GitUrl(s.clone()));
                }
                // Assume it's a relative path
                return Ok(InstallSource::RelativeInMarketplace {
                    subdir: s.clone(),
                    marketplace: marketplace.clone(),
                });
            }
            // Object source with type-specific fields
            serde_json::Value::Object(map) => {
                if let Some(repo) = map.get("repo").and_then(|v| v.as_str()) {
                    return Ok(InstallSource::GitUrl(format!(
                        "https://github.com/{}.git",
                        repo
                    )));
                }
                if let Some(url) = map.get("url").and_then(|v| v.as_str()) {
                    return Ok(InstallSource::GitUrl(url.to_string()));
                }
                if let Some(path) = map.get("path").and_then(|v| v.as_str()) {
                    return Ok(InstallSource::RelativeInMarketplace {
                        subdir: path.to_string(),
                        marketplace: marketplace.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    // Fallback: try to construct from repository field
    if let Some(repo) = &entry.repository {
        return Ok(InstallSource::GitUrl(repo.clone()));
    }

    Err(format!(
        "Cannot resolve install source for plugin '{}'",
        entry.name
    ))
}

/// Resolved install source for a marketplace plugin.
#[derive(Debug, Clone)]
pub enum InstallSource {
    /// A git URL to clone
    GitUrl(String),
    /// A subdirectory within the marketplace repo
    RelativeInMarketplace {
        subdir: String,
        marketplace: MarketplaceConfig,
    },
}

impl InstallSource {
    /// Serialize to a string for passing to the frontend.
    pub fn to_spec_string(&self) -> String {
        match self {
            Self::GitUrl(url) => format!("git:{}", url),
            Self::RelativeInMarketplace {
                subdir,
                marketplace,
            } => {
                format!("marketplace:{}:{}", marketplace.name, subdir)
            }
        }
    }
}

// ============================================================================
// Cache helpers
// ============================================================================

/// Cache a marketplace manifest to disk.
fn cache_manifest(manifest: &MarketplaceManifest, name: &str) -> Result<(), String> {
    let dir = cache_dir().ok_or("Cannot determine home directory")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create cache dir: {}", e))?;

    let safe_name = name.replace('/', "_");
    let path = dir.join(format!("{}.json", safe_name));
    let content = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("Failed to write cache: {}", e))?;
    Ok(())
}

/// Load a cached marketplace manifest from disk.
fn load_cached_manifest(name: &str) -> Option<MarketplaceManifest> {
    let dir = cache_dir()?;
    let safe_name = name.replace('/', "_");
    let path = dir.join(format!("{}.json", safe_name));

    if !path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Remove cached manifest for a marketplace.
pub fn remove_cached_manifest(name: &str) {
    if let Some(dir) = cache_dir() {
        let safe_name = name.replace('/', "_");
        let path = dir.join(format!("{}.json", safe_name));
        let _ = std::fs::remove_file(path);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_source_spec_string() {
        let git = InstallSource::GitUrl("https://github.com/test/plugin.git".to_string());
        assert!(git.to_spec_string().starts_with("git:"));

        let relative = InstallSource::RelativeInMarketplace {
            subdir: "./plugins/my-plugin".to_string(),
            marketplace: MarketplaceConfig {
                name: "official".to_string(),
                source: MarketplaceSourceType::Github {
                    repo: "anthropics/claude-plugins-official".to_string(),
                },
                enabled: true,
            },
        };
        assert!(relative.to_spec_string().starts_with("marketplace:"));
    }

    #[test]
    fn test_resolve_install_source_git_url() {
        let entry = MarketplacePluginEntry {
            name: "test".to_string(),
            source: Some(serde_json::Value::String(
                "https://github.com/test/plugin.git".to_string(),
            )),
            description: None,
            version: None,
            author: None,
            category: None,
            keywords: vec![],
            homepage: None,
            repository: None,
            license: None,
        };
        let config = MarketplaceConfig {
            name: "test-marketplace".to_string(),
            source: MarketplaceSourceType::Github {
                repo: "test/marketplace".to_string(),
            },
            enabled: true,
        };

        let result = resolve_install_source(&entry, &config).unwrap();
        matches!(result, InstallSource::GitUrl(_));
    }

    #[test]
    fn test_resolve_install_source_relative() {
        let entry = MarketplacePluginEntry {
            name: "test".to_string(),
            source: Some(serde_json::Value::String("./plugins/my-plugin".to_string())),
            description: None,
            version: None,
            author: None,
            category: None,
            keywords: vec![],
            homepage: None,
            repository: None,
            license: None,
        };
        let config = MarketplaceConfig {
            name: "test-marketplace".to_string(),
            source: MarketplaceSourceType::Github {
                repo: "test/marketplace".to_string(),
            },
            enabled: true,
        };

        let result = resolve_install_source(&entry, &config).unwrap();
        matches!(result, InstallSource::RelativeInMarketplace { .. });
    }

    #[test]
    fn test_resolve_install_source_object_repo() {
        let entry = MarketplacePluginEntry {
            name: "test".to_string(),
            source: Some(serde_json::json!({"repo": "owner/plugin"})),
            description: None,
            version: None,
            author: None,
            category: None,
            keywords: vec![],
            homepage: None,
            repository: None,
            license: None,
        };
        let config = MarketplaceConfig {
            name: "test".to_string(),
            source: MarketplaceSourceType::Github {
                repo: "test/marketplace".to_string(),
            },
            enabled: true,
        };

        let result = resolve_install_source(&entry, &config).unwrap();
        match result {
            InstallSource::GitUrl(url) => assert!(url.contains("owner/plugin")),
            _ => panic!("Expected GitUrl"),
        }
    }

    #[test]
    fn test_resolve_install_source_fallback_repository() {
        let entry = MarketplacePluginEntry {
            name: "test".to_string(),
            source: None,
            description: None,
            version: None,
            author: None,
            category: None,
            keywords: vec![],
            homepage: None,
            repository: Some("https://github.com/test/plugin.git".to_string()),
            license: None,
        };
        let config = MarketplaceConfig {
            name: "test".to_string(),
            source: MarketplaceSourceType::Github {
                repo: "test/marketplace".to_string(),
            },
            enabled: true,
        };

        let result = resolve_install_source(&entry, &config).unwrap();
        matches!(result, InstallSource::GitUrl(_));
    }
}
