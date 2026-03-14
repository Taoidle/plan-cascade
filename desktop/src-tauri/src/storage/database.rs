//! SQLite Database
//!
//! Embedded database for persistent storage using rusqlite with r2d2 connection pooling.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use crate::utils::error::{AppError, AppResult};
use crate::utils::paths::database_path;

/// Raw execution row from the database
#[derive(Debug, Clone)]
pub struct ExecutionRow {
    pub id: String,
    pub session_id: Option<String>,
    pub name: String,
    pub execution_mode: String,
    pub status: String,
    pub project_path: String,
    pub total_stories: i32,
    pub completed_stories: i32,
    pub current_story_id: Option<String>,
    pub progress: f64,
    pub context_snapshot: String,
    pub error_message: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub completed_at: Option<String>,
}

/// Raw checkpoint row from the database
#[derive(Debug, Clone)]
pub struct CheckpointRow {
    pub id: String,
    pub session_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub snapshot: String,
    pub created_at: Option<String>,
}

/// Raw MCP catalog cache row.
#[derive(Debug, Clone)]
pub struct McpCatalogCacheRow {
    pub source_id: String,
    pub payload_json: String,
    pub signature: Option<String>,
    pub etag: Option<String>,
    pub fetched_at: Option<String>,
    pub expires_at: Option<String>,
}

/// Raw MCP installer job row.
#[derive(Debug, Clone)]
pub struct McpInstallJobRow {
    pub job_id: String,
    pub item_id: String,
    pub server_id: Option<String>,
    pub phase: String,
    pub progress: f64,
    pub status: String,
    pub error_class: Option<String>,
    pub error_message: Option<String>,
    pub logs_json: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Type alias for the connection pool
pub type DbPool = Pool<SqliteConnectionManager>;

/// Database service for managing SQLite operations
#[derive(Clone)]
pub struct Database {
    pool: DbPool,
}

impl Database {
    /// Create a database from an existing connection pool.
    ///
    /// Useful when a component needs a `Database` instance but only has
    /// access to a `DbPool` (e.g. `IndexManager` resolving proxy settings).
    pub fn from_pool(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Create a database from an existing pool (for testing).
    #[cfg(test)]
    pub fn from_pool_for_test(pool: DbPool) -> Self {
        Self::from_pool(pool)
    }

    /// Create an in-memory database for testing.
    ///
    /// Uses an in-memory SQLite database with the same schema as the
    /// production database. Useful for integration and unit tests.
    pub fn new_in_memory() -> AppResult<Self> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder()
            .max_size(1)
            .build(manager)
            .map_err(|e| AppError::database(format!("Failed to create connection pool: {}", e)))?;

        let db = Self { pool };
        db.init_schema()?;
        Ok(db)
    }

    /// Create a new database instance with connection pooling
    pub fn new() -> AppResult<Self> {
        let db_path = database_path()?;

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::builder()
            .max_size(10)
            .build(manager)
            .map_err(|e| AppError::database(format!("Failed to create connection pool: {}", e)))?;

        let db = Self { pool };
        db.init_schema()?;

        Ok(db)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> AppResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Create settings table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Create sessions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                name TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                metadata TEXT
            )",
            [],
        )?;

