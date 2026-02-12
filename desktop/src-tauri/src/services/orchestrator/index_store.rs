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
                symbol.line as i64,  // start_line = line
                symbol.end_line as i64,
            ])?;
        }

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
        let embedding_chunks: i64 = conn.query_row(
            "SELECT COUNT(*) FROM file_embeddings WHERE project_path = ?1",
            params![project_path],
            |row| row.get(0),
        ).unwrap_or(0);

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

    /// Delete all index entries for a project.
    pub fn delete_project_index(&self, project_path: &str) -> AppResult<usize> {
        let conn = self.get_connection()?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;

        let deleted = conn.execute(
            "DELETE FROM file_index WHERE project_path = ?1",
            params![project_path],
        )?;

        Ok(deleted)
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
    // Embedding storage methods (feature-003)
    // =========================================================================

    /// Insert or update a chunk embedding for a file.
    ///
    /// Uses `INSERT ... ON CONFLICT` to upsert on the
    /// `(project_path, file_path, chunk_index)` unique constraint.
    pub fn upsert_chunk_embedding(
        &self,
        project_path: &str,
        file_path: &str,
        chunk_index: i64,
        chunk_text: &str,
        embedding: &[u8],
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO file_embeddings (project_path, file_path, chunk_index, chunk_text, embedding, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
             ON CONFLICT(project_path, file_path, chunk_index) DO UPDATE SET
                 chunk_text = excluded.chunk_text,
                 embedding = excluded.embedding,
                 created_at = CURRENT_TIMESTAMP",
            params![project_path, file_path, chunk_index, chunk_text, embedding],
        )?;
        Ok(())
    }

    /// Retrieve all embeddings for a project.
    ///
    /// Returns a vector of `(file_path, chunk_index, chunk_text, embedding_bytes)`.
    pub fn get_embeddings_for_project(
        &self,
        project_path: &str,
    ) -> AppResult<Vec<(String, i64, String, Vec<u8>)>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT file_path, chunk_index, chunk_text, embedding
             FROM file_embeddings
             WHERE project_path = ?1
             ORDER BY file_path, chunk_index",
        )?;

        let rows = stmt
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

    /// Perform a semantic search over stored embeddings.
    ///
    /// Computes cosine similarity between `query_embedding` and every stored
    /// embedding for the given project, returning the top-k results ranked by
    /// descending similarity.
    pub fn semantic_search(
        &self,
        query_embedding: &[f32],
        project_path: &str,
        top_k: usize,
    ) -> AppResult<Vec<SemanticSearchResult>> {
        let rows = self.get_embeddings_for_project(project_path)?;

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
        scored.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));

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

    fn get_connection(
        &self,
    ) -> AppResult<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))
    }
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

    fn make_item(path: &str, component: &str, language: &str, symbols: Vec<SymbolInfo>) -> FileInventoryItem {
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
        let item = make_item("src/main.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("main".to_string(), SymbolKind::Function, 1),
        ]);

        store.upsert_file_index("/project", &item, "abc123").unwrap();

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
        let item_v1 = make_item("src/lib.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("init".to_string(), SymbolKind::Function, 5),
        ]);
        store.upsert_file_index("/project", &item_v1, "hash_v1").unwrap();

        // Update with new symbols
        let item_v2 = make_item("src/lib.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("init".to_string(), SymbolKind::Function, 5),
            SymbolInfo::basic("Config".to_string(), SymbolKind::Struct, 20),
        ]);
        store.upsert_file_index("/project", &item_v2, "hash_v2").unwrap();

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
        let item_v1 = make_item("src/service.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("foo".to_string(), SymbolKind::Function, 1),
            SymbolInfo::basic("bar".to_string(), SymbolKind::Function, 10),
            SymbolInfo::basic("Baz".to_string(), SymbolKind::Struct, 20),
        ]);
        store.upsert_file_index("/project", &item_v1, "h1").unwrap();

        // Update with only 1 symbol
        let item_v2 = make_item("src/service.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("new_fn".to_string(), SymbolKind::Function, 1),
        ]);
        store.upsert_file_index("/project", &item_v2, "h2").unwrap();

        let symbols = store.get_file_symbols("/project", "src/service.rs").unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "new_fn");
    }

    // =========================================================================
    // Symbol query tests
    // =========================================================================

    #[test]
    fn query_symbols_by_name_pattern() {
        let store = create_test_store();

        let item1 = make_item("src/controller.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("UserController".to_string(), SymbolKind::Struct, 5),
            SymbolInfo::basic("handle_request".to_string(), SymbolKind::Function, 15),
        ]);
        let item2 = make_item("src/admin.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("AdminController".to_string(), SymbolKind::Struct, 3),
        ]);

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

        let item = make_item("src/main.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("main".to_string(), SymbolKind::Function, 1),
        ]);
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let results = store.query_symbols("%NonExistent%").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn query_symbols_exact_name() {
        let store = create_test_store();

        let item = make_item("src/models.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("Config".to_string(), SymbolKind::Struct, 1),
            SymbolInfo::basic("ConfigBuilder".to_string(), SymbolKind::Struct, 20),
        ]);
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
        let item3 = make_item("src/components/App.tsx", "desktop-web", "typescript", vec![]);

        store.upsert_file_index("/project", &item1, "h1").unwrap();
        store.upsert_file_index("/project", &item2, "h2").unwrap();
        store.upsert_file_index("/project", &item3, "h3").unwrap();

        let rust_files = store.query_files_by_component("/project", "desktop-rust").unwrap();
        assert_eq!(rust_files.len(), 2);
        assert!(rust_files.iter().all(|f| f.component == "desktop-rust"));

        let web_files = store.query_files_by_component("/project", "desktop-web").unwrap();
        assert_eq!(web_files.len(), 1);
        assert_eq!(web_files[0].file_path, "src/components/App.tsx");
    }

    #[test]
    fn query_files_by_component_empty_result() {
        let store = create_test_store();

        let item = make_item("src/main.rs", "desktop-rust", "rust", vec![]);
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let files = store.query_files_by_component("/project", "nonexistent").unwrap();
        assert!(files.is_empty());
    }

    // =========================================================================
    // Project summary tests
    // =========================================================================

    #[test]
    fn get_project_summary_basic() {
        let store = create_test_store();

        let item1 = make_item("src/main.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("main".to_string(), SymbolKind::Function, 1),
        ]);
        let item2 = make_item("src/app.tsx", "desktop-web", "typescript", vec![
            SymbolInfo::basic("app".to_string(), SymbolKind::Function, 1),
        ]);
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

        let stale = store.is_index_stale("/project", "src/missing.rs", "somehash").unwrap();
        assert!(stale);
    }

    #[test]
    fn is_index_stale_returns_false_for_matching_hash() {
        let store = create_test_store();

        let item = make_item("src/main.rs", "desktop-rust", "rust", vec![]);
        store.upsert_file_index("/project", &item, "abc123").unwrap();

        let stale = store.is_index_stale("/project", "src/main.rs", "abc123").unwrap();
        assert!(!stale);
    }

    #[test]
    fn is_index_stale_returns_true_for_different_hash() {
        let store = create_test_store();

        let item = make_item("src/main.rs", "desktop-rust", "rust", vec![]);
        store.upsert_file_index("/project", &item, "abc123").unwrap();

        let stale = store.is_index_stale("/project", "src/main.rs", "def456").unwrap();
        assert!(stale);
    }

    // =========================================================================
    // Foreign key cascade delete tests
    // =========================================================================

    #[test]
    fn cascade_delete_removes_symbols() {
        let store = create_test_store();

        let item = make_item("src/main.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("main".to_string(), SymbolKind::Function, 1),
            SymbolInfo::basic("Config".to_string(), SymbolKind::Struct, 10),
        ]);
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

        let item_a = make_item("src/main.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("main_a".to_string(), SymbolKind::Function, 1),
        ]);
        let item_b = make_item("src/main.rs", "desktop-rust", "rust", vec![
            SymbolInfo::basic("main_b".to_string(), SymbolKind::Function, 1),
        ]);

        store.upsert_file_index("/project-a", &item_a, "ha").unwrap();
        store.upsert_file_index("/project-b", &item_b, "hb").unwrap();

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

        let item = make_item("src/all_kinds.rs", "desktop-rust", "rust", all_kinds.clone());
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let stored = store.get_file_symbols("/project", "src/all_kinds.rs").unwrap();
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
                .map(|j| SymbolInfo::basic(
                    format!("symbol_{i}_{j}"),
                    SymbolKind::Function,
                    j + 1,
                ))
                .collect();

            let item = make_item(
                &format!("src/module_{i}.rs"),
                "desktop-rust",
                "rust",
                symbols,
            );
            store.upsert_file_index("/project", &item, &format!("hash_{i}")).unwrap();
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

        let stored = store.get_file_symbols("/project", "src/example.py").unwrap();
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
        assert_eq!(stored[1].signature.as_deref(), Some("def my_method(self, x: int) -> str:"));
        assert_eq!(stored[1].doc_comment.as_deref(), Some("Does something useful"));
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

        let deleted = store
            .delete_embeddings_for_project("/project")
            .unwrap();
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

        store.upsert_chunk_embedding("/project", "a.rs", 0, "chunk a", &embedding_to_bytes(&vec1)).unwrap();
        store.upsert_chunk_embedding("/project", "b.rs", 0, "chunk b", &embedding_to_bytes(&vec2)).unwrap();
        store.upsert_chunk_embedding("/project", "c.rs", 0, "chunk c", &embedding_to_bytes(&vec3)).unwrap();

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
            store.upsert_chunk_embedding("/project", &format!("file_{}.rs", i), 0, &format!("chunk {}", i), &emb).unwrap();
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
        store.upsert_chunk_embedding("/project", "a.rs", 0, "chunk", &emb).unwrap();

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

    #[test]
    fn query_symbols_returns_extended_fields() {
        let store = create_test_store();

        let symbols = vec![
            SymbolInfo {
                name: "process".to_string(),
                kind: SymbolKind::Function,
                line: 10,
                parent: Some("Handler".to_string()),
                signature: Some("fn process(&self, data: &[u8]) -> Result<()>".to_string()),
                doc_comment: Some("Process incoming data".to_string()),
                end_line: 30,
            },
        ];

        let item = make_item("src/handler.rs", "desktop-rust", "rust", symbols);
        store.upsert_file_index("/project", &item, "h1").unwrap();

        let results = store.query_symbols("%process%").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].parent_symbol.as_deref(), Some("Handler"));
        assert_eq!(results[0].signature.as_deref(), Some("fn process(&self, data: &[u8]) -> Result<()>"));
        assert_eq!(results[0].doc_comment.as_deref(), Some("Process incoming data"));
        assert_eq!(results[0].end_line, 30);
    }
}
