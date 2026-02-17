//! Ollama Embedding Provider
//!
//! Implements the `EmbeddingProvider` trait for Ollama local embedding models
//! using the `ollama-rs` native SDK. Supports configurable model names and
//! base URLs, requires no API key, and provides robust local health checks.
//!
//! ## Default Model
//!
//! Uses `nomic-embed-text` (768-dimensional) by default. The dimension is
//! detected automatically after the first successful embedding call.

use async_trait::async_trait;
use ollama_rs::generation::embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest};
use ollama_rs::Ollama;
use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::embedding_provider::{
    EmbeddingError, EmbeddingProvider, EmbeddingProviderConfig, EmbeddingProviderType,
    EmbeddingResult,
};
use crate::services::proxy::{build_http_client, ProxyConfig};

/// Default Ollama API endpoint.
const OLLAMA_DEFAULT_URL: &str = "http://localhost:11434";

/// Default embedding model.
const DEFAULT_MODEL: &str = "nomic-embed-text";

/// Default dimension for nomic-embed-text.
const DEFAULT_DIMENSION: usize = 768;

/// Maximum batch size for Ollama embedding requests.
const MAX_BATCH_SIZE: usize = 64;

/// Ollama embedding provider using the native ollama-rs SDK.
///
/// This provider runs entirely locally, requiring a running Ollama server
/// but no API keys. It supports batch embedding via Ollama's `/api/embed`
/// endpoint which accepts multiple inputs in a single request.
pub struct OllamaEmbeddingProvider {
    /// The ollama-rs client instance.
    client: Ollama,
    /// Model name to use for embedding requests.
    model: String,
    /// Detected embedding dimension (updated after first successful call).
    /// Uses AtomicUsize for lock-free interior mutability compatible with
    /// the `Send + Sync` requirement of the trait.
    dimension: AtomicUsize,
    /// Human-readable display name for this provider instance.
    display_name: String,
    /// The base URL string (for error messages).
    base_url: String,
}

impl OllamaEmbeddingProvider {
    /// Create a new Ollama embedding provider from an `EmbeddingProviderConfig`.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration specifying model, base_url, and dimension.
    ///
    /// # Returns
    ///
    /// A configured `OllamaEmbeddingProvider` ready to make embedding requests.
    pub fn new(config: &EmbeddingProviderConfig) -> Self {
        let model = if config.model.trim().is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            config.model.clone()
        };

        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(OLLAMA_DEFAULT_URL)
            .to_string();

        let client = Self::create_client(&base_url, config.proxy.as_ref());

        let initial_dimension = config.dimension.unwrap_or(DEFAULT_DIMENSION);
        let display_name = format!("Ollama ({})", model);

