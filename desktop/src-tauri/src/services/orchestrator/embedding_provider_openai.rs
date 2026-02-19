//! OpenAI Embedding Provider
//!
//! Implements the `EmbeddingProvider` trait for OpenAI's embedding models
//! using reqwest HTTP transport. Supports configurable output dimensions
//! via the Matryoshka `dimensions` parameter and batch embedding up to
//! 2048 inputs per request.
//!
//! ## API Details
//!
//! - Endpoint: `POST https://api.openai.com/v1/embeddings`
//! - Auth: `Authorization: Bearer {api_key}`
//! - Body: `{ model, input: ["text1", ...], dimensions? }`
//! - Response: `{ data: [{ embedding, index }], model, usage }`
//!
//! ## Design Decision
//!
//! API keys are stored in the OS keyring under the `openai_embedding` alias
//! and injected via `EmbeddingProviderConfig.api_key` at construction time.
//! Supports custom `base_url` for OpenAI-compatible APIs (e.g., Azure OpenAI,
//! vLLM, LiteLLM).

use async_trait::async_trait;
use serde::Deserialize;
use std::any::Any;

use super::embedding_provider::{
    EmbeddingError, EmbeddingProvider, EmbeddingProviderConfig, EmbeddingProviderType,
    EmbeddingResult,
};
use crate::services::proxy::build_http_client;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default OpenAI embedding API endpoint.
const OPENAI_EMBEDDING_API_URL: &str = "https://api.openai.com/v1/embeddings";

/// Default embedding model.
const DEFAULT_MODEL: &str = "text-embedding-3-small";

/// Default embedding dimension for text-embedding-3-small.
const DEFAULT_DIMENSION: usize = 1536;

/// Maximum batch size supported by OpenAI embedding API.
const MAX_BATCH_SIZE: usize = 2048;

// ---------------------------------------------------------------------------
// API response types
// ---------------------------------------------------------------------------

/// OpenAI embedding API response.
#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbeddingData>,
    #[allow(dead_code)]
    model: Option<String>,
    #[allow(dead_code)]
    usage: Option<OpenAIUsage>,
}

/// Individual embedding result within the response.
#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingData {
    embedding: Vec<f32>,
    index: usize,
    #[allow(dead_code)]
    object: Option<String>,
}

/// Token usage statistics from the API.
#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    #[allow(dead_code)]
    prompt_tokens: Option<u32>,
    #[allow(dead_code)]
    total_tokens: Option<u32>,
}

/// OpenAI API error response.
#[derive(Debug, Deserialize)]
struct OpenAIErrorResponse {
    error: Option<OpenAIErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct OpenAIErrorDetail {
    #[allow(dead_code)]
    code: Option<String>,
    message: Option<String>,
    #[allow(dead_code)]
    r#type: Option<String>,
}

// ---------------------------------------------------------------------------
// Provider implementation
// ---------------------------------------------------------------------------

/// OpenAI embedding provider using the OpenAI embeddings API.
///
/// Uses reqwest for HTTP transport. Supports configurable dimensions via
/// the Matryoshka `dimensions` parameter (text-embedding-3-*) and batch
/// embedding up to 2048 inputs per request.
///
/// Also works with any OpenAI-compatible API (Azure, vLLM, LiteLLM) via
/// the `base_url` configuration option.
///
/// # Thread Safety
///
/// This struct is `Send + Sync` — the reqwest `Client` is internally
/// arc'd and clone-safe, and all fields are immutable after construction.
pub struct OpenAIEmbeddingProvider {
    /// The reqwest HTTP client.
    client: reqwest::Client,
    /// API key for authentication.
    api_key: String,
    /// Model name (e.g., "text-embedding-3-small").
    model: String,
    /// API base URL.
    base_url: String,
    /// Embedding dimension.
    dimension: usize,
    /// Human-readable display name.
    display_name: String,
}

impl OpenAIEmbeddingProvider {
    /// Create a new OpenAI embedding provider from an `EmbeddingProviderConfig`.
    pub fn new(config: &EmbeddingProviderConfig) -> Self {
        let model = if config.model.trim().is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            config.model.clone()
        };

        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(OPENAI_EMBEDDING_API_URL)
            .to_string();

