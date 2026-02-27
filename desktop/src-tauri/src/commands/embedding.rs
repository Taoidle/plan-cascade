//! Embedding Configuration Commands
//!
//! Tauri commands for embedding provider configuration CRUD, health checks, and
//! API key management. Embedding config is persisted to the app config file
//! (non-secret values) while cloud API keys are stored in the OS keyring with
//! provider-specific aliases (`qwen_embedding`, `glm_embedding`, etc.).
//!
//! ## IPC Commands
//!
//! - `get_embedding_config` (IPC-001) — Retrieve current embedding provider configuration
//! - `set_embedding_config` (IPC-002) — Update embedding provider configuration
//! - `list_embedding_providers` (IPC-003) — List all available providers with capabilities
//! - `check_embedding_provider_health` (IPC-004) — Health check on specific provider
//! - `set_embedding_api_key` (IPC-005) — Store embedding API key in OS keyring
//! - `get_embedding_api_key` (IPC-006) — Retrieve embedding API key from OS keyring

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::orchestrator::analysis_index::{binary_extensions, default_excluded_roots};
use crate::services::orchestrator::embedding_provider::{
    CodebaseIndexConfig, EmbeddingProvider, EmbeddingProviderCapability, EmbeddingProviderConfig,
    EmbeddingProviderType, PersistedEmbeddingConfig, CODEBASE_INDEX_CONFIG_KEY,
    EMBEDDING_CONFIG_SETTING_KEY,
};
use crate::state::AppState;
use crate::storage::KeyringService;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// Request for `set_embedding_config` (IPC-002).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetEmbeddingConfigRequest {
    /// Provider type (e.g., "tf_idf", "ollama", "qwen", "glm", "open_ai").
    pub provider: String,
    /// Model identifier (optional — defaults to provider's default model).
    pub model: Option<String>,
    /// Base URL override (optional).
    pub base_url: Option<String>,
    /// Desired embedding dimension (optional).
    pub dimension: Option<usize>,
    /// Batch size (optional).
    pub batch_size: Option<usize>,
    /// Fallback provider type (optional).
    pub fallback_provider: Option<String>,
}

/// Response for `get_embedding_config` (IPC-001) and `set_embedding_config` (IPC-002).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfigResponse {
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    pub dimension: usize,
    pub batch_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_provider: Option<String>,
}

/// Response for `set_embedding_config` (IPC-002).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetEmbeddingConfigResponse {
    pub provider: String,
    pub model: String,
    /// Whether a reindex is required due to configuration changes.
    pub reindex_required: bool,
}

/// Request for `check_embedding_provider_health` (IPC-004).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckEmbeddingHealthRequest {
    pub provider: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

/// Response for `check_embedding_provider_health` (IPC-004).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingHealthResponse {
    pub healthy: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u32>,
}

/// Request for `set_embedding_api_key` (IPC-005).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetEmbeddingApiKeyRequest {
    /// Provider alias (e.g., "qwen_embedding", "glm_embedding", "openai_embedding").
    pub provider: String,
    /// The API key to store.
    pub api_key: String,
}

/// Response for `set_embedding_api_key` (IPC-005).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetEmbeddingApiKeyResponse {
    pub success: bool,
}

/// Request for `get_embedding_api_key` (IPC-006).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetEmbeddingApiKeyRequest {
    /// Provider alias (e.g., "qwen_embedding", "glm_embedding", "openai_embedding").
    pub provider: String,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Canonical embedding keyring aliases.
const EMBEDDING_KEYRING_PROVIDERS: &[&str] =
    &["qwen_embedding", "glm_embedding", "openai_embedding"];

/// Parse an `EmbeddingProviderType` from a user-facing string.
///
/// Accepts the serde snake_case values ("tf_idf", "ollama", "qwen", "glm",
/// "open_ai") as well as common aliases.
fn parse_provider_type(s: &str) -> Option<EmbeddingProviderType> {
    match s.trim().to_lowercase().as_str() {
        "tf_idf" | "tfidf" => Some(EmbeddingProviderType::TfIdf),
        "ollama" => Some(EmbeddingProviderType::Ollama),
        "qwen" | "qwen_embedding" | "dashscope" => Some(EmbeddingProviderType::Qwen),
        "glm" | "glm_embedding" | "zhipu" | "zhipuai" => Some(EmbeddingProviderType::Glm),
        "open_ai" | "openai" | "openai_embedding" => Some(EmbeddingProviderType::OpenAI),
        _ => None,
    }
}

