//! Qwen DashScope Embedding Provider
//!
//! Implements the `EmbeddingProvider` trait for Qwen embedding models using the
//! DashScope API. Supports explicit `text_type` semantics for query vs document
//! embedding generation, enabling asymmetric retrieval use cases.
//!
//! ## Default Model
//!
//! Uses `text-embedding-v3` (1024-dimensional) by default. The dimension is
//! fixed at construction time from the config and sent explicitly in every API
//! request, which is required for Matryoshka models (v3, v4) to return the
//! correct truncated vector size.
//!
//! ## DashScope API
//!
//! POST `https://dashscope.aliyuncs.com/api/v1/services/embeddings/text-embedding/text-embedding`
//!
//! - Header: `Authorization: Bearer {api_key}`
//! - Body: `{ model, input: { texts: [...] }, parameters: { text_type, dimension? } }`
//! - Response: `{ output: { embeddings: [{ text_index, embedding }] } }`

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::any::Any;
use tracing;

use super::embedding_provider::{
    EmbeddingError, EmbeddingProvider, EmbeddingProviderConfig, EmbeddingProviderType,
    EmbeddingResult,
};
use crate::services::proxy::build_http_client;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default DashScope embedding API endpoint.
const DASHSCOPE_DEFAULT_URL: &str =
    "https://dashscope.aliyuncs.com/api/v1/services/embeddings/text-embedding/text-embedding";

/// Default embedding model.
const DEFAULT_MODEL: &str = "text-embedding-v3";

/// Default dimension for text-embedding-v4.
const DEFAULT_DIMENSION: usize = 1024;

/// Maximum batch size for DashScope embedding requests.
const MAX_BATCH_SIZE: usize = 25;

// ---------------------------------------------------------------------------
// DashScope API request/response types
// ---------------------------------------------------------------------------

/// DashScope embedding API request body.
#[derive(Debug, Serialize)]
struct DashScopeRequest {
    model: String,
    input: DashScopeInput,
    parameters: DashScopeParameters,
}

/// Input section of the DashScope request.
#[derive(Debug, Serialize)]
struct DashScopeInput {
    texts: Vec<String>,
}

/// Parameters section of the DashScope request.
#[derive(Debug, Serialize)]
struct DashScopeParameters {
    text_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimension: Option<usize>,
}

/// DashScope embedding API response body.
#[derive(Debug, Deserialize)]
struct DashScopeResponse {
    output: Option<DashScopeOutput>,
    #[serde(default)]
    request_id: Option<String>,
    // Error fields (present on failure)
    code: Option<String>,
    message: Option<String>,
}

/// Output section of the DashScope response.
#[derive(Debug, Deserialize)]
struct DashScopeOutput {
    embeddings: Vec<DashScopeEmbedding>,
}

/// A single embedding result from DashScope.
#[derive(Debug, Deserialize)]
struct DashScopeEmbedding {
    text_index: usize,
    embedding: Vec<f32>,
}

// ---------------------------------------------------------------------------
// QwenEmbeddingProvider
// ---------------------------------------------------------------------------

/// Qwen embedding provider using the DashScope API.
///
/// This provider communicates with Alibaba Cloud's DashScope API for text
/// embeddings. It uses the `text_type` parameter to distinguish between
/// query and document embeddings, enabling asymmetric retrieval.
///
/// ## Error Handling
///
/// The provider maps DashScope-specific error codes to actionable
/// `EmbeddingError` variants:
/// - `InvalidApiKey` / `Unauthorized` -> `AuthenticationFailed`
/// - `Throttling` / 429 -> `RateLimited`
/// - `QuotaExhausted` -> `RateLimited` (with actionable message)
/// - `ModelNotFound` -> `ModelNotFound`
/// - 5xx -> `ServerError`
/// - Connection failures -> `NetworkError`
pub struct QwenEmbeddingProvider {
    /// HTTP client for API requests.
    client: Client,
    /// DashScope API key.
    api_key: String,
    /// Model name to use for embedding requests.
    model: String,
    /// Base URL for the DashScope embedding API.
    base_url: String,
    /// Embedding dimension. Fixed at construction time from the config.
    dimension: usize,
    /// Human-readable display name for this provider instance.
    display_name: String,
}

