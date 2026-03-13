//! Settings Models
//!
//! Application configuration and settings data structures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderEndpoint {
    pub id: String,
    pub name: String,
    pub base_url: String,
}

/// Application configuration stored in config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// UI theme: "light", "dark", or "system"
    pub theme: String,
    /// Language code (e.g., "en", "zh")
    pub language: String,
    /// Default LLM provider
    pub default_provider: String,
    /// Default model for the provider
    pub default_model: String,
    /// Per-provider last selected model (backend source of truth for binding).
    #[serde(default = "default_model_by_provider")]
    pub model_by_provider: HashMap<String, String>,
    /// GLM endpoint selection persisted for backend-only workflows.
    #[serde(default = "default_glm_endpoint")]
    pub glm_endpoint: String,
    /// MiniMax endpoint selection persisted for backend-only workflows.
    #[serde(default = "default_minimax_endpoint")]
    pub minimax_endpoint: String,
    /// Qwen endpoint selection persisted for backend-only workflows.
    #[serde(default = "default_qwen_endpoint")]
    pub qwen_endpoint: String,
    /// Explicit provider base URL overrides keyed by canonical provider name.
    #[serde(default)]
    pub custom_provider_base_urls: HashMap<String, String>,
    /// Full custom endpoint catalog per provider.
    #[serde(default)]
    pub custom_provider_endpoints: HashMap<String, Vec<CustomProviderEndpoint>>,
    /// Selected custom endpoint ID per provider.
    #[serde(default)]
    pub selected_custom_provider_endpoint_ids: HashMap<String, String>,
    /// Enable analytics tracking
    pub analytics_enabled: bool,
    /// Auto-save interval in seconds
    pub auto_save_interval: u32,
    /// Maximum recent projects to show
    pub max_recent_projects: u32,
    /// Enable debug mode
    pub debug_mode: bool,
    /// Web search provider: "tavily", "brave", or "duckduckgo"
    #[serde(default = "default_search_provider")]
    pub search_provider: String,
    /// Whether clicking the window close button keeps the app running in background.
    #[serde(default = "default_true")]
    pub close_to_background_enabled: bool,
    /// Whether deleting a workflow session should also delete its managed worktree by default.
    #[serde(default)]
    pub worktree_auto_cleanup_on_session_delete: bool,
}

fn default_search_provider() -> String {
    "duckduckgo".to_string()
}

fn default_true() -> bool {
    true
}

fn default_model_by_provider() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert(
        "anthropic".to_string(),
        "claude-sonnet-4-6-20260219".to_string(),
    );
    map
}

fn default_glm_endpoint() -> String {
    "standard".to_string()
}

fn default_minimax_endpoint() -> String {
    "international".to_string()
}

fn default_qwen_endpoint() -> String {
    "china".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            language: "en".to_string(),
            default_provider: "anthropic".to_string(),
            default_model: "claude-sonnet-4-6-20260219".to_string(),
            model_by_provider: default_model_by_provider(),
            glm_endpoint: default_glm_endpoint(),
            minimax_endpoint: default_minimax_endpoint(),
            qwen_endpoint: default_qwen_endpoint(),
            custom_provider_base_urls: HashMap::new(),
            custom_provider_endpoints: HashMap::new(),
            selected_custom_provider_endpoint_ids: HashMap::new(),
            analytics_enabled: true,
            auto_save_interval: 30,
            max_recent_projects: 10,
            debug_mode: false,
            search_provider: "duckduckgo".to_string(),
            close_to_background_enabled: true,
            worktree_auto_cleanup_on_session_delete: false,
        }
    }
}

/// Settings update request (partial update)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SettingsUpdate {
    pub theme: Option<String>,
    pub language: Option<String>,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub model_by_provider: Option<HashMap<String, String>>,
    pub glm_endpoint: Option<String>,
    pub minimax_endpoint: Option<String>,
    pub qwen_endpoint: Option<String>,
    pub custom_provider_base_urls: Option<HashMap<String, String>>,
    pub custom_provider_endpoints: Option<HashMap<String, Vec<CustomProviderEndpoint>>>,
    pub selected_custom_provider_endpoint_ids: Option<HashMap<String, String>>,
    pub analytics_enabled: Option<bool>,
    pub auto_save_interval: Option<u32>,
    pub max_recent_projects: Option<u32>,
    pub debug_mode: Option<bool>,
    pub search_provider: Option<String>,
    pub close_to_background_enabled: Option<bool>,
    pub worktree_auto_cleanup_on_session_delete: Option<bool>,
}