/// Return the keyring alias for an embedding provider.
fn embedding_keyring_alias(provider: EmbeddingProviderType) -> Option<&'static str> {
    match provider {
        EmbeddingProviderType::Qwen => Some("qwen_embedding"),
        EmbeddingProviderType::Glm => Some("glm_embedding"),
        EmbeddingProviderType::OpenAI => Some("openai_embedding"),
        // Local providers do not use API keys
        EmbeddingProviderType::TfIdf | EmbeddingProviderType::Ollama => None,
    }
}

/// Validate an embedding keyring provider alias and return it if recognised.
fn validate_embedding_keyring_provider(provider: &str) -> Option<&'static str> {
    let normalised = provider.trim().to_lowercase();
    EMBEDDING_KEYRING_PROVIDERS
        .iter()
        .find(|&&p| p == normalised)
        .copied()
}

/// Build an `EmbeddingConfigResponse` from an `EmbeddingProviderConfig` and
/// an optional fallback provider type.
fn config_to_response(
    config: &EmbeddingProviderConfig,
    fallback: Option<EmbeddingProviderType>,
) -> EmbeddingConfigResponse {
    EmbeddingConfigResponse {
        provider: serde_json::to_value(&config.provider)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| config.provider.to_string()),
        model: config.model.clone(),
        base_url: config.base_url.clone(),
        dimension: config.effective_dimension(),
        batch_size: config.batch_size,
        fallback_provider: fallback.map(|fp| {
            serde_json::to_value(&fp)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| fp.to_string())
        }),
    }
}

// ---------------------------------------------------------------------------
// IPC-001: get_embedding_config
// ---------------------------------------------------------------------------

/// Retrieve the current embedding provider configuration.
///
/// Reads the persisted embedding config from the database settings store.
/// If no config has been explicitly saved yet, returns the default TF-IDF
/// configuration.
#[tauri::command]
pub async fn get_embedding_config(
    _project_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<EmbeddingConfigResponse>, String> {
    // Try to load persisted config from database
    let persisted = state
        .with_database(|db| {
            match db.get_setting(EMBEDDING_CONFIG_SETTING_KEY) {
                Ok(Some(json_str)) => {
                    match serde_json::from_str::<PersistedEmbeddingConfig>(&json_str) {
                        Ok(cfg) => Ok(Some(cfg)),
                        Err(_) => Ok(None), // corrupted — fall back to defaults
                    }
                }
                Ok(None) => Ok(None),
                Err(e) => Err(e),
            }
        })
        .await;

    let (config, fallback) = match persisted {
        Ok(Some(persisted)) => {
            let mut cfg = EmbeddingProviderConfig::new(persisted.provider);
            cfg.model = persisted.model;
            cfg.base_url = persisted.base_url;
            cfg.dimension = persisted.dimension;
            cfg.batch_size = persisted.batch_size;
            (cfg, persisted.fallback_provider)
        }
        _ => {
            // Default: TF-IDF (local, no API key needed)
            let cfg = EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf);
            (cfg, None)
        }
    };

    Ok(CommandResponse::ok(config_to_response(&config, fallback)))
}

// ---------------------------------------------------------------------------
// IPC-002: set_embedding_config
// ---------------------------------------------------------------------------

