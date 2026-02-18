//! Gate Cache
//!
//! Caches quality gate results keyed by (gate_id, git_commit_hash, working_tree_hash)
//! in a SQLite table `quality_gate_cache`.
//!
//! Returns cache hits when git state hasn't changed, and cache misses after
//! file modifications.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::services::quality_gates::pipeline::PipelineGateResult;
use crate::utils::error::{AppError, AppResult};

/// Cache key for a gate result.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GateCacheKey {
    /// Gate identifier
    pub gate_id: String,
    /// Git commit hash (HEAD)
    pub commit_hash: String,
    /// Working tree hash (hash of uncommitted changes)
    pub tree_hash: String,
}

/// Cached gate result entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedGateResult {
    /// Cache key
    pub key: GateCacheKey,
    /// Whether the gate passed
    pub passed: bool,
    /// Serialized PipelineGateResult JSON
    pub result_json: String,
    /// When the result was cached
    pub cached_at: String,
}

/// Gate cache backed by SQLite.
pub struct GateCache {
    pool: Pool<SqliteConnectionManager>,
}

impl GateCache {
    /// Create a new GateCache with the given database pool.
    pub fn new(pool: Pool<SqliteConnectionManager>) -> AppResult<Self> {
        let cache = Self { pool };
        cache.init_schema()?;
        Ok(cache)
    }

    /// Initialize the cache table schema.
    fn init_schema(&self) -> AppResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS quality_gate_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                gate_id TEXT NOT NULL,
                commit_hash TEXT NOT NULL,
                tree_hash TEXT NOT NULL,
                passed INTEGER NOT NULL,
                result_json TEXT NOT NULL,
                cached_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(gate_id, commit_hash, tree_hash)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_gate_cache_key
             ON quality_gate_cache(gate_id, commit_hash, tree_hash)",
            [],
        )?;

        Ok(())
    }

    /// Look up a cached gate result.
    pub fn get(&self, key: &GateCacheKey) -> AppResult<Option<PipelineGateResult>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let result = conn.query_row(
            "SELECT result_json FROM quality_gate_cache
             WHERE gate_id = ?1 AND commit_hash = ?2 AND tree_hash = ?3",
            params![key.gate_id, key.commit_hash, key.tree_hash],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(json) => {
                let gate_result: PipelineGateResult = serde_json::from_str(&json)
                    .map_err(|e| AppError::parse(format!("Failed to parse cached result: {}", e)))?;
                Ok(Some(gate_result))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Store a gate result in the cache.
    pub fn put(&self, key: &GateCacheKey, result: &PipelineGateResult) -> AppResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let result_json = serde_json::to_string(result)
            .map_err(|e| AppError::Internal(format!("Failed to serialize result: {}", e)))?;

        conn.execute(
            "INSERT OR REPLACE INTO quality_gate_cache
             (gate_id, commit_hash, tree_hash, passed, result_json, cached_at)
             VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)",
            params![
                key.gate_id,
                key.commit_hash,
                key.tree_hash,
                result.passed as i32,
                result_json,
            ],
        )?;

        Ok(())
    }

    /// Invalidate all cache entries for a specific gate.
    pub fn invalidate_gate(&self, gate_id: &str) -> AppResult<u64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let count = conn.execute(
            "DELETE FROM quality_gate_cache WHERE gate_id = ?1",
            params![gate_id],
        )?;

        Ok(count as u64)
    }

    /// Invalidate all cache entries (e.g., after formatting changes files).
    pub fn invalidate_all(&self) -> AppResult<u64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let count = conn.execute("DELETE FROM quality_gate_cache", [])?;
        Ok(count as u64)
    }

    /// Get the number of cached entries.
    pub fn count(&self) -> AppResult<u64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM quality_gate_cache",
            [],
            |row| row.get(0),
        )?;

        Ok(count as u64)
    }

    /// Clean up old cache entries (older than specified days).
    pub fn cleanup(&self, days: i64) -> AppResult<u64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let count = conn.execute(
            "DELETE FROM quality_gate_cache
             WHERE cached_at < datetime('now', ?1 || ' days')",
            params![format!("-{}", days)],
        )?;

        Ok(count as u64)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::quality_gates::GateStatus;
    use crate::services::quality_gates::pipeline::GatePhase;

    fn create_test_cache() -> GateCache {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).unwrap();
        GateCache::new(pool).unwrap()
    }

    fn create_test_key() -> GateCacheKey {
        GateCacheKey {
            gate_id: "typecheck".to_string(),
            commit_hash: "abc123".to_string(),
            tree_hash: "def456".to_string(),
        }
    }

    fn create_test_result() -> PipelineGateResult {
        PipelineGateResult::passed("typecheck", "TypeCheck", GatePhase::Validation, 1000)
    }

    #[test]
    fn test_cache_miss() {
        let cache = create_test_cache();
        let key = create_test_key();
        let result = cache.get(&key).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_hit() {
        let cache = create_test_cache();
        let key = create_test_key();
        let gate_result = create_test_result();

        cache.put(&key, &gate_result).unwrap();
        let cached = cache.get(&key).unwrap();
        assert!(cached.is_some());
        let cached = cached.unwrap();
        assert!(cached.passed);
        assert_eq!(cached.gate_id, "typecheck");
        assert_eq!(cached.status, GateStatus::Passed);
    }

    #[test]
    fn test_cache_different_tree_hash_is_miss() {
        let cache = create_test_cache();
        let key = create_test_key();
        let gate_result = create_test_result();

        cache.put(&key, &gate_result).unwrap();

        let different_key = GateCacheKey {
            gate_id: "typecheck".to_string(),
            commit_hash: "abc123".to_string(),
            tree_hash: "different_hash".to_string(),
        };
        let result = cache.get(&different_key).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_invalidate_gate() {
        let cache = create_test_cache();
        let key = create_test_key();
        let gate_result = create_test_result();

        cache.put(&key, &gate_result).unwrap();
        assert_eq!(cache.count().unwrap(), 1);

        cache.invalidate_gate("typecheck").unwrap();
        assert_eq!(cache.count().unwrap(), 0);
    }

    #[test]
    fn test_cache_invalidate_all() {
        let cache = create_test_cache();

        let key1 = GateCacheKey {
            gate_id: "typecheck".to_string(),
            commit_hash: "abc".to_string(),
            tree_hash: "def".to_string(),
        };
        let key2 = GateCacheKey {
            gate_id: "lint".to_string(),
            commit_hash: "abc".to_string(),
            tree_hash: "def".to_string(),
        };
        let result = create_test_result();

        cache.put(&key1, &result).unwrap();
        cache.put(&key2, &result).unwrap();
        assert_eq!(cache.count().unwrap(), 2);

        cache.invalidate_all().unwrap();
        assert_eq!(cache.count().unwrap(), 0);
    }

    #[test]
    fn test_cache_upsert() {
        let cache = create_test_cache();
        let key = create_test_key();
        let passed_result = PipelineGateResult::passed("typecheck", "TypeCheck", GatePhase::Validation, 100);
        let failed_result = PipelineGateResult::failed(
            "typecheck",
            "TypeCheck",
            GatePhase::Validation,
            200,
            "error".to_string(),
            vec![],
        );

        cache.put(&key, &passed_result).unwrap();
        cache.put(&key, &failed_result).unwrap();
        assert_eq!(cache.count().unwrap(), 1);

        let cached = cache.get(&key).unwrap().unwrap();
        assert!(!cached.passed); // Should be the latest (failed) result
    }
}
