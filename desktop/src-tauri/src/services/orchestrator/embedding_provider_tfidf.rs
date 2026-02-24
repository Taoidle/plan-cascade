//! TF-IDF Embedding Provider Adapter
//!
//! Wraps the existing [`EmbeddingService`] in the [`EmbeddingProvider`] trait
//! so it can be used interchangeably with remote embedding backends (Ollama,
//! OpenAI, Qwen, GLM).
//!
//! ## Design Decisions
//!
//! * **Pure adapter** — delegates every call to `EmbeddingService` without
//!   changing any TF-IDF math, vocabulary behaviour, or thread-safety
//!   guarantees.
//! * **Shared ownership** — holds an `Arc<EmbeddingService>` so the same
//!   service instance can be used by both the provider trait and the legacy
//!   code paths during migration.
//! * **Sync-to-async bridge** — `EmbeddingService` uses `std::sync::Mutex`.
//!   We scope every lock acquisition tightly (no `.await` while the guard is
//!   alive) so there is no risk of holding a sync lock across an await point.

use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;

use super::embedding_provider::{
    EmbeddingError, EmbeddingProvider, EmbeddingProviderType, EmbeddingResult,
};
use super::embedding_service::EmbeddingService;

/// Adapter that exposes [`EmbeddingService`] through the [`EmbeddingProvider`]
/// async trait.
///
/// # Thread Safety
///
/// The inner `EmbeddingService` is already `Send + Sync` (via `Arc<Mutex<_>>`).
/// This wrapper adds no additional synchronisation — it simply delegates.
///
/// # TF-IDF–Specific Methods
///
/// Vocabulary management (`build_vocabulary`, `export_vocabulary`,
/// `import_vocabulary`) and `is_ready` are **not** part of the generic
/// `EmbeddingProvider` trait.  They are exposed as inherent methods on this
/// struct and can be accessed via a concrete reference or downcast.
pub struct TfIdfEmbeddingProvider {
    service: Arc<EmbeddingService>,
}

impl TfIdfEmbeddingProvider {
    /// Create a new adapter wrapping a shared `EmbeddingService`.
    pub fn new(service: Arc<EmbeddingService>) -> Self {
        Self { service }
    }

    /// Returns a reference to the underlying `EmbeddingService`.
    pub fn inner(&self) -> &EmbeddingService {
        &self.service
    }

    // -----------------------------------------------------------------
    // TF-IDF–specific helpers (NOT on the trait)
    // -----------------------------------------------------------------

    /// Build (or rebuild) the vocabulary from a corpus of documents.
    ///
    /// Delegates directly to [`EmbeddingService::build_vocabulary`].
    pub fn build_vocabulary(&self, corpus: &[&str]) {
        self.service.build_vocabulary(corpus);
    }

    /// Export the current vocabulary as a JSON string.
    ///
    /// Returns `None` if no vocabulary has been built yet.
    /// Delegates to [`EmbeddingService::export_vocabulary`].
    pub fn export_vocabulary(&self) -> Option<String> {
        self.service.export_vocabulary()
    }

    /// Import a vocabulary from a JSON string, replacing any existing
    /// vocabulary.
    ///
    /// Delegates to [`EmbeddingService::import_vocabulary`].
    pub fn import_vocabulary(&self, json: &str) -> Result<(), String> {
        self.service.import_vocabulary(json)
    }

    /// Check whether the vocabulary has been initialised.
    ///
    /// Delegates to [`EmbeddingService::is_ready`].
    pub fn is_ready(&self) -> bool {
        self.service.is_ready()
    }
}

// -------------------------------------------------------------------------
// EmbeddingProvider trait implementation
// -------------------------------------------------------------------------

