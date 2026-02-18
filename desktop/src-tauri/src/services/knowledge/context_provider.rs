//! Knowledge Context Provider
//!
//! Provides automatic retrieval of relevant knowledge context for
//! agent execution. Queries project knowledge collections and formats
//! results as structured context blocks.
//!
//! ## Usage
//!
//! ```rust,ignore
//! let provider = KnowledgeContextProvider::new(pipeline, config);
//! let chunks = provider.query_for_context("proj-1", "Implement auth feature", &config).await?;
//! let context_block = KnowledgeContextProvider::format_context_block(&chunks);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

use crate::services::knowledge::pipeline::RagPipeline;
use crate::utils::error::AppResult;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A chunk of context retrieved from the knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChunk {
    /// Text content of the context chunk.
    pub content: String,
    /// Name of the source document.
    pub source_document: String,
    /// Name of the collection this came from.
    pub collection_name: String,
    /// Relevance score (0.0 to 1.0).
    pub relevance_score: f32,
}

/// Configuration for knowledge context retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeContextConfig {
    /// Whether auto-retrieval is enabled.
    pub enabled: bool,
    /// Maximum number of context chunks to retrieve.
    pub max_context_chunks: usize,
    /// Minimum relevance score for inclusion.
    pub minimum_relevance_score: f32,
}

impl Default for KnowledgeContextConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_context_chunks: 5,
            minimum_relevance_score: 0.3,
        }
    }
}

// ---------------------------------------------------------------------------
// KnowledgeContextProvider
// ---------------------------------------------------------------------------

/// Provides automatic knowledge context retrieval for agent execution.
///
/// Queries all project knowledge collections, merges and deduplicates
/// results, and formats them as structured context blocks for injection
/// into agent system prompts.
pub struct KnowledgeContextProvider {
    pipeline: Arc<RagPipeline>,
}

impl KnowledgeContextProvider {
    /// Create a new KnowledgeContextProvider.
    pub fn new(pipeline: Arc<RagPipeline>) -> Self {
        Self { pipeline }
    }

    /// Query all project knowledge collections for relevant context.
    ///
    /// Returns deduplicated context chunks sorted by relevance score.
    pub async fn query_for_context(
        &self,
        project_id: &str,
        query: &str,
        config: &KnowledgeContextConfig,
    ) -> AppResult<Vec<ContextChunk>> {
        if !config.enabled {
            return Ok(Vec::new());
        }

        // List all collections for the project
        let collections = self.pipeline.list_collections(project_id)?;

        if collections.is_empty() {
            return Ok(Vec::new());
        }

        // Query each collection
        let mut all_chunks: Vec<ContextChunk> = Vec::new();
        let top_k = config.max_context_chunks * 2; // Fetch more for dedup

        for collection in &collections {
            match self
                .pipeline
                .query(&collection.name, project_id, query, top_k)
                .await
            {
                Ok(result) => {
                    for search_result in result.results {
                        if search_result.score >= config.minimum_relevance_score {
                            all_chunks.push(ContextChunk {
                                content: search_result.chunk_text,
                                source_document: search_result.document_id,
                                collection_name: search_result.collection_name,
                                relevance_score: search_result.score,
                            });
                        }
                    }
                }
                Err(e) => {
                    // Log error but continue with other collections
                    tracing::warn!(
                        "Failed to query collection '{}': {}",
                        collection.name,
                        e
                    );
                }
            }
        }

        // Sort by relevance score descending
        all_chunks.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Deduplicate by content (keep highest-scored version)
        let mut seen = HashSet::new();
        all_chunks.retain(|chunk| {
            let key = chunk.content.chars().take(200).collect::<String>();
            seen.insert(key)
        });

        // Truncate to max_context_chunks
        all_chunks.truncate(config.max_context_chunks);

        Ok(all_chunks)
    }

    /// Format context chunks as a structured context block for agent prompts.
    pub fn format_context_block(chunks: &[ContextChunk]) -> String {
        if chunks.is_empty() {
            return String::new();
        }

        let mut block = String::from("## Relevant Knowledge Context\n\n");
        block.push_str("The following context was automatically retrieved from the project knowledge base:\n\n");

        for (i, chunk) in chunks.iter().enumerate() {
            block.push_str(&format!(
                "### Context {} (relevance: {:.2}, source: {}, collection: {})\n\n",
                i + 1,
                chunk.relevance_score,
                chunk.source_document,
                chunk.collection_name,
            ));
            block.push_str(&chunk.content);
            block.push_str("\n\n---\n\n");
        }

        block
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // ContextChunk tests
    // ======================================================================

    #[test]
    fn context_chunk_serde() {
        let chunk = ContextChunk {
            content: "Some content".to_string(),
            source_document: "doc-1".to_string(),
            collection_name: "col-1".to_string(),
            relevance_score: 0.85,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: ContextChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.content, "Some content");
        assert!((deserialized.relevance_score - 0.85).abs() < 0.001);
    }

    // ======================================================================
    // KnowledgeContextConfig tests
    // ======================================================================

    #[test]
    fn config_default_values() {
        let config = KnowledgeContextConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_context_chunks, 5);
        assert!((config.minimum_relevance_score - 0.3).abs() < 0.001);
    }

    #[test]
    fn config_serde_roundtrip() {
        let config = KnowledgeContextConfig {
            enabled: false,
            max_context_chunks: 10,
            minimum_relevance_score: 0.5,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: KnowledgeContextConfig = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.enabled);
        assert_eq!(deserialized.max_context_chunks, 10);
    }

    // ======================================================================
    // format_context_block tests
    // ======================================================================