/// Update the embedding provider configuration.
///
/// Validates the new configuration, persists non-secret values to the database
/// settings store, and reports whether a reindex is required (i.e., the
/// provider or model changed relative to the previously stored config).
#[tauri::command]
pub async fn set_embedding_config(
    request: SetEmbeddingConfigRequest,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SetEmbeddingConfigResponse>, String> {
    // Parse provider type
    let provider_type = match parse_provider_type(&request.provider) {
        Some(p) => p,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown embedding provider: '{}'",
                request.provider
            )));
        }
    };

    // Parse optional fallback
    let fallback_provider = if let Some(ref fb) = request.fallback_provider {
        match parse_provider_type(fb) {
            Some(p) => Some(p),
            None => {
                return Ok(CommandResponse::err(format!(
                    "Unknown fallback provider: '{}'",
                    fb
                )));
            }
        }
    } else {
        None
    };

    // Build the new config
    let capability = provider_type.default_capability();
    let mut new_config = EmbeddingProviderConfig::new(provider_type);

    if let Some(model) = &request.model {
        new_config.model = model.clone();
    }
    new_config.base_url = request.base_url.clone();
    new_config.dimension = request.dimension;
    if let Some(batch_size) = request.batch_size {
        new_config.batch_size = batch_size;
    }

    // For remote providers that require an API key, temporarily inject a
    // placeholder so that validate() does not reject the config. The actual
    // key is stored in the OS keyring via `set_embedding_api_key`.
    if capability.requires_api_key {
        let keyring = KeyringService::new();
        if let Some(alias) = embedding_keyring_alias(provider_type) {
            match keyring.get_api_key(alias) {
                Ok(Some(key)) => new_config.api_key = Some(key),
                Ok(None) => {
                    // Allow saving config even without a key — the health check
                    // will later report the missing key. Use placeholder.
                    new_config.api_key = Some("placeholder-for-validation".to_string());
                }
                Err(e) => {
                    return Ok(CommandResponse::err(format!(
                        "Failed to read API key from keyring: {}",
                        e
                    )));
                }
            }
        } else {
            new_config.api_key = Some("placeholder-for-validation".to_string());
        }
    }

    // Validate the config (with API key temporarily set for remote providers)
    if let Err(e) = new_config.validate() {
        return Ok(CommandResponse::err(format!(
            "Invalid embedding configuration: {}",
            e
        )));
    }

    // Load existing persisted config to determine if reindex is needed
    let old_persisted = state
        .with_database(|db| match db.get_setting(EMBEDDING_CONFIG_SETTING_KEY) {
            Ok(Some(json_str)) => {
                Ok(serde_json::from_str::<PersistedEmbeddingConfig>(&json_str).ok())
            }
            _ => Ok(None),
        })
        .await
        .unwrap_or(None);

    let reindex_required = match old_persisted {
        Some(old) => {
            old.provider != provider_type
                || old.model != new_config.model
                || old.dimension != new_config.dimension
        }
        None => {
            // First explicit config — reindex required if not TF-IDF default
            provider_type != EmbeddingProviderType::TfIdf
        }
    };

    // Persist (without API key — that lives in keyring)
    let persisted = PersistedEmbeddingConfig {
        provider: provider_type,
        model: new_config.model.clone(),
        base_url: new_config.base_url.clone(),
        dimension: new_config.dimension,
        batch_size: new_config.batch_size,
        fallback_provider,
    };

    let json_str = serde_json::to_string(&persisted)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    let persist_result = state
        .with_database(|db| db.set_setting(EMBEDDING_CONFIG_SETTING_KEY, &json_str))
        .await;

    if let Err(e) = persist_result {
        return Ok(CommandResponse::err(format!(
            "Failed to persist embedding config: {}",
            e
        )));
    }

    Ok(CommandResponse::ok(SetEmbeddingConfigResponse {
        provider: serde_json::to_value(&provider_type)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| provider_type.to_string()),
        model: new_config.model,
        reindex_required,
    }))
}

// ---------------------------------------------------------------------------
// IPC-003: list_embedding_providers
// ---------------------------------------------------------------------------

/// List all available embedding providers with their capabilities.
#[tauri::command]
pub async fn list_embedding_providers() -> CommandResponse<Vec<EmbeddingProviderCapability>> {
    let providers: Vec<EmbeddingProviderCapability> = EmbeddingProviderType::all()
        .iter()
        .map(|p| p.default_capability())
        .collect();

    CommandResponse::ok(providers)
}

// ---------------------------------------------------------------------------
// IPC-004: check_embedding_provider_health
// ---------------------------------------------------------------------------

