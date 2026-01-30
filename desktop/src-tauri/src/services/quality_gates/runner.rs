//! Quality Gate Execution Orchestrator
//!
//! Executes quality gates and manages results, including storing them in SQLite.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::models::quality_gates::{
    CustomGateConfig, GateResult, GatesSummary, ProjectType, QualityGate,
    StoredGateResult,
};
use crate::services::quality_gates::{detect_project_type, ValidatorRegistry};
use crate::utils::error::{AppError, AppResult};

/// Quality gate runner configuration
#[derive(Debug, Clone)]
pub struct RunnerConfig {
    /// Maximum output size to capture (bytes)
    pub max_output_size: usize,
    /// Whether to run gates in parallel
    pub parallel: bool,
    /// Custom environment variables
    pub env: std::collections::HashMap<String, String>,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            max_output_size: 1024 * 1024, // 1MB
            parallel: false,
            env: std::collections::HashMap::new(),
        }
    }
}

/// Quality gate runner - orchestrates gate execution
pub struct QualityGateRunner {
    /// Project path
    project_path: PathBuf,
    /// Validator registry
    registry: ValidatorRegistry,
    /// Runner configuration
    config: RunnerConfig,
    /// Database pool for storing results
    db_pool: Option<Pool<SqliteConnectionManager>>,
    /// Session ID for result tracking
    session_id: Option<String>,
}

impl QualityGateRunner {
    /// Create a new quality gate runner
    pub fn new(project_path: impl AsRef<Path>) -> Self {
        Self {
            project_path: project_path.as_ref().to_path_buf(),
            registry: ValidatorRegistry::new(),
            config: RunnerConfig::default(),
            db_pool: None,
            session_id: None,
        }
    }

    /// Set configuration
    pub fn with_config(mut self, config: RunnerConfig) -> Self {
        self.config = config;
        self
    }

