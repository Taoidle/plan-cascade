//! Embedding Provider Abstraction Layer
//!
//! Defines the async `EmbeddingProvider` trait and supporting types for
//! pluggable embedding backends. Each backend (TF-IDF, Ollama, Qwen, GLM,
//! OpenAI) implements this trait to provide a unified embedding interface.
//!
//! ## Design Decision (ADR-F001)
//!
//! Embedding is a distinct responsibility from chat completion. Rather than
//! extending `LlmProvider`, we define a separate `EmbeddingProvider` trait
//! that is async-friendly and object-safe (`Send + Sync` for Tauri/Tokio).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::fmt;

use crate::services::proxy::ProxyConfig;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during embedding operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EmbeddingError {
    /// Authentication failed (invalid or missing API key).
    AuthenticationFailed { message: String },

    /// The requested model was not found or is not available.
    ModelNotFound { model: String },

    /// The provider is not reachable or not running.
    ProviderUnavailable { message: String },

    /// The input batch exceeds the provider's maximum batch size.
    BatchSizeLimitExceeded {
        requested: usize,
        max_allowed: usize,
    },

    /// The input text exceeds the provider's maximum token/character limit.
    InputTooLong { message: String },

    /// A network or connection error occurred.
    NetworkError { message: String },

    /// The provider returned an unexpected or unparseable response.
    ParseError { message: String },

    /// The provider returned an HTTP error.
    ServerError {
        message: String,
        status: Option<u16>,
    },

    /// Rate limit exceeded.
    RateLimited {
        message: String,
        retry_after: Option<u32>,
    },

    /// Configuration is invalid or incomplete.
    InvalidConfig { message: String },

    /// Any other error.
    Other { message: String },
}

impl fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthenticationFailed { message } => {
                write!(f, "authentication failed: {}", message)
            }
            Self::ModelNotFound { model } => write!(f, "model not found: {}", model),
            Self::ProviderUnavailable { message } => {
                write!(f, "provider unavailable: {}", message)
            }
            Self::BatchSizeLimitExceeded {
                requested,
                max_allowed,
            } => write!(
                f,
                "batch size {} exceeds maximum {}",
                requested, max_allowed
            ),
            Self::InputTooLong { message } => write!(f, "input too long: {}", message),
            Self::NetworkError { message } => write!(f, "network error: {}", message),
            Self::ParseError { message } => write!(f, "parse error: {}", message),
            Self::ServerError { message, status } => {
                if let Some(code) = status {
                    write!(f, "server error (HTTP {}): {}", code, message)
                } else {
                    write!(f, "server error: {}", message)
                }
            }
            Self::RateLimited { message, .. } => write!(f, "rate limited: {}", message),
            Self::InvalidConfig { message } => write!(f, "invalid config: {}", message),
            Self::Other { message } => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for EmbeddingError {}

impl EmbeddingError {
    /// Whether this error is transient and the operation should be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            EmbeddingError::NetworkError { .. }
                | EmbeddingError::RateLimited { .. }
                | EmbeddingError::ServerError { .. }
                | EmbeddingError::ProviderUnavailable { .. }
        )
    }

    /// For rate-limited errors, return the suggested wait time in seconds.
    pub fn retry_after_secs(&self) -> Option<u64> {
        if let EmbeddingError::RateLimited { retry_after, .. } = self {
            retry_after.map(|s| s as u64)
        } else {
            None
        }
    }
}

/// Convenience alias for embedding operation results.
pub type EmbeddingResult<T> = Result<T, EmbeddingError>;

// ---------------------------------------------------------------------------
// Provider type enum
// ---------------------------------------------------------------------------

/// Identifies the embedding backend type.
///
/// Each variant corresponds to a concrete `EmbeddingProvider` implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProviderType {
    /// Local TF-IDF vectorization (no external dependencies).
    TfIdf,
    /// Ollama local embedding models.
    Ollama,
    /// Qwen embedding models via DashScope API.
    Qwen,
    /// GLM embedding models via ZhipuAI API.
    Glm,
    /// OpenAI embedding models (text-embedding-3-small, etc.).
    #[serde(rename = "open_ai")]
    OpenAI,
}

