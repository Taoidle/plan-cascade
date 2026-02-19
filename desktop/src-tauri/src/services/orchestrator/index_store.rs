//! SQLite Persistent Index Storage
//!
//! Provides persistent storage for file index data, enabling fast lookups
//! of symbols, components, and project summaries across analysis runs.
//! Uses the shared DbPool from `storage::database`.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::storage::database::DbPool;
use crate::utils::error::{AppError, AppResult};

use super::analysis_index::{FileInventoryItem, SymbolInfo, SymbolKind};
use super::embedding_service::{bytes_to_embedding, cosine_similarity, SemanticSearchResult};

/// Result of a symbol query, including the file it belongs to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMatch {
    pub file_path: String,
    pub project_path: String,
    pub symbol_name: String,
    pub symbol_kind: String,
    pub line_number: usize,
    /// Parent symbol (e.g., class for a method)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_symbol: Option<String>,
    /// Function/method signature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Documentation comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_comment: Option<String>,
    /// End line of the symbol
    #[serde(default)]
    pub end_line: usize,
}

/// Component entry in a project summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentSummary {
    pub name: String,
    pub count: usize,
}

/// Aggregate statistics for an indexed project.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectIndexSummary {
    pub total_files: usize,
    pub languages: Vec<String>,
    pub components: Vec<ComponentSummary>,
    pub key_entry_points: Vec<String>,
    /// Total number of symbols (functions, classes, structs, etc.) across all files.
    #[serde(default)]
    pub total_symbols: usize,
    /// Total number of embedding chunks stored for this project.
    /// When > 0, semantic search is available.
    #[serde(default)]
    pub embedding_chunks: usize,
}

/// Metadata about stored embeddings: which provider and dimension were used.
///
/// Returned by `IndexStore::get_embedding_metadata` to help callers decide
/// whether existing embeddings are compatible with the current provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingMetadata {
    pub provider_type: String,
    pub provider_model: String,
    pub embedding_dimension: usize,
}

/// Persistent index store backed by SQLite.
#[derive(Debug, Clone)]
pub struct IndexStore {
    pool: DbPool,
}

impl IndexStore {
    /// Create a new IndexStore wrapping the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Insert or update a file index entry along with its symbols.
    ///
    /// Uses `INSERT ... ON CONFLICT` to upsert the file_index row, then
    /// replaces all symbols for that file (delete + re-insert).
    pub fn upsert_file_index(
        &self,
        project_path: &str,
        item: &FileInventoryItem,
        content_hash: &str,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;

        // Enable foreign keys for this connection
        conn.execute_batch("PRAGMA foreign_keys = ON")?;

        // Upsert the file_index row
        conn.execute(
            "INSERT INTO file_index (project_path, file_path, component, language, extension,
                                     size_bytes, line_count, is_test, content_hash, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, CURRENT_TIMESTAMP)
             ON CONFLICT(project_path, file_path) DO UPDATE SET
                 component = excluded.component,
                 language = excluded.language,
                 extension = excluded.extension,
                 size_bytes = excluded.size_bytes,
                 line_count = excluded.line_count,
                 is_test = excluded.is_test,
                 content_hash = excluded.content_hash,
                 indexed_at = CURRENT_TIMESTAMP",
            params![
                project_path,
                item.path,
                item.component,
                item.language,
                item.extension,
                item.size_bytes as i64,
                item.line_count as i64,
                item.is_test as i32,
                content_hash,
            ],
        )?;

        // Retrieve the file_index id
        let file_index_id: i64 = conn.query_row(
            "SELECT id FROM file_index WHERE project_path = ?1 AND file_path = ?2",
            params![project_path, item.path],
            |row| row.get(0),
        )?;

        // Collect old symbol rowids before deleting (for FTS cleanup)
        let old_symbol_ids: Vec<i64> = {
            let mut id_stmt = conn.prepare(
                "SELECT id FROM file_symbols WHERE file_index_id = ?1",
            )?;
            let mapped = id_stmt
                .query_map(params![file_index_id], |row| row.get::<_, i64>(0))?
                .filter_map(|r| r.ok())
                .collect();
            mapped
        };

        // Delete old FTS entries for these symbols (contentless mode: DELETE by rowid)
        if !old_symbol_ids.is_empty() {
            let mut fts_del = conn.prepare(
                "DELETE FROM symbol_fts WHERE rowid = ?1",
            )?;
            for id in &old_symbol_ids {
                let _ = fts_del.execute(params![id]);
            }
        }

        // Delete existing symbols for this file (cascade would handle on delete,
        // but for update we need to clear manually)
        conn.execute(
            "DELETE FROM file_symbols WHERE file_index_id = ?1",
            params![file_index_id],
        )?;

        // Insert new symbols with extended fields
        let mut stmt = conn.prepare(
            "INSERT INTO file_symbols (file_index_id, name, kind, line_number, parent_symbol, signature, doc_comment, start_line, end_line)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;

        for symbol in &item.symbols {
            let kind_str = symbol_kind_to_str(&symbol.kind);
            stmt.execute(params![
                file_index_id,
                symbol.name,
                kind_str,
                symbol.line as i64,
                symbol.parent,
                symbol.signature,
                symbol.doc_comment,
                symbol.line as i64, // start_line = line
                symbol.end_line as i64,
            ])?;
        }

        // Insert new FTS entries for symbols
        // Get the new symbol IDs (rowids) for FTS insertion
        {
            let mut sym_stmt = conn.prepare(
                "SELECT id, name, kind, COALESCE(doc_comment, ''), COALESCE(signature, '')
                 FROM file_symbols WHERE file_index_id = ?1",
            )?;
            let new_symbols: Vec<(i64, String, String, String, String)> = sym_stmt
                .query_map(params![file_index_id], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut fts_ins = conn.prepare(
                "INSERT INTO symbol_fts (rowid, symbol_name, file_path, symbol_kind, doc_comment, signature)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for (id, name, kind, doc, sig) in &new_symbols {
                fts_ins.execute(params![id, name, item.path, kind, doc, sig])?;
            }
        }

        // Sync filepath_fts: delete old entry for this file_index row, insert new one
        let _ = conn.execute(
            "DELETE FROM filepath_fts WHERE rowid = ?1",
            params![file_index_id],
        );
        conn.execute(
            "INSERT INTO filepath_fts (rowid, file_path, component, language)
             VALUES (?1, ?2, ?3, ?4)",
            params![file_index_id, item.path, item.component, item.language],
        )?;

        Ok(())
    }

    /// Query symbols whose name matches a SQL LIKE pattern.
    ///
    /// The `name_pattern` should use `%` as wildcard, e.g. `"%Controller%"`.
    pub fn query_symbols(&self, name_pattern: &str) -> AppResult<Vec<SymbolMatch>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT fi.file_path, fi.project_path, fs.name, fs.kind, fs.line_number,
                    fs.parent_symbol, fs.signature, fs.doc_comment, fs.end_line
             FROM file_symbols fs
             JOIN file_index fi ON fi.id = fs.file_index_id
             WHERE fs.name LIKE ?1
             ORDER BY fs.name, fi.file_path",
        )?;