        // Create analytics table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS analytics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                event_type TEXT NOT NULL,
                provider TEXT,
                model TEXT,
                input_tokens INTEGER DEFAULT 0,
                output_tokens INTEGER DEFAULT 0,
                cost REAL DEFAULT 0.0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                metadata TEXT,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
            [],
        )?;

        // Create agents table with all required fields
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                system_prompt TEXT NOT NULL,
                model TEXT NOT NULL DEFAULT 'claude-sonnet-4-20250514',
                allowed_tools TEXT DEFAULT '[]',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Create index on agent name for efficient lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agents_name ON agents(name)",
            [],
        )?;

        // Create agent_runs table for execution history
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_runs (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                input TEXT NOT NULL,
                output TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                duration_ms INTEGER,
                input_tokens INTEGER,
                output_tokens INTEGER,
                error TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                completed_at TEXT,
                FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create indexes on agent_runs for efficient queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_agent_id ON agent_runs(agent_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_created_at ON agent_runs(created_at DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_status ON agent_runs(status)",
            [],
        )?;

        // Create mcp_servers table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mcp_servers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                server_type TEXT NOT NULL DEFAULT 'stdio',
                command TEXT,
                args TEXT,
                env TEXT,
                url TEXT,
                headers TEXT,
                has_env_secret INTEGER NOT NULL DEFAULT 0,
                has_headers_secret INTEGER NOT NULL DEFAULT 0,
                enabled INTEGER DEFAULT 1,
                auto_connect INTEGER NOT NULL DEFAULT 1,
                status TEXT DEFAULT 'unknown',
                last_error TEXT,
                last_connected_at TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                last_checked TEXT,
                managed_install INTEGER NOT NULL DEFAULT 0,
                catalog_item_id TEXT,
                trust_level TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Migration: add MCP columns introduced after initial releases.
        if !Self::table_has_column(&conn, "mcp_servers", "has_env_secret") {
            let _ = conn.execute(
                "ALTER TABLE mcp_servers ADD COLUMN has_env_secret INTEGER NOT NULL DEFAULT 0",
                [],
            );
        }
        if !Self::table_has_column(&conn, "mcp_servers", "has_headers_secret") {
            let _ = conn.execute(
                "ALTER TABLE mcp_servers ADD COLUMN has_headers_secret INTEGER NOT NULL DEFAULT 0",
                [],
            );
        }
        if !Self::table_has_column(&conn, "mcp_servers", "auto_connect") {
            let _ = conn.execute(
                "ALTER TABLE mcp_servers ADD COLUMN auto_connect INTEGER NOT NULL DEFAULT 1",
                [],
            );
        }
        if !Self::table_has_column(&conn, "mcp_servers", "last_error") {
            let _ = conn.execute("ALTER TABLE mcp_servers ADD COLUMN last_error TEXT", []);
        }
        if !Self::table_has_column(&conn, "mcp_servers", "last_connected_at") {
            let _ = conn.execute(
                "ALTER TABLE mcp_servers ADD COLUMN last_connected_at TEXT",
                [],
            );
        }
        if !Self::table_has_column(&conn, "mcp_servers", "retry_count") {
            let _ = conn.execute(
                "ALTER TABLE mcp_servers ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0",
                [],
            );
        }
        if !Self::table_has_column(&conn, "mcp_servers", "managed_install") {
            let _ = conn.execute(
                "ALTER TABLE mcp_servers ADD COLUMN managed_install INTEGER NOT NULL DEFAULT 0",
                [],
            );
        }
        if !Self::table_has_column(&conn, "mcp_servers", "catalog_item_id") {
            let _ = conn.execute(
                "ALTER TABLE mcp_servers ADD COLUMN catalog_item_id TEXT",
                [],
            );
        }
        if !Self::table_has_column(&conn, "mcp_servers", "trust_level") {
            let _ = conn.execute("ALTER TABLE mcp_servers ADD COLUMN trust_level TEXT", []);
        }

        // Normalize legacy transport label.
        let _ = conn.execute(
            "UPDATE mcp_servers SET server_type = 'stream_http' WHERE server_type = 'sse'",
            [],
        );

        // Ensure case-insensitive MCP server names are unique before adding unique index.
        Self::normalize_mcp_server_duplicate_names(&conn)?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_mcp_servers_name_ci_unique
             ON mcp_servers(LOWER(name))",
            [],
        )?;

        // MCP catalog cache (signed payload + TTL metadata).
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mcp_catalog_cache (
                source_id TEXT PRIMARY KEY,
                payload_json TEXT NOT NULL,
                signature TEXT,
                etag TEXT,
                fetched_at TEXT DEFAULT CURRENT_TIMESTAMP,
                expires_at TEXT
            )",
            [],
        )?;

        // MCP managed installation records.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mcp_install_records (
                server_id TEXT PRIMARY KEY,
                catalog_item_id TEXT NOT NULL,
                catalog_version TEXT,
                strategy_id TEXT NOT NULL,
                trust_level TEXT NOT NULL,
                package_lock_json TEXT,
                runtime_snapshot_json TEXT,
                installed_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Runtime inventory collected by MCP runtime manager.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mcp_runtime_inventory (
                runtime_key TEXT PRIMARY KEY,
                runtime_kind TEXT NOT NULL,
                version TEXT,
                executable_path TEXT,
                source TEXT,
                managed INTEGER NOT NULL DEFAULT 0,
                health_status TEXT NOT NULL DEFAULT 'unknown',
                last_error TEXT,
                last_checked_at TEXT,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Installer jobs for observability and retry.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mcp_install_jobs (
                job_id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL,
                server_id TEXT,
                phase TEXT NOT NULL,
                progress REAL NOT NULL DEFAULT 0.0,
                status TEXT NOT NULL,
                error_class TEXT,
                error_message TEXT,
                logs_json TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_mcp_install_jobs_status
             ON mcp_install_jobs(status, updated_at DESC)",
            [],
        )?;

        // Create checkpoints table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                name TEXT,
                description TEXT,
                snapshot TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
            [],
        )?;

        // Create interviews table for spec interview persistence
        conn.execute(
            "CREATE TABLE IF NOT EXISTS interviews (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL DEFAULT 'in_progress',
                phase TEXT NOT NULL DEFAULT 'overview',
                flow_level TEXT NOT NULL DEFAULT 'standard',
                first_principles INTEGER NOT NULL DEFAULT 0,
                max_questions INTEGER NOT NULL DEFAULT 18,
                question_cursor INTEGER NOT NULL DEFAULT 0,
                description TEXT NOT NULL DEFAULT '',
                project_path TEXT,
                spec_data TEXT NOT NULL DEFAULT '{}',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Create interview_turns table for conversation history
        conn.execute(
            "CREATE TABLE IF NOT EXISTS interview_turns (
                id TEXT PRIMARY KEY,
                interview_id TEXT NOT NULL,
                turn_number INTEGER NOT NULL,
                phase TEXT NOT NULL,
                question TEXT NOT NULL,
                answer TEXT NOT NULL DEFAULT '',
                field_name TEXT NOT NULL DEFAULT '',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (interview_id) REFERENCES interviews(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create executions table for tracking execution lifecycle
        conn.execute(
            "CREATE TABLE IF NOT EXISTS executions (
                id TEXT PRIMARY KEY,
                session_id TEXT,
                name TEXT NOT NULL DEFAULT '',
                execution_mode TEXT NOT NULL DEFAULT 'direct',
                status TEXT NOT NULL DEFAULT 'pending',
                project_path TEXT NOT NULL DEFAULT '',
                total_stories INTEGER NOT NULL DEFAULT 0,
                completed_stories INTEGER NOT NULL DEFAULT 0,
                current_story_id TEXT,
                progress REAL NOT NULL DEFAULT 0.0,
                context_snapshot TEXT NOT NULL DEFAULT '{}',
                error_message TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                completed_at TEXT,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
            [],
        )?;

        // Indexes for execution queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_executions_status ON executions(status)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_executions_session_id ON executions(session_id)",
            [],
        )?;

        // Indexes for interview queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_interview_turns_interview_id
             ON interview_turns(interview_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_interview_turns_turn_number
             ON interview_turns(interview_id, turn_number)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_interviews_status
             ON interviews(status)",
            [],
        )?;

        // Enable foreign keys (must be set per-connection in SQLite)
        conn.execute_batch("PRAGMA foreign_keys = ON")?;

        // Create file_index table for persistent file index storage
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_index (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                file_path TEXT NOT NULL,
                component TEXT NOT NULL DEFAULT '',
                language TEXT NOT NULL DEFAULT '',
                extension TEXT,
                size_bytes INTEGER NOT NULL DEFAULT 0,
                line_count INTEGER NOT NULL DEFAULT 0,
                is_test INTEGER NOT NULL DEFAULT 0,
                content_hash TEXT NOT NULL DEFAULT '',
                modified_at TEXT,
                indexed_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(project_path, file_path)
            )",
            [],
        )?;

        // Create file_symbols table with cascade delete
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_symbols (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_index_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                line_number INTEGER NOT NULL DEFAULT 0,
                parent_symbol TEXT,
                signature TEXT,
                doc_comment TEXT,
                start_line INTEGER NOT NULL DEFAULT 0,
                end_line INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (file_index_id) REFERENCES file_index(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Migration: add new columns to existing file_symbols tables that lack them.
        // SQLite doesn't support IF NOT EXISTS for ALTER TABLE ADD COLUMN, so we
        // check column existence via PRAGMA and add only missing columns.
        {
            let has_parent = Self::table_has_column(&conn, "file_symbols", "parent_symbol");
            if !has_parent {
                let _ = conn.execute_batch(
                    "ALTER TABLE file_symbols ADD COLUMN parent_symbol TEXT;
                     ALTER TABLE file_symbols ADD COLUMN signature TEXT;
                     ALTER TABLE file_symbols ADD COLUMN doc_comment TEXT;
                     ALTER TABLE file_symbols ADD COLUMN start_line INTEGER NOT NULL DEFAULT 0;
                     ALTER TABLE file_symbols ADD COLUMN end_line INTEGER NOT NULL DEFAULT 0;",
                );
            }
        }

        // Indexes for file_index queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_file_index_project_path
             ON file_index(project_path)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_file_index_component
             ON file_index(project_path, component)",
            [],
        )?;

        // Indexes for file_symbols queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_file_symbols_file_index_id
             ON file_symbols(file_index_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_file_symbols_name
             ON file_symbols(name)",
            [],
        )?;

        // Create prompts table for prompt library
        conn.execute(
            "CREATE TABLE IF NOT EXISTS prompts (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                description TEXT,
                category TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '[]',
                variables TEXT NOT NULL DEFAULT '[]',
                is_builtin INTEGER NOT NULL DEFAULT 0,
                is_pinned INTEGER NOT NULL DEFAULT 0,
                use_count INTEGER NOT NULL DEFAULT 0,
                last_used_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_prompts_category ON prompts(category)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_prompts_pinned ON prompts(is_pinned DESC, use_count DESC)",
            [],
        )?;

        conn.execute(
            "UPDATE prompts
             SET category = ''
             WHERE lower(trim(category)) = 'custom'",
            [],
        )?;

        // Create file_embeddings table for vector embedding storage (feature-003)
        // provider_type, provider_model, embedding_dimension added in story-012
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_embeddings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                file_path TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                chunk_text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                provider_type TEXT NOT NULL DEFAULT 'tfidf',
                provider_model TEXT NOT NULL DEFAULT 'tfidf-v1',
                embedding_dimension INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(project_path, file_path, chunk_index)
            )",
            [],
        )?;

        // Indexes for file_embeddings queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_file_embeddings_project
             ON file_embeddings(project_path)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_file_embeddings_file
             ON file_embeddings(project_path, file_path)",
            [],
        )?;

        // Migration: add provider and dimension columns to file_embeddings (story-012).
        // Existing rows get default values: provider_type='tfidf', provider_model='tfidf-v1',
        // embedding_dimension=0 — preserving full backward compatibility.
        {
            let has_provider_type =
                Self::table_has_column(&conn, "file_embeddings", "provider_type");
            if !has_provider_type {
                let _ = conn.execute_batch(
                    "ALTER TABLE file_embeddings ADD COLUMN provider_type TEXT NOT NULL DEFAULT 'tfidf';
                     ALTER TABLE file_embeddings ADD COLUMN provider_model TEXT NOT NULL DEFAULT 'tfidf-v1';
                     ALTER TABLE file_embeddings ADD COLUMN embedding_dimension INTEGER NOT NULL DEFAULT 0;",
                );
            }
        }

        // Index for provider-filtered queries on file_embeddings
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_file_embeddings_provider
             ON file_embeddings(project_path, provider_type)",
            [],
        )?;

        // Create embedding_vocabulary table for persisting TF-IDF vocabulary (feature-003)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS embedding_vocabulary (
                project_path TEXT PRIMARY KEY,
                vocab_json BLOB NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Create component_mappings table for caching LLM/heuristic-derived
        // prefix→component classifications per project.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS component_mappings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                prefix TEXT NOT NULL,
                component_name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT 'heuristic',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(project_path, prefix)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_component_mappings_project
             ON component_mappings(project_path)",
            [],
        )?;

        // ====================================================================
        // Feature-003 (Phase 3): LSP Enhancement Layer
        // ====================================================================

        // Migration: add LSP enrichment columns to file_symbols table.
        // These columns are populated asynchronously by the LspEnricher after
        // Tree-sitter indexing completes. Defaults preserve backward compatibility.
        {
            let has_resolved_type = Self::table_has_column(&conn, "file_symbols", "resolved_type");
            if !has_resolved_type {
                let _ = conn.execute_batch(
                    "ALTER TABLE file_symbols ADD COLUMN resolved_type TEXT;
                     ALTER TABLE file_symbols ADD COLUMN reference_count INTEGER DEFAULT 0;
                     ALTER TABLE file_symbols ADD COLUMN is_exported BOOLEAN DEFAULT 0;",
                );
            }
        }

        // Cross-reference table: source -> target relationships from LSP
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cross_references (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                source_file TEXT NOT NULL,
                source_line INTEGER NOT NULL,
                source_symbol TEXT,
                target_file TEXT NOT NULL,
                target_line INTEGER NOT NULL,
                target_symbol TEXT,
                reference_kind TEXT NOT NULL DEFAULT 'usage',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(project_path, source_file, source_line, target_file, target_line)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cross_refs_source
             ON cross_references(project_path, source_file)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cross_refs_target
             ON cross_references(project_path, target_file, target_symbol)",
            [],
        )?;

        // LSP server detection cache table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS lsp_servers (
                language TEXT PRIMARY KEY,
                binary_path TEXT NOT NULL,
                server_name TEXT NOT NULL,
                version TEXT,
                detected_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // ====================================================================
        // Feature-001: Project Memory System tables
        // ====================================================================

        let has_legacy_project_memories = Self::table_exists(&conn, "project_memories");
        if has_legacy_project_memories {
            // Migration: add decay tracking column for incremental/idempotent decay.
            if !Self::table_has_column(&conn, "project_memories", "last_decay_at") {
                let _ = conn
                    .execute_batch("ALTER TABLE project_memories ADD COLUMN last_decay_at TEXT;");
            }

            // Migration: embedding metadata fields for memory provider awareness.
            if !Self::table_has_column(&conn, "project_memories", "embedding_provider") {
                let _ = conn.execute_batch(
                    "ALTER TABLE project_memories ADD COLUMN embedding_provider TEXT NOT NULL DEFAULT 'tfidf';",
                );
            }
            if !Self::table_has_column(&conn, "project_memories", "embedding_dim") {
                let _ = conn.execute_batch(
                    "ALTER TABLE project_memories ADD COLUMN embedding_dim INTEGER NOT NULL DEFAULT 0;",
                );
            }
            if !Self::table_has_column(&conn, "project_memories", "quality_score") {
                let _ = conn.execute_batch(
                    "ALTER TABLE project_memories ADD COLUMN quality_score REAL NOT NULL DEFAULT 1.0;",
                );
            }
        }

        if has_legacy_project_memories
            && !Self::table_exists(&conn, "project_memories_legacy_backup_v2")
        {
            let _ = conn.execute_batch(
                "ALTER TABLE project_memories RENAME TO project_memories_legacy_backup_v2;",
            );
        }

        // ====================================================================
        // Memory V2: explicit scope/status model + unified retrieval substrate
        // ====================================================================
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memory_entries_v2 (
                id TEXT PRIMARY KEY,
                scope TEXT NOT NULL CHECK(scope IN ('global', 'project', 'session')),
                project_path TEXT,
                session_id TEXT,
                category TEXT NOT NULL CHECK(category IN (
                    'preference',
                    'convention',
                    'pattern',
                    'correction',
                    'fact'
                )),
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL DEFAULT '',
                keywords TEXT NOT NULL DEFAULT '[]',
                embedding BLOB,
                importance REAL NOT NULL DEFAULT 0.5,
                access_count INTEGER NOT NULL DEFAULT 0,
                source_session_id TEXT,
                source_context TEXT,
                status TEXT NOT NULL DEFAULT 'active' CHECK(status IN (
                    'active',
                    'pending_review',
                    'rejected',
                    'archived',
                    'deleted'
                )),
                deleted_from_status TEXT CHECK(deleted_from_status IN (
                    'active',
                    'rejected',
                    'archived'
                )),
                risk_tier TEXT NOT NULL DEFAULT 'high' CHECK(risk_tier IN ('low', 'medium', 'high')),
                conflict_flag INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_decay_at TEXT,
                embedding_provider TEXT NOT NULL DEFAULT 'tfidf',
                embedding_dim INTEGER NOT NULL DEFAULT 0,
                quality_score REAL NOT NULL DEFAULT 1.0
            )",
            [],
        )?;
        Self::ensure_memory_entries_v2_schema(&conn)?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_entries_v2_unique_content
             ON memory_entries_v2(scope, IFNULL(project_path, ''), IFNULL(session_id, ''), content_hash)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_entries_v2_scope_project_session
             ON memory_entries_v2(scope, project_path, session_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_entries_v2_status_risk
             ON memory_entries_v2(status, risk_tier, updated_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_entries_v2_category_importance
             ON memory_entries_v2(category, importance DESC)",
            [],
        )?;

        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts_v2 USING fts5(
                memory_id UNINDEXED,
                content,
                keywords,
                tokenize=\"unicode61 remove_diacritics 2 tokenchars '_'\"
            )",
        )?;

        // Backfill memory_entries_v2 from legacy backup table (idempotent).
        if Self::table_exists(&conn, "project_memories_legacy_backup_v2") {
            let _ = conn.execute_batch(
                "INSERT OR IGNORE INTO memory_entries_v2 (
                    id, scope, project_path, session_id, category, content, content_hash,
                    keywords, embedding, importance, access_count, source_session_id, source_context,
                    status, deleted_from_status, risk_tier, conflict_flag, created_at, updated_at, last_accessed_at,
                    last_decay_at, embedding_provider, embedding_dim, quality_score
                )
                SELECT
                    id,
                    CASE
                        WHEN project_path = '__global__' THEN 'global'
                        WHEN project_path LIKE '__session__:%' THEN 'session'
                        ELSE 'project'
                    END,
                    CASE
                        WHEN project_path = '__global__' THEN NULL
                        WHEN project_path LIKE '__session__:%' THEN NULL
                        ELSE project_path
                    END,
                    CASE
                        WHEN project_path LIKE '__session__:%' THEN substr(project_path, 13)
                        ELSE NULL
                    END,
                    category,
                    content,
                    lower(trim(content)),
                    keywords,
                    embedding,
                    importance,
                    access_count,
                    source_session_id,
                    source_context,
                    CASE
                        WHEN source_context LIKE 'llm_extract:%' OR source_context LIKE 'rule_extract:%'
                            THEN 'pending_review'
                        ELSE 'active'
                    END,
                    NULL,
                    CASE
                        WHEN source_context LIKE 'llm_extract:%' THEN 'medium'
                        WHEN source_context LIKE 'rule_extract:%' THEN 'low'
                        ELSE 'high'
                    END,
                    0,
                    created_at,
                    updated_at,
                    last_accessed_at,
                    last_decay_at,
                    COALESCE(embedding_provider, 'tfidf'),
                    COALESCE(embedding_dim, 0),
                    COALESCE(quality_score, 1.0)
                FROM project_memories_legacy_backup_v2",
            );
        }
        let _ = conn.execute_batch(
            "UPDATE memory_entries_v2 AS pending
             SET conflict_flag = CASE
                 WHEN pending.status = 'pending_review'
                      AND EXISTS (
                          SELECT 1
                          FROM memory_entries_v2 AS active
                          WHERE active.id <> pending.id
                            AND active.status = 'active'
                            AND active.scope = pending.scope
                            AND IFNULL(active.project_path, '') = IFNULL(pending.project_path, '')
                            AND IFNULL(active.session_id, '') = IFNULL(pending.session_id, '')
                            AND active.category = pending.category
                      )
                 THEN 1
                 ELSE 0
             END",
        );
        let _ = conn.execute_batch(
            "DROP TRIGGER IF EXISTS trg_project_memories_to_v2_insert;
             DROP TRIGGER IF EXISTS trg_project_memories_to_v2_update;
             DROP TRIGGER IF EXISTS trg_project_memories_to_v2_delete;",
        );

        conn.execute_batch(
            "CREATE TRIGGER IF NOT EXISTS trg_memory_v2_fts_insert
             AFTER INSERT ON memory_entries_v2
             BEGIN
                 DELETE FROM memory_fts_v2 WHERE memory_id = NEW.id;
                 INSERT INTO memory_fts_v2(memory_id, content, keywords)
                 VALUES (NEW.id, NEW.content, NEW.keywords);
             END;

             CREATE TRIGGER IF NOT EXISTS trg_memory_v2_fts_update
             AFTER UPDATE ON memory_entries_v2
             BEGIN
                 DELETE FROM memory_fts_v2 WHERE memory_id = NEW.id;
                 INSERT INTO memory_fts_v2(memory_id, content, keywords)
                 VALUES (NEW.id, NEW.content, NEW.keywords);
             END;

             CREATE TRIGGER IF NOT EXISTS trg_memory_v2_fts_delete
             AFTER DELETE ON memory_entries_v2
             BEGIN
                 DELETE FROM memory_fts_v2 WHERE memory_id = OLD.id;
             END;

             CREATE TRIGGER IF NOT EXISTS trg_memory_v2_conflict_insert
             AFTER INSERT ON memory_entries_v2
             BEGIN
                 UPDATE memory_entries_v2
                 SET conflict_flag = CASE
                     WHEN NEW.status = 'pending_review'
                          AND EXISTS (
                              SELECT 1
                              FROM memory_entries_v2 AS active
                              WHERE active.id <> NEW.id
                                AND active.status = 'active'
                                AND active.scope = NEW.scope
                                AND IFNULL(active.project_path, '') = IFNULL(NEW.project_path, '')
                                AND IFNULL(active.session_id, '') = IFNULL(NEW.session_id, '')
                                AND active.category = NEW.category
                          )
                     THEN 1
                     ELSE 0
                 END
                 WHERE id = NEW.id;
             END;

             CREATE TRIGGER IF NOT EXISTS trg_memory_v2_conflict_update
             AFTER UPDATE OF status, scope, project_path, session_id, category ON memory_entries_v2
             BEGIN
                 UPDATE memory_entries_v2
                 SET conflict_flag = CASE
                     WHEN status = 'pending_review'
                          AND EXISTS (
                              SELECT 1
                              FROM memory_entries_v2 AS active
                              WHERE active.id <> memory_entries_v2.id
                                AND active.status = 'active'
                                AND active.scope = memory_entries_v2.scope
                                AND IFNULL(active.project_path, '') = IFNULL(memory_entries_v2.project_path, '')
                                AND IFNULL(active.session_id, '') = IFNULL(memory_entries_v2.session_id, '')
                                AND active.category = memory_entries_v2.category
                          )
                     THEN 1
                     ELSE 0
                 END
                 WHERE scope = NEW.scope
                   AND IFNULL(project_path, '') = IFNULL(NEW.project_path, '')
                   AND IFNULL(session_id, '') = IFNULL(NEW.session_id, '')
                   AND category = NEW.category;
             END;",
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS memory_review_audit_v2 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id TEXT NOT NULL,
                decision TEXT NOT NULL CHECK(decision IN (
                    'approve',
                    'reject',
                    'archive',
                    'restore',
                    'restore_deleted',
                    'restore_active',
                    'delete',
                    'purge'
                )),
                operator TEXT NOT NULL DEFAULT 'system',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;
        Self::ensure_memory_review_audit_v2_schema(&conn)?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_review_audit_v2_created
             ON memory_review_audit_v2(created_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_review_audit_v2_memory
             ON memory_review_audit_v2(memory_id)",
            [],
        )?;

        // Embedding metadata registry for memory retrieval diagnostics.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memory_embedding_meta (
                id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                provider_model TEXT,
                embedding_dim INTEGER NOT NULL DEFAULT 0,
                version TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_embedding_meta_project
             ON memory_embedding_meta(project_path, updated_at DESC)",
            [],
        )?;

        // Context trace events for ContextEnvelope v2 observability.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS context_trace_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                trace_id TEXT NOT NULL,
                session_id TEXT,
                turn_id TEXT,
                event_type TEXT NOT NULL,
                source_kind TEXT,
                source_id TEXT,
                message TEXT NOT NULL,
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_context_trace_events_trace
             ON context_trace_events(trace_id, id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_context_trace_events_session_turn
             ON context_trace_events(session_id, turn_id, created_at)",
            [],
        )?;

        // Reusable context artifacts for handoff across turns/sessions.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS context_artifacts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                project_path TEXT NOT NULL,
                session_id TEXT,
                envelope_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_context_artifacts_project_updated
             ON context_artifacts(project_path, updated_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_context_artifacts_session_updated
             ON context_artifacts(session_id, updated_at DESC)",
            [],
        )?;

        // Chaos probe run history for context pipeline reliability validation.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS context_chaos_runs (
                run_id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                session_id TEXT,
                report_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_context_chaos_runs_project_created
             ON context_chaos_runs(project_path, created_at DESC)",
            [],
        )?;

        // UI execution history (session + turn level) for durable chat restore.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS execution_history_sessions (
                id TEXT PRIMARY KEY,
                title TEXT,
                task_description TEXT NOT NULL,
                workspace_path TEXT,
                strategy TEXT,
                status TEXT,
                started_at INTEGER NOT NULL,
                completed_at INTEGER,
                duration_ms INTEGER,
                completed_stories INTEGER NOT NULL DEFAULT 0,
                total_stories INTEGER NOT NULL DEFAULT 0,
                success INTEGER NOT NULL DEFAULT 0,
                error_message TEXT,
                conversation_content TEXT,
                conversation_lines_json TEXT,
                session_id TEXT,
                llm_backend TEXT,
                llm_provider TEXT,
                llm_model TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_execution_history_sessions_started
             ON execution_history_sessions(started_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_execution_history_sessions_session
             ON execution_history_sessions(session_id)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS execution_history_turns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                history_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                line_type TEXT NOT NULL,
                content TEXT NOT NULL,
                card_payload_json TEXT,
                sub_agent_id TEXT,
                sub_agent_depth INTEGER,
                turn_id INTEGER,
                turn_boundary TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(history_id, seq),
                FOREIGN KEY(history_id) REFERENCES execution_history_sessions(id) ON DELETE CASCADE
            )",
            [],
        )?;
        if !Self::table_has_column(&conn, "execution_history_turns", "card_payload_json") {
            let _ = conn.execute(
                "ALTER TABLE execution_history_turns ADD COLUMN card_payload_json TEXT",
                [],
            );
        }
        if !Self::table_has_column(&conn, "execution_history_turns", "turn_id") {
            let _ = conn.execute(
                "ALTER TABLE execution_history_turns ADD COLUMN turn_id INTEGER",
                [],
            );
        }
        if !Self::table_has_column(&conn, "execution_history_turns", "turn_boundary") {
            let _ = conn.execute(
                "ALTER TABLE execution_history_turns ADD COLUMN turn_boundary TEXT",
                [],
            );
        }
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_execution_history_turns_history_seq
             ON execution_history_turns(history_id, seq)",
            [],
        )?;

        // Episodic records for learning from past interactions
        conn.execute(
            "CREATE TABLE IF NOT EXISTS episodic_records (
                id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                session_id TEXT NOT NULL,
                record_type TEXT NOT NULL CHECK(record_type IN (
                    'success',
                    'failure',
                    'discovery'
                )),
                task_summary TEXT NOT NULL,
                approach_summary TEXT NOT NULL,
                outcome_summary TEXT NOT NULL,
                tools_used TEXT NOT NULL DEFAULT '[]',
                keywords TEXT NOT NULL DEFAULT '[]',
                embedding BLOB,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_episodic_records_project
             ON episodic_records(project_path)",
            [],
        )?;

        // ====================================================================
        // Feature-002: Skill System tables
        // ====================================================================

        // Create skill_library table for auto-generated skills from successful sessions
        conn.execute(
            "CREATE TABLE IF NOT EXISTS skill_library (
                id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                tags TEXT NOT NULL DEFAULT '[]',
                body TEXT NOT NULL,
                source_type TEXT NOT NULL CHECK(source_type IN ('generated', 'refined')),
                source_session_ids TEXT NOT NULL DEFAULT '[]',
                usage_count INTEGER NOT NULL DEFAULT 0,
                success_rate REAL NOT NULL DEFAULT 1.0,
                keywords TEXT NOT NULL DEFAULT '[]',
                embedding BLOB,
                enabled INTEGER NOT NULL DEFAULT 1,
                review_status TEXT NOT NULL DEFAULT 'pending_review',
                review_notes TEXT,
                reviewed_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        if !Self::table_has_column(&conn, "skill_library", "review_status") {
            let _ = conn.execute(
                "ALTER TABLE skill_library ADD COLUMN review_status TEXT NOT NULL DEFAULT 'pending_review'",
                [],
            );
        }
        if !Self::table_has_column(&conn, "skill_library", "review_notes") {
            let _ = conn.execute("ALTER TABLE skill_library ADD COLUMN review_notes TEXT", []);
        }
        if !Self::table_has_column(&conn, "skill_library", "reviewed_at") {
            let _ = conn.execute("ALTER TABLE skill_library ADD COLUMN reviewed_at TEXT", []);
        }

        // Indexes for skill_library queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_skill_library_project
             ON skill_library(project_path)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_skill_library_enabled
             ON skill_library(project_path, enabled)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_skill_library_review_status
             ON skill_library(project_path, review_status)",
            [],
        )?;

        // ====================================================================
        // Feature-002: FTS5 Full-Text Search virtual tables
        // ====================================================================

        // Symbol FTS5 virtual table (contentless mode)
        // Columns mirror file_symbols fields used for search.
        // tokenize: unicode61 with diacritic removal and underscore as token char
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS symbol_fts USING fts5(
                symbol_name,
                file_path,
                symbol_kind,
                doc_comment,
                signature,
                content='',
                contentless_delete=1,
                tokenize=\"unicode61 remove_diacritics 2 tokenchars '_'\"
            )",
        )?;

        // File path FTS5 virtual table (contentless mode)
        // tokenize: unicode61 with underscore as token char (keeps snake_case terms together).
        // Slashes and dots remain separators so "src/services/auth.rs" tokenizes as
        // [src, services, auth, rs] — enabling prefix matching on path segments.
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS filepath_fts USING fts5(
                file_path,
                component,
                language,
                content='',
                contentless_delete=1,
                tokenize=\"unicode61 tokenchars '_'\"
            )",
        )?;

        // ====================================================================
        // Feature-004: Guardrail Security System tables
        // ====================================================================

        // Unified guardrail rule storage (built-ins + custom rules)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS guardrail_rules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                guardrail_type TEXT NOT NULL DEFAULT 'custom',
                builtin_key TEXT,
                pattern TEXT,
                action TEXT NOT NULL DEFAULT 'warn',
                scope TEXT NOT NULL DEFAULT '[\"input\",\"assistant_output\",\"tool_result\"]',
                enabled INTEGER NOT NULL DEFAULT 1,
                editable INTEGER NOT NULL DEFAULT 1,
                description TEXT NOT NULL DEFAULT '',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Sanitized guardrail audit log
        conn.execute(
            "CREATE TABLE IF NOT EXISTS guardrail_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rule_id TEXT NOT NULL,
                rule_name TEXT NOT NULL,
                surface TEXT NOT NULL,
                tool_name TEXT,
                session_id TEXT,
                execution_id TEXT,
                decision TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                safe_preview TEXT NOT NULL DEFAULT '',
                timestamp TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_guardrail_rules_builtin_key
             ON guardrail_rules(builtin_key)
             WHERE builtin_key IS NOT NULL",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_guardrail_events_timestamp
             ON guardrail_events(timestamp DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_guardrail_events_rule_id
             ON guardrail_events(rule_id)",
            [],
        )?;

        if !Self::table_has_column(&conn, "guardrail_rules", "guardrail_type") {
            let _ = conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN guardrail_type TEXT NOT NULL DEFAULT 'custom'",
                [],
            );
        }
        if !Self::table_has_column(&conn, "guardrail_rules", "builtin_key") {
            let _ = conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN builtin_key TEXT",
                [],
            );
        }
        if !Self::table_has_column(&conn, "guardrail_rules", "scope") {
            let _ = conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN scope TEXT NOT NULL DEFAULT '[\"input\",\"assistant_output\",\"tool_result\"]'",
                [],
            );
        }
        if !Self::table_has_column(&conn, "guardrail_rules", "editable") {
            let _ = conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN editable INTEGER NOT NULL DEFAULT 1",
                [],
            );
        }
        if !Self::table_has_column(&conn, "guardrail_rules", "description") {
            let _ = conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN description TEXT NOT NULL DEFAULT ''",
                [],
            );
        }
        if !Self::table_has_column(&conn, "guardrail_rules", "updated_at") {
            let _ = conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN updated_at TEXT DEFAULT CURRENT_TIMESTAMP",
                [],
            );
        }
        let _ = conn.execute("DELETE FROM guardrail_trigger_log", []);

        // ====================================================================
        // Webhook Notification System tables
        // ====================================================================

        // Webhook channel configurations
        conn.execute(
            "CREATE TABLE IF NOT EXISTS webhook_channels (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                channel_type TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                url TEXT NOT NULL,
                scope_type TEXT NOT NULL DEFAULT 'global',
                scope_sessions TEXT,
                events TEXT NOT NULL,
                template TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Webhook delivery history (for audit and retry)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS webhook_deliveries (
                id TEXT PRIMARY KEY,
                channel_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                status TEXT NOT NULL,
                status_code INTEGER,
                response_body TEXT,
                attempts INTEGER NOT NULL DEFAULT 0,
                last_attempt_at TEXT,
                next_retry_at TEXT,
                last_error TEXT,
                retryable INTEGER,
                error_class TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (channel_id) REFERENCES webhook_channels(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Migration: add retry metadata columns for delivery reliability.
        if !Self::table_has_column(&conn, "webhook_deliveries", "next_retry_at") {
            let _ = conn.execute(
                "ALTER TABLE webhook_deliveries ADD COLUMN next_retry_at TEXT",
                [],
            );
        }
        if !Self::table_has_column(&conn, "webhook_deliveries", "last_error") {
            let _ = conn.execute(
                "ALTER TABLE webhook_deliveries ADD COLUMN last_error TEXT",
                [],
            );
        }
        if !Self::table_has_column(&conn, "webhook_deliveries", "retryable") {
            let _ = conn.execute(
                "ALTER TABLE webhook_deliveries ADD COLUMN retryable INTEGER",
                [],
            );
        }
        if !Self::table_has_column(&conn, "webhook_deliveries", "error_class") {
            let _ = conn.execute(
                "ALTER TABLE webhook_deliveries ADD COLUMN error_class TEXT",
                [],
            );
        }

        // Index for delivery retry queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_status
             ON webhook_deliveries(status, last_attempt_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_retry_due
             ON webhook_deliveries(status, next_retry_at, attempts)",
            [],
        )?;

        // ====================================================================
        // Feature-002 (Phase 2): Remote Session Control tables
        // ====================================================================

        // Remote session mappings (chat_id -> local session)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS remote_session_mappings (
                chat_id INTEGER NOT NULL,
                user_id INTEGER NOT NULL,
                adapter_type TEXT NOT NULL,
                local_session_id TEXT,
                session_type TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (adapter_type, chat_id)
            )",
            [],
        )?;

        // Remote command audit log
        conn.execute(
            "CREATE TABLE IF NOT EXISTS remote_audit_log (
                id TEXT PRIMARY KEY,
                adapter_type TEXT NOT NULL,
                chat_id INTEGER NOT NULL,
                user_id INTEGER NOT NULL,
                username TEXT,
                command_text TEXT NOT NULL,
                command_type TEXT NOT NULL,
                result_status TEXT NOT NULL,
                error_message TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        // Index for audit log queries (ordered by most recent)
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_remote_audit_created
             ON remote_audit_log(created_at DESC)",
            [],
        )?;

        // Migration: add project_path column to remote_session_mappings
        {
            let has_column =
                Self::table_has_column(&conn, "remote_session_mappings", "project_path");
            if !has_column {
                let _ = conn.execute_batch(
                    "ALTER TABLE remote_session_mappings ADD COLUMN project_path TEXT;",
                );
            }
        }

        // ====================================================================
        // A2A Remote Agents: registered remote agents for pipeline integration
        // ====================================================================
        conn.execute(
            "CREATE TABLE IF NOT EXISTS remote_agents (
                id TEXT PRIMARY KEY,
                base_url TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                capabilities TEXT NOT NULL DEFAULT '[]',
                endpoint TEXT NOT NULL,
                version TEXT NOT NULL,
                auth_required INTEGER NOT NULL DEFAULT 0,
                supported_inputs TEXT NOT NULL DEFAULT '[]',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_remote_agents_base_url
             ON remote_agents(base_url)",
            [],
        )?;

        Ok(())
    }

    /// Populate FTS5 tables from existing file_symbols and file_index data.
    ///
    /// This is a migration helper for databases that were created before FTS5
    /// was added. It reads all existing symbols and file index entries and
    /// inserts them into the FTS5 virtual tables. Safe to call multiple times
    /// (clears FTS tables before populating).
    pub fn populate_fts_from_existing(&self) -> AppResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Clear existing FTS data to avoid duplicates on re-run
        conn.execute("DELETE FROM symbol_fts", [])?;
        conn.execute("DELETE FROM filepath_fts", [])?;

        // Populate symbol_fts from file_symbols + file_index
        conn.execute_batch(
            "INSERT INTO symbol_fts (rowid, symbol_name, file_path, symbol_kind, doc_comment, signature)
             SELECT fs.id, fs.name, fi.file_path, fs.kind,
                    COALESCE(fs.doc_comment, ''), COALESCE(fs.signature, '')
             FROM file_symbols fs
             JOIN file_index fi ON fi.id = fs.file_index_id"
        )?;

        // Populate filepath_fts from file_index
        conn.execute_batch(
            "INSERT INTO filepath_fts (rowid, file_path, component, language)
             SELECT id, file_path, component, language
             FROM file_index",
        )?;

        Ok(())
    }

    /// Check whether a table has a given column (via PRAGMA table_info).
    fn table_has_column(conn: &rusqlite::Connection, table: &str, column: &str) -> bool {
        let sql = format!("PRAGMA table_info({})", table);
        if let Ok(mut stmt) = conn.prepare(&sql) {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) {
                for row in rows.flatten() {
                    if row == column {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check whether a table exists in sqlite_master.
    fn table_exists(conn: &rusqlite::Connection, table: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
            params![table],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false)
    }

    fn table_sql_contains(conn: &rusqlite::Connection, table: &str, needle: &str) -> bool {
        conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name = ?1",
            params![table],
            |row| row.get::<_, String>(0),
        )
        .map(|sql| sql.contains(needle))
        .unwrap_or(false)
    }

    fn ensure_memory_entries_v2_schema(conn: &rusqlite::Connection) -> AppResult<()> {
        let has_deleted_from_status =
            Self::table_has_column(conn, "memory_entries_v2", "deleted_from_status");
        let supports_deleted_status =
            Self::table_sql_contains(conn, "memory_entries_v2", "'deleted'");
        if has_deleted_from_status && supports_deleted_status {
            return Ok(());
        }

        conn.execute_batch(
            "DROP TRIGGER IF EXISTS trg_memory_v2_fts_insert;
             DROP TRIGGER IF EXISTS trg_memory_v2_fts_update;
             DROP TRIGGER IF EXISTS trg_memory_v2_fts_delete;
             DROP TRIGGER IF EXISTS trg_memory_v2_conflict_insert;
             DROP TRIGGER IF EXISTS trg_memory_v2_conflict_update;
             DROP INDEX IF EXISTS idx_memory_entries_v2_unique_content;
             DROP INDEX IF EXISTS idx_memory_entries_v2_scope_project_session;
             DROP INDEX IF EXISTS idx_memory_entries_v2_status_risk;
             DROP INDEX IF EXISTS idx_memory_entries_v2_category_importance;
             ALTER TABLE memory_entries_v2 RENAME TO memory_entries_v2_legacy_deleted_upgrade;
             CREATE TABLE memory_entries_v2 (
                id TEXT PRIMARY KEY,
                scope TEXT NOT NULL CHECK(scope IN ('global', 'project', 'session')),
                project_path TEXT,
                session_id TEXT,
                category TEXT NOT NULL CHECK(category IN (
                    'preference',
                    'convention',
                    'pattern',
                    'correction',
                    'fact'
                )),
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL DEFAULT '',
                keywords TEXT NOT NULL DEFAULT '[]',
                embedding BLOB,
                importance REAL NOT NULL DEFAULT 0.5,
                access_count INTEGER NOT NULL DEFAULT 0,
                source_session_id TEXT,
                source_context TEXT,
                status TEXT NOT NULL DEFAULT 'active' CHECK(status IN (
                    'active',
                    'pending_review',
                    'rejected',
                    'archived',
                    'deleted'
                )),
                deleted_from_status TEXT CHECK(deleted_from_status IN (
                    'active',
                    'rejected',
                    'archived'
                )),
                risk_tier TEXT NOT NULL DEFAULT 'high' CHECK(risk_tier IN ('low', 'medium', 'high')),
                conflict_flag INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_decay_at TEXT,
                embedding_provider TEXT NOT NULL DEFAULT 'tfidf',
                embedding_dim INTEGER NOT NULL DEFAULT 0,
                quality_score REAL NOT NULL DEFAULT 1.0
             );
             INSERT INTO memory_entries_v2 (
                id, scope, project_path, session_id, category, content, content_hash,
                keywords, embedding, importance, access_count, source_session_id, source_context,
                status, deleted_from_status, risk_tier, conflict_flag, created_at, updated_at,
                last_accessed_at, last_decay_at, embedding_provider, embedding_dim, quality_score
             )
             SELECT
                id, scope, project_path, session_id, category, content, content_hash,
                keywords, embedding, importance, access_count, source_session_id, source_context,
                status, NULL, risk_tier, conflict_flag, created_at, updated_at,
                last_accessed_at, last_decay_at, embedding_provider, embedding_dim, quality_score
             FROM memory_entries_v2_legacy_deleted_upgrade;
             DROP TABLE memory_entries_v2_legacy_deleted_upgrade;",
        )?;

        if Self::table_exists(conn, "memory_fts_v2") {
            conn.execute("DELETE FROM memory_fts_v2", [])?;
            conn.execute(
                "INSERT INTO memory_fts_v2(memory_id, content, keywords)
                 SELECT id, content, keywords FROM memory_entries_v2",
                [],
            )?;
        }

        Ok(())
    }

    fn ensure_memory_review_audit_v2_schema(conn: &rusqlite::Connection) -> AppResult<()> {
        let supports_lifecycle_audit =
            Self::table_sql_contains(conn, "memory_review_audit_v2", "'purge'")
                && Self::table_sql_contains(conn, "memory_review_audit_v2", "'delete'");
        if supports_lifecycle_audit {
            return Ok(());
        }

        conn.execute_batch(
            "DROP INDEX IF EXISTS idx_memory_review_audit_v2_created;
             DROP INDEX IF EXISTS idx_memory_review_audit_v2_memory;
             ALTER TABLE memory_review_audit_v2 RENAME TO memory_review_audit_v2_legacy_actions;
             CREATE TABLE memory_review_audit_v2 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id TEXT NOT NULL,
                decision TEXT NOT NULL CHECK(decision IN (
                    'approve',
                    'reject',
                    'archive',
                    'restore',
                    'restore_deleted',
                    'restore_active',
                    'delete',
                    'purge'
                )),
                operator TEXT NOT NULL DEFAULT 'system',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             INSERT INTO memory_review_audit_v2 (id, memory_id, decision, operator, created_at)
             SELECT id, memory_id, decision, operator, created_at
             FROM memory_review_audit_v2_legacy_actions;
             DROP TABLE memory_review_audit_v2_legacy_actions;",
        )?;

        Ok(())
    }

    /// Normalize duplicate MCP server names in-place by appending numeric suffixes.
    fn normalize_mcp_server_duplicate_names(conn: &rusqlite::Connection) -> AppResult<()> {
        let mut stmt = conn.prepare(
            "SELECT id, name FROM mcp_servers
             ORDER BY LOWER(name) ASC, created_at ASC, id ASC",
        )?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut taken: std::collections::HashSet<String> =
            rows.iter().map(|(_, name)| name.to_lowercase()).collect();
        let mut seen: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        let mut renamed = 0u32;

        for (id, name) in rows {
            let key = name.to_lowercase();
            let entry = seen.entry(key.clone()).or_insert(0);
            *entry += 1;
            if *entry == 1 {
                continue;
            }

            let mut suffix = *entry;
            let base = name.clone();
            loop {
                let candidate = format!("{} ({})", base, suffix);
                let candidate_key = candidate.to_lowercase();
                if !taken.contains(&candidate_key) {
                    conn.execute(
                        "UPDATE mcp_servers
                         SET name = ?2, updated_at = CURRENT_TIMESTAMP
                         WHERE id = ?1",
                        params![id, candidate],
                    )?;
                    taken.insert(candidate_key);
                    renamed += 1;
                    break;
                }
                suffix += 1;
            }
        }

        if renamed > 0 {
            tracing::info!(
                renamed = renamed,
                "Normalized duplicate MCP server names before applying unique index"
            );
        }
        Ok(())
    }

    /// Get a connection from the pool
    pub fn get_connection(&self) -> AppResult<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))
    }

    /// Get the connection pool
    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    /// Check if the database is healthy
    pub fn is_healthy(&self) -> bool {
        if let Ok(conn) = self.pool.get() {
            conn.query_row("SELECT 1", [], |_| Ok(())).is_ok()
        } else {
            false
        }
    }

    /// Get a setting value by key
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
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Set a setting value
    pub fn set_setting(&self, key: &str, value: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = CURRENT_TIMESTAMP",
            params![key, value],
        )?;
        Ok(())
    }

    /// Delete a setting
    pub fn delete_setting(&self, key: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    /// Get all settings whose key starts with the given prefix
    pub fn get_settings_by_prefix(&self, prefix: &str) -> AppResult<Vec<(String, String)>> {
        let conn = self.get_connection()?;
        let pattern = format!("{}%", prefix);
        let mut stmt = conn.prepare("SELECT key, value FROM settings WHERE key LIKE ?1")?;
        let rows = stmt
            .query_map(params![pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    // ========================================================================
    // Execution Operations
    // ========================================================================

    /// Insert a new execution record
    pub fn insert_execution(
        &self,
        id: &str,
        session_id: Option<&str>,
        name: &str,
        execution_mode: &str,
        project_path: &str,
        total_stories: i32,
        context_snapshot: &str,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO executions (id, session_id, name, execution_mode, status, project_path, total_stories, context_snapshot, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?7, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            params![id, session_id, name, execution_mode, project_path, total_stories, context_snapshot],
        )?;
        Ok(())
    }

    /// Update execution progress
    pub fn update_execution_progress(
        &self,
        id: &str,
        completed_stories: i32,
        current_story_id: Option<&str>,
        progress: f64,
        context_snapshot: &str,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE executions SET completed_stories = ?2, current_story_id = ?3, progress = ?4,
             context_snapshot = ?5, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            params![
                id,
                completed_stories,
                current_story_id,
                progress,
                context_snapshot
            ],
        )?;
        Ok(())
    }

    /// Update execution status
    pub fn update_execution_status(
        &self,
        id: &str,
        status: &str,
        error_message: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        let completed_at = if status == "completed" || status == "cancelled" || status == "failed" {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            None
        };
        conn.execute(
            "UPDATE executions SET status = ?2, error_message = ?3, completed_at = ?4, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            params![id, status, error_message, completed_at],
        )?;
        Ok(())
    }

    /// Get all incomplete executions (status not completed or cancelled)
    pub fn get_incomplete_executions(&self) -> AppResult<Vec<ExecutionRow>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, name, execution_mode, status, project_path,
                    total_stories, completed_stories, current_story_id, progress,
                    context_snapshot, error_message, created_at, updated_at, completed_at
             FROM executions
             WHERE status NOT IN ('completed', 'cancelled')
             ORDER BY updated_at DESC",
        )?;

        let rows = stmt
            .query_map([], |row| {
                Ok(ExecutionRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    name: row.get(2)?,
                    execution_mode: row.get(3)?,
                    status: row.get(4)?,
                    project_path: row.get(5)?,
                    total_stories: row.get(6)?,
                    completed_stories: row.get(7)?,
                    current_story_id: row.get(8)?,
                    progress: row.get(9)?,
                    context_snapshot: row.get(10)?,
                    error_message: row.get(11)?,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                    completed_at: row.get(14)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Get a single execution by ID
    pub fn get_execution(&self, id: &str) -> AppResult<Option<ExecutionRow>> {
        let conn = self.get_connection()?;
        let result = conn.query_row(
            "SELECT id, session_id, name, execution_mode, status, project_path,
                    total_stories, completed_stories, current_story_id, progress,
                    context_snapshot, error_message, created_at, updated_at, completed_at
             FROM executions WHERE id = ?1",
            params![id],
            |row| {
                Ok(ExecutionRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    name: row.get(2)?,
                    execution_mode: row.get(3)?,
                    status: row.get(4)?,
                    project_path: row.get(5)?,
                    total_stories: row.get(6)?,
                    completed_stories: row.get(7)?,
                    current_story_id: row.get(8)?,
                    progress: row.get(9)?,
                    context_snapshot: row.get(10)?,
                    error_message: row.get(11)?,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                    completed_at: row.get(14)?,
                })
            },
        );

        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Delete an execution record
    pub fn delete_execution(&self, id: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM executions WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Get incomplete checkpoint chains for a session
    pub fn get_checkpoints_for_session(&self, session_id: &str) -> AppResult<Vec<CheckpointRow>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, name, description, snapshot, created_at
             FROM checkpoints
             WHERE session_id = ?1
             ORDER BY created_at DESC",
        )?;

        let rows = stmt
            .query_map(params![session_id], |row| {
                Ok(CheckpointRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    snapshot: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    // ========================================================================
    // MCP Server Operations
    // ========================================================================

    /// Insert a new MCP server
    pub fn insert_mcp_server(&self, server: &crate::models::McpServer) -> AppResult<()> {
        let conn = self.get_connection()?;

        let args_json = serde_json::to_string(&server.args).unwrap_or_default();
        let env_json = serde_json::to_string(&server.env).unwrap_or_default();
        let headers_json = serde_json::to_string(&server.headers).unwrap_or_default();
        let server_type = match server.server_type {
            crate::models::McpServerType::Stdio => "stdio",
            crate::models::McpServerType::StreamHttp => "stream_http",
        };

        conn.execute(
            "INSERT INTO mcp_servers (
                id, name, server_type, command, args, env, url, headers,
                has_env_secret, has_headers_secret, enabled, auto_connect,
                status, last_error, last_connected_at, retry_count, last_checked,
                managed_install, catalog_item_id, trust_level,
                created_at, updated_at
             )
             VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17,
                ?18, ?19, ?20,
                CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
             )",
            params![
                server.id,
                server.name,
                server_type,
                server.command,
                args_json,
                env_json,
                server.url,
                headers_json,
                server.has_env_secret as i32,
                server.has_headers_secret as i32,
                server.enabled as i32,
                server.auto_connect as i32,
                match &server.status {
                    crate::models::McpServerStatus::Connected => "connected".to_string(),
                    crate::models::McpServerStatus::Disconnected => "disconnected".to_string(),
                    crate::models::McpServerStatus::Error(msg) => format!("error:{}", msg),
                    crate::models::McpServerStatus::Unknown => "unknown".to_string(),
                },
                server.last_error,
                server.last_connected_at,
                server.retry_count as i64,
                server.last_checked,
                server.managed_install as i32,
                server.catalog_item_id,
                server.trust_level.as_ref().map(|level| match level {
                    crate::models::McpCatalogTrustLevel::Official => "official".to_string(),
                    crate::models::McpCatalogTrustLevel::Verified => "verified".to_string(),
                    crate::models::McpCatalogTrustLevel::Community => "community".to_string(),
                }),
            ],
        )?;

        Ok(())
    }

    /// Get an MCP server by ID
    pub fn get_mcp_server(&self, id: &str) -> AppResult<Option<crate::models::McpServer>> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT id, name, server_type, command, args, env, url, headers,
                    has_env_secret, has_headers_secret, enabled, auto_connect, status,
                    last_error, last_connected_at, retry_count,
                    last_checked, managed_install, catalog_item_id, trust_level,
                    created_at, updated_at
             FROM mcp_servers WHERE id = ?1",
            params![id],
            |row| Self::row_to_mcp_server(row),
        );

        match result {
            Ok(server) => Ok(Some(server)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// List all MCP servers
    pub fn list_mcp_servers(&self) -> AppResult<Vec<crate::models::McpServer>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT id, name, server_type, command, args, env, url, headers,
                    has_env_secret, has_headers_secret, enabled, auto_connect, status,
                    last_error, last_connected_at, retry_count,
                    last_checked, managed_install, catalog_item_id, trust_level,
                    created_at, updated_at
             FROM mcp_servers ORDER BY name ASC",
        )?;

        let servers = stmt
            .query_map([], |row| Self::row_to_mcp_server(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(servers)
    }

    /// Update an MCP server
    pub fn update_mcp_server(&self, server: &crate::models::McpServer) -> AppResult<()> {
        let conn = self.get_connection()?;

        let args_json = serde_json::to_string(&server.args).unwrap_or_default();
        let env_json = serde_json::to_string(&server.env).unwrap_or_default();
        let headers_json = serde_json::to_string(&server.headers).unwrap_or_default();
        let server_type = match server.server_type {
            crate::models::McpServerType::Stdio => "stdio",
            crate::models::McpServerType::StreamHttp => "stream_http",
        };
        let status = match &server.status {
            crate::models::McpServerStatus::Connected => "connected".to_string(),
            crate::models::McpServerStatus::Disconnected => "disconnected".to_string(),
            crate::models::McpServerStatus::Error(msg) => format!("error:{}", msg),
            crate::models::McpServerStatus::Unknown => "unknown".to_string(),
        };

        conn.execute(
            "UPDATE mcp_servers SET name = ?2, server_type = ?3, command = ?4, args = ?5, env = ?6,
             url = ?7, headers = ?8, has_env_secret = ?9, has_headers_secret = ?10, enabled = ?11,
             auto_connect = ?12, status = ?13, last_error = ?14, last_connected_at = ?15,
             retry_count = ?16, last_checked = ?17,
             managed_install = ?18, catalog_item_id = ?19, trust_level = ?20,
             updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![
                server.id,
                server.name,
                server_type,
                server.command,
                args_json,
                env_json,
                server.url,
                headers_json,
                server.has_env_secret as i32,
                server.has_headers_secret as i32,
                server.enabled as i32,
                server.auto_connect as i32,
                status,
                server.last_error,
                server.last_connected_at,
                server.retry_count as i64,
                server.last_checked,
                server.managed_install as i32,
                server.catalog_item_id,
                server.trust_level.as_ref().map(|level| match level {
                    crate::models::McpCatalogTrustLevel::Official => "official".to_string(),
                    crate::models::McpCatalogTrustLevel::Verified => "verified".to_string(),
                    crate::models::McpCatalogTrustLevel::Community => "community".to_string(),
                }),
            ],
        )?;

        Ok(())
    }

    /// Delete an MCP server
    pub fn delete_mcp_server(&self, id: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM mcp_servers WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Toggle MCP server enabled status
    pub fn toggle_mcp_server_enabled(&self, id: &str, enabled: bool) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE mcp_servers SET enabled = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            params![id, enabled as i32],
        )?;
        Ok(())
    }

    /// Update MCP server status after health check
    pub fn update_mcp_server_status(
        &self,
        id: &str,
        status: &crate::models::McpServerStatus,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        let status_str = match status {
            crate::models::McpServerStatus::Connected => "connected".to_string(),
            crate::models::McpServerStatus::Disconnected => "disconnected".to_string(),
            crate::models::McpServerStatus::Error(msg) => format!("error:{}", msg),
            crate::models::McpServerStatus::Unknown => "unknown".to_string(),
        };

        conn.execute(
            "UPDATE mcp_servers SET status = ?2, last_checked = CURRENT_TIMESTAMP WHERE id = ?1",
            params![id, status_str],
        )?;

        Ok(())
    }

    /// Mark an MCP server as connected and reset transient error counters.
    pub fn mark_mcp_server_connected(&self, id: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE mcp_servers
             SET status = 'connected',
                 last_error = NULL,
                 last_connected_at = CURRENT_TIMESTAMP,
                 retry_count = 0,
                 last_checked = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// Mark an MCP server as disconnected.
    pub fn mark_mcp_server_disconnected(&self, id: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE mcp_servers
             SET status = 'disconnected',
                 last_error = NULL,
                 last_checked = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// Mark an MCP server connection failure and increment retry counter.
    pub fn mark_mcp_server_connection_error(&self, id: &str, error: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE mcp_servers
             SET status = ?2,
                 last_error = ?3,
                 retry_count = retry_count + 1,
                 last_checked = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![id, format!("error:{}", error), error],
        )?;
        Ok(())
    }

    /// Get MCP server by name (for duplicate detection)
    pub fn get_mcp_server_by_name(
        &self,
        name: &str,
    ) -> AppResult<Option<crate::models::McpServer>> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT id, name, server_type, command, args, env, url, headers,
                    has_env_secret, has_headers_secret, enabled, auto_connect, status,
                    last_error, last_connected_at, retry_count,
                    last_checked, managed_install, catalog_item_id, trust_level,
                    created_at, updated_at
             FROM mcp_servers WHERE name = ?1",
            params![name],
            |row| Self::row_to_mcp_server(row),
        );

        match result {
            Ok(server) => Ok(Some(server)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Get MCP server by name (case-insensitive).
    pub fn get_mcp_server_by_name_case_insensitive(
        &self,
        name: &str,
    ) -> AppResult<Option<crate::models::McpServer>> {
        let conn = self.get_connection()?;
        let result = conn.query_row(
            "SELECT id, name, server_type, command, args, env, url, headers,
                    has_env_secret, has_headers_secret, enabled, auto_connect, status,
                    last_error, last_connected_at, retry_count,
                    last_checked, managed_install, catalog_item_id, trust_level,
                    created_at, updated_at
             FROM mcp_servers
             WHERE LOWER(name) = LOWER(?1)
             LIMIT 1",
            params![name],
            |row| Self::row_to_mcp_server(row),
        );
        match result {
            Ok(server) => Ok(Some(server)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Check whether an MCP server name conflicts (case-insensitive), excluding an optional id.
    pub fn find_mcp_server_name_conflict_case_insensitive(
        &self,
        name: &str,
        ignore_id: Option<&str>,
    ) -> AppResult<bool> {
        let conn = self.get_connection()?;
        let count: i64 = match ignore_id {
            Some(id) => conn.query_row(
                "SELECT COUNT(*) FROM mcp_servers
                 WHERE LOWER(name) = LOWER(?1) AND id <> ?2",
                params![name, id],
                |row| row.get(0),
            )?,
            None => conn.query_row(
                "SELECT COUNT(*) FROM mcp_servers
                 WHERE LOWER(name) = LOWER(?1)",
                params![name],
                |row| row.get(0),
            )?,
        };
        Ok(count > 0)
    }

    /// Helper function to convert a database row to McpServer
    fn row_to_mcp_server(row: &rusqlite::Row) -> rusqlite::Result<crate::models::McpServer> {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let server_type_str: String = row.get(2)?;
        let command: Option<String> = row.get(3)?;
        let args_json: String = row.get::<_, String>(4).unwrap_or_default();
        let env_json: String = row.get::<_, String>(5).unwrap_or_default();
        let url: Option<String> = row.get(6)?;
        let headers_json: String = row.get::<_, String>(7).unwrap_or_default();
        let has_env_secret: i32 = row.get::<_, i32>(8).unwrap_or(0);
        let has_headers_secret: i32 = row.get::<_, i32>(9).unwrap_or(0);
        let enabled: i32 = row.get(10)?;
        let auto_connect: i32 = row.get::<_, i32>(11).unwrap_or(1);
        let status_str: String = row
            .get::<_, String>(12)
            .unwrap_or_else(|_| "unknown".to_string());
        let last_error: Option<String> = row.get::<_, Option<String>>(13).unwrap_or(None);
        let last_connected_at: Option<String> = row.get::<_, Option<String>>(14).unwrap_or(None);
        let retry_count: i64 = row.get::<_, i64>(15).unwrap_or(0);
        let last_checked: Option<String> = row.get(16)?;
        let managed_install: i32 = row.get::<_, i32>(17).unwrap_or(0);
        let catalog_item_id: Option<String> = row.get::<_, Option<String>>(18).unwrap_or(None);
        let trust_level_str: Option<String> = row.get::<_, Option<String>>(19).unwrap_or(None);
        let created_at: Option<String> = row.get(20)?;
        let updated_at: Option<String> = row.get(21)?;

        let server_type = match server_type_str.as_str() {
            "stream_http" | "sse" => crate::models::McpServerType::StreamHttp,
            _ => crate::models::McpServerType::Stdio,
        };

        let args: Vec<String> = serde_json::from_str(&args_json).unwrap_or_default();
        let env: std::collections::HashMap<String, String> =
            serde_json::from_str(&env_json).unwrap_or_default();
        let headers: std::collections::HashMap<String, String> =
            serde_json::from_str(&headers_json).unwrap_or_default();

        let status = if status_str.starts_with("error:") {
            crate::models::McpServerStatus::Error(status_str[6..].to_string())
        } else {
            match status_str.as_str() {
                "connected" => crate::models::McpServerStatus::Connected,
                "disconnected" => crate::models::McpServerStatus::Disconnected,
                _ => crate::models::McpServerStatus::Unknown,
            }
        };

        let trust_level = trust_level_str.as_deref().and_then(|value| match value {
            "official" => Some(crate::models::McpCatalogTrustLevel::Official),
            "verified" => Some(crate::models::McpCatalogTrustLevel::Verified),
            "community" => Some(crate::models::McpCatalogTrustLevel::Community),
            _ => None,
        });

        Ok(crate::models::McpServer {
            id,
            name,
            server_type,
            command,
            args,
            env,
            url,
            headers,
            has_env_secret: has_env_secret != 0,
            has_headers_secret: has_headers_secret != 0,
            enabled: enabled != 0,
            auto_connect: auto_connect != 0,
            status,
            last_error,
            last_connected_at,
            retry_count: retry_count.max(0) as u32,
            last_checked,
            managed_install: managed_install != 0,
            catalog_item_id,
            trust_level,
            created_at,
            updated_at,
        })
    }

    /// Upsert cached MCP catalog payload.
    pub fn upsert_mcp_catalog_cache(
        &self,
        source_id: &str,
        payload_json: &str,
        signature: Option<&str>,
        etag: Option<&str>,
        expires_at: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO mcp_catalog_cache (source_id, payload_json, signature, etag, fetched_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP, ?5)
             ON CONFLICT(source_id) DO UPDATE SET
               payload_json = excluded.payload_json,
               signature = excluded.signature,
               etag = excluded.etag,
               fetched_at = CURRENT_TIMESTAMP,
               expires_at = excluded.expires_at",
            params![source_id, payload_json, signature, etag, expires_at],
        )?;
        Ok(())
    }

    /// Read cached MCP catalog payload by source id.
    pub fn get_mcp_catalog_cache(&self, source_id: &str) -> AppResult<Option<McpCatalogCacheRow>> {
        let conn = self.get_connection()?;
        let result = conn.query_row(
            "SELECT source_id, payload_json, signature, etag, fetched_at, expires_at
             FROM mcp_catalog_cache
             WHERE source_id = ?1",
            params![source_id],
            |row| {
                Ok(McpCatalogCacheRow {
                    source_id: row.get(0)?,
                    payload_json: row.get(1)?,
                    signature: row.get(2)?,
                    etag: row.get(3)?,
                    fetched_at: row.get(4)?,
                    expires_at: row.get(5)?,
                })
            },
        );

        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Upsert runtime inventory row.
    pub fn upsert_mcp_runtime_inventory(
        &self,
        runtime_key: &str,
        runtime: &crate::models::McpRuntimeInfo,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        let runtime_kind = match runtime.runtime {
            crate::models::McpRuntimeKind::Node => "node",
            crate::models::McpRuntimeKind::Uv => "uv",
            crate::models::McpRuntimeKind::Python => "python",
            crate::models::McpRuntimeKind::Docker => "docker",
        };
        let health_status = if runtime.healthy { "healthy" } else { "error" };
        conn.execute(
            "INSERT INTO mcp_runtime_inventory (
                runtime_key, runtime_kind, version, executable_path, source, managed,
                health_status, last_error, last_checked_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, CURRENT_TIMESTAMP)
             ON CONFLICT(runtime_key) DO UPDATE SET
                runtime_kind = excluded.runtime_kind,
                version = excluded.version,
                executable_path = excluded.executable_path,
                source = excluded.source,
                managed = excluded.managed,
                health_status = excluded.health_status,
                last_error = excluded.last_error,
                last_checked_at = excluded.last_checked_at,
                updated_at = CURRENT_TIMESTAMP",
            params![
                runtime_key,
                runtime_kind,
                runtime.version,
                runtime.path,
                runtime.source,
                runtime.managed as i32,
                health_status,
                runtime.last_error,
                runtime.last_checked,
            ],
        )?;
        Ok(())
    }

    /// List runtime inventory.
    pub fn list_mcp_runtime_inventory(&self) -> AppResult<Vec<crate::models::McpRuntimeInfo>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT runtime_kind, version, executable_path, source, managed, health_status, last_error, last_checked_at
             FROM mcp_runtime_inventory
             ORDER BY runtime_kind ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let runtime_kind: String = row.get(0)?;
                let runtime = match runtime_kind.as_str() {
                    "node" => crate::models::McpRuntimeKind::Node,
                    "uv" => crate::models::McpRuntimeKind::Uv,
                    "python" => crate::models::McpRuntimeKind::Python,
                    "docker" => crate::models::McpRuntimeKind::Docker,
                    _ => crate::models::McpRuntimeKind::Node,
                };
                let health_status: String = row.get::<_, String>(5).unwrap_or_default();
                Ok(crate::models::McpRuntimeInfo {
                    runtime,
                    version: row.get(1)?,
                    path: row.get(2)?,
                    source: row.get(3)?,
                    managed: row.get::<_, i32>(4).unwrap_or(0) != 0,
                    healthy: health_status == "healthy",
                    last_error: row.get(6)?,
                    last_checked: row.get(7)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Upsert install job state.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_mcp_install_job(
        &self,
        job_id: &str,
        item_id: &str,
        server_id: Option<&str>,
        phase: &str,
        progress: f64,
        status: &str,
        error_class: Option<&str>,
        error_message: Option<&str>,
        logs_json: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO mcp_install_jobs (
                job_id, item_id, server_id, phase, progress, status, error_class, error_message, logs_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
             ON CONFLICT(job_id) DO UPDATE SET
                item_id = excluded.item_id,
                server_id = excluded.server_id,
                phase = excluded.phase,
                progress = excluded.progress,
                status = excluded.status,
                error_class = excluded.error_class,
                error_message = excluded.error_message,
                logs_json = excluded.logs_json,
                updated_at = CURRENT_TIMESTAMP",
            params![
                job_id,
                item_id,
                server_id,
                phase,
                progress,
                status,
                error_class,
                error_message,
                logs_json,
            ],
        )?;
        Ok(())
    }

    /// Fetch install job row.
    pub fn get_mcp_install_job(&self, job_id: &str) -> AppResult<Option<McpInstallJobRow>> {
        let conn = self.get_connection()?;
        let result = conn.query_row(
            "SELECT job_id, item_id, server_id, phase, progress, status, error_class, error_message, logs_json, created_at, updated_at
             FROM mcp_install_jobs
             WHERE job_id = ?1",
            params![job_id],
            |row| {
                Ok(McpInstallJobRow {
                    job_id: row.get(0)?,
                    item_id: row.get(1)?,
                    server_id: row.get(2)?,
                    phase: row.get(3)?,
                    progress: row.get::<_, f64>(4).unwrap_or(0.0),
                    status: row.get(5)?,
                    error_class: row.get(6)?,
                    error_message: row.get(7)?,
                    logs_json: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        );

        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Upsert managed install metadata.
    pub fn upsert_mcp_install_record(
        &self,
        record: &crate::models::McpInstallRecord,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;
        let trust_level = match record.trust_level {
            crate::models::McpCatalogTrustLevel::Official => "official",
            crate::models::McpCatalogTrustLevel::Verified => "verified",
            crate::models::McpCatalogTrustLevel::Community => "community",
        };
        let lock_json = match &record.package_lock_json {
            Some(v) => Some(serde_json::to_string(v).unwrap_or_default()),
            None => None,
        };
        let runtime_snapshot_json = match &record.runtime_snapshot_json {
            Some(v) => Some(serde_json::to_string(v).unwrap_or_default()),
            None => None,
        };
        conn.execute(
            "INSERT INTO mcp_install_records (
                server_id, catalog_item_id, catalog_version, strategy_id, trust_level,
                package_lock_json, runtime_snapshot_json, installed_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
             ON CONFLICT(server_id) DO UPDATE SET
                catalog_item_id = excluded.catalog_item_id,
                catalog_version = excluded.catalog_version,
                strategy_id = excluded.strategy_id,
                trust_level = excluded.trust_level,
                package_lock_json = excluded.package_lock_json,
                runtime_snapshot_json = excluded.runtime_snapshot_json,
                updated_at = CURRENT_TIMESTAMP",
            params![
                record.server_id,
                record.catalog_item_id,
                record.catalog_version,
                record.strategy_id,
                trust_level,
                lock_json,
                runtime_snapshot_json,
            ],
        )?;
        Ok(())
    }

    /// Get managed install record by server id.
    pub fn get_mcp_install_record(
        &self,
        server_id: &str,
    ) -> AppResult<Option<crate::models::McpInstallRecord>> {
        let conn = self.get_connection()?;
        let result = conn.query_row(
            "SELECT server_id, catalog_item_id, catalog_version, strategy_id, trust_level, package_lock_json, runtime_snapshot_json, installed_at, updated_at
             FROM mcp_install_records WHERE server_id = ?1",
            params![server_id],
            |row| {
                let trust_level_str: String = row.get(4)?;
                let trust_level = match trust_level_str.as_str() {
                    "official" => crate::models::McpCatalogTrustLevel::Official,
                    "verified" => crate::models::McpCatalogTrustLevel::Verified,
                    _ => crate::models::McpCatalogTrustLevel::Community,
                };
                let package_lock_json: Option<String> = row.get(5)?;
                let runtime_snapshot_json: Option<String> = row.get(6)?;
                Ok(crate::models::McpInstallRecord {
                    server_id: row.get(0)?,
                    catalog_item_id: row.get(1)?,
                    catalog_version: row.get(2)?,
                    strategy_id: row.get(3)?,
                    trust_level,
                    package_lock_json: package_lock_json
                        .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok()),
                    runtime_snapshot_json: runtime_snapshot_json
                        .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok()),
                    installed_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        );

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Remove managed install record.
    pub fn delete_mcp_install_record(&self, server_id: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "DELETE FROM mcp_install_records WHERE server_id = ?1",
            params![server_id],
        )?;
        Ok(())
    }

    // ========================================================================
    // Webhook Operations
    // ========================================================================

    /// Insert a new webhook channel configuration.
    pub fn insert_webhook_channel(
        &self,
        config: &crate::services::webhook::types::WebhookChannelConfig,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;

        let scope_type = match &config.scope {
            crate::services::webhook::types::WebhookScope::Global => "global",
            crate::services::webhook::types::WebhookScope::Sessions(_) => "sessions",
        };
        let scope_sessions = match &config.scope {
            crate::services::webhook::types::WebhookScope::Global => None,
            crate::services::webhook::types::WebhookScope::Sessions(ids) => {
                Some(serde_json::to_string(ids).unwrap_or_default())
            }
        };
        let events_json = serde_json::to_string(&config.events).unwrap_or_default();

        conn.execute(
            "INSERT INTO webhook_channels (id, name, channel_type, enabled, url, scope_type, scope_sessions, events, template, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                config.id,
                config.name,
                config.channel_type.to_string(),
                config.enabled as i32,
                config.url,
                scope_type,
                scope_sessions,
                events_json,
                config.template,
                config.created_at,
                config.updated_at,
            ],
        )?;

        Ok(())
    }

    /// Get a webhook channel by ID.
    pub fn get_webhook_channel(
        &self,
        id: &str,
    ) -> AppResult<Option<crate::services::webhook::types::WebhookChannelConfig>> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT id, name, channel_type, enabled, url, scope_type, scope_sessions, events, template, created_at, updated_at
             FROM webhook_channels WHERE id = ?1",
            params![id],
            |row| Self::row_to_webhook_channel(row),
        );

        match result {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// List all webhook channels.
    pub fn list_webhook_channels(
        &self,
    ) -> AppResult<Vec<crate::services::webhook::types::WebhookChannelConfig>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT id, name, channel_type, enabled, url, scope_type, scope_sessions, events, template, created_at, updated_at
             FROM webhook_channels ORDER BY created_at DESC",
        )?;

        let channels = stmt
            .query_map([], |row| Self::row_to_webhook_channel(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(channels)
    }

    /// Update a webhook channel configuration.
    pub fn update_webhook_channel(
        &self,
        config: &crate::services::webhook::types::WebhookChannelConfig,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;

        let scope_type = match &config.scope {
            crate::services::webhook::types::WebhookScope::Global => "global",
            crate::services::webhook::types::WebhookScope::Sessions(_) => "sessions",
        };
        let scope_sessions = match &config.scope {
            crate::services::webhook::types::WebhookScope::Global => None,
            crate::services::webhook::types::WebhookScope::Sessions(ids) => {
                Some(serde_json::to_string(ids).unwrap_or_default())
            }
        };
        let events_json = serde_json::to_string(&config.events).unwrap_or_default();

        conn.execute(
            "UPDATE webhook_channels SET name = ?2, channel_type = ?3, enabled = ?4, url = ?5,
             scope_type = ?6, scope_sessions = ?7, events = ?8, template = ?9, updated_at = ?10
             WHERE id = ?1",
            params![
                config.id,
                config.name,
                config.channel_type.to_string(),
                config.enabled as i32,
                config.url,
                scope_type,
                scope_sessions,
                events_json,
                config.template,
                config.updated_at,
            ],
        )?;

        Ok(())
    }

    /// Delete a webhook channel by ID. Deliveries are cascade-deleted.
    pub fn delete_webhook_channel(&self, id: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        // Enable foreign keys for cascade delete
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        conn.execute("DELETE FROM webhook_channels WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Insert a webhook delivery record.
    pub fn insert_webhook_delivery(
        &self,
        delivery: &crate::services::webhook::types::WebhookDelivery,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;

        let payload_json = serde_json::to_string(&delivery.payload).unwrap_or_default();

        conn.execute(
            "INSERT INTO webhook_deliveries (id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, next_retry_at, last_error, retryable, error_class, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                delivery.id,
                delivery.channel_id,
                delivery.event_type.to_string(),
                payload_json,
                delivery.status.to_string(),
                delivery.status_code,
                delivery.response_body,
                delivery.attempts,
                delivery.last_attempt_at,
                delivery.next_retry_at,
                delivery.last_error,
                delivery.retryable.map(i32::from),
                delivery.error_class,
                delivery.created_at,
            ],
        )?;

        Ok(())
    }

    /// List webhook deliveries with optional channel_id filter and pagination.
    pub fn list_webhook_deliveries(
        &self,
        channel_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> AppResult<Vec<crate::services::webhook::types::WebhookDelivery>> {
        let conn = self.get_connection()?;

        match channel_id {
            Some(cid) => {
                let mut stmt = conn.prepare(
                    "SELECT id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, next_retry_at, last_error, retryable, error_class, created_at
                     FROM webhook_deliveries WHERE channel_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
                )?;
                let deliveries = stmt
                    .query_map(params![cid, limit, offset], |row| {
                        Self::row_to_webhook_delivery(row)
                    })?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(deliveries)
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, next_retry_at, last_error, retryable, error_class, created_at
                     FROM webhook_deliveries ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
                )?;
                let deliveries = stmt
                    .query_map(params![limit, offset], |row| {
                        Self::row_to_webhook_delivery(row)
                    })?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(deliveries)
            }
        }
    }

    /// Get failed deliveries eligible for retry (attempts < max_attempts).
    pub fn get_failed_deliveries(
        &self,
        max_attempts: u32,
    ) -> AppResult<Vec<crate::services::webhook::types::WebhookDelivery>> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, next_retry_at, last_error, retryable, error_class, created_at
             FROM webhook_deliveries
             WHERE status IN ('failed', 'retrying') AND attempts < ?1
             ORDER BY last_attempt_at ASC",
        )?;

        let deliveries = stmt
            .query_map(params![max_attempts], |row| {
                Self::row_to_webhook_delivery(row)
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(deliveries)
    }

    /// Get failed/retrying deliveries that are due for retry now.
    pub fn get_deliveries_due_for_retry(
        &self,
        max_attempts: u32,
        now_rfc3339: &str,
    ) -> AppResult<Vec<crate::services::webhook::types::WebhookDelivery>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, next_retry_at, last_error, retryable, error_class, created_at
             FROM webhook_deliveries
             WHERE status IN ('failed', 'retrying')
               AND attempts < ?1
               AND (next_retry_at IS NULL OR next_retry_at <= ?2)
             ORDER BY COALESCE(next_retry_at, last_attempt_at) ASC",
        )?;

        let deliveries = stmt
            .query_map(params![max_attempts, now_rfc3339], |row| {
                Self::row_to_webhook_delivery(row)
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(deliveries)
    }

    /// Count failed/retrying deliveries currently in queue.
    pub fn count_failed_webhook_deliveries(&self) -> AppResult<u32> {
        let conn = self.get_connection()?;
        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM webhook_deliveries WHERE status IN ('failed', 'retrying')",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Delete historical delivery records older than the provided timestamp.
    pub fn delete_webhook_deliveries_before(&self, cutoff_rfc3339: &str) -> AppResult<u32> {
        let conn = self.get_connection()?;
        let deleted = conn.execute(
            "DELETE FROM webhook_deliveries WHERE created_at < ?1",
            params![cutoff_rfc3339],
        )?;
        Ok(deleted as u32)
    }

    /// Update a webhook delivery status.
    pub fn update_webhook_delivery_status(
        &self,
        delivery: &crate::services::webhook::types::WebhookDelivery,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;

        conn.execute(
            "UPDATE webhook_deliveries
             SET status = ?2, status_code = ?3, response_body = ?4, attempts = ?5, last_attempt_at = ?6, next_retry_at = ?7, last_error = ?8, retryable = ?9, error_class = ?10
             WHERE id = ?1",
            params![
                delivery.id,
                delivery.status.to_string(),
                delivery.status_code,
                delivery.response_body,
                delivery.attempts,
                delivery.last_attempt_at,
                delivery.next_retry_at,
                delivery.last_error,
                delivery.retryable.map(i32::from),
                delivery.error_class,
            ],
        )?;

        Ok(())
    }

    /// Get a single delivery by ID.
    pub fn get_webhook_delivery(
        &self,
        id: &str,
    ) -> AppResult<Option<crate::services::webhook::types::WebhookDelivery>> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, next_retry_at, last_error, retryable, error_class, created_at
             FROM webhook_deliveries WHERE id = ?1",
            params![id],
            |row| Self::row_to_webhook_delivery(row),
        );

        match result {
            Ok(delivery) => Ok(Some(delivery)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Helper to convert a database row to WebhookChannelConfig.
    fn row_to_webhook_channel(
        row: &rusqlite::Row,
    ) -> rusqlite::Result<crate::services::webhook::types::WebhookChannelConfig> {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let channel_type_str: String = row.get(2)?;
        let enabled: i32 = row.get(3)?;
        let url: String = row.get(4)?;
        let scope_type: String = row.get(5)?;
        let scope_sessions_json: Option<String> = row.get(6)?;
        let events_json: String = row.get(7)?;
        let template: Option<String> = row.get(8)?;
        let created_at: String = row.get(9)?;
        let updated_at: String = row.get(10)?;

        let channel_type =
            crate::services::webhook::types::WebhookChannelType::from_str_value(&channel_type_str)
                .unwrap_or(crate::services::webhook::types::WebhookChannelType::Custom);

        let scope = match scope_type.as_str() {
            "sessions" => {
                let ids: Vec<String> = scope_sessions_json
                    .and_then(|json| serde_json::from_str(&json).ok())
                    .unwrap_or_default();
                crate::services::webhook::types::WebhookScope::Sessions(ids)
            }
            _ => crate::services::webhook::types::WebhookScope::Global,
        };

        let events: Vec<crate::services::webhook::types::WebhookEventType> =
            serde_json::from_str(&events_json).unwrap_or_default();

        Ok(crate::services::webhook::types::WebhookChannelConfig {
            id,
            name,
            channel_type,
            enabled: enabled != 0,
            url,
            secret: None, // Never loaded from DB, only from Keyring
            scope,
            events,
            template,
            created_at,
            updated_at,
        })
    }

    /// Helper to convert a database row to WebhookDelivery.
    fn row_to_webhook_delivery(
        row: &rusqlite::Row,
    ) -> rusqlite::Result<crate::services::webhook::types::WebhookDelivery> {
        let id: String = row.get(0)?;
        let channel_id: String = row.get(1)?;
        let event_type_str: String = row.get(2)?;
        let payload_json: String = row.get(3)?;
        let status_str: String = row.get(4)?;
        let status_code: Option<u16> = row.get(5)?;
        let response_body: Option<String> = row.get(6)?;
        let attempts: u32 = row.get(7)?;
        let last_attempt_at: String = row.get::<_, String>(8).unwrap_or_default();
        let next_retry_at: Option<String> = row.get(9)?;
        let last_error: Option<String> = row.get(10)?;
        let retryable_raw: Option<i32> = row.get(11)?;
        let error_class: Option<String> = row.get(12)?;
        let created_at: String = row.get(13)?;

        let event_type: crate::services::webhook::types::WebhookEventType =
            serde_json::from_str(&format!("\"{}\"", event_type_str))
                .unwrap_or(crate::services::webhook::types::WebhookEventType::TaskComplete);

        let payload: crate::services::webhook::types::WebhookPayload =
            serde_json::from_str(&payload_json).unwrap_or_default();

        let status = crate::services::webhook::types::DeliveryStatus::from_str_value(&status_str);

        Ok(crate::services::webhook::types::WebhookDelivery {
            id,
            channel_id,
            event_type,
            payload,
            status,
            status_code,
            response_body,
            attempts,
            last_attempt_at,
            next_retry_at,
            last_error,
            retryable: retryable_raw.map(|value| value != 0),
            error_class,
            created_at,
        })
    }
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("pool_size", &self.pool.state().connections)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    // Tests will use an in-memory database
    use super::*;

    fn create_test_db() -> AppResult<Database> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder()
            .max_size(1)
            .build(manager)
            .map_err(|e| AppError::database(e.to_string()))?;

        let db = Database { pool };
        db.init_schema()?;
        Ok(db)
    }

    #[test]
    fn test_database_health() {
        let db = create_test_db().unwrap();
        assert!(db.is_healthy());
    }

    #[test]
    fn test_settings_crud() {
        let db = create_test_db().unwrap();

        // Set a setting
        db.set_setting("test_key", "test_value").unwrap();

        // Get the setting
        let value = db.get_setting("test_key").unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        // Update the setting
        db.set_setting("test_key", "new_value").unwrap();
        let value = db.get_setting("test_key").unwrap();
        assert_eq!(value, Some("new_value".to_string()));

        // Delete the setting
        db.delete_setting("test_key").unwrap();
        let value = db.get_setting("test_key").unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn test_mcp_duplicate_names_are_normalized_before_unique_index() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        conn.execute("DROP INDEX IF EXISTS idx_mcp_servers_name_ci_unique", [])
            .unwrap();
        conn.execute(
            "INSERT INTO mcp_servers (id, name, server_type) VALUES (?1, ?2, ?3)",
            params!["mcp-1", "Demo", "stdio"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mcp_servers (id, name, server_type) VALUES (?1, ?2, ?3)",
            params!["mcp-2", "demo", "stdio"],
        )
        .unwrap();

        Database::normalize_mcp_server_duplicate_names(&conn).unwrap();
        conn.execute(
            "CREATE UNIQUE INDEX idx_mcp_servers_name_ci_unique ON mcp_servers(LOWER(name))",
            [],
        )
        .unwrap();

        let mut stmt = conn
            .prepare("SELECT name FROM mcp_servers ORDER BY id ASC")
            .unwrap();
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0], "Demo");
        assert_eq!(names[1], "demo (2)");
        let lowered: std::collections::HashSet<String> =
            names.into_iter().map(|name| name.to_lowercase()).collect();
        assert_eq!(lowered.len(), 2);
    }

    // =========================================================================
    // Feature-001: Project Memory System schema tests
    // =========================================================================

    #[test]
    fn test_memory_entries_v2_table_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        conn.execute(
            "INSERT INTO memory_entries_v2 (id, scope, project_path, category, content, content_hash, keywords, importance)
             VALUES ('mem-001', 'project', '/test/project', 'preference', 'Use pnpm not npm', lower(trim('Use pnpm not npm')), '[\"pnpm\",\"npm\"]', 0.9)",
            [],
        )
        .unwrap();

        let content: String = conn
            .query_row(
                "SELECT content FROM memory_entries_v2 WHERE id = 'mem-001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(content, "Use pnpm not npm");

        let result = conn.execute(
            "INSERT INTO memory_entries_v2 (id, scope, project_path, category, content, content_hash)
             VALUES ('mem-002', 'project', '/test/project', 'invalid_category', 'test', lower(trim('test')))",
            [],
        );
        assert!(result.is_err(), "Invalid category should be rejected");
    }

    #[test]
    fn test_memory_entries_v2_unique_constraint() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        conn.execute(
            "INSERT INTO memory_entries_v2 (id, scope, project_path, category, content, content_hash)
             VALUES ('mem-001', 'project', '/test/project', 'fact', 'This is a Tauri app', lower(trim('This is a Tauri app')))",
            [],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO memory_entries_v2 (id, scope, project_path, category, content, content_hash)
             VALUES ('mem-002', 'project', '/test/project', 'fact', 'This is a Tauri app', lower(trim('This is a Tauri app')))",
            [],
        );
        assert!(
            result.is_err(),
            "Duplicate scoped content should be rejected"
        );
    }

    #[test]
    fn test_memory_entries_v2_all_categories() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let categories = ["preference", "convention", "pattern", "correction", "fact"];
        for (i, cat) in categories.iter().enumerate() {
            let content = format!("test content {}", i);
            conn.execute(
                "INSERT INTO memory_entries_v2 (id, scope, project_path, category, content, content_hash)
                 VALUES (?1, 'project', '/test', ?2, ?3, lower(trim(?3)))",
                params![format!("mem-{}", i), *cat, content],
            )
            .unwrap();
        }

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_entries_v2 WHERE scope = 'project' AND project_path = '/test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_episodic_records_table_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        conn.execute(
            "INSERT INTO episodic_records (id, project_path, session_id, record_type, task_summary, approach_summary, outcome_summary)
             VALUES ('ep-001', '/test/project', 'session-1', 'success', 'Fix bug', 'Used grep to find issue', 'Bug fixed')",
            [],
        ).unwrap();

        let task: String = conn
            .query_row(
                "SELECT task_summary FROM episodic_records WHERE id = 'ep-001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(task, "Fix bug");

        // Verify record_type constraint
        let result = conn.execute(
            "INSERT INTO episodic_records (id, project_path, session_id, record_type, task_summary, approach_summary, outcome_summary)
             VALUES ('ep-002', '/test/project', 'session-1', 'invalid_type', 'Test', 'Test', 'Test')",
            [],
        );
        assert!(result.is_err(), "Invalid record_type should be rejected");
    }

    #[test]
    fn test_episodic_records_all_types() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let types = ["success", "failure", "discovery"];
        for (i, rt) in types.iter().enumerate() {
            conn.execute(
                "INSERT INTO episodic_records (id, project_path, session_id, record_type, task_summary, approach_summary, outcome_summary)
                 VALUES (?1, '/test', 'sess-1', ?2, 'task', 'approach', 'outcome')",
                params![format!("ep-{}", i), *rt],
            ).unwrap();
        }

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM episodic_records WHERE project_path = '/test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_memory_entries_v2_indexes_exist() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Check indexes via pragma
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='memory_entries_v2'",
            )
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(indexes.contains(&"idx_memory_entries_v2_unique_content".to_string()));
        assert!(indexes.contains(&"idx_memory_entries_v2_scope_project_session".to_string()));
        assert!(indexes.contains(&"idx_memory_entries_v2_status_risk".to_string()));
        assert!(indexes.contains(&"idx_memory_entries_v2_category_importance".to_string()));
    }

    #[test]
    fn test_memory_entries_v2_last_decay_column_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();
        assert!(Database::table_has_column(
            &conn,
            "memory_entries_v2",
            "last_decay_at"
        ));
    }

    fn create_legacy_memory_db_for_migration() -> Database {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).unwrap();
        {
            let conn = pool.get().unwrap();
            conn.execute_batch(
                "CREATE TABLE project_memories (
                    id TEXT PRIMARY KEY,
                    project_path TEXT NOT NULL,
                    category TEXT NOT NULL,
                    content TEXT NOT NULL,
                    keywords TEXT NOT NULL DEFAULT '[]',
                    embedding BLOB,
                    importance REAL NOT NULL DEFAULT 0.5,
                    access_count INTEGER NOT NULL DEFAULT 0,
                    source_session_id TEXT,
                    source_context TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                    last_accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
                    UNIQUE(project_path, content)
                );",
            )
            .unwrap();

            conn.execute(
                "INSERT INTO project_memories (id, project_path, category, content, keywords, importance, source_context)
                 VALUES ('legacy-project', '/legacy/project', 'fact', 'legacy project memory', '[\"legacy\"]', 0.8, 'manual')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO project_memories (id, project_path, category, content, keywords, importance, source_context)
                 VALUES ('legacy-global', '__global__', 'preference', 'legacy global memory', '[\"global\"]', 0.9, 'manual')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO project_memories (id, project_path, category, content, keywords, importance, source_context)
                 VALUES ('legacy-session', '__session__:sess-1', 'pattern', 'legacy session memory', '[\"session\"]', 0.7, 'llm_extract:session')",
                [],
            )
            .unwrap();
        }

        Database { pool }
    }

    fn restore_memory_entries_from_legacy_backup(conn: &rusqlite::Connection) {
        conn.execute_batch(
            "INSERT OR REPLACE INTO memory_entries_v2 (
                id, scope, project_path, session_id, category, content, content_hash,
                keywords, embedding, importance, access_count, source_session_id, source_context,
                status, risk_tier, conflict_flag, created_at, updated_at, last_accessed_at,
                last_decay_at, embedding_provider, embedding_dim, quality_score
            )
            SELECT
                id,
                CASE
                    WHEN project_path = '__global__' THEN 'global'
                    WHEN project_path LIKE '__session__:%' THEN 'session'
                    ELSE 'project'
                END,
                CASE
                    WHEN project_path = '__global__' THEN NULL
                    WHEN project_path LIKE '__session__:%' THEN NULL
                    ELSE project_path
                END,
                CASE
                    WHEN project_path LIKE '__session__:%' THEN substr(project_path, 13)
                    ELSE NULL
                END,
                category,
                content,
                lower(trim(content)),
                keywords,
                embedding,
                importance,
                access_count,
                source_session_id,
                source_context,
                CASE
                    WHEN source_context LIKE 'llm_extract:%' OR source_context LIKE 'rule_extract:%'
                        THEN 'pending_review'
                    ELSE 'active'
                END,
                CASE
                    WHEN source_context LIKE 'llm_extract:%' THEN 'medium'
                    WHEN source_context LIKE 'rule_extract:%' THEN 'low'
                    ELSE 'high'
                END,
                0,
                created_at,
                updated_at,
                last_accessed_at,
                last_decay_at,
                COALESCE(embedding_provider, 'tfidf'),
                COALESCE(embedding_dim, 0),
                COALESCE(quality_score, 1.0)
            FROM project_memories_legacy_backup_v2;",
        )
        .unwrap();
    }

    #[test]
    fn test_memory_v2_migration_renames_legacy_table_and_backfills() {
        let db = create_legacy_memory_db_for_migration();
        db.init_schema().unwrap();
        let conn = db.get_connection().unwrap();

        assert!(!Database::table_exists(&conn, "project_memories"));
        assert!(Database::table_exists(
            &conn,
            "project_memories_legacy_backup_v2"
        ));

        let backup_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_memories_legacy_backup_v2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let v2_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_entries_v2", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(backup_count, 3);
        assert_eq!(v2_count, 3);

        let (scope, project_path, session_id): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT scope, project_path, session_id
                 FROM memory_entries_v2
                 WHERE id = 'legacy-session'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(scope, "session");
        assert_eq!(project_path, None);
        assert_eq!(session_id, Some("sess-1".to_string()));
    }

    #[test]
    fn test_memory_v2_rollback_rehearsal_from_backup() {
        let db = create_legacy_memory_db_for_migration();
        db.init_schema().unwrap();
        let conn = db.get_connection().unwrap();

        conn.execute("DELETE FROM memory_entries_v2", []).unwrap();
        let count_after_delete: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_entries_v2", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count_after_delete, 0);

        restore_memory_entries_from_legacy_backup(&conn);

        let restored_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_entries_v2", [], |row| {
                row.get(0)
            })
            .unwrap();
        let backup_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_memories_legacy_backup_v2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(restored_count, backup_count);
    }

    #[test]
    fn test_episodic_records_indexes_exist() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='episodic_records'",
            )
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(indexes.contains(&"idx_episodic_records_project".to_string()));
    }

    // =========================================================================
    // Feature-003 (Phase 3): LSP Enhancement Layer schema tests
    // =========================================================================

    #[test]
    fn test_lsp_columns_added_to_file_symbols() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Verify the LSP columns exist
        assert!(Database::table_has_column(
            &conn,
            "file_symbols",
            "resolved_type"
        ));
        assert!(Database::table_has_column(
            &conn,
            "file_symbols",
            "reference_count"
        ));
        assert!(Database::table_has_column(
            &conn,
            "file_symbols",
            "is_exported"
        ));
    }

    #[test]
    fn test_lsp_columns_have_correct_defaults() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert a file_index and file_symbol to verify defaults
        conn.execute(
            "INSERT INTO file_index (project_path, file_path, content_hash)
             VALUES ('/test', 'src/main.rs', 'hash1')",
            [],
        )
        .unwrap();

        let file_id: i64 = conn
            .query_row(
                "SELECT id FROM file_index WHERE file_path = 'src/main.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        conn.execute(
            "INSERT INTO file_symbols (file_index_id, name, kind, line_number, start_line, end_line)
             VALUES (?1, 'test_fn', 'Function', 1, 1, 10)",
            params![file_id],
        ).unwrap();

        // Check defaults
        let (resolved_type, reference_count, is_exported): (Option<String>, i64, i64) = conn.query_row(
            "SELECT resolved_type, reference_count, is_exported FROM file_symbols WHERE name = 'test_fn'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();

        assert!(
            resolved_type.is_none(),
            "resolved_type should default to NULL"
        );
        assert_eq!(reference_count, 0, "reference_count should default to 0");
        assert_eq!(is_exported, 0, "is_exported should default to 0");
    }

    #[test]
    fn test_cross_references_table_created() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='cross_references'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "cross_references table should exist");
    }

    #[test]
    fn test_cross_references_crud() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert a cross-reference
        conn.execute(
            "INSERT INTO cross_references (project_path, source_file, source_line, source_symbol,
             target_file, target_line, target_symbol, reference_kind)
             VALUES ('/test', 'src/main.rs', 10, 'call_foo', 'src/lib.rs', 5, 'foo', 'call')",
            [],
        )
        .unwrap();

        // Query it back
        let (sf, sl, tf, tl, kind): (String, i64, String, i64, String) = conn
            .query_row(
                "SELECT source_file, source_line, target_file, target_line, reference_kind
             FROM cross_references WHERE project_path = '/test'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(sf, "src/main.rs");
        assert_eq!(sl, 10);
        assert_eq!(tf, "src/lib.rs");
        assert_eq!(tl, 5);
        assert_eq!(kind, "call");
    }

    #[test]
    fn test_cross_references_unique_constraint() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        conn.execute(
            "INSERT INTO cross_references (project_path, source_file, source_line, target_file, target_line)
             VALUES ('/test', 'a.rs', 1, 'b.rs', 5)",
            [],
        ).unwrap();

        // Duplicate should fail
        let result = conn.execute(
            "INSERT INTO cross_references (project_path, source_file, source_line, target_file, target_line)
             VALUES ('/test', 'a.rs', 1, 'b.rs', 5)",
            [],
        );
        assert!(
            result.is_err(),
            "Duplicate cross-reference should be rejected"
        );
    }

    #[test]
    fn test_cross_references_indexes_exist() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='cross_references'",
            )
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(indexes.contains(&"idx_cross_refs_source".to_string()));
        assert!(indexes.contains(&"idx_cross_refs_target".to_string()));
    }

    #[test]
    fn test_lsp_servers_table_created() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='lsp_servers'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "lsp_servers table should exist");
    }

    #[test]
    fn test_lsp_servers_crud() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert
        conn.execute(
            "INSERT INTO lsp_servers (language, binary_path, server_name, version)
             VALUES ('rust', '/usr/bin/rust-analyzer', 'rust-analyzer', '2024-01-01')",
            [],
        )
        .unwrap();

        // Query
        let (lang, path, name): (String, String, String) = conn.query_row(
            "SELECT language, binary_path, server_name FROM lsp_servers WHERE language = 'rust'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();

        assert_eq!(lang, "rust");
        assert_eq!(path, "/usr/bin/rust-analyzer");
        assert_eq!(name, "rust-analyzer");

        // Upsert (replace)
        conn.execute(
            "INSERT OR REPLACE INTO lsp_servers (language, binary_path, server_name, version)
             VALUES ('rust', '/new/path/rust-analyzer', 'rust-analyzer', '2024-02-01')",
            [],
        )
        .unwrap();

        let new_path: String = conn
            .query_row(
                "SELECT binary_path FROM lsp_servers WHERE language = 'rust'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(new_path, "/new/path/rust-analyzer");
    }

    #[test]
    fn test_lsp_schema_migration_idempotent() {
        // Running init_schema twice should not fail
        let db = create_test_db().unwrap();
        db.init_schema().unwrap();

        let conn = db.get_connection().unwrap();
        // Tables should still exist
        assert!(Database::table_has_column(
            &conn,
            "file_symbols",
            "resolved_type"
        ));

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='cross_references'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    // =========================================================================
    // Feature-002: FTS5 Full-Text Search schema tests
    // =========================================================================

    #[test]
    fn test_symbol_fts_table_created_on_fresh_db() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Verify symbol_fts exists by querying sqlite_master
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='symbol_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "symbol_fts virtual table should exist");
    }

    #[test]
    fn test_filepath_fts_table_created_on_fresh_db() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Verify filepath_fts exists by querying sqlite_master
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='filepath_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "filepath_fts virtual table should exist");
    }

    #[test]
    fn test_symbol_fts_accepts_inserts() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert into symbol_fts (contentless mode with manual rowid)
        conn.execute(
            "INSERT INTO symbol_fts (rowid, symbol_name, file_path, symbol_kind, doc_comment, signature)
             VALUES (1, 'UserController', 'src/controller.rs', 'Struct', 'A user controller', 'pub struct UserController')",
            [],
        ).unwrap();

        // Query via MATCH — in contentless mode, column values are NULL,
        // so we verify by counting matches and checking rowid
        let rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM symbol_fts WHERE symbol_fts MATCH '\"UserController\"*'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rowid, 1);
    }

    #[test]
    fn test_filepath_fts_accepts_inserts() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert into filepath_fts (contentless mode with manual rowid)
        conn.execute(
            "INSERT INTO filepath_fts (rowid, file_path, component, language)
             VALUES (1, 'src/services/auth.rs', 'backend', 'rust')",
            [],
        )
        .unwrap();

        // Query via MATCH — in contentless mode, column values are NULL,
        // so we verify by counting matches and checking rowid
        let rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM filepath_fts WHERE filepath_fts MATCH '\"auth\"*'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rowid, 1);
    }

    #[test]
    fn test_fts5_schema_idempotent() {
        // Running init_schema twice should not fail
        let db = create_test_db().unwrap();
        // init_schema was already called in create_test_db; call it again
        db.init_schema().unwrap();

        let conn = db.get_connection().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='symbol_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "symbol_fts should still exist after double init");
    }

    #[test]
    fn test_fts5_migration_populates_from_existing_data() {
        let db = create_test_db().unwrap();

        // Insert test data into file_index and file_symbols
        // Scope the connection so it's returned to pool before populate_fts
        {
            let conn = db.get_connection().unwrap();
            conn.execute(
                "INSERT INTO file_index (project_path, file_path, component, language, content_hash)
                 VALUES ('/test', 'src/main.rs', 'backend', 'rust', 'hash1')",
                [],
            ).unwrap();

            let file_id: i64 = conn
                .query_row(
                    "SELECT id FROM file_index WHERE file_path = 'src/main.rs'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();

            conn.execute(
                "INSERT INTO file_symbols (file_index_id, name, kind, line_number, signature, doc_comment, start_line, end_line)
                 VALUES (?1, 'main', 'Function', 1, 'fn main()', 'Entry point', 1, 10)",
                params![file_id],
            ).unwrap();
        }

        // Run migration (populate_fts_from_existing)
        db.populate_fts_from_existing().unwrap();

        // Verify symbol_fts was populated
        let conn = db.get_connection().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbol_fts WHERE symbol_fts MATCH '\"main\"*'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            count > 0,
            "symbol_fts should have been populated from existing data"
        );

        // Verify filepath_fts was populated
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM filepath_fts WHERE filepath_fts MATCH '\"main\"*'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            count > 0,
            "filepath_fts should have been populated from existing data"
        );
    }

    // =========================================================================
    // Feature-004: Guardrail Security System schema tests
    // =========================================================================

    #[test]
    fn test_guardrail_rules_table_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='guardrail_rules'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "guardrail_rules table should exist");
    }

    #[test]
    fn test_guardrail_rules_crud() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert
        conn.execute(
            "INSERT INTO guardrail_rules (id, name, pattern, action, enabled)
             VALUES ('rule-1', 'No TODOs', 'TODO', 'warn', 1)",
            [],
        )
        .unwrap();

        // Query
        let (name, pattern, action): (String, String, String) = conn
            .query_row(
                "SELECT name, pattern, action FROM guardrail_rules WHERE id = 'rule-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "No TODOs");
        assert_eq!(pattern, "TODO");
        assert_eq!(action, "warn");

        // Update
        conn.execute(
            "UPDATE guardrail_rules SET enabled = 0 WHERE id = 'rule-1'",
            [],
        )
        .unwrap();

        let enabled: i32 = conn
            .query_row(
                "SELECT enabled FROM guardrail_rules WHERE id = 'rule-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(enabled, 0);

        // Delete
        conn.execute("DELETE FROM guardrail_rules WHERE id = 'rule-1'", [])
            .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM guardrail_rules", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_guardrail_trigger_log_table_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='guardrail_trigger_log'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1, "guardrail_trigger_log table should exist");
    }

    #[test]
    fn test_guardrail_trigger_log_crud() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert
        conn.execute(
            "INSERT INTO guardrail_trigger_log (guardrail_name, direction, result_type, content_snippet)
             VALUES ('SensitiveData', 'input', 'redact', 'sk-abc...')",
            [],
        ).unwrap();

        // Query
        let (name, direction, result_type): (String, String, String) = conn.query_row(
            "SELECT guardrail_name, direction, result_type FROM guardrail_trigger_log WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        assert_eq!(name, "SensitiveData");
        assert_eq!(direction, "input");
        assert_eq!(result_type, "redact");
    }

    #[test]
    fn test_guardrail_trigger_log_indexes_exist() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='guardrail_trigger_log'"
        ).unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(indexes.contains(&"idx_guardrail_trigger_log_timestamp".to_string()));
        assert!(indexes.contains(&"idx_guardrail_trigger_log_name".to_string()));
    }

    #[test]
    fn test_guardrail_schema_idempotent() {
        let db = create_test_db().unwrap();
        db.init_schema().unwrap(); // second call

        let conn = db.get_connection().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='guardrail_rules'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "guardrail_rules should still exist after double init"
        );
    }

    // =========================================================================
    // Webhook Notification System schema tests
    // =========================================================================

    #[test]
    fn test_webhook_channels_table_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='webhook_channels'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "webhook_channels table should exist");
    }

    #[test]
    fn test_webhook_deliveries_table_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='webhook_deliveries'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1, "webhook_deliveries table should exist");
    }

    #[test]
    fn test_webhook_deliveries_status_index_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='webhook_deliveries'"
        ).unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(indexes.contains(&"idx_webhook_deliveries_status".to_string()));
        assert!(indexes.contains(&"idx_webhook_deliveries_retry_due".to_string()));
    }

    #[test]
    fn test_webhook_channel_crud() {
        use crate::services::webhook::types::*;

        let db = create_test_db().unwrap();

        let config = WebhookChannelConfig {
            id: "ch-001".to_string(),
            name: "Test Slack".to_string(),
            channel_type: WebhookChannelType::Slack,
            enabled: true,
            url: "https://hooks.slack.com/test".to_string(),
            secret: None,
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskComplete, WebhookEventType::TaskFailed],
            template: None,
            created_at: "2026-02-18T12:00:00Z".to_string(),
            updated_at: "2026-02-18T12:00:00Z".to_string(),
        };

        // Insert
        db.insert_webhook_channel(&config).unwrap();

        // Get by ID
        let loaded = db.get_webhook_channel("ch-001").unwrap().unwrap();
        assert_eq!(loaded.name, "Test Slack");
        assert!(loaded.enabled);
        assert_eq!(loaded.events.len(), 2);
        assert!(matches!(loaded.scope, WebhookScope::Global));

        // List
        let all = db.list_webhook_channels().unwrap();
        assert_eq!(all.len(), 1);

        // Update
        let mut updated = loaded;
        updated.name = "Updated Slack".to_string();
        updated.enabled = false;
        updated.updated_at = "2026-02-18T13:00:00Z".to_string();
        db.update_webhook_channel(&updated).unwrap();

        let loaded = db.get_webhook_channel("ch-001").unwrap().unwrap();
        assert_eq!(loaded.name, "Updated Slack");
        assert!(!loaded.enabled);

        // Delete
        db.delete_webhook_channel("ch-001").unwrap();
        let loaded = db.get_webhook_channel("ch-001").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_webhook_channel_scope_sessions() {
        use crate::services::webhook::types::*;

        let db = create_test_db().unwrap();

        let config = WebhookChannelConfig {
            id: "ch-002".to_string(),
            name: "Session Scoped".to_string(),
            channel_type: WebhookChannelType::Custom,
            enabled: true,
            url: "https://example.com/webhook".to_string(),
            secret: None,
            scope: WebhookScope::Sessions(vec!["s1".to_string(), "s2".to_string()]),
            events: vec![WebhookEventType::TaskComplete],
            template: Some("custom template".to_string()),
            created_at: "2026-02-18T12:00:00Z".to_string(),
            updated_at: "2026-02-18T12:00:00Z".to_string(),
        };

        db.insert_webhook_channel(&config).unwrap();

        let loaded = db.get_webhook_channel("ch-002").unwrap().unwrap();
        match &loaded.scope {
            WebhookScope::Sessions(ids) => {
                assert_eq!(ids.len(), 2);
                assert!(ids.contains(&"s1".to_string()));
                assert!(ids.contains(&"s2".to_string()));
            }
            _ => panic!("Expected Sessions scope"),
        }
        assert_eq!(loaded.template, Some("custom template".to_string()));
    }

    #[test]
    fn test_webhook_delivery_crud() {
        use crate::services::webhook::types::*;

        let db = create_test_db().unwrap();

        // First create a channel
        let channel = WebhookChannelConfig {
            id: "ch-001".to_string(),
            name: "Test".to_string(),
            channel_type: WebhookChannelType::Slack,
            enabled: true,
            url: "https://hooks.slack.com/test".to_string(),
            secret: None,
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskComplete],
            template: None,
            created_at: "2026-02-18T12:00:00Z".to_string(),
            updated_at: "2026-02-18T12:00:00Z".to_string(),
        };
        db.insert_webhook_channel(&channel).unwrap();

        // Insert delivery
        let payload = WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            summary: "Test delivery".to_string(),
            timestamp: "2026-02-18T12:00:00Z".to_string(),
            ..Default::default()
        };
        let delivery = WebhookDelivery::new(&channel, &payload);
        let delivery_id = delivery.id.clone();

        db.insert_webhook_delivery(&delivery).unwrap();

        // Get by ID
        let loaded = db.get_webhook_delivery(&delivery_id).unwrap().unwrap();
        assert_eq!(loaded.channel_id, "ch-001");
        assert_eq!(loaded.status, DeliveryStatus::Pending);

        // List with pagination
        let all = db.list_webhook_deliveries(None, 10, 0).unwrap();
        assert_eq!(all.len(), 1);

        // List filtered by channel
        let filtered = db.list_webhook_deliveries(Some("ch-001"), 10, 0).unwrap();
        assert_eq!(filtered.len(), 1);

        let empty = db.list_webhook_deliveries(Some("ch-999"), 10, 0).unwrap();
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_webhook_delivery_pagination() {
        use crate::services::webhook::types::*;

        let db = create_test_db().unwrap();

        let channel = WebhookChannelConfig {
            id: "ch-001".to_string(),
            name: "Test".to_string(),
            channel_type: WebhookChannelType::Slack,
            enabled: true,
            url: "https://hooks.slack.com/test".to_string(),
            secret: None,
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskComplete],
            template: None,
            created_at: "2026-02-18T12:00:00Z".to_string(),
            updated_at: "2026-02-18T12:00:00Z".to_string(),
        };
        db.insert_webhook_channel(&channel).unwrap();

        // Insert 5 deliveries
        for i in 0..5 {
            let payload = WebhookPayload {
                event_type: WebhookEventType::TaskComplete,
                summary: format!("Delivery {}", i),
                timestamp: "2026-02-18T12:00:00Z".to_string(),
                ..Default::default()
            };
            let delivery = WebhookDelivery::new(&channel, &payload);
            db.insert_webhook_delivery(&delivery).unwrap();
        }

        // Get first page
        let page1 = db.list_webhook_deliveries(None, 2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        // Get second page
        let page2 = db.list_webhook_deliveries(None, 2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        // Get third page
        let page3 = db.list_webhook_deliveries(None, 2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[test]
    fn test_webhook_delivery_cascade_delete() {
        use crate::services::webhook::types::*;

        let db = create_test_db().unwrap();

        // Enable foreign keys explicitly
        {
            let conn = db.get_connection().unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        }

        let channel = WebhookChannelConfig {
            id: "ch-cascade".to_string(),
            name: "Cascade Test".to_string(),
            channel_type: WebhookChannelType::Custom,
            enabled: true,
            url: "https://example.com/webhook".to_string(),
            secret: None,
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskComplete],
            template: None,
            created_at: "2026-02-18T12:00:00Z".to_string(),
            updated_at: "2026-02-18T12:00:00Z".to_string(),
        };
        db.insert_webhook_channel(&channel).unwrap();

        // Insert deliveries
        for _ in 0..3 {
            let payload = WebhookPayload::default();
            let delivery = WebhookDelivery::new(&channel, &payload);
            db.insert_webhook_delivery(&delivery).unwrap();
        }

        // Verify deliveries exist
        let deliveries = db
            .list_webhook_deliveries(Some("ch-cascade"), 10, 0)
            .unwrap();
        assert_eq!(deliveries.len(), 3);

        // Delete channel — deliveries should cascade
        db.delete_webhook_channel("ch-cascade").unwrap();

        let deliveries = db
            .list_webhook_deliveries(Some("ch-cascade"), 10, 0)
            .unwrap();
        assert_eq!(deliveries.len(), 0);
    }

    #[test]
    fn test_webhook_failed_deliveries_query() {
        use crate::services::webhook::types::*;

        let db = create_test_db().unwrap();

        let channel = WebhookChannelConfig {
            id: "ch-001".to_string(),
            name: "Test".to_string(),
            channel_type: WebhookChannelType::Slack,
            enabled: true,
            url: "https://hooks.slack.com/test".to_string(),
            secret: None,
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskComplete],
            template: None,
            created_at: "2026-02-18T12:00:00Z".to_string(),
            updated_at: "2026-02-18T12:00:00Z".to_string(),
        };
        db.insert_webhook_channel(&channel).unwrap();

        // Insert a failed delivery with 1 attempt
        let payload = WebhookPayload::default();
        let mut delivery = WebhookDelivery::new(&channel, &payload);
        delivery.status = DeliveryStatus::Failed;
        delivery.attempts = 1;
        db.insert_webhook_delivery(&delivery).unwrap();

        // Insert a successful delivery
        let mut delivery2 = WebhookDelivery::new(&channel, &payload);
        delivery2.status = DeliveryStatus::Success;
        delivery2.attempts = 1;
        db.insert_webhook_delivery(&delivery2).unwrap();

        // Only failed ones with attempts < 3
        let failed = db.get_failed_deliveries(3).unwrap();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].status, DeliveryStatus::Failed);

        // With max_attempts=1, failed delivery should not appear (attempts >= max)
        let failed = db.get_failed_deliveries(1).unwrap();
        assert_eq!(failed.len(), 0);
    }

    #[test]
    fn test_webhook_delivery_retry_metadata_roundtrip() {
        use crate::services::webhook::types::*;

        let db = create_test_db().unwrap();

        let channel = WebhookChannelConfig {
            id: "ch-retry-meta".to_string(),
            name: "Retry metadata".to_string(),
            channel_type: WebhookChannelType::Custom,
            enabled: true,
            url: "https://example.com/hook".to_string(),
            secret: None,
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskFailed],
            template: None,
            created_at: "2026-03-03T00:00:00Z".to_string(),
            updated_at: "2026-03-03T00:00:00Z".to_string(),
        };
        db.insert_webhook_channel(&channel).unwrap();

        let mut delivery = WebhookDelivery::new(&channel, &WebhookPayload::default());
        delivery.status = DeliveryStatus::Failed;
        delivery.attempts = 2;
        delivery.next_retry_at = Some("2026-03-03T00:05:00Z".to_string());
        delivery.last_error = Some("HTTP 500".to_string());
        delivery.retryable = Some(true);
        delivery.error_class = Some("http_retryable".to_string());
        db.insert_webhook_delivery(&delivery).unwrap();

        let loaded = db.get_webhook_delivery(&delivery.id).unwrap().unwrap();
        assert_eq!(
            loaded.next_retry_at,
            Some("2026-03-03T00:05:00Z".to_string())
        );
        assert_eq!(loaded.last_error, Some("HTTP 500".to_string()));
        assert_eq!(loaded.retryable, Some(true));
        assert_eq!(loaded.error_class, Some("http_retryable".to_string()));
    }

    #[test]
    fn test_webhook_get_deliveries_due_for_retry() {
        use crate::services::webhook::types::*;

        let db = create_test_db().unwrap();
        let channel = WebhookChannelConfig {
            id: "ch-due".to_string(),
            name: "Due test".to_string(),
            channel_type: WebhookChannelType::Custom,
            enabled: true,
            url: "https://example.com/hook".to_string(),
            secret: None,
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskFailed],
            template: None,
            created_at: "2026-03-03T00:00:00Z".to_string(),
            updated_at: "2026-03-03T00:00:00Z".to_string(),
        };
        db.insert_webhook_channel(&channel).unwrap();

        let mut due = WebhookDelivery::new(&channel, &WebhookPayload::default());
        due.status = DeliveryStatus::Failed;
        due.attempts = 1;
        due.next_retry_at = Some("2026-03-03T00:00:00Z".to_string());
        db.insert_webhook_delivery(&due).unwrap();

        let mut not_due = WebhookDelivery::new(&channel, &WebhookPayload::default());
        not_due.status = DeliveryStatus::Failed;
        not_due.attempts = 1;
        not_due.next_retry_at = Some("2026-03-03T01:00:00Z".to_string());
        db.insert_webhook_delivery(&not_due).unwrap();

        let due_items = db
            .get_deliveries_due_for_retry(5, "2026-03-03T00:30:00Z")
            .unwrap();
        assert_eq!(due_items.len(), 1);
        assert_eq!(due_items[0].id, due.id);

        let queue_len = db.count_failed_webhook_deliveries().unwrap();
        assert_eq!(queue_len, 2);
    }

    #[test]
    fn test_webhook_schema_idempotent() {
        let db = create_test_db().unwrap();
        db.init_schema().unwrap(); // second call

        let conn = db.get_connection().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='webhook_channels'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "webhook_channels should still exist after double init"
        );

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='webhook_deliveries'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(
            count, 1,
            "webhook_deliveries should still exist after double init"
        );
    }

    // =========================================================================
    // Feature-002 (Phase 2): Remote Session Control schema tests
    // =========================================================================

    #[test]
    fn test_remote_session_mappings_table_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='remote_session_mappings'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1, "remote_session_mappings table should exist");
    }

    #[test]
    fn test_remote_session_mappings_crud() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert a mapping
        conn.execute(
            "INSERT INTO remote_session_mappings (chat_id, user_id, adapter_type, local_session_id, session_type, created_at, updated_at)
             VALUES (123456789, 111222333, 'telegram', 'session-abc-123', '{\"ClaudeCode\"}', '2026-02-18T14:30:00Z', '2026-02-18T14:30:00Z')",
            [],
        ).unwrap();

        // Query it back
        let (chat_id, session_id): (i64, String) = conn.query_row(
            "SELECT chat_id, local_session_id FROM remote_session_mappings WHERE chat_id = 123456789",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(chat_id, 123456789);
        assert_eq!(session_id, "session-abc-123");
    }

    #[test]
    fn test_remote_session_mappings_primary_key() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert first mapping
        conn.execute(
            "INSERT INTO remote_session_mappings (chat_id, user_id, adapter_type, local_session_id, session_type, created_at, updated_at)
             VALUES (123, 456, 'telegram', 'sess-1', 'ClaudeCode', '2026-02-18T14:30:00Z', '2026-02-18T14:30:00Z')",
            [],
        ).unwrap();

        // Duplicate (adapter_type, chat_id) should fail
        let result = conn.execute(
            "INSERT INTO remote_session_mappings (chat_id, user_id, adapter_type, local_session_id, session_type, created_at, updated_at)
             VALUES (123, 789, 'telegram', 'sess-2', 'ClaudeCode', '2026-02-18T14:31:00Z', '2026-02-18T14:31:00Z')",
            [],
        );
        assert!(
            result.is_err(),
            "Duplicate PRIMARY KEY (adapter_type, chat_id) should be rejected"
        );

        // Different adapter_type with same chat_id should succeed
        conn.execute(
            "INSERT INTO remote_session_mappings (chat_id, user_id, adapter_type, local_session_id, session_type, created_at, updated_at)
             VALUES (123, 456, 'slack', 'sess-3', 'ClaudeCode', '2026-02-18T14:32:00Z', '2026-02-18T14:32:00Z')",
            [],
        ).unwrap();
    }

    #[test]
    fn test_remote_audit_log_table_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='remote_audit_log'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "remote_audit_log table should exist");
    }

    #[test]
    fn test_remote_audit_log_crud() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert an audit entry
        conn.execute(
            "INSERT INTO remote_audit_log (id, adapter_type, chat_id, user_id, username, command_text, command_type, result_status, error_message, created_at)
             VALUES ('audit-001', 'telegram', 123456789, 111222333, 'testuser', '/new ~/projects/myapp', 'NewSession', 'success', NULL, '2026-02-18T14:30:00Z')",
            [],
        ).unwrap();

        // Query it back
        let (id, command_type, result_status): (String, String, String) = conn.query_row(
            "SELECT id, command_type, result_status FROM remote_audit_log WHERE id = 'audit-001'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        assert_eq!(id, "audit-001");
        assert_eq!(command_type, "NewSession");
        assert_eq!(result_status, "success");

        // Insert an error entry
        conn.execute(
            "INSERT INTO remote_audit_log (id, adapter_type, chat_id, user_id, username, command_text, command_type, result_status, error_message, created_at)
             VALUES ('audit-002', 'telegram', 123456789, 111222333, 'testuser', '/new ~/secret', 'NewSession', 'error', 'Unauthorized path', '2026-02-18T14:31:00Z')",
            [],
        ).unwrap();

        let error_msg: Option<String> = conn
            .query_row(
                "SELECT error_message FROM remote_audit_log WHERE id = 'audit-002'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(error_msg, Some("Unauthorized path".to_string()));
    }

    #[test]
    fn test_remote_audit_log_index_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='remote_audit_log'",
            )
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(
            indexes.contains(&"idx_remote_audit_created".to_string()),
            "idx_remote_audit_created index should exist"
        );
    }

    #[test]
    fn test_remote_audit_log_pagination() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert 5 entries
        for i in 0..5 {
            conn.execute(
                "INSERT INTO remote_audit_log (id, adapter_type, chat_id, user_id, command_text, command_type, result_status, created_at)
                 VALUES (?1, 'telegram', 123, 456, '/status', 'Status', 'success', ?2)",
                params![format!("audit-{}", i), format!("2026-02-18T14:3{}:00Z", i)],
            ).unwrap();
        }

        // Query with limit and offset
        let mut stmt = conn
            .prepare("SELECT id FROM remote_audit_log ORDER BY created_at DESC LIMIT 2 OFFSET 1")
            .unwrap();
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_remote_schema_idempotent() {
        let db = create_test_db().unwrap();
        db.init_schema().unwrap(); // second call

        let conn = db.get_connection().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='remote_session_mappings'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(
            count, 1,
            "remote_session_mappings should still exist after double init"
        );

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='remote_audit_log'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "remote_audit_log should still exist after double init"
        );
    }
}