    /// Set database pool for result storage
    pub fn with_database(mut self, pool: Pool<SqliteConnectionManager>) -> Self {
        self.db_pool = Some(pool);
        self
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Run all quality gates for the detected project type
    pub async fn run_all(&self) -> AppResult<GatesSummary> {
        let detection = detect_project_type(&self.project_path)?;

        if detection.project_type == ProjectType::Unknown {
            return Ok(GatesSummary::new(
                self.project_path.to_string_lossy(),
                ProjectType::Unknown,
            ));
        }

        let gates = self.registry.get_for_project_type(detection.project_type);
        let gates: Vec<QualityGate> = gates.into_iter().cloned().collect();

        self.run_gates(&gates, detection.project_type).await
    }

    /// Run specific quality gates by ID
    pub async fn run_specific(&self, gate_ids: &[String]) -> AppResult<GatesSummary> {
        let detection = detect_project_type(&self.project_path)?;
        let gates = self.registry.get_by_ids(gate_ids);
        let gates: Vec<QualityGate> = gates.into_iter().cloned().collect();

        self.run_gates(&gates, detection.project_type).await
    }

    /// Run custom gates from configuration
    pub async fn run_custom(&self, custom_gates: Vec<CustomGateConfig>) -> AppResult<GatesSummary> {
        let detection = detect_project_type(&self.project_path)?;
        let gates: Vec<QualityGate> = custom_gates.into_iter().map(|c| c.into()).collect();

        self.run_gates(&gates, detection.project_type).await
    }

    /// Run a list of quality gates
    async fn run_gates(&self, gates: &[QualityGate], project_type: ProjectType) -> AppResult<GatesSummary> {
        let mut summary = GatesSummary::new(
            self.project_path.to_string_lossy(),
            project_type,
        );

        for gate in gates {
            let result = self.run_single_gate(gate).await;

            // Store result in database if available
            if let Some(pool) = &self.db_pool {
                if let Err(e) = self.store_result(pool, &result) {
                    eprintln!("Failed to store gate result: {}", e);
                }
            }

            summary.add_result(result);
        }

        summary.finalize();
        Ok(summary)
    }

    /// Run a single quality gate
    async fn run_single_gate(&self, gate: &QualityGate) -> GateResult {
        let start = Instant::now();

        // Check if the command exists
        if !self.command_exists(&gate.command).await {
            return GateResult::skipped(
                gate,
                format!("Command '{}' not found in PATH", gate.command),
            );
        }

        // Determine working directory
        let working_dir = if let Some(ref dir) = gate.working_dir {
            self.project_path.join(dir)
        } else {
            self.project_path.clone()
        };

        // Build command
        let mut cmd = Command::new(&gate.command);
        cmd.args(&gate.args)
            .current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Add environment variables
        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }
        for (key, value) in &gate.env {
            cmd.env(key, value);
        }

        // Execute with timeout
        let timeout_duration = Duration::from_secs(gate.timeout_secs);
        match timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let stdout = self.truncate_output(&output.stdout);
                let stderr = self.truncate_output(&output.stderr);

                if output.status.success() {
                    GateResult::passed(gate, stdout, stderr, duration_ms)
                } else {
                    let exit_code = output.status.code().unwrap_or(-1);
                    GateResult::failed(gate, exit_code, stdout, stderr, duration_ms)
                }
            }
            Ok(Err(e)) => {
                GateResult::error(gate, format!("Failed to execute command: {}", e))
            }
            Err(_) => {
                GateResult::error(
                    gate,
                    format!("Command timed out after {} seconds", gate.timeout_secs),
                )
            }
        }
    }

    /// Check if a command exists in PATH
    async fn command_exists(&self, command: &str) -> bool {
        // On Windows, try with .exe and .cmd extensions
        #[cfg(windows)]
        {
            let check = Command::new("where")
                .arg(command)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
            check.map(|s| s.success()).unwrap_or(false)
        }

        #[cfg(not(windows))]
        {
            let check = Command::new("which")
                .arg(command)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
            check.map(|s| s.success()).unwrap_or(false)
        }
    }

    /// Truncate output to max size
    fn truncate_output(&self, bytes: &[u8]) -> String {
        let s = String::from_utf8_lossy(bytes);
        if s.len() > self.config.max_output_size {
            let truncated = &s[..self.config.max_output_size];
            format!("{}\n... (output truncated)", truncated)
        } else {
            s.into_owned()
        }
    }

    /// Store result in database
    fn store_result(&self, pool: &Pool<SqliteConnectionManager>, result: &GateResult) -> AppResult<()> {
        let conn = pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Truncate stdout/stderr for storage
        let max_storage = 64 * 1024; // 64KB
        let stdout = if result.stdout.len() > max_storage {
            format!("{}... (truncated)", &result.stdout[..max_storage])
        } else {
            result.stdout.clone()
        };
        let stderr = if result.stderr.len() > max_storage {
            format!("{}... (truncated)", &result.stderr[..max_storage])
        } else {
            result.stderr.clone()
        };

        conn.execute(
            "INSERT INTO quality_gate_results
             (project_path, session_id, gate_id, gate_name, status, exit_code,
              stdout, stderr, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, CURRENT_TIMESTAMP)",
            params![
                self.project_path.to_string_lossy(),
                self.session_id,
                result.gate_id,
                result.gate_name,
                result.status.to_string(),
                result.exit_code,
                stdout,
                stderr,
                result.duration_ms as i64,
            ],
        )?;

        Ok(())
    }
}

/// Quality gates database service
pub struct QualityGatesStore {
    pool: Pool<SqliteConnectionManager>,
}

impl QualityGatesStore {
    /// Create a new store with the given pool
    pub fn new(pool: Pool<SqliteConnectionManager>) -> AppResult<Self> {
        let store = Self { pool };
        store.init_schema()?;
        Ok(store)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> AppResult<()> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS quality_gate_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                session_id TEXT,
                gate_id TEXT NOT NULL,
                gate_name TEXT NOT NULL,
                status TEXT NOT NULL,
                exit_code INTEGER,
                stdout TEXT,
                stderr TEXT,
                duration_ms INTEGER NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_gate_results_project
             ON quality_gate_results(project_path)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_gate_results_session
             ON quality_gate_results(session_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_gate_results_created
             ON quality_gate_results(created_at DESC)",
            [],
        )?;

        Ok(())
    }

    /// Get recent results for a project
    pub fn get_results_for_project(
        &self,
        project_path: &str,
        limit: Option<i64>,
    ) -> AppResult<Vec<StoredGateResult>> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let limit = limit.unwrap_or(100);
        let mut stmt = conn.prepare(
            "SELECT id, project_path, session_id, gate_id, gate_name, status,
                    exit_code, stdout, stderr, duration_ms, created_at
             FROM quality_gate_results
             WHERE project_path = ?1
             ORDER BY created_at DESC
             LIMIT ?2"
        )?;

