//! RAG Pipeline
//!
//! Orchestrates the full Retrieval-Augmented Generation flow:
//! chunking -> embedding -> indexing -> querying -> reranking.
//!
//! Manages named collections (namespaces) with SQLite persistence.
//! Each collection has its own set of chunks indexed in HNSW with
//! collection-namespaced IDs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::services::knowledge::chunker::{Chunk, Chunker, Document};
use crate::services::knowledge::reranker::{Reranker, SearchResult};
use crate::services::orchestrator::embedding_manager::EmbeddingManager;
use crate::services::orchestrator::embedding_service::{
    cosine_similarity, embedding_to_bytes, bytes_to_embedding,
};
use crate::services::orchestrator::hnsw_index::HnswIndex;
use crate::storage::database::Database;
use crate::utils::error::{AppError, AppResult};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Metadata for a knowledge collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeCollection {
    /// Unique collection ID.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Associated project ID.
    pub project_id: String,
    /// Description of what this collection contains.
    pub description: String,
    /// Number of chunks indexed.
    pub chunk_count: i64,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last-update timestamp.
    pub updated_at: String,
}

/// Query result from the RAG pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQueryResult {
    /// Matched search results.
    pub results: Vec<SearchResult>,
    /// Total number of chunks searched.
    pub total_searched: usize,
    /// Collection queried.
    pub collection_name: String,
}

// ---------------------------------------------------------------------------
// RagPipeline
// ---------------------------------------------------------------------------

/// Core RAG pipeline composing Chunker + EmbeddingManager + HnswIndex + optional Reranker.
pub struct RagPipeline {
    chunker: Arc<dyn Chunker>,
    embedding_manager: Arc<EmbeddingManager>,
    hnsw_index: Arc<HnswIndex>,
    reranker: Option<Arc<dyn Reranker>>,
    database: Arc<Database>,
}

impl RagPipeline {
    /// Create a new RagPipeline.
    pub fn new(
        chunker: Arc<dyn Chunker>,
        embedding_manager: Arc<EmbeddingManager>,
        hnsw_index: Arc<HnswIndex>,
        reranker: Option<Arc<dyn Reranker>>,
        database: Arc<Database>,
    ) -> AppResult<Self> {
        let pipeline = Self {
            chunker,
            embedding_manager,
            hnsw_index,
            reranker,
            database,
        };
        pipeline.init_schema()?;
        Ok(pipeline)
    }