impl EmbeddingProviderType {
    /// Returns the default capability metadata for this provider type.
    pub fn default_capability(&self) -> EmbeddingProviderCapability {
        match self {
            Self::TfIdf => EmbeddingProviderCapability {
                provider_type: *self,
                display_name: "TF-IDF (Local)".to_string(),
                is_local: true,
                requires_api_key: false,
                default_model: "tfidf".to_string(),
                default_dimension: 0, // dynamic, depends on vocabulary
                max_batch_size: 1000,
                supported_dimensions: None,
            },
            Self::Ollama => EmbeddingProviderCapability {
                provider_type: *self,
                display_name: "Ollama".to_string(),
                is_local: true,
                requires_api_key: false,
                default_model: "nomic-embed-text".to_string(),
                default_dimension: 768,
                max_batch_size: 64,
                supported_dimensions: None,
            },
            Self::Qwen => EmbeddingProviderCapability {
                provider_type: *self,
                display_name: "Qwen (DashScope)".to_string(),
                is_local: false,
                requires_api_key: true,
                default_model: "text-embedding-v3".to_string(),
                default_dimension: 1024,
                max_batch_size: 25,
                supported_dimensions: Some(vec![512, 1024, 1536]),
            },
            Self::Glm => EmbeddingProviderCapability {
                provider_type: *self,
                display_name: "GLM (ZhipuAI)".to_string(),
                is_local: false,
                requires_api_key: true,
                default_model: "embedding-3".to_string(),
                default_dimension: 2048,
                max_batch_size: 64,
                supported_dimensions: Some(vec![256, 512, 1024, 2048]),
            },
            Self::OpenAI => EmbeddingProviderCapability {
                provider_type: *self,
                display_name: "OpenAI".to_string(),
                is_local: false,
                requires_api_key: true,
                default_model: "text-embedding-3-small".to_string(),
                default_dimension: 1536,
                max_batch_size: 2048,
                supported_dimensions: Some(vec![256, 512, 1024, 1536, 3072]),
            },
        }
    }

    /// Returns all supported provider types.
    pub fn all() -> &'static [EmbeddingProviderType] {
        &[
            Self::TfIdf,
            Self::Ollama,
            Self::Qwen,
            Self::Glm,
            Self::OpenAI,
        ]
    }
}

impl fmt::Display for EmbeddingProviderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TfIdf => write!(f, "tfidf"),
            Self::Ollama => write!(f, "ollama"),
            Self::Qwen => write!(f, "qwen"),
            Self::Glm => write!(f, "glm"),
            Self::OpenAI => write!(f, "openai"),
        }
    }
}

// ---------------------------------------------------------------------------
// Provider configuration
// ---------------------------------------------------------------------------

/// Configuration for an embedding provider instance.
///
/// Used to construct a concrete `EmbeddingProvider` implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProviderConfig {
    /// The embedding backend type.
    pub provider: EmbeddingProviderType,

    /// Model identifier (e.g., "text-embedding-3-small", "nomic-embed-text").
    pub model: String,

    /// API key for remote providers. Not needed for local providers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Base URL override for the provider API. If `None`, the provider's
    /// default endpoint is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Desired embedding dimension. If `None`, the provider's default is used.
    /// Some providers support dimension reduction (e.g., OpenAI Matryoshka).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<usize>,

    /// Maximum number of texts to embed in a single request.
    /// Defaults to the provider's `max_batch_size` if not set.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Resolved proxy configuration for this embedding provider.
    /// None means no proxy (direct connection).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub proxy: Option<ProxyConfig>,
}

fn default_batch_size() -> usize {
    32
}

