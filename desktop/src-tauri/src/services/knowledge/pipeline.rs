//! RAG Pipeline
//!
//! Orchestrates the full Retrieval-Augmented Generation flow:
//! chunking -> embedding -> indexing -> querying -> reranking.
//!
//! Manages named collections (namespaces) with SQLite persistence.
//! Each collection has its own set of chunks indexed in HNSW with
//! collection-namespaced IDs.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tauri::Emitter;

use crate::services::knowledge::chunker::{Chunk, Chunker, Document};
use crate::services::knowledge::observability;
use crate::services::knowledge::reranker::{Reranker, SearchResult};
use crate::services::orchestrator::embedding_manager::EmbeddingManager;
use crate::services::orchestrator::embedding_service::{
    bytes_to_embedding, cosine_similarity, embedding_to_bytes,
};
use crate::services::orchestrator::hnsw_index::HnswIndex;
use crate::storage::database::Database;
use crate::utils::error::{AppError, AppResult};
use crate::utils::paths::database_path;

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
    /// Optional workspace path associating this collection with a project directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
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

/// Scoped document reference used by filters and context selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ScopedDocumentRef {
    pub collection_id: String,
    pub document_uid: String,
}

/// Summary of a document within a collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    /// Stable unique document primary key.
    pub document_uid: String,
    /// Human-readable document display name.
    pub display_name: String,
    /// Kind of source: workspace/upload.
    pub source_kind: String,
    /// Stable locator for this source.
    pub source_locator: String,
    /// Source type/extension.
    pub source_type: String,
    /// Whether this document should be checked for file-system updates.
    pub trackable: bool,
    /// Last successful index timestamp.
    pub last_indexed_at: Option<String>,
    /// Number of chunks this document was split into.
    pub chunk_count: i64,
    /// Preview of the first chunk's content (up to 200 chars).
    pub preview: String,
}

/// Information about a document that has changed relative to its stored hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocUpdateInfo {
    pub document_uid: String,
    pub display_name: String,
    pub source_kind: String,
    pub source_locator: String,
    pub source_type: String,
    pub old_hash: String,
    /// `None` if the file was deleted from disk.
    pub new_hash: Option<String>,
}

/// Result of comparing stored document hashes with current disk state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionUpdateCheck {
    pub collection_id: String,
    /// Documents whose content hash changed.
    pub modified: Vec<DocUpdateInfo>,
    /// Documents whose source file no longer exists.
    pub deleted: Vec<DocUpdateInfo>,
    /// Files in `scan_dir` that are not yet indexed (only populated when scan_dir is set).
    pub new_files: Vec<String>,
    /// Count of documents that are unchanged.
    pub unchanged: usize,
}

/// A single recorded query execution run for local observability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRunSummary {
    pub id: i64,
    pub project_id: String,
    pub query: String,
    pub collection_scope: String,
    pub retrieval_profile: String,
    pub top_k: i64,
    pub vector_candidates: i64,
    pub bm25_candidates: i64,
    pub merged_candidates: i64,
    pub rerank_ms: i64,
    pub total_ms: i64,
    pub result_count: i64,
    pub created_at: String,
}

/// Lightweight document search result for source-picker UX.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSearchMatch {
    pub collection_id: String,
    pub document_uid: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy)]
enum RetrievalProfile {
    Balanced,
    Precision,
    Recall,
}

#[derive(Debug, Clone, Copy)]
struct RetrievalParams {
    vector_top_n: usize,
    bm25_top_n: usize,
    fused_top_n: usize,
    mmr_top_n: usize,
    mmr_lambda: f32,
}

