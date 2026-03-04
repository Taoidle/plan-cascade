//! Secure Secret Storage
//!
//! Encrypted API key storage using AES-256-GCM with a locally-managed key.
//! Replaces OS-native keyring to avoid platform-specific permission dialogs
//! (e.g., macOS Keychain access prompts) while maintaining equivalent security.
//!
//! Storage layout (in ~/.plan-cascade/):
//!   .secret_key   — versioned encryption key payload (permissions 0600)
//!   secrets.json  — { "version": 2, "secrets": { "provider": "base64(nonce || ciphertext)" } }

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
const KEY_FILE_MAGIC: &[u8] = b"PCSK1\0";
const SECRETS_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SecretsFilePayload {
    version: u32,
    secrets: HashMap<String, String>,
}

/// Internal encrypted storage engine.
struct SecureStore {
    cipher: Aes256Gcm,
    store_path: PathBuf,
    key_path: PathBuf,
    read_only: bool,
}

impl SecureStore {
    /// Initialize the secure store: load or generate the encryption key.
    fn new(data_dir: &PathBuf) -> AppResult<Self> {
        let key_path = data_dir.join(KEY_FILE);
        let store_path = data_dir.join(SECRETS_FILE);
        let mut read_only = false;

        let key_bytes = if key_path.exists() {
            let bytes = std::fs::read(&key_path).map_err(|e| {
                AppError::keyring(format!("Failed to read encryption key file: {}", e))
            })?;
            Self::decode_key_bytes(&bytes)?
        } else {
            // First run: generate a random encryption key
            let mut key = vec![0u8; KEY_SIZE];
            OsRng.fill_bytes(&mut key);
            Self::write_key_file_atomic(&key_path, &Self::encode_key_bytes(&key))?;
            key
        };

        if !Self::is_secure_permissions(&key_path)? {
            tracing::warn!(
                path = %key_path.display(),
                "Insecure key file permissions detected; secure storage is now read-only"
            );
            read_only = true;
        }

        if store_path.exists() {
            if !Self::is_secure_permissions(&store_path)? {
                tracing::warn!(
                    path = %store_path.display(),
                    "Insecure secrets file permissions detected; secure storage is now read-only"
                );
                read_only = true;
            }
            if let Err(e) = Self::load_plaintext_secrets_from_file(&store_path) {
                tracing::warn!(
                    path = %store_path.display(),
                    error = %e,
                    "Secrets file integrity check failed; secure storage is now read-only"
                );
                read_only = true;
            }
        }

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);

