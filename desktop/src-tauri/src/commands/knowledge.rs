//! Knowledge Base Commands
//!
//! Tauri commands for RAG pipeline operations: document ingestion,
//! querying, and collection management.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::services::knowledge::chunker::{Chunker, Document, ParagraphChunker};
use crate::services::knowledge::pipeline::{KnowledgeCollection, RagPipeline, RagQueryResult};
use crate::services::knowledge::reranker::{NoopReranker, Reranker, SearchResult};
use crate::services::orchestrator::embedding_manager::{EmbeddingManager, EmbeddingManagerConfig};
use crate::services::orchestrator::embedding_provider::{
    EmbeddingProviderConfig, EmbeddingProviderType,
};
use crate::services::orchestrator::hnsw_index::HnswIndex;
use crate::storage::Database;
use crate::utils::error::{AppError, AppResult};

/// Tauri-managed state for the knowledge pipeline.
///
/// Uses the lazy initialization pattern (`Arc<RwLock<Option<T>>>`) consistent
/// with the AppState pattern. The pipeline is constructed on first use from the
/// application's Database instance.
pub struct KnowledgeState {
    /// Lazily-initialized RAG pipeline.
    pipeline: Arc<RwLock<Option<Arc<RagPipeline>>>>,
}

impl KnowledgeState {
    /// Create a new uninitialized KnowledgeState.
    pub fn new() -> Self {
        Self {
            pipeline: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the knowledge pipeline from a Database instance.
    ///
    /// Uses ParagraphChunker (default 1000 chars), TfIdf embeddings,
    /// HNSW index, and NoopReranker as the default configuration.
    /// Subsequent calls are no-ops if already initialized.
    pub async fn initialize(&self, database: Arc<Database>) -> AppResult<()> {
        let mut guard = self.pipeline.write().await;
        if guard.is_some() {
            return Ok(());
        }

        let chunker: Arc<dyn Chunker> = Arc::new(ParagraphChunker::new(1000));

        let emb_config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let embedding_manager = Arc::new(
            EmbeddingManager::from_config(emb_config)
                .map_err(|e| AppError::internal(format!("Failed to create EmbeddingManager: {}", e)))?,
        );

        // Store HNSW index under ~/.plan-cascade/knowledge-hnsw
        let hnsw_dir = dirs::home_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join(".plan-cascade")
            .join("knowledge-hnsw");
        let hnsw_index = Arc::new(HnswIndex::new(hnsw_dir, 8192));

        let reranker: Option<Arc<dyn Reranker>> = Some(Arc::new(NoopReranker));

        let pipeline = RagPipeline::new(chunker, embedding_manager, hnsw_index, reranker, database)?;
        *guard = Some(Arc::new(pipeline));

        Ok(())
    }

    /// Initialize the knowledge pipeline with an existing pipeline instance.
    ///
    /// This is useful for testing or when the pipeline is pre-constructed.
    pub async fn initialize_with_pipeline(&self, pipeline: Arc<RagPipeline>) {
        let mut guard = self.pipeline.write().await;
        *guard = Some(pipeline);
    }

    /// Get the initialized pipeline, or an error if not yet initialized.
    pub async fn get_pipeline(&self) -> AppResult<Arc<RagPipeline>> {
        let guard = self.pipeline.read().await;
        guard
            .clone()
            .ok_or_else(|| AppError::internal("Knowledge pipeline not initialized. Call initialize() first."))
    }

    /// Check whether the pipeline has been initialized.
    pub async fn is_initialized(&self) -> bool {
        let guard = self.pipeline.read().await;
        guard.is_some()
    }
}

impl Default for KnowledgeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Request for document ingestion.
#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub collection_name: String,
    pub project_id: String,
    pub description: Option<String>,
    pub documents: Vec<DocumentInput>,
}

/// Document input from frontend.
#[derive(Debug, Deserialize)]
pub struct DocumentInput {
    pub id: String,
    pub content: String,
    pub source_path: Option<String>,
    pub source_type: Option<String>,
}

/// Request for querying a collection.
#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub collection_name: String,
    pub project_id: String,
    pub query: String,
    pub top_k: Option<usize>,
}