    /// Initialize knowledge tables in SQLite.
    fn init_schema(&self) -> AppResult<()> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS knowledge_collections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                project_id TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                chunk_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(name, project_id)
            )",
            [],
        )
        .map_err(|e| AppError::database(format!("Failed to create knowledge_collections: {}", e)))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS knowledge_chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                collection_id TEXT NOT NULL,
                document_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                embedding BLOB,
                metadata TEXT DEFAULT '{}',
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (collection_id) REFERENCES knowledge_collections(id) ON DELETE CASCADE
            )",
            [],
        )
        .map_err(|e| AppError::database(format!("Failed to create knowledge_chunks: {}", e)))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_knowledge_chunks_collection ON knowledge_chunks(collection_id)",
            [],
        )
        .map_err(|e| AppError::database(format!("Failed to create index: {}", e)))?;

        Ok(())
    }

    /// Get or create a collection by name for a project.
    fn get_or_create_collection(
        &self,
        name: &str,
        project_id: &str,
        description: &str,
    ) -> AppResult<String> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Check if collection exists
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM knowledge_collections WHERE name = ?1 AND project_id = ?2",
                rusqlite::params![name, project_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            return Ok(id);
        }

        // Create new collection
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO knowledge_collections (id, name, project_id, description)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, name, project_id, description],
        )
        .map_err(|e| AppError::database(format!("Failed to create collection: {}", e)))?;

        Ok(id)
    }

    /// Ingest documents into a collection: chunk, embed, and store.
    pub async fn ingest(
        &self,
        collection_name: &str,
        project_id: &str,
        description: &str,
        documents: Vec<Document>,
    ) -> AppResult<KnowledgeCollection> {
        let collection_id = self.get_or_create_collection(collection_name, project_id, description)?;

        // Chunk all documents
        let mut all_chunks: Vec<Chunk> = Vec::new();
        for doc in &documents {
            let chunks = self.chunker.chunk(doc)?;
            all_chunks.extend(chunks);
        }

        if all_chunks.is_empty() {
            return self.get_collection(&collection_id);
        }

        // Embed all chunks
        let chunk_texts: Vec<&str> = all_chunks.iter().map(|c| c.content.as_str()).collect();
        let embeddings = self
            .embedding_manager
            .embed_documents(&chunk_texts)
            .await
            .map_err(|e| AppError::internal(format!("Embedding failed: {}", e)))?;

        // Store chunks in SQLite and insert into HNSW
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut hnsw_items: Vec<(usize, Vec<f32>)> = Vec::new();

        for (i, chunk) in all_chunks.iter().enumerate() {
            let embedding_bytes = embedding_to_bytes(&embeddings[i]);
            let metadata_json =
                serde_json::to_string(&chunk.metadata).unwrap_or_else(|_| "{}".to_string());

            // Insert chunk into SQLite
            conn.execute(
                "INSERT INTO knowledge_chunks (collection_id, document_id, chunk_index, content, embedding, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    collection_id,
                    chunk.document_id,
                    chunk.index as i64,
                    chunk.content,
                    embedding_bytes,
                    metadata_json,
                ],
            )
            .map_err(|e| AppError::database(format!("Failed to insert chunk: {}", e)))?;

            // Get the auto-generated SQLite rowid for HNSW
            let chunk_rowid = conn.last_insert_rowid() as usize;
            hnsw_items.push((chunk_rowid, embeddings[i].clone()));
        }

        // Initialize HNSW if needed
        if !self.hnsw_index.is_ready().await {
            self.hnsw_index.initialize().await;
        }

        // Batch insert into HNSW
        self.hnsw_index.batch_insert(&hnsw_items).await;

        // Update chunk count
        let chunk_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_chunks WHERE collection_id = ?1",
                rusqlite::params![collection_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        conn.execute(
            "UPDATE knowledge_collections SET chunk_count = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![chunk_count, collection_id],
        )
        .map_err(|e| AppError::database(format!("Failed to update collection: {}", e)))?;

        self.get_collection(&collection_id)
    }

    /// Query a collection with a natural language query.
    pub async fn query(
        &self,
        collection_name: &str,
        project_id: &str,
        query_text: &str,
        top_k: usize,
    ) -> AppResult<RagQueryResult> {
        // Find collection
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let collection_id: String = conn
            .query_row(
                "SELECT id FROM knowledge_collections WHERE name = ?1 AND project_id = ?2",
                rusqlite::params![collection_name, project_id],
                |row| row.get(0),
            )
            .map_err(|_| {
                AppError::not_found(format!(
                    "Collection '{}' not found for project '{}'",
                    collection_name, project_id
                ))
            })?;

        // Embed the query
        let query_embedding = self
            .embedding_manager
            .embed_query(query_text)
            .await
            .map_err(|e| AppError::internal(format!("Query embedding failed: {}", e)))?;

        // Search HNSW
        let hnsw_results = self.hnsw_index.search(&query_embedding, top_k * 3).await;

        // Filter results to only include chunks from this collection
        let mut search_results = Vec::new();
        for (chunk_rowid, distance) in &hnsw_results {
            // Look up chunk by rowid, filter by collection_id
            let chunk_data: Option<(String, String, String, Vec<u8>)> = conn
                .query_row(
                    "SELECT document_id, content, metadata, COALESCE(embedding, X'')
                     FROM knowledge_chunks
                     WHERE rowid = ?1 AND collection_id = ?2",
                    rusqlite::params![*chunk_rowid as i64, collection_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                        ))
                    },
                )
                .ok();

            if let Some((document_id, content, metadata_json, _emb_bytes)) = chunk_data {
                let metadata: HashMap<String, String> =
                    serde_json::from_str(&metadata_json).unwrap_or_default();

                // Convert distance to similarity score (cosine distance to similarity)
                let score = 1.0 - distance;

                search_results.push(SearchResult {
                    chunk_text: content,
                    document_id,
                    collection_name: collection_name.to_string(),
                    score,
                    metadata,
                });
            }
        }

        let total_searched = search_results.len();

        // Apply reranker if configured
        if let Some(ref reranker) = self.reranker {
            search_results = reranker.rerank(query_text, search_results).await?;
        }

        // Truncate to top_k
        search_results.truncate(top_k);

        Ok(RagQueryResult {
            results: search_results,
            total_searched,
            collection_name: collection_name.to_string(),
        })
    }

    /// List all collections for a project.
    pub fn list_collections(&self, project_id: &str) -> AppResult<Vec<KnowledgeCollection>> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, project_id, description, chunk_count, created_at, updated_at
                 FROM knowledge_collections WHERE project_id = ?1 ORDER BY name",
            )
            .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt
            .query_map(rusqlite::params![project_id], |row| {
                Ok(KnowledgeCollection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    project_id: row.get(2)?,
                    description: row.get(3)?,
                    chunk_count: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|e| AppError::database(format!("Failed to query collections: {}", e)))?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Delete a collection and all its chunks.
    pub async fn delete_collection(
        &self,
        collection_name: &str,
        project_id: &str,
    ) -> AppResult<()> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Find collection
        let collection_id: String = conn
            .query_row(
                "SELECT id FROM knowledge_collections WHERE name = ?1 AND project_id = ?2",
                rusqlite::params![collection_name, project_id],
                |row| row.get(0),
            )
            .map_err(|_| {
                AppError::not_found(format!(
                    "Collection '{}' not found",
                    collection_name
                ))
            })?;

        // Get all chunk rowids to mark stale in HNSW
        let mut stmt = conn
            .prepare("SELECT rowid FROM knowledge_chunks WHERE collection_id = ?1")
            .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

        let rowids: Vec<i64> = stmt
            .query_map(rusqlite::params![collection_id], |row| row.get(0))
            .map_err(|e| AppError::database(format!("Failed to query chunk rowids: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        // Mark stale in HNSW
        for rowid in rowids {
            self.hnsw_index.mark_stale(rowid as usize).await;
        }

        // Delete from SQLite (cascade handles chunks)
        conn.execute(
            "DELETE FROM knowledge_chunks WHERE collection_id = ?1",
            rusqlite::params![collection_id],
        )
        .map_err(|e| AppError::database(format!("Failed to delete chunks: {}", e)))?;

        conn.execute(
            "DELETE FROM knowledge_collections WHERE id = ?1",
            rusqlite::params![collection_id],
        )
        .map_err(|e| AppError::database(format!("Failed to delete collection: {}", e)))?;

        Ok(())
    }

    /// Get a collection by ID.
    fn get_collection(&self, collection_id: &str) -> AppResult<KnowledgeCollection> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.query_row(
            "SELECT id, name, project_id, description, chunk_count, created_at, updated_at
             FROM knowledge_collections WHERE id = ?1",
            rusqlite::params![collection_id],
            |row| {
                Ok(KnowledgeCollection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    project_id: row.get(2)?,
                    description: row.get(3)?,
                    chunk_count: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .map_err(|e| AppError::database(format!("Collection not found: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::knowledge::chunker::ParagraphChunker;
    use crate::services::knowledge::reranker::NoopReranker;
    use crate::services::orchestrator::embedding_manager::EmbeddingManagerConfig;
    use crate::services::orchestrator::embedding_provider::{
        EmbeddingProviderConfig, EmbeddingProviderType,
    };
    use crate::storage::database::Database;
    use tempfile::tempdir;

    async fn create_test_pipeline() -> (RagPipeline, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        // Use file-based SQLite to avoid pool size=1 issues with in-memory DB
        let db_path = dir.path().join("test.db");
        let manager = r2d2_sqlite::SqliteConnectionManager::file(&db_path);
        let pool = r2d2::Pool::builder()
            .max_size(5)
            .build(manager)
            .expect("pool");
        // Initialize schema using raw pool
        {
            let conn = pool.get().expect("conn");
            // Create minimal required tables
            conn.execute("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP)", []).unwrap();
        }
        let db = Arc::new(Database::from_pool_for_test(pool));

        // Create EmbeddingManager with TfIdf provider
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let embedding_manager = Arc::new(
            EmbeddingManager::from_config(config).expect("create embedding manager"),
        );

        let hnsw_index = Arc::new(HnswIndex::new(dir.path().join("hnsw"), 8192));

        let chunker: Arc<dyn Chunker> = Arc::new(ParagraphChunker::new(500));
        let reranker: Option<Arc<dyn Reranker>> = Some(Arc::new(NoopReranker));

        let pipeline = RagPipeline::new(chunker, embedding_manager, hnsw_index, reranker, db)
            .expect("create pipeline");

        (pipeline, dir)
    }

    // ======================================================================
    // Collection management tests
    // ======================================================================

    #[tokio::test]
    async fn create_and_list_collections() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let docs = vec![Document::new("d1", "Hello world, this is a test document.")];
        pipeline
            .ingest("test-collection", "proj-1", "Test collection", docs)
            .await
            .unwrap();

        let collections = pipeline.list_collections("proj-1").unwrap();
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].name, "test-collection");
        assert!(collections[0].chunk_count > 0);
    }

    #[tokio::test]
    async fn list_collections_empty() {
        let (pipeline, _dir) = create_test_pipeline().await;
        let collections = pipeline.list_collections("nonexistent").unwrap();
        assert!(collections.is_empty());
    }

    // ======================================================================
    // Ingest tests
    // ======================================================================

    #[tokio::test]
    async fn ingest_documents_creates_chunks() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let docs = vec![
            Document::new("d1", "First document about Rust programming.\n\nIt has multiple paragraphs."),
            Document::new("d2", "Second document about Python programming.\n\nAlso multiple paragraphs."),
        ];

        let collection = pipeline
            .ingest("test-col", "proj-1", "Test", docs)
            .await
            .unwrap();

        assert_eq!(collection.name, "test-col");
        assert!(collection.chunk_count >= 2, "Should have at least 2 chunks from 2 docs");
    }

    #[tokio::test]
    async fn ingest_same_collection_twice_appends() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let docs1 = vec![Document::new("d1", "Document one content here.")];
        pipeline
            .ingest("col", "proj-1", "Test", docs1)
            .await
            .unwrap();

        let docs2 = vec![Document::new("d2", "Document two content here.")];
        let collection = pipeline
            .ingest("col", "proj-1", "Test", docs2)
            .await
            .unwrap();

        assert!(collection.chunk_count >= 2, "Should have chunks from both ingests");
    }

    // ======================================================================
    // Query tests
    // ======================================================================

    #[tokio::test]
    async fn query_returns_results() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let docs = vec![
            Document::new("d1", "Rust programming language provides memory safety without garbage collection."),
            Document::new("d2", "Python is a popular language for data science and machine learning."),
            Document::new("d3", "JavaScript runs in web browsers and Node.js environments."),
            Document::new("d4", "Rust ownership system prevents data races at compile time."),
            Document::new("d5", "Go language was created at Google for concurrent programming."),
        ];

        pipeline
            .ingest("docs", "proj-1", "Programming docs", docs)
            .await
            .unwrap();

        let result = pipeline
            .query("docs", "proj-1", "Rust programming", 3)
            .await
            .unwrap();

        assert!(!result.results.is_empty(), "Should return some results");
        assert!(result.results.len() <= 3, "Should return at most top_k results");
    }

    #[tokio::test]
    async fn query_nonexistent_collection_errors() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let result = pipeline
            .query("nonexistent", "proj-1", "query", 5)
            .await;

        assert!(result.is_err());
    }

    // ======================================================================
    // Delete collection tests
    // ======================================================================

    #[tokio::test]
    async fn delete_collection_removes_data() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let docs = vec![Document::new("d1", "Test document for deletion.")];
        pipeline
            .ingest("to-delete", "proj-1", "Will be deleted", docs)
            .await
            .unwrap();

        let collections = pipeline.list_collections("proj-1").unwrap();
        assert_eq!(collections.len(), 1);

        pipeline
            .delete_collection("to-delete", "proj-1")
            .await
            .unwrap();

        let collections = pipeline.list_collections("proj-1").unwrap();
        assert!(collections.is_empty());
    }

    #[tokio::test]
    async fn delete_nonexistent_collection_errors() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let result = pipeline
            .delete_collection("nonexistent", "proj-1")
            .await;

        assert!(result.is_err());
    }

    // ======================================================================
    // Integration: ingest-then-query roundtrip
    // ======================================================================

    #[tokio::test]
    async fn ingest_then_query_roundtrip_with_5_documents() {
        let (pipeline, _dir) = create_test_pipeline().await;

        // Ingest 5 distinct documents
        let docs = vec![
            Document::from_parsed_content("doc-1", "The Rust programming language focuses on safety and performance. Ownership system prevents memory bugs.", "/docs/rust.md", "markdown"),
            Document::from_parsed_content("doc-2", "Python is widely used in data science. Libraries like pandas and numpy are essential.", "/docs/python.md", "markdown"),
            Document::from_parsed_content("doc-3", "Web development with JavaScript involves frameworks like React and Vue.", "/docs/webdev.md", "markdown"),
            Document::from_parsed_content("doc-4", "Database design requires understanding normalization and indexing strategies.", "/docs/database.md", "markdown"),
            Document::from_parsed_content("doc-5", "Machine learning models need training data and evaluation metrics.", "/docs/ml.md", "markdown"),
        ];

        let collection = pipeline
            .ingest("knowledge", "proj-test", "Test knowledge base", docs)
            .await
            .unwrap();

        assert!(collection.chunk_count >= 5, "At least 5 chunks from 5 docs, got {}", collection.chunk_count);

        // Query for Rust-related content
        let result = pipeline
            .query("knowledge", "proj-test", "Rust programming safety", 3)
            .await
            .unwrap();

        assert!(!result.results.is_empty(), "Query should return results");

        // Query for database-related content
        let result2 = pipeline
            .query("knowledge", "proj-test", "database indexing", 2)
            .await
            .unwrap();

        assert!(!result2.results.is_empty(), "Database query should return results");
    }

    // ======================================================================
    // KnowledgeCollection serialization
    // ======================================================================

    #[test]
    fn knowledge_collection_serde() {
        let col = KnowledgeCollection {
            id: "id-1".to_string(),
            name: "test".to_string(),
            project_id: "proj-1".to_string(),
            description: "desc".to_string(),
            chunk_count: 42,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&col).unwrap();
        let deserialized: KnowledgeCollection = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.chunk_count, 42);
    }
}
