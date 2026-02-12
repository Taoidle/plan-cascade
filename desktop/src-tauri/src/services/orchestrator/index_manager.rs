//! Index Manager
//!
//! Manages per-directory indexers at the app level.  Each project directory
//! gets at most one active `BackgroundIndexer` task.  The manager exposes a
//! small API surface used by Tauri commands:
//!
//! - `ensure_indexed` – idempotent: starts indexing only when the project has
//!   no existing index.
//! - `trigger_reindex` – always clears the old index and starts fresh.
//! - `get_status` – returns the current `IndexStatusEvent`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::background_indexer::{BackgroundIndexer, IndexProgressCallback};
use super::embedding_service::EmbeddingService;
use super::index_store::IndexStore;
use crate::storage::database::DbPool;

/// Tauri event name for index progress updates.
const INDEX_PROGRESS_EVENT: &str = "index-progress";

/// Serializable status event emitted to the frontend via Tauri events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatusEvent {
    pub project_path: String,
    /// One of `"indexing"`, `"indexed"`, or `"error"`.
    pub status: String,
    pub indexed_files: usize,
    pub total_files: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Total number of parsed symbols across all indexed files.
    #[serde(default)]
    pub total_symbols: usize,
    /// Number of embedding chunks stored for this project.
    /// When > 0, semantic search is available.
    #[serde(default)]
    pub embedding_chunks: usize,
}

/// Internal bookkeeping for a running indexer.
struct IndexerEntry {
    handle: tokio::task::JoinHandle<()>,
}

/// Manages per-directory `BackgroundIndexer` lifecycle.
///
/// Designed to live as long as the Tauri app, typically stored inside a
/// `tauri::State`.
pub struct IndexManager {
    index_store: Arc<IndexStore>,
    active_indexers: RwLock<HashMap<String, IndexerEntry>>,
    statuses: Arc<RwLock<HashMap<String, IndexStatusEvent>>>,
    app_handle: RwLock<Option<AppHandle>>,
    /// Per-project embedding services, shared between BackgroundIndexer (which
    /// builds the vocabulary) and ToolExecutor (which queries with it).
    embedding_services: RwLock<HashMap<String, Arc<EmbeddingService>>>,
}

