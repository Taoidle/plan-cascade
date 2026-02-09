//! JSON Configuration Management
//!
//! Handles reading and writing the application configuration file.

use std::fs;
use std::path::PathBuf;

use crate::models::settings::{AppConfig, SettingsUpdate};
use crate::utils::error::{AppError, AppResult};
use crate::utils::paths::{config_path, ensure_plan_cascade_dir};

/// Configuration service for managing app settings
#[derive(Debug)]
pub struct ConfigService {
    config_path: PathBuf,
    config: AppConfig,
}

impl ConfigService {
    /// Create a new config service, loading existing config or creating defaults
    pub fn new() -> AppResult<Self> {
        // Ensure the config directory exists
        ensure_plan_cascade_dir()?;

        let config_path = config_path()?;
        let config = if config_path.exists() {
            Self::load_from_file(&config_path)?
        } else {
            let default_config = AppConfig::default();
            Self::save_to_file(&config_path, &default_config)?;
            default_config
        };

        Ok(Self {
            config_path,
            config,
        })
    }

    /// Load configuration from a file
    fn load_from_file(path: &PathBuf) -> AppResult<AppConfig> {
        let content = fs::read_to_string(path)?;
        let config: AppConfig = serde_json::from_str(&content)?;
        config.validate().map_err(AppError::validation)?;
        Ok(config)
    }

    /// Save configuration to a file with pretty formatting
    fn save_to_file(path: &PathBuf, config: &AppConfig) -> AppResult<()> {
        config.validate().map_err(AppError::validation)?;
        let content = serde_json::to_string_pretty(config)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get the current configuration
    pub fn get_config(&self) -> &AppConfig {
        &self.config
    }

    /// Get a clone of the current configuration
    pub fn get_config_clone(&self) -> AppConfig {
        self.config.clone()
    }

    /// Update the configuration with a partial update
    pub fn update_config(&mut self, update: SettingsUpdate) -> AppResult<AppConfig> {
        self.config.apply_update(update);
        self.save()?;
        Ok(self.config.clone())
    }

    /// Save the current configuration to disk
    pub fn save(&self) -> AppResult<()> {
        Self::save_to_file(&self.config_path, &self.config)
    }

    /// Reload configuration from disk
    pub fn reload(&mut self) -> AppResult<()> {
        self.config = Self::load_from_file(&self.config_path)?;
        Ok(())
    }

    /// Reset configuration to defaults
    pub fn reset(&mut self) -> AppResult<()> {
        self.config = AppConfig::default();
        self.save()?;
        Ok(())
    }

    /// Check if the config service is healthy
    pub fn is_healthy(&self) -> bool {
        self.config_path.exists() && self.config.validate().is_ok()
    }
}

impl Default for ConfigService {
    fn default() -> Self {
        Self {
            config_path: PathBuf::new(),
            config: AppConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_config_file() -> (NamedTempFile, PathBuf) {
        let mut file = NamedTempFile::new().unwrap();
        let config = AppConfig::default();
        let content = serde_json::to_string_pretty(&config).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        let path = file.path().to_path_buf();
        (file, path)
    }

    #[test]
    fn test_load_config_from_file() {
        let (_file, path) = create_test_config_file();
        let config = ConfigService::load_from_file(&path).unwrap();
        assert_eq!(config.theme, "system");
    }

    #[test]
    fn test_save_config_to_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("config.json");
        let config = AppConfig::default();

        ConfigService::save_to_file(&path, &config).unwrap();

        assert!(path.exists());
        let loaded = ConfigService::load_from_file(&path).unwrap();
        assert_eq!(loaded.theme, config.theme);
    }

    #[test]
    fn test_config_update() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("config.json");
        let config = AppConfig::default();
        ConfigService::save_to_file(&path, &config).unwrap();

        let mut service = ConfigService {
            config_path: path,
            config,
        };

        let update = SettingsUpdate {
            theme: Some("dark".to_string()),
            ..Default::default()
        };

        let updated = service.update_config(update).unwrap();
        assert_eq!(updated.theme, "dark");
    }
}
