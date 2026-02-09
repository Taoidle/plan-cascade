//! Settings Models
//!
//! Application configuration and settings data structures.

use serde::{Deserialize, Serialize};

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
}

fn default_search_provider() -> String {
    "duckduckgo".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            language: "en".to_string(),
            default_provider: "anthropic".to_string(),
            default_model: "claude-sonnet-4-20250514".to_string(),
            analytics_enabled: true,
            auto_save_interval: 30,
            max_recent_projects: 10,
            debug_mode: false,
            search_provider: "duckduckgo".to_string(),
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
    pub analytics_enabled: Option<bool>,
    pub auto_save_interval: Option<u32>,
    pub max_recent_projects: Option<u32>,
    pub debug_mode: Option<bool>,
    pub search_provider: Option<String>,
}

impl AppConfig {
    /// Apply a partial update to the configuration
    pub fn apply_update(&mut self, update: SettingsUpdate) {
        if let Some(theme) = update.theme {
            self.theme = theme;
        }
        if let Some(language) = update.language {
            self.language = language;
        }
        if let Some(provider) = update.default_provider {
            self.default_provider = provider;
        }
        if let Some(model) = update.default_model {
            self.default_model = model;
        }
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
    }

    #[test]
    fn test_apply_update() {
        let mut config = AppConfig::default();
        let update = SettingsUpdate {
            theme: Some("dark".to_string()),
            language: Some("zh".to_string()),
            ..Default::default()
        };
        config.apply_update(update);
        assert_eq!(config.theme, "dark");
        assert_eq!(config.language, "zh");
        // Other fields should remain unchanged
        assert_eq!(config.default_provider, "anthropic");
    }

    #[test]
    fn test_validate_valid_config() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_theme() {
        let mut config = AppConfig::default();
        config.theme = "invalid".to_string();
        assert!(config.validate().is_err());
    }
}
