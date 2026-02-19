//! SQLite Checkpointer
//!
//! Production-ready implementation of the `Checkpointer` trait using SQLite
//! for persistent storage. Checkpoints survive process restarts.
//!
//! Uses r2d2 connection pooling for thread-safe database access.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use super::checkpointer::{Checkpointer, GraphCheckpoint, Interrupt};
use crate::utils::error::{AppError, AppResult};

/// SQLite-backed checkpoint storage for production use.
pub struct SqliteCheckpointer {
    pool: Arc<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>,
}

impl SqliteCheckpointer {
    /// Create a new SqliteCheckpointer with the given connection pool.
    ///
    /// Automatically creates the checkpoints table if it does not exist.
    pub fn new(pool: Arc<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>) -> AppResult<Self> {
        let conn = pool.get().map_err(|e| {
            AppError::database(format!("Failed to get connection: {}", e))
        })?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS graph_checkpoints (
                id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL,
                state TEXT NOT NULL,
                step TEXT NOT NULL,
                pending_nodes TEXT NOT NULL,
                interrupt TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| AppError::database(format!("Failed to create checkpoints table: {}", e)))?;

        // Create index for thread_id lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_checkpoints_thread_id
             ON graph_checkpoints(thread_id, created_at DESC)",
            [],
        )
        .map_err(|e| AppError::database(format!("Failed to create index: {}", e)))?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl Checkpointer for SqliteCheckpointer {
    async fn save(&self, checkpoint: GraphCheckpoint) -> AppResult<()> {
        let pool = self.pool.clone();
        let cp = checkpoint;

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            let state_json = serde_json::to_string(&cp.state)
                .map_err(|e| AppError::parse(format!("Failed to serialize state: {}", e)))?;
            let pending_json = serde_json::to_string(&cp.pending_nodes)
                .map_err(|e| AppError::parse(format!("Failed to serialize pending_nodes: {}", e)))?;
            let interrupt_json = cp
                .interrupt
                .as_ref()
                .map(|i| serde_json::to_string(i))
                .transpose()
                .map_err(|e| AppError::parse(format!("Failed to serialize interrupt: {}", e)))?;

            conn.execute(
                "INSERT OR REPLACE INTO graph_checkpoints
                 (id, thread_id, state, step, pending_nodes, interrupt, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    cp.id,
                    cp.thread_id,
                    state_json,
                    cp.step,
                    pending_json,
                    interrupt_json,
                    cp.created_at,
                ],
            )
            .map_err(|e| AppError::database(format!("Failed to save checkpoint: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| AppError::database(format!("Task join error: {}", e)))?
    }

    async fn load(&self, thread_id: &str) -> AppResult<Option<GraphCheckpoint>> {
        let pool = self.pool.clone();
        let tid = thread_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            let result = conn.query_row(
                "SELECT id, thread_id, state, step, pending_nodes, interrupt, created_at
                 FROM graph_checkpoints
                 WHERE thread_id = ?1
                 ORDER BY created_at DESC
                 LIMIT 1",
                rusqlite::params![tid],
                |row| {
                    Ok(RawCheckpointRow {
                        id: row.get(0)?,
                        thread_id: row.get(1)?,
                        state: row.get(2)?,
                        step: row.get(3)?,
                        pending_nodes: row.get(4)?,
                        interrupt: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            );

            match result {
                Ok(raw) => Ok(Some(parse_checkpoint_row(raw)?)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(AppError::database(format!("Failed to load checkpoint: {}", e))),
            }
        })
        .await
        .map_err(|e| AppError::database(format!("Task join error: {}", e)))?
    }

    async fn load_by_id(&self, checkpoint_id: &str) -> AppResult<Option<GraphCheckpoint>> {
        let pool = self.pool.clone();
        let cid = checkpoint_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            let result = conn.query_row(
                "SELECT id, thread_id, state, step, pending_nodes, interrupt, created_at
                 FROM graph_checkpoints
                 WHERE id = ?1",
                rusqlite::params![cid],
                |row| {
                    Ok(RawCheckpointRow {
                        id: row.get(0)?,
                        thread_id: row.get(1)?,
                        state: row.get(2)?,
                        step: row.get(3)?,
                        pending_nodes: row.get(4)?,
                        interrupt: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            );

            match result {
                Ok(raw) => Ok(Some(parse_checkpoint_row(raw)?)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(AppError::database(format!("Failed to load checkpoint: {}", e))),
            }
        })
        .await
        .map_err(|e| AppError::database(format!("Task join error: {}", e)))?
    }

    async fn list(&self, thread_id: &str) -> AppResult<Vec<GraphCheckpoint>> {
        let pool = self.pool.clone();
        let tid = thread_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            let mut stmt = conn
                .prepare(
                    "SELECT id, thread_id, state, step, pending_nodes, interrupt, created_at
                     FROM graph_checkpoints
                     WHERE thread_id = ?1
                     ORDER BY created_at DESC",
                )
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let rows: Vec<RawCheckpointRow> = stmt
                .query_map(rusqlite::params![tid], |row| {
                    Ok(RawCheckpointRow {
                        id: row.get(0)?,
                        thread_id: row.get(1)?,
                        state: row.get(2)?,
                        step: row.get(3)?,
                        pending_nodes: row.get(4)?,
                        interrupt: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                })
                .map_err(|e| AppError::database(format!("Failed to query checkpoints: {}", e)))?
                .filter_map(|r| r.ok())
                .collect();

            let mut checkpoints = Vec::new();
            for raw in rows {
                checkpoints.push(parse_checkpoint_row(raw)?);
            }

            Ok(checkpoints)
        })
        .await
        .map_err(|e| AppError::database(format!("Task join error: {}", e)))?
    }

    async fn delete(&self, checkpoint_id: &str) -> AppResult<bool> {
        let pool = self.pool.clone();
        let cid = checkpoint_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            let deleted = conn
                .execute(
                    "DELETE FROM graph_checkpoints WHERE id = ?1",
                    rusqlite::params![cid],
                )
                .map_err(|e| AppError::database(format!("Failed to delete checkpoint: {}", e)))?;

            Ok(deleted > 0)
        })
        .await
        .map_err(|e| AppError::database(format!("Task join error: {}", e)))?
    }
}

// ============================================================================
// Internal Helpers
// ============================================================================

/// Raw row data from the database before parsing JSON fields.
struct RawCheckpointRow {
    id: String,
    thread_id: String,
    state: String,
    step: String,
    pending_nodes: String,
    interrupt: Option<String>,
    created_at: String,
}

/// Parse a raw database row into a GraphCheckpoint.
fn parse_checkpoint_row(raw: RawCheckpointRow) -> AppResult<GraphCheckpoint> {
    let state: HashMap<String, Value> = serde_json::from_str(&raw.state)
        .map_err(|e| AppError::parse(format!("Failed to parse checkpoint state: {}", e)))?;

    let pending_nodes: Vec<String> = serde_json::from_str(&raw.pending_nodes)
        .map_err(|e| AppError::parse(format!("Failed to parse pending_nodes: {}", e)))?;

    let interrupt: Option<Interrupt> = raw
        .interrupt
        .as_deref()
        .map(|s| serde_json::from_str(s))
        .transpose()
        .map_err(|e| AppError::parse(format!("Failed to parse interrupt: {}", e)))?;

    Ok(GraphCheckpoint {
        id: raw.id,
        thread_id: raw.thread_id,
        state,
        step: raw.step,
        pending_nodes,
        interrupt,
        created_at: raw.created_at,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    fn create_test_pool() -> Arc<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>> {
        let db = Database::new_in_memory().unwrap();
        Arc::new(db.pool().clone())
    }

    #[test]
    fn test_sqlite_checkpointer_new() {
        let pool = create_test_pool();
        let result = SqliteCheckpointer::new(pool);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sqlite_checkpointer_creates_table() {
        let pool = create_test_pool();
        let _cp = SqliteCheckpointer::new(pool.clone()).unwrap();

        let conn = pool.get().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='graph_checkpoints'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_sqlite_checkpointer_idempotent_creation() {
        let pool = create_test_pool();
        let _cp1 = SqliteCheckpointer::new(pool.clone()).unwrap();
        let _cp2 = SqliteCheckpointer::new(pool).unwrap();
    }

    #[tokio::test]
    async fn test_sqlite_save_and_load() {
        let pool = create_test_pool();
        let cp_store = SqliteCheckpointer::new(pool).unwrap();

        let mut state = HashMap::new();
        state.insert("counter".to_string(), serde_json::json!(42));

        let checkpoint = GraphCheckpoint {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            state,
            step: "node-a".to_string(),
            pending_nodes: vec!["node-b".to_string()],
            interrupt: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        cp_store.save(checkpoint).await.unwrap();

        let loaded = cp_store.load("thread-1").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, "cp-1");
        assert_eq!(loaded.thread_id, "thread-1");
        assert_eq!(loaded.step, "node-a");
        assert_eq!(loaded.state.get("counter"), Some(&serde_json::json!(42)));
        assert_eq!(loaded.pending_nodes, vec!["node-b"]);
        assert!(loaded.interrupt.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_save_with_interrupt() {
        let pool = create_test_pool();
        let cp_store = SqliteCheckpointer::new(pool).unwrap();

        let checkpoint = GraphCheckpoint {
            id: "cp-2".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "review-node".to_string(),
            pending_nodes: vec![],
            interrupt: Some(Interrupt::Before {
                node_id: "review-node".to_string(),
            }),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        cp_store.save(checkpoint).await.unwrap();

        let loaded = cp_store.load_by_id("cp-2").await.unwrap().unwrap();
        assert!(loaded.interrupt.is_some());
        match loaded.interrupt.unwrap() {
            Interrupt::Before { node_id } => assert_eq!(node_id, "review-node"),
            _ => panic!("Expected Before interrupt"),
        }
    }

    #[tokio::test]
    async fn test_sqlite_load_nonexistent() {
        let pool = create_test_pool();
        let cp_store = SqliteCheckpointer::new(pool).unwrap();

        let loaded = cp_store.load("nonexistent").await.unwrap();
        assert!(loaded.is_none());

        let loaded = cp_store.load_by_id("nonexistent").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_load_returns_newest() {
        let pool = create_test_pool();
        let cp_store = SqliteCheckpointer::new(pool).unwrap();

        let cp1 = GraphCheckpoint {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-a".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let cp2 = GraphCheckpoint {
            id: "cp-2".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-b".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-02T00:00:00Z".to_string(),
        };

        cp_store.save(cp1).await.unwrap();
        cp_store.save(cp2).await.unwrap();

        let loaded = cp_store.load("thread-1").await.unwrap().unwrap();
        assert_eq!(loaded.id, "cp-2");
    }

    #[tokio::test]
    async fn test_sqlite_list() {
        let pool = create_test_pool();
        let cp_store = SqliteCheckpointer::new(pool).unwrap();

        let cp1 = GraphCheckpoint {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-a".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let cp2 = GraphCheckpoint {
            id: "cp-2".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-b".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-02T00:00:00Z".to_string(),
        };
        let cp3 = GraphCheckpoint {
            id: "cp-3".to_string(),
            thread_id: "thread-2".to_string(),
            state: HashMap::new(),
            step: "node-c".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-03T00:00:00Z".to_string(),
        };

        cp_store.save(cp1).await.unwrap();
        cp_store.save(cp2).await.unwrap();
        cp_store.save(cp3).await.unwrap();

        let list = cp_store.list("thread-1").await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "cp-2"); // Newest first
        assert_eq!(list[1].id, "cp-1");

        let list2 = cp_store.list("thread-2").await.unwrap();
        assert_eq!(list2.len(), 1);

        let empty = cp_store.list("nonexistent").await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_sqlite_delete() {
        let pool = create_test_pool();
        let cp_store = SqliteCheckpointer::new(pool).unwrap();

        let cp = GraphCheckpoint {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-a".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        cp_store.save(cp).await.unwrap();

        let deleted = cp_store.delete("cp-1").await.unwrap();
        assert!(deleted);

        let loaded = cp_store.load_by_id("cp-1").await.unwrap();
        assert!(loaded.is_none());

        let deleted_again = cp_store.delete("cp-1").await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_sqlite_save_overwrites() {
        let pool = create_test_pool();
        let cp_store = SqliteCheckpointer::new(pool).unwrap();

        let cp1 = GraphCheckpoint {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-a".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        cp_store.save(cp1).await.unwrap();

        let cp1_updated = GraphCheckpoint {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-b".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-02T00:00:00Z".to_string(),
        };

        cp_store.save(cp1_updated).await.unwrap();

        let loaded = cp_store.load_by_id("cp-1").await.unwrap().unwrap();
        assert_eq!(loaded.step, "node-b");
    }

    #[tokio::test]
    async fn test_sqlite_save_dynamic_interrupt() {
        let pool = create_test_pool();
        let cp_store = SqliteCheckpointer::new(pool).unwrap();

        let cp = GraphCheckpoint {
            id: "cp-dyn".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-a".to_string(),
            pending_nodes: vec!["node-b".to_string(), "node-c".to_string()],
            interrupt: Some(Interrupt::Dynamic {
                message: "Need approval".to_string(),
                data: Some(serde_json::json!({"level": "high"})),
            }),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        cp_store.save(cp).await.unwrap();

        let loaded = cp_store.load_by_id("cp-dyn").await.unwrap().unwrap();
        match loaded.interrupt.unwrap() {
            Interrupt::Dynamic { message, data } => {
                assert_eq!(message, "Need approval");
                assert!(data.is_some());
            }
            _ => panic!("Expected Dynamic interrupt"),
        }
    }

    /// Verifies that checkpoints persist across SqliteCheckpointer recreation.
    /// This simulates the scenario where the application restarts but the
    /// underlying database file (or in-memory pool) persists.
    #[tokio::test]
    async fn test_sqlite_checkpoint_persists_across_recreation() {
        let pool = create_test_pool();

        // Create first checkpointer and save a checkpoint
        {
            let cp_store = SqliteCheckpointer::new(pool.clone()).unwrap();

            let mut state = HashMap::new();
            state.insert("workflow_step".to_string(), serde_json::json!("analyze"));
            state.insert("iteration".to_string(), serde_json::json!(3));

            let checkpoint = GraphCheckpoint {
                id: "persist-cp-1".to_string(),
                thread_id: "persist-thread".to_string(),
                state,
                step: "analysis-node".to_string(),
                pending_nodes: vec!["review-node".to_string(), "output-node".to_string()],
                interrupt: Some(Interrupt::Before {
                    node_id: "review-node".to_string(),
                }),
                created_at: "2026-02-01T12:00:00Z".to_string(),
            };

            cp_store.save(checkpoint).await.unwrap();
        }
        // First checkpointer is dropped here, simulating process exit

        // Create a second checkpointer with the same pool (simulating restart)
        {
            let cp_store2 = SqliteCheckpointer::new(pool.clone()).unwrap();

            // Load by thread_id - should find the checkpoint saved by the first instance
            let loaded = cp_store2.load("persist-thread").await.unwrap();
            assert!(loaded.is_some(), "Checkpoint should survive checkpointer recreation");
            let loaded = loaded.unwrap();

            assert_eq!(loaded.id, "persist-cp-1");
            assert_eq!(loaded.thread_id, "persist-thread");
            assert_eq!(loaded.step, "analysis-node");
            assert_eq!(
                loaded.state.get("workflow_step"),
                Some(&serde_json::json!("analyze"))
            );
            assert_eq!(
                loaded.state.get("iteration"),
                Some(&serde_json::json!(3))
            );
            assert_eq!(
                loaded.pending_nodes,
                vec!["review-node".to_string(), "output-node".to_string()]
            );
            assert!(loaded.interrupt.is_some());
            match loaded.interrupt.unwrap() {
                Interrupt::Before { node_id } => {
                    assert_eq!(node_id, "review-node");
                }
                _ => panic!("Expected Before interrupt"),
            }

            // Load by checkpoint_id - should also work
            let loaded_by_id = cp_store2.load_by_id("persist-cp-1").await.unwrap();
            assert!(
                loaded_by_id.is_some(),
                "Checkpoint should be loadable by ID after recreation"
            );

            // List should also work
            let list = cp_store2.list("persist-thread").await.unwrap();
            assert_eq!(list.len(), 1);
        }
    }
}
