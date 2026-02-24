//! GLM (ZhipuAI) Embedding Provider
//!
//! Implements the `EmbeddingProvider` trait for ZhipuAI's embedding-3 model
//! using reqwest HTTP transport. Supports configurable output dimensions
//! (256, 512, 1024, 2048) and batch embedding up to 64 inputs per request.
//!
//! ## API Details
//!
//! - Endpoint: `POST https://open.bigmodel.cn/api/paas/v4/embeddings`
//! - Auth: `Authorization: Bearer {api_key}`
//! - Body: `{ model, input, dimensions? }`
//! - Response: `{ data: [{ embedding, index }], model, usage }`
//!
//! ## Design Decision (ADR-F006)
//!
//! API keys are stored in the OS keyring under the `glm_embedding` alias
//! and injected via `EmbeddingProviderConfig.api_key` at construction time.

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

/// Default ZhipuAI embedding API endpoint.
const GLM_EMBEDDING_API_URL: &str = "https://open.bigmodel.cn/api/paas/v4/embeddings";

/// Default embedding model.
const DEFAULT_MODEL: &str = "embedding-3";

/// Default embedding dimension (highest precision).
const DEFAULT_DIMENSION: usize = 2048;

/// Maximum batch size supported by ZhipuAI embedding API.
const MAX_BATCH_SIZE: usize = 64;

// ---------------------------------------------------------------------------
// API response types
// ---------------------------------------------------------------------------

/// ZhipuAI embedding API response.
#[derive(Debug, Deserialize)]
struct GlmEmbeddingResponse {
    data: Vec<GlmEmbeddingData>,
    #[allow(dead_code)]
    model: Option<String>,
    #[allow(dead_code)]
    usage: Option<GlmUsage>,
}

/// Individual embedding result within the response.
#[derive(Debug, Deserialize)]
struct GlmEmbeddingData {
    embedding: Vec<f32>,
    index: usize,
    #[allow(dead_code)]
    object: Option<String>,
}

/// Token usage statistics from the API.
#[derive(Debug, Deserialize)]
struct GlmUsage {
    #[allow(dead_code)]
    prompt_tokens: Option<u32>,
    #[allow(dead_code)]
    completion_tokens: Option<u32>,
    #[allow(dead_code)]
    total_tokens: Option<u32>,
}

/// ZhipuAI API error response.
#[derive(Debug, Deserialize)]
struct GlmErrorResponse {
    error: Option<GlmErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct GlmErrorDetail {
    code: Option<String>,
    message: Option<String>,
}

// ---------------------------------------------------------------------------
// Provider implementation
// ---------------------------------------------------------------------------

/// GLM embedding provider using the ZhipuAI embedding-3 API.
///
/// Uses reqwest for HTTP transport, consistent with the GLM LLM provider
/// pattern. Supports configurable dimensions (256, 512, 1024, 2048) and
/// batch embedding up to 64 inputs per request.
///
/// # Thread Safety
///
/// This struct is `Send + Sync` — the reqwest `Client` is internally
/// arc'd and clone-safe, and all fields are immutable after construction.
pub struct GlmEmbeddingProvider {
    /// The reqwest HTTP client.
    client: reqwest::Client,
    /// API key for authentication.
    api_key: String,
    /// Model name (e.g., "embedding-3").
    model: String,
    /// API base URL.
    base_url: String,
    /// Embedding dimension.
    dimension: usize,
    /// Human-readable display name.
    display_name: String,
}

impl GlmEmbeddingProvider {
    /// Create a new GLM embedding provider from an `EmbeddingProviderConfig`.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration specifying api_key, model, base_url, and dimension.
    ///
    /// # Returns
    ///
    /// A configured `GlmEmbeddingProvider` ready to make embedding requests.
    ///
    /// # Panics
    ///
    /// Does not panic. If `api_key` is `None`, requests will fail at call time
    /// with `AuthenticationFailed`. Prefer using `EmbeddingProviderConfig::validate()`
    /// before construction.
    pub fn new(config: &EmbeddingProviderConfig) -> Self {
        let model = if config.model.trim().is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            config.model.clone()
        };

        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(GLM_EMBEDDING_API_URL)
            .to_string();

