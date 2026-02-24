//! Secure Secret Storage
//!
//! Encrypted API key storage using AES-256-GCM with a locally-managed key.
//! Replaces OS-native keyring to avoid platform-specific permission dialogs
//! (e.g., macOS Keychain access prompts) while maintaining equivalent security.
//!
//! Storage layout (in ~/.plan-cascade/):
//!   .secret_key   — 32-byte random encryption key (permissions 0600)
//!   secrets.json  — { "provider": "base64(nonce || ciphertext)", ... }

use std::collections::HashMap;
use std::path::PathBuf;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::utils::error::{AppError, AppResult};
use crate::utils::paths::ensure_plan_cascade_dir;

const KEY_FILE: &str = ".secret_key";
const SECRETS_FILE: &str = "secrets.json";
const KEY_SIZE: usize = 32;
const NONCE_SIZE: usize = 12;

/// Internal encrypted storage engine.
struct SecureStore {
    cipher: Aes256Gcm,
    store_path: PathBuf,
}

impl SecureStore {
    /// Initialize the secure store: load or generate the encryption key.
    fn new(data_dir: &PathBuf) -> AppResult<Self> {
        let key_path = data_dir.join(KEY_FILE);

        let key_bytes = if key_path.exists() {
            let bytes = std::fs::read(&key_path)
                .map_err(|e| AppError::keyring(format!("Failed to read encryption key: {}", e)))?;
            if bytes.len() != KEY_SIZE {
                return Err(AppError::keyring(format!(
                    "Invalid encryption key size: expected {}, got {}",
                    KEY_SIZE,
                    bytes.len()
                )));
            }
            bytes
        } else {
            // First run: generate a random encryption key
            let mut key = vec![0u8; KEY_SIZE];
            OsRng.fill_bytes(&mut key);
            std::fs::write(&key_path, &key)
                .map_err(|e| AppError::keyring(format!("Failed to write encryption key: {}", e)))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| {
                        AppError::keyring(format!("Failed to set key file permissions: {}", e))
                    })?;
            }
            key
        };

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);

        Ok(Self {
            cipher,
            store_path: data_dir.join(SECRETS_FILE),
        })
    }

    /// Encrypt a plaintext string, returning base64(nonce || ciphertext).
    fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| AppError::keyring(format!("Encryption failed: {}", e)))?;

        // Prepend nonce to ciphertext for self-contained storage
        let mut combined = nonce_bytes.to_vec();
        combined.extend(ciphertext);

        Ok(BASE64.encode(combined))
    }

    /// Decrypt a base64-encoded value back to plaintext.
    fn decrypt(&self, encoded: &str) -> AppResult<String> {
        let data = BASE64
            .decode(encoded)
            .map_err(|e| AppError::keyring(format!("Base64 decode failed: {}", e)))?;

        if data.len() <= NONCE_SIZE {
            return Err(AppError::keyring("Invalid encrypted data: too short"));
        }

        let (nonce_bytes, ciphertext) = data.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::keyring(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext)
            .map_err(|e| AppError::keyring(format!("Decrypted data is not valid UTF-8: {}", e)))
    }

    /// Load the secrets map from disk.
    fn load(&self) -> HashMap<String, String> {
        if !self.store_path.exists() {
            return HashMap::new();
        }
        std::fs::read_to_string(&self.store_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist the secrets map to disk atomically (write-then-rename).
    fn save(&self, secrets: &HashMap<String, String>) -> AppResult<()> {
        let json = serde_json::to_string_pretty(secrets)
            .map_err(|e| AppError::keyring(format!("Failed to serialize secrets: {}", e)))?;

        // Atomic write: write to temp file then rename
        let tmp_path = self.store_path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json.as_bytes())
            .map_err(|e| AppError::keyring(format!("Failed to write secrets file: {}", e)))?;
        std::fs::rename(&tmp_path, &self.store_path)
            .map_err(|e| AppError::keyring(format!("Failed to finalize secrets file: {}", e)))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&self.store_path, std::fs::Permissions::from_mode(0o600));
        }

        Ok(())
    }
}

/// Secure secret storage service.
///
/// Drop-in replacement for the previous OS keyring-based implementation.
/// Uses AES-256-GCM encrypted file storage to avoid platform permission dialogs.
pub struct KeyringService {
    /// List of known provider names for enumeration
    known_providers: Vec<String>,
    /// Encrypted storage engine (None if initialization failed)
    inner: Option<SecureStore>,
}

