//! Embedding Manager Dispatch Layer
//!
//! Central orchestration layer for embedding operations. Routes requests to
//! a primary provider with automatic fallback, handles batch chunking by
//! provider limits, and maintains a content-level embedding cache keyed by
//! provider/model/text hash.
//!
//! ## Design Decision (ADR-F002)
//!
//! `EmbeddingManager` is the single dispatch point that consumers receive via
//! `Arc<EmbeddingManager>`. It centralises provider selection, fallback logic,
//! and caching policies so individual consumers never manage providers directly.
//!
//! ## Thread Safety
//!
//! The manager is `Send + Sync`. The internal cache uses `mini_moka::sync::Cache`
//! which is thread-safe with no external locking needed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tracing;

/// Maximum number of retry attempts for transient embedding errors.
const EMBED_MAX_RETRY_ATTEMPTS: usize = 3;

/// Base delay between retries (in milliseconds).  Actual delays follow
/// exponential backoff: 500ms, 1000ms, 2000ms, …
const EMBED_RETRY_BASE_DELAY_MS: u64 = 500;

/// Maximum delay cap to prevent excessively long waits.
const EMBED_RETRY_MAX_DELAY_MS: u64 = 10_000;

use mini_moka::sync::{Cache, ConcurrentCacheExt};

use super::embedding_provider::{
    EmbeddingError, EmbeddingProvider, EmbeddingProviderConfig, EmbeddingProviderType,
    EmbeddingResult,
};
use super::embedding_provider_glm::GlmEmbeddingProvider;
use super::embedding_provider_ollama::OllamaEmbeddingProvider;
use super::embedding_provider_openai::OpenAIEmbeddingProvider;
use super::embedding_provider_qwen::QwenEmbeddingProvider;
use super::embedding_provider_tfidf::TfIdfEmbeddingProvider;
use super::embedding_service::EmbeddingService;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the `EmbeddingManager` dispatch layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingManagerConfig {
    /// Primary embedding provider configuration.
    pub primary: EmbeddingProviderConfig,

    /// Optional fallback provider configuration.
    /// Used when the primary provider returns a retryable error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<EmbeddingProviderConfig>,

    /// Whether to enable the content-level embedding cache.
    #[serde(default = "default_cache_enabled")]
    pub cache_enabled: bool,

    /// Maximum number of entries in the embedding cache.
    #[serde(default = "default_cache_max_entries")]
    pub cache_max_entries: usize,
}

fn default_cache_enabled() -> bool {
    true
}

fn default_cache_max_entries() -> usize {
    10_000
}

// ---------------------------------------------------------------------------
// Cache key
// ---------------------------------------------------------------------------

/// Cache key combining provider type, model name, dimension, and a SHA-256
/// text hash.
///
/// Two embedding requests with identical (provider, model, dimension, text)
/// produce the same cache key, avoiding redundant provider calls.
/// Including `dimension` ensures that switching embedding dimensions (e.g.
/// after a provider config change) does not serve stale cached vectors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    provider: EmbeddingProviderType,
    model: String,
    dimension: usize,
    text_hash: [u8; 32],
}

impl CacheKey {
    fn new(provider: EmbeddingProviderType, model: &str, text: &str, dimension: usize) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();

        Self {
            provider,
            model: model.to_string(),
            dimension,
            text_hash: hash,
        }
    }
}

// ---------------------------------------------------------------------------
// EmbeddingManager
// ---------------------------------------------------------------------------

/// Central dispatch layer for embedding operations.
///
/// Manages a primary provider and optional fallback, with batch chunking and
/// content-level caching. Designed to be shared via `Arc<EmbeddingManager>`
/// across Tokio tasks.
///
/// # Fallback Behaviour
///
/// When the primary provider returns a *retryable* error (network, rate limit,
/// server error, provider unavailable), the manager automatically retries the
/// request against the fallback provider (if configured). Non-retryable errors
/// (authentication, invalid config, etc.) are returned immediately.
///
/// # Batch Chunking
///
/// If the caller passes N documents but the provider's `max_batch_size` is M < N,
/// the manager chunks the input into ceil(N/M) batches, embeds each, and
/// reassembles the results in order.
pub struct EmbeddingManager {
    /// Primary embedding provider.
    primary: Box<dyn EmbeddingProvider>,

    /// Optional fallback provider for retryable errors.
    fallback: Option<Box<dyn EmbeddingProvider>>,

