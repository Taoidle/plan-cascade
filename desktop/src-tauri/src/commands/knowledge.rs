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
use crate::services::knowledge::pipeline::{
    DocumentSummary, KnowledgeCollection, RagPipeline, RagQueryResult,
};
use crate::services::knowledge::reranker::{NoopReranker, Reranker, SearchResult};
use crate::services::orchestrator::embedding_config_builder;
use crate::services::orchestrator::embedding_manager::{EmbeddingManager, EmbeddingManagerConfig};
use crate::services::orchestrator::embedding_provider::{
    EmbeddingProviderConfig, EmbeddingProviderType,
};
use crate::services::orchestrator::embedding_provider_tfidf::TfIdfEmbeddingProvider;
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

    /// Initialize the knowledge pipeline with a pre-built embedding config.
    ///
    /// Uses the provided `EmbeddingManagerConfig` (typically from
    /// `embedding_config_builder::build_embedding_config_from_settings`).
    /// When using TF-IDF, attempts to restore persisted vocabulary from disk.
    ///
    /// Subsequent calls are no-ops if already initialized.
    pub async fn initialize_with_config(
        &self,
        database: Arc<Database>,
        emb_config: EmbeddingManagerConfig,
        is_tfidf: bool,
    ) -> AppResult<()> {
        let mut guard = self.pipeline.write().await;
        if guard.is_some() {
            return Ok(());
        }

        let chunker: Arc<dyn Chunker> = Arc::new(ParagraphChunker::new(1000));

        let embedding_manager =
            Arc::new(EmbeddingManager::from_config(emb_config).map_err(|e| {
                AppError::internal(format!("Failed to create EmbeddingManager: {}", e))
            })?);

        // Use the actual provider dimension for the HNSW index.
        // TF-IDF has dynamic dimension (0 until vocabulary is built), so we
        // default to 0 and let HNSW accept the first insert's dimension.
        let emb_dim = embedding_manager.dimension();
        let hnsw_dir = dirs::home_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join(".plan-cascade")
            .join("knowledge-hnsw");
        let hnsw_index = Arc::new(HnswIndex::new(hnsw_dir, emb_dim));

        let reranker: Option<Arc<dyn Reranker>> = Some(Arc::new(NoopReranker));

        let pipeline = RagPipeline::new(
            chunker,
            Arc::clone(&embedding_manager),
            hnsw_index,
            reranker,
            database,
        )?;

        // If using TF-IDF, try to restore persisted vocabulary from disk
        if is_tfidf {
            load_tfidf_vocab(&embedding_manager);
        }

        *guard = Some(Arc::new(pipeline));

        Ok(())
    }

    /// Initialize the knowledge pipeline from a Database instance.
    ///
    /// Convenience method that defaults to TF-IDF embeddings. Used by tests
    /// and callers that don't need custom embedding configuration.
    ///
    /// Subsequent calls are no-ops if already initialized.
    pub async fn initialize(&self, database: Arc<Database>) -> AppResult<()> {
        let emb_config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        self.initialize_with_config(database, emb_config, true).await
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
        guard.clone().ok_or_else(|| {
            AppError::internal("Knowledge pipeline not initialized. Call initialize() first.")
        })
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
///
/// For text files (.md, .txt), the content is sent as plain text in `content`.
/// For binary files (PDF, DOCX, XLSX), the raw file bytes are base64-encoded
/// and sent in `content_base64`. The backend decodes, writes to a temp file,
/// and uses the existing file parsers to extract text.
#[derive(Debug, Deserialize)]
pub struct DocumentInput {
    pub id: String,
    pub content: String,
    /// Base64-encoded binary content (for PDF/DOCX/XLSX files).
    pub content_base64: Option<String>,
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

// ---------------------------------------------------------------------------
// TF-IDF Vocabulary Persistence
// ---------------------------------------------------------------------------

use std::path::PathBuf;

/// Path to the persisted TF-IDF vocabulary file for the knowledge pipeline.
fn vocab_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join(".plan-cascade")
        .join("knowledge-tfidf-vocab.json")
}

/// Save the current TF-IDF vocabulary to disk (if the primary provider is TF-IDF).
fn save_tfidf_vocab(embedding_manager: &EmbeddingManager) {
    let provider = embedding_manager.primary_provider();
    if let Some(tfidf) = provider.as_any().downcast_ref::<TfIdfEmbeddingProvider>() {
        if let Some(json) = tfidf.export_vocabulary() {
            let path = vocab_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&path, &json) {
                Ok(()) => {
                    tracing::debug!("knowledge: saved TF-IDF vocabulary to {}", path.display());
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "knowledge: failed to save TF-IDF vocabulary"
                    );
                }
            }
        }
    }
}