        let dimension = config.dimension.unwrap_or(DEFAULT_DIMENSION);
        let display_name = format!("GLM ({})", model);
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

        // Only include dimensions if not the default (2048), to avoid
        // sending unnecessary parameters to the API.
        if self.dimension != DEFAULT_DIMENSION {
            body["dimensions"] = serde_json::json!(self.dimension);
        }

        body
    }

    /// Send a POST request to the embedding API and parse the response.
    async fn post_embeddings(
        &self,
        body: &serde_json::Value,
    ) -> EmbeddingResult<GlmEmbeddingResponse> {
        if self.api_key.is_empty() {
            return Err(EmbeddingError::AuthenticationFailed {
                message: "GLM API key is not configured. Set it via embedding config commands \
                          (keyring alias: 'glm_embedding')."
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

            serde_json::from_str::<GlmEmbeddingResponse>(&resp_text).map_err(|e| {
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
                    "Cannot connect to ZhipuAI API at {}. Check your network connectivity.",
                    self.base_url
                ),
            }
        } else if err.is_timeout() {
            EmbeddingError::NetworkError {
                message: format!("Request to ZhipuAI API timed out: {}", msg),
            }
        } else {
            EmbeddingError::NetworkError { message: msg }
        }
    }

    /// Map an HTTP error response to `EmbeddingError`.
    fn map_http_error(&self, status: u16, body_text: &str) -> EmbeddingError {
        // Try to parse structured error response
        let error_detail = serde_json::from_str::<GlmErrorResponse>(body_text)
            .ok()
            .and_then(|r| r.error);

        let error_message = error_detail
            .as_ref()
            .and_then(|d| d.message.as_deref())
            .unwrap_or(body_text);
        let error_code = error_detail.as_ref().and_then(|d| d.code.as_deref());

        match status {
            401 => EmbeddingError::AuthenticationFailed {
                message: format!(
                    "ZhipuAI authentication failed: {}. Verify your API key \
                     (keyring alias: 'glm_embedding').",
                    error_message
                ),
            },
            429 => {
                // Parse retry-after header if available
                let retry_after = error_code.and_then(|c| c.parse::<u32>().ok());
                EmbeddingError::RateLimited {
                    message: format!("ZhipuAI rate limit exceeded: {}", error_message),
                    retry_after,
                }
            }
            400 => {
                // Check for specific error codes
                if let Some(code) = error_code {
                    if code == "1210" {
                        return EmbeddingError::InvalidConfig {
                            message: format!("ZhipuAI invalid parameter: {}", error_message),
                        };
                    }
                }
                // Input too long or other 400 errors
                if error_message.contains("token") || error_message.contains("length") {
                    EmbeddingError::InputTooLong {
                        message: format!("ZhipuAI: {}", error_message),
                    }
                } else {
                    EmbeddingError::ServerError {
                        message: format!("ZhipuAI bad request: {}", error_message),
                        status: Some(status),
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
                message: format!("ZhipuAI server error (HTTP {}): {}", status, error_message),
                status: Some(status),
            },
            _ => EmbeddingError::ServerError {
                message: format!(
                    "ZhipuAI unexpected error (HTTP {}): {}",
                    status, error_message
                ),
                status: Some(status),
            },
        }
    }

    /// Sort and extract embedding vectors from the API response.
    ///
    /// The API returns data items with an `index` field. We sort by index
    /// to ensure the output order matches the input order.
    fn extract_embeddings(
        &self,
        mut response: GlmEmbeddingResponse,
        expected_count: usize,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        if response.data.len() != expected_count {
            return Err(EmbeddingError::ParseError {
                message: format!(
                    "expected {} embeddings but ZhipuAI returned {}",
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
impl EmbeddingProvider for GlmEmbeddingProvider {
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

        // ZhipuAI accepts string arrays for batch embedding
        let input = serde_json::json!(documents);
        let body = self.build_request_body(input);
        let response = self.post_embeddings(&body).await?;

        self.extract_embeddings(response, documents.len())
    }

    async fn embed_query(&self, query: &str) -> EmbeddingResult<Vec<f32>> {
        // Single string input for query embedding
        let input = serde_json::json!(query);
        let body = self.build_request_body(input);
        let response = self.post_embeddings(&body).await?;

        response
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| EmbeddingError::ParseError {
                message: "ZhipuAI returned empty embeddings for query".to_string(),
            })
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        // Step 1: Validate API key is present
        if self.api_key.is_empty() {
            return Err(EmbeddingError::AuthenticationFailed {
                message: "GLM API key is not configured. Set it via embedding config commands \
                          (keyring alias: 'glm_embedding')."
                    .to_string(),
            });
        }

        // Step 2: Send a minimal embedding request to validate credentials
        // and endpoint reachability.
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
        EmbeddingProviderType::Glm
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

    // =====================================================================
    // Helper functions
    // =====================================================================

    fn default_config() -> EmbeddingProviderConfig {
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::Glm);
        config.api_key = Some("test-api-key".to_string());
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
        EmbeddingProviderConfig::new(EmbeddingProviderType::Glm)
    }

    // =====================================================================
    // Construction tests
    // =====================================================================

    #[test]
    fn new_with_default_config() {
        let config = default_config();
        let provider = GlmEmbeddingProvider::new(&config);

        assert_eq!(provider.model, "embedding-3");
        assert_eq!(provider.dimension(), 2048);
        assert_eq!(provider.display_name(), "GLM (embedding-3)");
        assert_eq!(provider.provider_type(), EmbeddingProviderType::Glm);
        assert!(!provider.is_local());
        assert_eq!(provider.max_batch_size(), 64);
        assert_eq!(provider.base_url, GLM_EMBEDDING_API_URL);
        assert_eq!(provider.api_key, "test-api-key");
    }

    #[test]
    fn new_with_custom_model() {
        let config = config_with_model("embedding-2");
        let provider = GlmEmbeddingProvider::new(&config);

        assert_eq!(provider.model, "embedding-2");
        assert_eq!(provider.display_name(), "GLM (embedding-2)");
    }

    #[test]
    fn new_with_custom_base_url() {
        let config = config_with_base_url("https://custom.api.example.com/v4/embeddings");
        let provider = GlmEmbeddingProvider::new(&config);

        assert_eq!(
            provider.base_url,
            "https://custom.api.example.com/v4/embeddings"
        );
    }

    #[test]
    fn new_with_custom_dimension() {
        let config = config_with_dimension(512);
        let provider = GlmEmbeddingProvider::new(&config);

        assert_eq!(provider.dimension(), 512);
    }

    #[test]
    fn new_with_empty_model_uses_default() {
        let config = EmbeddingProviderConfig {
            model: "  ".to_string(),
            ..default_config()
        };
        let provider = GlmEmbeddingProvider::new(&config);

        assert_eq!(provider.model, DEFAULT_MODEL);
    }

    #[test]
    fn new_without_api_key_stores_empty() {
        let config = config_without_api_key();
        let provider = GlmEmbeddingProvider::new(&config);

        assert!(provider.api_key.is_empty());
    }

    // =====================================================================
    // Trait property tests
    // =====================================================================

    #[test]
    fn provider_is_not_local() {
        let provider = GlmEmbeddingProvider::new(&default_config());
        assert!(!provider.is_local());
    }

    #[test]
    fn provider_type_is_glm() {
        let provider = GlmEmbeddingProvider::new(&default_config());
        assert_eq!(provider.provider_type(), EmbeddingProviderType::Glm);
    }

    #[test]
    fn max_batch_size_is_64() {
        let provider = GlmEmbeddingProvider::new(&default_config());
        assert_eq!(provider.max_batch_size(), 64);
    }

    #[test]
    fn display_name_includes_model() {
        let config = config_with_model("embedding-3");
        let provider = GlmEmbeddingProvider::new(&config);
        assert!(provider.display_name().contains("embedding-3"));
        assert!(provider.display_name().contains("GLM"));
    }

    // =====================================================================
    // Request body construction tests
    // =====================================================================

    #[test]
    fn build_request_body_single_input() {
        let provider = GlmEmbeddingProvider::new(&default_config());
        let body = provider.build_request_body(serde_json::json!("hello world"));

        assert_eq!(body["model"], "embedding-3");
        assert_eq!(body["input"], "hello world");
        // Default dimension (2048) should not include dimensions field
        assert!(body.get("dimensions").is_none());
    }

    #[test]
    fn build_request_body_batch_input() {
        let provider = GlmEmbeddingProvider::new(&default_config());
        let body = provider.build_request_body(serde_json::json!(["hello", "world"]));

        assert_eq!(body["model"], "embedding-3");
        assert_eq!(body["input"], serde_json::json!(["hello", "world"]));
    }

    #[test]
    fn build_request_body_with_custom_dimension() {
        let config = config_with_dimension(1024);
        let provider = GlmEmbeddingProvider::new(&config);
        let body = provider.build_request_body(serde_json::json!("test"));

        assert_eq!(body["dimensions"], 1024);
    }

    #[test]
    fn build_request_body_omits_default_dimension() {
        // When dimension is 2048 (default), the dimensions field should be omitted
        let config = config_with_dimension(2048);
        let provider = GlmEmbeddingProvider::new(&config);
        let body = provider.build_request_body(serde_json::json!("test"));

        assert!(body.get("dimensions").is_none());
    }

    // =====================================================================
    // Error mapping tests
    // =====================================================================

    #[test]
    fn map_http_error_401_auth_failed() {
        let provider = GlmEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(
            401,
            r#"{"error":{"code":"1001","message":"Invalid API key"}}"#,
        );
        assert!(matches!(err, EmbeddingError::AuthenticationFailed { .. }));
        let msg = err.to_string();
        assert!(msg.contains("authentication failed"));
        assert!(msg.contains("glm_embedding"));
    }

    #[test]
    fn map_http_error_429_rate_limited() {
        let provider = GlmEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(
            429,
            r#"{"error":{"code":"1302","message":"Too many requests"}}"#,
        );
        assert!(matches!(err, EmbeddingError::RateLimited { .. }));
        assert!(err.is_retryable());
    }

    #[test]
    fn map_http_error_400_invalid_param() {
        let provider = GlmEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(
            400,
            r#"{"error":{"code":"1210","message":"Invalid parameter"}}"#,
        );
        assert!(matches!(err, EmbeddingError::InvalidConfig { .. }));
    }

    #[test]
    fn map_http_error_400_input_too_long() {
        let provider = GlmEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(
            400,
            r#"{"error":{"code":"1214","message":"token limit exceeded"}}"#,
        );
        assert!(matches!(err, EmbeddingError::InputTooLong { .. }));
    }

    #[test]
    fn map_http_error_404_model_not_found() {
        let provider = GlmEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(
            404,
            r#"{"error":{"code":"1404","message":"Model not found"}}"#,
        );
        assert!(matches!(err, EmbeddingError::ModelNotFound { .. }));
    }

    #[test]
    fn map_http_error_500_server_error() {
        let provider = GlmEmbeddingProvider::new(&default_config());
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
        let provider = GlmEmbeddingProvider::new(&default_config());
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
        let provider = GlmEmbeddingProvider::new(&default_config());

        // Simulate response where items are returned out of order
        let response = GlmEmbeddingResponse {
            data: vec![
                GlmEmbeddingData {
                    embedding: vec![2.0, 2.0],
                    index: 1,
                    object: Some("embedding".to_string()),
                },
                GlmEmbeddingData {
                    embedding: vec![1.0, 1.0],
                    index: 0,
                    object: Some("embedding".to_string()),
                },
            ],
            model: Some("embedding-3".to_string()),
            usage: None,
        };

        let result = provider.extract_embeddings(response, 2).unwrap();
        assert_eq!(result.len(), 2);
        // Index 0 should come first
        assert_eq!(result[0], vec![1.0, 1.0]);
        assert_eq!(result[1], vec![2.0, 2.0]);
    }

    #[test]
    fn extract_embeddings_count_mismatch() {
        let provider = GlmEmbeddingProvider::new(&default_config());

        let response = GlmEmbeddingResponse {
            data: vec![GlmEmbeddingData {
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
        let provider = GlmEmbeddingProvider::new(&default_config());
        let result = provider.embed_documents(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn embed_documents_rejects_oversized_batch() {
        let provider = GlmEmbeddingProvider::new(&default_config());
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

    #[tokio::test]
    async fn embed_query_without_api_key_fails() {
        let config = config_without_api_key();
        let provider = GlmEmbeddingProvider::new(&config);
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
        let provider = GlmEmbeddingProvider::new(&config);
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
        assert_send_sync::<GlmEmbeddingProvider>();
    }

    #[test]
    fn provider_is_object_safe() {
        fn _assert_object_safe(_: &dyn EmbeddingProvider) {}
    }

    #[test]
    fn provider_as_any_downcast() {
        let config = default_config();
        let provider = GlmEmbeddingProvider::new(&config);
        let any_ref = provider.as_any();
        assert!(any_ref.downcast_ref::<GlmEmbeddingProvider>().is_some());
    }

    // =====================================================================
    // Integration tests (require valid ZhipuAI API key — marked #[ignore])
    // =====================================================================

    #[tokio::test]
    #[ignore = "requires a valid ZhipuAI API key in GLM_EMBEDDING_API_KEY env var"]
    async fn integration_health_check() {
        let api_key =
            std::env::var("GLM_EMBEDDING_API_KEY").expect("GLM_EMBEDDING_API_KEY must be set");
        let mut config = default_config();
        config.api_key = Some(api_key);
        let provider = GlmEmbeddingProvider::new(&config);
        let result = provider.health_check().await;
        assert!(result.is_ok(), "health_check failed: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore = "requires a valid ZhipuAI API key in GLM_EMBEDDING_API_KEY env var"]
    async fn integration_embed_query() {
        let api_key =
            std::env::var("GLM_EMBEDDING_API_KEY").expect("GLM_EMBEDDING_API_KEY must be set");
        let mut config = default_config();
        config.api_key = Some(api_key);
        let provider = GlmEmbeddingProvider::new(&config);

        let result = provider.embed_query("hello world").await;
        assert!(result.is_ok(), "embed_query failed: {:?}", result.err());

        let embedding = result.unwrap();
        assert_eq!(embedding.len(), 2048); // default dimension
                                           // Verify non-zero values
        assert!(embedding.iter().any(|&v| v != 0.0));
    }

    #[tokio::test]
    #[ignore = "requires a valid ZhipuAI API key in GLM_EMBEDDING_API_KEY env var"]
    async fn integration_embed_documents() {
        let api_key =
            std::env::var("GLM_EMBEDDING_API_KEY").expect("GLM_EMBEDDING_API_KEY must be set");
        let mut config = default_config();
        config.api_key = Some(api_key);
        let provider = GlmEmbeddingProvider::new(&config);

        let docs = vec!["hello world", "foo bar baz", "rust programming language"];
        let result = provider.embed_documents(&docs).await;
        assert!(result.is_ok(), "embed_documents failed: {:?}", result.err());

        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 3);
        // All embeddings should have the configured dimension
        for emb in &embeddings {
            assert_eq!(emb.len(), 2048);
        }
    }

    #[tokio::test]
    #[ignore = "requires a valid ZhipuAI API key in GLM_EMBEDDING_API_KEY env var"]
    async fn integration_embed_with_custom_dimension() {
        let api_key =
            std::env::var("GLM_EMBEDDING_API_KEY").expect("GLM_EMBEDDING_API_KEY must be set");
        let mut config = config_with_dimension(512);
        config.api_key = Some(api_key);
        let provider = GlmEmbeddingProvider::new(&config);

        let result = provider.embed_query("test dimension").await;
        assert!(result.is_ok(), "embed_query failed: {:?}", result.err());

        let embedding = result.unwrap();
        assert_eq!(embedding.len(), 512);
    }
}
