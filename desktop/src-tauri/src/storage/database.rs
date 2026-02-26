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
                enabled INTEGER DEFAULT 1,
                status TEXT DEFAULT 'unknown',
                last_checked TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
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
                category TEXT NOT NULL DEFAULT 'custom',
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

        // Cross-session project memory
        conn.execute(
            "CREATE TABLE IF NOT EXISTS project_memories (
                id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                category TEXT NOT NULL CHECK(category IN (
                    'preference',
                    'convention',
                    'pattern',
                    'correction',
                    'fact'
                )),
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
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_project_memories_project
             ON project_memories(project_path)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_project_memories_category
             ON project_memories(project_path, category)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_project_memories_importance
             ON project_memories(project_path, importance DESC)",
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
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

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

        // Custom guardrail rules persisted across sessions
        conn.execute(
            "CREATE TABLE IF NOT EXISTS guardrail_rules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                pattern TEXT NOT NULL,
                action TEXT NOT NULL DEFAULT 'warn',
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Guardrail trigger event log
        conn.execute(
            "CREATE TABLE IF NOT EXISTS guardrail_trigger_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                guardrail_name TEXT NOT NULL,
                direction TEXT NOT NULL,
                result_type TEXT NOT NULL,
                content_snippet TEXT NOT NULL DEFAULT '',
                timestamp TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        // Indexes for guardrail queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_guardrail_trigger_log_timestamp
             ON guardrail_trigger_log(timestamp DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_guardrail_trigger_log_name
             ON guardrail_trigger_log(guardrail_name)",
            [],
        )?;

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
                created_at TEXT NOT NULL,
                FOREIGN KEY (channel_id) REFERENCES webhook_channels(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Index for delivery retry queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_status
             ON webhook_deliveries(status, last_attempt_at)",
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
            crate::models::McpServerType::Sse => "sse",
        };

        conn.execute(
            "INSERT INTO mcp_servers (id, name, server_type, command, args, env, url, headers, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            params![
                server.id,
                server.name,
                server_type,
                server.command,
                args_json,
                env_json,
                server.url,
                headers_json,
                server.enabled as i32,
            ],
        )?;

        Ok(())
    }

    /// Get an MCP server by ID
    pub fn get_mcp_server(&self, id: &str) -> AppResult<Option<crate::models::McpServer>> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT id, name, server_type, command, args, env, url, headers, enabled, status, last_checked, created_at, updated_at
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
            "SELECT id, name, server_type, command, args, env, url, headers, enabled, status, last_checked, created_at, updated_at
             FROM mcp_servers ORDER BY name ASC"
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
            crate::models::McpServerType::Sse => "sse",
        };
        let status = match &server.status {
            crate::models::McpServerStatus::Connected => "connected".to_string(),
            crate::models::McpServerStatus::Disconnected => "disconnected".to_string(),
            crate::models::McpServerStatus::Error(msg) => format!("error:{}", msg),
            crate::models::McpServerStatus::Unknown => "unknown".to_string(),
        };

        conn.execute(
            "UPDATE mcp_servers SET name = ?2, server_type = ?3, command = ?4, args = ?5, env = ?6,
             url = ?7, headers = ?8, enabled = ?9, status = ?10, last_checked = ?11, updated_at = CURRENT_TIMESTAMP
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
                server.enabled as i32,
                status,
                server.last_checked,
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

    /// Get MCP server by name (for duplicate detection)
    pub fn get_mcp_server_by_name(
        &self,
        name: &str,
    ) -> AppResult<Option<crate::models::McpServer>> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT id, name, server_type, command, args, env, url, headers, enabled, status, last_checked, created_at, updated_at
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
        let enabled: i32 = row.get(8)?;
        let status_str: String = row
            .get::<_, String>(9)
            .unwrap_or_else(|_| "unknown".to_string());
        let last_checked: Option<String> = row.get(10)?;
        let created_at: Option<String> = row.get(11)?;
        let updated_at: Option<String> = row.get(12)?;

        let server_type = match server_type_str.as_str() {
            "sse" => crate::models::McpServerType::Sse,
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

        Ok(crate::models::McpServer {
            id,
            name,
            server_type,
            command,
            args,
            env,
            url,
            headers,
            enabled: enabled != 0,
            status,
            last_checked,
            created_at,
            updated_at,
        })
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
            "INSERT INTO webhook_deliveries (id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                    "SELECT id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, created_at
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
                    "SELECT id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, created_at
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
            "SELECT id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, created_at
             FROM webhook_deliveries
             WHERE status = 'failed' AND attempts < ?1
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

    /// Update a webhook delivery status.
    pub fn update_webhook_delivery_status(
        &self,
        delivery: &crate::services::webhook::types::WebhookDelivery,
    ) -> AppResult<()> {
        let conn = self.get_connection()?;

        conn.execute(
            "UPDATE webhook_deliveries SET status = ?2, status_code = ?3, response_body = ?4, attempts = ?5, last_attempt_at = ?6
             WHERE id = ?1",
            params![
                delivery.id,
                delivery.status.to_string(),
                delivery.status_code,
                delivery.response_body,
                delivery.attempts,
                delivery.last_attempt_at,
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
            "SELECT id, channel_id, event_type, payload, status, status_code, response_body, attempts, last_attempt_at, created_at
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
        let created_at: String = row.get(9)?;

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

    // =========================================================================
    // Feature-001: Project Memory System schema tests
    // =========================================================================

    #[test]
    fn test_project_memories_table_exists() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert a memory entry
        conn.execute(
            "INSERT INTO project_memories (id, project_path, category, content, keywords, importance)
             VALUES ('mem-001', '/test/project', 'preference', 'Use pnpm not npm', '[\"pnpm\",\"npm\"]', 0.9)",
            [],
        ).unwrap();

        // Query it back
        let content: String = conn
            .query_row(
                "SELECT content FROM project_memories WHERE id = 'mem-001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(content, "Use pnpm not npm");

        // Verify category constraint
        let result = conn.execute(
            "INSERT INTO project_memories (id, project_path, category, content)
             VALUES ('mem-002', '/test/project', 'invalid_category', 'test')",
            [],
        );
        assert!(result.is_err(), "Invalid category should be rejected");
    }

    #[test]
    fn test_project_memories_unique_constraint() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        conn.execute(
            "INSERT INTO project_memories (id, project_path, category, content)
             VALUES ('mem-001', '/test/project', 'fact', 'This is a Tauri app')",
            [],
        )
        .unwrap();

        // Same project_path + content should fail with UNIQUE constraint
        let result = conn.execute(
            "INSERT INTO project_memories (id, project_path, category, content)
             VALUES ('mem-002', '/test/project', 'fact', 'This is a Tauri app')",
            [],
        );
        assert!(
            result.is_err(),
            "Duplicate project_path+content should be rejected"
        );
    }

    #[test]
    fn test_project_memories_all_categories() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        let categories = ["preference", "convention", "pattern", "correction", "fact"];
        for (i, cat) in categories.iter().enumerate() {
            conn.execute(
                "INSERT INTO project_memories (id, project_path, category, content)
                 VALUES (?1, '/test', ?2, ?3)",
                params![format!("mem-{}", i), *cat, format!("test content {}", i)],
            )
            .unwrap();
        }

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_memories WHERE project_path = '/test'",
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
    fn test_project_memories_indexes_exist() {
        let db = create_test_db().unwrap();
        let conn = db.get_connection().unwrap();

        // Check indexes via pragma
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='project_memories'",
            )
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(indexes.contains(&"idx_project_memories_project".to_string()));
        assert!(indexes.contains(&"idx_project_memories_category".to_string()));
        assert!(indexes.contains(&"idx_project_memories_importance".to_string()));
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
