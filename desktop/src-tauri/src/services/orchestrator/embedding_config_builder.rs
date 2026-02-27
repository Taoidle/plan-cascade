//! Shared Embedding Configuration Builder
//!
//! Extracts embedding config loading logic from `IndexManager` into a reusable
//! module so that both the codebase indexer and the knowledge pipeline can read
//! the user's configured embedding provider without duplicating keyring/proxy
//! resolution logic.

use tracing::warn;

use super::embedding_manager::EmbeddingManagerConfig;
use super::embedding_provider::{
    EmbeddingProviderConfig, EmbeddingProviderType, PersistedEmbeddingConfig,
    EMBEDDING_CONFIG_SETTING_KEY,
};
use crate::commands::proxy::resolve_provider_proxy;
use crate::services::proxy::ProxyConfig;
use crate::storage::database::Database;
use crate::storage::KeyringService;

/// Load the persisted embedding configuration from the `settings` table.
///
/// Returns `None` if no config is stored or if deserialization fails.
pub fn load_persisted_embedding_config(db: &Database) -> Option<PersistedEmbeddingConfig> {
    match db.get_setting(EMBEDDING_CONFIG_SETTING_KEY) {
        Ok(Some(json)) => match serde_json::from_str::<PersistedEmbeddingConfig>(&json) {
            Ok(config) => Some(config),
            Err(e) => {
                warn!(
                    error = %e,
                    "embedding_config_builder: failed to parse persisted embedding config"
                );
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            warn!(
                error = %e,
                "embedding_config_builder: failed to read embedding config from DB"
            );
            None
        }
    }
}

/// Return the keyring alias for a given embedding provider type.
pub fn embedding_keyring_alias(provider: EmbeddingProviderType) -> Option<&'static str> {
    match provider {
        EmbeddingProviderType::Qwen => Some("qwen_embedding"),
        EmbeddingProviderType::Glm => Some("glm_embedding"),
        EmbeddingProviderType::OpenAI => Some("openai_embedding"),
        EmbeddingProviderType::TfIdf | EmbeddingProviderType::Ollama => None,
    }
}

/// Return the proxy alias for a given embedding provider type.
///
/// Returns `None` for TF-IDF (no network needed).
pub fn embedding_proxy_alias(provider: EmbeddingProviderType) -> Option<&'static str> {
    match provider {
        EmbeddingProviderType::OpenAI => Some("embedding_openai"),
        EmbeddingProviderType::Qwen => Some("embedding_qwen"),
        EmbeddingProviderType::Glm => Some("embedding_glm"),
        EmbeddingProviderType::Ollama => Some("embedding_ollama"),
        EmbeddingProviderType::TfIdf => None,
    }
}

/// Resolve the proxy configuration for a given embedding provider type.
fn resolve_embedding_proxy(
    keyring: &KeyringService,
    db: &Database,
    provider: EmbeddingProviderType,
) -> Option<ProxyConfig> {
    let alias = embedding_proxy_alias(provider)?;
    resolve_provider_proxy(keyring, db, alias)
}

/// Build an `EmbeddingManagerConfig` from the persisted DB settings.
///
/// Reads the user's configured embedding provider, resolves the API key from
/// the OS keyring, and constructs the full `EmbeddingManagerConfig`. Falls back
/// to TF-IDF on any failure.
///
/// Returns `(config, dimension, is_tfidf)`:
/// - `config`: The fully-resolved `EmbeddingManagerConfig`.
/// - `dimension`: The configured embedding dimension (0 for TF-IDF as it is
///   determined dynamically by vocabulary size).
/// - `is_tfidf`: Whether the returned config uses TF-IDF (useful for callers
///   that need vocabulary persistence).
pub fn build_embedding_config_from_settings(
    db: &Database,
    keyring: &KeyringService,
) -> (EmbeddingManagerConfig, usize, bool) {
    let persisted = load_persisted_embedding_config(db);

    let persisted = match persisted {
        Some(c) if c.provider != EmbeddingProviderType::TfIdf => c,
        _ => {
            // No config or TF-IDF configured → default TF-IDF path
            return tfidf_config();
        }
    };

    // Resolve API key from the OS keyring for cloud providers.
    let api_key: Option<String> = match embedding_keyring_alias(persisted.provider) {
        Some(alias) => match keyring.get_api_key(alias) {
            Ok(Some(key)) if !key.is_empty() => Some(key),
            Ok(_) => {
                warn!(
                    provider = ?persisted.provider,
                    "embedding_config_builder: API key is empty or missing, falling back to TF-IDF"
                );
                return tfidf_config();
            }
            Err(e) => {
                warn!(
                    provider = ?persisted.provider,
                    error = %e,
                    "embedding_config_builder: failed to get API key, falling back to TF-IDF"
                );
                return tfidf_config();
            }
        },
        None => None, // Local providers (Ollama) don't need keys
    };

    // Build primary provider config.
    let mut primary_config = EmbeddingProviderConfig::new(persisted.provider);
    primary_config.model = persisted.model.clone();
    primary_config.api_key = api_key;
    primary_config.base_url = persisted.base_url.clone();
    primary_config.dimension = persisted.dimension;
    primary_config.batch_size = persisted.batch_size;
    primary_config.proxy = resolve_embedding_proxy(keyring, db, persisted.provider);

    let dimension = persisted.dimension.unwrap_or(0);

    // Build optional fallback config (TF-IDF fallback is common).
    let fallback_config = persisted
        .fallback_provider
        .map(|fb_type| EmbeddingProviderConfig::new(fb_type));

    let manager_config = EmbeddingManagerConfig {
        primary: primary_config,
        fallback: fallback_config,
        cache_enabled: true,
        cache_max_entries: 10_000,
    };

    (manager_config, dimension, false)
}

/// Build a default TF-IDF config tuple.
fn tfidf_config() -> (EmbeddingManagerConfig, usize, bool) {
    let config = EmbeddingManagerConfig {
        primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
        fallback: None,
        cache_enabled: true,
        cache_max_entries: 10_000,
    };
    (config, 0, true)
}