impl QwenEmbeddingProvider {
    /// Create a new Qwen embedding provider from an `EmbeddingProviderConfig`.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration specifying model, api_key, base_url, and dimension.
    ///
    /// # Returns
    ///
    /// A configured `QwenEmbeddingProvider` ready to make embedding requests.
    ///
    /// # Panics
    ///
    /// Does not panic. If the API key is missing in the config, requests will
    /// fail at runtime with `AuthenticationFailed`.
    pub fn new(config: &EmbeddingProviderConfig) -> Self {
        let model = if config.model.trim().is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            config.model.clone()
        };

        let api_key = config.api_key.clone().unwrap_or_default();

        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(DASHSCOPE_DEFAULT_URL)
            .to_string();

        let initial_dimension = config.dimension.unwrap_or(DEFAULT_DIMENSION);
        let display_name = format!("Qwen ({})", model);

        let client = build_http_client(config.proxy.as_ref());

        Self {
            client,
            api_key,
            model,
            base_url,
            dimension: initial_dimension,
            display_name,
        }
    }

    /// Call the DashScope embedding API with the given texts and text_type.
    ///
    /// # Arguments
    ///
    /// * `texts` - Slice of texts to embed.
    /// * `text_type` - Either "query" or "document".
    ///
    /// # Returns
    ///
    /// Embedding vectors ordered by the original text index.
    async fn call_dashscope(
        &self,
        texts: &[&str],
        text_type: &str,
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        if self.api_key.is_empty() {
            return Err(EmbeddingError::AuthenticationFailed {
                message: "DashScope API key is not configured. \
                          Set it via the embedding config command with the 'qwen_embedding' alias."
                    .to_string(),
            });
        }

        let request_body = DashScopeRequest {
            model: self.model.clone(),
            input: DashScopeInput {
                texts: texts.iter().map(|s| s.to_string()).collect(),
            },
            parameters: DashScopeParameters {
                text_type: text_type.to_string(),
                dimension: Some(self.dimension),
            },
        };

        let http_response = self
            .client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        let status = http_response.status();
        let status_code = status.as_u16();

        // Read the response body
        let response_text = http_response.text().await.map_err(|e| {
            EmbeddingError::NetworkError {
                message: format!("failed to read DashScope response body: {}", e),
            }
        })?;

        // Check for HTTP-level errors first
        if !status.is_success() {
            return Err(self.map_http_error(status_code, &response_text));
        }

        // Parse the successful response
        let response: DashScopeResponse =
            serde_json::from_str(&response_text).map_err(|e| EmbeddingError::ParseError {
                message: format!(
                    "failed to parse DashScope response: {}. Response body: {}",
                    e,
                    &response_text[..response_text.len().min(500)]
                ),
            })?;

        // Check for API-level errors (some may come with 200 status)
        if let Some(ref code) = response.code {
            return Err(self.map_api_error(code, response.message.as_deref()));
        }

        // Extract embeddings from the output
        let output = response.output.ok_or_else(|| EmbeddingError::ParseError {
            message: "DashScope response missing 'output' field".to_string(),
        })?;

        if output.embeddings.len() != texts.len() {
            return Err(EmbeddingError::ParseError {
                message: format!(
                    "expected {} embeddings but DashScope returned {}",
                    texts.len(),
                    output.embeddings.len()
                ),
            });
        }

        // Sort embeddings by text_index to ensure correct ordering
        let mut indexed_embeddings: Vec<(usize, Vec<f32>)> = output
            .embeddings
            .into_iter()
            .map(|e| (e.text_index, e.embedding))
            .collect();
        indexed_embeddings.sort_by_key(|(idx, _)| *idx);

        let embeddings: Vec<Vec<f32>> = indexed_embeddings.into_iter().map(|(_, e)| e).collect();

        Ok(embeddings)
    }

    /// Map a reqwest transport error to our `EmbeddingError` type.
    fn map_reqwest_error(&self, err: reqwest::Error) -> EmbeddingError {
        if err.is_connect() {
            EmbeddingError::ProviderUnavailable {
                message: format!(
                    "Cannot connect to DashScope API at {}. \
                     Check your network connection and try again.",
                    self.base_url
                ),
            }
        } else if err.is_timeout() {
            EmbeddingError::NetworkError {
                message: format!("DashScope API request timed out: {}", err),
            }
        } else {
            EmbeddingError::NetworkError {
                message: format!("DashScope API request failed: {}", err),
            }
        }
    }

    /// Map an HTTP status error to our `EmbeddingError` type.
    fn map_http_error(&self, status_code: u16, body: &str) -> EmbeddingError {
        // Try to parse the body as a DashScope error response
        if let Ok(response) = serde_json::from_str::<DashScopeResponse>(body) {
            if let Some(ref code) = response.code {
                return self.map_api_error(code, response.message.as_deref());
            }
        }

        // Fallback to HTTP status code mapping
        match status_code {
            401 => EmbeddingError::AuthenticationFailed {
                message: "DashScope API key is invalid or expired. \
                          Update it via the embedding config command with the 'qwen_embedding' alias."
                    .to_string(),
            },
            403 => EmbeddingError::AuthenticationFailed {
                message: "Access denied to DashScope API. \
                          Verify your API key permissions and account status."
                    .to_string(),
            },
            429 => {
                // Try to extract retry-after from body
                EmbeddingError::RateLimited {
                    message: "DashScope API rate limit exceeded. \
                              Reduce request frequency or upgrade your plan."
                        .to_string(),
                    retry_after: None,
                }
            }
            400 => EmbeddingError::InvalidConfig {
                message: format!(
                    "DashScope API rejected the request (400 Bad Request): {}",
                    &body[..body.len().min(300)]
                ),
            },
            404 => EmbeddingError::ModelNotFound {
                model: self.model.clone(),
            },
            status if status >= 500 => EmbeddingError::ServerError {
                message: format!(
                    "DashScope API server error: {}",
                    &body[..body.len().min(300)]
                ),
                status: Some(status),
            },
            _ => EmbeddingError::ServerError {
                message: format!(
                    "DashScope API returned HTTP {}: {}",
                    status_code,
                    &body[..body.len().min(300)]
                ),
                status: Some(status_code),
            },
        }
    }

    /// Map a DashScope API error code to our `EmbeddingError` type.
    ///
    /// DashScope error codes documented at:
    /// https://help.aliyun.com/document_detail/2712195.html
    fn map_api_error(&self, code: &str, message: Option<&str>) -> EmbeddingError {
        let msg = message.unwrap_or("unknown error");

        match code {
            "InvalidApiKey" | "Unauthorized" | "InvalidParameter.ApiKey" => {
                EmbeddingError::AuthenticationFailed {
                    message: format!(
                        "DashScope authentication failed ({}): {}. \
                         Update your API key via the embedding config command \
                         with the 'qwen_embedding' alias.",
                        code, msg
                    ),
                }
            }
            "Throttling" | "Throttling.RateQuota" | "Throttling.AllocationQuota" => {
                EmbeddingError::RateLimited {
                    message: format!(
                        "DashScope rate limit exceeded ({}): {}. \
                         Wait a moment before retrying or upgrade your plan.",
                        code, msg
                    ),
                    retry_after: Some(5), // Suggest 5 second wait
                }
            }
            "QuotaExhausted" | "Throttling.QuotaExhausted" => EmbeddingError::RateLimited {
                message: format!(
                    "DashScope quota exhausted ({}): {}. \
                     Your account has reached its usage limit. \
                     Top up your balance or upgrade your plan at https://dashscope.console.aliyun.com/.",
                    code, msg
                ),
                retry_after: None,
            },
            "ModelNotFound" | "InvalidParameter.Model" => EmbeddingError::ModelNotFound {
                model: format!(
                    "'{}' — DashScope error ({}): {}",
                    self.model, code, msg
                ),
            },
            "InvalidParameter" | "InvalidParameter.TextType" => EmbeddingError::InvalidConfig {
                message: format!("DashScope parameter error ({}): {}", code, msg),
            },
            "DataInspectionFailed" => EmbeddingError::Other {
                message: format!(
                    "DashScope content inspection failed ({}): {}. \
                     The input text may contain restricted content.",
                    code, msg
                ),
            },
            _ if code.starts_with("InternalError") || code.starts_with("SystemError") => {
                EmbeddingError::ServerError {
                    message: format!("DashScope server error ({}): {}", code, msg),
                    status: None,
                }
            }
            _ => EmbeddingError::Other {
                message: format!("DashScope error ({}): {}", code, msg),
            },
        }
    }

}