    /// Content-level embedding cache.
    /// Key: (provider_type, model, sha256(text)) -> embedding vector.
    cache: Option<Cache<CacheKey, Vec<f32>>>,

    /// The configuration used to construct this manager.
    config: EmbeddingManagerConfig,
}

impl EmbeddingManager {
    /// Construct a new `EmbeddingManager` with pre-built providers.
    ///
    /// # Arguments
    ///
    /// * `primary` - The primary embedding provider.
    /// * `fallback` - Optional fallback provider.
    /// * `config` - The manager configuration.
    pub fn new(
        primary: Box<dyn EmbeddingProvider>,
        fallback: Option<Box<dyn EmbeddingProvider>>,
        config: EmbeddingManagerConfig,
    ) -> Self {
        let cache = if config.cache_enabled {
            Some(
                Cache::builder()
                    .max_capacity(config.cache_max_entries as u64)
                    .time_to_live(Duration::from_secs(30 * 60)) // 30 min TTL
                    .build(),
            )
        } else {
            None
        };

        Self {
            primary,
            fallback,
            cache,
            config,
        }
    }

    /// Factory method: construct an `EmbeddingManager` from configuration.
    ///
    /// Creates the appropriate provider instances based on the config.
    /// Currently supports `TfIdf`, `Ollama`, `Qwen`, and `Glm` provider types.
    ///
    /// # Errors
    ///
    /// Returns `EmbeddingError::InvalidConfig` if the provider type is not
    /// yet supported by the factory or if config validation fails.
    pub fn from_config(config: EmbeddingManagerConfig) -> EmbeddingResult<Self> {
        // Validate primary config
        config.primary.validate()?;

        let primary = Self::build_provider(&config.primary)?;

        let fallback = if let Some(ref fb_config) = config.fallback {
            fb_config.validate()?;
            Some(Self::build_provider(fb_config)?)
        } else {
            None
        };

        Ok(Self::new(primary, fallback, config))
    }

