//! Keyring Integration
//!
//! Secure API key storage using OS-native keyring (Credential Manager, Keychain, Secret Service).

use keyring::Entry;

use crate::utils::error::{AppError, AppResult};

/// Service name used for keyring entries
const SERVICE_NAME: &str = "plan-cascade";

/// Keyring service for secure secret storage
#[derive(Debug, Default)]
pub struct KeyringService {
    /// List of known provider names
    known_providers: Vec<String>,
}

impl KeyringService {
    /// Create a new keyring service
    pub fn new() -> Self {
        Self {
            known_providers: vec![
                "anthropic".to_string(),
                "openai".to_string(),
                "deepseek".to_string(),
                "glm".to_string(),
                "qwen".to_string(),
                "google".to_string(),
                "ollama".to_string(),
                "tavily".to_string(),
                "brave_search".to_string(),
            ],
        }
    }

    /// Store an API key for a provider
    pub fn set_api_key(&self, provider: &str, key: &str) -> AppResult<()> {
        let entry = Entry::new(SERVICE_NAME, provider)
            .map_err(|e| AppError::keyring(format!("Failed to create keyring entry: {}", e)))?;

        entry
            .set_password(key)
            .map_err(|e| AppError::keyring(format!("Failed to store API key: {}", e)))?;

        Ok(())
    }

    /// Retrieve an API key for a provider
    pub fn get_api_key(&self, provider: &str) -> AppResult<Option<String>> {
        let entry = Entry::new(SERVICE_NAME, provider)
            .map_err(|e| AppError::keyring(format!("Failed to create keyring entry: {}", e)))?;

        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::keyring(format!(
                "Failed to retrieve API key: {}",
                e
            ))),
        }
    }

    /// Delete an API key for a provider
    pub fn delete_api_key(&self, provider: &str) -> AppResult<()> {
        let entry = Entry::new(SERVICE_NAME, provider)
            .map_err(|e| AppError::keyring(format!("Failed to create keyring entry: {}", e)))?;

        match entry.delete_credential() {
            Ok(_) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // Already deleted, not an error
            Err(e) => Err(AppError::keyring(format!(
                "Failed to delete API key: {}",
                e
            ))),
        }
    }

    /// List all providers that have stored API keys
    pub fn list_providers(&self) -> AppResult<Vec<String>> {
        let mut providers = Vec::new();

        for provider in &self.known_providers {
            if let Ok(Some(_)) = self.get_api_key(provider) {
                providers.push(provider.clone());
            }
        }

        Ok(providers)
    }

    /// Check if an API key exists for a provider
    pub fn has_api_key(&self, provider: &str) -> AppResult<bool> {
        Ok(self.get_api_key(provider)?.is_some())
    }

    /// Check if the keyring service is healthy (can access the keyring)
    pub fn is_healthy(&self) -> bool {
        // Try to create an entry to verify keyring access
        Entry::new(SERVICE_NAME, "health_check").is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyring_service_creation() {
        let service = KeyringService::new();
        assert!(!service.known_providers.is_empty());
    }

    // Note: Integration tests for actual keyring operations require
    // platform-specific setup and are skipped in unit tests.
    // They should be run manually or in CI with proper keyring access.
}