#[async_trait]
impl EmbeddingProvider for QwenEmbeddingProvider {
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

        tracing::debug!(
            model = %self.model,
            count = documents.len(),
            "embedding documents via DashScope"
        );

        self.call_dashscope(documents, "document").await
    }

    async fn embed_query(&self, query: &str) -> EmbeddingResult<Vec<f32>> {
        tracing::debug!(
            model = %self.model,
            "embedding query via DashScope"
        );

        let results = self.call_dashscope(&[query], "query").await?;
        results.into_iter().next().ok_or_else(|| EmbeddingError::ParseError {
            message: "DashScope returned empty embeddings for query".to_string(),
        })
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        if self.api_key.is_empty() {
            return Err(EmbeddingError::AuthenticationFailed {
                message: "DashScope API key is not configured. \
                          Set it via the embedding config command with the 'qwen_embedding' alias."
                    .to_string(),
            });
        }

        // Perform a minimal embedding request to verify connectivity and auth.
        tracing::debug!("performing DashScope health check");
        self.call_dashscope(&["health check"], "query").await?;

        Ok(())
    }

    fn is_local(&self) -> bool {
        false
    }

    fn max_batch_size(&self) -> usize {
        MAX_BATCH_SIZE
    }

    fn provider_type(&self) -> EmbeddingProviderType {
        EmbeddingProviderType::Qwen
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
        let mut config = EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen);
        config.api_key = Some("test-api-key".to_string());
        config
    }

    fn config_with_model(model: &str) -> EmbeddingProviderConfig {
        EmbeddingProviderConfig {
            model: model.to_string(),
            api_key: Some("test-api-key".to_string()),
            ..EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen)
        }
    }

    fn config_with_base_url(base_url: &str) -> EmbeddingProviderConfig {
        EmbeddingProviderConfig {
            base_url: Some(base_url.to_string()),
            api_key: Some("test-api-key".to_string()),
            ..EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen)
        }
    }

    // =========================================================================
    // Construction tests
    // =========================================================================

    #[test]
    fn new_with_default_config() {
        let config = default_config();
        let provider = QwenEmbeddingProvider::new(&config);

        assert_eq!(provider.model, "text-embedding-v3");
        assert_eq!(provider.dimension(), 1024);
        assert_eq!(provider.display_name(), "Qwen (text-embedding-v3)");
        assert_eq!(provider.provider_type(), EmbeddingProviderType::Qwen);
        assert!(!provider.is_local());
        assert_eq!(provider.max_batch_size(), 25);
        assert_eq!(provider.api_key, "test-api-key");
    }

    #[test]
    fn new_with_custom_model() {
        let config = config_with_model("text-embedding-v4");
        let provider = QwenEmbeddingProvider::new(&config);

        assert_eq!(provider.model, "text-embedding-v4");
        assert_eq!(provider.display_name(), "Qwen (text-embedding-v4)");
    }

    #[test]
    fn new_with_custom_base_url() {
        let config = config_with_base_url("https://custom-endpoint.example.com/embed");
        let provider = QwenEmbeddingProvider::new(&config);

        assert_eq!(
            provider.base_url,
            "https://custom-endpoint.example.com/embed"
        );
    }

    #[test]
    fn new_with_empty_model_uses_default() {
        let config = EmbeddingProviderConfig {
            model: "  ".to_string(),
            api_key: Some("test-key".to_string()),
            ..EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen)
        };
        let provider = QwenEmbeddingProvider::new(&config);

        assert_eq!(provider.model, DEFAULT_MODEL);
    }

    #[test]
    fn new_with_custom_dimension() {
        let config = EmbeddingProviderConfig {
            dimension: Some(512),
            api_key: Some("test-key".to_string()),
            ..EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen)
        };
        let provider = QwenEmbeddingProvider::new(&config);

        assert_eq!(provider.dimension(), 512);
    }

    #[test]
    fn new_without_api_key() {
        let config = EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen);
        let provider = QwenEmbeddingProvider::new(&config);

        assert_eq!(provider.api_key, "");
    }

    // =========================================================================
    // Trait property tests
    // =========================================================================

    #[test]
    fn provider_is_not_local() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        assert!(!provider.is_local());
    }

    #[test]
    fn provider_type_is_qwen() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        assert_eq!(provider.provider_type(), EmbeddingProviderType::Qwen);
    }

    #[test]
    fn max_batch_size_is_25() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        assert_eq!(provider.max_batch_size(), 25);
    }

    #[test]
    fn display_name_includes_model() {
        let config = config_with_model("text-embedding-v4");
        let provider = QwenEmbeddingProvider::new(&config);
        assert!(provider.display_name().contains("text-embedding-v4"));
        assert!(provider.display_name().contains("Qwen"));
    }

    // =========================================================================
    // Dimension tests
    // =========================================================================

    #[test]
    fn dimension_defaults_to_1024() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        assert_eq!(provider.dimension(), 1024);
    }

    #[test]
    fn dimension_is_fixed_at_construction() {
        let config = EmbeddingProviderConfig {
            dimension: Some(512),
            api_key: Some("test-key".to_string()),
            ..EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen)
        };
        let provider = QwenEmbeddingProvider::new(&config);
        assert_eq!(provider.dimension(), 512);
    }

    // =========================================================================
    // Error mapping tests
    // =========================================================================

    #[test]
    fn map_api_error_auth_failure() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_api_error("InvalidApiKey", Some("key is invalid"));
        assert!(matches!(err, EmbeddingError::AuthenticationFailed { .. }));
        assert!(err.to_string().contains("qwen_embedding"));
    }

    #[test]
    fn map_api_error_unauthorized() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_api_error("Unauthorized", Some("not authorized"));
        assert!(matches!(err, EmbeddingError::AuthenticationFailed { .. }));
    }

    #[test]
    fn map_api_error_rate_limited() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_api_error("Throttling", Some("too many requests"));
        assert!(matches!(err, EmbeddingError::RateLimited { .. }));
        assert!(err.is_retryable());
        assert_eq!(err.retry_after_secs(), Some(5));
    }

    #[test]
    fn map_api_error_quota_exhausted() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_api_error("QuotaExhausted", Some("no more tokens"));
        assert!(matches!(err, EmbeddingError::RateLimited { .. }));
        assert!(err.to_string().contains("quota"));
    }

    #[test]
    fn map_api_error_model_not_found() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_api_error("ModelNotFound", Some("model does not exist"));
        assert!(matches!(err, EmbeddingError::ModelNotFound { .. }));
    }

    #[test]
    fn map_api_error_internal_error() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_api_error("InternalError.Timeout", Some("request timeout"));
        assert!(matches!(err, EmbeddingError::ServerError { .. }));
        assert!(err.is_retryable());
    }

    #[test]
    fn map_api_error_unknown_code() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_api_error("SomeNewError", Some("unexpected"));
        assert!(matches!(err, EmbeddingError::Other { .. }));
    }

    #[test]
    fn map_api_error_data_inspection() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_api_error("DataInspectionFailed", Some("content blocked"));
        assert!(matches!(err, EmbeddingError::Other { .. }));
        assert!(err.to_string().contains("restricted content"));
    }

    #[test]
    fn map_http_error_401() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(401, "Unauthorized");
        assert!(matches!(err, EmbeddingError::AuthenticationFailed { .. }));
    }

    #[test]
    fn map_http_error_429() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(429, "Too Many Requests");
        assert!(matches!(err, EmbeddingError::RateLimited { .. }));
        assert!(err.is_retryable());
    }

    #[test]
    fn map_http_error_500() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let err = provider.map_http_error(500, "Internal Server Error");
        assert!(matches!(err, EmbeddingError::ServerError { status: Some(500), .. }));
        assert!(err.is_retryable());
    }

    #[test]
    fn map_http_error_with_api_error_body() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let body = r#"{"code":"InvalidApiKey","message":"bad key"}"#;
        let err = provider.map_http_error(401, body);
        assert!(matches!(err, EmbeddingError::AuthenticationFailed { .. }));
        // Should extract the API error code, not just use HTTP status
        assert!(err.to_string().contains("InvalidApiKey"));
    }

    // =========================================================================
    // Batch size enforcement tests
    // =========================================================================

    #[tokio::test]
    async fn embed_documents_rejects_oversized_batch() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let docs: Vec<&str> = (0..26).map(|_| "test").collect();
        let result = provider.embed_documents(&docs).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::BatchSizeLimitExceeded {
                requested: 26,
                max_allowed: 25,
            }
        ));
    }

    #[tokio::test]
    async fn embed_documents_empty_returns_empty() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let result = provider.embed_documents(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // =========================================================================
    // Auth validation tests
    // =========================================================================

    #[tokio::test]
    async fn embed_query_without_api_key_returns_auth_error() {
        let config = EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen);
        let provider = QwenEmbeddingProvider::new(&config);
        let result = provider.embed_query("test").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::AuthenticationFailed { .. }
        ));
    }

    #[tokio::test]
    async fn embed_documents_without_api_key_returns_auth_error() {
        let config = EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen);
        let provider = QwenEmbeddingProvider::new(&config);
        let result = provider.embed_documents(&["test"]).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::AuthenticationFailed { .. }
        ));
    }

    #[tokio::test]
    async fn health_check_without_api_key_returns_auth_error() {
        let config = EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen);
        let provider = QwenEmbeddingProvider::new(&config);
        let result = provider.health_check().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::AuthenticationFailed { .. }
        ));
    }

    // =========================================================================
    // Trait object safety tests
    // =========================================================================

    #[test]
    fn provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<QwenEmbeddingProvider>();
    }

    #[test]
    fn provider_is_object_safe() {
        fn _assert_object_safe(_: &dyn EmbeddingProvider) {}
    }

    #[test]
    fn as_any_downcast_works() {
        let provider = QwenEmbeddingProvider::new(&default_config());
        let any_ref: &dyn Any = provider.as_any();
        assert!(any_ref.downcast_ref::<QwenEmbeddingProvider>().is_some());
    }

    // =========================================================================
    // Request serialization tests
    // =========================================================================

    #[test]
    fn dashscope_request_serializes_with_dimension() {
        let request = DashScopeRequest {
            model: "text-embedding-v4".to_string(),
            input: DashScopeInput {
                texts: vec!["hello world".to_string(), "foo bar".to_string()],
            },
            parameters: DashScopeParameters {
                text_type: "document".to_string(),
                dimension: Some(1024),
            },
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "text-embedding-v4");
        assert_eq!(json["input"]["texts"].as_array().unwrap().len(), 2);
        assert_eq!(json["parameters"]["text_type"], "document");
        assert_eq!(json["parameters"]["dimension"], 1024);
    }

    #[test]
    fn dashscope_request_omits_dimension_when_none() {
        let request = DashScopeRequest {
            model: "text-embedding-v3".to_string(),
            input: DashScopeInput {
                texts: vec!["hello".to_string()],
            },
            parameters: DashScopeParameters {
                text_type: "query".to_string(),
                dimension: None,
            },
        };

        let json = serde_json::to_value(&request).unwrap();
        assert!(json["parameters"].get("dimension").is_none());
    }

    #[test]
    fn dashscope_response_deserializes_correctly() {
        let json = r#"{
            "output": {
                "embeddings": [
                    {"text_index": 0, "embedding": [0.1, 0.2, 0.3]},
                    {"text_index": 1, "embedding": [0.4, 0.5, 0.6]}
                ]
            },
            "request_id": "req-123"
        }"#;

        let response: DashScopeResponse = serde_json::from_str(json).unwrap();
        assert!(response.output.is_some());
        let output = response.output.unwrap();
        assert_eq!(output.embeddings.len(), 2);
        assert_eq!(output.embeddings[0].text_index, 0);
        assert_eq!(output.embeddings[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(output.embeddings[1].text_index, 1);
        assert_eq!(response.request_id, Some("req-123".to_string()));
        assert!(response.code.is_none());
    }

    #[test]
    fn dashscope_error_response_deserializes_correctly() {
        let json = r#"{
            "code": "InvalidApiKey",
            "message": "Invalid API-key provided",
            "request_id": "req-456"
        }"#;

        let response: DashScopeResponse = serde_json::from_str(json).unwrap();
        assert!(response.output.is_none());
        assert_eq!(response.code, Some("InvalidApiKey".to_string()));
        assert_eq!(
            response.message,
            Some("Invalid API-key provided".to_string())
        );
    }

    // =========================================================================
    // Integration tests (require valid DashScope API key — marked #[ignore])
    // =========================================================================

    #[tokio::test]
    #[ignore = "requires a valid DashScope API key set in config"]
    async fn integration_health_check() {
        let config = default_config();
        let provider = QwenEmbeddingProvider::new(&config);
        let result = provider.health_check().await;
        assert!(result.is_ok(), "health_check failed: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore = "requires a valid DashScope API key set in config"]
    async fn integration_embed_query() {
        let config = default_config();
        let provider = QwenEmbeddingProvider::new(&config);
        let result = provider.embed_query("hello world").await;
        assert!(result.is_ok(), "embed_query failed: {:?}", result.err());

        let embedding = result.unwrap();
        assert!(!embedding.is_empty());
        // After first call, dimension should be updated
        assert_eq!(provider.dimension(), embedding.len());
    }

    #[tokio::test]
    #[ignore = "requires a valid DashScope API key set in config"]
    async fn integration_embed_documents() {
        let config = default_config();
        let provider = QwenEmbeddingProvider::new(&config);
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
    #[ignore = "requires a valid DashScope API key set in config"]
    async fn integration_query_vs_document_produces_different_embeddings() {
        let config = default_config();
        let provider = QwenEmbeddingProvider::new(&config);

        let text = "machine learning algorithms";
        let query_embedding = provider.embed_query(text).await.unwrap();

        let doc_results = provider.embed_documents(&[text]).await.unwrap();
        let doc_embedding = &doc_results[0];

        // Query and document embeddings should differ (asymmetric)
        assert_ne!(
            &query_embedding, doc_embedding,
            "query and document embeddings should differ for asymmetric retrieval"
        );
    }
}