    /// Build a single provider from its configuration.
    fn build_provider(
        config: &EmbeddingProviderConfig,
    ) -> EmbeddingResult<Box<dyn EmbeddingProvider>> {
        match config.provider {
            EmbeddingProviderType::TfIdf => {
                let service = Arc::new(EmbeddingService::new());
                let provider = TfIdfEmbeddingProvider::new(service);
                Ok(Box::new(provider))
            }
            EmbeddingProviderType::Ollama => {
                let provider = OllamaEmbeddingProvider::new(config);
                Ok(Box::new(provider))
            }
            EmbeddingProviderType::Qwen => {
                let provider = QwenEmbeddingProvider::new(config);
                Ok(Box::new(provider))
            }
            EmbeddingProviderType::Glm => {
                let provider = GlmEmbeddingProvider::new(config);
                Ok(Box::new(provider))
            }
            EmbeddingProviderType::OpenAI => {
                let provider = OpenAIEmbeddingProvider::new(config);
                Ok(Box::new(provider))
            }
        }
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Embed a batch of document texts with batching, caching, and fallback.
    ///
    /// - Checks the cache for each document, only embedding cache misses.
    /// - Chunks the uncached documents by provider `max_batch_size`.
    /// - Falls back to the fallback provider on retryable errors.
    /// - Returns vectors in the same order as the input documents.
    pub async fn embed_documents(&self, documents: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        // Try primary first, fallback on retryable errors.
        match self
            .embed_documents_with_provider(&*self.primary, documents)
            .await
        {
            Ok(results) => Ok(results),
            Err(err) if err.is_retryable() && self.fallback.is_some() => {
                tracing::warn!(
                    "primary provider {} failed with retryable error: {}; trying fallback",
                    self.primary.display_name(),
                    err
                );
                let fallback = self.fallback.as_ref().unwrap();
                self.embed_documents_with_provider(&**fallback, documents)
                    .await
            }
            Err(err) => Err(err),
        }
    }

    /// Embed a single query text with caching and fallback.
    ///
    /// Checks the cache first. On cache miss, delegates to the primary
    /// provider (with fallback on retryable errors).
    pub async fn embed_query(&self, query: &str) -> EmbeddingResult<Vec<f32>> {
        // Try primary first, fallback on retryable errors.
        match self.embed_query_with_provider(&*self.primary, query).await {
            Ok(result) => Ok(result),
            Err(err) if err.is_retryable() && self.fallback.is_some() => {
                tracing::warn!(
                    "primary provider {} failed with retryable error: {}; trying fallback",
                    self.primary.display_name(),
                    err
                );
                let fallback = self.fallback.as_ref().unwrap();
                self.embed_query_with_provider(&**fallback, query).await
            }
            Err(err) => Err(err),
        }
    }

    /// Check if the primary (and optionally fallback) providers are healthy.
    pub async fn health_check(&self) -> EmbeddingResult<()> {
        self.primary.health_check().await?;
        if let Some(ref fallback) = self.fallback {
            fallback.health_check().await?;
        }
        Ok(())
    }

    /// Returns the dimensionality of the primary provider's embeddings.
    pub fn dimension(&self) -> usize {
        self.primary.dimension()
    }

    /// Returns the primary provider's type identifier.
    pub fn provider_type(&self) -> EmbeddingProviderType {
        self.primary.provider_type()
    }

    /// Returns the primary provider's human-readable display name.
    /// Examples: "OpenAI (text-embedding-3-small)", "TF-IDF (Local)", "Ollama (all-minilm)"
    pub fn display_name(&self) -> &str {
        self.primary.display_name()
    }

    /// Returns a reference to the primary provider for direct access
    /// (e.g., TF-IDF vocabulary operations via downcast).
    pub fn primary_provider(&self) -> &dyn EmbeddingProvider {
        &*self.primary
    }

    /// Returns a reference to the fallback provider, if configured.
    pub fn fallback_provider(&self) -> Option<&dyn EmbeddingProvider> {
        self.fallback.as_deref()
    }

    /// Returns a reference to the manager configuration.
    pub fn config(&self) -> &EmbeddingManagerConfig {
        &self.config
    }

    /// Returns the number of entries currently in the cache.
    ///
    /// Runs pending internal tasks first to ensure the count is accurate.
    /// Returns 0 if the cache is disabled.
    pub fn cache_entry_count(&self) -> u64 {
        self.cache.as_ref().map_or(0, |c| {
            c.sync();
            c.entry_count()
        })
    }

    /// Invalidate all entries in the cache.
    pub fn cache_invalidate_all(&self) {
        if let Some(ref cache) = self.cache {
            cache.invalidate_all();
            cache.sync();
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Embed a batch of texts with retry + exponential backoff for transient errors.
    ///
    /// Retries up to `EMBED_MAX_RETRY_ATTEMPTS` times on retryable errors.
    /// Respects `retry_after_secs()` from rate-limit responses; otherwise uses
    /// exponential backoff starting at `EMBED_RETRY_BASE_DELAY_MS`.
    async fn embed_batch_with_retry(
        provider: &dyn EmbeddingProvider,
        batch: &[&str],
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        let mut last_err: Option<EmbeddingError> = None;

        for attempt in 0..EMBED_MAX_RETRY_ATTEMPTS {
            match provider.embed_documents(batch).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    if !err.is_retryable() {
                        return Err(err);
                    }

                    // Determine wait duration
                    let wait_ms = if let Some(secs) = err.retry_after_secs() {
                        secs * 1000
                    } else {
                        let backoff = EMBED_RETRY_BASE_DELAY_MS * (1u64 << attempt);
                        backoff.min(EMBED_RETRY_MAX_DELAY_MS)
                    };

                    tracing::warn!(
                        attempt = attempt + 1,
                        max_attempts = EMBED_MAX_RETRY_ATTEMPTS,
                        wait_ms,
                        error = %err,
                        "embed_batch_with_retry: retryable error, backing off"
                    );

                    last_err = Some(err);
                    tokio::time::sleep(Duration::from_millis(wait_ms)).await;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| EmbeddingError::Other {
            message: "retry attempts exhausted".to_string(),
        }))
    }

    /// Embed a single query with retry + exponential backoff for transient errors.
    async fn embed_query_with_retry(
        provider: &dyn EmbeddingProvider,
        query: &str,
    ) -> EmbeddingResult<Vec<f32>> {
        let mut last_err: Option<EmbeddingError> = None;

        for attempt in 0..EMBED_MAX_RETRY_ATTEMPTS {
            match provider.embed_query(query).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    if !err.is_retryable() {
                        return Err(err);
                    }

                    let wait_ms = if let Some(secs) = err.retry_after_secs() {
                        secs * 1000
                    } else {
                        let backoff = EMBED_RETRY_BASE_DELAY_MS * (1u64 << attempt);
                        backoff.min(EMBED_RETRY_MAX_DELAY_MS)
                    };

                    tracing::warn!(
                        attempt = attempt + 1,
                        max_attempts = EMBED_MAX_RETRY_ATTEMPTS,
                        wait_ms,
                        error = %err,
                        "embed_query_with_retry: retryable error, backing off"
                    );

                    last_err = Some(err);
                    tokio::time::sleep(Duration::from_millis(wait_ms)).await;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| EmbeddingError::Other {
            message: "retry attempts exhausted".to_string(),
        }))
    }

    /// Embed documents via a specific provider, with cache lookup and batch chunking.
    async fn embed_documents_with_provider(
        &self,
        provider: &dyn EmbeddingProvider,
        documents: &[&str],
    ) -> EmbeddingResult<Vec<Vec<f32>>> {
        let provider_type = provider.provider_type();
        let model = provider.display_name().to_string();
        let expected_dim = provider.dimension();

        // Phase 1: Check cache for each document
        let mut results: Vec<Option<Vec<f32>>> = Vec::with_capacity(documents.len());
        let mut uncached_indices: Vec<usize> = Vec::new();
        let mut uncached_texts: Vec<&str> = Vec::new();

        for (i, &doc) in documents.iter().enumerate() {
            if let Some(cached) = self.cache_get(provider_type, &model, doc, expected_dim) {
                results.push(Some(cached));
            } else {
                results.push(None);
                uncached_indices.push(i);
                uncached_texts.push(doc);
            }
        }

        // Phase 2: Embed uncached texts in batches (with retry)
        if !uncached_texts.is_empty() {
            let max_batch = provider.max_batch_size();
            let mut all_embeddings: Vec<Vec<f32>> = Vec::with_capacity(uncached_texts.len());

            for chunk in uncached_texts.chunks(max_batch) {
                let batch_result = Self::embed_batch_with_retry(provider, chunk).await?;
                all_embeddings.extend(batch_result);
            }

            // Phase 3: Insert into cache and fill in results
            for (batch_idx, &original_idx) in uncached_indices.iter().enumerate() {
                let embedding = all_embeddings[batch_idx].clone();
                self.cache_put(
                    provider_type,
                    &model,
                    documents[original_idx],
                    embedding.clone(),
                );
                results[original_idx] = Some(embedding);
            }
        }

        // Unwrap all Option<Vec<f32>> to Vec<f32>
        Ok(results.into_iter().map(|opt| opt.unwrap()).collect())
    }

    /// Embed a single query via a specific provider, with cache lookup.
    async fn embed_query_with_provider(
        &self,
        provider: &dyn EmbeddingProvider,
        query: &str,
    ) -> EmbeddingResult<Vec<f32>> {
        let provider_type = provider.provider_type();
        let model = provider.display_name().to_string();
        let expected_dim = provider.dimension();

        // Check cache
        if let Some(cached) = self.cache_get(provider_type, &model, query, expected_dim) {
            return Ok(cached);
        }

        // Cache miss: call provider with retry
        let embedding = Self::embed_query_with_retry(provider, query).await?;

        // Store in cache
        self.cache_put(provider_type, &model, query, embedding.clone());

        Ok(embedding)
    }

    /// Look up a cached embedding for the given (provider, model, text).
    ///
    /// Includes the expected dimension in the cache key to avoid serving
    /// stale vectors after a provider dimension change. On cache hit,
    /// verifies the cached vector length matches `expected_dimension`.
    fn cache_get(
        &self,
        provider: EmbeddingProviderType,
        model: &str,
        text: &str,
        expected_dimension: usize,
    ) -> Option<Vec<f32>> {
        self.cache.as_ref().and_then(|c| {
            let key = CacheKey::new(provider, model, text, expected_dimension);
            let cached = c.get(&key)?;
            // Reject if the cached vector dimension doesn't match
            if expected_dimension > 0 && cached.len() != expected_dimension {
                tracing::warn!(
                    cached_dim = cached.len(),
                    expected_dim = expected_dimension,
                    "Embedding cache hit rejected: dimension mismatch"
                );
                return None;
            }
            Some(cached)
        })
    }

    /// Store an embedding in the cache.
    fn cache_put(
        &self,
        provider: EmbeddingProviderType,
        model: &str,
        text: &str,
        embedding: Vec<f32>,
    ) {
        if let Some(ref cache) = self.cache {
            let dimension = embedding.len();
            let key = CacheKey::new(provider, model, text, dimension);
            cache.insert(key, embedding);
        }
    }
}

// ---------------------------------------------------------------------------
// Thread safety assertions
// ---------------------------------------------------------------------------

// Compile-time assertion that EmbeddingManager is Send + Sync.
const _: () = {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_embedding_manager() {
        assert_send_sync::<EmbeddingManager>();
    }
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::any::Any;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // =====================================================================
    // Mock providers
    // =====================================================================

    /// A mock embedding provider that returns predictable vectors.
    ///
    /// Each document gets a vector of `[hash(text) as f32; dimension]`.
    /// Tracks call counts for cache verification.
    struct MockProvider {
        provider_type: EmbeddingProviderType,
        name: String,
        dim: usize,
        max_batch: usize,
        embed_call_count: AtomicUsize,
        should_fail: bool,
        fail_retryable: bool,
    }

    impl MockProvider {
        fn new(name: &str, provider_type: EmbeddingProviderType) -> Self {
            Self {
                provider_type,
                name: name.to_string(),
                dim: 4,
                max_batch: 3,
                embed_call_count: AtomicUsize::new(0),
                should_fail: false,
                fail_retryable: false,
            }
        }

        fn with_max_batch(mut self, max: usize) -> Self {
            self.max_batch = max;
            self
        }

        fn with_failure(mut self, retryable: bool) -> Self {
            self.should_fail = true;
            self.fail_retryable = retryable;
            self
        }

        fn call_count(&self) -> usize {
            self.embed_call_count.load(Ordering::SeqCst)
        }

        /// Generate a deterministic fake embedding for a text.
        fn fake_embedding(&self, text: &str) -> Vec<f32> {
            // Use a simple hash to produce deterministic but distinct vectors.
            let mut hash: u32 = 0;
            for b in text.bytes() {
                hash = hash.wrapping_mul(31).wrapping_add(b as u32);
            }
            vec![hash as f32; self.dim]
        }
    }

    #[async_trait]
    impl EmbeddingProvider for MockProvider {
        async fn embed_documents(&self, documents: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>> {
            self.embed_call_count.fetch_add(1, Ordering::SeqCst);

            if self.should_fail {
                if self.fail_retryable {
                    return Err(EmbeddingError::NetworkError {
                        message: "mock retryable failure".to_string(),
                    });
                } else {
                    return Err(EmbeddingError::AuthenticationFailed {
                        message: "mock non-retryable failure".to_string(),
                    });
                }
            }

            if documents.len() > self.max_batch {
                return Err(EmbeddingError::BatchSizeLimitExceeded {
                    requested: documents.len(),
                    max_allowed: self.max_batch,
                });
            }

            Ok(documents.iter().map(|d| self.fake_embedding(d)).collect())
        }

        async fn embed_query(&self, query: &str) -> EmbeddingResult<Vec<f32>> {
            self.embed_call_count.fetch_add(1, Ordering::SeqCst);

            if self.should_fail {
                if self.fail_retryable {
                    return Err(EmbeddingError::NetworkError {
                        message: "mock retryable failure".to_string(),
                    });
                } else {
                    return Err(EmbeddingError::AuthenticationFailed {
                        message: "mock non-retryable failure".to_string(),
                    });
                }
            }

            Ok(self.fake_embedding(query))
        }

        fn dimension(&self) -> usize {
            self.dim
        }

        async fn health_check(&self) -> EmbeddingResult<()> {
            if self.should_fail {
                Err(EmbeddingError::ProviderUnavailable {
                    message: "mock unhealthy".to_string(),
                })
            } else {
                Ok(())
            }
        }

        fn is_local(&self) -> bool {
            true
        }

        fn max_batch_size(&self) -> usize {
            self.max_batch
        }

        fn provider_type(&self) -> EmbeddingProviderType {
            self.provider_type
        }

        fn display_name(&self) -> &str {
            &self.name
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    // =====================================================================
    // Helper: default test config
    // =====================================================================

    fn test_config(cache_enabled: bool) -> EmbeddingManagerConfig {
        EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled,
            cache_max_entries: 100,
        }
    }

    // =====================================================================
    // Construction tests
    // =====================================================================

    #[test]
    fn new_creates_manager_with_cache() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(true);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        assert!(manager.cache.is_some());
        assert_eq!(manager.cache_entry_count(), 0);
    }

    #[test]
    fn new_creates_manager_without_cache() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        assert!(manager.cache.is_none());
        assert_eq!(manager.cache_entry_count(), 0);
    }

    #[test]
    fn manager_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EmbeddingManager>();
    }