impl EmbeddingProviderConfig {
    /// Create a new configuration with sensible defaults for the given provider type.
    pub fn new(provider: EmbeddingProviderType) -> Self {
        let capability = provider.default_capability();
        Self {
            provider,
            model: capability.default_model,
            api_key: None,
            base_url: None,
            dimension: None,
            batch_size: capability.max_batch_size.min(default_batch_size()),
            proxy: None,
        }
    }

    /// Validate the configuration and return any issues found.
    ///
    /// Returns `Ok(())` if the configuration is valid, or an `EmbeddingError`
    /// describing the first validation failure.
    pub fn validate(&self) -> EmbeddingResult<()> {
        let capability = self.provider.default_capability();

        // Remote providers require an API key.
        if capability.requires_api_key && self.api_key.is_none() {
            return Err(EmbeddingError::InvalidConfig {
                message: format!(
                    "{} requires an API key but none was provided",
                    capability.display_name
                ),
            });
        }

        // Model name must not be empty.
        if self.model.trim().is_empty() {
            return Err(EmbeddingError::InvalidConfig {
                message: "model name must not be empty".to_string(),
            });
        }

        // Batch size must be positive and within the provider's limit.
        if self.batch_size == 0 {
            return Err(EmbeddingError::InvalidConfig {
                message: "batch_size must be at least 1".to_string(),
            });
        }
        if self.batch_size > capability.max_batch_size {
            return Err(EmbeddingError::InvalidConfig {
                message: format!(
                    "batch_size {} exceeds {} maximum of {}",
                    self.batch_size, capability.display_name, capability.max_batch_size
                ),
            });
        }

        // If a specific dimension is requested, validate it against supported dimensions.
        if let Some(dim) = self.dimension {
            if dim == 0 {
                return Err(EmbeddingError::InvalidConfig {
                    message: "dimension must be at least 1".to_string(),
                });
            }
            if let Some(ref supported) = capability.supported_dimensions {
                if !supported.contains(&dim) {
                    return Err(EmbeddingError::InvalidConfig {
                        message: format!(
                            "dimension {} is not supported by {}; supported: {:?}",
                            dim, capability.display_name, supported
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    /// Returns whether this configuration targets a local provider.
    pub fn is_local(&self) -> bool {
        self.provider.default_capability().is_local
    }

    /// Returns the effective dimension: the configured dimension or the provider's default.
    pub fn effective_dimension(&self) -> usize {
        self.dimension
            .unwrap_or(self.provider.default_capability().default_dimension)
    }

    /// Returns the effective model name (trimmed).
    pub fn effective_model(&self) -> &str {
        self.model.trim()
    }
}

// ---------------------------------------------------------------------------
// Persisted embedding config (shared between commands and index_manager)
// ---------------------------------------------------------------------------

/// Persisted form of the embedding config (no secrets).
///
/// Stored in the `settings` table under key `embedding_config`. Used by
/// both `commands::embedding` (to save user configuration) and
/// `IndexManager` (to read it when building providers for indexing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedEmbeddingConfig {
    pub provider: EmbeddingProviderType,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<usize>,
    pub batch_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_provider: Option<EmbeddingProviderType>,
}

/// Database setting key for persisted embedding config.
pub const EMBEDDING_CONFIG_SETTING_KEY: &str = "embedding_config";

// ---------------------------------------------------------------------------
// Provider capability metadata
// ---------------------------------------------------------------------------

/// Metadata describing a provider's capabilities and defaults.
///
/// Used for UI display, provider selection, and validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProviderCapability {
    /// Which provider this describes.
    pub provider_type: EmbeddingProviderType,

    /// Human-readable display name (e.g., "OpenAI", "Ollama").
    pub display_name: String,

    /// Whether this provider runs locally (no network calls).
    pub is_local: bool,

    /// Whether this provider requires an API key.
    pub requires_api_key: bool,

    /// The default model identifier for this provider.
    pub default_model: String,

    /// Default embedding dimension for the default model.
    pub default_dimension: usize,

    /// Maximum number of texts that can be embedded in one batch.
    pub max_batch_size: usize,

    /// If the provider supports multiple output dimensions, list them here.
    /// `None` means the dimension is fixed or determined by the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_dimensions: Option<Vec<usize>>,
}

// ---------------------------------------------------------------------------
// Embedding provider trait
// ---------------------------------------------------------------------------

/// Async trait for embedding providers.
///
/// Implementations produce dense vector representations of text. The trait is
/// object-safe and requires `Send + Sync` for safe use across Tokio tasks and
/// Tauri's multi-threaded runtime.
///
/// # Example
///
/// ```ignore
/// let provider: Box<dyn EmbeddingProvider> = create_provider(config).await?;
/// let vectors = provider.embed_documents(&["hello world", "foo bar"]).await?;
/// let query_vec = provider.embed_query("search term").await?;
/// ```
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed a batch of document texts into dense vectors.
    ///
    /// Each input string produces one vector. The returned vectors all have
    /// the same dimensionality (equal to `self.dimension()`).
    ///
    /// # Arguments
    /// * `documents` - Slice of document texts to embed.
    ///
    /// # Returns
    /// A vector of embedding vectors, one per input document.
    async fn embed_documents(&self, documents: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>>;

    /// Embed a single query text into a dense vector.
    ///
    /// Some providers use different models or parameters for queries vs
    /// documents (e.g., asymmetric retrieval). This method handles that
    /// distinction. The default implementation delegates to `embed_documents`
    /// with a single-element slice.
    ///
    /// # Arguments
    /// * `query` - The query text to embed.
    ///
    /// # Returns
    /// A single embedding vector.
    async fn embed_query(&self, query: &str) -> EmbeddingResult<Vec<f32>> {
        let results = self.embed_documents(&[query]).await?;
        results.into_iter().next().ok_or_else(|| EmbeddingError::Other {
            message: "embed_documents returned empty results for single query".to_string(),
        })
    }

    /// Returns the dimensionality of the embedding vectors produced.
    ///
    /// For providers with a fixed dimension, this returns that value.
    /// For TF-IDF, this returns 0 until the vocabulary is built.
    fn dimension(&self) -> usize;

    /// Check if the provider is healthy and reachable.
    ///
    /// For local providers (TF-IDF), this always succeeds.
    /// For API providers, this validates connectivity and credentials.
    /// For Ollama, this checks if the server is running.
    async fn health_check(&self) -> EmbeddingResult<()>;

    /// Returns whether this provider runs locally without network calls.
    fn is_local(&self) -> bool;

    /// Returns the maximum number of texts that can be embedded in a single
    /// batch request.
    fn max_batch_size(&self) -> usize;

    /// Returns the provider type identifier.
    fn provider_type(&self) -> EmbeddingProviderType;

    /// Returns a human-readable name for this provider instance.
    fn display_name(&self) -> &str;

    /// Returns `self` as `&dyn Any` to allow downcasting to concrete types.
    ///
    /// This enables accessing provider-specific methods (e.g., TF-IDF vocabulary
    /// building) through a `dyn EmbeddingProvider` reference.
    fn as_any(&self) -> &dyn Any;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // EmbeddingProviderType tests
    // =========================================================================

    #[test]
    fn provider_type_serde_roundtrip() {
        for provider in EmbeddingProviderType::all() {
            let json = serde_json::to_string(provider).unwrap();
            let deserialized: EmbeddingProviderType = serde_json::from_str(&json).unwrap();
            assert_eq!(*provider, deserialized);
        }
    }

    #[test]
    fn provider_type_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&EmbeddingProviderType::TfIdf).unwrap(),
            "\"tf_idf\""
        );
        assert_eq!(
            serde_json::to_string(&EmbeddingProviderType::Ollama).unwrap(),
            "\"ollama\""
        );
        assert_eq!(
            serde_json::to_string(&EmbeddingProviderType::Qwen).unwrap(),
            "\"qwen\""
        );
        assert_eq!(
            serde_json::to_string(&EmbeddingProviderType::Glm).unwrap(),
            "\"glm\""
        );
        assert_eq!(
            serde_json::to_string(&EmbeddingProviderType::OpenAI).unwrap(),
            "\"open_ai\""
        );
    }

    #[test]
    fn provider_type_deserialize_from_string() {
        let tfidf: EmbeddingProviderType = serde_json::from_str("\"tf_idf\"").unwrap();
        assert_eq!(tfidf, EmbeddingProviderType::TfIdf);

        let openai: EmbeddingProviderType = serde_json::from_str("\"open_ai\"").unwrap();
        assert_eq!(openai, EmbeddingProviderType::OpenAI);
    }

    #[test]
    fn provider_type_display() {
        assert_eq!(EmbeddingProviderType::TfIdf.to_string(), "tfidf");
        assert_eq!(EmbeddingProviderType::Ollama.to_string(), "ollama");
        assert_eq!(EmbeddingProviderType::Qwen.to_string(), "qwen");
        assert_eq!(EmbeddingProviderType::Glm.to_string(), "glm");
        assert_eq!(EmbeddingProviderType::OpenAI.to_string(), "openai");
    }

    #[test]
    fn provider_type_all_returns_five_variants() {
        assert_eq!(EmbeddingProviderType::all().len(), 5);
    }

    // =========================================================================
    // EmbeddingProviderCapability tests
    // =========================================================================

    #[test]
    fn default_capability_tfidf_is_local() {
        let cap = EmbeddingProviderType::TfIdf.default_capability();
        assert!(cap.is_local);
        assert!(!cap.requires_api_key);
        assert!(cap.supported_dimensions.is_none());
    }

    #[test]
    fn default_capability_ollama_is_local() {
        let cap = EmbeddingProviderType::Ollama.default_capability();
        assert!(cap.is_local);
        assert!(!cap.requires_api_key);
    }

    #[test]
    fn default_capability_remote_providers_require_api_key() {
        for provider in &[
            EmbeddingProviderType::Qwen,
            EmbeddingProviderType::Glm,
            EmbeddingProviderType::OpenAI,
        ] {
            let cap = provider.default_capability();
            assert!(!cap.is_local, "{} should not be local", cap.display_name);
            assert!(
                cap.requires_api_key,
                "{} should require API key",
                cap.display_name
            );
        }
    }

    #[test]
    fn default_capability_openai_supports_multiple_dimensions() {
        let cap = EmbeddingProviderType::OpenAI.default_capability();
        let dims = cap.supported_dimensions.as_ref().unwrap();
        assert!(dims.contains(&1536));
        assert!(dims.contains(&3072));
    }

    // =========================================================================
    // EmbeddingProviderConfig tests
    // =========================================================================

    #[test]
    fn config_new_uses_defaults() {
        let config = EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI);
        assert_eq!(config.provider, EmbeddingProviderType::OpenAI);
        assert_eq!(config.model, "text-embedding-3-small");
        assert!(config.api_key.is_none());
        assert!(config.base_url.is_none());
        assert!(config.dimension.is_none());
        assert!(config.batch_size > 0);
    }

    #[test]
    fn config_validate_requires_api_key_for_remote() {
        let config = EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI);
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, EmbeddingError::InvalidConfig { .. }));
    }

    #[test]
    fn config_validate_succeeds_for_local_without_key() {
        let config = EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_validate_succeeds_with_api_key() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI);
        config.api_key = Some("sk-test-key".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_validate_rejects_empty_model() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf);
        config.model = "  ".to_string();
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn config_validate_rejects_zero_batch_size() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf);
        config.batch_size = 0;
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn config_validate_rejects_excessive_batch_size() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::Ollama);
        config.batch_size = 10_000; // Exceeds Ollama max
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn config_validate_rejects_unsupported_dimension() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI);
        config.api_key = Some("sk-test".to_string());
        config.dimension = Some(999); // Not in supported list
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn config_validate_accepts_supported_dimension() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI);
        config.api_key = Some("sk-test".to_string());
        config.dimension = Some(1536);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_validate_rejects_zero_dimension() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf);
        config.dimension = Some(0);
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn config_is_local() {
        assert!(EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf).is_local());
        assert!(EmbeddingProviderConfig::new(EmbeddingProviderType::Ollama).is_local());
        assert!(!EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI).is_local());
    }

    #[test]
    fn config_effective_dimension_uses_default() {
        let config = EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI);
        assert_eq!(config.effective_dimension(), 1536);
    }

    #[test]
    fn config_effective_dimension_uses_override() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI);
        config.dimension = Some(512);
        assert_eq!(config.effective_dimension(), 512);
    }

    #[test]
    fn config_serde_roundtrip() {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen);
        config.api_key = Some("test-key".to_string());
        config.dimension = Some(1024);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: EmbeddingProviderConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.provider, EmbeddingProviderType::Qwen);
        assert_eq!(deserialized.model, config.model);
        assert_eq!(deserialized.api_key, Some("test-key".to_string()));
        assert_eq!(deserialized.dimension, Some(1024));
    }

    #[test]
    fn config_serde_skips_none_fields() {
        let config = EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf);
        let json = serde_json::to_string(&config).unwrap();
        // api_key and base_url should not appear in JSON
        assert!(!json.contains("api_key"));
        assert!(!json.contains("base_url"));
    }

    // =========================================================================
    // EmbeddingError tests
    // =========================================================================

    #[test]
    fn error_is_retryable() {
        assert!(EmbeddingError::NetworkError {
            message: "timeout".into()
        }
        .is_retryable());
        assert!(EmbeddingError::RateLimited {
            message: "slow down".into(),
            retry_after: Some(5)
        }
        .is_retryable());
        assert!(EmbeddingError::ServerError {
            message: "500".into(),
            status: Some(500)
        }
        .is_retryable());
        assert!(EmbeddingError::ProviderUnavailable {
            message: "offline".into()
        }
        .is_retryable());

        assert!(!EmbeddingError::AuthenticationFailed {
            message: "bad key".into()
        }
        .is_retryable());
        assert!(!EmbeddingError::InvalidConfig {
            message: "bad config".into()
        }
        .is_retryable());
    }

    #[test]
    fn error_retry_after_secs() {
        let err = EmbeddingError::RateLimited {
            message: "slow down".into(),
            retry_after: Some(30),
        };
        assert_eq!(err.retry_after_secs(), Some(30));

        let err = EmbeddingError::NetworkError {
            message: "timeout".into(),
        };
        assert_eq!(err.retry_after_secs(), None);
    }

    #[test]
    fn error_display() {
        let err = EmbeddingError::AuthenticationFailed {
            message: "invalid key".into(),
        };
        assert_eq!(err.to_string(), "authentication failed: invalid key");

        let err = EmbeddingError::BatchSizeLimitExceeded {
            requested: 100,
            max_allowed: 64,
        };
        assert_eq!(err.to_string(), "batch size 100 exceeds maximum 64");
    }

    #[test]
    fn error_serde_roundtrip() {
        let err = EmbeddingError::ServerError {
            message: "internal error".into(),
            status: Some(500),
        };
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: EmbeddingError = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            EmbeddingError::ServerError { status: Some(500), .. }
        ));
    }

    // =========================================================================
    // Trait object safety tests
    // =========================================================================

    #[test]
    fn embedding_provider_trait_is_object_safe() {
        // This compiles only if the trait is object-safe.
        fn _assert_object_safe(_: &dyn EmbeddingProvider) {}
    }

    #[test]
    fn embedding_provider_trait_is_send_sync() {
        fn _assert_send_sync<T: Send + Sync>() {}
        // Verify the trait object itself is Send + Sync.
        _assert_send_sync::<Box<dyn EmbeddingProvider>>();
    }
}