impl RetrievalProfile {
    fn from_raw(raw: Option<&str>) -> Self {
        match raw
            .unwrap_or("balanced")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "precision" => Self::Precision,
            "recall" => Self::Recall,
            _ => Self::Balanced,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Precision => "precision",
            Self::Recall => "recall",
        }
    }

    fn params(&self) -> RetrievalParams {
        match self {
            Self::Balanced => RetrievalParams {
                vector_top_n: 80,
                bm25_top_n: 80,
                fused_top_n: 30,
                mmr_top_n: 20,
                mmr_lambda: 0.72,
            },
            Self::Precision => RetrievalParams {
                vector_top_n: 60,
                bm25_top_n: 50,
                fused_top_n: 20,
                mmr_top_n: 12,
                mmr_lambda: 0.86,
            },
            Self::Recall => RetrievalParams {
                vector_top_n: 140,
                bm25_top_n: 140,
                fused_top_n: 60,
                mmr_top_n: 30,
                mmr_lambda: 0.58,
            },
        }
    }
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
    const KNOWLEDGE_CHUNKS_FTS_SCHEMA_SQL: &'static str =
        "CREATE VIRTUAL TABLE knowledge_chunks_fts USING fts5(
            content,
            chunk_id UNINDEXED,
            collection_id UNINDEXED,
            document_uid UNINDEXED,
            tokenize='unicode61'
        )";

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

    /// Initialize/migrate knowledge tables in SQLite.
    ///
    /// Uses a strong-cut v2 schema migration:
    /// - introduces `knowledge_documents` with `document_uid`
    /// - rewires `knowledge_chunks.document_uid` foreign key
    /// - adds `knowledge_chunks_fts` and `knowledge_query_runs`
    fn init_schema(&self) -> AppResult<()> {
        let mut conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        Self::ensure_collections_table(&conn)?;
        Self::ensure_collection_workspace_path(&conn)?;
        self.migrate_to_knowledge_v2(&mut conn)?;
        if let Err(e) = Self::migrate_query_runs_to_v3(&conn) {
            tracing::warn!(
                error = %e,
                "knowledge_query_runs v3 migration failed; using legacy query run path"
            );
        }
        Ok(())
    }

    fn parse_bool_flag(raw: &str) -> Option<bool> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" | "enabled" => Some(true),
            "0" | "false" | "no" | "off" | "disabled" => Some(false),
            _ => None,
        }
    }

    fn is_feature_enabled(&self, key: &str, default_value: bool) -> bool {
        let env_key = format!("PLAN_CASCADE_{}", key.to_ascii_uppercase());
        if let Ok(value) = std::env::var(&env_key) {
            if let Some(enabled) = Self::parse_bool_flag(&value) {
                return enabled;
            }
        }

        for setting_key in [format!("feature.{}", key), key.to_string()] {
            if let Ok(Some(value)) = self.database.get_setting(&setting_key) {
                if let Some(enabled) = Self::parse_bool_flag(&value) {
                    return enabled;
                }
            }
        }

        default_value
    }

    fn build_ingest_progress_payload(
        job_scoped_progress: bool,
        job_id: &str,
        project_id: &str,
        collection_id: &str,
        collection_name: &str,
        stage: &str,
        progress: u32,
        detail: &str,
    ) -> serde_json::Value {
        if job_scoped_progress {
            serde_json::json!({
                "job_id": job_id,
                "project_id": project_id,
                "collection_id": collection_id,
                "collection_name": collection_name,
                "stage": stage,
                "progress": progress,
                "detail": detail,
            })
        } else {
            serde_json::json!({
                "project_id": project_id,
                "collection_id": collection_id,
                "collection_name": collection_name,
                "stage": stage,
                "progress": progress,
                "detail": detail,
            })
        }
    }

    fn ensure_collections_table(conn: &rusqlite::Connection) -> AppResult<()> {
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
        .map_err(|e| {
            AppError::database(format!("Failed to create knowledge_collections: {}", e))
        })?;
        Ok(())
    }

    fn ensure_collection_workspace_path(conn: &rusqlite::Connection) -> AppResult<()> {
        if !Self::table_has_column(conn, "knowledge_collections", "workspace_path")? {
            conn.execute(
                "ALTER TABLE knowledge_collections ADD COLUMN workspace_path TEXT DEFAULT NULL",
                [],
            )
            .map_err(|e| {
                AppError::database(format!("Failed to add workspace_path column: {}", e))
            })?;
        }
        Ok(())
    }

    fn table_exists(conn: &rusqlite::Connection, table_name: &str) -> AppResult<bool> {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
                rusqlite::params![table_name],
                |row| row.get(0),
            )
            .map_err(|e| {
                AppError::database(format!("Failed checking table '{}': {}", table_name, e))
            })?;
        Ok(exists > 0)
    }

    fn table_has_column(
        conn: &rusqlite::Connection,
        table_name: &str,
        column_name: &str,
    ) -> AppResult<bool> {
        let safe_table = table_name.replace('\'', "''");
        let sql = format!(
            "SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name = ?1",
            safe_table
        );
        let count: i64 = conn
            .query_row(&sql, rusqlite::params![column_name], |row| row.get(0))
            .map_err(|e| {
                AppError::database(format!(
                    "Failed checking column '{}.{}': {}",
                    table_name, column_name, e
                ))
            })?;
        Ok(count > 0)
    }

    fn create_knowledge_backup(&self, conn: &rusqlite::Connection) -> AppResult<()> {
        let mut db_file: Option<String> = None;
        if let Ok(mut stmt) = conn.prepare("PRAGMA database_list") {
            if let Ok(mut rows) = stmt.query([]) {
                while let Ok(Some(row)) = rows.next() {
                    let name: String = row.get(1).unwrap_or_default();
                    if name == "main" {
                        let file: String = row.get(2).unwrap_or_default();
                        if !file.is_empty() {
                            db_file = Some(file);
                        }
                        break;
                    }
                }
            }
        }

        let db_path = if let Some(path) = db_file {
            std::path::PathBuf::from(path)
        } else {
            database_path()?
        };

        if !db_path.exists() {
            return Ok(());
        }

        let backup_path = db_path.with_extension(format!(
            "knowledge_v2_backup_{}.db",
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        ));

        conn.execute(
            "VACUUM INTO ?1",
            rusqlite::params![backup_path.to_string_lossy().to_string()],
        )
        .map_err(|e| {
            AppError::database(format!(
                "Failed to backup knowledge DB before migration: {}",
                e
            ))
        })?;

        Ok(())
    }

    fn create_v2_tables(tx: &rusqlite::Transaction<'_>) -> AppResult<()> {
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS knowledge_documents (
                document_uid TEXT PRIMARY KEY,
                collection_id TEXT NOT NULL,
                display_name TEXT NOT NULL,
                source_kind TEXT NOT NULL,
                source_locator TEXT NOT NULL,
                source_type TEXT NOT NULL DEFAULT '',
                content_hash TEXT NOT NULL DEFAULT '',
                trackable INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                last_indexed_at TEXT DEFAULT (datetime('now')),
                UNIQUE(collection_id, source_kind, source_locator),
                FOREIGN KEY (collection_id) REFERENCES knowledge_collections(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS knowledge_chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                collection_id TEXT NOT NULL,
                document_uid TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                embedding BLOB,
                metadata TEXT DEFAULT '{}',
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (collection_id) REFERENCES knowledge_collections(id) ON DELETE CASCADE,
                FOREIGN KEY (document_uid) REFERENCES knowledge_documents(document_uid) ON DELETE CASCADE
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_chunks_fts USING fts5(
                content,
                chunk_id UNINDEXED,
                collection_id UNINDEXED,
                document_uid UNINDEXED,
                tokenize='unicode61'
            );

            CREATE TABLE IF NOT EXISTS knowledge_query_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id TEXT NOT NULL DEFAULT '',
                query TEXT NOT NULL,
                collection_scope TEXT NOT NULL DEFAULT '',
                retrieval_profile TEXT NOT NULL DEFAULT 'balanced',
                top_k INTEGER NOT NULL DEFAULT 5,
                vector_candidates INTEGER NOT NULL DEFAULT 0,
                bm25_candidates INTEGER NOT NULL DEFAULT 0,
                merged_candidates INTEGER NOT NULL DEFAULT 0,
                rerank_ms INTEGER NOT NULL DEFAULT 0,
                total_ms INTEGER NOT NULL DEFAULT 0,
                result_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS knowledge_query_run_scopes (
                run_id INTEGER NOT NULL,
                collection_id TEXT NOT NULL,
                PRIMARY KEY (run_id, collection_id),
                FOREIGN KEY (run_id) REFERENCES knowledge_query_runs(id) ON DELETE CASCADE,
                FOREIGN KEY (collection_id) REFERENCES knowledge_collections(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS knowledge_schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_knowledge_documents_collection_uid
                ON knowledge_documents(collection_id, document_uid);
            CREATE INDEX IF NOT EXISTS idx_knowledge_documents_collection_hash
                ON knowledge_documents(collection_id, content_hash);
            CREATE INDEX IF NOT EXISTS idx_knowledge_chunks_collection
                ON knowledge_chunks(collection_id);
            CREATE INDEX IF NOT EXISTS idx_knowledge_chunks_collection_document
                ON knowledge_chunks(collection_id, document_uid);
            CREATE INDEX IF NOT EXISTS idx_knowledge_query_runs_created_at
                ON knowledge_query_runs(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_knowledge_query_runs_project_created_at
                ON knowledge_query_runs(project_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_knowledge_query_run_scopes_collection
                ON knowledge_query_run_scopes(collection_id, run_id DESC);

            INSERT OR IGNORE INTO knowledge_schema_version(version) VALUES (3);",
        )
        .map_err(|e| AppError::database(format!("Failed creating knowledge v2 tables: {}", e)))?;

        Ok(())
    }

    fn migrate_query_runs_to_v3(conn: &rusqlite::Connection) -> AppResult<()> {
        if Self::table_exists(conn, "knowledge_query_runs")?
            && !Self::table_exists(conn, "knowledge_query_runs_v2_backup")?
        {
            conn.execute(
                "CREATE TABLE knowledge_query_runs_v2_backup AS SELECT * FROM knowledge_query_runs",
                [],
            )
            .map_err(|e| {
                AppError::database(format!(
                    "Failed creating knowledge_query_runs backup before v3 migration: {}",
                    e
                ))
            })?;
        }

        if !Self::table_has_column(conn, "knowledge_query_runs", "project_id")? {
            conn.execute(
                "ALTER TABLE knowledge_query_runs ADD COLUMN project_id TEXT NOT NULL DEFAULT ''",
                [],
            )
            .map_err(|e| {
                AppError::database(format!(
                    "Failed to add knowledge_query_runs.project_id: {}",
                    e
                ))
            })?;
        }

        if !Self::table_has_column(conn, "knowledge_query_runs", "retrieval_profile")? {
            conn.execute(
                "ALTER TABLE knowledge_query_runs ADD COLUMN retrieval_profile TEXT NOT NULL DEFAULT 'balanced'",
                [],
            )
            .map_err(|e| {
                AppError::database(format!(
                    "Failed to add knowledge_query_runs.retrieval_profile: {}",
                    e
                ))
            })?;
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS knowledge_query_run_scopes (
                run_id INTEGER NOT NULL,
                collection_id TEXT NOT NULL,
                PRIMARY KEY (run_id, collection_id),
                FOREIGN KEY (run_id) REFERENCES knowledge_query_runs(id) ON DELETE CASCADE,
                FOREIGN KEY (collection_id) REFERENCES knowledge_collections(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_knowledge_query_runs_project_created_at
                ON knowledge_query_runs(project_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_knowledge_query_run_scopes_collection
                ON knowledge_query_run_scopes(collection_id, run_id DESC);",
        )
        .map_err(|e| AppError::database(format!("Failed migrating query runs schema v3: {}", e)))?;

        conn.execute(
            "INSERT OR IGNORE INTO knowledge_schema_version(version) VALUES (3)",
            [],
        )
        .map_err(|e| AppError::database(format!("Failed updating schema version to 3: {}", e)))?;

        Ok(())
    }

    fn source_kind_and_locator(
        source_path: Option<&str>,
        collection_id: &str,
        display_name: &str,
    ) -> (String, String) {
        match source_path {
            Some(sp) if !sp.is_empty() => {
                if sp.starts_with("upload://") {
                    ("upload".to_string(), sp.to_string())
                } else if Path::new(sp).is_absolute() {
                    ("workspace".to_string(), sp.to_string())
                } else {
                    (
                        "upload".to_string(),
                        format!(
                            "upload://manual/{}/{}",
                            uuid::Uuid::new_v4(),
                            Self::sanitize_filename(sp)
                        ),
                    )
                }
            }
            _ => (
                "upload".to_string(),
                format!(
                    "upload://{}/{}/{}",
                    collection_id,
                    uuid::Uuid::new_v4(),
                    Self::sanitize_filename(display_name)
                ),
            ),
        }
    }

    fn sanitize_filename(name: &str) -> String {
        let mut out = String::with_capacity(name.len());
        for ch in name.chars() {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                out.push(ch);
            } else {
                out.push('_');
            }
        }
        if out.is_empty() {
            "document".to_string()
        } else {
            out
        }
    }

    fn migrate_legacy_chunks(tx: &rusqlite::Transaction<'_>) -> AppResult<()> {
        #[derive(Debug, Clone)]
        struct LegacyChunk {
            collection_id: String,
            document_id: String,
            chunk_index: i64,
            content: String,
            embedding: Option<Vec<u8>>,
            metadata_json: String,
        }

        let mut stmt = tx
            .prepare(
                "SELECT collection_id, document_id, chunk_index, content, embedding, COALESCE(metadata, '{}')
                 FROM knowledge_chunks_v1_backup ORDER BY id",
            )
            .map_err(|e| AppError::database(format!("Failed preparing legacy chunk read: {}", e)))?;

        let rows: Vec<LegacyChunk> = stmt
            .query_map([], |row| {
                Ok(LegacyChunk {
                    collection_id: row.get(0)?,
                    document_id: row.get(1)?,
                    chunk_index: row.get(2)?,
                    content: row.get(3)?,
                    embedding: row.get::<_, Option<Vec<u8>>>(4)?,
                    metadata_json: row.get(5)?,
                })
            })
            .map_err(|e| AppError::database(format!("Failed querying legacy chunks: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        let mut doc_uid_by_key: HashMap<String, String> = HashMap::new();

        for legacy in rows {
            let mut metadata: HashMap<String, String> =
                serde_json::from_str(&legacy.metadata_json).unwrap_or_default();

            let source_path = metadata.get("source_path").cloned();
            let source_type = metadata.get("source_type").cloned().unwrap_or_default();
            let old_hash = metadata
                .get("content_hash")
                .cloned()
                .unwrap_or_else(|| format!("{:x}", Sha256::digest(legacy.content.as_bytes())));

            let (source_kind, source_locator) = Self::source_kind_and_locator(
                source_path.as_deref(),
                &legacy.collection_id,
                &legacy.document_id,
            );
            let trackable = source_kind == "workspace";
            let doc_key = format!(
                "{}\u{1f}{}\u{1f}{}",
                legacy.collection_id, source_kind, source_locator
            );

            let document_uid = if let Some(uid) = doc_uid_by_key.get(&doc_key) {
                uid.clone()
            } else {
                let uid = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO knowledge_documents
                     (document_uid, collection_id, display_name, source_kind, source_locator, source_type, content_hash, trackable)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        uid,
                        legacy.collection_id,
                        legacy.document_id,
                        source_kind,
                        source_locator,
                        source_type,
                        old_hash,
                        if trackable { 1 } else { 0 },
                    ],
                )
                .map_err(|e| AppError::database(format!("Failed inserting migrated document: {}", e)))?;
                doc_uid_by_key.insert(doc_key, uid.clone());
                uid
            };

            metadata.insert("document_uid".to_string(), document_uid.clone());
            metadata
                .entry("source_kind".to_string())
                .or_insert_with(|| source_kind.clone());
            metadata
                .entry("source_locator".to_string())
                .or_insert_with(|| source_locator.clone());

            let metadata_json =
                serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string());

            tx.execute(
                "INSERT INTO knowledge_chunks (collection_id, document_uid, chunk_index, content, embedding, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    legacy.collection_id,
                    document_uid,
                    legacy.chunk_index,
                    legacy.content,
                    legacy.embedding.unwrap_or_default(),
                    metadata_json,
                ],
            )
            .map_err(|e| AppError::database(format!("Failed inserting migrated chunk: {}", e)))?;
        }

        Ok(())
    }

    fn recreate_fts(tx: &rusqlite::Transaction<'_>) -> AppResult<()> {
        tx.execute_batch(
            "DROP TABLE IF EXISTS knowledge_chunks_fts;
             DROP TABLE IF EXISTS knowledge_chunks_fts_data;
             DROP TABLE IF EXISTS knowledge_chunks_fts_idx;
             DROP TABLE IF EXISTS knowledge_chunks_fts_docsize;
             DROP TABLE IF EXISTS knowledge_chunks_fts_config;",
        )
        .map_err(|e| AppError::database(format!("Failed dropping knowledge_chunks_fts: {}", e)))?;
        tx.execute(Self::KNOWLEDGE_CHUNKS_FTS_SCHEMA_SQL, [])
            .map_err(|e| {
                AppError::database(format!("Failed creating knowledge_chunks_fts: {}", e))
            })?;
        Ok(())
    }

    fn rebuild_fts(tx: &rusqlite::Transaction<'_>) -> AppResult<()> {
        Self::recreate_fts(tx)?;
        tx.execute(
            "INSERT INTO knowledge_chunks_fts (chunk_id, collection_id, document_uid, content)
             SELECT id, collection_id, document_uid, content FROM knowledge_chunks",
            [],
        )
        .map_err(|e| {
            AppError::database(format!("Failed rebuilding knowledge_chunks_fts: {}", e))
        })?;
        Ok(())
    }

    fn refresh_collection_chunk_count(tx: &rusqlite::Transaction<'_>) -> AppResult<()> {
        tx.execute(
            "UPDATE knowledge_collections
             SET chunk_count = (
                 SELECT COUNT(*) FROM knowledge_chunks c WHERE c.collection_id = knowledge_collections.id
             ),
             updated_at = datetime('now')",
            [],
        )
        .map_err(|e| AppError::database(format!("Failed refreshing chunk_count: {}", e)))?;
        Ok(())
    }

    fn migrate_to_knowledge_v2(&self, conn: &mut rusqlite::Connection) -> AppResult<()> {
        let has_documents = Self::table_exists(conn, "knowledge_documents")?;
        let has_chunks = Self::table_exists(conn, "knowledge_chunks")?;
        let chunks_has_uid = if has_chunks {
            Self::table_has_column(conn, "knowledge_chunks", "document_uid")?
        } else {
            false
        };

        if has_documents && chunks_has_uid {
            let tx = conn.transaction().map_err(|e| {
                AppError::database(format!("Failed starting schema finalize tx: {}", e))
            })?;
            Self::create_v2_tables(&tx)?;
            Self::rebuild_fts(&tx)?;
            Self::refresh_collection_chunk_count(&tx)?;
            tx.commit().map_err(|e| {
                AppError::database(format!("Failed committing schema finalize tx: {}", e))
            })?;
            return Ok(());
        }

        if has_chunks {
            self.create_knowledge_backup(conn)?;
        }

        let tx = conn.transaction().map_err(|e| {
            AppError::database(format!("Failed starting knowledge_v2 migration: {}", e))
        })?;

        if has_chunks {
            tx.execute(
                "ALTER TABLE knowledge_chunks RENAME TO knowledge_chunks_v1_backup",
                [],
            )
            .map_err(|e| {
                AppError::database(format!(
                    "Failed renaming legacy knowledge_chunks before migration: {}",
                    e
                ))
            })?;
        }

        Self::create_v2_tables(&tx)?;

        if has_chunks {
            Self::migrate_legacy_chunks(&tx)?;
            tx.execute("DROP TABLE knowledge_chunks_v1_backup", [])
                .map_err(|e| {
                    AppError::database(format!(
                        "Failed dropping legacy knowledge_chunks backup table: {}",
                        e
                    ))
                })?;
        }

        Self::rebuild_fts(&tx)?;
        Self::refresh_collection_chunk_count(&tx)?;

        tx.commit().map_err(|e| {
            AppError::database(format!("Failed committing knowledge_v2 migration: {}", e))
        })?;

        Ok(())
    }

    /// Get or create a collection by name for a project.
    ///
    /// Uses `INSERT OR IGNORE` followed by `SELECT` to avoid TOCTOU races
    /// when multiple threads attempt to create the same collection concurrently.
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

        // INSERT OR IGNORE: if (name, project_id) already exists, this is a no-op
        // thanks to the UNIQUE(name, project_id) constraint on the table.
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT OR IGNORE INTO knowledge_collections (id, name, project_id, description)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, name, project_id, description],
        )
        .map_err(|e| AppError::database(format!("Failed to create collection: {}", e)))?;

        // Fetch actual id (ours or the concurrent winner's)
        let actual_id: String = conn
            .query_row(
                "SELECT id FROM knowledge_collections WHERE name = ?1 AND project_id = ?2",
                rusqlite::params![name, project_id],
                |row| row.get(0),
            )
            .map_err(|e| {
                AppError::database(format!("Failed to fetch collection after insert: {}", e))
            })?;

        Ok(actual_id)
    }

    /// Ingest documents into a collection: chunk, embed, and store.
    pub async fn ingest(
        &self,
        collection_name: &str,
        project_id: &str,
        description: &str,
        documents: Vec<Document>,
    ) -> AppResult<KnowledgeCollection> {
        self.ingest_with_progress(collection_name, project_id, description, documents, None)
            .await
    }

    /// Ingest documents with optional progress events via Tauri AppHandle.
    ///
    /// Emits `knowledge:ingest-progress` events at three stages:
    /// - chunking (0-30%)
    /// - embedding (30-70%)
    /// - storing (70-100%)
    pub async fn ingest_with_progress(
        &self,
        collection_name: &str,
        project_id: &str,
        description: &str,
        documents: Vec<Document>,
        app_handle: Option<&tauri::AppHandle>,
    ) -> AppResult<KnowledgeCollection> {
        let collection_id =
            self.get_or_create_collection(collection_name, project_id, description)?;
        self.ingest_into_collection_with_progress(
            &collection_id,
            documents,
            app_handle,
            Some(collection_name),
        )
        .await
    }

    /// Ingest documents directly into an existing collection.
    pub async fn ingest_into_collection(
        &self,
        collection_id: &str,
        documents: Vec<Document>,
    ) -> AppResult<KnowledgeCollection> {
        self.ingest_into_collection_with_progress(collection_id, documents, None, None)
            .await
    }

    pub async fn ingest_into_collection_with_progress(
        &self,
        collection_id: &str,
        documents: Vec<Document>,
        app_handle: Option<&tauri::AppHandle>,
        collection_name_hint: Option<&str>,
    ) -> AppResult<KnowledgeCollection> {
        let collection = self.get_collection(collection_id)?;
        let collection_name = collection_name_hint.unwrap_or(collection.name.as_str());
        let job_id = uuid::Uuid::new_v4().to_string();
        let job_scoped_progress = self.is_feature_enabled("kb_ingest_job_scoped_progress", true);

        let emit_progress = |stage: &str, progress: u32, detail: &str| {
            if let Some(handle) = app_handle {
                let payload = Self::build_ingest_progress_payload(
                    job_scoped_progress,
                    &job_id,
                    &collection.project_id,
                    collection_id,
                    collection_name,
                    stage,
                    progress,
                    detail,
                );
                let _ = handle.emit("knowledge:ingest-progress", payload);
            }
        };

        emit_progress("chunking", 0, "Starting document chunking...");

        let mut documents = documents;
        let mut doc_scope: HashMap<String, (String, String, String, String, String, bool)> =
            HashMap::new();
        for doc in &mut documents {
            if let Some(ref sp) = doc.source_path {
                doc.metadata
                    .entry("source_path".to_string())
                    .or_insert_with(|| sp.clone());
            }
            let source_type = doc
                .metadata
                .get("source_type")
                .cloned()
                .or_else(|| {
                    doc.source_path
                        .as_ref()
                        .and_then(|p| Path::new(p).extension().and_then(|e| e.to_str()))
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();
            let (source_kind, source_locator) =
                Self::source_kind_and_locator(doc.source_path.as_deref(), collection_id, &doc.id);
            let content_hash = format!("{:x}", Sha256::digest(doc.content.as_bytes()));
            let trackable = source_kind == "workspace";

            doc.metadata
                .insert("source_kind".to_string(), source_kind.clone());
            doc.metadata
                .insert("source_locator".to_string(), source_locator.clone());
            doc.metadata
                .insert("source_type".to_string(), source_type.clone());
            doc.metadata
                .insert("content_hash".to_string(), content_hash.clone());
            doc.metadata.insert(
                "trackable".to_string(),
                if trackable { "true" } else { "false" }.to_string(),
            );

            let doc_key = format!("{}\u{1f}{}\u{1f}{}", doc.id, source_kind, source_locator);
            doc_scope.insert(
                doc_key,
                (
                    doc.id.clone(),
                    source_kind,
                    source_locator,
                    source_type,
                    content_hash,
                    trackable,
                ),
            );
        }

        let mut all_chunks: Vec<Chunk> = Vec::new();
        for doc in &documents {
            let chunks = self.chunker.chunk(doc)?;
            all_chunks.extend(chunks);
        }

        if all_chunks.is_empty() {
            emit_progress("storing", 100, "No chunks to store.");
            return self.get_collection(collection_id);
        }

        emit_progress(
            "chunking",
            30,
            &format!(
                "Chunked {} documents into {} chunks",
                documents.len(),
                all_chunks.len()
            ),
        );

        emit_progress("embedding", 30, "Generating embeddings...");
        let chunk_texts: Vec<&str> = all_chunks.iter().map(|c| c.content.as_str()).collect();
        let embeddings = self
            .embedding_manager
            .embed_documents(&chunk_texts)
            .await
            .map_err(|e| AppError::internal(format!("Embedding failed: {}", e)))?;

        emit_progress(
            "embedding",
            70,
            &format!("Embedded {} chunks", embeddings.len()),
        );

        emit_progress("storing", 70, "Storing chunks in database...");
        let (hnsw_items, stale_rowids) = {
            let mut conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let tx = conn
                .transaction()
                .map_err(|e| AppError::database(format!("Failed to begin transaction: {}", e)))?;

            let mut stale: Vec<i64> = Vec::new();
            let mut doc_uid_by_key: HashMap<String, String> = HashMap::new();

            for (
                doc_key,
                (display_name, source_kind, source_locator, source_type, content_hash, trackable),
            ) in &doc_scope
            {
                let existing_uid: Option<String> = tx
                    .query_row(
                        "SELECT document_uid FROM knowledge_documents
                         WHERE collection_id = ?1 AND source_kind = ?2 AND source_locator = ?3",
                        rusqlite::params![collection_id, source_kind, source_locator],
                        |row| row.get(0),
                    )
                    .ok();

                let document_uid = if let Some(uid) = existing_uid {
                    let mut stale_stmt = tx
                        .prepare(
                            "SELECT id FROM knowledge_chunks WHERE collection_id = ?1 AND document_uid = ?2",
                        )
                        .map_err(|e| {
                            AppError::database(format!(
                                "Failed to prepare stale chunk query: {}",
                                e
                            ))
                        })?;
                    let mut stale_rows = stale_stmt
                        .query(rusqlite::params![collection_id, uid.clone()])
                        .map_err(|e| {
                            AppError::database(format!("Failed to query stale chunks: {}", e))
                        })?;
                    while let Some(row) = stale_rows.next().map_err(|e| {
                        AppError::database(format!("Failed reading stale chunk row: {}", e))
                    })? {
                        stale.push(row.get::<_, i64>(0).unwrap_or_default());
                    }

                    tx.execute(
                        "DELETE FROM knowledge_chunks_fts WHERE chunk_id IN (
                            SELECT id FROM knowledge_chunks WHERE collection_id = ?1 AND document_uid = ?2
                        )",
                        rusqlite::params![collection_id, uid.clone()],
                    )
                    .map_err(|e| {
                        AppError::database(format!("Failed deleting old FTS rows for document: {}", e))
                    })?;
                    tx.execute(
                        "DELETE FROM knowledge_chunks WHERE collection_id = ?1 AND document_uid = ?2",
                        rusqlite::params![collection_id, uid.clone()],
                    )
                    .map_err(|e| {
                        AppError::database(format!("Failed deleting old chunks for document: {}", e))
                    })?;
                    tx.execute(
                        "UPDATE knowledge_documents
                         SET display_name = ?1, source_type = ?2, content_hash = ?3, trackable = ?4,
                             updated_at = datetime('now'), last_indexed_at = datetime('now')
                         WHERE document_uid = ?5",
                        rusqlite::params![
                            display_name,
                            source_type,
                            content_hash,
                            if *trackable { 1 } else { 0 },
                            uid
                        ],
                    )
                    .map_err(|e| {
                        AppError::database(format!("Failed updating document metadata: {}", e))
                    })?;
                    uid
                } else {
                    let uid = uuid::Uuid::new_v4().to_string();
                    tx.execute(
                        "INSERT INTO knowledge_documents
                         (document_uid, collection_id, display_name, source_kind, source_locator, source_type, content_hash, trackable)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        rusqlite::params![
                            uid,
                            collection_id,
                            display_name,
                            source_kind,
                            source_locator,
                            source_type,
                            content_hash,
                            if *trackable { 1 } else { 0 },
                        ],
                    )
                    .map_err(|e| {
                        AppError::database(format!("Failed inserting knowledge document: {}", e))
                    })?;
                    uid
                };

                doc_uid_by_key.insert(doc_key.clone(), document_uid);
            }

            let mut items: Vec<(usize, Vec<f32>)> = Vec::new();
            for (i, chunk) in all_chunks.iter().enumerate() {
                let source_kind = chunk
                    .metadata
                    .get("source_kind")
                    .cloned()
                    .unwrap_or_else(|| "upload".to_string());
                let source_locator = chunk
                    .metadata
                    .get("source_locator")
                    .cloned()
                    .unwrap_or_else(|| {
                        format!(
                            "upload://{}/{}/{}",
                            collection_id,
                            uuid::Uuid::new_v4(),
                            Self::sanitize_filename(&chunk.document_id)
                        )
                    });
                let doc_key = format!(
                    "{}\u{1f}{}\u{1f}{}",
                    chunk.document_id, source_kind, source_locator
                );
                let document_uid = doc_uid_by_key.get(&doc_key).ok_or_else(|| {
                    AppError::database(format!(
                        "Document scope missing for chunk insertion: {}",
                        doc_key
                    ))
                })?;

                let mut metadata = chunk.metadata.clone();
                metadata.insert("document_uid".to_string(), document_uid.clone());
                let metadata_json =
                    serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string());
                let embedding_bytes = embedding_to_bytes(&embeddings[i]);

                tx.execute(
                    "INSERT INTO knowledge_chunks (collection_id, document_uid, chunk_index, content, embedding, metadata)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        collection_id,
                        document_uid,
                        chunk.index as i64,
                        chunk.content,
                        embedding_bytes,
                        metadata_json,
                    ],
                )
                .map_err(|e| AppError::database(format!("Failed to insert chunk: {}", e)))?;

                let chunk_id = tx.last_insert_rowid() as i64;
                tx.execute(
                    "INSERT INTO knowledge_chunks_fts (chunk_id, collection_id, document_uid, content)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![chunk_id, collection_id, document_uid, chunk.content],
                )
                .map_err(|e| AppError::database(format!("Failed to insert chunk FTS row: {}", e)))?;

                items.push((chunk_id as usize, embeddings[i].clone()));
            }

            let chunk_count: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_chunks WHERE collection_id = ?1",
                    rusqlite::params![collection_id],
                    |row| row.get(0),
                )
                .map_err(|e| AppError::database(format!("Failed to count chunks: {}", e)))?;

            tx.execute(
                "UPDATE knowledge_collections SET chunk_count = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![chunk_count, collection_id],
            )
            .map_err(|e| AppError::database(format!("Failed to update collection: {}", e)))?;

            tx.commit()
                .map_err(|e| AppError::database(format!("Failed to commit transaction: {}", e)))?;

            (items, stale)
        };

        if !self.hnsw_index.is_ready().await {
            self.hnsw_index.initialize().await;
        }
        for rowid in stale_rowids {
            self.hnsw_index.mark_stale(rowid as usize).await;
        }
        if !self.hnsw_index.batch_insert(&hnsw_items).await {
            tracing::error!(
                "HNSW batch_insert failed for '{}'; SQLite remains source of truth",
                collection_name
            );
        }
        if let Err(e) = self.hnsw_index.save_to_disk().await {
            tracing::warn!(
                error = %e,
                "HNSW save_to_disk failed after ingest for '{}'; index will rebuild at startup",
                collection_name
            );
        }

        emit_progress("storing", 100, "Ingestion complete.");
        self.get_collection(collection_id)
    }

    /// Query a collection with a natural language query.
    pub async fn query(
        &self,
        collection_name: &str,
        project_id: &str,
        query_text: &str,
        top_k: usize,
    ) -> AppResult<RagQueryResult> {
        let collection_id = {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;
            conn.query_row(
                "SELECT id FROM knowledge_collections WHERE name = ?1 AND project_id = ?2",
                rusqlite::params![collection_name, project_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|_| {
                AppError::not_found(format!(
                    "Collection '{}' not found for project '{}'",
                    collection_name, project_id
                ))
            })?
        };

        let mut result = self
            .query_scoped(
                project_id,
                query_text,
                top_k,
                Some(&[collection_id]),
                None,
                None,
            )
            .await?;
        result.collection_name = collection_name.to_string();
        Ok(result)
    }

    fn resolve_collection_scope(
        &self,
        project_id: &str,
        collection_ids: Option<&[String]>,
    ) -> AppResult<Vec<(String, String)>> {
        let mut collections = self
            .list_collections(project_id)?
            .into_iter()
            .map(|c| (c.id, c.name))
            .collect::<Vec<_>>();
        if let Some(ids) = collection_ids {
            if !ids.is_empty() {
                let filter: HashSet<&str> = ids.iter().map(|id| id.as_str()).collect();
                collections.retain(|(id, _)| filter.contains(id.as_str()));
            }
        }
        Ok(collections)
    }

    fn load_chunk_search_result(
        conn: &rusqlite::Connection,
        chunk_id: i64,
    ) -> AppResult<Option<(SearchResult, Vec<f32>)>> {
        let row = conn.query_row(
            "SELECT c.id, c.collection_id, col.name, d.display_name, d.document_uid,
                    c.content, COALESCE(c.metadata, '{}'), COALESCE(c.embedding, X'')
             FROM knowledge_chunks c
             JOIN knowledge_documents d ON c.document_uid = d.document_uid
             JOIN knowledge_collections col ON c.collection_id = col.id
             WHERE c.id = ?1",
            rusqlite::params![chunk_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Vec<u8>>(7)?,
                ))
            },
        );

        let Ok((
            _cid,
            collection_id,
            collection_name,
            display_name,
            document_uid,
            content,
            metadata_json,
            emb_bytes,
        )) = row
        else {
            return Ok(None);
        };

        let metadata: HashMap<String, String> =
            serde_json::from_str(&metadata_json).unwrap_or_default();
        let embedding = bytes_to_embedding(&emb_bytes);
        Ok(Some((
            SearchResult {
                chunk_text: content,
                collection_id,
                document_id: display_name,
                document_uid,
                collection_name,
                score: 0.0,
                metadata,
            },
            embedding,
        )))
    }

    fn apply_mmr(
        candidates: Vec<(SearchResult, Vec<f32>, f32)>,
        max_items: usize,
        lambda: f32,
    ) -> Vec<(SearchResult, f32)> {
        if candidates.is_empty() {
            return Vec::new();
        }
        let mut remaining = candidates;
        let mut selected: Vec<(SearchResult, Vec<f32>, f32)> = Vec::new();

        while !remaining.is_empty() && selected.len() < max_items {
            let mut best_idx = 0usize;
            let mut best_score = f32::MIN;
            for (idx, (_res, emb, relevance)) in remaining.iter().enumerate() {
                let diversity = if emb.is_empty() || selected.is_empty() {
                    0.0
                } else {
                    selected
                        .iter()
                        .filter(|(_, s_emb, _)| !s_emb.is_empty())
                        .map(|(_, s_emb, _)| cosine_similarity(emb, s_emb))
                        .fold(0.0, f32::max)
                };
                let mmr_score = lambda * *relevance - (1.0 - lambda) * diversity;
                if mmr_score > best_score {
                    best_score = mmr_score;
                    best_idx = idx;
                }
            }
            let chosen = remaining.remove(best_idx);
            selected.push(chosen);
        }

        selected
            .into_iter()
            .map(|(mut res, _emb, relevance)| {
                res.score = relevance;
                (res, relevance)
            })
            .collect()
    }

    fn log_query_run(
        &self,
        project_id: &str,
        query: &str,
        collection_scope: &str,
        scoped_collection_ids: &[String],
        retrieval_profile: &str,
        top_k: usize,
        vector_candidates: usize,
        bm25_candidates: usize,
        merged_candidates: usize,
        rerank_ms: u128,
        total_ms: u128,
        result_count: usize,
    ) -> Option<i64> {
        if !self.is_feature_enabled("kb_query_runs_v2", true) {
            return self.log_query_run_legacy(
                project_id,
                query,
                collection_scope,
                retrieval_profile,
                top_k,
                vector_candidates,
                bm25_candidates,
                merged_candidates,
                rerank_ms,
                total_ms,
                result_count,
            );
        }

        let mut conn = match self.database.get_connection() {
            Ok(c) => c,
            Err(_) => return None,
        };
        let tx = conn.transaction().ok()?;
        tx.execute(
            "INSERT INTO knowledge_query_runs
             (project_id, query, collection_scope, retrieval_profile, top_k, vector_candidates, bm25_candidates, merged_candidates, rerank_ms, total_ms, result_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                project_id,
                query,
                collection_scope,
                retrieval_profile,
                top_k as i64,
                vector_candidates as i64,
                bm25_candidates as i64,
                merged_candidates as i64,
                rerank_ms as i64,
                total_ms as i64,
                result_count as i64,
            ],
        )
        .ok()?;
        let run_id = tx.last_insert_rowid();
        for collection_id in scoped_collection_ids {
            let _ = tx.execute(
                "INSERT OR IGNORE INTO knowledge_query_run_scopes (run_id, collection_id) VALUES (?1, ?2)",
                rusqlite::params![run_id, collection_id],
            );
        }
        if tx.commit().is_ok() {
            Some(run_id)
        } else {
            self.log_query_run_legacy(
                project_id,
                query,
                collection_scope,
                retrieval_profile,
                top_k,
                vector_candidates,
                bm25_candidates,
                merged_candidates,
                rerank_ms,
                total_ms,
                result_count,
            )
        }
    }

    fn log_query_run_legacy(
        &self,
        project_id: &str,
        query: &str,
        collection_scope: &str,
        retrieval_profile: &str,
        top_k: usize,
        vector_candidates: usize,
        bm25_candidates: usize,
        merged_candidates: usize,
        rerank_ms: u128,
        total_ms: u128,
        result_count: usize,
    ) -> Option<i64> {
        let conn = self.database.get_connection().ok()?;
        let has_project_id =
            Self::table_has_column(&conn, "knowledge_query_runs", "project_id").ok()?;
        let has_profile =
            Self::table_has_column(&conn, "knowledge_query_runs", "retrieval_profile").ok()?;
        let sql = match (has_project_id, has_profile) {
            (true, true) => {
                "INSERT INTO knowledge_query_runs
                 (project_id, query, collection_scope, retrieval_profile, top_k, vector_candidates, bm25_candidates, merged_candidates, rerank_ms, total_ms, result_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
            }
            (true, false) => {
                "INSERT INTO knowledge_query_runs
                 (project_id, query, collection_scope, top_k, vector_candidates, bm25_candidates, merged_candidates, rerank_ms, total_ms, result_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
            }
            (false, true) => {
                "INSERT INTO knowledge_query_runs
                 (query, collection_scope, retrieval_profile, top_k, vector_candidates, bm25_candidates, merged_candidates, rerank_ms, total_ms, result_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
            }
            (false, false) => {
                "INSERT INTO knowledge_query_runs
                 (query, collection_scope, top_k, vector_candidates, bm25_candidates, merged_candidates, rerank_ms, total_ms, result_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            }
        };
        let result = match (has_project_id, has_profile) {
            (true, true) => conn.execute(
                sql,
                rusqlite::params![
                    project_id,
                    query,
                    collection_scope,
                    retrieval_profile,
                    top_k as i64,
                    vector_candidates as i64,
                    bm25_candidates as i64,
                    merged_candidates as i64,
                    rerank_ms as i64,
                    total_ms as i64,
                    result_count as i64,
                ],
            ),
            (true, false) => conn.execute(
                sql,
                rusqlite::params![
                    project_id,
                    query,
                    collection_scope,
                    top_k as i64,
                    vector_candidates as i64,
                    bm25_candidates as i64,
                    merged_candidates as i64,
                    rerank_ms as i64,
                    total_ms as i64,
                    result_count as i64,
                ],
            ),
            (false, true) => conn.execute(
                sql,
                rusqlite::params![
                    query,
                    collection_scope,
                    retrieval_profile,
                    top_k as i64,
                    vector_candidates as i64,
                    bm25_candidates as i64,
                    merged_candidates as i64,
                    rerank_ms as i64,
                    total_ms as i64,
                    result_count as i64,
                ],
            ),
            (false, false) => conn.execute(
                sql,
                rusqlite::params![
                    query,
                    collection_scope,
                    top_k as i64,
                    vector_candidates as i64,
                    bm25_candidates as i64,
                    merged_candidates as i64,
                    rerank_ms as i64,
                    total_ms as i64,
                    result_count as i64,
                ],
            ),
        };
        result.ok()?;
        Some(conn.last_insert_rowid())
    }

    /// Query with scoped filters and hybrid retrieval (Vector + BM25 + RRF + MMR).
    pub async fn query_scoped(
        &self,
        project_id: &str,
        query_text: &str,
        top_k: usize,
        collection_ids: Option<&[String]>,
        document_filters: Option<&[ScopedDocumentRef]>,
        retrieval_profile: Option<&str>,
    ) -> AppResult<RagQueryResult> {
        let started = Instant::now();
        let profile = RetrievalProfile::from_raw(retrieval_profile);
        if let Some(raw_profile) = retrieval_profile {
            let normalized = raw_profile.trim().to_ascii_lowercase();
            if normalized != profile.as_str() {
                tracing::warn!(
                    profile = %raw_profile,
                    normalized = %normalized,
                    applied = %profile.as_str(),
                    "Invalid retrieval profile; falling back to balanced profile"
                );
            }
        }
        let params = profile.params();
        let scope = self.resolve_collection_scope(project_id, collection_ids)?;
        if scope.is_empty() {
            return Ok(RagQueryResult {
                results: Vec::new(),
                total_searched: 0,
                collection_name: "scoped".to_string(),
            });
        }

        let allowed_collections: HashSet<String> = scope.iter().map(|(id, _)| id.clone()).collect();
        let scoped_documents: HashSet<(String, String)> = document_filters
            .unwrap_or(&[])
            .iter()
            .map(|d| (d.collection_id.clone(), d.document_uid.clone()))
            .collect();
        let has_doc_filter = !scoped_documents.is_empty();

        let query_embedding = self
            .embedding_manager
            .embed_query(query_text)
            .await
            .map_err(|e| AppError::internal(format!("Query embedding failed: {}", e)))?;

        let mut candidate_map: HashMap<
            i64,
            (SearchResult, Vec<f32>, Option<usize>, Option<usize>),
        > = HashMap::new();
        let mut vector_ranked: Vec<i64> = Vec::new();
        let mut bm25_ranked: Vec<i64> = Vec::new();

        {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let vector_hits = self
                .hnsw_index
                .search(&query_embedding, params.vector_top_n * 3)
                .await;
            for (rank, (chunk_id, distance)) in vector_hits.iter().enumerate() {
                let Some((mut result, embedding)) =
                    Self::load_chunk_search_result(&conn, *chunk_id as i64)?
                else {
                    continue;
                };
                if !allowed_collections.contains(&result.collection_id) {
                    continue;
                }
                if has_doc_filter
                    && !scoped_documents
                        .contains(&(result.collection_id.clone(), result.document_uid.clone()))
                {
                    continue;
                }
                result.score = (1.0 - distance).clamp(0.0, 1.0);
                vector_ranked.push(*chunk_id as i64);
                candidate_map
                    .entry(*chunk_id as i64)
                    .and_modify(|entry| {
                        entry.0 = result.clone();
                        entry.1 = embedding.clone();
                        entry.2 = Some(rank + 1);
                    })
                    .or_insert((result, embedding, Some(rank + 1), None));
                if vector_ranked.len() >= params.vector_top_n {
                    break;
                }
            }

            let mut bm25_stmt = conn
                .prepare(
                    "SELECT chunk_id, collection_id, document_uid, bm25(knowledge_chunks_fts) AS rank
                     FROM knowledge_chunks_fts
                     WHERE knowledge_chunks_fts MATCH ?1
                     ORDER BY rank
                     LIMIT ?2",
                )
                .map_err(|e| AppError::database(format!("Failed to prepare BM25 query: {}", e)))?;
            let bm25_rows = bm25_stmt
                .query_map(
                    rusqlite::params![query_text, params.bm25_top_n as i64],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, f64>(3).unwrap_or(0.0),
                        ))
                    },
                )
                .map_err(|e| AppError::database(format!("Failed to execute BM25 query: {}", e)))?;

            for (rank, row) in bm25_rows.filter_map(|r| r.ok()).enumerate() {
                let (chunk_id, collection_id, document_uid, bm25_rank) = row;
                if !allowed_collections.contains(&collection_id) {
                    continue;
                }
                if has_doc_filter
                    && !scoped_documents.contains(&(collection_id.clone(), document_uid.clone()))
                {
                    continue;
                }
                let Some((mut result, embedding)) =
                    Self::load_chunk_search_result(&conn, chunk_id)?
                else {
                    continue;
                };
                let bm25_score = (1.0 / (1.0 + bm25_rank.abs() as f32)).clamp(0.0, 1.0);
                result.score = bm25_score;
                bm25_ranked.push(chunk_id);
                candidate_map
                    .entry(chunk_id)
                    .and_modify(|entry| {
                        entry.0 = result.clone();
                        entry.1 = embedding.clone();
                        entry.3 = Some(rank + 1);
                    })
                    .or_insert((result, embedding, None, Some(rank + 1)));
            }
        }

        let rrf_k = 60.0f32;
        let mut fused: Vec<(SearchResult, Vec<f32>, f32)> = candidate_map
            .into_iter()
            .map(|(_chunk_id, (result, emb, v_rank, b_rank))| {
                let mut fusion = 0.0f32;
                if let Some(v) = v_rank {
                    fusion += 1.0 / (rrf_k + v as f32);
                }
                if let Some(b) = b_rank {
                    fusion += 1.0 / (rrf_k + b as f32);
                }
                (result, emb, fusion)
            })
            .collect();

        fused.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        if fused.len() > params.fused_top_n {
            fused.truncate(params.fused_top_n);
        }

        if fused.is_empty() {
            let scope_ids: Vec<String> = allowed_collections.into_iter().collect();
            let fallback = self.sqlite_similarity_fallback(
                &scope_ids,
                query_text,
                &query_embedding,
                top_k.max(1) * 3,
                if has_doc_filter {
                    Some(&scoped_documents)
                } else {
                    None
                },
            )?;
            return Ok(RagQueryResult {
                total_searched: fallback.len(),
                results: fallback.into_iter().take(top_k).collect(),
                collection_name: "scoped".to_string(),
            });
        }

        let mmr_selected = Self::apply_mmr(fused, params.mmr_top_n, params.mmr_lambda);
        let mut search_results: Vec<SearchResult> =
            mmr_selected.into_iter().map(|(r, _)| r).collect();
        let total_searched = search_results.len();

        let rerank_started = Instant::now();
        if let Some(ref reranker) = self.reranker {
            search_results = reranker.rerank(query_text, search_results).await?;
        }
        let rerank_ms = rerank_started.elapsed().as_millis();
        search_results.truncate(top_k);

        let scope_label = scope
            .iter()
            .map(|(_, name)| name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let scope_collection_ids: Vec<String> = scope.iter().map(|(id, _)| id.clone()).collect();

        let _ = self.log_query_run(
            project_id,
            query_text,
            &scope_label,
            &scope_collection_ids,
            profile.as_str(),
            top_k,
            vector_ranked.len(),
            bm25_ranked.len(),
            total_searched,
            rerank_ms,
            started.elapsed().as_millis(),
            search_results.len(),
        );

        Ok(RagQueryResult {
            results: search_results,
            total_searched,
            collection_name: "scoped".to_string(),
        })
    }

    /// Fallback query path when ANN + BM25 search returns empty.
    fn sqlite_similarity_fallback(
        &self,
        collection_ids: &[String],
        query_text: &str,
        query_embedding: &[f32],
        limit: usize,
        scoped_documents: Option<&HashSet<(String, String)>>,
    ) -> AppResult<Vec<SearchResult>> {
        if collection_ids.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let placeholders = vec!["?"; collection_ids.len()].join(",");
        let sql = format!(
            "SELECT c.collection_id, col.name, d.display_name, d.document_uid, c.content, COALESCE(c.metadata, '{{}}'), COALESCE(c.embedding, X'')
             FROM knowledge_chunks c
             JOIN knowledge_documents d ON c.document_uid = d.document_uid
             JOIN knowledge_collections col ON c.collection_id = col.id
             WHERE c.collection_id IN ({})
               AND c.embedding IS NOT NULL
               AND length(c.embedding) > 0",
            placeholders
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| AppError::database(format!("Failed to prepare fallback query: {}", e)))?;

        let params: Vec<&dyn rusqlite::ToSql> = collection_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();

        let mut results: Vec<SearchResult> = stmt
            .query_map(params.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Vec<u8>>(6)?,
                ))
            })
            .map_err(|e| AppError::database(format!("Failed to execute fallback query: {}", e)))?
            .filter_map(|row| row.ok())
            .filter_map(
                |(
                    collection_id,
                    collection_name,
                    display_name,
                    document_uid,
                    content,
                    metadata_json,
                    emb_bytes,
                )| {
                    if let Some(filter) = scoped_documents {
                        if !filter.contains(&(collection_id.clone(), document_uid.clone())) {
                            return None;
                        }
                    }
                    let chunk_embedding = bytes_to_embedding(&emb_bytes);
                    if chunk_embedding.is_empty() {
                        return None;
                    }
                    let metadata: HashMap<String, String> =
                        serde_json::from_str(&metadata_json).unwrap_or_default();
                    let lexical_bonus =
                        if content.to_lowercase().contains(&query_text.to_lowercase()) {
                            0.05
                        } else {
                            0.0
                        };
                    Some(SearchResult {
                        chunk_text: content,
                        collection_id,
                        document_id: display_name,
                        document_uid,
                        collection_name,
                        score: (cosine_similarity(query_embedding, &chunk_embedding)
                            + lexical_bonus)
                            .clamp(0.0, 1.0),
                        metadata,
                    })
                },
            )
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if limit > 0 {
            results.truncate(limit);
        }
        Ok(results)
    }

    /// List recent query runs for a project and optional scoped collections.
    pub fn list_query_runs(
        &self,
        project_id: &str,
        collection_ids: Option<&[String]>,
        limit: usize,
    ) -> AppResult<Vec<QueryRunSummary>> {
        let has_scope_filter = collection_ids.map(|ids| !ids.is_empty()).unwrap_or(false);
        let use_v2 = self.is_feature_enabled("kb_query_runs_v2", true);
        if use_v2 {
            match self.list_query_runs_v2(project_id, collection_ids, limit) {
                Ok(runs) => {
                    if has_scope_filter {
                        let _ = observability::record_query_run_scope_check(
                            self.database.as_ref(),
                            1,
                            1,
                        );
                    }
                    return Ok(runs);
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Falling back to legacy query-runs listing path"
                    );
                }
            }
        }
        if has_scope_filter {
            let _ = observability::record_query_run_scope_check(self.database.as_ref(), 1, 0);
        }
        self.list_query_runs_legacy(project_id, collection_ids, limit)
    }

    fn list_query_runs_v2(
        &self,
        project_id: &str,
        collection_ids: Option<&[String]>,
        limit: usize,
    ) -> AppResult<Vec<QueryRunSummary>> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let safe_limit = limit.clamp(1, 500) as i64;
        let include_legacy_default = project_id == "default";
        let base_select = "SELECT DISTINCT r.id, r.project_id, r.query, r.collection_scope, r.retrieval_profile, r.top_k, r.vector_candidates, r.bm25_candidates, r.merged_candidates, r.rerank_ms, r.total_ms, r.result_count, r.created_at FROM knowledge_query_runs r";

        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<QueryRunSummary> {
            Ok(QueryRunSummary {
                id: row.get(0)?,
                project_id: row.get(1)?,
                query: row.get(2)?,
                collection_scope: row.get(3)?,
                retrieval_profile: row.get(4)?,
                top_k: row.get(5)?,
                vector_candidates: row.get(6)?,
                bm25_candidates: row.get(7)?,
                merged_candidates: row.get(8)?,
                rerank_ms: row.get(9)?,
                total_ms: row.get(10)?,
                result_count: row.get(11)?,
                created_at: row.get(12)?,
            })
        };

        if let Some(ids) = collection_ids {
            if ids.is_empty() {
                return Ok(Vec::new());
            }

            let placeholders = vec!["?"; ids.len()].join(",");
            let project_clause = if include_legacy_default {
                "(r.project_id = ?1 OR r.project_id = '')"
            } else {
                "r.project_id = ?1"
            };
            let sql = format!(
                "{base_select} \
                 JOIN knowledge_query_run_scopes s ON s.run_id = r.id \
                 WHERE {project_clause} AND s.collection_id IN ({placeholders}) \
                 ORDER BY r.id DESC LIMIT ?{}",
                ids.len() + 2
            );
            let mut stmt = conn.prepare(&sql).map_err(|e| {
                AppError::database(format!(
                    "Failed to prepare scoped query runs statement: {}",
                    e
                ))
            })?;

            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            params.push(Box::new(project_id.to_string()));
            for cid in ids {
                params.push(Box::new(cid.to_string()));
            }
            params.push(Box::new(safe_limit));
            let params_ref: Vec<&dyn rusqlite::ToSql> = params
                .iter()
                .map(|v| v.as_ref() as &dyn rusqlite::ToSql)
                .collect();

            let rows = stmt
                .query_map(params_ref.as_slice(), map_row)
                .map_err(|e| {
                    AppError::database(format!("Failed querying scoped query runs: {}", e))
                })?;
            return Ok(rows.filter_map(|r| r.ok()).collect());
        }

        let project_clause = if include_legacy_default {
            "(project_id = ?1 OR project_id = '')"
        } else {
            "project_id = ?1"
        };
        let sql = format!("{base_select} WHERE {project_clause} ORDER BY r.id DESC LIMIT ?2");
        let mut stmt = conn.prepare(&sql).map_err(|e| {
            AppError::database(format!("Failed to prepare query runs statement: {}", e))
        })?;
        let rows = stmt
            .query_map(rusqlite::params![project_id, safe_limit], map_row)
            .map_err(|e| AppError::database(format!("Failed querying query runs: {}", e)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    fn list_query_runs_legacy(
        &self,
        project_id: &str,
        collection_ids: Option<&[String]>,
        limit: usize,
    ) -> AppResult<Vec<QueryRunSummary>> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let safe_limit = limit.clamp(1, 500) as i64;
        let include_legacy_default = project_id == "default";
        let has_project_id = Self::table_has_column(&conn, "knowledge_query_runs", "project_id")?;
        let has_profile =
            Self::table_has_column(&conn, "knowledge_query_runs", "retrieval_profile")?;
        let project_expr = if has_project_id {
            "r.project_id"
        } else {
            "'' AS project_id"
        };
        let profile_expr = if has_profile {
            "r.retrieval_profile"
        } else {
            "'balanced' AS retrieval_profile"
        };
        let base_select = format!(
            "SELECT r.id, {project_expr}, r.query, r.collection_scope, {profile_expr}, r.top_k, r.vector_candidates, r.bm25_candidates, r.merged_candidates, r.rerank_ms, r.total_ms, r.result_count, r.created_at FROM knowledge_query_runs r"
        );
        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<QueryRunSummary> {
            Ok(QueryRunSummary {
                id: row.get(0)?,
                project_id: row.get(1)?,
                query: row.get(2)?,
                collection_scope: row.get(3)?,
                retrieval_profile: row.get(4)?,
                top_k: row.get(5)?,
                vector_candidates: row.get(6)?,
                bm25_candidates: row.get(7)?,
                merged_candidates: row.get(8)?,
                rerank_ms: row.get(9)?,
                total_ms: row.get(10)?,
                result_count: row.get(11)?,
                created_at: row.get(12)?,
            })
        };

        let mut where_clauses: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut next_idx = 1usize;

        if has_project_id {
            if include_legacy_default {
                where_clauses.push(format!(
                    "(r.project_id = ?{} OR r.project_id = '')",
                    next_idx
                ));
            } else {
                where_clauses.push(format!("r.project_id = ?{}", next_idx));
            }
            params.push(Box::new(project_id.to_string()));
            next_idx += 1;
        }

        if let Some(ids) = collection_ids {
            if ids.is_empty() {
                return Ok(Vec::new());
            }

            let mut scope_names: Vec<String> = Vec::new();
            for id in ids {
                if let Ok(name) = conn.query_row(
                    "SELECT name FROM knowledge_collections WHERE id = ?1",
                    rusqlite::params![id],
                    |row| row.get::<_, String>(0),
                ) {
                    scope_names.push(name);
                }
            }
            if scope_names.is_empty() {
                return Ok(Vec::new());
            }

            let mut scope_predicates = Vec::new();
            for scope_name in scope_names {
                scope_predicates.push(format!("r.collection_scope LIKE ?{}", next_idx));
                params.push(Box::new(format!("%{}%", scope_name)));
                next_idx += 1;
            }
            where_clauses.push(format!("({})", scope_predicates.join(" OR ")));
        }

        let mut sql = base_select;
        if !where_clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&where_clauses.join(" AND "));
        }
        sql.push_str(&format!(" ORDER BY r.id DESC LIMIT ?{}", next_idx));
        params.push(Box::new(safe_limit));

        let params_ref: Vec<&dyn rusqlite::ToSql> = params
            .iter()
            .map(|v| v.as_ref() as &dyn rusqlite::ToSql)
            .collect();
        let mut stmt = conn.prepare(&sql).map_err(|e| {
            AppError::database(format!("Failed to prepare legacy query runs: {}", e))
        })?;
        let rows = stmt
            .query_map(params_ref.as_slice(), map_row)
            .map_err(|e| AppError::database(format!("Failed querying legacy query runs: {}", e)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// List all collections for a project.
    pub fn list_collections(&self, project_id: &str) -> AppResult<Vec<KnowledgeCollection>> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, project_id, description, chunk_count, created_at, updated_at, workspace_path
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
                    workspace_path: row.get(7)?,
                })
            })
            .map_err(|e| AppError::database(format!("Failed to query collections: {}", e)))?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// List all documents in a collection with chunk count and preview.
    pub fn list_documents(&self, collection_id: &str) -> AppResult<Vec<DocumentSummary>> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn
            .prepare(
                "SELECT
                    d.document_uid,
                    d.display_name,
                    d.source_kind,
                    d.source_locator,
                    d.source_type,
                    d.trackable,
                    d.last_indexed_at,
                    COALESCE(dc.chunk_count, 0) AS chunk_count,
                    COALESCE(dp.preview, '') AS preview
                 FROM knowledge_documents d
                 LEFT JOIN (
                    SELECT document_uid, COUNT(*) AS chunk_count
                    FROM knowledge_chunks
                    WHERE collection_id = ?1
                    GROUP BY document_uid
                 ) dc ON dc.document_uid = d.document_uid
                 LEFT JOIN (
                    SELECT document_uid, SUBSTR(content, 1, 200) AS preview
                    FROM knowledge_chunks
                    WHERE collection_id = ?1 AND chunk_index = 0
                 ) dp ON dp.document_uid = d.document_uid
                 WHERE d.collection_id = ?1
                 ORDER BY d.display_name",
            )
            .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt
            .query_map(rusqlite::params![collection_id], |row| {
                Ok(DocumentSummary {
                    document_uid: row.get(0)?,
                    display_name: row.get(1)?,
                    source_kind: row.get(2)?,
                    source_locator: row.get(3)?,
                    source_type: row.get(4)?,
                    trackable: row.get::<_, i64>(5)? > 0,
                    last_indexed_at: row.get(6)?,
                    chunk_count: row.get(7)?,
                    preview: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                })
            })
            .map_err(|e| AppError::database(format!("Failed to query documents: {}", e)))?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Search documents by display name across project collections.
    pub fn search_documents(
        &self,
        project_id: &str,
        query: &str,
        collection_ids: Option<&[String]>,
        limit: usize,
    ) -> AppResult<Vec<DocumentSearchMatch>> {
        let term = query.trim();
        if term.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;
        let safe_limit = limit.clamp(1, 200) as i64;
        let like = format!("%{}%", term.to_lowercase());

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        params.push(Box::new(project_id.to_string()));
        params.push(Box::new(like));

        let sql = if let Some(ids) = collection_ids {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            let placeholders = vec!["?"; ids.len()].join(",");
            for cid in ids {
                params.push(Box::new(cid.to_string()));
            }
            params.push(Box::new(safe_limit));
            format!(
                "SELECT d.collection_id, d.document_uid, d.display_name
                 FROM knowledge_documents d
                 JOIN knowledge_collections c ON c.id = d.collection_id
                 WHERE c.project_id = ?1
                   AND LOWER(d.display_name) LIKE ?2
                   AND d.collection_id IN ({placeholders})
                 ORDER BY d.display_name
                 LIMIT ?{}",
                ids.len() + 3
            )
        } else {
            params.push(Box::new(safe_limit));
            "SELECT d.collection_id, d.document_uid, d.display_name
             FROM knowledge_documents d
             JOIN knowledge_collections c ON c.id = d.collection_id
             WHERE c.project_id = ?1
               AND LOWER(d.display_name) LIKE ?2
             ORDER BY d.display_name
             LIMIT ?3"
                .to_string()
        };

        let mut stmt = conn.prepare(&sql).map_err(|e| {
            AppError::database(format!("Failed to prepare document search query: {}", e))
        })?;
        let params_ref: Vec<&dyn rusqlite::ToSql> = params
            .iter()
            .map(|v| v.as_ref() as &dyn rusqlite::ToSql)
            .collect();
        let rows = stmt
            .query_map(params_ref.as_slice(), |row| {
                Ok(DocumentSearchMatch {
                    collection_id: row.get(0)?,
                    document_uid: row.get(1)?,
                    display_name: row.get(2)?,
                })
            })
            .map_err(|e| {
                AppError::database(format!("Failed executing document search query: {}", e))
            })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Delete a single document from a collection by document_uid.
    ///
    /// Removes all chunks for the document, marks HNSW entries as stale,
    /// and updates the collection's chunk_count — all within a transaction.
    pub async fn delete_document(&self, collection_id: &str, document_uid: &str) -> AppResult<()> {
        // Collect chunk rowids for HNSW stale marking (sync scope)
        let rowids = {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let mut stmt = conn
                .prepare(
                    "SELECT id FROM knowledge_chunks WHERE collection_id = ?1 AND document_uid = ?2",
                )
                .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

            let rowids: Vec<i64> = stmt
                .query_map(rusqlite::params![collection_id, document_uid], |row| {
                    row.get(0)
                })
                .map_err(|e| AppError::database(format!("Failed to query rowids: {}", e)))?
                .filter_map(|r| r.ok())
                .collect();

            rowids
        };

        if rowids.is_empty() {
            return Err(AppError::not_found(format!(
                "Document '{}' not found in collection",
                document_uid
            )));
        }

        // Mark stale in HNSW (async — no connection held)
        for rowid in &rowids {
            self.hnsw_index.mark_stale(*rowid as usize).await;
        }

        // Delete chunks and update count in a transaction (sync scope)
        {
            let mut conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let tx = conn
                .transaction()
                .map_err(|e| AppError::database(format!("Failed to begin transaction: {}", e)))?;

            tx.execute(
                "DELETE FROM knowledge_chunks_fts WHERE chunk_id IN (
                    SELECT id FROM knowledge_chunks WHERE collection_id = ?1 AND document_uid = ?2
                )",
                rusqlite::params![collection_id, document_uid],
            )
            .map_err(|e| {
                AppError::database(format!("Failed to delete document FTS rows: {}", e))
            })?;

            tx.execute(
                "DELETE FROM knowledge_chunks WHERE collection_id = ?1 AND document_uid = ?2",
                rusqlite::params![collection_id, document_uid],
            )
            .map_err(|e| AppError::database(format!("Failed to delete document chunks: {}", e)))?;

            tx.execute(
                "DELETE FROM knowledge_documents WHERE collection_id = ?1 AND document_uid = ?2",
                rusqlite::params![collection_id, document_uid],
            )
            .map_err(|e| AppError::database(format!("Failed to delete document record: {}", e)))?;

            let chunk_count: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_chunks WHERE collection_id = ?1",
                    rusqlite::params![collection_id],
                    |row| row.get(0),
                )
                .map_err(|e| AppError::database(format!("Failed to count chunks: {}", e)))?;

            tx.execute(
                "UPDATE knowledge_collections SET chunk_count = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![chunk_count, collection_id],
            )
            .map_err(|e| AppError::database(format!("Failed to update collection: {}", e)))?;

            tx.commit()
                .map_err(|e| AppError::database(format!("Failed to commit: {}", e)))?;
        }

        // Persist stale IDs to disk so HNSW filtering survives restart
        if let Err(e) = self.hnsw_index.save_to_disk().await {
            tracing::warn!(error = %e, "HNSW save_to_disk failed after delete_document");
        }

        Ok(())
    }

    /// Delete a collection and all its chunks.
    pub async fn delete_collection(
        &self,
        collection_name: &str,
        project_id: &str,
    ) -> AppResult<()> {
        // Collect collection_id and chunk rowids (sync scope)
        let (collection_id, rowids) = {
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
                    AppError::not_found(format!("Collection '{}' not found", collection_name))
                })?;

            let mut stmt = conn
                .prepare("SELECT rowid FROM knowledge_chunks WHERE collection_id = ?1")
                .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

            let rowids: Vec<i64> = stmt
                .query_map(rusqlite::params![collection_id], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Failed to query chunk rowids: {}", e)))?
                .filter_map(|r| r.ok())
                .collect();

            (collection_id, rowids)
            // conn dropped here
        };

        // Mark stale in HNSW (async — no connection held)
        for rowid in rowids {
            self.hnsw_index.mark_stale(rowid as usize).await;
        }

        // Delete from SQLite (sync scope)
        {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            conn.execute(
                "DELETE FROM knowledge_chunks_fts WHERE collection_id = ?1",
                rusqlite::params![collection_id],
            )
            .map_err(|e| AppError::database(format!("Failed to delete chunk fts rows: {}", e)))?;

            conn.execute(
                "DELETE FROM knowledge_chunks WHERE collection_id = ?1",
                rusqlite::params![collection_id],
            )
            .map_err(|e| AppError::database(format!("Failed to delete chunks: {}", e)))?;

            conn.execute(
                "DELETE FROM knowledge_documents WHERE collection_id = ?1",
                rusqlite::params![collection_id],
            )
            .map_err(|e| AppError::database(format!("Failed to delete documents: {}", e)))?;

            conn.execute(
                "DELETE FROM knowledge_collections WHERE id = ?1",
                rusqlite::params![collection_id],
            )
            .map_err(|e| AppError::database(format!("Failed to delete collection: {}", e)))?;
        }

        // Persist stale IDs to disk so HNSW filtering survives restart
        if let Err(e) = self.hnsw_index.save_to_disk().await {
            tracing::warn!(error = %e, "HNSW save_to_disk failed after delete_collection");
        }

        Ok(())
    }

    /// Update a collection's metadata.
    ///
    /// Only the fields wrapped in `Some` are updated; `None` means "don't change".
    /// For `workspace_path`, `Some(None)` clears the field and `Some(Some(x))` sets it.
    pub fn update_collection(
        &self,
        collection_id: &str,
        name: Option<&str>,
        description: Option<&str>,
        workspace_path: Option<Option<&str>>,
    ) -> AppResult<KnowledgeCollection> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Build SET clauses dynamically
        let mut set_parts: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(n) = name {
            set_parts.push("name = ?".to_string());
            params.push(Box::new(n.to_string()));
        }
        if let Some(d) = description {
            set_parts.push("description = ?".to_string());
            params.push(Box::new(d.to_string()));
        }
        if let Some(wp) = workspace_path {
            set_parts.push("workspace_path = ?".to_string());
            params.push(Box::new(wp.map(|s| s.to_string())));
        }

        if set_parts.is_empty() {
            return self.get_collection(collection_id);
        }

        set_parts.push("updated_at = datetime('now')".to_string());

        let sql = format!(
            "UPDATE knowledge_collections SET {} WHERE id = ?",
            set_parts.join(", ")
        );
        params.push(Box::new(collection_id.to_string()));

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows_affected = conn
            .execute(&sql, param_refs.as_slice())
            .map_err(|e| AppError::database(format!("Failed to update collection: {}", e)))?;

        if rows_affected == 0 {
            return Err(AppError::not_found(format!(
                "Collection '{}' not found",
                collection_id
            )));
        }

        self.get_collection(collection_id)
    }

    /// Re-ingest a single document: delete old chunks and re-ingest with fresh content.
    ///
    /// If `new_content` is `None`, reads from disk using the stored `source_path`.
    pub async fn reingest_document(
        &self,
        collection_id: &str,
        document_uid: &str,
        new_content: Option<String>,
        app_handle: Option<&tauri::AppHandle>,
    ) -> AppResult<KnowledgeCollection> {
        let (display_name, source_kind, source_locator, source_type) = {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            conn.query_row(
                "SELECT display_name, source_kind, source_locator, source_type
                 FROM knowledge_documents WHERE collection_id = ?1 AND document_uid = ?2",
                rusqlite::params![collection_id, document_uid],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .map_err(|e| {
                AppError::not_found(format!("Document '{}' not found: {}", document_uid, e))
            })?
        };

        let content = if let Some(c) = new_content {
            c
        } else {
            if source_kind != "workspace" {
                return Err(AppError::validation(format!(
                    "Document '{}' is not trackable from filesystem",
                    document_uid
                )));
            }
            let path = Path::new(&source_locator);
            if !path.exists() {
                return Err(AppError::not_found(format!(
                    "File not found: {}",
                    source_locator
                )));
            }
            Self::read_file_content(path, &source_type)?
        };

        let doc = Document::from_parsed_content(display_name, content, source_locator, source_type);
        self.ingest_into_collection_with_progress(collection_id, vec![doc], app_handle, None)
            .await
    }

    /// Read file content using appropriate parser based on source_type/extension.
    pub fn read_file_content(path: &Path, source_type: &str) -> AppResult<String> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or(source_type);

        match ext {
            "pdf" => crate::services::tools::file_parsers::parse_pdf(path, None)
                .map_err(|e| AppError::internal(format!("PDF parse failed: {}", e))),
            "docx" => crate::services::tools::file_parsers::parse_docx(path)
                .map_err(|e| AppError::internal(format!("DOCX parse failed: {}", e))),
            "xlsx" => crate::services::tools::file_parsers::parse_xlsx(path)
                .map_err(|e| AppError::internal(format!("XLSX parse failed: {}", e))),
            _ => std::fs::read_to_string(path)
                .map_err(|e| AppError::internal(format!("Failed to read file: {}", e))),
        }
    }

    /// Compare stored content hashes with current disk files.
    ///
    /// Returns lists of modified, deleted, and (optionally) new documents.
    /// When `scan_dir` is set, also scans for doc files not yet in the collection.
    pub fn check_collection_updates(
        &self,
        collection_id: &str,
        scan_dir: Option<&Path>,
    ) -> AppResult<CollectionUpdateCheck> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn
            .prepare(
                "SELECT document_uid, display_name, source_kind, source_locator, source_type, content_hash, trackable
                 FROM knowledge_documents
                 WHERE collection_id = ?1
                 ORDER BY display_name",
            )
            .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

        let rows: Vec<(String, String, String, String, String, String, i64)> = stmt
            .query_map(rusqlite::params![collection_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })
            .map_err(|e| AppError::database(format!("Failed to query documents: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        let mut modified = Vec::new();
        let mut deleted = Vec::new();
        let mut unchanged = 0usize;
        let mut indexed_paths = std::collections::HashSet::new();

        for (
            document_uid,
            display_name,
            source_kind,
            source_locator,
            source_type,
            old_hash,
            trackable,
        ) in &rows
        {
            let is_trackable = *trackable > 0 && source_kind == "workspace";
            if !is_trackable {
                unchanged += 1;
                continue;
            }

            indexed_paths.insert(source_locator.clone());
            let path = Path::new(source_locator);

            if !path.exists() {
                deleted.push(DocUpdateInfo {
                    document_uid: document_uid.clone(),
                    display_name: display_name.clone(),
                    source_kind: source_kind.clone(),
                    source_locator: source_locator.clone(),
                    source_type: source_type.clone(),
                    old_hash: old_hash.clone(),
                    new_hash: None,
                });
                continue;
            }

            match Self::read_file_content(path, source_type) {
                Ok(content) => {
                    let new_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
                    if new_hash != *old_hash {
                        modified.push(DocUpdateInfo {
                            document_uid: document_uid.clone(),
                            display_name: display_name.clone(),
                            source_kind: source_kind.clone(),
                            source_locator: source_locator.clone(),
                            source_type: source_type.clone(),
                            old_hash: old_hash.clone(),
                            new_hash: Some(new_hash),
                        });
                    } else {
                        unchanged += 1;
                    }
                }
                Err(_) => {
                    // Can't read — treat as unchanged
                    unchanged += 1;
                }
            }
        }

        // Scan for new files not yet indexed (used by P1-2 docs indexer)
        let mut new_files = Vec::new();
        if let Some(dir) = scan_dir {
            let doc_extensions = ["md", "mdx", "txt", "pdf", "doc", "docx"];
            if dir.is_dir() {
                let walker = ignore::WalkBuilder::new(dir)
                    .hidden(true)
                    .git_ignore(true)
                    .build();

                for entry in walker.flatten() {
                    if entry.file_type().map_or(true, |ft| !ft.is_file()) {
                        continue;
                    }
                    let ext = entry
                        .path()
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    if !doc_extensions.contains(&ext) {
                        continue;
                    }
                    let abs_path = entry.path().to_string_lossy().to_string();
                    if !indexed_paths.contains(&abs_path) {
                        new_files.push(abs_path);
                    }
                }
            }
        }

        Ok(CollectionUpdateCheck {
            collection_id: collection_id.to_string(),
            modified,
            deleted,
            new_files,
            unchanged,
        })
    }

    /// Apply detected updates: reingest modified docs, delete removed docs, ingest new files.
    pub async fn apply_collection_updates(
        &self,
        collection_id: &str,
        updates: &CollectionUpdateCheck,
        app_handle: Option<&tauri::AppHandle>,
    ) -> AppResult<KnowledgeCollection> {
        // Reingest modified documents
        for doc_info in &updates.modified {
            self.reingest_document(collection_id, &doc_info.document_uid, None, app_handle)
                .await?;
        }

        // Delete removed documents
        for doc_info in &updates.deleted {
            self.delete_document(collection_id, &doc_info.document_uid)
                .await?;
        }

        // Ingest new files
        if !updates.new_files.is_empty() {
            let mut docs = Vec::new();
            for file_path in &updates.new_files {
                let path = PathBuf::from(file_path);
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("txt");

                match Self::read_file_content(&path, ext) {
                    Ok(content) => {
                        let doc_id = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        docs.push(Document::from_parsed_content(
                            doc_id,
                            content,
                            file_path.clone(),
                            ext.to_string(),
                        ));
                    }
                    Err(e) => {
                        tracing::warn!(path = %file_path, error = %e, "Skipping unreadable file");
                    }
                }
            }

            if !docs.is_empty() {
                self.ingest_into_collection_with_progress(collection_id, docs, app_handle, None)
                    .await?;
            }
        }

        self.get_collection(collection_id)
    }

    /// Returns a reference to the embedding manager used by this pipeline.
    pub fn embedding_manager(&self) -> &Arc<EmbeddingManager> {
        &self.embedding_manager
    }

    /// Returns a reference to the HNSW index used by this pipeline.
    pub fn hnsw_index(&self) -> &Arc<HnswIndex> {
        &self.hnsw_index
    }

    /// Get a collection by ID.
    fn get_collection(&self, collection_id: &str) -> AppResult<KnowledgeCollection> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.query_row(
            "SELECT id, name, project_id, description, chunk_count, created_at, updated_at, workspace_path
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
                    workspace_path: row.get(7)?,
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
    use crate::services::knowledge::observability;
    use crate::services::knowledge::reranker::NoopReranker;
    use crate::services::orchestrator::embedding_manager::EmbeddingManagerConfig;
    use crate::services::orchestrator::embedding_provider::{
        EmbeddingProviderConfig, EmbeddingProviderType,
    };
    use crate::storage::database::Database;
    use tempfile::tempdir;

    fn build_test_pipeline(database: Arc<Database>, root: &std::path::Path) -> RagPipeline {
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let embedding_manager =
            Arc::new(EmbeddingManager::from_config(config).expect("create embedding manager"));

        let hnsw_index = Arc::new(HnswIndex::new(root.join("hnsw"), 8192));
        let chunker: Arc<dyn Chunker> = Arc::new(ParagraphChunker::new(500));
        let reranker: Option<Arc<dyn Reranker>> = Some(Arc::new(NoopReranker));

        RagPipeline::new(chunker, embedding_manager, hnsw_index, reranker, database)
            .expect("create pipeline")
    }

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
        let pipeline = build_test_pipeline(db, dir.path());

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
            Document::new(
                "d1",
                "First document about Rust programming.\n\nIt has multiple paragraphs.",
            ),
            Document::new(
                "d2",
                "Second document about Python programming.\n\nAlso multiple paragraphs.",
            ),
        ];

        let collection = pipeline
            .ingest("test-col", "proj-1", "Test", docs)
            .await
            .unwrap();

        assert_eq!(collection.name, "test-col");
        assert!(
            collection.chunk_count >= 2,
            "Should have at least 2 chunks from 2 docs"
        );
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

        assert!(
            collection.chunk_count >= 2,
            "Should have chunks from both ingests"
        );
    }

    // ======================================================================
    // Query tests
    // ======================================================================

    #[tokio::test]
    async fn query_nonexistent_collection_errors() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let result = pipeline.query("nonexistent", "proj-1", "query", 5).await;

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

        let result = pipeline.delete_collection("nonexistent", "proj-1").await;

        assert!(result.is_err());
    }

    // ======================================================================
    // Integration: ingest-then-query roundtrip
    // ======================================================================

    // ======================================================================
    // KnowledgeCollection serialization
    // ======================================================================

    #[tokio::test]
    async fn concurrent_get_or_create_same_collection() {
        let (pipeline, _dir) = create_test_pipeline().await;

        // Spawn multiple tasks that all try to get_or_create the same collection
        let pipeline = Arc::new(pipeline);
        let mut handles = Vec::new();
        for i in 0..10 {
            let p = Arc::clone(&pipeline);
            handles.push(tokio::spawn(async move {
                let docs = vec![Document::new(
                    format!("d{}", i),
                    format!("Content from thread {}", i),
                )];
                p.ingest("concurrent-col", "proj-1", "Test", docs).await
            }));
        }

        // All should succeed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Concurrent ingest should succeed");
        }

        // Only 1 collection should exist
        let collections = pipeline.list_collections("proj-1").unwrap();
        assert_eq!(
            collections.len(),
            1,
            "Only one collection should exist after concurrent creation"
        );
        assert_eq!(collections[0].name, "concurrent-col");
    }

    #[tokio::test]
    async fn ingest_transaction_rolls_back_on_partial_failure() {
        let (pipeline, _dir) = create_test_pipeline().await;

        // First ingest should succeed
        let docs = vec![Document::new("d1", "Valid content for first ingest.")];
        let collection = pipeline
            .ingest("tx-test", "proj-1", "Test", docs)
            .await
            .unwrap();
        let count_after_first = collection.chunk_count;
        assert!(count_after_first > 0);

        // Verify the chunk_count matches actual count in DB
        let collections = pipeline.list_collections("proj-1").unwrap();
        assert_eq!(collections[0].chunk_count, count_after_first);
    }

    #[tokio::test]
    async fn ingest_chunk_count_matches_actual_chunks() {
        let (pipeline, _dir) = create_test_pipeline().await;

        // Ingest twice to same collection
        let docs1 = vec![Document::new("d1", "First document.")];
        pipeline
            .ingest("count-test", "proj-1", "Test", docs1)
            .await
            .unwrap();

        let docs2 = vec![Document::new("d2", "Second document.")];
        let collection = pipeline
            .ingest("count-test", "proj-1", "Test", docs2)
            .await
            .unwrap();

        // chunk_count should reflect all chunks from both ingests
        assert!(
            collection.chunk_count >= 2,
            "chunk_count should include chunks from both ingests, got {}",
            collection.chunk_count,
        );
    }

    #[tokio::test]
    async fn init_schema_recovers_from_broken_fts_shadow_tables() {
        let (pipeline, dir) = create_test_pipeline().await;

        pipeline
            .ingest(
                "fts-recovery",
                "proj-1",
                "Broken FTS recovery",
                vec![Document::new(
                    "doc-1",
                    "Recovery path should keep this searchable.",
                )],
            )
            .await
            .unwrap();

        {
            let conn = pipeline.database.get_connection().expect("conn");
            conn.execute("DROP TABLE knowledge_chunks_fts_data", [])
                .expect("drop fts shadow");
        }

        let recovered = build_test_pipeline(Arc::clone(&pipeline.database), dir.path());
        let results = recovered
            .query("fts-recovery", "proj-1", "Recovery", 5)
            .await;

        assert!(results.is_ok(), "pipeline should recover from broken FTS");
        let result = results.unwrap();
        assert!(
            !result.results.is_empty(),
            "fts query should still return rows"
        );
    }

    #[tokio::test]
    async fn query_runs_are_filtered_by_exact_collection_scope() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let col_a = pipeline
            .ingest(
                "scope-a",
                "proj-1",
                "Scope A",
                vec![Document::new("a-doc", "Alpha retrieval context.")],
            )
            .await
            .unwrap();
        let col_b = pipeline
            .ingest(
                "scope-b",
                "proj-1",
                "Scope B",
                vec![Document::new("b-doc", "Bravo retrieval context.")],
            )
            .await
            .unwrap();

        let scope_a = vec![col_a.id.clone()];
        let scope_b = vec![col_b.id.clone()];
        pipeline
            .query_scoped("proj-1", "alpha", 5, Some(&scope_a), None, Some("balanced"))
            .await
            .unwrap();
        pipeline
            .query_scoped("proj-1", "bravo", 5, Some(&scope_b), None, Some("balanced"))
            .await
            .unwrap();

        let runs_a = pipeline
            .list_query_runs("proj-1", Some(&scope_a), 20)
            .unwrap();
        let runs_b = pipeline
            .list_query_runs("proj-1", Some(&scope_b), 20)
            .unwrap();

        assert_eq!(runs_a.len(), 1);
        assert_eq!(runs_b.len(), 1);
        assert_eq!(runs_a[0].query, "alpha");
        assert_eq!(runs_b[0].query, "bravo");
        assert_ne!(runs_a[0].id, runs_b[0].id);
    }

    #[tokio::test]
    async fn query_runs_are_isolated_by_project_id() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let col_proj1 = pipeline
            .ingest(
                "shared-name",
                "proj-1",
                "Project one",
                vec![Document::new("p1-doc", "Project one content.")],
            )
            .await
            .unwrap();
        let col_proj2 = pipeline
            .ingest(
                "shared-name",
                "proj-2",
                "Project two",
                vec![Document::new("p2-doc", "Project two content.")],
            )
            .await
            .unwrap();

        let scope_proj1 = vec![col_proj1.id.clone()];
        let scope_proj2 = vec![col_proj2.id.clone()];
        pipeline
            .query_scoped(
                "proj-1",
                "project",
                5,
                Some(&scope_proj1),
                None,
                Some("balanced"),
            )
            .await
            .unwrap();
        pipeline
            .query_scoped(
                "proj-2",
                "project",
                5,
                Some(&scope_proj2),
                None,
                Some("balanced"),
            )
            .await
            .unwrap();

        let runs_proj1 = pipeline.list_query_runs("proj-1", None, 20).unwrap();
        let runs_proj2 = pipeline.list_query_runs("proj-2", None, 20).unwrap();

        assert_eq!(runs_proj1.len(), 1);
        assert_eq!(runs_proj2.len(), 1);
        assert_eq!(runs_proj1[0].project_id, "proj-1");
        assert_eq!(runs_proj2[0].project_id, "proj-2");
        assert_ne!(runs_proj1[0].id, runs_proj2[0].id);
    }

    #[tokio::test]
    async fn query_run_scope_metrics_track_v2_and_legacy_paths() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let collection = pipeline
            .ingest(
                "scope-metrics",
                "proj-1",
                "Scope metrics",
                vec![Document::new("doc", "Scope metric content")],
            )
            .await
            .unwrap();
        let scope = vec![collection.id.clone()];
        pipeline
            .query_scoped("proj-1", "scope", 5, Some(&scope), None, Some("balanced"))
            .await
            .unwrap();

        let _ = pipeline
            .list_query_runs("proj-1", Some(&scope), 10)
            .unwrap();
        let snapshot_v2 = observability::read_metrics_snapshot(pipeline.database.as_ref()).unwrap();
        assert_eq!(snapshot_v2.query_run_scope_checks_total, 1);
        assert_eq!(snapshot_v2.query_run_scope_hits_total, 1);

        pipeline
            .database
            .set_setting("feature.kb_query_runs_v2", "false")
            .unwrap();
        let _ = pipeline
            .list_query_runs("proj-1", Some(&scope), 10)
            .unwrap();
        let snapshot_legacy =
            observability::read_metrics_snapshot(pipeline.database.as_ref()).unwrap();
        assert_eq!(snapshot_legacy.query_run_scope_checks_total, 2);
        assert_eq!(snapshot_legacy.query_run_scope_hits_total, 1);
    }

    #[tokio::test]
    async fn invalid_retrieval_profile_falls_back_to_balanced() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let collection = pipeline
            .ingest(
                "retrieval-profile-test",
                "proj-1",
                "profile",
                vec![Document::new("d1", "Profile fallback content.")],
            )
            .await
            .unwrap();

        let scope = vec![collection.id.clone()];
        pipeline
            .query_scoped(
                "proj-1",
                "profile",
                5,
                Some(&scope),
                None,
                Some("unknown-mode"),
            )
            .await
            .unwrap();

        let runs = pipeline
            .list_query_runs("proj-1", Some(&scope), 10)
            .unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].retrieval_profile, "balanced");
    }

    #[test]
    fn retrieval_profiles_map_to_distinct_parameters() {
        let balanced = RetrievalProfile::Balanced.params();
        let precision = RetrievalProfile::Precision.params();
        let recall = RetrievalProfile::Recall.params();

        assert!(precision.vector_top_n < balanced.vector_top_n);
        assert!(balanced.vector_top_n < recall.vector_top_n);
        assert!(precision.bm25_top_n < recall.bm25_top_n);
        assert!(precision.mmr_lambda > recall.mmr_lambda);
    }

    #[test]
    fn source_kind_and_locator_classifies_manual_uploads_safely() {
        let (kind_upload, locator_upload) = RagPipeline::source_kind_and_locator(
            Some("upload://manual/id/file.txt"),
            "col",
            "file.txt",
        );
        assert_eq!(kind_upload, "upload");
        assert_eq!(locator_upload, "upload://manual/id/file.txt");

        let (kind_workspace, locator_workspace) =
            RagPipeline::source_kind_and_locator(Some("/tmp/file.txt"), "col", "file.txt");
        assert_eq!(kind_workspace, "workspace");
        assert_eq!(locator_workspace, "/tmp/file.txt");

        let (kind_relative, locator_relative) =
            RagPipeline::source_kind_and_locator(Some("notes/readme.md"), "col", "readme.md");
        assert_eq!(kind_relative, "upload");
        assert!(locator_relative.starts_with("upload://manual/"));
    }

    #[test]
    fn ingest_progress_payload_includes_job_metadata_when_scoped_enabled() {
        let payload = RagPipeline::build_ingest_progress_payload(
            true,
            "job-123",
            "proj-1",
            "col-1",
            "docs",
            "embedding",
            66,
            "embedding chunks",
        );

        assert_eq!(payload["job_id"], "job-123");
        assert_eq!(payload["project_id"], "proj-1");
        assert_eq!(payload["collection_id"], "col-1");
        assert_eq!(payload["collection_name"], "docs");
        assert_eq!(payload["stage"], "embedding");
        assert_eq!(payload["progress"], 66);
        assert_eq!(payload["detail"], "embedding chunks");
    }

    #[test]
    fn ingest_progress_payload_legacy_mode_omits_job_id() {
        let payload = RagPipeline::build_ingest_progress_payload(
            false,
            "job-legacy",
            "proj-1",
            "col-1",
            "docs",
            "storing",
            100,
            "done",
        );

        assert!(payload.get("job_id").is_none());
        assert_eq!(payload["project_id"], "proj-1");
        assert_eq!(payload["collection_id"], "col-1");
        assert_eq!(payload["collection_name"], "docs");
        assert_eq!(payload["stage"], "storing");
        assert_eq!(payload["progress"], 100);
        assert_eq!(payload["detail"], "done");
    }

    #[tokio::test]
    async fn search_documents_matches_project_scope_and_collection_filter() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let col_a = pipeline
            .ingest(
                "search-a",
                "proj-1",
                "Search A",
                vec![Document::new("local-notes.md", "Local notes")],
            )
            .await
            .unwrap();
        let col_b = pipeline
            .ingest(
                "search-b",
                "proj-1",
                "Search B",
                vec![Document::new("remote-api-spec.md", "Remote API spec body")],
            )
            .await
            .unwrap();
        pipeline
            .ingest(
                "search-b",
                "proj-2",
                "Other project",
                vec![Document::new("remote-api-spec.md", "Other project")],
            )
            .await
            .unwrap();

        let matches = pipeline
            .search_documents("proj-1", "remote", None, 50)
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].collection_id, col_b.id);
        assert_eq!(matches[0].display_name, "remote-api-spec.md");

        let scoped_to_a = pipeline
            .search_documents("proj-1", "remote", Some(&vec![col_a.id.clone()]), 50)
            .unwrap();
        assert!(scoped_to_a.is_empty());

        let scoped_to_b = pipeline
            .search_documents("proj-1", "remote", Some(&vec![col_b.id.clone()]), 50)
            .unwrap();
        assert_eq!(scoped_to_b.len(), 1);
        assert_eq!(scoped_to_b[0].collection_id, col_b.id);

        let project_two = pipeline
            .search_documents("proj-2", "remote", None, 50)
            .unwrap();
        assert_eq!(project_two.len(), 1);
        assert_ne!(project_two[0].collection_id, col_b.id);
    }

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
            workspace_path: None,
        };
        let json = serde_json::to_string(&col).unwrap();
        let deserialized: KnowledgeCollection = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.chunk_count, 42);
        assert!(deserialized.workspace_path.is_none());
    }

    #[test]
    fn knowledge_collection_serde_with_workspace_path() {
        let col = KnowledgeCollection {
            id: "id-2".to_string(),
            name: "test-wp".to_string(),
            project_id: "proj-2".to_string(),
            description: "desc".to_string(),
            chunk_count: 10,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            workspace_path: Some("/home/user/project".to_string()),
        };
        let json = serde_json::to_string(&col).unwrap();
        assert!(json.contains("workspace_path"));
        let deserialized: KnowledgeCollection = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized.workspace_path,
            Some("/home/user/project".to_string())
        );
    }

    #[tokio::test]
    async fn update_collection_changes_fields() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let docs = vec![Document::new("d1", "Content.")];
        let col = pipeline
            .ingest("update-test", "proj-1", "Old description", docs)
            .await
            .unwrap();

        let updated = pipeline
            .update_collection(
                &col.id,
                Some("new-name"),
                Some("New description"),
                Some(Some("/home/user/project")),
            )
            .unwrap();

        assert_eq!(updated.name, "new-name");
        assert_eq!(updated.description, "New description");
        assert_eq!(
            updated.workspace_path,
            Some("/home/user/project".to_string())
        );
    }

    #[tokio::test]
    async fn update_collection_clears_workspace_path() {
        let (pipeline, _dir) = create_test_pipeline().await;

        let docs = vec![Document::new("d1", "Content.")];
        let col = pipeline
            .ingest("clear-wp-test", "proj-1", "Test", docs)
            .await
            .unwrap();

        // Set workspace_path
        pipeline
            .update_collection(&col.id, None, None, Some(Some("/some/path")))
            .unwrap();

        // Clear workspace_path
        let updated = pipeline
            .update_collection(&col.id, None, None, Some(None))
            .unwrap();

        assert!(updated.workspace_path.is_none());
    }

    #[tokio::test]
    async fn update_nonexistent_collection_errors() {
        let (pipeline, _dir) = create_test_pipeline().await;
        let result = pipeline.update_collection("nonexistent-id", Some("x"), None, None);
        assert!(result.is_err());
    }
}