        let results = stmt.query_map(params![project_path, limit], |row| {
            Ok(StoredGateResult {
                id: row.get(0)?,
                project_path: row.get(1)?,
                session_id: row.get(2)?,
                gate_id: row.get(3)?,
                gate_name: row.get(4)?,
                status: row.get(5)?,
                exit_code: row.get(6)?,
                stdout: row.get(7)?,
                stderr: row.get(8)?,
                duration_ms: row.get::<_, i64>(9)? as u64,
                created_at: row.get(10)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(results)
    }

    /// Get results for a session
    pub fn get_results_for_session(
        &self,
        session_id: &str,
    ) -> AppResult<Vec<StoredGateResult>> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn.prepare(
            "SELECT id, project_path, session_id, gate_id, gate_name, status,
                    exit_code, stdout, stderr, duration_ms, created_at
             FROM quality_gate_results
             WHERE session_id = ?1
             ORDER BY created_at DESC"
        )?;

        let results = stmt.query_map(params![session_id], |row| {
            Ok(StoredGateResult {
                id: row.get(0)?,
                project_path: row.get(1)?,
                session_id: row.get(2)?,
                gate_id: row.get(3)?,
                gate_name: row.get(4)?,
                status: row.get(5)?,
                exit_code: row.get(6)?,
                stdout: row.get(7)?,
                stderr: row.get(8)?,
                duration_ms: row.get::<_, i64>(9)? as u64,
                created_at: row.get(10)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(results)
    }

    /// Get a single result by ID
    pub fn get_result(&self, id: i64) -> AppResult<Option<StoredGateResult>> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let result = conn.query_row(
            "SELECT id, project_path, session_id, gate_id, gate_name, status,
                    exit_code, stdout, stderr, duration_ms, created_at
             FROM quality_gate_results
             WHERE id = ?1",
            params![id],
            |row| {
                Ok(StoredGateResult {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    session_id: row.get(2)?,
                    gate_id: row.get(3)?,
                    gate_name: row.get(4)?,
                    status: row.get(5)?,
                    exit_code: row.get(6)?,
                    stdout: row.get(7)?,
                    stderr: row.get(8)?,
                    duration_ms: row.get::<_, i64>(9)? as u64,
                    created_at: row.get(10)?,
                })
            },
        );

        match result {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Delete old results (older than days)
    pub fn cleanup_old_results(&self, days: i64) -> AppResult<i64> {
        let conn = self.pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let count = conn.execute(
            "DELETE FROM quality_gate_results
             WHERE created_at < datetime('now', ?1 || ' days')",
            params![format!("-{}", days)],
        )?;

        Ok(count as i64)
    }
}

/// Run quality gates for a project
pub async fn run_quality_gates(
    project_path: impl AsRef<Path>,
    gate_ids: Option<Vec<String>>,
    pool: Option<Pool<SqliteConnectionManager>>,
    session_id: Option<String>,
) -> AppResult<GatesSummary> {
    let mut runner = QualityGateRunner::new(project_path);

    if let Some(p) = pool {
        runner = runner.with_database(p);
    }

    if let Some(sid) = session_id {
        runner = runner.with_session(sid);
    }

    if let Some(ids) = gate_ids {
        runner.run_specific(&ids).await
    } else {
        runner.run_all().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use crate::models::quality_gates::GateStatus;

    fn create_temp_project() -> TempDir {
        let temp = tempfile::tempdir().unwrap();
        // Create a minimal package.json
        let package = r#"{"name": "test", "version": "1.0.0"}"#;
        fs::write(temp.path().join("package.json"), package).unwrap();
        temp
    }

    #[tokio::test]
    async fn test_runner_unknown_project() {
        let temp = tempfile::tempdir().unwrap();
        let runner = QualityGateRunner::new(temp.path());
        let summary = runner.run_all().await.unwrap();

        assert_eq!(summary.project_type, ProjectType::Unknown);
        assert_eq!(summary.total_gates, 0);
    }

    #[tokio::test]
    async fn test_runner_skips_missing_commands() {
        let temp = create_temp_project();
        let runner = QualityGateRunner::new(temp.path());

        let gate = QualityGate::new("test", "Test", "nonexistent-command-12345");
        let result = runner.run_single_gate(&gate).await;

        assert_eq!(result.status, GateStatus::Skipped);
    }

    #[test]
    fn test_store_init_schema() {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).unwrap();
        let store = QualityGatesStore::new(pool).unwrap();

        // Should be able to query the table
        let results = store.get_results_for_project("/test", Some(10)).unwrap();
        assert!(results.is_empty());
    }
}