/// Load persisted TF-IDF vocabulary from disk into the embedding manager.
fn load_tfidf_vocab(embedding_manager: &EmbeddingManager) {
    let path = vocab_path();
    if !path.exists() {
        return;
    }
    match std::fs::read_to_string(&path) {
        Ok(json) => {
            let provider = embedding_manager.primary_provider();
            if let Some(tfidf) = provider.as_any().downcast_ref::<TfIdfEmbeddingProvider>() {
                match tfidf.import_vocabulary(&json) {
                    Ok(()) => {
                        tracing::info!(
                            "knowledge: restored TF-IDF vocabulary from {}",
                            path.display()
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "knowledge: failed to import TF-IDF vocabulary"
                        );
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "knowledge: failed to read TF-IDF vocabulary file"
            );
        }
    }
}

/// Check if the pipeline's embedding manager uses TF-IDF as primary provider.
fn is_tfidf_pipeline(pipeline: &RagPipeline) -> bool {
    pipeline
        .embedding_manager()
        .primary_provider()
        .as_any()
        .downcast_ref::<TfIdfEmbeddingProvider>()
        .is_some()
}

// ---------------------------------------------------------------------------
// Pipeline initialization helper
// ---------------------------------------------------------------------------

/// Ensure the knowledge pipeline is initialized, using AppState's database.
///
/// This is the lazy initialization entry point. On first call, it clones the
/// Database from AppState, reads the keyring, and uses them to construct the
/// RagPipeline with the user's configured embedding provider.
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

    // Build embedding config using keyring for API key resolution.
    // We use `with_keyring` to access the keyring while the lock is held,
    // computing the config we need and then dropping the lock.
    let (emb_config, is_tfidf) = app_state
        .with_keyring(|keyring| {
            let (config, _dim, is_tfidf) =
                embedding_config_builder::build_embedding_config_from_settings(&db, keyring);
            Ok((config, is_tfidf))
        })
        .await
        .map_err(|e| format!("Failed to build embedding config: {}", e))?;

    knowledge_state
        .initialize_with_config(db, emb_config, is_tfidf)
        .await
        .map_err(|e| format!("Failed to initialize knowledge pipeline: {}", e))?;
    Ok(())
}

/// Ingest documents into a knowledge collection.
#[tauri::command]
pub async fn rag_ingest_documents(
    app_handle: tauri::AppHandle,
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

    match pipeline
        .ingest_with_progress(&collection_name, &project_id, desc, docs, Some(&app_handle))
        .await
    {
        Ok(collection) => {
            // Persist TF-IDF vocabulary after successful ingest
            if is_tfidf_pipeline(&pipeline) {
                save_tfidf_vocab(pipeline.embedding_manager());
            }
            Ok(CommandResponse::ok(collection))
        }
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

    match pipeline
        .query(&collection_name, &project_id, &query, k)
        .await
    {
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

    match pipeline
        .delete_collection(&collection_name, &project_id)
        .await
    {
        Ok(()) => Ok(CommandResponse::ok(true)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List documents in a knowledge collection.
#[tauri::command]
pub async fn rag_list_documents(
    knowledge_state: State<'_, KnowledgeState>,
    app_state: State<'_, crate::state::AppState>,
    collection_id: String,
) -> Result<CommandResponse<Vec<DocumentSummary>>, String> {
    ensure_initialized(&knowledge_state, &app_state).await?;

    let pipeline = match knowledge_state.get_pipeline().await {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match pipeline.list_documents(&collection_id) {
        Ok(documents) => Ok(CommandResponse::ok(documents)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update a knowledge collection's metadata.
#[tauri::command]
pub async fn rag_update_collection(
    knowledge_state: State<'_, KnowledgeState>,
    app_state: State<'_, crate::state::AppState>,
    collection_id: String,
    name: Option<String>,
    description: Option<String>,
    workspace_path: Option<Option<String>>,
) -> Result<CommandResponse<KnowledgeCollection>, String> {
    ensure_initialized(&knowledge_state, &app_state).await?;

    let pipeline = match knowledge_state.get_pipeline().await {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match pipeline.update_collection(
        &collection_id,
        name.as_deref(),
        description.as_deref(),
        workspace_path
            .as_ref()
            .map(|wp| wp.as_deref()),
    ) {
        Ok(collection) => Ok(CommandResponse::ok(collection)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete a single document from a knowledge collection.
#[tauri::command]
pub async fn rag_delete_document(
    knowledge_state: State<'_, KnowledgeState>,
    app_state: State<'_, crate::state::AppState>,
    collection_id: String,
    document_id: String,
) -> Result<CommandResponse<bool>, String> {
    ensure_initialized(&knowledge_state, &app_state).await?;

    let pipeline = match knowledge_state.get_pipeline().await {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match pipeline.delete_document(&collection_id, &document_id).await {
        Ok(()) => Ok(CommandResponse::ok(true)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Helper: convert a DocumentInput to a chunker::Document.
impl DocumentInput {
    /// Convert frontend DocumentInput to the internal chunker Document type.
    ///
    /// For binary files (PDF/DOCX/XLSX), decodes base64 content, writes to a
    /// temp file, and uses file parsers to extract text. Falls back to the
    /// `content` field if base64 decoding or parsing fails.
    pub fn into_document(self) -> Document {
        // Try binary path if content_base64 is provided
        if let Some(ref b64) = self.content_base64 {
            if let Some(ref source_type) = self.source_type {
                match Self::parse_binary_content(b64, source_type) {
                    Ok(parsed_text) => {
                        let sp = self.source_path.unwrap_or_default();
                        return Document::from_parsed_content(
                            self.id,
                            parsed_text,
                            sp,
                            source_type.clone(),
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            id = %self.id,
                            source_type = %source_type,
                            "Failed to parse binary content, falling back to text: {}",
                            e
                        );
                    }
                }
            }
        }

        // Text path (default)
        if let (Some(source_path), Some(source_type)) = (self.source_path, self.source_type) {
            Document::from_parsed_content(self.id, self.content, source_path, source_type)
        } else {
            Document::new(self.id, self.content)
        }
    }

    /// Decode base64 binary content, write to a temp file, and parse using
    /// the appropriate file parser based on source_type.
    fn parse_binary_content(b64: &str, source_type: &str) -> Result<String, String> {
        use base64::Engine;
        use std::io::Write;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| format!("Base64 decode failed: {}", e))?;

        // Write to temp file with appropriate extension
        let ext = match source_type {
            "pdf" => ".pdf",
            "docx" => ".docx",
            "xlsx" => ".xlsx",
            _ => return Err(format!("Unsupported binary source type: {}", source_type)),
        };

        let mut tmp = tempfile::Builder::new()
            .suffix(ext)
            .tempfile()
            .map_err(|e| format!("Failed to create temp file: {}", e))?;

        tmp.write_all(&bytes)
            .map_err(|e| format!("Failed to write temp file: {}", e))?;
        tmp.flush()
            .map_err(|e| format!("Failed to flush temp file: {}", e))?;

        let tmp_path = tmp.path().to_path_buf();

        let parsed = match source_type {
            "pdf" => {
                crate::services::tools::file_parsers::parse_pdf(&tmp_path, None)
                    .map_err(|e| format!("PDF parse failed: {}", e))?
            }
            "docx" => {
                crate::services::tools::file_parsers::parse_docx(&tmp_path)
                    .map_err(|e| format!("DOCX parse failed: {}", e))?
            }
            "xlsx" => {
                crate::services::tools::file_parsers::parse_xlsx(&tmp_path)
                    .map_err(|e| format!("XLSX parse failed: {}", e))?
            }
            _ => return Err(format!("Unsupported binary source type: {}", source_type)),
        };

        // tmp file is automatically deleted when `tmp` drops
        Ok(parsed)
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
            content_base64: None,
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
            content_base64: None,
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
        assert!(
            collections.is_empty(),
            "New pipeline should have no collections"
        );
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
                content_base64: None,
                source_path: Some("/docs/rust.md".to_string()),
                source_type: Some("markdown".to_string()),
            },
            DocumentInput {
                id: "doc-2".to_string(),
                content: "Python is great for scripting.".to_string(),
                content_base64: None,
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

        assert!(
            chunks.is_empty(),
            "Disabled config should return no context"
        );
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

    // ======================================================================
    // P1-1: Binary document input tests
    // ======================================================================

    #[test]
    fn document_input_base64_fallback_on_invalid() {
        // Invalid base64 should fall back to text content
        let input = DocumentInput {
            id: "doc-b64-bad".to_string(),
            content: "fallback text content".to_string(),
            content_base64: Some("not-valid-base64!!!".to_string()),
            source_path: Some("test.pdf".to_string()),
            source_type: Some("pdf".to_string()),
        };
        let doc = input.into_document();
        assert_eq!(doc.id, "doc-b64-bad");
        // Should have fallen back to text content since base64 is invalid
        assert_eq!(doc.content, "fallback text content");
    }

    #[test]
    fn document_input_no_base64_uses_text_content() {
        let input = DocumentInput {
            id: "doc-text".to_string(),
            content: "plain text content".to_string(),
            content_base64: None,
            source_path: Some("readme.md".to_string()),
            source_type: Some("md".to_string()),
        };
        let doc = input.into_document();
        assert_eq!(doc.content, "plain text content");
        assert_eq!(doc.metadata.get("source_type"), Some(&"md".to_string()));
    }

    #[test]
    fn document_input_unsupported_binary_type_falls_back() {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"some binary data");
        let input = DocumentInput {
            id: "doc-unsupported".to_string(),
            content: "fallback".to_string(),
            content_base64: Some(b64),
            source_path: Some("file.unknown".to_string()),
            source_type: Some("unknown".to_string()),
        };
        let doc = input.into_document();
        // Unsupported binary type should fall back to text content
        assert_eq!(doc.content, "fallback");
    }
}