/// Health check for a specific embedding provider.
///
/// For local providers (TF-IDF) this always succeeds. For Ollama, it checks
/// server connectivity. For remote API providers (Qwen, GLM, OpenAI), it
/// verifies the API key exists in the keyring and attempts a lightweight
/// validation call.
#[tauri::command]
pub async fn check_embedding_provider_health(
    request: CheckEmbeddingHealthRequest,
) -> CommandResponse<EmbeddingHealthResponse> {
    let provider_type = match parse_provider_type(&request.provider) {
        Some(p) => p,
        None => {
            return CommandResponse::err(format!(
                "Unknown embedding provider: '{}'",
                request.provider
            ));
        }
    };

    let capability = provider_type.default_capability();
    let model = request
        .model
        .unwrap_or_else(|| capability.default_model.clone());

    // Build a provider config
    let mut config = EmbeddingProviderConfig::new(provider_type);
    config.model = model;
    config.base_url = request.base_url;

    // For remote providers, retrieve the API key from keyring
    if capability.requires_api_key {
        let keyring = KeyringService::new();
        if let Some(alias) = embedding_keyring_alias(provider_type) {
            match keyring.get_api_key(alias) {
                Ok(Some(key)) => config.api_key = Some(key),
                Ok(None) => {
                    return CommandResponse::ok(EmbeddingHealthResponse {
                        healthy: false,
                        message: format!(
                            "API key not configured for {} (keyring alias: '{}')",
                            capability.display_name, alias
                        ),
                        latency_ms: None,
                    });
                }
                Err(e) => {
                    return CommandResponse::ok(EmbeddingHealthResponse {
                        healthy: false,
                        message: format!("Failed to read API key from keyring: {}", e),
                        latency_ms: None,
                    });
                }
            }
        } else {
            return CommandResponse::ok(EmbeddingHealthResponse {
                healthy: false,
                message: format!(
                    "No keyring alias configured for provider '{}'",
                    capability.display_name
                ),
                latency_ms: None,
            });
        }
    }

    // Validate config before attempting health check
    if let Err(e) = config.validate() {
        return CommandResponse::ok(EmbeddingHealthResponse {
            healthy: false,
            message: format!("Invalid configuration: {}", e),
            latency_ms: None,
        });
    }

    // Attempt to build the provider and run health check
    let start = std::time::Instant::now();

    match provider_type {
        EmbeddingProviderType::TfIdf => {
            // TF-IDF is always healthy (local, no dependencies)
            CommandResponse::ok(EmbeddingHealthResponse {
                healthy: true,
                message: "TF-IDF provider is always available (local)".to_string(),
                latency_ms: Some(start.elapsed().as_millis() as u32),
            })
        }
        EmbeddingProviderType::Ollama => {
            // Attempt to connect to Ollama
            let provider =
                crate::services::orchestrator::embedding_provider_ollama::OllamaEmbeddingProvider::new(
                    &config,
                );
            match provider.health_check().await {
                Ok(()) => CommandResponse::ok(EmbeddingHealthResponse {
                    healthy: true,
                    message: "Ollama embedding provider is healthy".to_string(),
                    latency_ms: Some(start.elapsed().as_millis() as u32),
                }),
                Err(e) => CommandResponse::ok(EmbeddingHealthResponse {
                    healthy: false,
                    message: format!("Ollama health check failed: {}", e),
                    latency_ms: Some(start.elapsed().as_millis() as u32),
                }),
            }
        }
        EmbeddingProviderType::Qwen => {
            let provider =
                crate::services::orchestrator::embedding_provider_qwen::QwenEmbeddingProvider::new(
                    &config,
                );
            match provider.health_check().await {
                Ok(()) => CommandResponse::ok(EmbeddingHealthResponse {
                    healthy: true,
                    message: "Qwen embedding provider is healthy".to_string(),
                    latency_ms: Some(start.elapsed().as_millis() as u32),
                }),
                Err(e) => CommandResponse::ok(EmbeddingHealthResponse {
                    healthy: false,
                    message: format!("Qwen health check failed: {}", e),
                    latency_ms: Some(start.elapsed().as_millis() as u32),
                }),
            }
        }
        EmbeddingProviderType::Glm => {
            let provider =
                crate::services::orchestrator::embedding_provider_glm::GlmEmbeddingProvider::new(
                    &config,
                );
            match provider.health_check().await {
                Ok(()) => CommandResponse::ok(EmbeddingHealthResponse {
                    healthy: true,
                    message: "GLM embedding provider is healthy".to_string(),
                    latency_ms: Some(start.elapsed().as_millis() as u32),
                }),
                Err(e) => CommandResponse::ok(EmbeddingHealthResponse {
                    healthy: false,
                    message: format!("GLM health check failed: {}", e),
                    latency_ms: Some(start.elapsed().as_millis() as u32),
                }),
            }
        }
        EmbeddingProviderType::OpenAI => {
            let provider =
                crate::services::orchestrator::embedding_provider_openai::OpenAIEmbeddingProvider::new(
                    &config,
                );
            match provider.health_check().await {
                Ok(()) => CommandResponse::ok(EmbeddingHealthResponse {
                    healthy: true,
                    message: "OpenAI embedding provider is healthy".to_string(),
                    latency_ms: Some(start.elapsed().as_millis() as u32),
                }),
                Err(e) => CommandResponse::ok(EmbeddingHealthResponse {
                    healthy: false,
                    message: format!("OpenAI health check failed: {}", e),
                    latency_ms: Some(start.elapsed().as_millis() as u32),
                }),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// IPC-005: set_embedding_api_key
// ---------------------------------------------------------------------------

/// Store an embedding API key in the OS keyring.
///
/// Uses provider-specific keyring aliases (`qwen_embedding`, `glm_embedding`,
/// `openai_embedding`) that are separate from LLM provider keys.
#[tauri::command]
pub async fn set_embedding_api_key(
    request: SetEmbeddingApiKeyRequest,
) -> Result<CommandResponse<SetEmbeddingApiKeyResponse>, String> {
    // Validate provider alias
    let alias = match validate_embedding_keyring_provider(&request.provider) {
        Some(a) => a,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown embedding provider keyring alias: '{}'. \
                 Valid aliases: {:?}",
                request.provider, EMBEDDING_KEYRING_PROVIDERS
            )));
        }
    };

    // Validate API key is not empty
    let key = request.api_key.trim();
    if key.is_empty() {
        return Ok(CommandResponse::err(
            "API key must not be empty".to_string(),
        ));
    }

    let keyring = KeyringService::new();
    match keyring.set_api_key(alias, key) {
        Ok(()) => Ok(CommandResponse::ok(SetEmbeddingApiKeyResponse {
            success: true,
        })),
        Err(e) => Ok(CommandResponse::err(format!(
            "Failed to store embedding API key: {}",
            e
        ))),
    }
}