impl AppConfig {
    fn canonical_provider_name(provider: &str) -> String {
        provider.trim().to_ascii_lowercase()
    }

    fn normalize_custom_provider_endpoints(
        endpoints: HashMap<String, Vec<CustomProviderEndpoint>>,
    ) -> HashMap<String, Vec<CustomProviderEndpoint>> {
        let mut normalized = HashMap::new();
        for (provider, entries) in endpoints {
            let canonical = Self::canonical_provider_name(&provider);
            if canonical.is_empty() {
                continue;
            }

            let valid_entries = entries
                .into_iter()
                .filter_map(|entry| {
                    let id = entry.id.trim().to_string();
                    let name = entry.name.trim().to_string();
                    let base_url = entry.base_url.trim().to_string();
                    if id.is_empty() || base_url.is_empty() {
                        return None;
                    }
                    Some(CustomProviderEndpoint {
                        id,
                        name: if name.is_empty() {
                            "Custom endpoint".to_string()
                        } else {
                            name
                        },
                        base_url,
                    })
                })
                .collect::<Vec<_>>();

            if !valid_entries.is_empty() {
                normalized.insert(canonical, valid_entries);
            }
        }
        normalized
    }

    fn normalize_selected_custom_provider_endpoint_ids(
        selected_ids: HashMap<String, String>,
        endpoints: &HashMap<String, Vec<CustomProviderEndpoint>>,
    ) -> HashMap<String, String> {
        let mut normalized = HashMap::new();
        for (provider, endpoint_id) in selected_ids {
            let canonical = Self::canonical_provider_name(&provider);
            let trimmed_id = endpoint_id.trim().to_string();
            if canonical.is_empty() || trimmed_id.is_empty() {
                continue;
            }
            let exists = endpoints
                .get(&canonical)
                .map(|entries| entries.iter().any(|entry| entry.id == trimmed_id))
                .unwrap_or(false);
            if exists {
                normalized.insert(canonical, trimmed_id);
            }
        }
        normalized
    }

    fn build_custom_provider_base_urls(
        endpoints: &HashMap<String, Vec<CustomProviderEndpoint>>,
        selected_ids: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut base_urls = HashMap::new();
        for (provider, endpoint_id) in selected_ids {
            if let Some(endpoint) = endpoints
                .get(provider)
                .and_then(|entries| entries.iter().find(|entry| entry.id == *endpoint_id))
            {
                base_urls.insert(provider.clone(), endpoint.base_url.clone());
            }
        }
        base_urls
    }

    fn derive_endpoint_catalog_from_base_urls(
        base_urls: &HashMap<String, String>,
    ) -> (
        HashMap<String, Vec<CustomProviderEndpoint>>,
        HashMap<String, String>,
    ) {
        let mut endpoints = HashMap::new();
        let mut selected_ids = HashMap::new();
        for (provider, base_url) in base_urls {
            let trimmed = base_url.trim();
            if trimmed.is_empty() {
                continue;
            }
            let endpoint_id = format!("legacy-{}", provider);
            endpoints.insert(
                provider.clone(),
                vec![CustomProviderEndpoint {
                    id: endpoint_id.clone(),
                    name: "Custom endpoint".to_string(),
                    base_url: trimmed.to_string(),
                }],
            );
            selected_ids.insert(provider.clone(), endpoint_id);
        }
        (endpoints, selected_ids)
    }