    #[test]
    fn manager_works_with_arc() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(true);
        let manager = Arc::new(EmbeddingManager::new(Box::new(primary), None, config));

        // Can clone Arc and access from multiple references.
        let _clone = Arc::clone(&manager);
        assert_eq!(manager.dimension(), 4);
    }

    // =====================================================================
    // from_config factory tests
    // =====================================================================

    #[test]
    fn from_config_creates_tfidf_manager() {
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: true,
            cache_max_entries: 100,
        };
        let manager = EmbeddingManager::from_config(config);
        assert!(manager.is_ok());

        let m = manager.unwrap();
        assert_eq!(m.provider_type(), EmbeddingProviderType::TfIdf);
    }

    #[test]
    fn from_config_creates_ollama_manager() {
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::Ollama),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let manager = EmbeddingManager::from_config(config);
        assert!(manager.is_ok());

        let m = manager.unwrap();
        assert_eq!(m.provider_type(), EmbeddingProviderType::Ollama);
    }

    #[test]
    fn from_config_creates_qwen_manager() {
        let mut qwen_config = EmbeddingProviderConfig::new(EmbeddingProviderType::Qwen);
        qwen_config.api_key = Some("test-dashscope-key".to_string());
        let config = EmbeddingManagerConfig {
            primary: qwen_config,
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let manager = EmbeddingManager::from_config(config);
        assert!(manager.is_ok());

        let m = manager.unwrap();
        assert_eq!(m.provider_type(), EmbeddingProviderType::Qwen);
    }

    #[test]
    fn from_config_creates_glm_manager() {
        let mut glm_config = EmbeddingProviderConfig::new(EmbeddingProviderType::Glm);
        glm_config.api_key = Some("test-glm-key".to_string());
        let config = EmbeddingManagerConfig {
            primary: glm_config,
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let manager = EmbeddingManager::from_config(config);
        assert!(manager.is_ok());

        let m = manager.unwrap();
        assert_eq!(m.provider_type(), EmbeddingProviderType::Glm);
    }

    #[test]
    fn from_config_with_fallback() {
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: Some(EmbeddingProviderConfig::new(EmbeddingProviderType::Ollama)),
            cache_enabled: true,
            cache_max_entries: 50,
        };
        let manager = EmbeddingManager::from_config(config);
        assert!(manager.is_ok());

        let m = manager.unwrap();
        assert!(m.fallback_provider().is_some());
    }

    #[test]
    fn from_config_creates_openai_manager() {
        let mut config_primary = EmbeddingProviderConfig::new(EmbeddingProviderType::OpenAI);
        config_primary.api_key = Some("sk-test".to_string());
        let config = EmbeddingManagerConfig {
            primary: config_primary,
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let manager = EmbeddingManager::from_config(config);
        assert!(manager.is_ok());

        let m = manager.unwrap();
        assert_eq!(m.provider_type(), EmbeddingProviderType::OpenAI);
    }

    #[test]
    fn from_config_validates_primary_config() {
        let mut bad_config = EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf);
        bad_config.model = "".to_string(); // invalid
        let config = EmbeddingManagerConfig {
            primary: bad_config,
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let manager = EmbeddingManager::from_config(config);
        assert!(manager.is_err());
    }

    // =====================================================================
    // embed_documents tests
    // =====================================================================

    #[tokio::test]
    async fn embed_documents_returns_correct_count() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        let result = manager.embed_documents(&["hello", "world", "foo"]).await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert_eq!(vectors.len(), 3);
        assert_eq!(vectors[0].len(), 4); // dimension
    }

    #[tokio::test]
    async fn embed_documents_empty_input() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        let result = manager.embed_documents(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn embed_documents_preserves_order_with_batching() {
        // Provider max_batch_size = 2, send 5 documents.
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf).with_max_batch(2);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        let docs = ["alpha", "beta", "gamma", "delta", "epsilon"];
        let result = manager.embed_documents(&docs).await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert_eq!(vectors.len(), 5);

        // Verify order: each vector should match the expected fake embedding.
        let mock = MockProvider::new("verify", EmbeddingProviderType::TfIdf);
        for (i, doc) in docs.iter().enumerate() {
            assert_eq!(
                vectors[i],
                mock.fake_embedding(doc),
                "vector {} is out of order",
                i
            );
        }
    }

    #[tokio::test]
    async fn embed_documents_chunks_correctly() {
        // Provider max_batch_size = 3. Sending 7 docs should produce 3 batches (3+3+1).
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf).with_max_batch(3);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        let docs: Vec<&str> = (0..7)
            .map(|i| match i {
                0 => "a",
                1 => "b",
                2 => "c",
                3 => "d",
                4 => "e",
                5 => "f",
                _ => "g",
            })
            .collect();

        let result = manager.embed_documents(&docs).await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert_eq!(vectors.len(), 7);

        // The primary was called 3 times (3 batches).
        // We can't check call count directly since the manager owns the provider,
        // but the correct count of results confirms batching worked.
    }

    // =====================================================================
    // embed_query tests
    // =====================================================================

    #[tokio::test]
    async fn embed_query_returns_vector() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        let result = manager.embed_query("hello world").await;
        assert!(result.is_ok());
        let vector = result.unwrap();
        assert_eq!(vector.len(), 4);
    }

    // =====================================================================
    // Cache tests
    // =====================================================================

    #[tokio::test]
    async fn cache_hit_avoids_reembedding() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(true);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        // First call: cache miss
        let r1 = manager.embed_query("hello").await.unwrap();
        assert_eq!(manager.cache_entry_count(), 1);

        // Second call: cache hit
        let r2 = manager.embed_query("hello").await.unwrap();
        assert_eq!(r1, r2);

        // Cache still has 1 entry (no duplicates)
        assert_eq!(manager.cache_entry_count(), 1);
    }

    #[tokio::test]
    async fn cache_hit_in_documents() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(true);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        // First call: embeds "alpha" and "beta"
        let r1 = manager.embed_documents(&["alpha", "beta"]).await.unwrap();
        assert_eq!(manager.cache_entry_count(), 2);

        // Second call: "alpha" cached, "gamma" is new
        let r2 = manager.embed_documents(&["alpha", "gamma"]).await.unwrap();
        assert_eq!(manager.cache_entry_count(), 3);

        // "alpha" should produce the same embedding in both calls
        assert_eq!(r1[0], r2[0]);
    }

    #[tokio::test]
    async fn cache_disabled_does_not_store() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        let _r1 = manager.embed_query("hello").await.unwrap();
        assert_eq!(manager.cache_entry_count(), 0); // no cache
    }

    #[tokio::test]
    async fn cache_invalidate_all_clears_entries() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(true);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        manager.embed_query("hello").await.unwrap();
        manager.embed_query("world").await.unwrap();
        assert_eq!(manager.cache_entry_count(), 2);

        manager.cache_invalidate_all();
        assert_eq!(manager.cache_entry_count(), 0);
    }

    // =====================================================================
    // Fallback tests
    // =====================================================================

    #[tokio::test]
    async fn fallback_used_on_retryable_error() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf).with_failure(true); // retryable
        let fallback = MockProvider::new("fallback", EmbeddingProviderType::Ollama);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), Some(Box::new(fallback)), config);

        let result = manager.embed_query("hello").await;
        assert!(result.is_ok(), "should have succeeded via fallback");
    }

    #[tokio::test]
    async fn fallback_not_used_on_nonretryable_error() {
        let primary =
            MockProvider::new("primary", EmbeddingProviderType::TfIdf).with_failure(false); // non-retryable
        let fallback = MockProvider::new("fallback", EmbeddingProviderType::Ollama);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), Some(Box::new(fallback)), config);

        let result = manager.embed_query("hello").await;
        assert!(result.is_err(), "non-retryable error should not fallback");
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::AuthenticationFailed { .. }
        ));
    }

    #[tokio::test]
    async fn no_fallback_returns_primary_error() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf).with_failure(true); // retryable, but no fallback
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        let result = manager.embed_query("hello").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmbeddingError::NetworkError { .. }
        ));
    }

    #[tokio::test]
    async fn fallback_used_for_documents_on_retryable_error() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf).with_failure(true); // retryable
        let fallback = MockProvider::new("fallback", EmbeddingProviderType::Ollama);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), Some(Box::new(fallback)), config);

        let result = manager.embed_documents(&["hello", "world"]).await;
        assert!(result.is_ok(), "should have succeeded via fallback");
        assert_eq!(result.unwrap().len(), 2);
    }

    // =====================================================================
    // Metadata delegation tests
    // =====================================================================

    #[test]
    fn dimension_delegates_to_primary() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);
        assert_eq!(manager.dimension(), 4);
    }

    #[test]
    fn provider_type_delegates_to_primary() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);
        assert_eq!(manager.provider_type(), EmbeddingProviderType::TfIdf);
    }

    #[test]
    fn primary_provider_returns_reference() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);
        assert_eq!(
            manager.primary_provider().provider_type(),
            EmbeddingProviderType::TfIdf
        );
    }

    #[test]
    fn fallback_provider_returns_none_when_absent() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);
        assert!(manager.fallback_provider().is_none());
    }

    #[test]
    fn fallback_provider_returns_some_when_present() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let fallback = MockProvider::new("fallback", EmbeddingProviderType::Ollama);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), Some(Box::new(fallback)), config);
        assert!(manager.fallback_provider().is_some());
        assert_eq!(
            manager.fallback_provider().unwrap().provider_type(),
            EmbeddingProviderType::Ollama
        );
    }

    // =====================================================================
    // health_check tests
    // =====================================================================

    #[tokio::test]
    async fn health_check_succeeds_when_both_healthy() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let fallback = MockProvider::new("fallback", EmbeddingProviderType::Ollama);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), Some(Box::new(fallback)), config);

        assert!(manager.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn health_check_fails_when_primary_unhealthy() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf).with_failure(true);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), None, config);

        assert!(manager.health_check().await.is_err());
    }

    #[tokio::test]
    async fn health_check_fails_when_fallback_unhealthy() {
        let primary = MockProvider::new("primary", EmbeddingProviderType::TfIdf);
        let fallback =
            MockProvider::new("fallback", EmbeddingProviderType::Ollama).with_failure(true);
        let config = test_config(false);
        let manager = EmbeddingManager::new(Box::new(primary), Some(Box::new(fallback)), config);

        assert!(manager.health_check().await.is_err());
    }

    // =====================================================================
    // CacheKey tests
    // =====================================================================

    #[test]
    fn cache_key_deterministic() {
        let k1 = CacheKey::new(EmbeddingProviderType::TfIdf, "model-a", "hello", 128);
        let k2 = CacheKey::new(EmbeddingProviderType::TfIdf, "model-a", "hello", 128);
        assert_eq!(k1, k2);
        assert_eq!(k1.text_hash, k2.text_hash);
    }

    #[test]
    fn cache_key_different_text() {
        let k1 = CacheKey::new(EmbeddingProviderType::TfIdf, "model-a", "hello", 128);
        let k2 = CacheKey::new(EmbeddingProviderType::TfIdf, "model-a", "world", 128);
        assert_ne!(k1, k2);
    }

    #[test]
    fn cache_key_different_model() {
        let k1 = CacheKey::new(EmbeddingProviderType::TfIdf, "model-a", "hello", 128);
        let k2 = CacheKey::new(EmbeddingProviderType::TfIdf, "model-b", "hello", 128);
        assert_ne!(k1, k2);
    }

    #[test]
    fn cache_key_different_provider() {
        let k1 = CacheKey::new(EmbeddingProviderType::TfIdf, "model-a", "hello", 128);
        let k2 = CacheKey::new(EmbeddingProviderType::Ollama, "model-a", "hello", 128);
        assert_ne!(k1, k2);
    }

    #[test]
    fn cache_key_different_dimensions_not_equal() {
        let k1 = CacheKey::new(EmbeddingProviderType::TfIdf, "model-a", "hello", 128);
        let k2 = CacheKey::new(EmbeddingProviderType::TfIdf, "model-a", "hello", 768);
        assert_ne!(k1, k2, "Different dimensions should produce different cache keys");
    }

    // =====================================================================
    // Config serialization tests
    // =====================================================================

    #[test]
    fn config_serde_roundtrip() {
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: Some(EmbeddingProviderConfig::new(EmbeddingProviderType::Ollama)),
            cache_enabled: true,
            cache_max_entries: 5000,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: EmbeddingManagerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.cache_max_entries, 5000);
        assert!(deserialized.cache_enabled);
        assert!(deserialized.fallback.is_some());
    }

    #[test]
    fn config_serde_without_fallback() {
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("fallback"));
        let deserialized: EmbeddingManagerConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.fallback.is_none());
    }
}