    #[test]
    fn format_context_block_empty() {
        let result = KnowledgeContextProvider::format_context_block(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn format_context_block_with_chunks() {
        let chunks = vec![
            ContextChunk {
                content: "Rust uses ownership".to_string(),
                source_document: "rust-guide".to_string(),
                collection_name: "docs".to_string(),
                relevance_score: 0.95,
            },
            ContextChunk {
                content: "Lifetimes prevent dangling references".to_string(),
                source_document: "rust-guide".to_string(),
                collection_name: "docs".to_string(),
                relevance_score: 0.8,
            },
        ];

        let block = KnowledgeContextProvider::format_context_block(&chunks);
        assert!(block.contains("Relevant Knowledge Context"));
        assert!(block.contains("Rust uses ownership"));
        assert!(block.contains("Lifetimes prevent"));
        assert!(block.contains("0.95"));
        assert!(block.contains("Context 1"));
        assert!(block.contains("Context 2"));
    }

    // ======================================================================
    // Integration test with full pipeline
    // ======================================================================

    #[tokio::test]
    async fn query_for_context_with_pipeline() {
        use crate::services::knowledge::chunker::ParagraphChunker;
        use crate::services::knowledge::chunker::{Chunker, Document};
        use crate::services::knowledge::reranker::{NoopReranker, Reranker};
        use crate::services::orchestrator::embedding_manager::{
            EmbeddingManager, EmbeddingManagerConfig,
        };
        use crate::services::orchestrator::embedding_provider::{
            EmbeddingProviderConfig, EmbeddingProviderType,
        };
        use crate::services::orchestrator::hnsw_index::HnswIndex;
        use crate::storage::database::Database;
        use tempfile::tempdir;

        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test_ctx.db");
        let manager = r2d2_sqlite::SqliteConnectionManager::file(&db_path);
        let pool = r2d2::Pool::builder()
            .max_size(5)
            .build(manager)
            .expect("pool");
        {
            let conn = pool.get().expect("conn");
            conn.execute("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)", []).unwrap();
        }
        let db = Arc::new(Database::from_pool_for_test(pool));

        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let emb = Arc::new(EmbeddingManager::from_config(config).expect("emb"));
        let hnsw = Arc::new(HnswIndex::new(dir.path().join("hnsw"), 8192));
        let chunker: Arc<dyn Chunker> = Arc::new(ParagraphChunker::new(500));
        let reranker: Option<Arc<dyn Reranker>> = Some(Arc::new(NoopReranker));

        let pipeline = Arc::new(
            RagPipeline::new(chunker, emb, hnsw, reranker, db).expect("pipeline"),
        );

        // Ingest test documents
        let docs = vec![
            Document::new("d1", "Rust programming provides memory safety through ownership."),
            Document::new("d2", "Python is great for data science and machine learning."),
            Document::new("d3", "Database normalization reduces data redundancy."),
        ];
        pipeline
            .ingest("test-kb", "proj-1", "Test KB", docs)
            .await
            .unwrap();

        // Create provider and query
        let provider = KnowledgeContextProvider::new(pipeline);
        let ctx_config = KnowledgeContextConfig {
            enabled: true,
            max_context_chunks: 3,
            minimum_relevance_score: 0.0, // Accept all for testing
        };

        let chunks = provider
            .query_for_context("proj-1", "Rust memory safety", &ctx_config)
            .await
            .unwrap();

        assert!(!chunks.is_empty(), "Should return context chunks");

        // Test formatting
        let block = KnowledgeContextProvider::format_context_block(&chunks);
        assert!(block.contains("Relevant Knowledge Context"));
    }

    #[tokio::test]
    async fn query_for_context_disabled_returns_empty() {
        use crate::services::knowledge::chunker::ParagraphChunker;
        use crate::services::knowledge::chunker::Chunker;
        use crate::services::knowledge::reranker::{NoopReranker, Reranker};
        use crate::services::orchestrator::embedding_manager::{
            EmbeddingManager, EmbeddingManagerConfig,
        };
        use crate::services::orchestrator::embedding_provider::{
            EmbeddingProviderConfig, EmbeddingProviderType,
        };
        use crate::services::orchestrator::hnsw_index::HnswIndex;
        use crate::storage::database::Database;
        use tempfile::tempdir;

        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test_disabled.db");
        let manager = r2d2_sqlite::SqliteConnectionManager::file(&db_path);
        let pool = r2d2::Pool::builder()
            .max_size(5)
            .build(manager)
            .expect("pool");
        {
            let conn = pool.get().expect("conn");
            conn.execute("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)", []).unwrap();
        }
        let db = Arc::new(Database::from_pool_for_test(pool));

        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let emb = Arc::new(EmbeddingManager::from_config(config).expect("emb"));
        let hnsw = Arc::new(HnswIndex::new(dir.path().join("hnsw"), 8192));
        let chunker: Arc<dyn Chunker> = Arc::new(ParagraphChunker::new(500));
        let reranker: Option<Arc<dyn Reranker>> = Some(Arc::new(NoopReranker));

        let pipeline = Arc::new(
            RagPipeline::new(chunker, emb, hnsw, reranker, db).expect("pipeline"),
        );

        let provider = KnowledgeContextProvider::new(pipeline);
        let config = KnowledgeContextConfig {
            enabled: false, // Disabled
            max_context_chunks: 5,
            minimum_relevance_score: 0.3,
        };

        let chunks = provider
            .query_for_context("proj-1", "anything", &config)
            .await
            .unwrap();

        assert!(chunks.is_empty(), "Disabled config should return empty");
    }
}