    pub fn normalize_persistence_fields(&mut self) {
        self.custom_provider_base_urls = self
            .custom_provider_base_urls
            .iter()
            .filter_map(|(provider, base_url)| {
                let canonical = Self::canonical_provider_name(provider);
                let trimmed = base_url.trim().to_string();
                if canonical.is_empty() || trimmed.is_empty() {
                    None
                } else {
                    Some((canonical, trimmed))
                }
            })
            .collect();

        self.custom_provider_endpoints = Self::normalize_custom_provider_endpoints(std::mem::take(
            &mut self.custom_provider_endpoints,
        ));
        self.selected_custom_provider_endpoint_ids =
            Self::normalize_selected_custom_provider_endpoint_ids(
                std::mem::take(&mut self.selected_custom_provider_endpoint_ids),
                &self.custom_provider_endpoints,
            );

        if self.custom_provider_endpoints.is_empty() && !self.custom_provider_base_urls.is_empty() {
            let (endpoints, selected_ids) =
                Self::derive_endpoint_catalog_from_base_urls(&self.custom_provider_base_urls);
            self.custom_provider_endpoints = endpoints;
            self.selected_custom_provider_endpoint_ids = selected_ids;
        }

        if self.custom_provider_base_urls.is_empty() && !self.custom_provider_endpoints.is_empty() {
            self.custom_provider_base_urls = Self::build_custom_provider_base_urls(
                &self.custom_provider_endpoints,
                &self.selected_custom_provider_endpoint_ids,
            );
        }
    }

    /// Resolve a provider-specific model from `model_by_provider`, falling back to
    /// `default_model` for the current `default_provider`.
    pub fn model_for_provider(&self, provider: &str) -> String {
        let canonical = Self::canonical_provider_name(provider);
        if let Some(model) = self
            .model_by_provider
            .get(&canonical)
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        {
            return model.to_string();
        }
        if canonical == Self::canonical_provider_name(&self.default_provider) {
            return self.default_model.clone();
        }
        String::new()
    }

    pub fn provider_base_url(&self, provider: &str) -> Option<String> {
        let canonical = Self::canonical_provider_name(provider);
        if let Some(custom) = self
            .custom_provider_base_urls
            .get(&canonical)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            return Some(custom.to_string());
        }