// ---------------------------------------------------------------------------
// IPC-006: get_embedding_api_key
// ---------------------------------------------------------------------------

/// Retrieve an embedding API key from the OS keyring.
///
/// Returns the stored key for the given provider alias, or `None` if no key
/// has been saved. This is the read counterpart of `set_embedding_api_key`.
#[tauri::command]
pub async fn get_embedding_api_key(
    request: GetEmbeddingApiKeyRequest,
) -> CommandResponse<Option<String>> {
    let alias = match validate_embedding_keyring_provider(&request.provider) {
        Some(a) => a,
        None => {
            return CommandResponse::err(format!(
                "Unknown embedding provider keyring alias: '{}'. \
                 Valid aliases: {:?}",
                request.provider, EMBEDDING_KEYRING_PROVIDERS
            ));
        }
    };

    let keyring = KeyringService::new();
    match keyring.get_api_key(alias) {
        Ok(key) => CommandResponse::ok(key),
        Err(e) => CommandResponse::err(format!(
            "Failed to read embedding API key from keyring: {}",
            e
        )),
    }
}

// ---------------------------------------------------------------------------
// IPC-007: get_codebase_index_config
// ---------------------------------------------------------------------------

/// Response for `get_codebase_index_config` (IPC-007).
///
/// Returns both built-in exclusions (read-only for display) and user-added
/// extras (editable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseIndexConfigResponse {
    /// Built-in excluded dirs (read-only for frontend display).
    pub builtin_excluded_dirs: Vec<String>,
    /// Built-in binary extensions (read-only for frontend display).
    pub builtin_excluded_extensions: Vec<String>,
    /// User-added extra excluded dirs (editable).
    pub extra_excluded_dirs: Vec<String>,
    /// User-added extra excluded extensions (editable).
    pub extra_excluded_extensions: Vec<String>,
}

/// Retrieve the current codebase index exclusion configuration.
///
/// Returns built-in exclusion lists (always applied, not editable) alongside
/// any user-added extras from the database settings store.
#[tauri::command]
pub async fn get_codebase_index_config(
    state: State<'_, AppState>,
) -> Result<CommandResponse<CodebaseIndexConfigResponse>, String> {
    let user_config = state
        .with_database(|db| match db.get_setting(CODEBASE_INDEX_CONFIG_KEY) {
            Ok(Some(json_str)) => {
                Ok(serde_json::from_str::<CodebaseIndexConfig>(&json_str).unwrap_or_default())
            }
            Ok(None) => Ok(CodebaseIndexConfig::default()),
            Err(e) => Err(e),
        })
        .await
        .unwrap_or_default();

    Ok(CommandResponse::ok(CodebaseIndexConfigResponse {
        builtin_excluded_dirs: default_excluded_roots()
            .iter()
            .map(|s| s.to_string())
            .collect(),
        builtin_excluded_extensions: binary_extensions()
            .iter()
            .map(|s| s.to_string())
            .collect(),
        extra_excluded_dirs: user_config.extra_excluded_dirs,
        extra_excluded_extensions: user_config.extra_excluded_extensions,
    }))
}

// ---------------------------------------------------------------------------
// IPC-008: set_codebase_index_config
// ---------------------------------------------------------------------------

/// Request for `set_codebase_index_config` (IPC-008).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCodebaseIndexConfigRequest {
    pub extra_excluded_dirs: Vec<String>,
    pub extra_excluded_extensions: Vec<String>,
}

