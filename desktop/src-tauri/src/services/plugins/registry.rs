//! Plugin Registry
//!
//! Fetches the plugin registry from a remote URL.
//! Falls back to an embedded registry when the network is unavailable.

use crate::services::plugins::models::PluginRegistry;

/// Default registry URL.
pub const DEFAULT_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/plan-cascade/plugin-registry/main/registry.json";

/// Embedded fallback registry for offline use.
const FALLBACK_REGISTRY: &str = include_str!("fallback_registry.json");

/// Fetch the plugin registry from a remote URL.
///
/// Falls back to the embedded registry if the fetch fails.
pub async fn fetch_registry(url: Option<&str>) -> PluginRegistry {
    let registry_url = url.unwrap_or(DEFAULT_REGISTRY_URL);

    match fetch_remote_registry(registry_url).await {
        Ok(registry) => {
            eprintln!(
                "[plugins] Fetched registry: {} plugins from {}",
                registry.plugins.len(),
                registry_url
            );
            registry
        }
        Err(e) => {
            eprintln!(
                "[plugins] Failed to fetch registry from {}: {}. Using fallback.",
                registry_url, e
            );
            parse_fallback_registry()
        }
    }
}

/// Fetch registry from a remote URL via HTTP.
async fn fetch_remote_registry(url: &str) -> Result<PluginRegistry, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("plan-cascade-desktop")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}: {}", response.status(), url));
    }

    response
        .json::<PluginRegistry>()
        .await
        .map_err(|e| format!("Failed to parse registry JSON: {}", e))
}

/// Parse the embedded fallback registry.
fn parse_fallback_registry() -> PluginRegistry {
    serde_json::from_str(FALLBACK_REGISTRY).unwrap_or_else(|e| {
        eprintln!("[plugins] Failed to parse fallback registry: {}", e);
        PluginRegistry {
            version: "1.0.0".to_string(),
            updated_at: "unknown".to_string(),
            plugins: vec![],
            categories: vec![],
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_registry_parses() {
        let registry = parse_fallback_registry();
        assert_eq!(registry.version, "1.0.0");
        assert!(!registry.categories.is_empty());
    }

    #[test]
    fn test_default_registry_url() {
        assert!(DEFAULT_REGISTRY_URL.starts_with("https://"));
    }
}