        let dimension = config.dimension.unwrap_or(DEFAULT_DIMENSION);
        let display_name = format!("OpenAI ({})", model);
        let api_key = config.api_key.clone().unwrap_or_default();

        Self {
            client: build_http_client(config.proxy.as_ref()),
            api_key,
            model,
            base_url,
            dimension,
            display_name,
        }
    }

    /// Build the JSON request body for the embedding API.
    fn build_request_body(&self, input: serde_json::Value) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.model,
            "input": input,
        });

        // Include dimensions parameter for models that support Matryoshka
        // (text-embedding-3-small, text-embedding-3-large).
        // For text-embedding-ada-002 and compatible APIs, omit it to use
        // the model's native dimension.
        if self.dimension != DEFAULT_DIMENSION
            || self.model.contains("text-embedding-3")
        {
            body["dimensions"] = serde_json::json!(self.dimension);
        }

        body
    }

    /// Send a POST request to the embedding API and parse the response.
    async fn post_embeddings(
        &self,
        body: &serde_json::Value,
    ) -> EmbeddingResult<OpenAIEmbeddingResponse> {
        if self.api_key.is_empty() {
            return Err(EmbeddingError::AuthenticationFailed {
                message: "OpenAI API key is not configured. Set it via embedding config commands \
                          (keyring alias: 'openai_embedding')."
                    .to_string(),
            });
        }

        let response = self
            .client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        let status = response.status().as_u16();

        if status == 200 {
            let resp_text = response
                .text()
                .await
                .map_err(|e| EmbeddingError::NetworkError {
                    message: format!("failed to read response body: {}", e),
                })?;

            serde_json::from_str::<OpenAIEmbeddingResponse>(&resp_text).map_err(|e| {
                EmbeddingError::ParseError {
                    message: format!("failed to parse embedding response: {}", e),
                }
            })
        } else {
            let body_text = response.text().await.unwrap_or_default();
            Err(self.map_http_error(status, &body_text))
        }
    }

    /// Map a reqwest transport error to `EmbeddingError`.
    fn map_reqwest_error(&self, err: reqwest::Error) -> EmbeddingError {
        let msg = err.to_string();

        if err.is_connect() || msg.contains("connect") || msg.contains("Connection refused") {
            EmbeddingError::ProviderUnavailable {
                message: format!(
                    "Cannot connect to OpenAI API at {}. Check your network connectivity.",
                    self.base_url
                ),
            }
        } else if err.is_timeout() {
            EmbeddingError::NetworkError {
                message: format!("Request to OpenAI API timed out: {}", msg),
            }
        } else {
            EmbeddingError::NetworkError { message: msg }
        }
    }

    /// Map an HTTP error response to `EmbeddingError`.
    fn map_http_error(&self, status: u16, body_text: &str) -> EmbeddingError {
        // Try to parse structured error response
        let error_detail = serde_json::from_str::<OpenAIErrorResponse>(body_text)
            .ok()
            .and_then(|r| r.error);

        let error_message = error_detail
            .as_ref()
            .and_then(|d| d.message.as_deref())
            .unwrap_or(body_text);

        match status {
            401 => EmbeddingError::AuthenticationFailed {
                message: format!(
                    "OpenAI authentication failed: {}. Verify your API key \
                     (keyring alias: 'openai_embedding').",
                    error_message
                ),
            },
            429 => EmbeddingError::RateLimited {
                message: format!("OpenAI rate limit exceeded: {}", error_message),
                retry_after: None,
            },
            400 => {
                if error_message.contains("token") || error_message.contains("length") {
                    EmbeddingError::InputTooLong {
                        message: format!("OpenAI: {}", error_message),
                    }
                } else {
                    EmbeddingError::InvalidConfig {
                        message: format!("OpenAI bad request: {}", error_message),
                    }
                }
            }
            404 => EmbeddingError::ModelNotFound {
                model: format!(
                    "'{}' not found at {}. {}",
                    self.model, self.base_url, error_message
                ),
            },
            500..=599 => EmbeddingError::ServerError {
                message: format!("OpenAI server error (HTTP {}): {}", status, error_message),
                status: Some(status),
            },
            _ => EmbeddingError::ServerError {
                message: format!(
                    "OpenAI unexpected error (HTTP {}): {}",
                    status, error_message
                ),
                status: Some(status),
            },
        }
    }

    /// Sort and extract embedding vectors from the API response.
    fn extract_embeddings(
        &self,
        mut response: OpenAIEmbeddingResponse,
        expected_count: usize,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        if response.data.len() != expected_count {
            return Err(EmbeddingError::ParseError {
                message: format!(
                    "expected {} embeddings but OpenAI returned {}",
                    expected_count,
                    response.data.len()
                ),
            });
        }

        // Sort by index to preserve input order
        response.data.sort_by_key(|d| d.index);

        Ok(response.data.into_iter().map(|d| d.embedding).collect())
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAIEmbeddingProvider {
    async fn embed_documents(&self, documents: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        if documents.len() > MAX_BATCH_SIZE {
            return Err(EmbeddingError::BatchSizeLimitExceeded {
                requested: documents.len(),
                max_allowed: MAX_BATCH_SIZE,
            });
        }

        let input = serde_json::json!(documents);
        let body = self.build_request_body(input);
        let response = self.post_embeddings(&body).await?;

        self.extract_embeddings(response, documents.len())
    }

    async fn embed_query(&self, query: &str) -> EmbeddingResult<Vec<f32>> {
        let input = serde_json::json!(query);
        let body = self.build_request_body(input);
        let response = self.post_embeddings(&body).await?;

        response
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| EmbeddingError::ParseError {
                message: "OpenAI returned empty embeddings for query".to_string(),
            })
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        if self.api_key.is_empty() {
            return Err(EmbeddingError::AuthenticationFailed {
                message: "OpenAI API key is not configured. Set it via embedding config commands \
                          (keyring alias: 'openai_embedding')."
                    .to_string(),
            });
        }

        let body = self.build_request_body(serde_json::json!("health check"));
        self.post_embeddings(&body).await?;

        Ok(())
    }

    fn is_local(&self) -> bool {
        false
    }

    fn max_batch_size(&self) -> usize {
        MAX_BATCH_SIZE
    }

    fn provider_type(&self) -> EmbeddingProviderType {
        EmbeddingProviderType::OpenAI
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
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI);
        config.api_key = Some("sk-test-api-key".to_string());
        config
    }

    fn config_with_model(model: &str) -> EmbeddingProviderConfig {
        EmbeddingProviderConfig {
            model: model.to_string(),
            ..default_config()
        }
    }

    fn config_with_base_url(base_url: &str) -> EmbeddingProviderConfig {
        EmbeddingProviderConfig {
            base_url: Some(base_url.to_string()),
            ..default_config()
        }
    }

    fn config_with_dimension(dim: usize) -> EmbeddingProviderConfig {
        EmbeddingProviderConfig {
            dimension: Some(dim),
            ..default_config()
        }
    }

    fn config_without_api_key() -> EmbeddingProviderConfig {
        EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI)
    }

    // =====================================================================
    // Construction tests
    // =====================================================================

    #[test]
    fn new_with_default_config() {
        let config = default_config();
        let provider = OpenAIEmbeddingProvider::new(&config);

        assert_eq!(provider.model, "text-embedding-3-small");
        assert_eq!(provider.dimension(), 1536);
        assert_eq!(provider.display_name(), "OpenAI (text-embedding-3-small)");
        assert_eq!(provider.provider_type(), EmbeddingProviderType::OpenAI);
        assert!(!provider.is_local());
        assert_eq!(provider.max_batch_size(), 2048);
        assert_eq!(provider.base_url, OPENAI_EMBEDDING_API_URL);
        assert_eq!(provider.api_key, "sk-test-api-key");
    }

    #[test]
    fn new_with_custom_model() {
        let config = config_with_model("text-embedding-3-large");
        let provider = OpenAIEmbeddingProvider::new(&config);

        assert_eq!(provider.model, "text-embedding-3-large");
        assert_eq!(provider.display_name(), "OpenAI (text-embedding-3-large)");
    }

    #[test]
    fn new_with_custom_base_url() {
        let config = config_with_base_url("https://custom.api.example.com/v1/embeddings");
        let provider = OpenAIEmbeddingProvider::new(&config);

        assert_eq!(
            provider.base_url,
            "https://custom.api.example.com/v1/embeddings"
        );
    }

    #[test]
    fn new_with_custom_dimension() {
        let config = config_with_dimension(512);
        let provider = OpenAIEmbeddingProvider::new(&config);

        assert_eq!(provider.dimension(), 512);
    }

    #[test]
    fn new_with_empty_model_uses_default() {
        let config = EmbeddingProviderConfig {
            model: "  ".to_string(),
            ..default_config()
        };
        let provider = OpenAIEmbeddingProvider::new(&config);

        assert_eq!(provider.model, DEFAULT_MODEL);
    }

    #[test]
    fn new_without_api_key_stores_empty() {
        let config = config_without_api_key();
        let provider = OpenAIEmbeddingProvider::new(&config);

        assert!(provider.api_key.is_empty());
    }

    // =====================================================================
    // Trait property tests
    // =====================================================================

    #[test]
    fn provider_is_not_local() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        assert!(!provider.is_local());
    }

    #[test]
    fn provider_type_is_openai() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        assert_eq!(provider.provider_type(), EmbeddingProviderType::OpenAI);
    }

    #[test]
    fn max_batch_size_is_2048() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        assert_eq!(provider.max_batch_size(), 2048);
    }

    #[test]
    fn display_name_includes_model() {
        let config = config_with_model("text-embedding-3-small");
        let provider = OpenAIEmbeddingProvider::new(&config);
        assert!(provider.display_name().contains("text-embedding-3-small"));
        assert!(provider.display_name().contains("OpenAI"));
    }

    // =====================================================================
    // Request body construction tests
    // =====================================================================

    #[test]
    fn build_request_body_single_input() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        let body = provider.build_request_body(serde_json::json!("hello world"));

        assert_eq!(body["model"], "text-embedding-3-small");
        assert_eq!(body["input"], "hello world");
        // text-embedding-3-* should include dimensions
        assert_eq!(body["dimensions"], 1536);
    }

    #[test]
    fn build_request_body_batch_input() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        let body = provider.build_request_body(serde_json::json!(["hello", "world"]));

        assert_eq!(body["model"], "text-embedding-3-small");
        assert_eq!(body["input"], serde_json::json!(["hello", "world"]));
    }

    #[test]
    fn build_request_body_with_custom_dimension() {
        let config = config_with_dimension(512);
        let provider = OpenAIEmbeddingProvider::new(&config);
        let body = provider.build_request_body(serde_json::json!("test"));

        assert_eq!(body["dimensions"], 512);
    }

    // =====================================================================
    // Error mapping tests
    // =====================================================================

    #[test]
    fn map_http_error_401_auth_failed() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(
            401,
            r#"{"error":{"message":"Invalid API key","type":"invalid_request_error","code":"invalid_api_key"}}"#,
        );
        assert!(matches!(err, EmbeddingError::AuthenticationFailed { .. }));
        let msg = err.to_string();
        assert!(msg.contains("authentication failed"));
        assert!(msg.contains("openai_embedding"));
    }

    #[test]
    fn map_http_error_429_rate_limited() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(
            429,
            r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#,
        );
        assert!(matches!(err, EmbeddingError::RateLimited { .. }));
        assert!(err.is_retryable());
    }

    #[test]
    fn map_http_error_400_input_too_long() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(
            400,
            r#"{"error":{"message":"max token length exceeded","type":"invalid_request_error"}}"#,
        );
        assert!(matches!(err, EmbeddingError::InputTooLong { .. }));
    }

    #[test]
    fn map_http_error_404_model_not_found() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(
            404,
            r#"{"error":{"message":"Model not found","type":"invalid_request_error"}}"#,
        );
        assert!(matches!(err, EmbeddingError::ModelNotFound { .. }));
    }

    #[test]
    fn map_http_error_500_server_error() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(500, r#"{"error":{"message":"Internal error"}}"#);
        assert!(matches!(
            err,
            EmbeddingError::ServerError {
                status: Some(500),
                ..
            }
        ));
        assert!(err.is_retryable());
    }

    #[test]
    fn map_http_error_unparseable_body() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(503, "service unavailable");
        assert!(matches!(
            err,
            EmbeddingError::ServerError {
                status: Some(503),
                ..
            }
        ));
    }

    // =====================================================================
    // Response extraction tests
    // =====================================================================

    #[test]
    fn extract_embeddings_correct_order() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());

        let response = OpenAIEmbeddingResponse {
            data: vec![
                OpenAIEmbeddingData {
                    embedding: vec![2.0, 2.0],
                    index: 1,
                    object: Some("embedding".to_string()),
                },
                OpenAIEmbeddingData {
                    embedding: vec![1.0, 1.0],
                    index: 0,
                    object: Some("embedding".to_string()),
                },
            ],
            model: Some("text-embedding-3-small".to_string()),
            usage: None,
        };

        let result = provider.extract_embeddings(response, 2).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], vec![1.0, 1.0]);
        assert_eq!(result[1], vec![2.0, 2.0]);
    }

    #[test]
    fn extract_embeddings_count_mismatch() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());

        let response = OpenAIEmbeddingResponse {
            data: vec![OpenAIEmbeddingData {
                embedding: vec![1.0],
                index: 0,
                object: None,
            }],
            model: None,
            usage: None,
        };

        let result = provider.extract_embeddings(response, 3);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::ParseError { .. }
        ));
    }

    // =====================================================================
    // Async operation tests (no real HTTP calls)
    // =====================================================================

    #[tokio::test]
    async fn embed_documents_empty_returns_empty() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        let result = provider.embed_documents(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn embed_documents_rejects_oversized_batch() {
        let provider = OpenAIEmbeddingProvider::new(&default_config());
        let docs: Vec<&str> = (0..2049).map(|_| "test").collect();
        let result = provider.embed_documents(&docs).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::BatchSizeLimitExceeded {
                requested: 2049,
                max_allowed: 2048,
            }
        ));
    }

    #[tokio::test]
    async fn embed_query_without_api_key_fails() {
        let config = config_without_api_key();
        let provider = OpenAIEmbeddingProvider::new(&config);
        let result = provider.embed_query("hello").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::AuthenticationFailed { .. }
        ));
    }

    #[tokio::test]
    async fn health_check_without_api_key_fails() {
        let config = config_without_api_key();
        let provider = OpenAIEmbeddingProvider::new(&config);
        let result = provider.health_check().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::AuthenticationFailed { .. }
        ));
    }

    // =====================================================================
    // Trait object safety tests
    // =====================================================================

    #[test]
    fn provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OpenAIEmbeddingProvider>();
    }

    #[test]
    fn provider_is_object_safe() {
        fn _assert_object_safe(_: &dyn EmbeddingProvider) {}
    }

    #[test]
    fn provider_as_any_downcast() {
        let config = default_config();
        let provider = OpenAIEmbeddingProvider::new(&config);
        let any_ref = provider.as_any();
        assert!(any_ref.downcast_ref::<OpenAIEmbeddingProvider>().is_some());
    }
}