#[async_trait]
impl EmbeddingProvider for TfIdfEmbeddingProvider {
    async fn embed_documents(&self, documents: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>> {
        // EmbeddingService::embed_batch acquires a std::sync::Mutex internally.
        // The lock is scoped within the synchronous call — no await while held.
        let vectors = self.service.embed_batch(documents);
        Ok(vectors)
    }

    async fn embed_query(&self, query: &str) -> EmbeddingResult<Vec<f32>> {
        let vector = self.service.embed_text(query);
        Ok(vector)
    }

    fn dimension(&self) -> usize {
        self.service.dimension()
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        if self.service.is_ready() {
            Ok(())
        } else {
            Err(EmbeddingError::ProviderUnavailable {
                message: "TF-IDF vocabulary has not been built yet".to_string(),
            })
        }
    }

    fn is_local(&self) -> bool {
        true
    }

    fn max_batch_size(&self) -> usize {
        1000
    }

    fn provider_type(&self) -> EmbeddingProviderType {
        EmbeddingProviderType::TfIdf
    }

    fn display_name(&self) -> &str {
        "TF-IDF (Local)"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// -------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // Construction & inner access
    // =====================================================================

    #[test]
    fn new_wraps_shared_service() {
        let svc = Arc::new(EmbeddingService::new());
        let provider = TfIdfEmbeddingProvider::new(Arc::clone(&svc));
        // Both point to the same underlying service.
        assert!(!provider.inner().is_ready());
        assert!(!svc.is_ready());
    }

    #[test]
    fn shared_arc_reflects_mutations() {
        let svc = Arc::new(EmbeddingService::new());
        let provider = TfIdfEmbeddingProvider::new(Arc::clone(&svc));

        // Build vocabulary through the raw service.
        svc.build_vocabulary(&["hello world", "foo bar"]);
        // The provider should immediately see the change.
        assert!(provider.is_ready());
        assert!(provider.inner().is_ready());
    }

    // =====================================================================
    // Trait metadata methods
    // =====================================================================

    #[test]
    fn is_local_returns_true() {
        let provider = TfIdfEmbeddingProvider::new(Arc::new(EmbeddingService::new()));
        assert!(provider.is_local());
    }

    #[test]
    fn max_batch_size_returns_1000() {
        let provider = TfIdfEmbeddingProvider::new(Arc::new(EmbeddingService::new()));
        assert_eq!(provider.max_batch_size(), 1000);
    }

    #[test]
    fn provider_type_is_tfidf() {
        let provider = TfIdfEmbeddingProvider::new(Arc::new(EmbeddingService::new()));
        assert_eq!(provider.provider_type(), EmbeddingProviderType::TfIdf);
    }

    #[test]
    fn display_name_is_correct() {
        let provider = TfIdfEmbeddingProvider::new(Arc::new(EmbeddingService::new()));
        assert_eq!(provider.display_name(), "TF-IDF (Local)");
    }

    #[test]
    fn dimension_returns_zero_before_vocab() {
        let provider = TfIdfEmbeddingProvider::new(Arc::new(EmbeddingService::new()));
        assert_eq!(provider.dimension(), 0);
    }

    #[test]
    fn dimension_returns_nonzero_after_vocab() {
        let svc = Arc::new(EmbeddingService::new());
        svc.build_vocabulary(&["hello world", "foo bar"]);
        let provider = TfIdfEmbeddingProvider::new(svc);
        assert!(provider.dimension() > 0);
    }

    // =====================================================================
    // Async trait methods (embed_documents, embed_query, health_check)
    // =====================================================================

    #[tokio::test]
    async fn embed_documents_returns_vectors() {
        let svc = Arc::new(EmbeddingService::new());
        svc.build_vocabulary(&["hello world", "foo bar", "baz qux"]);
        let provider = TfIdfEmbeddingProvider::new(svc);

        let result = provider.embed_documents(&["hello world", "foo bar"]).await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0].len(), provider.dimension());
        assert_eq!(vectors[1].len(), provider.dimension());
    }

    #[tokio::test]
    async fn embed_documents_builds_vocab_lazily() {
        let svc = Arc::new(EmbeddingService::new());
        let provider = TfIdfEmbeddingProvider::new(Arc::clone(&svc));

        // embed_batch (and therefore embed_documents) builds vocab if missing.
        let result = provider.embed_documents(&["hello world", "foo bar"]).await;
        assert!(result.is_ok());
        assert!(svc.is_ready());
    }

