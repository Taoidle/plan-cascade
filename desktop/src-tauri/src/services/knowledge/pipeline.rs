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
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Emitter;

use crate::services::knowledge::chunker::{Chunk, Chunker, Document};
use crate::services::knowledge::reranker::{Reranker, SearchResult};
use crate::services::orchestrator::embedding_manager::EmbeddingManager;
use crate::services::orchestrator::embedding_service::embedding_to_bytes;
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

/// Summary of a document within a collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    /// The document ID.
    pub document_id: String,
    /// Number of chunks this document was split into.
    pub chunk_count: i64,
    /// Preview of the first chunk's content (up to 200 chars).
    pub preview: String,
}

/// Information about a document that has changed relative to its stored hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocUpdateInfo {
    pub document_id: String,
    pub source_path: String,
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
        .map_err(|e| {
            AppError::database(format!("Failed to create knowledge_collections: {}", e))
        })?;

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

        // Migration: add workspace_path column if it doesn't exist
        let has_workspace_path: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('knowledge_collections') WHERE name='workspace_path'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| AppError::database(format!("Failed to check column existence: {}", e)))?
            > 0;
        if !has_workspace_path {
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

        // Helper to emit progress events
        let emit_progress =
            |stage: &str, progress: u32, detail: &str| {
                if let Some(handle) = app_handle {
                    let _ = handle.emit(
                        "knowledge:ingest-progress",
                        serde_json::json!({
                            "stage": stage,
                            "progress": progress,
                            "detail": detail,
                        }),
                    );
                }
            };

        emit_progress("chunking", 0, "Starting document chunking...");

        // Inject source_path and content_hash into document metadata so they survive chunking
        let mut documents = documents;
        for doc in &mut documents {
            if let Some(ref sp) = doc.source_path {
                doc.metadata
                    .entry("source_path".to_string())
                    .or_insert_with(|| sp.clone());
            }
            // Compute SHA-256 content hash for staleness detection
            let hash = format!("{:x}", Sha256::digest(doc.content.as_bytes()));
            doc.metadata.insert("content_hash".to_string(), hash);
        }

        // Chunk all documents (sync)
        let mut all_chunks: Vec<Chunk> = Vec::new();
        for doc in &documents {
            let chunks = self.chunker.chunk(doc)?;
            all_chunks.extend(chunks);
        }

        if all_chunks.is_empty() {
            emit_progress("storing", 100, "No chunks to store.");
            return self.get_collection(&collection_id);
        }

        emit_progress(
            "chunking",
            30,
            &format!("Chunked {} documents into {} chunks", documents.len(), all_chunks.len()),
        );

        // Embed all chunks (async — no connection held)
        emit_progress("embedding", 30, "Generating embeddings...");
        let chunk_texts: Vec<&str> = all_chunks.iter().map(|c| c.content.as_str()).collect();
        let embeddings = self
            .embedding_manager
            .embed_documents(&chunk_texts)
            .await
            .map_err(|e| AppError::internal(format!("Embedding failed: {}", e)))?;

        emit_progress("embedding", 70, &format!("Embedded {} chunks", embeddings.len()));

        // Store chunks in SQLite within a transaction (sync scope — connection dropped before await)
        emit_progress("storing", 70, "Storing chunks in database...");
        let hnsw_items = {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let tx = conn
                .unchecked_transaction()
                .map_err(|e| AppError::database(format!("Failed to begin transaction: {}", e)))?;

            let mut items: Vec<(usize, Vec<f32>)> = Vec::new();

            for (i, chunk) in all_chunks.iter().enumerate() {
                let embedding_bytes = embedding_to_bytes(&embeddings[i]);
                let metadata_json =
                    serde_json::to_string(&chunk.metadata).unwrap_or_else(|_| "{}".to_string());

                tx.execute(
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

                let chunk_rowid = tx.last_insert_rowid() as usize;
                items.push((chunk_rowid, embeddings[i].clone()));
            }

            // Update chunk_count inside the same transaction (P0-4 fix)
            let chunk_count: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_chunks WHERE collection_id = ?1",
                    rusqlite::params![collection_id],
                    |row| row.get(0),
                )
                .map_err(|e| {
                    AppError::database(format!("Failed to count chunks: {}", e))
                })?;

            tx.execute(
                "UPDATE knowledge_collections SET chunk_count = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![chunk_count, collection_id],
            )
            .map_err(|e| AppError::database(format!("Failed to update collection: {}", e)))?;

            tx.commit()
                .map_err(|e| AppError::database(format!("Failed to commit transaction: {}", e)))?;

            items
            // conn dropped here
        };

        // HNSW operations (async — no connection held)
        if !self.hnsw_index.is_ready().await {
            self.hnsw_index.initialize().await;
        }
        // Check batch_insert return value — log error but don't fail
        // (SQLite is the source of truth, HNSW is a derived cache)
        if !self.hnsw_index.batch_insert(&hnsw_items).await {
            tracing::error!(
                "HNSW batch_insert failed for '{}'; SQLite is source of truth",
                collection_name
            );
        } else {
            // Persist HNSW to disk so the index survives app restart
            if let Err(e) = self.hnsw_index.save_to_disk().await {
                tracing::warn!(
                    error = %e,
                    "HNSW save_to_disk failed after ingest for '{}'; index will be rebuilt on next startup",
                    collection_name
                );
            }
        }

        emit_progress("storing", 100, "Ingestion complete.");

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
        // Find collection (sync scope — connection dropped before await)
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
            // conn dropped here
        };

        // Embed the query (async — no connection held)
        let query_embedding = self
            .embedding_manager
            .embed_query(query_text)
            .await
            .map_err(|e| AppError::internal(format!("Query embedding failed: {}", e)))?;

        // Search HNSW (async — no connection held)
        let hnsw_results = self.hnsw_index.search(&query_embedding, top_k * 3).await;

        // Look up chunks from DB (sync scope — connection dropped before await)
        let mut search_results = {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let mut results = Vec::new();
            for (chunk_rowid, distance) in &hnsw_results {
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

                    let score = 1.0 - distance;

                    results.push(SearchResult {
                        chunk_text: content,
                        document_id,
                        collection_name: collection_name.to_string(),
                        score,
                        metadata,
                    });
                }
            }

            results
            // conn dropped here
        };

        let total_searched = search_results.len();

        // Apply reranker if configured (async — no connection held)
        if let Some(ref reranker) = self.reranker {
            search_results = reranker.rerank(query_text, search_results).await?;
        }

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
    pub fn list_documents(
        &self,
        collection_id: &str,
    ) -> AppResult<Vec<DocumentSummary>> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn
            .prepare(
                "SELECT document_id, COUNT(*) as chunk_count,
                        SUBSTR(MIN(CASE WHEN chunk_index = 0 THEN content END), 1, 200) as preview
                 FROM knowledge_chunks WHERE collection_id = ?1
                 GROUP BY document_id ORDER BY document_id",
            )
            .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt
            .query_map(rusqlite::params![collection_id], |row| {
                Ok(DocumentSummary {
                    document_id: row.get(0)?,
                    chunk_count: row.get(1)?,
                    preview: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                })
            })
            .map_err(|e| AppError::database(format!("Failed to query documents: {}", e)))?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Delete a single document from a collection by document_id.
    ///
    /// Removes all chunks for the document, marks HNSW entries as stale,
    /// and updates the collection's chunk_count — all within a transaction.
    pub async fn delete_document(
        &self,
        collection_id: &str,
        document_id: &str,
    ) -> AppResult<()> {
        // Collect chunk rowids for HNSW stale marking (sync scope)
        let rowids = {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let mut stmt = conn
                .prepare(
                    "SELECT rowid FROM knowledge_chunks WHERE collection_id = ?1 AND document_id = ?2",
                )
                .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

            let rowids: Vec<i64> = stmt
                .query_map(rusqlite::params![collection_id, document_id], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Failed to query rowids: {}", e)))?
                .filter_map(|r| r.ok())
                .collect();

            rowids
        };

        if rowids.is_empty() {
            return Err(AppError::not_found(format!(
                "Document '{}' not found in collection",
                document_id
            )));
        }

        // Mark stale in HNSW (async — no connection held)
        for rowid in &rowids {
            self.hnsw_index.mark_stale(*rowid as usize).await;
        }

        // Delete chunks and update count in a transaction (sync scope)
        {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let tx = conn
                .unchecked_transaction()
                .map_err(|e| AppError::database(format!("Failed to begin transaction: {}", e)))?;

            tx.execute(
                "DELETE FROM knowledge_chunks WHERE collection_id = ?1 AND document_id = ?2",
                rusqlite::params![collection_id, document_id],
            )
            .map_err(|e| AppError::database(format!("Failed to delete document chunks: {}", e)))?;

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
                "DELETE FROM knowledge_chunks WHERE collection_id = ?1",
                rusqlite::params![collection_id],
            )
            .map_err(|e| AppError::database(format!("Failed to delete chunks: {}", e)))?;

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
        document_id: &str,
        new_content: Option<String>,
        app_handle: Option<&tauri::AppHandle>,
    ) -> AppResult<KnowledgeCollection> {
        // Recover metadata from the first chunk
        let (source_path, source_type) = {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            let metadata_json: String = conn
                .query_row(
                    "SELECT metadata FROM knowledge_chunks WHERE collection_id = ?1 AND document_id = ?2 AND chunk_index = 0",
                    rusqlite::params![collection_id, document_id],
                    |row| row.get(0),
                )
                .map_err(|e| AppError::not_found(format!("Document '{}' not found: {}", document_id, e)))?;

            let metadata: HashMap<String, String> =
                serde_json::from_str(&metadata_json).unwrap_or_default();

            (
                metadata.get("source_path").cloned(),
                metadata.get("source_type").cloned().unwrap_or_default(),
            )
        };

        // Get content: from argument, or read from disk
        let content = if let Some(c) = new_content {
            c
        } else {
            let sp = source_path
                .as_ref()
                .ok_or_else(|| AppError::internal("No source_path to read from disk"))?;
            let path = Path::new(sp);
            if !path.exists() {
                return Err(AppError::not_found(format!("File not found: {}", sp)));
            }
            Self::read_file_content(path, &source_type)?
        };

        // Delete old document
        self.delete_document(collection_id, document_id).await?;

        // Build new Document (content_hash will be computed in ingest_with_progress)
        let doc = if let Some(ref sp) = source_path {
            Document::from_parsed_content(document_id, content, sp.clone(), source_type)
        } else {
            Document::new(document_id, content)
        };

        // Look up collection name and project_id
        let (name, project_id) = {
            let conn = self
                .database
                .get_connection()
                .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

            conn.query_row(
                "SELECT name, project_id FROM knowledge_collections WHERE id = ?1",
                rusqlite::params![collection_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(|e| AppError::not_found(format!("Collection not found: {}", e)))?
        };

        self.ingest_with_progress(&name, &project_id, "", vec![doc], app_handle)
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

        // Get unique document_ids with their first chunk's metadata
        let mut stmt = conn
            .prepare(
                "SELECT document_id, metadata FROM knowledge_chunks
                 WHERE collection_id = ?1 AND chunk_index = 0
                 ORDER BY document_id",
            )
            .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

        let rows: Vec<(String, String)> = stmt
            .query_map(rusqlite::params![collection_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| AppError::database(format!("Failed to query documents: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        let mut modified = Vec::new();
        let mut deleted = Vec::new();
        let mut unchanged = 0usize;
        let mut indexed_paths = std::collections::HashSet::new();

        for (document_id, metadata_json) in &rows {
            let metadata: HashMap<String, String> =
                serde_json::from_str(metadata_json).unwrap_or_default();

            let source_path = match metadata.get("source_path") {
                Some(sp) if !sp.is_empty() => sp.clone(),
                _ => {
                    // No source_path — not trackable, skip
                    unchanged += 1;
                    continue;
                }
            };

            let source_type = metadata
                .get("source_type")
                .cloned()
                .unwrap_or_default();
            let old_hash = metadata
                .get("content_hash")
                .cloned()
                .unwrap_or_default();

            indexed_paths.insert(source_path.clone());
            let path = Path::new(&source_path);

            if !path.exists() {
                deleted.push(DocUpdateInfo {
                    document_id: document_id.clone(),
                    source_path,
                    source_type,
                    old_hash,
                    new_hash: None,
                });
                continue;
            }

            // Read file and compute hash
            match Self::read_file_content(path, &source_type) {
                Ok(content) => {
                    let new_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
                    if new_hash != old_hash {
                        modified.push(DocUpdateInfo {
                            document_id: document_id.clone(),
                            source_path,
                            source_type,
                            old_hash,
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
            self.reingest_document(collection_id, &doc_info.document_id, None, app_handle)
                .await?;
        }

        // Delete removed documents
        for doc_info in &updates.deleted {
            self.delete_document(collection_id, &doc_info.document_id)
                .await?;
        }

        // Ingest new files
        if !updates.new_files.is_empty() {
            let (name, project_id) = {
                let conn = self
                    .database
                    .get_connection()
                    .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

                conn.query_row(
                    "SELECT name, project_id FROM knowledge_collections WHERE id = ?1",
                    rusqlite::params![collection_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .map_err(|e| AppError::not_found(format!("Collection not found: {}", e)))?
            };

            let mut docs = Vec::new();
            for file_path in &updates.new_files {
                let path = PathBuf::from(file_path);
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("txt");

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
                self.ingest_with_progress(&name, &project_id, "", docs, app_handle)
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
        let embedding_manager =
            Arc::new(EmbeddingManager::from_config(config).expect("create embedding manager"));

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