        Ok(Self {
            cipher,
            store_path,
            key_path,
            read_only,
        })
    }

    /// Encrypt a plaintext string, returning base64(nonce || ciphertext).
    fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        Self::encrypt_with_cipher(&self.cipher, plaintext)
    }

    fn encrypt_with_cipher(cipher: &Aes256Gcm, plaintext: &str) -> AppResult<String> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
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

    /// Load plaintext secret payload from disk.
    fn load_plaintext_secrets_from_file(path: &Path) -> AppResult<HashMap<String, String>> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AppError::keyring(format!("Failed to read secrets file: {}", e)))?;
        if content.trim().is_empty() {
            return Ok(HashMap::new());
        }

        if let Ok(payload) = serde_json::from_str::<SecretsFilePayload>(&content) {
            if payload.version == 0 || payload.version > SECRETS_SCHEMA_VERSION {
                return Err(AppError::keyring(format!(
                    "Unsupported secrets payload version: {}",
                    payload.version
                )));
            }
            return Ok(payload.secrets);
        }

        // Backward compatibility for legacy payload: flat map without version header.
        if let Ok(legacy) = serde_json::from_str::<HashMap<String, String>>(&content) {
            return Ok(legacy);
        }

        Err(AppError::keyring(
            "Secrets file has invalid JSON payload; refusing to mutate storage",
        ))
    }

    /// Load the encrypted secrets map from disk.
    fn load(&self) -> HashMap<String, String> {
        if !self.store_path.exists() {
            return HashMap::new();
        }
        match Self::load_plaintext_secrets_from_file(&self.store_path) {
            Ok(secrets) => secrets,
            Err(e) => {
                tracing::warn!(
                    path = %self.store_path.display(),
                    error = %e,
                    "Failed to parse secrets payload"
                );
                HashMap::new()
            }
        }
    }

    fn ensure_writable(&self) -> AppResult<()> {
        if self.read_only {
            return Err(AppError::keyring(
                "Secure storage is read-only due to insecure file permissions or integrity issues",
            ));
        }
        Ok(())
    }

    /// Persist the secrets map to disk atomically (write-then-rename).
    fn save(&self, secrets: &HashMap<String, String>) -> AppResult<()> {
        self.ensure_writable()?;

        let payload = SecretsFilePayload {
            version: SECRETS_SCHEMA_VERSION,
            secrets: secrets.clone(),
        };
        let json = serde_json::to_string_pretty(&payload)
            .map_err(|e| AppError::keyring(format!("Failed to serialize secrets: {}", e)))?;

        // Atomic write with best-effort rollback.
        let tmp_path = self.store_path.with_extension("json.tmp");
        let backup_path = self.store_path.with_extension("json.bak");
        let had_original = self.store_path.exists();

        if had_original {
            if let Err(e) = std::fs::copy(&self.store_path, &backup_path) {
                tracing::warn!(
                    path = %self.store_path.display(),
                    backup = %backup_path.display(),
                    error = %e,
                    "Failed to create backup before secrets write"
                );
            }
        }

        std::fs::write(&tmp_path, json.as_bytes()).map_err(|e| {
            AppError::keyring(format!(
                "Failed to write temporary secrets file '{}': {}",
                tmp_path.display(),
                e
            ))
        })?;
        Self::set_secure_permissions(&tmp_path)?;

        if let Err(e) = std::fs::rename(&tmp_path, &self.store_path) {
            tracing::error!(
                path = %self.store_path.display(),
                tmp = %tmp_path.display(),
                error = %e,
                "Failed to finalize secrets file write"
            );
            if backup_path.exists() {
                let _ = std::fs::copy(&backup_path, &self.store_path);
            }
            return Err(AppError::keyring(format!(
                "Failed to finalize secrets file: {}",
                e
            )));
        }

        if backup_path.exists() {
            let _ = std::fs::remove_file(&backup_path);
        }
        Self::set_secure_permissions(&self.store_path)?;

        Ok(())
    }

    fn encode_key_bytes(key_bytes: &[u8]) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(KEY_FILE_MAGIC.len() + key_bytes.len());
        encoded.extend_from_slice(KEY_FILE_MAGIC);
        encoded.extend_from_slice(key_bytes);
        encoded
    }

    fn decode_key_bytes(raw: &[u8]) -> AppResult<Vec<u8>> {
        if raw.len() == KEY_SIZE {
            return Ok(raw.to_vec());
        }
        if raw.starts_with(KEY_FILE_MAGIC) && raw.len() == KEY_FILE_MAGIC.len() + KEY_SIZE {
            return Ok(raw[KEY_FILE_MAGIC.len()..].to_vec());
        }
        Err(AppError::keyring(format!(
            "Invalid encryption key size: expected {} or {}, got {}",
            KEY_SIZE,
            KEY_FILE_MAGIC.len() + KEY_SIZE,
            raw.len()
        )))
    }

    fn write_key_file_atomic(path: &Path, encoded_key: &[u8]) -> AppResult<()> {
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, encoded_key).map_err(|e| {
            AppError::keyring(format!(
                "Failed to write temporary encryption key file '{}': {}",
                tmp.display(),
                e
            ))
        })?;
        Self::set_secure_permissions(&tmp)?;
        std::fs::rename(&tmp, path).map_err(|e| {
            AppError::keyring(format!(
                "Failed to finalize encryption key file '{}': {}",
                path.display(),
                e
            ))
        })?;
        Self::set_secure_permissions(path)?;
        Ok(())
    }

    fn set_secure_permissions(path: &Path) -> AppResult<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(
                |e| {
                    AppError::keyring(format!(
                        "Failed to set file permissions for '{}': {}",
                        path.display(),
                        e
                    ))
                },
            )?;
        }
        Ok(())
    }

    fn is_secure_permissions(path: &Path) -> AppResult<bool> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(path).map_err(|e| {
                AppError::keyring(format!(
                    "Failed to inspect file permissions for '{}': {}",
                    path.display(),
                    e
                ))
            })?;
            let mode = metadata.permissions().mode() & 0o777;
            return Ok((mode & 0o077) == 0);
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Ok(true)
        }
    }

    fn save_plaintext_map_with_cipher(
        &self,
        secrets: &HashMap<String, String>,
        cipher: &Aes256Gcm,
    ) -> AppResult<()> {
        let mut encrypted = HashMap::with_capacity(secrets.len());
        for (key, plaintext) in secrets {
            encrypted.insert(key.clone(), Self::encrypt_with_cipher(cipher, plaintext)?);
        }
        self.save(&encrypted)
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
                "brave".to_string(),
                "brave_search".to_string(),
                "searxng".to_string(),
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

    /// List all secret keys stored in encrypted storage that match a prefix.
    pub fn list_keys_with_prefix(&self, prefix: &str) -> AppResult<Vec<String>> {
        let store = self.store()?;
        let secrets = store.load();
        let mut keys = secrets
            .keys()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();
        Ok(keys)
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

    /// Export all stored secrets as decrypted plaintext key-value pairs.
    ///
    /// Returns a map of provider name → plaintext API key for all stored secrets.
    pub fn export_all_decrypted(&self) -> AppResult<HashMap<String, String>> {
        let store = self.store()?;
        let secrets = store.load();
        let mut result = HashMap::new();

        for (key, encrypted_value) in &secrets {
            match store.decrypt(encrypted_value) {
                Ok(plaintext) => {
                    result.insert(key.clone(), plaintext);
                }
                Err(e) => {
                    tracing::warn!("Failed to decrypt secret for '{}': {}", key, e);
                }
            }
        }

        Ok(result)
    }

    /// Import secrets from a plaintext key-value map, re-encrypting each with the internal key.
    ///
    /// This replaces all existing secrets with the provided ones.
    pub fn import_all(&self, secrets: &HashMap<String, String>) -> AppResult<()> {
        let store = self.store()?;
        store.save_plaintext_map_with_cipher(secrets, &store.cipher)
    }

    /// Rotate MCP secret encryption key and re-encrypt all stored values.
    ///
    /// This is an internal maintenance helper and is intentionally not wired to UI yet.
    pub fn rotate_mcp_secret_key(&mut self) -> AppResult<()> {
        let current_store = self.store()?;
        current_store.ensure_writable()?;

        let plaintext = self.export_all_decrypted()?;
        let previous_key_bytes = std::fs::read(&current_store.key_path).map_err(|e| {
            AppError::keyring(format!(
                "Failed to read current encryption key '{}': {}",
                current_store.key_path.display(),
                e
            ))
        })?;

        let mut rotated_key = vec![0u8; KEY_SIZE];
        OsRng.fill_bytes(&mut rotated_key);

        if let Err(rotate_err) = SecureStore::write_key_file_atomic(
            &current_store.key_path,
            &SecureStore::encode_key_bytes(&rotated_key),
        ) {
            return Err(AppError::keyring(format!(
                "Failed to rotate encryption key: {}",
                rotate_err
            )));
        }

        let rotated_store = match ensure_plan_cascade_dir().and_then(|dir| SecureStore::new(&dir)) {
            Ok(store) => store,
            Err(e) => {
                let _ = SecureStore::write_key_file_atomic(
                    &current_store.key_path,
                    &previous_key_bytes,
                );
                return Err(AppError::keyring(format!(
                    "Failed to initialize rotated key store: {}",
                    e
                )));
            }
        };

        if let Err(e) =
            rotated_store.save_plaintext_map_with_cipher(&plaintext, &rotated_store.cipher)
        {
            tracing::error!(error = %e, "Failed to persist secrets after key rotation; attempting rollback");
            let _ =
                SecureStore::write_key_file_atomic(&current_store.key_path, &previous_key_bytes);
            if let Ok(restored) = ensure_plan_cascade_dir().and_then(|dir| SecureStore::new(&dir)) {
                self.inner = Some(restored);
            }
            return Err(AppError::keyring(format!(
                "Failed to re-encrypt secrets after key rotation: {}",
                e
            )));
        }

        self.inner = Some(rotated_store);
        tracing::info!("Rotated MCP secret encryption key successfully");
        Ok(())
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

    #[test]
    fn test_legacy_secrets_payload_compatibility() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecureStore::new(&dir.path().to_path_buf()).unwrap();

        let legacy_payload = serde_json::json!({
            "openai": store.encrypt("legacy-key").unwrap()
        });
        std::fs::write(
            dir.path().join(SECRETS_FILE),
            serde_json::to_string(&legacy_payload).unwrap(),
        )
        .unwrap();

        let reloaded = SecureStore::new(&dir.path().to_path_buf()).unwrap();
        let loaded = reloaded.load();
        let decrypted = reloaded.decrypt(loaded.get("openai").unwrap()).unwrap();
        assert_eq!(decrypted, "legacy-key");
    }

    #[test]
    fn test_integrity_failure_forces_read_only_mode() {
        let dir = tempfile::tempdir().unwrap();
        let _store = SecureStore::new(&dir.path().to_path_buf()).unwrap();
        std::fs::write(dir.path().join(SECRETS_FILE), "{invalid").unwrap();

        let reloaded = SecureStore::new(&dir.path().to_path_buf()).unwrap();
        assert!(reloaded.read_only);
    }
}