    #[tokio::test]
    async fn embed_query_returns_vector() {
        let svc = Arc::new(EmbeddingService::new());
        svc.build_vocabulary(&["hello world", "foo bar"]);
        let provider = TfIdfEmbeddingProvider::new(svc);

        let result = provider.embed_query("hello world").await;
        assert!(result.is_ok());
        let vector = result.unwrap();
        assert_eq!(vector.len(), provider.dimension());
    }

    #[tokio::test]
    async fn embed_query_returns_empty_before_vocab() {
        let provider = TfIdfEmbeddingProvider::new(Arc::new(EmbeddingService::new()));
        let result = provider.embed_query("hello world").await;
        assert!(result.is_ok());
        // EmbeddingService returns an empty vec when vocab is not built.
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn health_check_ok_when_ready() {
        let svc = Arc::new(EmbeddingService::new());
        svc.build_vocabulary(&["hello world"]);
        let provider = TfIdfEmbeddingProvider::new(svc);

        assert!(provider.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn health_check_err_when_not_ready() {
        let provider = TfIdfEmbeddingProvider::new(Arc::new(EmbeddingService::new()));
        let result = provider.health_check().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, EmbeddingError::ProviderUnavailable { .. }));
    }

    // =====================================================================
    // Vocabulary helper delegation
    // =====================================================================

    #[test]
    fn build_vocabulary_delegates() {
        let svc = Arc::new(EmbeddingService::new());
        let provider = TfIdfEmbeddingProvider::new(Arc::clone(&svc));
        assert!(!svc.is_ready());

        provider.build_vocabulary(&["hello world", "foo bar"]);
        assert!(svc.is_ready());
        assert!(provider.is_ready());
    }

    #[test]
    fn export_import_vocabulary_delegates() {
        let svc = Arc::new(EmbeddingService::new());
        let provider = TfIdfEmbeddingProvider::new(Arc::clone(&svc));

        // Build vocabulary and export through provider.
        provider.build_vocabulary(&["hello world", "foo bar"]);
        let json = provider.export_vocabulary();
        assert!(json.is_some());

        // Import into a fresh provider.
        let svc2 = Arc::new(EmbeddingService::new());
        let provider2 = TfIdfEmbeddingProvider::new(svc2);
        assert!(!provider2.is_ready());

        provider2.import_vocabulary(&json.unwrap()).unwrap();
        assert!(provider2.is_ready());
    }

    #[test]
    fn import_vocabulary_rejects_invalid_json() {
        let provider = TfIdfEmbeddingProvider::new(Arc::new(EmbeddingService::new()));
        let result = provider.import_vocabulary("not valid json");
        assert!(result.is_err());
    }

    // =====================================================================
    // Consistency: adapter produces same vectors as raw service
    // =====================================================================

    #[tokio::test]
    async fn adapter_produces_identical_vectors_to_raw_service() {
        let svc = Arc::new(EmbeddingService::new());
        svc.build_vocabulary(&["fn main() { }", "struct Config { }", "pub fn helper() { }"]);

        let provider = TfIdfEmbeddingProvider::new(Arc::clone(&svc));

        // Compare embed_query vs embed_text
        let raw_vec = svc.embed_text("fn main() { }");
        let provider_vec = provider.embed_query("fn main() { }").await.unwrap();
        assert_eq!(raw_vec, provider_vec);

        // Compare embed_documents vs embed_batch
        let texts = &["struct Config { }", "pub fn helper() { }"];
        let raw_batch = svc.embed_batch(texts);
        let provider_batch = provider.embed_documents(texts).await.unwrap();
        assert_eq!(raw_batch, provider_batch);
    }

    // =====================================================================
    // Thread safety
    // =====================================================================

    #[test]
    fn provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TfIdfEmbeddingProvider>();
    }

    #[test]
    fn provider_as_trait_object_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn EmbeddingProvider>>();
    }
}
