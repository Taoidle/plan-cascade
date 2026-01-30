//! SQLite Database
//!
//! Embedded database for persistent storage using rusqlite with r2d2 connection pooling.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use crate::utils::error::{AppError, AppResult};
use crate::utils::paths::database_path;

/// Type alias for the connection pool
pub type DbPool = Pool<SqliteConnectionManager>;

/// Database service for managing SQLite operations
pub struct Database {
    pool: DbPool,
}

impl Database {
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
        let conn = self.pool.get()
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

        Ok(())
    }

    /// Get a connection from the pool
    pub fn get_connection(&self) -> AppResult<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get()
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

        let servers = stmt.query_map([], |row| Self::row_to_mcp_server(row))?
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
    pub fn update_mcp_server_status(&self, id: &str, status: &crate::models::McpServerStatus) -> AppResult<()> {
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
    pub fn get_mcp_server_by_name(&self, name: &str) -> AppResult<Option<crate::models::McpServer>> {
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
        let status_str: String = row.get::<_, String>(9).unwrap_or_else(|_| "unknown".to_string());
        let last_checked: Option<String> = row.get(10)?;
        let created_at: Option<String> = row.get(11)?;
        let updated_at: Option<String> = row.get(12)?;

        let server_type = match server_type_str.as_str() {
            "sse" => crate::models::McpServerType::Sse,
            _ => crate::models::McpServerType::Stdio,
        };

        let args: Vec<String> = serde_json::from_str(&args_json).unwrap_or_default();
        let env: std::collections::HashMap<String, String> = serde_json::from_str(&env_json).unwrap_or_default();
        let headers: std::collections::HashMap<String, String> = serde_json::from_str(&headers_json).unwrap_or_default();

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
}