        let rows = stmt
            .query_map(params![name_pattern], |row| {
                Ok(SymbolMatch {
                    file_path: row.get(0)?,
                    project_path: row.get(1)?,
                    symbol_name: row.get(2)?,
                    symbol_kind: row.get(3)?,
                    line_number: row.get::<_, i64>(4)? as usize,
                    parent_symbol: row.get(5)?,
                    signature: row.get(6)?,
                    doc_comment: row.get(7)?,
                    end_line: row.get::<_, i64>(8).unwrap_or(0) as usize,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Query all files belonging to a specific component within a project.
    pub fn query_files_by_component(
        &self,
        project_path: &str,
        component: &str,
    ) -> AppResult<Vec<FileIndexRow>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT id, project_path, file_path, component, language, extension,
                    size_bytes, line_count, is_test, content_hash, indexed_at
             FROM file_index
             WHERE project_path = ?1 AND component = ?2
             ORDER BY file_path",
        )?;

        let rows = stmt
            .query_map(params![project_path, component], |row| {
                Ok(FileIndexRow {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    file_path: row.get(2)?,
                    component: row.get(3)?,
                    language: row.get(4)?,
                    extension: row.get(5)?,
                    size_bytes: row.get::<_, i64>(6)? as u64,
                    line_count: row.get::<_, i64>(7)? as usize,
                    is_test: row.get::<_, i32>(8)? != 0,
                    content_hash: row.get(9)?,
                    indexed_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Compute aggregate statistics for an indexed project.
    pub fn get_project_summary(&self, project_path: &str) -> AppResult<ProjectIndexSummary> {
        let conn = self.get_connection()?;

        // Total file count
        let total_files: i64 = conn.query_row(
            "SELECT COUNT(*) FROM file_index WHERE project_path = ?1",
            params![project_path],
            |row| row.get(0),
        )?;

        // Distinct languages
        let mut lang_stmt = conn.prepare(
            "SELECT DISTINCT language FROM file_index
             WHERE project_path = ?1 AND language != ''
             ORDER BY language",
        )?;
        let languages: Vec<String> = lang_stmt
            .query_map(params![project_path], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Components with counts
        let mut comp_stmt = conn.prepare(
            "SELECT component, COUNT(*) as cnt FROM file_index
             WHERE project_path = ?1 AND component != ''
             GROUP BY component
             ORDER BY cnt DESC",
        )?;
        let components: Vec<ComponentSummary> = comp_stmt
            .query_map(params![project_path], |row| {
                Ok(ComponentSummary {
                    name: row.get(0)?,
                    count: row.get::<_, i64>(1)? as usize,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Key entry points: files with common entry-point names
        let mut entry_stmt = conn.prepare(
            "SELECT DISTINCT fi.file_path FROM file_index fi
             JOIN file_symbols fs ON fs.file_index_id = fi.id
             WHERE fi.project_path = ?1
               AND (fs.name IN ('main', 'app', 'index', 'run', 'start', 'setup', 'init')
                    OR fi.file_path LIKE '%main.%'
                    OR fi.file_path LIKE '%index.%'
                    OR fi.file_path LIKE '%app.%'
                    OR fi.file_path LIKE '%lib.%')
             ORDER BY fi.file_path
             LIMIT 20",
        )?;
        let key_entry_points: Vec<String> = entry_stmt
            .query_map(params![project_path], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Total symbol count across all files
        let total_symbols: i64 = conn.query_row(
            "SELECT COUNT(*) FROM file_symbols fs
             JOIN file_index fi ON fi.id = fs.file_index_id
             WHERE fi.project_path = ?1",
            params![project_path],
            |row| row.get(0),
        )?;

        // Embedding chunk count (indicates semantic search availability)
        let embedding_chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM file_embeddings WHERE project_path = ?1",
                params![project_path],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(ProjectIndexSummary {
            total_files: total_files as usize,
            languages,
            components,
            key_entry_points,
            total_symbols: total_symbols as usize,
            embedding_chunks: embedding_chunks as usize,
        })
    }

    /// Check whether a file's stored content hash differs from the given hash.
    ///
    /// Returns `true` if the file is not in the index or has a different hash
    /// (meaning it is stale and should be re-indexed).
    pub fn is_index_stale(
        &self,
        project_path: &str,
        file_path: &str,
        current_hash: &str,
    ) -> AppResult<bool> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT content_hash FROM file_index WHERE project_path = ?1 AND file_path = ?2",
            params![project_path, file_path],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(stored_hash) => Ok(stored_hash != current_hash),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(true),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Delete all index entries for a project, including FTS5 entries.
    pub fn delete_project_index(&self, project_path: &str) -> AppResult<usize> {
        let conn = self.get_connection()?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;

        // Collect file_index IDs and symbol IDs for FTS cleanup before deletion
        let file_ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM file_index WHERE project_path = ?1",
            )?;
            let mapped = stmt
                .query_map(params![project_path], |row| row.get::<_, i64>(0))?
                .filter_map(|r| r.ok())
                .collect();
            mapped
        };

        // Delete symbol_fts entries (by symbol rowids belonging to this project)
        if !file_ids.is_empty() {
            let mut sym_id_stmt = conn.prepare(
                "SELECT id FROM file_symbols WHERE file_index_id = ?1",
            )?;
            let mut fts_del = conn.prepare(
                "DELETE FROM symbol_fts WHERE rowid = ?1",
            )?;
            for file_id in &file_ids {
                let sym_ids: Vec<i64> = sym_id_stmt
                    .query_map(params![file_id], |row| row.get::<_, i64>(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                for sym_id in sym_ids {
                    let _ = fts_del.execute(params![sym_id]);
                }
            }

            // Delete filepath_fts entries (by file_index rowids)
            let mut fp_del = conn.prepare(
                "DELETE FROM filepath_fts WHERE rowid = ?1",
            )?;
            for file_id in &file_ids {
                let _ = fp_del.execute(params![file_id]);
            }
        }

        // Delete from file_index (cascades to file_symbols)
        let deleted = conn.execute(
            "DELETE FROM file_index WHERE project_path = ?1",
            params![project_path],
        )?;

        Ok(deleted)
    }

    /// Query files whose path matches a SQL LIKE pattern.
    ///
    /// The `path_pattern` should use `%` as wildcard, e.g. `"%controller%"`.
    /// Results are ordered by file_path for deterministic output.
    pub fn query_files_by_path(
        &self,
        project_path: &str,
        path_pattern: &str,
    ) -> AppResult<Vec<FileIndexRow>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT id, project_path, file_path, component, language, extension,
                    size_bytes, line_count, is_test, content_hash, indexed_at
             FROM file_index
             WHERE project_path = ?1 AND file_path LIKE ?2
             ORDER BY file_path",
        )?;

        let rows = stmt
            .query_map(params![project_path, path_pattern], |row| {
                Ok(FileIndexRow {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    file_path: row.get(2)?,
                    component: row.get(3)?,
                    language: row.get(4)?,
                    extension: row.get(5)?,
                    size_bytes: row.get::<_, i64>(6)? as u64,
                    line_count: row.get::<_, i64>(7)? as usize,
                    is_test: row.get::<_, i32>(8)? != 0,
                    content_hash: row.get(9)?,
                    indexed_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Get symbols for a specific file.
    pub fn get_file_symbols(
        &self,
        project_path: &str,
        file_path: &str,
    ) -> AppResult<Vec<SymbolInfo>> {
        let conn = self.get_connection()?;

        let file_index_id: i64 = match conn.query_row(
            "SELECT id FROM file_index WHERE project_path = ?1 AND file_path = ?2",
            params![project_path, file_path],
            |row| row.get(0),
        ) {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(Vec::new()),
            Err(e) => return Err(AppError::database(e.to_string())),
        };

        let mut stmt = conn.prepare(
            "SELECT name, kind, line_number, parent_symbol, signature, doc_comment, end_line
             FROM file_symbols
             WHERE file_index_id = ?1
             ORDER BY line_number",
        )?;

        let symbols = stmt
            .query_map(params![file_index_id], |row| {
                let name: String = row.get(0)?;
                let kind_str: String = row.get(1)?;
                let line: i64 = row.get(2)?;
                let parent: Option<String> = row.get(3)?;
                let signature: Option<String> = row.get(4)?;
                let doc_comment: Option<String> = row.get(5)?;
                let end_line: i64 = row.get::<_, i64>(6).unwrap_or(0);
                Ok(SymbolInfo {
                    name,
                    kind: str_to_symbol_kind(&kind_str),
                    line: line as usize,
                    parent,
                    signature,
                    doc_comment,
                    end_line: end_line as usize,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(symbols)
    }

    // =========================================================================
    // Embedding storage methods (feature-003, story-012)
    // =========================================================================

    /// Insert or update a chunk embedding for a file.
    ///
    /// Uses `INSERT ... ON CONFLICT` to upsert on the
    /// `(project_path, file_path, chunk_index)` unique constraint.
    ///
    /// The `provider_type`, `provider_model`, and `embedding_dimension` parameters
    /// are optional for backward compatibility. When `None`, they default to
    /// `"tfidf"`, `"tfidf-v1"`, and `0` respectively.
    pub fn upsert_chunk_embedding(
        &self,
        project_path: &str,
        file_path: &str,
        chunk_index: i64,
        chunk_text: &str,
        embedding: &[u8],
    ) -> AppResult<()> {
        self.upsert_chunk_embedding_with_provider(
            project_path,
            file_path,
            chunk_index,
            chunk_text,
            embedding,
            None,
            None,
            None,
        )
    }

    /// Insert or update a chunk embedding with explicit provider metadata.
    ///
    /// Extended version of `upsert_chunk_embedding` that records which embedding
    /// provider generated the vector, enabling multi-provider storage and filtering.
    pub fn upsert_chunk_embedding_with_provider(
        &self,
        project_path: &str,
        file_path: &str,
        chunk_index: i64,
        chunk_text: &str,
        embedding: &[u8],
        provider_type: Option<&str>,
        provider_model: Option<&str>,
        embedding_dimension: Option<i64>,
    ) -> AppResult<()> {
        let pt = provider_type.unwrap_or("tfidf");
        let pm = provider_model.unwrap_or("tfidf-v1");
        let ed = embedding_dimension.unwrap_or(0);

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO file_embeddings (project_path, file_path, chunk_index, chunk_text, embedding,
                                          provider_type, provider_model, embedding_dimension, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CURRENT_TIMESTAMP)
             ON CONFLICT(project_path, file_path, chunk_index) DO UPDATE SET
                 chunk_text = excluded.chunk_text,
                 embedding = excluded.embedding,
                 provider_type = excluded.provider_type,
                 provider_model = excluded.provider_model,
                 embedding_dimension = excluded.embedding_dimension,
                 created_at = CURRENT_TIMESTAMP",
            params![project_path, file_path, chunk_index, chunk_text, embedding, pt, pm, ed],
        )?;
        Ok(())
    }

    /// Retrieve all embeddings for a project.
    ///
    /// Returns a vector of `(file_path, chunk_index, chunk_text, embedding_bytes)`.
    ///
    /// When `provider_filter` is `None`, returns all embeddings regardless of
    /// provider. When `Some`, only returns embeddings matching that provider_type.
    pub fn get_embeddings_for_project(
        &self,
        project_path: &str,
    ) -> AppResult<Vec<(String, i64, String, Vec<u8>)>> {
        self.get_embeddings_for_project_filtered(project_path, None)
    }

    /// Retrieve embeddings for a project, optionally filtered by provider_type.
    pub fn get_embeddings_for_project_filtered(
        &self,
        project_path: &str,
        provider_filter: Option<&str>,
    ) -> AppResult<Vec<(String, i64, String, Vec<u8>)>> {
        let conn = self.get_connection()?;

        let rows = if let Some(provider) = provider_filter {
            let mut stmt = conn.prepare(
                "SELECT file_path, chunk_index, chunk_text, embedding
                 FROM file_embeddings
                 WHERE project_path = ?1 AND provider_type = ?2
                 ORDER BY file_path, chunk_index",
            )?;
            let result: Vec<_> = stmt
                .query_map(params![project_path, provider], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();
            result
        } else {
            let mut stmt = conn.prepare(
                "SELECT file_path, chunk_index, chunk_text, embedding
                 FROM file_embeddings
                 WHERE project_path = ?1
                 ORDER BY file_path, chunk_index",
            )?;
            let result: Vec<_> = stmt
                .query_map(params![project_path], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();
            result
        };

        Ok(rows)
    }

    /// Delete all embeddings for a specific file within a project.
    pub fn delete_embeddings_for_file(
        &self,
        project_path: &str,
        file_path: &str,
    ) -> AppResult<usize> {
        let conn = self.get_connection()?;
        let deleted = conn.execute(
            "DELETE FROM file_embeddings WHERE project_path = ?1 AND file_path = ?2",
            params![project_path, file_path],
        )?;
        Ok(deleted)
    }

    /// Delete all embeddings for a project.
    pub fn delete_embeddings_for_project(&self, project_path: &str) -> AppResult<usize> {
        let conn = self.get_connection()?;
        let deleted = conn.execute(
            "DELETE FROM file_embeddings WHERE project_path = ?1",
            params![project_path],
        )?;
        Ok(deleted)
    }

    /// Delete all embeddings for a project that match a specific provider_type.
    ///
    /// Useful for clearing stale embeddings when switching to a different
    /// embedding provider.
    pub fn delete_embeddings_by_provider(
        &self,
        project_path: &str,
        provider_type: &str,
    ) -> AppResult<usize> {
        let conn = self.get_connection()?;
        let deleted = conn.execute(
            "DELETE FROM file_embeddings WHERE project_path = ?1 AND provider_type = ?2",
            params![project_path, provider_type],
        )?;
        Ok(deleted)
    }

    /// Get metadata about stored embeddings for a project.
    ///
    /// Returns a list of distinct `(provider_type, provider_model, embedding_dimension)`
    /// combinations found in the stored embeddings. Useful for checking compatibility
    /// when deciding whether to re-embed or reuse existing vectors.
    pub fn get_embedding_metadata(
        &self,
        project_path: &str,
    ) -> AppResult<Vec<EmbeddingMetadata>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT provider_type, provider_model, embedding_dimension
             FROM file_embeddings
             WHERE project_path = ?1
             ORDER BY provider_type, provider_model",
        )?;

        let rows = stmt
            .query_map(params![project_path], |row| {
                Ok(EmbeddingMetadata {
                    provider_type: row.get(0)?,
                    provider_model: row.get(1)?,
                    embedding_dimension: row.get::<_, i64>(2)? as usize,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Perform a semantic search over stored embeddings.
    ///
    /// Computes cosine similarity between `query_embedding` and every stored
    /// embedding for the given project, returning the top-k results ranked by
    /// descending similarity.
    ///
    /// When `provider_filter` is `None`, searches all embeddings. When `Some`,
    /// only searches embeddings matching the given provider_type.
    pub fn semantic_search(
        &self,
        query_embedding: &[f32],
        project_path: &str,
        top_k: usize,
    ) -> AppResult<Vec<SemanticSearchResult>> {
        self.semantic_search_filtered(query_embedding, project_path, top_k, None)
    }

    /// Perform a semantic search with an optional provider filter.
    pub fn semantic_search_filtered(
        &self,
        query_embedding: &[f32],
        project_path: &str,
        top_k: usize,
        provider_filter: Option<&str>,
    ) -> AppResult<Vec<SemanticSearchResult>> {
        let rows = self.get_embeddings_for_project_filtered(project_path, provider_filter)?;

        if rows.is_empty() || query_embedding.is_empty() {
            return Ok(Vec::new());
        }

        // Score each chunk against the query
        let mut scored: Vec<SemanticSearchResult> = rows
            .into_iter()
            .map(|(file_path, chunk_index, chunk_text, emb_bytes)| {
                let stored_emb = bytes_to_embedding(&emb_bytes);
                let similarity = cosine_similarity(query_embedding, &stored_emb);
                SemanticSearchResult {
                    file_path,
                    chunk_index,
                    chunk_text,
                    similarity,
                }
            })
            .collect();

        // Sort by similarity descending
        scored.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top-k
        scored.truncate(top_k);

        Ok(scored)
    }

    /// Count the total number of embedding chunks for a project.
    pub fn count_embeddings(&self, project_path: &str) -> AppResult<usize> {
        let conn = self.get_connection()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM file_embeddings WHERE project_path = ?1",
            params![project_path],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    // =========================================================================
    // HNSW rebuild helpers (feature-001)
    // =========================================================================

    /// Retrieve all embedding IDs and vectors for a project.
    ///
    /// Returns a vector of `(row_id, embedding_f32_vec)` suitable for
    /// rebuilding an HNSW index.  The `row_id` is the SQLite ROWID of the
    /// `file_embeddings` row, used as the HNSW data ID.
    pub fn get_all_embedding_ids_and_vectors(
        &self,
        project_path: &str,
    ) -> AppResult<Vec<(usize, Vec<f32>)>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT rowid, embedding FROM file_embeddings WHERE project_path = ?1",
        )?;

        let rows: Vec<(usize, Vec<f32>)> = stmt
            .query_map(params![project_path], |row| {
                let rowid: i64 = row.get(0)?;
                let emb_bytes: Vec<u8> = row.get(1)?;
                Ok((rowid as usize, bytes_to_embedding(&emb_bytes)))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Retrieve chunk metadata (file_path, chunk_text) for a given ROWID.
    ///
    /// Used by HNSW search to fetch display data for matched embedding IDs.
    pub fn get_embedding_by_rowid(
        &self,
        rowid: usize,
    ) -> AppResult<Option<(String, i64, String)>> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT file_path, chunk_index, chunk_text FROM file_embeddings WHERE rowid = ?1",
            params![rowid as i64],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        );

        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Retrieve chunk metadata for multiple ROWIDs at once.
    ///
    /// Returns a map from rowid to (file_path, chunk_index, chunk_text).
    pub fn get_embeddings_by_rowids(
        &self,
        rowids: &[usize],
    ) -> AppResult<std::collections::HashMap<usize, (String, i64, String)>> {
        if rowids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let conn = self.get_connection()?;

        // Build a parameterized query for the rowids
        let placeholders: Vec<String> = rowids.iter().map(|_| "?".to_string()).collect();
        let query = format!(
            "SELECT rowid, file_path, chunk_index, chunk_text FROM file_embeddings WHERE rowid IN ({})",
            placeholders.join(", ")
        );

        let mut stmt = conn.prepare(&query)?;

        let params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = rowids
            .iter()
            .map(|id| Box::new(*id as i64) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let result: std::collections::HashMap<usize, (String, i64, String)> = stmt
            .query_map(params_refs.as_slice(), |row| {
                let rowid: i64 = row.get(0)?;
                let file_path: String = row.get(1)?;
                let chunk_index: i64 = row.get(2)?;
                let chunk_text: String = row.get(3)?;
                Ok((rowid as usize, (file_path, chunk_index, chunk_text)))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(result)
    }

    // =========================================================================
    // Vocabulary persistence methods (feature-003, story-002)
    // =========================================================================

    /// Save a TF-IDF vocabulary JSON blob for a project.
    ///
    /// Uses `INSERT OR REPLACE` to upsert on the `project_path` primary key.
    pub fn save_vocabulary(&self, project_path: &str, vocab_json: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO embedding_vocabulary (project_path, vocab_json, created_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)",
            params![project_path, vocab_json.as_bytes()],
        )?;
        Ok(())
    }

    /// Load a TF-IDF vocabulary JSON blob for a project.
    ///
    /// Returns `None` if no vocabulary has been saved for this project.
    pub fn load_vocabulary(&self, project_path: &str) -> AppResult<Option<String>> {
        let conn = self.get_connection()?;
        let result = conn.query_row(
            "SELECT vocab_json FROM embedding_vocabulary WHERE project_path = ?1",
            params![project_path],
            |row| row.get::<_, Vec<u8>>(0),
        );

        match result {
            Ok(bytes) => {
                let json = String::from_utf8(bytes)
                    .map_err(|e| AppError::database(format!("invalid UTF-8 in vocab: {}", e)))?;
                Ok(Some(json))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Delete the saved vocabulary for a project.
    pub fn delete_vocabulary(&self, project_path: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "DELETE FROM embedding_vocabulary WHERE project_path = ?1",
            params![project_path],
        )?;
        Ok(())
    }

    // =========================================================================
    // FTS5 query methods (feature-002, story-003)
    // =========================================================================

    /// Search symbols using FTS5 full-text search with BM25 ranking.
    ///
    /// The query is sanitized and wrapped for prefix matching. Results are
    /// ranked by BM25 relevance (FTS5 `rank` column, lower = more relevant).
    /// Returns up to `limit` results.
    ///
    /// Returns an empty Vec for empty queries without error.
    pub fn fts_search_symbols(
        &self,
        query: &str,
        limit: usize,
    ) -> AppResult<Vec<SymbolMatch>> {
        let sanitized = sanitize_fts_query(query);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.get_connection()?;

        // Restrict FTS MATCH to only the symbol_name column using FTS5
        // column filter syntax: {symbol_name} : <query>
        let fts_expr = format!("symbol_name : {}", sanitized);

        // Join symbol_fts (by rowid) with file_symbols and file_index to get
        // full result data. In contentless mode, FTS columns are NULL so we
        // must join back to the source tables.
        let mut stmt = conn.prepare(
            "SELECT fi.file_path, fi.project_path, fs.name, fs.kind, fs.line_number,
                    fs.parent_symbol, fs.signature, fs.doc_comment, fs.end_line
             FROM symbol_fts
             JOIN file_symbols fs ON fs.id = symbol_fts.rowid
             JOIN file_index fi ON fi.id = fs.file_index_id
             WHERE symbol_fts MATCH ?1
             ORDER BY symbol_fts.rank
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![fts_expr, limit as i64], |row| {
                Ok(SymbolMatch {
                    file_path: row.get(0)?,
                    project_path: row.get(1)?,
                    symbol_name: row.get(2)?,
                    symbol_kind: row.get(3)?,
                    line_number: row.get::<_, i64>(4)? as usize,
                    parent_symbol: row.get(5)?,
                    signature: row.get(6)?,
                    doc_comment: row.get(7)?,
                    end_line: row.get::<_, i64>(8).unwrap_or(0) as usize,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Search file paths using FTS5 full-text search with BM25 ranking.
    ///
    /// Results are filtered by `project_path` and ranked by BM25 relevance.
    /// Returns up to `limit` results.
    ///
    /// Returns an empty Vec for empty queries without error.
    pub fn fts_search_files(
        &self,
        query: &str,
        project_path: &str,
        limit: usize,
    ) -> AppResult<Vec<FileIndexRow>> {
        let sanitized = sanitize_fts_query(query);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.get_connection()?;

        // Restrict FTS MATCH to only the file_path column
        let fts_expr = format!("file_path : {}", sanitized);

        // Join filepath_fts (by rowid) with file_index to get full data and
        // filter by project_path.
        let mut stmt = conn.prepare(
            "SELECT fi.id, fi.project_path, fi.file_path, fi.component, fi.language,
                    fi.extension, fi.size_bytes, fi.line_count, fi.is_test,
                    fi.content_hash, fi.indexed_at
             FROM filepath_fts
             JOIN file_index fi ON fi.id = filepath_fts.rowid
             WHERE filepath_fts MATCH ?1 AND fi.project_path = ?2
             ORDER BY filepath_fts.rank
             LIMIT ?3",
        )?;

        let rows = stmt
            .query_map(params![fts_expr, project_path, limit as i64], |row| {
                Ok(FileIndexRow {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    file_path: row.get(2)?,
                    component: row.get(3)?,
                    language: row.get(4)?,
                    extension: row.get(5)?,
                    size_bytes: row.get::<_, i64>(6)? as u64,
                    line_count: row.get::<_, i64>(7)? as usize,
                    is_test: row.get::<_, i32>(8)? != 0,
                    content_hash: row.get(9)?,
                    indexed_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    // =========================================================================
    // LSP enrichment methods (feature-003, Phase 3)
    // =========================================================================

    /// Update the resolved type for a symbol by its rowid.
    pub fn update_symbol_type(&self, rowid: i64, resolved_type: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE file_symbols SET resolved_type = ?2 WHERE id = ?1",
            params![rowid, resolved_type],
        )?;
        Ok(())
    }

    /// Update the reference count for a symbol by its rowid.
    pub fn update_reference_count(&self, rowid: i64, count: i64) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE file_symbols SET reference_count = ?2 WHERE id = ?1",
            params![rowid, count],
        )?;
        Ok(())
    }

    /// Set whether a symbol is exported by its rowid.
    pub fn set_symbol_exported(&self, rowid: i64, is_exported: bool) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE file_symbols SET is_exported = ?2 WHERE id = ?1",
            params![rowid, is_exported as i32],
        )?;
        Ok(())
    }

    /// Insert a cross-reference record.
    ///
    /// Uses `INSERT OR IGNORE` to skip duplicates based on the unique constraint.
    pub fn insert_cross_reference(
        &self,
        project_path: &str,
        source_file: &str,
        source_line: i64,
        source_symbol: Option<&str>,
        target_file: &str,
        target_line: i64,
        target_symbol: Option<&str>,
        reference_kind: &str,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR IGNORE INTO cross_references
             (project_path, source_file, source_line, source_symbol,
              target_file, target_line, target_symbol, reference_kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                project_path,
                source_file,
                source_line,
                source_symbol,
                target_file,
                target_line,
                target_symbol,
                reference_kind,
            ],
        )?;
        Ok(())
    }

    /// Get symbols for LSP enrichment, grouped by language and file path.
    ///
    /// Returns `(symbol_rowid, file_path, symbol_name, line_number, language)` tuples.
    pub fn get_symbols_for_enrichment(
        &self,
        project_path: &str,
        language: &str,
    ) -> AppResult<Vec<(i64, String, String, i64, String)>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT fs.id, fi.file_path, fs.name, fs.line_number, fi.language
             FROM file_symbols fs
             JOIN file_index fi ON fi.id = fs.file_index_id
             WHERE fi.project_path = ?1 AND fi.language = ?2
             ORDER BY fi.file_path, fs.line_number",
        )?;

        let rows = stmt
            .query_map(params![project_path, language], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Clear all enrichment data for a project.
    ///
    /// Resets resolved_type, reference_count, is_exported columns and deletes
    /// cross-references. Used before re-enrichment.
    pub fn clear_enrichment_data(&self, project_path: &str) -> AppResult<()> {
        let conn = self.get_connection()?;

        // Reset enrichment columns on file_symbols
        conn.execute(
            "UPDATE file_symbols SET resolved_type = NULL, reference_count = 0, is_exported = 0
             WHERE file_index_id IN (
                 SELECT id FROM file_index WHERE project_path = ?1
             )",
            params![project_path],
        )?;

        // Delete cross-references
        conn.execute(
            "DELETE FROM cross_references WHERE project_path = ?1",
            params![project_path],
        )?;

        Ok(())
    }

    /// Get cross-references for a specific file in a project.
    pub fn get_cross_references(
        &self,
        project_path: &str,
        file_path: &str,
    ) -> AppResult<Vec<CrossReference>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT id, source_file, source_line, source_symbol,
                    target_file, target_line, target_symbol, reference_kind
             FROM cross_references
             WHERE project_path = ?1 AND (source_file = ?2 OR target_file = ?2)
             ORDER BY source_file, source_line",
        )?;

        let rows = stmt
            .query_map(params![project_path, file_path], |row| {
                Ok(CrossReference {
                    id: row.get(0)?,
                    source_file: row.get(1)?,
                    source_line: row.get(2)?,
                    source_symbol: row.get(3)?,
                    target_file: row.get(4)?,
                    target_line: row.get(5)?,
                    target_symbol: row.get(6)?,
                    reference_kind: row.get(7)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Insert or update an LSP server detection cache entry.
    pub fn upsert_lsp_server(
        &self,
        language: &str,
        binary_path: &str,
        server_name: &str,
        version: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO lsp_servers (language, binary_path, server_name, version, detected_at)
             VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)",
            params![language, binary_path, server_name, version],
        )?;
        Ok(())
    }

    /// Get all cached LSP server entries.
    pub fn get_lsp_servers(&self) -> AppResult<Vec<LspServerInfo>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT language, binary_path, server_name, version, detected_at
             FROM lsp_servers
             ORDER BY language",
        )?;

        let rows = stmt
            .query_map([], |row| {
                Ok(LspServerInfo {
                    language: row.get(0)?,
                    binary_path: row.get(1)?,
                    server_name: row.get(2)?,
                    version: row.get(3)?,
                    detected_at: row.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Read a value from the `settings` table by key.
    ///
    /// This mirrors `Database::get_setting` but operates on the `IndexStore`'s
    /// own pool, so callers that only have an `IndexStore` reference (e.g.
    /// `IndexManager`) do not need a separate `Database` handle.
    pub fn get_setting(&self, key: &str) -> AppResult<Option<String>> {
        let conn = self.get_connection()?;
        let result = conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(format!(
                "Failed to read setting '{}': {}",
                key, e
            ))),
        }
    }

    fn get_connection(
        &self,
    ) -> AppResult<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))
    }
}

/// A cross-reference record from the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossReference {
    pub id: i64,
    pub source_file: String,
    pub source_line: i64,
    pub source_symbol: Option<String>,
    pub target_file: String,
    pub target_line: i64,
    pub target_symbol: Option<String>,
    pub reference_kind: String,
}

/// An LSP server info record from the cache table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerInfo {
    pub language: String,
    pub binary_path: String,
    pub server_name: String,
    pub version: Option<String>,
    pub detected_at: Option<String>,
}

/// Sanitize a user query string for safe use in FTS5 MATCH expressions.
///
/// Each whitespace-delimited token is:
/// 1. Escaped (inner double-quotes are doubled)
/// 2. Wrapped in double-quotes
/// 3. Suffixed with `*` for prefix matching
///
/// Tokens are joined with spaces (implicit AND in FTS5).
///
/// Returns an empty string for empty/whitespace-only input.
pub fn sanitize_fts_query(input: &str) -> String {
    let tokens: Vec<String> = input
        .split_whitespace()
        .map(|token| {
            let escaped = token.replace('"', "\"\"");
            format!("\"{}\"*", escaped)
        })
        .collect();
    tokens.join(" ")
}

/// A row from the file_index table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIndexRow {
    pub id: i64,
    pub project_path: String,
    pub file_path: String,
    pub component: String,
    pub language: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub line_count: usize,
    pub is_test: bool,
    pub content_hash: String,
    pub indexed_at: Option<String>,
}

fn symbol_kind_to_str(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "Function",
        SymbolKind::Class => "Class",
        SymbolKind::Struct => "Struct",
        SymbolKind::Enum => "Enum",
        SymbolKind::Interface => "Interface",
        SymbolKind::Type => "Type",
        SymbolKind::Const => "Const",
        SymbolKind::Module => "Module",
    }
}

fn str_to_symbol_kind(s: &str) -> SymbolKind {
    match s {
        "Function" => SymbolKind::Function,
        "Class" => SymbolKind::Class,
        "Struct" => SymbolKind::Struct,
        "Enum" => SymbolKind::Enum,
        "Interface" => SymbolKind::Interface,
        "Type" => SymbolKind::Type,
        "Const" => SymbolKind::Const,
        "Module" => SymbolKind::Module,
        _ => SymbolKind::Function, // fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    fn create_test_store() -> IndexStore {
        let db = Database::new_in_memory().expect("in-memory db");
        IndexStore::new(db.pool().clone())
    }

    fn make_item(
        path: &str,
        component: &str,
        language: &str,
        symbols: Vec<SymbolInfo>,
    ) -> FileInventoryItem {
        FileInventoryItem {
            path: path.to_string(),
            component: component.to_string(),
            language: language.to_string(),
            extension: path.rsplit('.').next().map(|s| s.to_string()),
            size_bytes: 1024,
            line_count: 50,
            is_test: false,
            symbols,
        }
    }

    // =========================================================================
    // Upsert tests
    // =========================================================================

    #[test]
    fn upsert_inserts_new_file_index() {
        let store = create_test_store();
        let item = make_item(
            "src/main.rs",
            "desktop-rust",
            "rust",
            vec![SymbolInfo::basic(
                "main".to_string(),
                SymbolKind::Function,
                1,
            )],
        );

        store
            .upsert_file_index("/project", &item, "abc123")
            .unwrap();

        let symbols = store.get_file_symbols("/project", "src/main.rs").unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "main");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].line, 1);
    }

    #[test]
    fn upsert_updates_existing_file_index() {
        let store = create_test_store();

        // Initial insert
        let item_v1 = make_item(
            "src/lib.rs",
            "desktop-rust",
            "rust",
            vec![SymbolInfo::basic(
                "init".to_string(),
                SymbolKind::Function,
                5,
            )],
        );
        store
            .upsert_file_index("/project", &item_v1, "hash_v1")
            .unwrap();

        // Update with new symbols
        let item_v2 = make_item(
            "src/lib.rs",
            "desktop-rust",
            "rust",
            vec![
                SymbolInfo::basic("init".to_string(), SymbolKind::Function, 5),
                SymbolInfo::basic("Config".to_string(), SymbolKind::Struct, 20),
            ],
        );
        store
            .upsert_file_index("/project", &item_v2, "hash_v2")
            .unwrap();

        let symbols = store.get_file_symbols("/project", "src/lib.rs").unwrap();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "init");
        assert_eq!(symbols[1].name, "Config");
        assert_eq!(symbols[1].kind, SymbolKind::Struct);
    }

    #[test]
    fn upsert_replaces_symbols_on_update() {
        let store = create_test_store();

        // Initial insert with 3 symbols
        let item_v1 = make_item(
            "src/service.rs",
            "desktop-rust",
            "rust",
            vec![
                SymbolInfo::basic("foo".to_string(), SymbolKind::Function, 1),
                SymbolInfo::basic("bar".to_string(), SymbolKind::Function, 10),
                SymbolInfo::basic("Baz".to_string(), SymbolKind::Struct, 20),
            ],
        );
        store.upsert_file_index("/project", &item_v1, "h1").unwrap();

        // Update with only 1 symbol
        let item_v2 = make_item(
            "src/service.rs",
            "desktop-rust",
            "rust",
            vec![SymbolInfo::basic(
                "new_fn".to_string(),
                SymbolKind::Function,
                1,
            )],
        );
        store.upsert_file_index("/project", &item_v2, "h2").unwrap();

        let symbols = store
            .get_file_symbols("/project", "src/service.rs")
            .unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "new_fn");
    }

    // =========================================================================
    // Symbol query tests
    // =========================================================================

    #[test]
    fn query_symbols_by_name_pattern() {
        let store = create_test_store();

        let item1 = make_item(
            "src/controller.rs",
            "desktop-rust",
            "rust",
            vec![
                SymbolInfo::basic("UserController".to_string(), SymbolKind::Struct, 5),
                SymbolInfo::basic("handle_request".to_string(), SymbolKind::Function, 15),
            ],
        );
        let item2 = make_item(
            "src/admin.rs",
            "desktop-rust",
            "rust",
            vec![SymbolInfo::basic(
                "AdminController".to_string(),
                SymbolKind::Struct,
                3,
            )],
        );

        store.upsert_file_index("/project", &item1, "h1").unwrap();
        store.upsert_file_index("/project", &item2, "h2").unwrap();

        let results = store.query_symbols("%Controller%").unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.symbol_name == "UserController"));
        assert!(results.iter().any(|r| r.symbol_name == "AdminController"));
    }

    #[test]
    fn query_symbols_returns_empty_for_no_match() {
        let store = create_test_store();

        let item = make_item(
            "src/main.rs",
            "desktop-rust",
            "rust",
            vec![SymbolInfo::basic(
                "main".to_string(),
                SymbolKind::Function,
                1,
            )],
        );
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let results = store.query_symbols("%NonExistent%").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn query_symbols_exact_name() {
        let store = create_test_store();

        let item = make_item(
            "src/models.rs",
            "desktop-rust",
            "rust",
            vec![
                SymbolInfo::basic("Config".to_string(), SymbolKind::Struct, 1),
                SymbolInfo::basic("ConfigBuilder".to_string(), SymbolKind::Struct, 20),
            ],
        );
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let results = store.query_symbols("Config").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].symbol_name, "Config");
    }

    // =========================================================================
    // Component query tests
    // =========================================================================

    #[test]
    fn query_files_by_component() {
        let store = create_test_store();

        let item1 = make_item("src/main.rs", "desktop-rust", "rust", vec![]);
        let item2 = make_item("src/lib.rs", "desktop-rust", "rust", vec![]);
        let item3 = make_item(
            "src/components/App.tsx",
            "desktop-web",
            "typescript",
            vec![],
        );

        store.upsert_file_index("/project", &item1, "h1").unwrap();
        store.upsert_file_index("/project", &item2, "h2").unwrap();
        store.upsert_file_index("/project", &item3, "h3").unwrap();

        let rust_files = store
            .query_files_by_component("/project", "desktop-rust")
            .unwrap();
        assert_eq!(rust_files.len(), 2);
        assert!(rust_files.iter().all(|f| f.component == "desktop-rust"));

        let web_files = store
            .query_files_by_component("/project", "desktop-web")
            .unwrap();
        assert_eq!(web_files.len(), 1);
        assert_eq!(web_files[0].file_path, "src/components/App.tsx");
    }

    #[test]
    fn query_files_by_component_empty_result() {
        let store = create_test_store();

        let item = make_item("src/main.rs", "desktop-rust", "rust", vec![]);
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let files = store
            .query_files_by_component("/project", "nonexistent")
            .unwrap();
        assert!(files.is_empty());
    }

    // =========================================================================
    // Project summary tests
    // =========================================================================

    #[test]
    fn get_project_summary_basic() {
        let store = create_test_store();

        let item1 = make_item(
            "src/main.rs",
            "desktop-rust",
            "rust",
            vec![SymbolInfo::basic(
                "main".to_string(),
                SymbolKind::Function,
                1,
            )],
        );
        let item2 = make_item(
            "src/app.tsx",
            "desktop-web",
            "typescript",
            vec![SymbolInfo::basic(
                "app".to_string(),
                SymbolKind::Function,
                1,
            )],
        );
        let item3 = make_item("src/utils.py", "python-core", "python", vec![]);

        store.upsert_file_index("/project", &item1, "h1").unwrap();
        store.upsert_file_index("/project", &item2, "h2").unwrap();
        store.upsert_file_index("/project", &item3, "h3").unwrap();

        let summary = store.get_project_summary("/project").unwrap();
        assert_eq!(summary.total_files, 3);
        assert_eq!(summary.languages.len(), 3);
        assert!(summary.languages.contains(&"rust".to_string()));
        assert!(summary.languages.contains(&"typescript".to_string()));
        assert!(summary.languages.contains(&"python".to_string()));
        assert_eq!(summary.components.len(), 3);

        // main and app are key entry point symbols
        assert!(!summary.key_entry_points.is_empty());
    }

    #[test]
    fn get_project_summary_empty_project() {
        let store = create_test_store();

        let summary = store.get_project_summary("/empty-project").unwrap();
        assert_eq!(summary.total_files, 0);
        assert!(summary.languages.is_empty());
        assert!(summary.components.is_empty());
        assert!(summary.key_entry_points.is_empty());
    }

    // =========================================================================
    // Staleness tests
    // =========================================================================

    #[test]
    fn is_index_stale_returns_true_for_missing_file() {
        let store = create_test_store();

        let stale = store
            .is_index_stale("/project", "src/missing.rs", "somehash")
            .unwrap();
        assert!(stale);
    }

    #[test]
    fn is_index_stale_returns_false_for_matching_hash() {
        let store = create_test_store();

        let item = make_item("src/main.rs", "desktop-rust", "rust", vec![]);
        store
            .upsert_file_index("/project", &item, "abc123")
            .unwrap();

        let stale = store
            .is_index_stale("/project", "src/main.rs", "abc123")
            .unwrap();
        assert!(!stale);
    }

    #[test]
    fn is_index_stale_returns_true_for_different_hash() {
        let store = create_test_store();

        let item = make_item("src/main.rs", "desktop-rust", "rust", vec![]);
        store
            .upsert_file_index("/project", &item, "abc123")
            .unwrap();

        let stale = store
            .is_index_stale("/project", "src/main.rs", "def456")
            .unwrap();
        assert!(stale);
    }

    // =========================================================================
    // Foreign key cascade delete tests
    // =========================================================================

    #[test]
    fn cascade_delete_removes_symbols() {
        let store = create_test_store();

        let item = make_item(
            "src/main.rs",
            "desktop-rust",
            "rust",
            vec![
                SymbolInfo::basic("main".to_string(), SymbolKind::Function, 1),
                SymbolInfo::basic("Config".to_string(), SymbolKind::Struct, 10),
            ],
        );
        store.upsert_file_index("/project", &item, "h1").unwrap();

        // Verify symbols exist
        let symbols = store.get_file_symbols("/project", "src/main.rs").unwrap();
        assert_eq!(symbols.len(), 2);

        // Delete the project index (triggers cascade)
        let deleted = store.delete_project_index("/project").unwrap();
        assert_eq!(deleted, 1);

        // Verify symbols are gone
        let symbols = store.get_file_symbols("/project", "src/main.rs").unwrap();
        assert!(symbols.is_empty());

        // Verify no orphaned symbols remain via query
        let matches = store.query_symbols("%main%").unwrap();
        assert!(matches.is_empty());
    }

    // =========================================================================
    // Multi-project isolation tests
    // =========================================================================

    #[test]
    fn separate_projects_are_isolated() {
        let store = create_test_store();

        let item_a = make_item(
            "src/main.rs",
            "desktop-rust",
            "rust",
            vec![SymbolInfo::basic(
                "main_a".to_string(),
                SymbolKind::Function,
                1,
            )],
        );
        let item_b = make_item(
            "src/main.rs",
            "desktop-rust",
            "rust",
            vec![SymbolInfo::basic(
                "main_b".to_string(),
                SymbolKind::Function,
                1,
            )],
        );

        store
            .upsert_file_index("/project-a", &item_a, "ha")
            .unwrap();
        store
            .upsert_file_index("/project-b", &item_b, "hb")
            .unwrap();

        let summary_a = store.get_project_summary("/project-a").unwrap();
        assert_eq!(summary_a.total_files, 1);

        let summary_b = store.get_project_summary("/project-b").unwrap();
        assert_eq!(summary_b.total_files, 1);

        let symbols_a = store.get_file_symbols("/project-a", "src/main.rs").unwrap();
        assert_eq!(symbols_a[0].name, "main_a");

        let symbols_b = store.get_file_symbols("/project-b", "src/main.rs").unwrap();
        assert_eq!(symbols_b[0].name, "main_b");
    }

    // =========================================================================
    // Symbol kind roundtrip test
    // =========================================================================

    #[test]
    fn symbol_kind_roundtrips_through_storage() {
        let store = create_test_store();

        let all_kinds = vec![
            SymbolInfo::basic("my_func".to_string(), SymbolKind::Function, 1),
            SymbolInfo::basic("MyClass".to_string(), SymbolKind::Class, 2),
            SymbolInfo::basic("MyStruct".to_string(), SymbolKind::Struct, 3),
            SymbolInfo::basic("MyEnum".to_string(), SymbolKind::Enum, 4),
            SymbolInfo::basic("MyInterface".to_string(), SymbolKind::Interface, 5),
            SymbolInfo::basic("MyType".to_string(), SymbolKind::Type, 6),
            SymbolInfo::basic("MY_CONST".to_string(), SymbolKind::Const, 7),
            SymbolInfo::basic("my_module".to_string(), SymbolKind::Module, 8),
        ];

        let item = make_item(
            "src/all_kinds.rs",
            "desktop-rust",
            "rust",
            all_kinds.clone(),
        );
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let stored = store
            .get_file_symbols("/project", "src/all_kinds.rs")
            .unwrap();
        assert_eq!(stored.len(), 8);

        for (original, stored) in all_kinds.iter().zip(stored.iter()) {
            assert_eq!(original.name, stored.name);
            assert_eq!(original.kind, stored.kind);
            assert_eq!(original.line, stored.line);
        }
    }

    // =========================================================================
    // Large batch test
    // =========================================================================

    #[test]
    fn handles_many_files_and_symbols() {
        let store = create_test_store();

        for i in 0..50 {
            let symbols: Vec<SymbolInfo> = (0..5)
                .map(|j| SymbolInfo::basic(format!("symbol_{i}_{j}"), SymbolKind::Function, j + 1))
                .collect();

            let item = make_item(
                &format!("src/module_{i}.rs"),
                "desktop-rust",
                "rust",
                symbols,
            );
            store
                .upsert_file_index("/project", &item, &format!("hash_{i}"))
                .unwrap();
        }

        let summary = store.get_project_summary("/project").unwrap();
        assert_eq!(summary.total_files, 50);

        // Query for a specific symbol pattern
        let results = store.query_symbols("symbol_25_%").unwrap();
        assert_eq!(results.len(), 5);
    }

    // =========================================================================
    // Extended symbol fields roundtrip test
    // =========================================================================

    #[test]
    fn extended_symbol_fields_roundtrip() {
        let store = create_test_store();

        let symbols = vec![
            SymbolInfo {
                name: "MyClass".to_string(),
                kind: SymbolKind::Class,
                line: 5,
                parent: None,
                signature: Some("class MyClass(Base):".to_string()),
                doc_comment: Some("A sample class".to_string()),
                end_line: 25,
            },
            SymbolInfo {
                name: "my_method".to_string(),
                kind: SymbolKind::Function,
                line: 10,
                parent: Some("MyClass".to_string()),
                signature: Some("def my_method(self, x: int) -> str:".to_string()),
                doc_comment: Some("Does something useful".to_string()),
                end_line: 20,
            },
            SymbolInfo::basic("standalone_fn".to_string(), SymbolKind::Function, 30),
        ];

        let item = make_item("src/example.py", "python-core", "python", symbols);
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let stored = store
            .get_file_symbols("/project", "src/example.py")
            .unwrap();
        assert_eq!(stored.len(), 3);

        // Check extended fields for MyClass
        assert_eq!(stored[0].name, "MyClass");
        assert_eq!(stored[0].signature.as_deref(), Some("class MyClass(Base):"));
        assert_eq!(stored[0].doc_comment.as_deref(), Some("A sample class"));
        assert_eq!(stored[0].end_line, 25);
        assert!(stored[0].parent.is_none());

        // Check extended fields for my_method
        assert_eq!(stored[1].name, "my_method");
        assert_eq!(stored[1].parent.as_deref(), Some("MyClass"));
        assert_eq!(
            stored[1].signature.as_deref(),
            Some("def my_method(self, x: int) -> str:")
        );
        assert_eq!(
            stored[1].doc_comment.as_deref(),
            Some("Does something useful")
        );
        assert_eq!(stored[1].end_line, 20);

        // Check that basic symbol has None for extended fields
        assert_eq!(stored[2].name, "standalone_fn");
        assert!(stored[2].parent.is_none());
        assert!(stored[2].signature.is_none());
        assert!(stored[2].doc_comment.is_none());
        assert_eq!(stored[2].end_line, 0);
    }

    // =========================================================================
    // Embedding storage tests (feature-003)
    // =========================================================================

    #[test]
    fn upsert_and_retrieve_chunk_embedding() {
        let store = create_test_store();
        let embedding: Vec<u8> = vec![0, 0, 128, 63, 0, 0, 0, 64]; // [1.0f32, 2.0f32] as bytes

        store
            .upsert_chunk_embedding("/project", "src/main.rs", 0, "fn main() {}", &embedding)
            .unwrap();

        let results = store.get_embeddings_for_project("/project").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "src/main.rs");
        assert_eq!(results[0].1, 0);
        assert_eq!(results[0].2, "fn main() {}");
        assert_eq!(results[0].3, embedding);
    }

    #[test]
    fn upsert_chunk_embedding_updates_on_conflict() {
        let store = create_test_store();
        let emb1: Vec<u8> = vec![0, 0, 128, 63]; // 1.0f32
        let emb2: Vec<u8> = vec![0, 0, 0, 64]; // 2.0f32

        store
            .upsert_chunk_embedding("/project", "src/main.rs", 0, "original", &emb1)
            .unwrap();
        store
            .upsert_chunk_embedding("/project", "src/main.rs", 0, "updated", &emb2)
            .unwrap();

        let results = store.get_embeddings_for_project("/project").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "updated");
        assert_eq!(results[0].3, emb2);
    }

    #[test]
    fn delete_embeddings_for_file() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        store
            .upsert_chunk_embedding("/project", "src/a.rs", 0, "chunk a", &emb)
            .unwrap();
        store
            .upsert_chunk_embedding("/project", "src/b.rs", 0, "chunk b", &emb)
            .unwrap();

        let deleted = store
            .delete_embeddings_for_file("/project", "src/a.rs")
            .unwrap();
        assert_eq!(deleted, 1);

        let remaining = store.get_embeddings_for_project("/project").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, "src/b.rs");
    }

    #[test]
    fn delete_embeddings_for_project() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        store
            .upsert_chunk_embedding("/project", "src/a.rs", 0, "chunk a", &emb)
            .unwrap();
        store
            .upsert_chunk_embedding("/project", "src/b.rs", 0, "chunk b", &emb)
            .unwrap();

        let deleted = store.delete_embeddings_for_project("/project").unwrap();
        assert_eq!(deleted, 2);

        let remaining = store.get_embeddings_for_project("/project").unwrap();
        assert!(remaining.is_empty());
    }

    #[test]
    fn count_embeddings_basic() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        assert_eq!(store.count_embeddings("/project").unwrap(), 0);

        store
            .upsert_chunk_embedding("/project", "src/a.rs", 0, "chunk 0", &emb)
            .unwrap();
        store
            .upsert_chunk_embedding("/project", "src/a.rs", 1, "chunk 1", &emb)
            .unwrap();
        store
            .upsert_chunk_embedding("/project", "src/b.rs", 0, "chunk 0", &emb)
            .unwrap();

        assert_eq!(store.count_embeddings("/project").unwrap(), 3);
    }

    #[test]
    fn embeddings_isolated_by_project() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        store
            .upsert_chunk_embedding("/project-a", "src/a.rs", 0, "chunk a", &emb)
            .unwrap();
        store
            .upsert_chunk_embedding("/project-b", "src/b.rs", 0, "chunk b", &emb)
            .unwrap();

        assert_eq!(store.count_embeddings("/project-a").unwrap(), 1);
        assert_eq!(store.count_embeddings("/project-b").unwrap(), 1);

        let results_a = store.get_embeddings_for_project("/project-a").unwrap();
        assert_eq!(results_a[0].0, "src/a.rs");

        let results_b = store.get_embeddings_for_project("/project-b").unwrap();
        assert_eq!(results_b[0].0, "src/b.rs");
    }

    // =========================================================================
    // Semantic search tests (story-004)
    // =========================================================================

    #[test]
    fn semantic_search_returns_sorted_results() {
        use super::super::embedding_service::embedding_to_bytes;

        let store = create_test_store();

        // Create embeddings where vec1 is closer to query than vec2
        let query = vec![1.0f32, 0.0, 0.0];
        let vec1 = vec![0.9f32, 0.1, 0.0]; // very similar to query
        let vec2 = vec![0.0f32, 1.0, 0.0]; // orthogonal to query
        let vec3 = vec![0.5f32, 0.5, 0.0]; // moderately similar

        store
            .upsert_chunk_embedding("/project", "a.rs", 0, "chunk a", &embedding_to_bytes(&vec1))
            .unwrap();
        store
            .upsert_chunk_embedding("/project", "b.rs", 0, "chunk b", &embedding_to_bytes(&vec2))
            .unwrap();
        store
            .upsert_chunk_embedding("/project", "c.rs", 0, "chunk c", &embedding_to_bytes(&vec3))
            .unwrap();

        let results = store.semantic_search(&query, "/project", 10).unwrap();
        assert_eq!(results.len(), 3);

        // First result should be the most similar (vec1 -> "chunk a")
        assert_eq!(results[0].chunk_text, "chunk a");
        assert!(results[0].similarity > results[1].similarity);

        // Last result should be the least similar (vec2 -> "chunk b")
        assert_eq!(results[2].chunk_text, "chunk b");
    }

    #[test]
    fn semantic_search_top_k() {
        use super::super::embedding_service::embedding_to_bytes;

        let store = create_test_store();
        let emb = embedding_to_bytes(&vec![1.0f32, 0.0]);

        for i in 0..10 {
            store
                .upsert_chunk_embedding(
                    "/project",
                    &format!("file_{}.rs", i),
                    0,
                    &format!("chunk {}", i),
                    &emb,
                )
                .unwrap();
        }

        let query = vec![1.0f32, 0.0];
        let results = store.semantic_search(&query, "/project", 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn semantic_search_empty_project() {
        let store = create_test_store();
        let query = vec![1.0f32, 0.0, 0.0];

        let results = store.semantic_search(&query, "/nonexistent", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn semantic_search_empty_query() {
        use super::super::embedding_service::embedding_to_bytes;

        let store = create_test_store();
        let emb = embedding_to_bytes(&vec![1.0f32, 0.0]);
        store
            .upsert_chunk_embedding("/project", "a.rs", 0, "chunk", &emb)
            .unwrap();

        let empty_query: Vec<f32> = vec![];
        let results = store.semantic_search(&empty_query, "/project", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn multiple_chunks_per_file() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        for i in 0..5 {
            store
                .upsert_chunk_embedding("/project", "src/main.rs", i, &format!("chunk {}", i), &emb)
                .unwrap();
        }

        let results = store.get_embeddings_for_project("/project").unwrap();
        assert_eq!(results.len(), 5);
        // Should be ordered by chunk_index
        for (i, r) in results.iter().enumerate() {
            assert_eq!(r.1, i as i64);
        }
    }

    // =========================================================================
    // Vocabulary persistence tests (feature-003, story-002)
    // =========================================================================

    #[test]
    fn save_then_load_vocabulary_roundtrip() {
        let store = create_test_store();
        let vocab_json = r#"{"token_to_idx":{"hello":0},"idf":[1.0],"num_docs":1}"#;
        store.save_vocabulary("/project", vocab_json).unwrap();

        let loaded = store.load_vocabulary("/project").unwrap();
        assert_eq!(loaded, Some(vocab_json.to_string()));
    }

    #[test]
    fn load_vocabulary_returns_none_for_nonexistent() {
        let store = create_test_store();
        let loaded = store.load_vocabulary("/nonexistent").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn save_vocabulary_upserts_on_conflict() {
        let store = create_test_store();
        store.save_vocabulary("/project", "v1").unwrap();
        store.save_vocabulary("/project", "v2").unwrap();

        let loaded = store.load_vocabulary("/project").unwrap();
        assert_eq!(loaded, Some("v2".to_string()));
    }

    #[test]
    fn delete_vocabulary_removes_entry() {
        let store = create_test_store();
        store.save_vocabulary("/project", "vocab").unwrap();
        store.delete_vocabulary("/project").unwrap();

        let loaded = store.load_vocabulary("/project").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn vocabulary_isolated_per_project() {
        let store = create_test_store();
        store.save_vocabulary("/project-a", "vocab-a").unwrap();
        store.save_vocabulary("/project-b", "vocab-b").unwrap();

        assert_eq!(
            store.load_vocabulary("/project-a").unwrap(),
            Some("vocab-a".to_string())
        );
        assert_eq!(
            store.load_vocabulary("/project-b").unwrap(),
            Some("vocab-b".to_string())
        );

        // Deleting one should not affect the other
        store.delete_vocabulary("/project-a").unwrap();
        assert!(store.load_vocabulary("/project-a").unwrap().is_none());
        assert_eq!(
            store.load_vocabulary("/project-b").unwrap(),
            Some("vocab-b".to_string())
        );
    }

    #[test]
    fn query_symbols_returns_extended_fields() {
        let store = create_test_store();

        let symbols = vec![SymbolInfo {
            name: "process".to_string(),
            kind: SymbolKind::Function,
            line: 10,
            parent: Some("Handler".to_string()),
            signature: Some("fn process(&self, data: &[u8]) -> Result<()>".to_string()),
            doc_comment: Some("Process incoming data".to_string()),
            end_line: 30,
        }];

        let item = make_item("src/handler.rs", "desktop-rust", "rust", symbols);
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let results = store.query_symbols("%process%").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].parent_symbol.as_deref(), Some("Handler"));
        assert_eq!(
            results[0].signature.as_deref(),
            Some("fn process(&self, data: &[u8]) -> Result<()>")
        );
        assert_eq!(
            results[0].doc_comment.as_deref(),
            Some("Process incoming data")
        );
        assert_eq!(results[0].end_line, 30);
    }

    // =========================================================================
    // Provider-aware embedding tests (story-012)
    // =========================================================================

    #[test]
    fn upsert_with_provider_metadata() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        store
            .upsert_chunk_embedding_with_provider(
                "/project",
                "src/main.rs",
                0,
                "fn main() {}",
                &emb,
                Some("ollama"),
                Some("nomic-embed-text"),
                Some(768),
            )
            .unwrap();

        // Verify it's stored and retrievable
        let results = store.get_embeddings_for_project("/project").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "fn main() {}");

        // Verify metadata
        let meta = store.get_embedding_metadata("/project").unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].provider_type, "ollama");
        assert_eq!(meta[0].provider_model, "nomic-embed-text");
        assert_eq!(meta[0].embedding_dimension, 768);
    }

    #[test]
    fn upsert_without_provider_uses_defaults() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        store
            .upsert_chunk_embedding("/project", "src/main.rs", 0, "fn main() {}", &emb)
            .unwrap();

        let meta = store.get_embedding_metadata("/project").unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].provider_type, "tfidf");
        assert_eq!(meta[0].provider_model, "tfidf-v1");
        assert_eq!(meta[0].embedding_dimension, 0);
    }

    #[test]
    fn get_embeddings_filtered_by_provider() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        // Insert one with default provider (tfidf)
        store
            .upsert_chunk_embedding("/project", "src/a.rs", 0, "chunk a", &emb)
            .unwrap();

        // Insert another with a different provider (overwrite is per file+chunk,
        // so use a different file)
        store
            .upsert_chunk_embedding_with_provider(
                "/project",
                "src/b.rs",
                0,
                "chunk b",
                &emb,
                Some("ollama"),
                Some("nomic-embed-text"),
                Some(768),
            )
            .unwrap();

        // Unfiltered should return both
        let all = store.get_embeddings_for_project("/project").unwrap();
        assert_eq!(all.len(), 2);

        // Filter by tfidf should return only a.rs
        let tfidf = store
            .get_embeddings_for_project_filtered("/project", Some("tfidf"))
            .unwrap();
        assert_eq!(tfidf.len(), 1);
        assert_eq!(tfidf[0].0, "src/a.rs");

        // Filter by ollama should return only b.rs
        let ollama = store
            .get_embeddings_for_project_filtered("/project", Some("ollama"))
            .unwrap();
        assert_eq!(ollama.len(), 1);
        assert_eq!(ollama[0].0, "src/b.rs");

        // Filter by nonexistent provider returns empty
        let none = store
            .get_embeddings_for_project_filtered("/project", Some("openai"))
            .unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn semantic_search_filtered_by_provider() {
        use super::super::embedding_service::embedding_to_bytes;

        let store = create_test_store();

        let query = vec![1.0f32, 0.0, 0.0];
        let vec1 = embedding_to_bytes(&vec![0.9f32, 0.1, 0.0]);
        let vec2 = embedding_to_bytes(&vec![0.5f32, 0.5, 0.0]);

        // Store vec1 as tfidf
        store
            .upsert_chunk_embedding("/project", "a.rs", 0, "tfidf chunk", &vec1)
            .unwrap();

        // Store vec2 as ollama
        store
            .upsert_chunk_embedding_with_provider(
                "/project",
                "b.rs",
                0,
                "ollama chunk",
                &vec2,
                Some("ollama"),
                Some("nomic-embed-text"),
                Some(768),
            )
            .unwrap();

        // Unfiltered search returns both
        let all_results = store.semantic_search(&query, "/project", 10).unwrap();
        assert_eq!(all_results.len(), 2);

        // Filtered by tfidf returns only tfidf chunk
        let tfidf_results = store
            .semantic_search_filtered(&query, "/project", 10, Some("tfidf"))
            .unwrap();
        assert_eq!(tfidf_results.len(), 1);
        assert_eq!(tfidf_results[0].chunk_text, "tfidf chunk");

        // Filtered by ollama returns only ollama chunk
        let ollama_results = store
            .semantic_search_filtered(&query, "/project", 10, Some("ollama"))
            .unwrap();
        assert_eq!(ollama_results.len(), 1);
        assert_eq!(ollama_results[0].chunk_text, "ollama chunk");
    }

    #[test]
    fn delete_embeddings_by_provider() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        // Insert tfidf embeddings
        store
            .upsert_chunk_embedding("/project", "src/a.rs", 0, "chunk a", &emb)
            .unwrap();
        store
            .upsert_chunk_embedding("/project", "src/b.rs", 0, "chunk b", &emb)
            .unwrap();

        // Insert an ollama embedding (different file)
        store
            .upsert_chunk_embedding_with_provider(
                "/project",
                "src/c.rs",
                0,
                "chunk c",
                &emb,
                Some("ollama"),
                Some("nomic-embed-text"),
                Some(768),
            )
            .unwrap();

        assert_eq!(store.count_embeddings("/project").unwrap(), 3);

        // Delete only tfidf embeddings
        let deleted = store
            .delete_embeddings_by_provider("/project", "tfidf")
            .unwrap();
        assert_eq!(deleted, 2);

        // Only ollama embedding remains
        let remaining = store.get_embeddings_for_project("/project").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, "src/c.rs");

        // Metadata should only show ollama
        let meta = store.get_embedding_metadata("/project").unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].provider_type, "ollama");
    }

    #[test]
    fn get_embedding_metadata_empty_project() {
        let store = create_test_store();
        let meta = store.get_embedding_metadata("/nonexistent").unwrap();
        assert!(meta.is_empty());
    }

    #[test]
    fn get_embedding_metadata_multiple_providers() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        store
            .upsert_chunk_embedding("/project", "src/a.rs", 0, "chunk a", &emb)
            .unwrap();

        store
            .upsert_chunk_embedding_with_provider(
                "/project",
                "src/b.rs",
                0,
                "chunk b",
                &emb,
                Some("ollama"),
                Some("nomic-embed-text"),
                Some(768),
            )
            .unwrap();

        store
            .upsert_chunk_embedding_with_provider(
                "/project",
                "src/c.rs",
                0,
                "chunk c",
                &emb,
                Some("ollama"),
                Some("mxbai-embed-large"),
                Some(1024),
            )
            .unwrap();

        let meta = store.get_embedding_metadata("/project").unwrap();
        // Should return 3 distinct combinations
        assert_eq!(meta.len(), 3);

        // Check that all are present
        let types: Vec<&str> = meta.iter().map(|m| m.provider_type.as_str()).collect();
        assert!(types.contains(&"tfidf"));
        assert!(types.contains(&"ollama"));

        let models: Vec<&str> = meta.iter().map(|m| m.provider_model.as_str()).collect();
        assert!(models.contains(&"tfidf-v1"));
        assert!(models.contains(&"nomic-embed-text"));
        assert!(models.contains(&"mxbai-embed-large"));
    }

    #[test]
    fn upsert_with_provider_updates_provider_on_conflict() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        // First insert as tfidf
        store
            .upsert_chunk_embedding("/project", "src/main.rs", 0, "original", &emb)
            .unwrap();

        let meta = store.get_embedding_metadata("/project").unwrap();
        assert_eq!(meta[0].provider_type, "tfidf");

        // Update same file+chunk with ollama provider
        store
            .upsert_chunk_embedding_with_provider(
                "/project",
                "src/main.rs",
                0,
                "updated",
                &emb,
                Some("ollama"),
                Some("nomic-embed-text"),
                Some(768),
            )
            .unwrap();

        // Should still be 1 row (upserted), now with ollama metadata
        let results = store.get_embeddings_for_project("/project").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "updated");

        let meta = store.get_embedding_metadata("/project").unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].provider_type, "ollama");
        assert_eq!(meta[0].provider_model, "nomic-embed-text");
        assert_eq!(meta[0].embedding_dimension, 768);
    }

    #[test]
    fn delete_embeddings_for_file_removes_all_providers() {
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        // Insert with default provider
        store
            .upsert_chunk_embedding("/project", "src/main.rs", 0, "chunk 0", &emb)
            .unwrap();

        // Delete should remove regardless of provider
        let deleted = store
            .delete_embeddings_for_file("/project", "src/main.rs")
            .unwrap();
        assert_eq!(deleted, 1);

        let remaining = store.get_embeddings_for_project("/project").unwrap();
        assert!(remaining.is_empty());
    }

    #[test]
    fn migration_idempotent_schema_has_provider_columns() {
        // Creating a new test store runs init_schema which includes the migration.
        // Creating another store against the same DB would re-run it (simulated by
        // the fact that create_test_store always runs init_schema on a fresh DB).
        // This test verifies that provider columns exist after init.
        let store = create_test_store();
        let emb: Vec<u8> = vec![0, 0, 128, 63];

        // If columns didn't exist, this would fail
        store
            .upsert_chunk_embedding_with_provider(
                "/project",
                "src/main.rs",
                0,
                "test",
                &emb,
                Some("openai"),
                Some("text-embedding-3-small"),
                Some(1536),
            )
            .unwrap();

        let meta = store.get_embedding_metadata("/project").unwrap();
        assert_eq!(meta[0].provider_type, "openai");
        assert_eq!(meta[0].embedding_dimension, 1536);
    }

    // =========================================================================
    // FTS5 sync tests (story-002)
    // =========================================================================

    /// Helper to count FTS matches via direct SQL on the pool.
    fn fts_symbol_count(store: &IndexStore, match_expr: &str) -> i64 {
        let conn = store.get_connection().unwrap();
        let query = format!(
            "SELECT COUNT(*) FROM symbol_fts WHERE symbol_fts MATCH '{}'",
            match_expr
        );
        conn.query_row(&query, [], |row| row.get(0)).unwrap_or(0)
    }

    fn fts_filepath_count(store: &IndexStore, match_expr: &str) -> i64 {
        let conn = store.get_connection().unwrap();
        let query = format!(
            "SELECT COUNT(*) FROM filepath_fts WHERE filepath_fts MATCH '{}'",
            match_expr
        );
        conn.query_row(&query, [], |row| row.get(0)).unwrap_or(0)
    }

    #[test]
    fn upsert_populates_symbol_fts() {
        let store = create_test_store();
        let item = make_item(
            "src/controller.rs",
            "backend",
            "rust",
            vec![
                SymbolInfo::basic("UserController".to_string(), SymbolKind::Struct, 5),
                SymbolInfo::basic("handle_request".to_string(), SymbolKind::Function, 15),
            ],
        );
        store.upsert_file_index("/project", &item, "h1").unwrap();

        // Verify FTS has entries for these symbols
        assert!(fts_symbol_count(&store, "\"UserController\"*") > 0,
            "UserController should be in symbol_fts");
        assert!(fts_symbol_count(&store, "\"handle_request\"*") > 0,
            "handle_request should be in symbol_fts");
    }

    #[test]
    fn upsert_populates_filepath_fts() {
        let store = create_test_store();
        let item = make_item("src/services/auth.rs", "backend", "rust", vec![]);
        store.upsert_file_index("/project", &item, "h1").unwrap();

        // Verify FTS has entry for this file path
        assert!(fts_filepath_count(&store, "\"auth\"*") > 0,
            "auth should be in filepath_fts");
    }

    #[test]
    fn upsert_update_replaces_fts_entries() {
        let store = create_test_store();

        // Initial insert with one symbol
        let item_v1 = make_item(
            "src/service.rs",
            "backend",
            "rust",
            vec![SymbolInfo::basic("old_function".to_string(), SymbolKind::Function, 1)],
        );
        store.upsert_file_index("/project", &item_v1, "h1").unwrap();
        assert!(fts_symbol_count(&store, "\"old_function\"*") > 0);

        // Update with different symbol
        let item_v2 = make_item(
            "src/service.rs",
            "backend",
            "rust",
            vec![SymbolInfo::basic("new_function".to_string(), SymbolKind::Function, 1)],
        );
        store.upsert_file_index("/project", &item_v2, "h2").unwrap();

        // Old symbol should be gone from FTS, new one present
        assert_eq!(fts_symbol_count(&store, "\"old_function\"*"), 0,
            "old_function should be removed from symbol_fts after update");
        assert!(fts_symbol_count(&store, "\"new_function\"*") > 0,
            "new_function should be in symbol_fts after update");
    }

    #[test]
    fn delete_project_clears_fts_entries() {
        let store = create_test_store();

        let item = make_item(
            "src/main.rs",
            "backend",
            "rust",
            vec![SymbolInfo::basic("main".to_string(), SymbolKind::Function, 1)],
        );
        store.upsert_file_index("/project", &item, "h1").unwrap();

        // Verify FTS entries exist
        assert!(fts_symbol_count(&store, "\"main\"*") > 0);
        assert!(fts_filepath_count(&store, "\"main\"*") > 0);

        // Delete the project index
        store.delete_project_index("/project").unwrap();

        // FTS entries should be cleared
        assert_eq!(fts_symbol_count(&store, "\"main\"*"), 0,
            "symbol_fts should be cleared after delete_project_index");
        assert_eq!(fts_filepath_count(&store, "\"main\"*"), 0,
            "filepath_fts should be cleared after delete_project_index");
    }

    #[test]
    fn fts_entries_isolated_by_project() {
        let store = create_test_store();

        let item_a = make_item(
            "src/main.rs",
            "backend",
            "rust",
            vec![SymbolInfo::basic("main_a".to_string(), SymbolKind::Function, 1)],
        );
        let item_b = make_item(
            "src/main.rs",
            "backend",
            "rust",
            vec![SymbolInfo::basic("main_b".to_string(), SymbolKind::Function, 1)],
        );

        store.upsert_file_index("/project-a", &item_a, "ha").unwrap();
        store.upsert_file_index("/project-b", &item_b, "hb").unwrap();

        // Delete project-a should only remove its FTS entries
        store.delete_project_index("/project-a").unwrap();

        assert_eq!(fts_symbol_count(&store, "\"main_a\"*"), 0,
            "main_a should be removed from symbol_fts");
        assert!(fts_symbol_count(&store, "\"main_b\"*") > 0,
            "main_b should remain in symbol_fts");
    }

    // =========================================================================
    // FTS5 query tests (story-003)
    // =========================================================================

    #[test]
    fn test_sanitize_fts_query_basic() {
        let result = sanitize_fts_query("hello world");
        assert_eq!(result, "\"hello\"* \"world\"*");
    }

    #[test]
    fn test_sanitize_fts_query_empty() {
        let result = sanitize_fts_query("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_sanitize_fts_query_special_chars() {
        // Double quotes in input should be escaped
        let result = sanitize_fts_query("test\"value");
        assert_eq!(result, "\"test\"\"value\"*");
    }

    #[test]
    fn test_sanitize_fts_query_single_token() {
        let result = sanitize_fts_query("controller");
        assert_eq!(result, "\"controller\"*");
    }

    #[test]
    fn fts_search_symbols_returns_bm25_ranked_results() {
        let store = create_test_store();

        // Insert files with symbols. FTS5 tokenizes CamelCase as single tokens,
        // and underscore_separated symbols use _ as a token char (part of token).
        // So we use snake_case names that match how FTS5 tokenizes them.
        let item1 = make_item(
            "src/controller.rs",
            "backend",
            "rust",
            vec![
                SymbolInfo::basic("user_controller".to_string(), SymbolKind::Struct, 1),
                SymbolInfo::basic("admin_controller".to_string(), SymbolKind::Function, 10),
            ],
        );
        let item2 = make_item(
            "src/handler.rs",
            "backend",
            "rust",
            vec![SymbolInfo::basic("request_handler".to_string(), SymbolKind::Struct, 1)],
        );

        store.upsert_file_index("/project", &item1, "h1").unwrap();
        store.upsert_file_index("/project", &item2, "h2").unwrap();

        // With tokenchars='_', "user_controller" tokenizes as a single token.
        // The query "user_controller" should match via prefix.
        let results = store.fts_search_symbols("user_controller", 10).unwrap();
        assert!(!results.is_empty(), "Should find user_controller symbol");
        assert_eq!(results[0].symbol_name, "user_controller");

        // Also test that the handler symbol is NOT returned for this query
        assert!(
            results.iter().all(|r| r.symbol_name != "request_handler"),
            "request_handler should not match user_controller query"
        );
    }

    #[test]
    fn fts_search_symbols_prefix_matching() {
        let store = create_test_store();

        let item = make_item(
            "src/service.rs",
            "backend",
            "rust",
            vec![SymbolInfo::basic("user_controller".to_string(), SymbolKind::Struct, 1)],
        );
        store.upsert_file_index("/project", &item, "h1").unwrap();

        // "user" should match "user_controller" via prefix matching
        // (with tokenchars='_', user_controller is a single token; "user"* matches prefix)
        let results = store.fts_search_symbols("user", 10).unwrap();
        assert!(!results.is_empty(), "Prefix 'user' should match 'user_controller'");
        assert_eq!(results[0].symbol_name, "user_controller");
    }

    #[test]
    fn fts_search_symbols_empty_query() {
        let store = create_test_store();

        let item = make_item(
            "src/main.rs",
            "backend",
            "rust",
            vec![SymbolInfo::basic("main".to_string(), SymbolKind::Function, 1)],
        );
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let results = store.fts_search_symbols("", 10).unwrap();
        assert!(results.is_empty(), "Empty query should return empty results");
    }

    #[test]
    fn fts_search_files_returns_results_filtered_by_project() {
        let store = create_test_store();

        let item_a = make_item("src/auth.rs", "backend", "rust", vec![]);
        let item_b = make_item("src/auth.rs", "backend", "rust", vec![]);

        store.upsert_file_index("/project-a", &item_a, "ha").unwrap();
        store.upsert_file_index("/project-b", &item_b, "hb").unwrap();

        let results = store.fts_search_files("auth", "/project-a", 10).unwrap();
        assert_eq!(results.len(), 1, "Should find exactly one match for project-a");
        assert_eq!(results[0].project_path, "/project-a");
    }

    #[test]
    fn fts_search_files_empty_query() {
        let store = create_test_store();

        let item = make_item("src/main.rs", "backend", "rust", vec![]);
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let results = store.fts_search_files("", "/project", 10).unwrap();
        assert!(results.is_empty(), "Empty query should return empty results");
    }

    #[test]
    fn fts_search_symbols_special_chars_safe() {
        let store = create_test_store();

        let item = make_item(
            "src/main.rs",
            "backend",
            "rust",
            vec![SymbolInfo::basic("main".to_string(), SymbolKind::Function, 1)],
        );
        store.upsert_file_index("/project", &item, "h1").unwrap();

        // Queries with special characters should not cause SQL errors
        let results = store.fts_search_symbols("test\"value OR DROP", 10).unwrap();
        // Just verify it doesn't panic/error
        let _ = results;
    }

    // =========================================================================
    // LSP enrichment method tests (feature-003, Phase 3)
    // =========================================================================

    fn setup_lsp_test_data(store: &IndexStore) -> i64 {
        let item = make_item(
            "src/main.rs",
            "backend",
            "rust",
            vec![
                SymbolInfo::basic("my_func".to_string(), SymbolKind::Function, 10),
                SymbolInfo::basic("MyStruct".to_string(), SymbolKind::Struct, 20),
            ],
        );
        store.upsert_file_index("/test", &item, "h1").unwrap();

        // Return the first symbol's rowid (scope connection to avoid pool exhaustion)
        let rowid: i64 = {
            let conn = store.get_connection().unwrap();
            conn.query_row(
                "SELECT id FROM file_symbols WHERE name = 'my_func'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        rowid
    }

    #[test]
    fn test_update_symbol_type() {
        let store = create_test_store();
        let rowid = setup_lsp_test_data(&store);

        store.update_symbol_type(rowid, "fn() -> i32").unwrap();

        let conn = store.get_connection().unwrap();
        let resolved: String = conn
            .query_row(
                "SELECT resolved_type FROM file_symbols WHERE id = ?1",
                params![rowid],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(resolved, "fn() -> i32");
    }

    #[test]
    fn test_update_reference_count() {
        let store = create_test_store();
        let rowid = setup_lsp_test_data(&store);

        store.update_reference_count(rowid, 42).unwrap();

        let conn = store.get_connection().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT reference_count FROM file_symbols WHERE id = ?1",
                params![rowid],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 42);
    }

    #[test]
    fn test_set_symbol_exported() {
        let store = create_test_store();
        let rowid = setup_lsp_test_data(&store);

        store.set_symbol_exported(rowid, true).unwrap();

        let conn = store.get_connection().unwrap();
        let exported: i32 = conn
            .query_row(
                "SELECT is_exported FROM file_symbols WHERE id = ?1",
                params![rowid],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exported, 1);
    }

    #[test]
    fn test_insert_cross_reference() {
        let store = create_test_store();

        store
            .insert_cross_reference(
                "/test",
                "src/main.rs",
                10,
                Some("call_foo"),
                "src/lib.rs",
                5,
                Some("foo"),
                "call",
            )
            .unwrap();

        let refs = store.get_cross_references("/test", "src/main.rs").unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].source_file, "src/main.rs");
        assert_eq!(refs[0].target_file, "src/lib.rs");
        assert_eq!(refs[0].reference_kind, "call");
    }

    #[test]
    fn test_insert_cross_reference_ignores_duplicates() {
        let store = create_test_store();

        // Insert the same cross-reference twice
        store
            .insert_cross_reference("/test", "a.rs", 1, None, "b.rs", 5, None, "usage")
            .unwrap();
        store
            .insert_cross_reference("/test", "a.rs", 1, None, "b.rs", 5, None, "usage")
            .unwrap();

        let refs = store.get_cross_references("/test", "a.rs").unwrap();
        assert_eq!(refs.len(), 1, "Duplicate should be ignored");
    }

    #[test]
    fn test_get_symbols_for_enrichment() {
        let store = create_test_store();

        let item = make_item(
            "src/main.rs",
            "backend",
            "rust",
            vec![
                SymbolInfo::basic("fn1".to_string(), SymbolKind::Function, 1),
                SymbolInfo::basic("fn2".to_string(), SymbolKind::Function, 10),
            ],
        );
        store.upsert_file_index("/test", &item, "h1").unwrap();

        let item2 = make_item(
            "src/app.py",
            "backend",
            "python",
            vec![SymbolInfo::basic("py_fn".to_string(), SymbolKind::Function, 1)],
        );
        store.upsert_file_index("/test", &item2, "h2").unwrap();

        // Query Rust symbols only
        let rust_symbols = store.get_symbols_for_enrichment("/test", "rust").unwrap();
        assert_eq!(rust_symbols.len(), 2);
        assert_eq!(rust_symbols[0].2, "fn1"); // symbol_name
        assert_eq!(rust_symbols[1].2, "fn2");

        // Query Python symbols only
        let py_symbols = store.get_symbols_for_enrichment("/test", "python").unwrap();
        assert_eq!(py_symbols.len(), 1);
        assert_eq!(py_symbols[0].2, "py_fn");
    }

    #[test]
    fn test_clear_enrichment_data() {
        let store = create_test_store();
        let rowid = setup_lsp_test_data(&store);

        // Set some enrichment data
        store.update_symbol_type(rowid, "fn() -> i32").unwrap();
        store.update_reference_count(rowid, 5).unwrap();
        store.set_symbol_exported(rowid, true).unwrap();
        store
            .insert_cross_reference("/test", "src/main.rs", 10, None, "b.rs", 5, None, "usage")
            .unwrap();

        // Clear enrichment data
        store.clear_enrichment_data("/test").unwrap();

        // Verify symbol fields are reset (use store methods instead of raw conn)
        // Query the symbol through a fresh connection scope
        {
            let conn = store.get_connection().unwrap();
            let (rt, rc, ex): (Option<String>, i64, i32) = conn
                .query_row(
                    "SELECT resolved_type, reference_count, is_exported FROM file_symbols WHERE id = ?1",
                    params![rowid],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();

            assert!(rt.is_none(), "resolved_type should be NULL after clear");
            assert_eq!(rc, 0, "reference_count should be 0 after clear");
            assert_eq!(ex, 0, "is_exported should be 0 after clear");
        }

        let refs = store.get_cross_references("/test", "src/main.rs").unwrap();
        assert!(refs.is_empty(), "cross_references should be empty after clear");
    }

    #[test]
    fn test_upsert_lsp_server() {
        let store = create_test_store();

        store
            .upsert_lsp_server("rust", "/usr/bin/rust-analyzer", "rust-analyzer", Some("v1.0"))
            .unwrap();

        let servers = store.get_lsp_servers().unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].language, "rust");
        assert_eq!(servers[0].binary_path, "/usr/bin/rust-analyzer");
        assert_eq!(servers[0].server_name, "rust-analyzer");
        assert_eq!(servers[0].version, Some("v1.0".to_string()));
    }

    #[test]
    fn test_upsert_lsp_server_replaces_existing() {
        let store = create_test_store();

        store
            .upsert_lsp_server("rust", "/old/path", "rust-analyzer", Some("v1.0"))
            .unwrap();
        store
            .upsert_lsp_server("rust", "/new/path", "rust-analyzer", Some("v2.0"))
            .unwrap();

        let servers = store.get_lsp_servers().unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].binary_path, "/new/path");
        assert_eq!(servers[0].version, Some("v2.0".to_string()));
    }

    #[test]
    fn test_get_lsp_servers_multiple() {
        let store = create_test_store();

        store
            .upsert_lsp_server("go", "/usr/bin/gopls", "gopls", None)
            .unwrap();
        store
            .upsert_lsp_server("rust", "/usr/bin/rust-analyzer", "rust-analyzer", Some("v1"))
            .unwrap();

        let servers = store.get_lsp_servers().unwrap();
        assert_eq!(servers.len(), 2);
        // Ordered by language
        assert_eq!(servers[0].language, "go");
        assert_eq!(servers[1].language, "rust");
    }
}