impl KeyringService {
    /// Create a new secure storage service.
    ///
    /// On first run, generates a random encryption key in ~/.plan-cascade/.secret_key.
    /// If initialization fails (e.g., filesystem issues), the service degrades gracefully:
    /// all operations will return errors but the app continues to function.
    pub fn new() -> Self {
        let inner = ensure_plan_cascade_dir()
            .and_then(|dir| SecureStore::new(&dir))
            .map_err(|e| {
                tracing::warn!(
                    "Failed to initialize secure storage: {}. API key storage will be unavailable.",
                    e
                );
                e
            })
            .ok();

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
            inner,
        }
    }

    /// Get a reference to the secure store, or an error if not initialized.
    fn store(&self) -> AppResult<&SecureStore> {
        self.inner
            .as_ref()
            .ok_or_else(|| AppError::keyring("Secure storage not initialized"))
    }

    /// Store an API key for a provider
    pub fn set_api_key(&self, provider: &str, key: &str) -> AppResult<()> {
        let store = self.store()?;
        let encrypted = store.encrypt(key)?;
        let mut secrets = store.load();
        secrets.insert(provider.to_string(), encrypted);
        store.save(&secrets)
    }

    /// Retrieve an API key for a provider
    pub fn get_api_key(&self, provider: &str) -> AppResult<Option<String>> {
        let store = self.store()?;
        let secrets = store.load();
        match secrets.get(provider) {
            Some(encrypted) => Ok(Some(store.decrypt(encrypted)?)),
            None => Ok(None),
        }
    }

    /// Delete an API key for a provider
    pub fn delete_api_key(&self, provider: &str) -> AppResult<()> {
        let store = self.store()?;
        let mut secrets = store.load();
        if secrets.remove(provider).is_some() {
            store.save(&secrets)?;
        }
        Ok(())
    }

    /// List all known providers that have stored API keys
    pub fn list_providers(&self) -> AppResult<Vec<String>> {
        let store = self.store()?;
        let secrets = store.load();
        let mut providers = Vec::new();

        for provider in &self.known_providers {
            if secrets.contains_key(provider) {
                providers.push(provider.clone());
            }
        }

        Ok(providers)
    }

    /// Check if an API key exists for a provider
    pub fn has_api_key(&self, provider: &str) -> AppResult<bool> {
        let store = self.store()?;
        let secrets = store.load();
        Ok(secrets.contains_key(provider))
    }

    /// Check if the secure storage service is healthy
    pub fn is_healthy(&self) -> bool {
        self.inner.is_some()
    }
}

impl Default for KeyringService {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for KeyringService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyringService")
            .field("healthy", &self.inner.is_some())
            .field("known_providers", &self.known_providers)
            .finish()
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

    #[test]
    fn test_secure_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecureStore::new(&dir.path().to_path_buf()).unwrap();

        // Encrypt and decrypt
        let plaintext = "sk-test-key-12345";
        let encrypted = store.encrypt(plaintext).unwrap();
        let decrypted = store.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext, decrypted);

        // Different encryptions of the same plaintext produce different ciphertexts (random nonce)
        let encrypted2 = store.encrypt(plaintext).unwrap();
        assert_ne!(encrypted, encrypted2);

        // Both decrypt to the same value
        let decrypted2 = store.decrypt(&encrypted2).unwrap();
        assert_eq!(plaintext, decrypted2);
    }

    #[test]
    fn test_store_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecureStore::new(&dir.path().to_path_buf()).unwrap();

        // Save a secret
        let mut secrets = HashMap::new();
        let encrypted = store.encrypt("my-api-key").unwrap();
        secrets.insert("openai".to_string(), encrypted);
        store.save(&secrets).unwrap();

        // Reload and verify
        let loaded = store.load();
        assert!(loaded.contains_key("openai"));
        let decrypted = store.decrypt(loaded.get("openai").unwrap()).unwrap();
        assert_eq!("my-api-key", decrypted);
    }

    #[test]
    fn test_key_persistence_across_instances() {
        let dir = tempfile::tempdir().unwrap();

        // First instance: save a secret
        {
            let store = SecureStore::new(&dir.path().to_path_buf()).unwrap();
            let mut secrets = HashMap::new();
            secrets.insert("test".to_string(), store.encrypt("secret-value").unwrap());
            store.save(&secrets).unwrap();
        }

        // Second instance: should use the same encryption key and decrypt successfully
        {
            let store = SecureStore::new(&dir.path().to_path_buf()).unwrap();
            let secrets = store.load();
            let decrypted = store.decrypt(secrets.get("test").unwrap()).unwrap();
            assert_eq!("secret-value", decrypted);
        }
    }

    #[test]
    fn test_invalid_decrypt() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecureStore::new(&dir.path().to_path_buf()).unwrap();

        // Too short
        assert!(store.decrypt("AAAA").is_err());

        // Invalid base64
        assert!(store.decrypt("not-valid-base64!!!").is_err());

        // Valid base64 but wrong ciphertext (tampered)
        let encrypted = store.encrypt("test").unwrap();
        let mut data = BASE64.decode(&encrypted).unwrap();
        // Flip a byte in the ciphertext portion
        if let Some(byte) = data.get_mut(NONCE_SIZE + 1) {
            *byte ^= 0xFF;
        }
        let tampered = BASE64.encode(&data);
        assert!(store.decrypt(&tampered).is_err());
    }
}