/// Ensure the knowledge pipeline is initialized, using AppState's database.
///
/// This is the lazy initialization entry point. On first call, it clones the
/// Database from AppState and uses it to construct the RagPipeline.
async fn ensure_initialized(
    knowledge_state: &KnowledgeState,
    app_state: &crate::state::AppState,
) -> Result<(), String> {
    if knowledge_state.is_initialized().await {
        return Ok(());
    }
    let db = app_state
        .with_database(|db| Ok(Arc::new(db.clone())))
        .await
        .map_err(|e| format!("Failed to access database: {}", e))?;
    knowledge_state
        .initialize(db)
        .await
        .map_err(|e| format!("Failed to initialize knowledge pipeline: {}", e))?;
    Ok(())
}

/// Ingest documents into a knowledge collection.
#[tauri::command]
pub async fn rag_ingest_documents(
    knowledge_state: State<'_, KnowledgeState>,
    app_state: State<'_, crate::state::AppState>,
    collection_name: String,
    project_id: String,
    description: Option<String>,
    documents: Vec<DocumentInput>,
) -> Result<CommandResponse<KnowledgeCollection>, String> {
    ensure_initialized(&knowledge_state, &app_state).await?;

    let pipeline = match knowledge_state.get_pipeline().await {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // Convert DocumentInput to chunker::Document
    let docs: Vec<Document> = documents.into_iter().map(|d| d.into_document()).collect();

    let desc = description.as_deref().unwrap_or("");

    match pipeline.ingest(&collection_name, &project_id, desc, docs).await {
        Ok(collection) => Ok(CommandResponse::ok(collection)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Query a knowledge collection.
#[tauri::command]
pub async fn rag_query(
    knowledge_state: State<'_, KnowledgeState>,
    app_state: State<'_, crate::state::AppState>,
    collection_name: String,
    project_id: String,
    query: String,
    top_k: Option<usize>,
) -> Result<CommandResponse<RagQueryResult>, String> {
    ensure_initialized(&knowledge_state, &app_state).await?;

    let pipeline = match knowledge_state.get_pipeline().await {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let k = top_k.unwrap_or(5);

    match pipeline.query(&collection_name, &project_id, &query, k).await {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List all knowledge collections for a project.
#[tauri::command]
pub async fn rag_list_collections(
    knowledge_state: State<'_, KnowledgeState>,
    app_state: State<'_, crate::state::AppState>,
    project_id: String,
) -> Result<CommandResponse<Vec<KnowledgeCollection>>, String> {
    ensure_initialized(&knowledge_state, &app_state).await?;

    let pipeline = match knowledge_state.get_pipeline().await {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match pipeline.list_collections(&project_id) {
        Ok(collections) => Ok(CommandResponse::ok(collections)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete a knowledge collection.
#[tauri::command]
pub async fn rag_delete_collection(
    knowledge_state: State<'_, KnowledgeState>,
    app_state: State<'_, crate::state::AppState>,
    collection_name: String,
    project_id: String,
) -> Result<CommandResponse<bool>, String> {
    ensure_initialized(&knowledge_state, &app_state).await?;

    let pipeline = match knowledge_state.get_pipeline().await {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match pipeline.delete_collection(&collection_name, &project_id).await {
        Ok(()) => Ok(CommandResponse::ok(true)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Helper: convert a DocumentInput to a chunker::Document.
impl DocumentInput {
    /// Convert frontend DocumentInput to the internal chunker Document type.
    pub fn into_document(self) -> Document {
        if let (Some(source_path), Some(source_type)) = (self.source_path, self.source_type) {
            Document::from_parsed_content(self.id, self.content, source_path, source_type)
        } else {
            Document::new(self.id, self.content)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Helper: create a test database in a temporary directory.
    fn create_test_db(dir: &std::path::Path) -> Arc<Database> {
        let db_path = dir.join("test_knowledge_state.db");
        let manager = r2d2_sqlite::SqliteConnectionManager::file(&db_path);
        let pool = r2d2::Pool::builder()
            .max_size(5)
            .build(manager)
            .expect("pool");
        {
            let conn = pool.get().expect("conn");
            conn.execute(
                "CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP)",
                [],
            )
            .unwrap();
        }
        Arc::new(Database::from_pool_for_test(pool))
    }

    // ======================================================================
    // KnowledgeState lifecycle tests
    // ======================================================================

    #[tokio::test]
    async fn knowledge_state_new_is_uninitialized() {
        let state = KnowledgeState::new();
        assert!(!state.is_initialized().await);
    }

    #[tokio::test]
    async fn knowledge_state_default_is_uninitialized() {
        let state = KnowledgeState::default();
        assert!(!state.is_initialized().await);
    }

    #[tokio::test]
    async fn knowledge_state_get_pipeline_before_init_errors() {
        let state = KnowledgeState::new();
        let result = state.get_pipeline().await;
        assert!(result.is_err());
        let err_msg = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err_msg.contains("not initialized"));
    }

    #[tokio::test]
    async fn knowledge_state_initialize_succeeds() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());

        let state = KnowledgeState::new();
        let result = state.initialize(db).await;
        assert!(result.is_ok(), "initialize should succeed: {:?}", result);
        assert!(state.is_initialized().await);
    }

    #[tokio::test]
    async fn knowledge_state_get_pipeline_after_init_succeeds() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());

        let state = KnowledgeState::new();
        state.initialize(db).await.unwrap();

        let pipeline = state.get_pipeline().await;
        assert!(pipeline.is_ok(), "get_pipeline should succeed after init");
    }

    #[tokio::test]
    async fn knowledge_state_double_init_is_noop() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());

        let state = KnowledgeState::new();
        state.initialize(db.clone()).await.unwrap();
        // Second init should be a no-op
        let result = state.initialize(db).await;
        assert!(result.is_ok());
        assert!(state.is_initialized().await);
    }

    #[tokio::test]
    async fn knowledge_state_initialize_with_pipeline() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());

        let chunker: Arc<dyn Chunker> = Arc::new(ParagraphChunker::new(500));
        let emb_config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let emb = Arc::new(EmbeddingManager::from_config(emb_config).unwrap());
        let hnsw = Arc::new(HnswIndex::new(dir.path().join("hnsw"), 8192));
        let reranker: Option<Arc<dyn Reranker>> = Some(Arc::new(NoopReranker));
        let pipeline = Arc::new(RagPipeline::new(chunker, emb, hnsw, reranker, db).unwrap());

        let state = KnowledgeState::new();
        state.initialize_with_pipeline(pipeline).await;
        assert!(state.is_initialized().await);
        assert!(state.get_pipeline().await.is_ok());
    }

    // ======================================================================
    // DocumentInput conversion tests
    // ======================================================================

    #[test]
    fn document_input_into_document_minimal() {
        let input = DocumentInput {
            id: "doc-1".to_string(),
            content: "Hello world".to_string(),
            source_path: None,
            source_type: None,
        };
        let doc = input.into_document();
        assert_eq!(doc.id, "doc-1");
        assert_eq!(doc.content, "Hello world");
        assert!(doc.source_path.is_none());
    }

    #[test]
    fn document_input_into_document_with_source() {
        let input = DocumentInput {
            id: "doc-2".to_string(),
            content: "Rust code".to_string(),
            source_path: Some("/src/main.rs".to_string()),
            source_type: Some("rust".to_string()),
        };
        let doc = input.into_document();
        assert_eq!(doc.id, "doc-2");
        assert_eq!(doc.source_path, Some("/src/main.rs".to_string()));
        assert_eq!(doc.metadata.get("source_type"), Some(&"rust".to_string()));
    }

    // ======================================================================
    // End-to-end: init + pipeline operations
    // ======================================================================

    #[tokio::test]
    async fn knowledge_state_pipeline_can_list_collections() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());

        let state = KnowledgeState::new();
        state.initialize(db).await.unwrap();

        let pipeline = state.get_pipeline().await.unwrap();
        let collections = pipeline.list_collections("test-project").unwrap();
        assert!(collections.is_empty(), "New pipeline should have no collections");
    }

    // ======================================================================
    // Story-002: CRUD command wiring tests (via pipeline directly)
    // ======================================================================

    /// Helper: create a KnowledgeState with an initialized pipeline.
    async fn create_initialized_state() -> (KnowledgeState, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());

        let chunker: Arc<dyn Chunker> = Arc::new(ParagraphChunker::new(500));
        let emb_config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let emb = Arc::new(EmbeddingManager::from_config(emb_config).unwrap());
        let hnsw = Arc::new(HnswIndex::new(dir.path().join("hnsw"), 8192));
        let reranker: Option<Arc<dyn Reranker>> = Some(Arc::new(NoopReranker));
        let pipeline = Arc::new(RagPipeline::new(chunker, emb, hnsw, reranker, db).unwrap());

        let state = KnowledgeState::new();
        state.initialize_with_pipeline(pipeline).await;
        (state, dir)
    }

    #[tokio::test]
    async fn crud_ingest_converts_and_stores_documents() {
        let (state, _dir) = create_initialized_state().await;
        let pipeline = state.get_pipeline().await.unwrap();

        let doc_inputs = vec![
            DocumentInput {
                id: "doc-1".to_string(),
                content: "Rust programming is fast and safe.".to_string(),
                source_path: Some("/docs/rust.md".to_string()),
                source_type: Some("markdown".to_string()),
            },
            DocumentInput {
                id: "doc-2".to_string(),
                content: "Python is great for scripting.".to_string(),
                source_path: None,
                source_type: None,
            },
        ];

        // Convert and ingest through the same path the command would use
        let docs: Vec<Document> = doc_inputs.into_iter().map(|d| d.into_document()).collect();
        let collection = pipeline
            .ingest("test-col", "proj-1", "Test", docs)
            .await
            .unwrap();

        assert_eq!(collection.name, "test-col");
        assert!(collection.chunk_count >= 2);
    }

    #[tokio::test]
    async fn crud_query_returns_results_after_ingest() {
        let (state, _dir) = create_initialized_state().await;
        let pipeline = state.get_pipeline().await.unwrap();

        let docs = vec![
            Document::new("d1", "Rust provides memory safety through ownership."),
            Document::new("d2", "Python is popular for data science."),
            Document::new("d3", "JavaScript powers the web."),
        ];
        pipeline
            .ingest("docs", "proj-1", "Docs", docs)
            .await
            .unwrap();

        let result = pipeline.query("docs", "proj-1", "Rust memory", 3).await.unwrap();
        assert!(!result.results.is_empty(), "Query should return results");
        assert_eq!(result.collection_name, "docs");
    }

    #[tokio::test]
    async fn crud_list_collections_returns_created() {
        let (state, _dir) = create_initialized_state().await;
        let pipeline = state.get_pipeline().await.unwrap();

        // Initially empty
        let collections = pipeline.list_collections("proj-1").unwrap();
        assert!(collections.is_empty());

        // After ingest
        let docs = vec![Document::new("d1", "Content here.")];
        pipeline
            .ingest("my-collection", "proj-1", "Desc", docs)
            .await
            .unwrap();

        let collections = pipeline.list_collections("proj-1").unwrap();
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].name, "my-collection");
        assert_eq!(collections[0].project_id, "proj-1");
    }

    #[tokio::test]
    async fn crud_delete_collection_removes_it() {
        let (state, _dir) = create_initialized_state().await;
        let pipeline = state.get_pipeline().await.unwrap();

        let docs = vec![Document::new("d1", "Some content.")];
        pipeline
            .ingest("to-delete", "proj-1", "Will be deleted", docs)
            .await
            .unwrap();

        assert_eq!(pipeline.list_collections("proj-1").unwrap().len(), 1);

        pipeline
            .delete_collection("to-delete", "proj-1")
            .await
            .unwrap();

        assert!(pipeline.list_collections("proj-1").unwrap().is_empty());
    }

    #[tokio::test]
    async fn crud_query_nonexistent_collection_errors() {
        let (state, _dir) = create_initialized_state().await;
        let pipeline = state.get_pipeline().await.unwrap();

        let result = pipeline.query("nonexistent", "proj-1", "query", 5).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn crud_delete_nonexistent_collection_errors() {
        let (state, _dir) = create_initialized_state().await;
        let pipeline = state.get_pipeline().await.unwrap();

        let result = pipeline.delete_collection("nonexistent", "proj-1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn crud_ingest_query_roundtrip() {
        let (state, _dir) = create_initialized_state().await;
        let pipeline = state.get_pipeline().await.unwrap();

        // Ingest varied documents
        let doc_inputs = vec![
            DocumentInput {
                id: "api-doc".to_string(),
                content: "REST API endpoints for user management with CRUD operations.".to_string(),
                source_path: Some("/docs/api.md".to_string()),
                source_type: Some("markdown".to_string()),
            },
            DocumentInput {
                id: "db-doc".to_string(),
                content: "Database schema uses PostgreSQL with normalized tables.".to_string(),
                source_path: Some("/docs/db.md".to_string()),
                source_type: Some("markdown".to_string()),
            },
        ];

        let docs: Vec<Document> = doc_inputs.into_iter().map(|d| d.into_document()).collect();
        let collection = pipeline.ingest("knowledge", "proj-2", "Project docs", docs).await.unwrap();
        assert!(collection.chunk_count >= 2);

        // Query
        let result = pipeline.query("knowledge", "proj-2", "REST API user", 2).await.unwrap();
        assert!(!result.results.is_empty());

        // List collections shows it
        let colls = pipeline.list_collections("proj-2").unwrap();
        assert_eq!(colls.len(), 1);
        assert_eq!(colls[0].name, "knowledge");

        // Delete
        pipeline.delete_collection("knowledge", "proj-2").await.unwrap();
        assert!(pipeline.list_collections("proj-2").unwrap().is_empty());
    }

    // ======================================================================
    // Story-003: KnowledgeContextProvider integration tests
    // ======================================================================

    #[tokio::test]
    async fn context_provider_from_knowledge_state() {
        use crate::services::knowledge::context_provider::{
            KnowledgeContextConfig, KnowledgeContextProvider,
        };

        let (state, _dir) = create_initialized_state().await;
        let pipeline = state.get_pipeline().await.unwrap();

        // Ingest documents
        let docs = vec![
            Document::new("d1", "Rust ownership prevents memory bugs."),
            Document::new("d2", "Python data science with pandas."),
        ];
        pipeline
            .ingest("kb", "proj-ctx", "Test KB", docs)
            .await
            .unwrap();

        // Create provider from the same pipeline
        let provider = KnowledgeContextProvider::new(pipeline);
        let config = KnowledgeContextConfig {
            enabled: true,
            max_context_chunks: 3,
            minimum_relevance_score: 0.0,
        };

        let chunks = provider
            .query_for_context("proj-ctx", "Rust ownership", &config)
            .await
            .unwrap();

        assert!(!chunks.is_empty(), "Should return relevant context");

        // Format as block for system prompt
        let block = KnowledgeContextProvider::format_context_block(&chunks);
        assert!(block.contains("Relevant Knowledge Context"));
        assert!(block.contains("Context 1"));
    }

    #[tokio::test]
    async fn context_provider_disabled_returns_empty() {
        use crate::services::knowledge::context_provider::{
            KnowledgeContextConfig, KnowledgeContextProvider,
        };

        let (state, _dir) = create_initialized_state().await;
        let pipeline = state.get_pipeline().await.unwrap();

        let docs = vec![Document::new("d1", "Some content here.")];
        pipeline
            .ingest("kb", "proj-disabled", "Test", docs)
            .await
            .unwrap();

        let provider = KnowledgeContextProvider::new(pipeline);
        let config = KnowledgeContextConfig {
            enabled: false,
            max_context_chunks: 5,
            minimum_relevance_score: 0.3,
        };

        let chunks = provider
            .query_for_context("proj-disabled", "anything", &config)
            .await
            .unwrap();

        assert!(chunks.is_empty(), "Disabled config should return no context");
    }

    #[tokio::test]
    async fn context_provider_no_collections_returns_empty() {
        use crate::services::knowledge::context_provider::{
            KnowledgeContextConfig, KnowledgeContextProvider,
        };

        let (state, _dir) = create_initialized_state().await;
        let pipeline = state.get_pipeline().await.unwrap();

        // No documents ingested
        let provider = KnowledgeContextProvider::new(pipeline);
        let config = KnowledgeContextConfig::default();

        let chunks = provider
            .query_for_context("empty-project", "query", &config)
            .await
            .unwrap();

        assert!(chunks.is_empty(), "No collections should return empty");
    }

    #[tokio::test]
    async fn context_provider_format_block_empty_chunks_returns_empty_string() {
        use crate::services::knowledge::context_provider::KnowledgeContextProvider;

        let block = KnowledgeContextProvider::format_context_block(&[]);
        assert!(block.is_empty());
    }
}