/// Update the codebase index exclusion configuration.
///
/// Normalizes inputs (lowercase extensions, trim dir names, dedup, remove
/// entries that duplicate builtins) and persists to the settings table.
#[tauri::command]
pub async fn set_codebase_index_config(
    request: SetCodebaseIndexConfigRequest,
    state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    let builtin_dirs: std::collections::HashSet<&str> =
        default_excluded_roots().iter().copied().collect();
    let builtin_exts: std::collections::HashSet<&str> =
        binary_extensions().iter().copied().collect();

    // Normalize dirs: trim, dedup, remove builtins
    let mut dirs: Vec<String> = request
        .extra_excluded_dirs
        .iter()
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty() && !builtin_dirs.contains(d.as_str()))
        .collect();
    dirs.sort();
    dirs.dedup();

    // Normalize extensions: strip leading dot, lowercase, dedup, remove builtins
    let mut exts: Vec<String> = request
        .extra_excluded_extensions
        .iter()
        .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|e| !e.is_empty() && !builtin_exts.contains(e.as_str()))
        .collect();
    exts.sort();
    exts.dedup();

    let config = CodebaseIndexConfig {
        extra_excluded_dirs: dirs,
        extra_excluded_extensions: exts,
    };

    let json_str = serde_json::to_string(&config)
        .map_err(|e| format!("Failed to serialize codebase index config: {}", e))?;

    let persist_result = state
        .with_database(|db| db.set_setting(CODEBASE_INDEX_CONFIG_KEY, &json_str))
        .await;

    if let Err(e) = persist_result {
        return Ok(CommandResponse::err(format!(
            "Failed to persist codebase index config: {}",
            e
        )));
    }

    Ok(CommandResponse::ok(true))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // parse_provider_type tests
    // =====================================================================

    #[test]
    fn parse_provider_type_accepts_canonical_names() {
        assert_eq!(
            parse_provider_type("tf_idf"),
            Some(EmbeddingProviderType::TfIdf)
        );
        assert_eq!(
            parse_provider_type("tfidf"),
            Some(EmbeddingProviderType::TfIdf)
        );
        assert_eq!(
            parse_provider_type("ollama"),
            Some(EmbeddingProviderType::Ollama)
        );
        assert_eq!(
            parse_provider_type("qwen"),
            Some(EmbeddingProviderType::Qwen)
        );
        assert_eq!(parse_provider_type("glm"), Some(EmbeddingProviderType::Glm));
        assert_eq!(
            parse_provider_type("open_ai"),
            Some(EmbeddingProviderType::OpenAI)
        );
        assert_eq!(
            parse_provider_type("openai"),
            Some(EmbeddingProviderType::OpenAI)
        );
    }

    #[test]
    fn parse_provider_type_accepts_aliases() {
        assert_eq!(
            parse_provider_type("qwen_embedding"),
            Some(EmbeddingProviderType::Qwen)
        );
        assert_eq!(
            parse_provider_type("glm_embedding"),
            Some(EmbeddingProviderType::Glm)
        );
        assert_eq!(
            parse_provider_type("openai_embedding"),
            Some(EmbeddingProviderType::OpenAI)
        );
        assert_eq!(
            parse_provider_type("dashscope"),
            Some(EmbeddingProviderType::Qwen)
        );
        assert_eq!(
            parse_provider_type("zhipu"),
            Some(EmbeddingProviderType::Glm)
        );
        assert_eq!(
            parse_provider_type("zhipuai"),
            Some(EmbeddingProviderType::Glm)
        );
    }

    #[test]
    fn parse_provider_type_case_insensitive() {
        assert_eq!(
            parse_provider_type("TfIdf"),
            Some(EmbeddingProviderType::TfIdf)
        );
        assert_eq!(
            parse_provider_type("OLLAMA"),
            Some(EmbeddingProviderType::Ollama)
        );
        assert_eq!(
            parse_provider_type("QWEN"),
            Some(EmbeddingProviderType::Qwen)
        );
    }

    #[test]
    fn parse_provider_type_rejects_unknown() {
        assert_eq!(parse_provider_type("unknown_provider"), None);
        assert_eq!(parse_provider_type(""), None);
        assert_eq!(parse_provider_type("   "), None);
    }

    // =====================================================================
    // embedding_keyring_alias tests
    // =====================================================================

    #[test]
    fn keyring_alias_for_remote_providers() {
        assert_eq!(
            embedding_keyring_alias(EmbeddingProviderType::Qwen),
            Some("qwen_embedding")
        );
        assert_eq!(
            embedding_keyring_alias(EmbeddingProviderType::Glm),
            Some("glm_embedding")
        );
        assert_eq!(
            embedding_keyring_alias(EmbeddingProviderType::OpenAI),
            Some("openai_embedding")
        );
    }

    #[test]
    fn keyring_alias_none_for_local_providers() {
        assert_eq!(embedding_keyring_alias(EmbeddingProviderType::TfIdf), None);
        assert_eq!(embedding_keyring_alias(EmbeddingProviderType::Ollama), None);
    }

    // =====================================================================
    // validate_embedding_keyring_provider tests
    // =====================================================================

    #[test]
    fn validate_embedding_keyring_provider_accepts_valid() {
        assert_eq!(
            validate_embedding_keyring_provider("qwen_embedding"),
            Some("qwen_embedding")
        );
        assert_eq!(
            validate_embedding_keyring_provider("glm_embedding"),
            Some("glm_embedding")
        );
        assert_eq!(
            validate_embedding_keyring_provider("openai_embedding"),
            Some("openai_embedding")
        );
    }

    #[test]
    fn validate_embedding_keyring_provider_case_insensitive() {
        assert_eq!(
            validate_embedding_keyring_provider("QWEN_EMBEDDING"),
            Some("qwen_embedding")
        );
        assert_eq!(
            validate_embedding_keyring_provider("GLM_Embedding"),
            Some("glm_embedding")
        );
    }

    #[test]
    fn validate_embedding_keyring_provider_rejects_invalid() {
        assert_eq!(validate_embedding_keyring_provider("qwen"), None);
        assert_eq!(validate_embedding_keyring_provider("anthropic"), None);
        assert_eq!(validate_embedding_keyring_provider(""), None);
    }

    // =====================================================================
    // config_to_response tests
    // =====================================================================

    #[test]
    fn config_to_response_tfidf_defaults() {
        let config = EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf);
        let response = config_to_response(&config, None);

        assert_eq!(response.provider, "tf_idf");
        assert_eq!(response.model, "tfidf");
        assert_eq!(response.dimension, 0); // dynamic for TF-IDF
        assert!(response.base_url.is_none());
        assert!(response.fallback_provider.is_none());
    }

    #[test]
    fn config_to_response_openai_with_fallback() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI);
        config.dimension = Some(1536);

        let response = config_to_response(&config, Some(EmbeddingProviderType::TfIdf));

        assert_eq!(response.provider, "open_ai");
        assert_eq!(response.model, "text-embedding-3-small");
        assert_eq!(response.dimension, 1536);
        assert_eq!(response.fallback_provider, Some("tf_idf".to_string()));
    }

    #[test]
    fn config_to_response_with_base_url() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::Ollama);
        config.base_url = Some("http://localhost:11434".to_string());

        let response = config_to_response(&config, None);

        assert_eq!(
            response.base_url,
            Some("http://localhost:11434".to_string())
        );
    }

    // =====================================================================
    // PersistedEmbeddingConfig serde tests
    // =====================================================================

    #[test]
    fn persisted_config_serde_roundtrip() {
        let config = PersistedEmbeddingConfig {
            provider: EmbeddingProviderType::Qwen,
            model: "text-embedding-v3".to_string(),
            base_url: Some("https://custom.endpoint.com".to_string()),
            dimension: Some(1024),
            batch_size: 25,
            fallback_provider: Some(EmbeddingProviderType::TfIdf),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: PersistedEmbeddingConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.provider, EmbeddingProviderType::Qwen);
        assert_eq!(deserialized.model, "text-embedding-v3");
        assert_eq!(
            deserialized.base_url,
            Some("https://custom.endpoint.com".to_string())
        );
        assert_eq!(deserialized.dimension, Some(1024));
        assert_eq!(deserialized.batch_size, 25);
        assert_eq!(
            deserialized.fallback_provider,
            Some(EmbeddingProviderType::TfIdf)
        );
    }

    #[test]
    fn persisted_config_serde_skips_none_fields() {
        let config = PersistedEmbeddingConfig {
            provider: EmbeddingProviderType::TfIdf,
            model: "tfidf".to_string(),
            base_url: None,
            dimension: None,
            batch_size: 32,
            fallback_provider: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("base_url"));
        assert!(!json.contains("dimension"));
        assert!(!json.contains("fallback_provider"));
    }

    // =====================================================================
    // SetEmbeddingConfigRequest serde tests
    // =====================================================================

    #[test]
    fn set_config_request_deserialize_minimal() {
        let json = r#"{"provider": "ollama"}"#;
        let req: SetEmbeddingConfigRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.provider, "ollama");
        assert!(req.model.is_none());
        assert!(req.base_url.is_none());
        assert!(req.dimension.is_none());
        assert!(req.batch_size.is_none());
        assert!(req.fallback_provider.is_none());
    }

    #[test]
    fn set_config_request_deserialize_full() {
        let json = r#"{
            "provider": "qwen",
            "model": "text-embedding-v3",
            "base_url": "https://custom.api.com",
            "dimension": 1024,
            "batch_size": 25,
            "fallback_provider": "tf_idf"
        }"#;
        let req: SetEmbeddingConfigRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.provider, "qwen");
        assert_eq!(req.model, Some("text-embedding-v3".to_string()));
        assert_eq!(req.base_url, Some("https://custom.api.com".to_string()));
        assert_eq!(req.dimension, Some(1024));
        assert_eq!(req.batch_size, Some(25));
        assert_eq!(req.fallback_provider, Some("tf_idf".to_string()));
    }

    // =====================================================================
    // SetEmbeddingApiKeyRequest validation tests
    // =====================================================================

    #[test]
    fn set_api_key_request_deserialize() {
        let json = r#"{
            "provider": "qwen_embedding",
            "api_key": "sk-test-key-12345"
        }"#;
        let req: SetEmbeddingApiKeyRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.provider, "qwen_embedding");
        assert_eq!(req.api_key, "sk-test-key-12345");
    }

    // =====================================================================
    // list_embedding_providers tests
    // =====================================================================

    #[tokio::test]
    async fn list_providers_returns_all_five() {
        let result = list_embedding_providers().await;
        assert!(result.success);
        let providers = result.data.unwrap();
        assert_eq!(providers.len(), 5);
    }

    #[tokio::test]
    async fn list_providers_includes_expected_types() {
        let result = list_embedding_providers().await;
        let providers = result.data.unwrap();

        let types: Vec<EmbeddingProviderType> = providers.iter().map(|p| p.provider_type).collect();

        assert!(types.contains(&EmbeddingProviderType::TfIdf));
        assert!(types.contains(&EmbeddingProviderType::Ollama));
        assert!(types.contains(&EmbeddingProviderType::Qwen));
        assert!(types.contains(&EmbeddingProviderType::Glm));
        assert!(types.contains(&EmbeddingProviderType::OpenAI));
    }

    #[tokio::test]
    async fn list_providers_local_providers_dont_require_key() {
        let result = list_embedding_providers().await;
        let providers = result.data.unwrap();

        for p in &providers {
            if p.is_local {
                assert!(
                    !p.requires_api_key,
                    "{} is local but requires API key",
                    p.display_name
                );
            }
        }
    }

    #[tokio::test]
    async fn list_providers_remote_providers_require_key() {
        let result = list_embedding_providers().await;
        let providers = result.data.unwrap();

        for p in &providers {
            if !p.is_local {
                assert!(
                    p.requires_api_key,
                    "{} is remote but does not require API key",
                    p.display_name
                );
            }
        }
    }

    // =====================================================================
    // EmbeddingHealthResponse serde tests
    // =====================================================================

    #[test]
    fn health_response_serialize_healthy() {
        let resp = EmbeddingHealthResponse {
            healthy: true,
            message: "All good".to_string(),
            latency_ms: Some(42),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"healthy\":true"));
        assert!(json.contains("\"latency_ms\":42"));
    }

    #[test]
    fn health_response_serialize_unhealthy_no_latency() {
        let resp = EmbeddingHealthResponse {
            healthy: false,
            message: "API key missing".to_string(),
            latency_ms: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"healthy\":false"));
        assert!(!json.contains("latency_ms"));
    }

    // =====================================================================
    // SetEmbeddingConfigResponse serde tests
    // =====================================================================

    #[test]
    fn set_config_response_serialize() {
        let resp = SetEmbeddingConfigResponse {
            provider: "qwen".to_string(),
            model: "text-embedding-v3".to_string(),
            reindex_required: true,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"reindex_required\":true"));
        assert!(json.contains("\"provider\":\"qwen\""));
    }

    // =====================================================================
    // SetEmbeddingApiKeyResponse serde tests
    // =====================================================================

    #[test]
    fn set_api_key_response_serialize() {
        let resp = SetEmbeddingApiKeyResponse { success: true };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":true"));
    }

    // =====================================================================
    // EmbeddingConfigResponse serde tests
    // =====================================================================

    #[test]
    fn embedding_config_response_serialize() {
        let resp = EmbeddingConfigResponse {
            provider: "ollama".to_string(),
            model: "nomic-embed-text".to_string(),
            base_url: Some("http://localhost:11434".to_string()),
            dimension: 768,
            batch_size: 32,
            fallback_provider: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"provider\":\"ollama\""));
        assert!(json.contains("\"dimension\":768"));
        assert!(!json.contains("fallback_provider"));
    }
}
