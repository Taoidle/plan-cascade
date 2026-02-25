//! Application State
//!
//! Global state managed by Tauri, containing all services.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::settings::{AppConfig, SettingsUpdate};
use crate::services::memory::ProjectMemoryStore;
use crate::services::orchestrator::embedding_service::EmbeddingService;
use crate::storage::{ConfigService, Database, KeyringService};
use crate::utils::error::{AppError, AppResult};

/// Application state managed by Tauri
pub struct AppState {
    /// SQLite database with connection pool
    database: Arc<RwLock<Option<Database>>>,
    /// Keyring service for secure secret storage
    keyring: Arc<RwLock<Option<KeyringService>>>,
    /// Configuration service for app settings
    config: Arc<RwLock<Option<ConfigService>>>,
    /// Project memory store for cross-session persistent memory
    memory_store: Arc<RwLock<Option<ProjectMemoryStore>>>,
    /// Whether the state has been initialized
    initialized: Arc<RwLock<bool>>,
}

impl AppState {
    /// Create a new uninitialized app state
    pub fn new() -> Self {
        Self {
            database: Arc::new(RwLock::new(None)),
            keyring: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(None)),
            memory_store: Arc::new(RwLock::new(None)),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// Initialize all services
    pub async fn initialize(&self) -> AppResult<()> {
        let mut initialized = self.initialized.write().await;
        if *initialized {
            return Ok(());
        }

        // Initialize database
        {
            let db = Database::new()?;
            let mut db_lock = self.database.write().await;
            *db_lock = Some(db);
        }

        // Initialize keyring
        {
            let keyring = KeyringService::new();
            let mut keyring_lock = self.keyring.write().await;
            *keyring_lock = Some(keyring);
        }

        // Initialize config
        {
            let config = ConfigService::new()?;
            let mut config_lock = self.config.write().await;
            *config_lock = Some(config);
        }

        // Initialize memory store using the database pool
        {
            let db_guard = self.database.read().await;
            if let Some(ref db) = *db_guard {
                let embedding_service = Arc::new(EmbeddingService::new());
                let store = ProjectMemoryStore::from_database(db, embedding_service);
                let mut store_lock = self.memory_store.write().await;
                *store_lock = Some(store);
            }
        }

        *initialized = true;
        Ok(())
    }

    /// Check if database is healthy
    pub fn is_database_healthy(&self) -> bool {
        // Use try_read to avoid blocking
        if let Ok(guard) = self.database.try_read() {
            if let Some(ref db) = *guard {
                return db.is_healthy();
            }
        }
        false
    }

    /// Check if keyring is healthy
    pub fn is_keyring_healthy(&self) -> bool {
        if let Ok(guard) = self.keyring.try_read() {
            if let Some(ref keyring) = *guard {
                return keyring.is_healthy();
            }
        }
        false
    }

    /// Check if config is healthy
    pub fn is_config_healthy(&self) -> bool {
        if let Ok(guard) = self.config.try_read() {
            if let Some(ref config) = *guard {
                return config.is_healthy();
            }
        }
        false
    }

    /// Get the current configuration
    pub async fn get_config(&self) -> AppResult<AppConfig> {
        let guard = self.config.read().await;
        match &*guard {
            Some(config) => Ok(config.get_config_clone()),
            None => Err(AppError::config("Config service not initialized")),
        }
    }

    /// Update the configuration
    pub async fn update_config(&self, update: SettingsUpdate) -> AppResult<AppConfig> {
        let mut guard = self.config.write().await;
        match &mut *guard {
            Some(config) => config.update_config(update),
            None => Err(AppError::config("Config service not initialized")),
        }
    }

    /// Get an API key from the keyring
    pub async fn get_api_key(&self, provider: &str) -> AppResult<Option<String>> {
        let guard = self.keyring.read().await;
        match &*guard {
            Some(keyring) => keyring.get_api_key(provider),
            None => Err(AppError::keyring("Keyring service not initialized")),
        }
    }

    /// Set an API key in the keyring
    pub async fn set_api_key(&self, provider: &str, key: &str) -> AppResult<()> {
        let guard = self.keyring.read().await;
        match &*guard {
            Some(keyring) => keyring.set_api_key(provider, key),
            None => Err(AppError::keyring("Keyring service not initialized")),
        }
    }

    /// Delete an API key from the keyring
    pub async fn delete_api_key(&self, provider: &str) -> AppResult<()> {
        let guard = self.keyring.read().await;
        match &*guard {
            Some(keyring) => keyring.delete_api_key(provider),
            None => Err(AppError::keyring("Keyring service not initialized")),
        }
    }

    /// List providers with stored API keys
    pub async fn list_api_key_providers(&self) -> AppResult<Vec<String>> {
        let guard = self.keyring.read().await;
        match &*guard {
            Some(keyring) => keyring.list_providers(),
            None => Err(AppError::keyring("Keyring service not initialized")),
        }
    }

    /// Export all stored API keys as decrypted plaintext
    pub async fn export_all_secrets(&self) -> AppResult<std::collections::HashMap<String, String>> {
        let guard = self.keyring.read().await;
        match &*guard {
            Some(keyring) => keyring.export_all_decrypted(),
            None => Err(AppError::keyring("Keyring service not initialized")),
        }
    }

    /// Import API keys from a plaintext map, re-encrypting with the internal key
    pub async fn import_all_secrets(
        &self,
        secrets: &std::collections::HashMap<String, String>,
    ) -> AppResult<()> {
        let guard = self.keyring.read().await;
        match &*guard {
            Some(keyring) => keyring.import_all(secrets),
            None => Err(AppError::keyring("Keyring service not initialized")),
        }
    }

    /// Get mutable config service access for settings import
    pub async fn with_config_mut<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&mut ConfigService) -> AppResult<T>,
    {
        let mut guard = self.config.write().await;
        match &mut *guard {
            Some(config) => f(config),
            None => Err(AppError::config("Config service not initialized")),
        }
    }

    /// Get database access for direct queries
    pub async fn with_database<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&Database) -> AppResult<T>,
    {
        let guard = self.database.read().await;
        match &*guard {
            Some(db) => f(db),
            None => Err(AppError::database("Database not initialized")),
        }
    }

    /// Get memory store access for memory operations
    pub async fn with_memory_store<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&ProjectMemoryStore) -> AppResult<T>,
    {
        let guard = self.memory_store.read().await;
        match &*guard {
            Some(store) => f(store),
            None => Err(AppError::Internal(
                "Memory store not initialized".to_string(),
            )),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("initialized", &self.initialized)
            .finish()
    }
}