        Self {
            client,
            model,
            dimension: AtomicUsize::new(initial_dimension),
            display_name,
            base_url,
        }
    }

    /// Create an Ollama SDK client from a base URL string.
    ///
    /// Parses the URL to extract host and port for `Ollama::new()`.
    /// Falls back to `Ollama::default()` if parsing fails.
    fn create_client(base_url: &str, proxy: Option<&ProxyConfig>) -> Ollama {
        if let Ok(parsed) = url::Url::parse(base_url) {
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("localhost");
            let port = parsed.port().unwrap_or(11434);
            let host_url = format!("{}://{}", scheme, host);
            if proxy.is_some() {
                let http_client = build_http_client(proxy);
                Ollama::new_with_client(host_url, port, http_client)
            } else {
                Ollama::new(host_url, port)
            }
        } else {
            Ollama::default()
        }
    }

    /// Map an ollama-rs error to our `EmbeddingError` type.
    fn map_ollama_error(&self, err: ollama_rs::error::OllamaError) -> EmbeddingError {
        let msg = err.to_string();

        if msg.contains("connect") || msg.contains("Connection refused") {
            EmbeddingError::ProviderUnavailable {
                message: format!(
                    "Cannot connect to Ollama at {}. Is the Ollama server running? \
                     Start it with: ollama serve",
                    self.base_url
                ),
            }
        } else if msg.contains("not found") || msg.contains("404") {
            EmbeddingError::ModelNotFound {
                model: self.model.clone(),
            }
        } else if msg.contains("model") && msg.contains("not") {
            // Ollama may return errors like "model 'xxx' not found"
            EmbeddingError::ModelNotFound {
                model: self.model.clone(),
            }
        } else {
            EmbeddingError::NetworkError { message: msg }
        }
    }

    /// Update the stored dimension from a successful embedding response.
    fn update_dimension(&self, embeddings: &[Vec<f32>]) {
        if let Some(first) = embeddings.first() {
            if !first.is_empty() {
                self.dimension.store(first.len(), Ordering::Relaxed);
            }
        }
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbeddingProvider {
    async fn embed_documents(&self, documents: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        // Enforce batch size limit
        if documents.len() > MAX_BATCH_SIZE {
            return Err(EmbeddingError::BatchSizeLimitExceeded {
                requested: documents.len(),
                max_allowed: MAX_BATCH_SIZE,
            });
        }

        // Build the request using the Multiple variant for batch embedding.
        // ollama-rs EmbeddingsInput::from(Vec<&str>) converts to Multiple.
        let input = EmbeddingsInput::from(documents.to_vec());
        let request = GenerateEmbeddingsRequest::new(self.model.clone(), input);

        let response = self
            .client
            .generate_embeddings(request)
            .await
            .map_err(|e| self.map_ollama_error(e))?;

        // Validate response: we should get one embedding per input document
        if response.embeddings.len() != documents.len() {
            return Err(EmbeddingError::ParseError {
                message: format!(
                    "expected {} embeddings but Ollama returned {}",
                    documents.len(),
                    response.embeddings.len()
                ),
            });
        }

        // Update cached dimension from the first embedding
        self.update_dimension(&response.embeddings);

        Ok(response.embeddings)
    }

    async fn embed_query(&self, query: &str) -> EmbeddingResult<Vec<f32>> {
        // Ollama does not distinguish between query and document embeddings,
        // so we use the same endpoint with a single input.
        let input = EmbeddingsInput::from(query);
        let request = GenerateEmbeddingsRequest::new(self.model.clone(), input);

        let response = self
            .client
            .generate_embeddings(request)
            .await
            .map_err(|e| self.map_ollama_error(e))?;

        // Update cached dimension
        self.update_dimension(&response.embeddings);

        response
            .embeddings
            .into_iter()
            .next()
            .ok_or_else(|| EmbeddingError::ParseError {
                message: "Ollama returned empty embeddings for query".to_string(),
            })
    }

    fn dimension(&self) -> usize {
        self.dimension.load(Ordering::Relaxed)
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        // Step 1: Check if Ollama server is reachable by listing models
        let models = self
            .client
            .list_local_models()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect") || msg.contains("Connection refused") {
                    EmbeddingError::ProviderUnavailable {
                        message: format!(
                            "Cannot connect to Ollama at {}. Is the Ollama server running? \
                             Start it with: ollama serve",
                            self.base_url
                        ),
                    }
                } else {
                    EmbeddingError::NetworkError { message: msg }
                }
            })?;

        // Step 2: Check if the configured model is available locally
        let model_base = self.model.split(':').next().unwrap_or(&self.model);
        let model_available = models.iter().any(|m| {
            let local_base = m.name.split(':').next().unwrap_or(&m.name);
            local_base == model_base || m.name == self.model
        });

        if !model_available {
            let available_names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
            return Err(EmbeddingError::ModelNotFound {
                model: format!(
                    "'{}' is not available locally. Available models: [{}]. \
                     Pull it with: ollama pull {}",
                    self.model,
                    available_names.join(", "),
                    self.model
                ),
            });
        }

        Ok(())
    }

    fn is_local(&self) -> bool {
        true
    }

    fn max_batch_size(&self) -> usize {
        MAX_BATCH_SIZE
    }

    fn provider_type(&self) -> EmbeddingProviderType {
        EmbeddingProviderType::Ollama
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::orchestrator::embedding_provider::EmbeddingProviderConfig;

    fn default_config() -> EmbeddingProviderConfig {
        EmbeddingProviderConfig::new(EmbeddingProviderType::Ollama)
    }

    fn config_with_model(model: &str) -> EmbeddingProviderConfig {
        EmbeddingProviderConfig {
            model: model.to_string(),
            ..EmbeddingProviderConfig::new(EmbeddingProviderType::Ollama)
        }
    }

    fn config_with_base_url(base_url: &str) -> EmbeddingProviderConfig {
        EmbeddingProviderConfig {
            base_url: Some(base_url.to_string()),
            ..EmbeddingProviderConfig::new(EmbeddingProviderType::Ollama)
        }
    }

    // =========================================================================
    // Construction tests
    // =========================================================================

    #[test]
    fn new_with_default_config() {
        let config = default_config();
        let provider = OllamaEmbeddingProvider::new(&config);

        assert_eq!(provider.model, "nomic-embed-text");
        assert_eq!(provider.dimension(), 768);
        assert_eq!(provider.display_name(), "Ollama (nomic-embed-text)");
        assert_eq!(provider.provider_type(), EmbeddingProviderType::Ollama);
        assert!(provider.is_local());
        assert_eq!(provider.max_batch_size(), 64);
    }

    #[test]
    fn new_with_custom_model() {
        let config = config_with_model("mxbai-embed-large");
        let provider = OllamaEmbeddingProvider::new(&config);

        assert_eq!(provider.model, "mxbai-embed-large");
        assert_eq!(provider.display_name(), "Ollama (mxbai-embed-large)");
    }

    #[test]
    fn new_with_custom_base_url() {
        let config = config_with_base_url("http://192.168.1.100:11434");
        let provider = OllamaEmbeddingProvider::new(&config);

        assert_eq!(provider.base_url, "http://192.168.1.100:11434");
    }

    #[test]
    fn new_with_empty_model_uses_default() {
        let config = EmbeddingProviderConfig {
            model: "  ".to_string(),
            ..default_config()
        };
        let provider = OllamaEmbeddingProvider::new(&config);

        assert_eq!(provider.model, DEFAULT_MODEL);
    }

    #[test]
    fn new_with_custom_dimension() {
        let config = EmbeddingProviderConfig {
            dimension: Some(1024),
            ..default_config()
        };
        let provider = OllamaEmbeddingProvider::new(&config);

        assert_eq!(provider.dimension(), 1024);
    }

    // =========================================================================
    // Trait property tests
    // =========================================================================

    #[test]
    fn provider_is_local() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        assert!(provider.is_local());
    }

    #[test]
    fn provider_type_is_ollama() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        assert_eq!(provider.provider_type(), EmbeddingProviderType::Ollama);
    }

    #[test]
    fn max_batch_size_is_64() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        assert_eq!(provider.max_batch_size(), 64);
    }

    #[test]
    fn display_name_includes_model() {
        let config = config_with_model("all-minilm");
        let provider = OllamaEmbeddingProvider::new(&config);
        assert!(provider.display_name().contains("all-minilm"));
        assert!(provider.display_name().contains("Ollama"));
    }

    // =========================================================================
    // Dimension tracking tests
    // =========================================================================

    #[test]
    fn dimension_defaults_to_768() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        assert_eq!(provider.dimension(), 768);
    }

    #[test]
    fn update_dimension_from_embeddings() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        assert_eq!(provider.dimension(), 768);

        // Simulate receiving embeddings with different dimension
        let fake_embeddings = vec![vec![0.0f32; 384]];
        provider.update_dimension(&fake_embeddings);
        assert_eq!(provider.dimension(), 384);
    }

    #[test]
    fn update_dimension_ignores_empty_embeddings() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        provider.update_dimension(&[]);
        assert_eq!(provider.dimension(), 768); // unchanged

        provider.update_dimension(&[vec![]]);
        assert_eq!(provider.dimension(), 768); // unchanged
    }

    // =========================================================================
    // Error mapping tests
    // =========================================================================

    #[test]
    fn map_connection_refused_error() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        let err = provider.map_ollama_error(ollama_rs::error::OllamaError::Other(
            "Connection refused".to_string(),
        ));
        assert!(matches!(err, EmbeddingError::ProviderUnavailable { .. }));
        let msg = err.to_string();
        assert!(msg.contains("ollama serve"), "should suggest ollama serve");
    }

    #[test]
    fn map_not_found_error() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        let err = provider.map_ollama_error(ollama_rs::error::OllamaError::Other(
            "model not found".to_string(),
        ));
        assert!(matches!(err, EmbeddingError::ModelNotFound { .. }));
    }

    #[test]
    fn map_generic_error() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        let err = provider.map_ollama_error(ollama_rs::error::OllamaError::Other(
            "something unexpected".to_string(),
        ));
        assert!(matches!(err, EmbeddingError::NetworkError { .. }));
    }

    // =========================================================================
    // Trait object safety tests
    // =========================================================================

    #[test]
    fn provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OllamaEmbeddingProvider>();
    }

    #[test]
    fn provider_is_object_safe() {
        fn _assert_object_safe(_: &dyn EmbeddingProvider) {}
    }

    // =========================================================================
    // Integration tests (require running Ollama server — marked #[ignore])
    // =========================================================================

    #[tokio::test]
    #[ignore = "requires a running Ollama server with nomic-embed-text model"]
    async fn integration_health_check() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        let result = provider.health_check().await;
        assert!(result.is_ok(), "health_check failed: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore = "requires a running Ollama server with nomic-embed-text model"]
    async fn integration_embed_query() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        let result = provider.embed_query("hello world").await;
        assert!(result.is_ok(), "embed_query failed: {:?}", result.err());

        let embedding = result.unwrap();
        assert!(!embedding.is_empty());
        // After first call, dimension should be updated
        assert_eq!(provider.dimension(), embedding.len());
    }

    #[tokio::test]
    #[ignore = "requires a running Ollama server with nomic-embed-text model"]
    async fn integration_embed_documents() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        let docs = vec!["hello world", "foo bar baz", "rust programming language"];
        let result = provider.embed_documents(&docs).await;
        assert!(
            result.is_ok(),
            "embed_documents failed: {:?}",
            result.err()
        );

        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 3);
        // All embeddings should have the same dimension
        let dim = embeddings[0].len();
        assert!(dim > 0);
        for emb in &embeddings {
            assert_eq!(emb.len(), dim);
        }
    }

    #[tokio::test]
    #[ignore = "requires a running Ollama server with nomic-embed-text model"]
    async fn integration_embed_empty_documents() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        let result = provider.embed_documents(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn embed_documents_rejects_oversized_batch() {
        let provider = OllamaEmbeddingProvider::new(&default_config());
        let docs: Vec<&str> = (0..65).map(|_| "test").collect();
        let result = provider.embed_documents(&docs).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::BatchSizeLimitExceeded {
                requested: 65,
                max_allowed: 64,
            }
        ));
    }
}