        match canonical.as_str() {
            "glm" => match self.glm_endpoint.as_str() {
                "coding" => {
                    Some("https://open.bigmodel.cn/api/coding/paas/v4/chat/completions".to_string())
                }
                "international" => {
                    Some("https://api.z.ai/api/paas/v4/chat/completions".to_string())
                }
                "international-coding" => {
                    Some("https://api.z.ai/api/coding/paas/v4/chat/completions".to_string())
                }
                _ => None,
            },
            "minimax" => match self.minimax_endpoint.as_str() {
                "china" => Some("https://api.minimaxi.com/v1/chat/completions".to_string()),
                _ => None,
            },
            "qwen" => match self.qwen_endpoint.as_str() {
                "singapore" => Some(
                    "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions"
                        .to_string(),
                ),
                "us" => Some(
                    "https://dashscope-us.aliyuncs.com/compatible-mode/v1/chat/completions"
                        .to_string(),
                ),
                _ => None,
            },
            _ => None,
        }
    }

    /// Apply a partial update to the configuration
    pub fn apply_update(&mut self, update: SettingsUpdate) {
        if let Some(theme) = update.theme {
            self.theme = theme;
        }
        if let Some(language) = update.language {
            self.language = language;
        }
        if let Some(provider) = update.default_provider {
            self.default_provider = Self::canonical_provider_name(&provider);
            if let Some(provider_model) = self
                .model_by_provider
                .get(&self.default_provider)
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
            {
                self.default_model = provider_model.to_string();
            }
        }
        if let Some(model_map) = update.model_by_provider {
            let mut normalized = HashMap::new();
            for (provider, model) in model_map {
                let canonical = Self::canonical_provider_name(&provider);
                let trimmed = model.trim().to_string();
                if !trimmed.is_empty() {
                    normalized.insert(canonical, trimmed);
                }
            }
            self.model_by_provider = normalized;
            if let Some(provider_model) = self
                .model_by_provider
                .get(&self.default_provider)
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
            {
                self.default_model = provider_model.to_string();
            }
        }
        if let Some(model) = update.default_model {
            self.default_model = model;
            let canonical = Self::canonical_provider_name(&self.default_provider);
            if !self.default_model.trim().is_empty() {
                self.model_by_provider
                    .insert(canonical, self.default_model.clone());
            }
        }
        if let Some(endpoint) = update.glm_endpoint {
            self.glm_endpoint = endpoint;
        }
        if let Some(endpoint) = update.minimax_endpoint {
            self.minimax_endpoint = endpoint;
        }
        if let Some(endpoint) = update.qwen_endpoint {
            self.qwen_endpoint = endpoint;
        }
        if let Some(base_urls) = update.custom_provider_base_urls {
            let mut normalized = HashMap::new();
            for (provider, base_url) in base_urls {
                let canonical = Self::canonical_provider_name(&provider);
                let trimmed = base_url.trim().to_string();
                if !canonical.is_empty() && !trimmed.is_empty() {
                    normalized.insert(canonical, trimmed);
                }
            }
            self.custom_provider_base_urls = normalized;
        }
        if let Some(endpoints) = update.custom_provider_endpoints {
            self.custom_provider_endpoints = Self::normalize_custom_provider_endpoints(endpoints);
        }
        if let Some(selected_ids) = update.selected_custom_provider_endpoint_ids {
            self.selected_custom_provider_endpoint_ids =
                Self::normalize_selected_custom_provider_endpoint_ids(
                    selected_ids,
                    &self.custom_provider_endpoints,
                );
        }
        if self.custom_provider_endpoints.is_empty() && !self.custom_provider_base_urls.is_empty() {
            let (endpoints, selected_ids) =
                Self::derive_endpoint_catalog_from_base_urls(&self.custom_provider_base_urls);
            self.custom_provider_endpoints = endpoints;
            self.selected_custom_provider_endpoint_ids = selected_ids;
        }
        if self.custom_provider_base_urls.is_empty() && !self.custom_provider_endpoints.is_empty() {
            self.custom_provider_base_urls = Self::build_custom_provider_base_urls(
                &self.custom_provider_endpoints,
                &self.selected_custom_provider_endpoint_ids,
            );
        }
        self.normalize_persistence_fields();
        if let Some(enabled) = update.analytics_enabled {
            self.analytics_enabled = enabled;
        }
        if let Some(interval) = update.auto_save_interval {
            self.auto_save_interval = interval;
        }
        if let Some(max) = update.max_recent_projects {
            self.max_recent_projects = max;
        }
        if let Some(debug) = update.debug_mode {
            self.debug_mode = debug;
        }
        if let Some(search_provider) = update.search_provider {
            self.search_provider = search_provider;
        }
        if let Some(close_to_background_enabled) = update.close_to_background_enabled {
            self.close_to_background_enabled = close_to_background_enabled;
        }
        if let Some(enabled) = update.worktree_auto_cleanup_on_session_delete {
            self.worktree_auto_cleanup_on_session_delete = enabled;
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate theme
        if !["light", "dark", "system"].contains(&self.theme.as_str()) {
            return Err(format!(
                "Invalid theme: {}. Must be 'light', 'dark', or 'system'",
                self.theme
            ));
        }

        // Validate language (basic check)
        if self.language.len() < 2 || self.language.len() > 5 {
            return Err(format!("Invalid language code: {}", self.language));
        }

        // Validate auto_save_interval
        if self.auto_save_interval < 5 {
            return Err("auto_save_interval must be at least 5 seconds".to_string());
        }

        // Validate max_recent_projects
        if self.max_recent_projects > 100 {
            return Err("max_recent_projects cannot exceed 100".to_string());
        }

        if self.default_provider.trim().is_empty() {
            return Err("default_provider cannot be empty".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.theme, "system");
        assert_eq!(config.language, "en");
        assert_eq!(config.default_provider, "anthropic");
        assert_eq!(config.default_model, "claude-sonnet-4-6-20260219");
        assert_eq!(config.glm_endpoint, "standard");
        assert_eq!(config.minimax_endpoint, "international");
        assert_eq!(config.qwen_endpoint, "china");
        assert!(config.custom_provider_base_urls.is_empty());
        assert!(config.custom_provider_endpoints.is_empty());
        assert!(config.selected_custom_provider_endpoint_ids.is_empty());
        assert!(config.close_to_background_enabled);
        assert_eq!(
            config.model_by_provider.get("anthropic"),
            Some(&"claude-sonnet-4-6-20260219".to_string())
        );
    }

    #[test]
    fn test_apply_update() {
        let mut config = AppConfig::default();
        let update = SettingsUpdate {
            theme: Some("dark".to_string()),
            language: Some("zh".to_string()),
            default_provider: Some("openai".to_string()),
            default_model: Some("gpt-5.1".to_string()),
            minimax_endpoint: Some("china".to_string()),
            ..Default::default()
        };
        config.apply_update(update);
        assert_eq!(config.theme, "dark");
        assert_eq!(config.language, "zh");
        assert_eq!(config.default_provider, "openai");
        assert_eq!(config.default_model, "gpt-5.1");
        assert_eq!(config.minimax_endpoint, "china");
        assert!(config.close_to_background_enabled);
        assert_eq!(
            config.model_by_provider.get("openai"),
            Some(&"gpt-5.1".to_string())
        );
        // Other fields should remain unchanged
        assert_eq!(config.search_provider, "duckduckgo");
    }

    #[test]
    fn test_apply_update_model_map_drives_default_model() {
        let mut config = AppConfig::default();
        let mut map = HashMap::new();
        map.insert("qwen".to_string(), "qwen3-plus".to_string());
        let update = SettingsUpdate {
            default_provider: Some("qwen".to_string()),
            model_by_provider: Some(map),
            ..Default::default()
        };
        config.apply_update(update);
        assert_eq!(config.default_provider, "qwen");
        assert_eq!(config.default_model, "qwen3-plus");
        assert_eq!(config.model_for_provider("qwen"), "qwen3-plus");
    }

    #[test]
    fn test_validate_valid_config() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_provider_base_url_from_endpoint_settings() {
        let mut config = AppConfig::default();
        config.minimax_endpoint = "china".to_string();
        config.glm_endpoint = "international".to_string();
        config.qwen_endpoint = "us".to_string();
        assert_eq!(
            config.provider_base_url("minimax").as_deref(),
            Some("https://api.minimaxi.com/v1/chat/completions")
        );
        assert_eq!(
            config.provider_base_url("glm").as_deref(),
            Some("https://api.z.ai/api/paas/v4/chat/completions")
        );
        assert_eq!(
            config.provider_base_url("qwen").as_deref(),
            Some("https://dashscope-us.aliyuncs.com/compatible-mode/v1/chat/completions")
        );
    }

    #[test]
    fn test_provider_base_url_prefers_custom_override() {
        let mut config = AppConfig::default();
        config.custom_provider_base_urls.insert(
            "openai".to_string(),
            "https://gateway.example.com/v1/chat/completions".to_string(),
        );
        config.custom_provider_base_urls.insert(
            "glm".to_string(),
            "https://glm.example.com/chat/completions".to_string(),
        );
        config.glm_endpoint = "international".to_string();

        assert_eq!(
            config.provider_base_url("openai").as_deref(),
            Some("https://gateway.example.com/v1/chat/completions")
        );
        assert_eq!(
            config.provider_base_url("glm").as_deref(),
            Some("https://glm.example.com/chat/completions")
        );
    }

    #[test]
    fn test_normalize_persistence_fields_derives_catalog_from_legacy_base_urls() {
        let mut config = AppConfig::default();
        config.custom_provider_base_urls.insert(
            "openai".to_string(),
            "https://gateway.example.com/v1/chat/completions".to_string(),
        );

        config.normalize_persistence_fields();

        assert_eq!(
            config.selected_custom_provider_endpoint_ids.get("openai"),
            Some(&"legacy-openai".to_string())
        );
        assert_eq!(
            config
                .custom_provider_endpoints
                .get("openai")
                .and_then(|entries| entries.first())
                .map(|entry| entry.base_url.as_str()),
            Some("https://gateway.example.com/v1/chat/completions")
        );
    }

    #[test]
    fn test_validate_invalid_theme() {
        let mut config = AppConfig::default();
        config.theme = "invalid".to_string();
        assert!(config.validate().is_err());
    }
}