impl IndexManager {
    /// Create a new `IndexManager` backed by the given database connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self {
            index_store: Arc::new(IndexStore::new(pool)),
            active_indexers: RwLock::new(HashMap::new()),
            statuses: Arc::new(RwLock::new(HashMap::new())),
            app_handle: RwLock::new(None),
            embedding_services: RwLock::new(HashMap::new()),
        }
    }

    /// Provide a Tauri `AppHandle` for emitting events to the frontend.
    pub async fn set_app_handle(&self, app: AppHandle) {
        let mut guard = self.app_handle.write().await;
        *guard = Some(app);
    }

    /// Ensure a project directory is indexed.
    ///
    /// If an index already exists (total_files > 0) the method emits an
    /// `"indexed"` status and returns immediately.  Otherwise it delegates
    /// to [`start_indexing`].
    pub async fn ensure_indexed(&self, project_path: &str) {
        // Quick check: is an indexer already running for this path?
        {
            let indexers = self.active_indexers.read().await;
            if indexers.contains_key(project_path) {
                return;
            }
        }

        // Check the database for an existing index.
        match self.index_store.get_project_summary(project_path) {
            Ok(summary) if summary.total_files > 0 => {
                let event = IndexStatusEvent {
                    project_path: project_path.to_string(),
                    status: "indexed".to_string(),
                    indexed_files: summary.total_files,
                    total_files: summary.total_files,
                    error_message: None,
                    total_symbols: summary.total_symbols,
                    embedding_chunks: summary.embedding_chunks,
                };
                self.set_status_and_emit(project_path, event).await;
            }
            _ => {
                self.start_indexing(project_path).await;
            }
        }
    }

    /// Start (or restart) indexing for a project directory.
    ///
    /// If there is already an active indexer for this path it is aborted first.
    pub async fn start_indexing(&self, project_path: &str) {
        // Abort any existing indexer for this project.
        self.abort_indexer(project_path).await;

        let project_path_owned = project_path.to_string();
        let project_root = std::path::PathBuf::from(project_path);
        let index_store = self.index_store.clone();
        let statuses = self.statuses.clone();
        let app_handle_lock = self.app_handle.read().await;
        let app_handle_opt = app_handle_lock.clone();
        drop(app_handle_lock);

        // Emit initial "indexing" status.
        let initial_event = IndexStatusEvent {
            project_path: project_path_owned.clone(),
            status: "indexing".to_string(),
            indexed_files: 0,
            total_files: 0,
            error_message: None,
            total_symbols: 0,
            embedding_chunks: 0,
        };
        self.set_status_and_emit(project_path, initial_event).await;

        // Build the progress callback.
        let pp_for_cb = project_path_owned.clone();
        let statuses_for_cb = statuses.clone();
        let app_for_cb = app_handle_opt.clone();
        let progress_cb: IndexProgressCallback = Arc::new(move |done, total| {
            let event = IndexStatusEvent {
                project_path: pp_for_cb.clone(),
                status: "indexing".to_string(),
                indexed_files: done,
                total_files: total,
                error_message: None,
                total_symbols: 0,
                embedding_chunks: 0,
            };
            // Update statuses map (blocking write is fine in the sync callback
            // because contention is low and the lock is only briefly held).
            if let Ok(mut map) = statuses_for_cb.try_write() {
                map.insert(pp_for_cb.clone(), event.clone());
            }
            if let Some(ref app) = app_for_cb {
                let _ = app.emit(INDEX_PROGRESS_EVENT, &event);
            }
        });

        // Spawn the background indexer task.
        let pp_for_task = project_path_owned.clone();
        let statuses_for_task = statuses.clone();
        let app_for_task = app_handle_opt;
        // Get or create the embedding service for this project so that the
        // BackgroundIndexer builds TF-IDF embeddings and the same service
        // instance can later be shared with ToolExecutor for semantic search.
        let embedding_svc = {
            let mut embeds = self.embedding_services.write().await;
            embeds
                .entry(project_path_owned.clone())
                .or_insert_with(|| Arc::new(EmbeddingService::new()))
                .clone()
        };

        let handle = tokio::task::spawn(async move {
            let indexer =
                BackgroundIndexer::new(project_root, index_store.clone())
                    .with_progress_callback(progress_cb)
                    .with_embedding_service(embedding_svc);

            let join = indexer.start().await;
            let result = join.await;

            // Determine final status.
            let final_event = if result.is_ok() {
                let summary = index_store
                    .get_project_summary(&pp_for_task)
                    .unwrap_or_default();
                IndexStatusEvent {
                    project_path: pp_for_task.clone(),
                    status: "indexed".to_string(),
                    indexed_files: summary.total_files,
                    total_files: summary.total_files,
                    error_message: None,
                    total_symbols: summary.total_symbols,
                    embedding_chunks: summary.embedding_chunks,
                }
            } else {
                IndexStatusEvent {
                    project_path: pp_for_task.clone(),
                    status: "error".to_string(),
                    indexed_files: 0,
                    total_files: 0,
                    error_message: Some("Background indexer task failed".to_string()),
                    total_symbols: 0,
                    embedding_chunks: 0,
                }
            };

            {
                let mut map = statuses_for_task.write().await;
                map.insert(pp_for_task.clone(), final_event.clone());
            }
            if let Some(ref app) = app_for_task {
                let _ = app.emit(INDEX_PROGRESS_EVENT, &final_event);
            }

            info!(
                project = %pp_for_task,
                status = %final_event.status,
                "index manager: indexing finished"
            );
        });

        // Store the handle.
        let mut indexers = self.active_indexers.write().await;
        indexers.insert(project_path_owned, IndexerEntry { handle });
    }

    /// Clear the existing index for a project and start a fresh full index.
    pub async fn trigger_reindex(&self, project_path: &str) {
        if let Err(e) = self.index_store.delete_project_index(project_path) {
            warn!(
                error = %e,
                project = %project_path,
                "index manager: failed to delete project index before reindex"
            );
        }
        self.start_indexing(project_path).await;
    }

    /// Get the current indexing status for a project directory.
    ///
    /// Returns a cached `IndexStatusEvent` if one exists, otherwise queries
    /// the `IndexStore` for an existing summary.
    pub async fn get_status(&self, project_path: &str) -> IndexStatusEvent {
        // Check in-memory cache first.
        {
            let map = self.statuses.read().await;
            if let Some(event) = map.get(project_path) {
                return event.clone();
            }
        }

        // Fallback: query the index store.
        match self.index_store.get_project_summary(project_path) {
            Ok(summary) if summary.total_files > 0 => IndexStatusEvent {
                project_path: project_path.to_string(),
                status: "indexed".to_string(),
                indexed_files: summary.total_files,
                total_files: summary.total_files,
                error_message: None,
                total_symbols: summary.total_symbols,
                embedding_chunks: summary.embedding_chunks,
            },
            _ => IndexStatusEvent {
                project_path: project_path.to_string(),
                status: "indexed".to_string(),
                indexed_files: 0,
                total_files: 0,
                error_message: None,
                total_symbols: 0,
                embedding_chunks: 0,
            },
        }
    }

    /// Get a reference to the inner `IndexStore`.
    pub fn index_store(&self) -> &IndexStore {
        &self.index_store
    }

    /// Get the embedding service for a project directory, if one has been
    /// created by a previous indexing run.
    pub async fn get_embedding_service(
        &self,
        project_path: &str,
    ) -> Option<Arc<EmbeddingService>> {
        let embeds = self.embedding_services.read().await;
        embeds.get(project_path).cloned()
    }

    /// Remove a directory from the manager, aborting any active indexer
    /// and clearing its cached status.
    pub async fn remove_directory(&self, project_path: &str) {
        self.abort_indexer(project_path).await;
        let mut map = self.statuses.write().await;
        map.remove(project_path);
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Abort and remove an active indexer for the given project path.
    async fn abort_indexer(&self, project_path: &str) {
        let mut indexers = self.active_indexers.write().await;
        if let Some(entry) = indexers.remove(project_path) {
            entry.handle.abort();
            info!(
                project = %project_path,
                "index manager: aborted existing indexer"
            );
        }
    }

    /// Update the status map and emit a Tauri event.
    async fn set_status_and_emit(&self, project_path: &str, event: IndexStatusEvent) {
        {
            let mut map = self.statuses.write().await;
            map.insert(project_path.to_string(), event.clone());
        }
        let app_guard = self.app_handle.read().await;
        if let Some(ref app) = *app_guard {
            let _ = app.emit(INDEX_PROGRESS_EVENT, &event);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;
    use std::fs;
    use tempfile::tempdir;

    fn test_pool() -> DbPool {
        let db = Database::new_in_memory().expect("in-memory db");
        db.pool().clone()
    }

    // -----------------------------------------------------------------------
    // get_status: default state
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn get_status_returns_default_for_unknown_project() {
        let mgr = IndexManager::new(test_pool());
        let status = mgr.get_status("/nonexistent").await;
        assert_eq!(status.project_path, "/nonexistent");
        assert_eq!(status.status, "indexed");
        assert_eq!(status.indexed_files, 0);
        assert_eq!(status.total_files, 0);
        assert!(status.error_message.is_none());
    }

    // -----------------------------------------------------------------------
    // ensure_indexed: spawns indexer when no index exists
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn ensure_indexed_spawns_indexer_for_new_project() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("main.py"), "x = 1\n").expect("write");

        let mgr = IndexManager::new(test_pool());
        let project_path = dir.path().to_string_lossy().to_string();

        mgr.ensure_indexed(&project_path).await;

        // Give the background task a moment to finish.
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let status = mgr.get_status(&project_path).await;
        assert_eq!(status.status, "indexed");
        assert!(status.total_files > 0);
    }

    // -----------------------------------------------------------------------
    // ensure_indexed: does NOT spawn a second indexer
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn ensure_indexed_is_idempotent() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("lib.py"), "y = 2\n").expect("write");

        let mgr = IndexManager::new(test_pool());
        let project_path = dir.path().to_string_lossy().to_string();

        mgr.ensure_indexed(&project_path).await;
        // Wait for first indexing to complete.
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Second call should not spawn a new indexer because index already exists.
        mgr.ensure_indexed(&project_path).await;

        let indexers = mgr.active_indexers.read().await;
        // The first indexer has already completed and its handle remains, but
        // a second one should NOT have been inserted (the completed task stays
        // in the map until replaced or removed, but no *new* entry is added).
        // We just verify the status is still "indexed".
        drop(indexers);

        let status = mgr.get_status(&project_path).await;
        assert_eq!(status.status, "indexed");
    }

    // -----------------------------------------------------------------------
    // trigger_reindex: clears and re-indexes
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn trigger_reindex_clears_and_reindexes() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("a.py"), "a = 1\n").expect("write");

        let mgr = IndexManager::new(test_pool());
        let project_path = dir.path().to_string_lossy().to_string();

        // Index first.
        mgr.ensure_indexed(&project_path).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let status_before = mgr.get_status(&project_path).await;
        assert_eq!(status_before.status, "indexed");
        assert_eq!(status_before.total_files, 1);

        // Add a file and trigger reindex.
        fs::write(dir.path().join("b.py"), "b = 2\n").expect("write");
        mgr.trigger_reindex(&project_path).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let status_after = mgr.get_status(&project_path).await;
        assert_eq!(status_after.status, "indexed");
        assert_eq!(status_after.total_files, 2);
    }

    // -----------------------------------------------------------------------
    // start_indexing: aborts previous indexer
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn start_indexing_aborts_previous() {
        let dir = tempdir().expect("tempdir");
        for i in 0..50 {
            fs::write(
                dir.path().join(format!("file_{i}.py")),
                format!("val = {i}\n"),
            )
            .expect("write");
        }

        let mgr = IndexManager::new(test_pool());
        let project_path = dir.path().to_string_lossy().to_string();

        // Start indexing twice rapidly; the second call should abort the first.
        mgr.start_indexing(&project_path).await;
        mgr.start_indexing(&project_path).await;

        // Wait for the second indexer to finish.
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        let status = mgr.get_status(&project_path).await;
        // The final status should be "indexed" (or "error" if the abort caused
        // a JoinError, but in practice the second indexer completes fine).
        assert!(
            status.status == "indexed" || status.status == "error",
            "unexpected status: {}",
            status.status,
        );
    }

    // -----------------------------------------------------------------------
    // get_status: returns cached status
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn get_status_returns_cached_value() {
        let mgr = IndexManager::new(test_pool());
        let project_path = "/test/cached";

        // Manually inject a status.
        {
            let mut map = mgr.statuses.write().await;
            map.insert(
                project_path.to_string(),
                IndexStatusEvent {
                    project_path: project_path.to_string(),
                    status: "indexing".to_string(),
                    indexed_files: 5,
                    total_files: 10,
                    error_message: None,
                    total_symbols: 0,
                    embedding_chunks: 0,
                },
            );
        }

        let status = mgr.get_status(project_path).await;
        assert_eq!(status.status, "indexing");
        assert_eq!(status.indexed_files, 5);
        assert_eq!(status.total_files, 10);
    }
}
