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
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::background_indexer::{BackgroundIndexer, BatchCompleteCallback, IndexProgressCallback};
use super::embedding_manager::{EmbeddingManager, EmbeddingManagerConfig};
use super::embedding_provider::{
    EmbeddingProviderConfig, EmbeddingProviderType, PersistedEmbeddingConfig,
    EMBEDDING_CONFIG_SETTING_KEY,
};
use super::embedding_provider_tfidf::TfIdfEmbeddingProvider;
use super::embedding_service::EmbeddingService;
use super::hnsw_index::HnswIndex;
use super::index_store::IndexStore;
use crate::commands::proxy::resolve_provider_proxy;
use crate::services::proxy::ProxyConfig;
use crate::storage::database::{Database, DbPool};
use crate::storage::KeyringService;

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
    /// Active embedding provider display name, `None` when not configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_provider_name: Option<String>,
    /// LSP enrichment state: "none", "enriching", or "enriched".
    #[serde(default = "default_lsp_enrichment_none")]
    pub lsp_enrichment: String,
}

fn default_lsp_enrichment_none() -> String {
    "none".into()
}

/// Internal bookkeeping for a running indexer.
struct IndexerEntry {
    handle: tokio::task::JoinHandle<()>,
    /// Send file-change notifications to the indexer's Phase 2 loop.
    change_tx: tokio::sync::mpsc::Sender<std::path::PathBuf>,
    /// Keeps the file-system watcher alive; dropped when the entry is removed.
    _watcher: Option<notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>>,
}

/// Manages per-directory `BackgroundIndexer` lifecycle.
///
/// Designed to live as long as the Tauri app, typically stored inside a
/// `tauri::State`.
pub struct IndexManager {
    index_store: Arc<IndexStore>,
    /// Database pool kept around for proxy resolution — constructing a
    /// lightweight `Database` wrapper on demand.
    db_pool: DbPool,
    active_indexers: RwLock<HashMap<String, IndexerEntry>>,
    statuses: Arc<RwLock<HashMap<String, IndexStatusEvent>>>,
    app_handle: RwLock<Option<AppHandle>>,
    /// Per-project embedding services, shared between BackgroundIndexer (which
    /// builds the vocabulary) and ToolExecutor (which queries with it).
    embedding_services: RwLock<HashMap<String, Arc<EmbeddingService>>>,
    /// Per-project `EmbeddingManager` instances that wrap an `EmbeddingService`
    /// inside a `TfIdfEmbeddingProvider`, providing the dispatch-layer API
    /// (caching, fallback, batching) on top of the same underlying service.
    embedding_managers: RwLock<HashMap<String, Arc<EmbeddingManager>>>,
    /// Per-project HNSW indexes for O(log n) approximate nearest neighbor search.
    /// The HNSW index is a derived cache of the SQLite embeddings (ADR-004).
    hnsw_indexes: RwLock<HashMap<String, Arc<HnswIndex>>>,
    /// Guard against concurrent `ensure_indexed` calls for the same project.
    /// Prevents duplicate indexer spawns from any caller.
    trigger_guard: tokio::sync::Mutex<HashSet<String>>,
}

impl IndexManager {
    /// Create a new `IndexManager` backed by the given database connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self {
            index_store: Arc::new(IndexStore::new(pool.clone())),
            db_pool: pool,
            active_indexers: RwLock::new(HashMap::new()),
            statuses: Arc::new(RwLock::new(HashMap::new())),
            app_handle: RwLock::new(None),
            embedding_services: RwLock::new(HashMap::new()),
            embedding_managers: RwLock::new(HashMap::new()),
            hnsw_indexes: RwLock::new(HashMap::new()),
            trigger_guard: tokio::sync::Mutex::new(HashSet::new()),
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
        // Dedup guard: prevent concurrent ensure_indexed calls for the same path.
        {
            let mut guard = self.trigger_guard.lock().await;
            if guard.contains(project_path) {
                return;
            }
            guard.insert(project_path.to_string());
        }

        // Quick check: is an indexer already running for this path?
        {
            let indexers = self.active_indexers.read().await;
            if indexers.contains_key(project_path) {
                self.trigger_guard.lock().await.remove(project_path);
                return;
            }
        }

        // Check the database for an existing index.
        match self.index_store.get_project_summary(project_path) {
            Ok(summary) if summary.total_files > 0 => {
                // Restore the TF-IDF vocabulary from SQLite so that semantic
                // search is available immediately without a full re-embed.
                let embedding_svc = {
                    let mut embeds = self.embedding_services.write().await;
                    embeds
                        .entry(project_path.to_string())
                        .or_insert_with(|| Arc::new(EmbeddingService::new()))
                        .clone()
                };
                if !embedding_svc.is_ready() {
                    match self.index_store.load_vocabulary(project_path) {
                        Ok(Some(json)) => match embedding_svc.import_vocabulary(&json) {
                            Ok(()) => {
                                info!(
                                    project = %project_path,
                                    "index manager: restored vocabulary from SQLite"
                                );
                            }
                            Err(e) => {
                                warn!(
                                    project = %project_path,
                                    error = %e,
                                    "index manager: failed to import vocabulary"
                                );
                            }
                        },
                        Ok(None) => {
                            // No vocabulary in DB — will be built on next embedding pass
                        }
                        Err(e) => {
                            warn!(
                                project = %project_path,
                                error = %e,
                                "index manager: failed to load vocabulary from SQLite"
                            );
                        }
                    }
                }

                // Create an EmbeddingManager from the persisted DB config.
                // If a cloud provider is configured, use it; otherwise fall
                // back to TF-IDF wrapping the shared EmbeddingService so the
                // vocabulary restore above is visible through the manager.
                {
                    let mut managers = self.embedding_managers.write().await;
                    managers.entry(project_path.to_string()).or_insert_with(|| {
                        let (mgr, _is_tfidf) =
                            self.build_embedding_manager_from_config(Arc::clone(&embedding_svc));
                        mgr
                    });
                }

                // Load or rebuild HNSW index for fast semantic search.
                // Infer dimension from the embedding manager if available,
                // otherwise pass 0 and let load_from_disk restore it from metadata.
                if summary.embedding_chunks > 0 {
                    let dim = {
                        let managers = self.embedding_managers.read().await;
                        managers
                            .get(project_path)
                            .map(|m| m.dimension())
                            .unwrap_or(0)
                    };
                    let _ = self.get_or_create_hnsw(project_path, dim).await;
                }

                let embedding_provider_name = {
                    let managers = self.embedding_managers.read().await;
                    managers
                        .get(project_path)
                        .map(|m| m.display_name().to_string())
                };
                let lsp_enrichment = if self.index_store.has_enrichment_data(project_path).unwrap_or(false) {
                    "enriched".to_string()
                } else {
                    "none".to_string()
                };
                let event = IndexStatusEvent {
                    project_path: project_path.to_string(),
                    status: "indexed".to_string(),
                    indexed_files: summary.total_files,
                    total_files: summary.total_files,
                    error_message: None,
                    total_symbols: summary.total_symbols,
                    embedding_chunks: summary.embedding_chunks,
                    embedding_provider_name,
                    lsp_enrichment,
                };
                self.set_status_and_emit(project_path, event).await;

                // Restore the incremental file watcher so that file changes
                // are picked up even after an app restart.
                self.start_incremental_watcher(project_path).await;
            }
            _ => {
                self.start_indexing(project_path).await;
            }
        }

        // Release the dedup guard
        self.trigger_guard.lock().await.remove(project_path);
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
            embedding_provider_name: None,
            lsp_enrichment: "none".to_string(),
        };
        self.set_status_and_emit(project_path, initial_event).await;

        // Build the progress callback.
        let pp_for_cb = project_path_owned.clone();
        let statuses_for_cb = statuses.clone();
        let app_for_cb = app_handle_opt.clone();
        let progress_cb: IndexProgressCallback = Arc::new(move |done, total| {
            // Preserve existing lsp_enrichment so indexing events don't
            // overwrite an in-progress or completed enrichment status.
            let prev_lsp = statuses_for_cb
                .try_read()
                .ok()
                .and_then(|map| map.get(&pp_for_cb).map(|e| e.lsp_enrichment.clone()))
                .unwrap_or_else(|| "none".to_string());
            let event = IndexStatusEvent {
                project_path: pp_for_cb.clone(),
                status: "indexing".to_string(),
                indexed_files: done,
                total_files: total,
                error_message: None,
                total_symbols: 0,
                embedding_chunks: 0,
                embedding_provider_name: None,
                lsp_enrichment: prev_lsp,
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

        // Build an EmbeddingManager from the persisted DB config.
        // Always replace the old manager so that a config change followed by
        // trigger_reindex takes effect immediately.
        let embedding_mgr = {
            let (mgr, _is_tfidf) =
                self.build_embedding_manager_from_config(Arc::clone(&embedding_svc));
            let mut managers = self.embedding_managers.write().await;
            managers.insert(project_path_owned.clone(), Arc::clone(&mgr));
            mgr
        };

        // Get or create HNSW index for this project (will try disk load first,
        // then rebuild from SQLite, or initialize empty).
        let dim = embedding_mgr.dimension();
        let hnsw_idx = self.get_or_create_hnsw(project_path, dim).await;

        // Create a channel + file-system watcher for Phase 2.
        let (change_tx, change_rx, watcher, overflow_flag) =
            Self::create_file_watcher(project_path);

        // Capture provider display name before embedding_mgr is moved into the indexer.
        let provider_display_name = embedding_mgr.display_name().to_string();

        let handle = tokio::task::spawn(async move {
            // Build batch callback for incremental status refresh.
            let pp_for_batch = pp_for_task.clone();
            let statuses_for_batch = statuses_for_task.clone();
            let app_for_batch = app_for_task.clone();
            let store_for_batch = index_store.clone();
            let provider_name_for_batch = provider_display_name.clone();
            let batch_cb: BatchCompleteCallback = Arc::new(move || {
                if let Ok(summary) = store_for_batch.get_project_summary(&pp_for_batch) {
                    let prev_lsp = statuses_for_batch
                        .try_read()
                        .ok()
                        .and_then(|map| map.get(&pp_for_batch).map(|e| e.lsp_enrichment.clone()))
                        .unwrap_or_else(|| "none".to_string());
                    let event = IndexStatusEvent {
                        project_path: pp_for_batch.clone(),
                        status: "indexed".to_string(),
                        indexed_files: summary.total_files,
                        total_files: summary.total_files,
                        error_message: None,
                        total_symbols: summary.total_symbols,
                        embedding_chunks: summary.embedding_chunks,
                        embedding_provider_name: Some(provider_name_for_batch.clone()),
                        lsp_enrichment: prev_lsp,
                    };
                    if let Ok(mut map) = statuses_for_batch.try_write() {
                        map.insert(pp_for_batch.clone(), event.clone());
                    }
                    if let Some(ref app) = app_for_batch {
                        let _ = app.emit(INDEX_PROGRESS_EVENT, &event);
                    }
                }
            });

            let indexer = BackgroundIndexer::new(project_root, index_store.clone())
                .with_progress_callback(progress_cb)
                .with_embedding_service(embedding_svc)
                .with_embedding_manager(embedding_mgr)
                .with_hnsw_index(hnsw_idx)
                .with_change_receiver(change_rx)
                .with_channel_overflow_flag(overflow_flag)
                .with_batch_callback(batch_cb);

            let join = indexer.start().await;
            let result = join.await;

            // Determine final status.
            // Preserve lsp_enrichment from cached status so indexing completion
            // does not overwrite an in-progress or completed enrichment state.
            let prev_lsp = statuses_for_task
                .read()
                .await
                .get(&pp_for_task)
                .map(|e| e.lsp_enrichment.clone())
                .unwrap_or_else(|| "none".to_string());

            let final_event = match result {
                Ok(embedding_stats) => {
                    let summary = index_store
                        .get_project_summary(&pp_for_task)
                        .unwrap_or_default();

                    // Distinguish "indexed" (has embeddings) from
                    // "indexed_no_embedding" (file index OK but embedding failed).
                    let (status, error_msg) = if summary.total_files == 0 {
                        (
                            "error".to_string(),
                            Some("No files were indexed".to_string()),
                        )
                    } else if summary.embedding_chunks == 0 {
                        let err = embedding_stats.as_ref().and_then(|s| {
                            if s.has_failures() {
                                Some(format!(
                                    "Embedding failed for {}/{} files",
                                    s.failed_files, s.total_files
                                ))
                            } else {
                                None
                            }
                        });
                        ("indexed_no_embedding".to_string(), err)
                    } else {
                        let err = embedding_stats.as_ref().and_then(|s| {
                            if s.has_failures() {
                                Some(format!(
                                    "Embedding partially failed ({}/{} files failed)",
                                    s.failed_files, s.total_files
                                ))
                            } else {
                                None
                            }
                        });
                        ("indexed".to_string(), err)
                    };

                    IndexStatusEvent {
                        project_path: pp_for_task.clone(),
                        status,
                        indexed_files: summary.total_files,
                        total_files: summary.total_files,
                        error_message: error_msg,
                        total_symbols: summary.total_symbols,
                        embedding_chunks: summary.embedding_chunks,
                        embedding_provider_name: Some(provider_display_name.clone()),
                        lsp_enrichment: prev_lsp.clone(),
                    }
                }
                Err(_) => IndexStatusEvent {
                    project_path: pp_for_task.clone(),
                    status: "error".to_string(),
                    indexed_files: 0,
                    total_files: 0,
                    error_message: Some("Background indexer task failed".to_string()),
                    total_symbols: 0,
                    embedding_chunks: 0,
                    embedding_provider_name: None,
                    lsp_enrichment: "none".to_string(),
                },
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

        // Store the handle, change sender, and watcher.
        let mut indexers = self.active_indexers.write().await;
        indexers.insert(
            project_path_owned,
            IndexerEntry {
                handle,
                change_tx,
                _watcher: watcher,
            },
        );
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
        // Clear existing HNSW index and delete disk files so a fresh one is created.
        // This prevents hnsw_rs from attempting to load potentially corrupt/stale
        // HNSW files from a previous indexing run.
        {
            let mut indexes = self.hnsw_indexes.write().await;
            indexes.remove(project_path);
        }
        // Delete HNSW disk files for this project
        let hnsw_dir = Self::hnsw_index_dir(project_path);
        if hnsw_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&hnsw_dir) {
                warn!(
                    error = %e,
                    dir = %hnsw_dir.display(),
                    "index manager: failed to delete HNSW disk files during reindex"
                );
            }
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
            Ok(summary) if summary.total_files > 0 => {
                let embedding_provider_name = {
                    let managers = self.embedding_managers.read().await;
                    managers
                        .get(project_path)
                        .map(|m| m.display_name().to_string())
                };
                let lsp_enrichment = if self.index_store.has_enrichment_data(project_path).unwrap_or(false) {
                    "enriched".to_string()
                } else {
                    "none".to_string()
                };
                IndexStatusEvent {
                    project_path: project_path.to_string(),
                    status: "indexed".to_string(),
                    indexed_files: summary.total_files,
                    total_files: summary.total_files,
                    error_message: None,
                    total_symbols: summary.total_symbols,
                    embedding_chunks: summary.embedding_chunks,
                    embedding_provider_name,
                    lsp_enrichment,
                }
            }
            _ => IndexStatusEvent {
                project_path: project_path.to_string(),
                status: "idle".to_string(),
                indexed_files: 0,
                total_files: 0,
                error_message: None,
                total_symbols: 0,
                embedding_chunks: 0,
                embedding_provider_name: None,
                lsp_enrichment: "none".to_string(),
            },
        }
    }

    /// Update the LSP enrichment status for a project and emit the event.
    ///
    /// Called by the `trigger_lsp_enrichment` command to keep the frontend
    /// status badge in sync with the enrichment lifecycle.
    pub async fn set_lsp_enrichment_status(&self, project_path: &str, state: &str) {
        let mut event = self.get_status(project_path).await;
        event.lsp_enrichment = state.to_string();
        self.set_status_and_emit(project_path, event).await;
    }

    /// Push a file-change notification to the running indexer for a project.
    ///
    /// This is intended for external callers (e.g. `FileWatcherService`) that
    /// already have their own file-change events and want to feed them into the
    /// incremental indexing pipeline.  If no indexer is running for the project,
    /// or the channel is full, the notification is silently dropped.
    pub async fn notify_file_changed(&self, project_path: &str, file_path: PathBuf) {
        let indexers = self.active_indexers.read().await;
        if let Some(entry) = indexers.get(project_path) {
            if let Err(e) = entry.change_tx.try_send(file_path) {
                debug!(
                    error = %e,
                    project = %project_path,
                    "index manager: could not send file change notification"
                );
            }
        }
    }

    /// Get a reference to the inner `IndexStore`.
    pub fn index_store(&self) -> &IndexStore {
        &self.index_store
    }

    /// Get the embedding service for a project directory, if one has been
    /// created by a previous indexing run.
    pub async fn get_embedding_service(&self, project_path: &str) -> Option<Arc<EmbeddingService>> {
        let embeds = self.embedding_services.read().await;
        embeds.get(project_path).cloned()
    }

    /// Get the `EmbeddingManager` for a project directory, if one has been
    /// created by `ensure_indexed` or `start_indexing`.
    ///
    /// The returned manager wraps the same `EmbeddingService` (via
    /// `TfIdfEmbeddingProvider`) and provides the dispatch-layer API with
    /// caching, batching, and optional fallback support.
    pub async fn get_embedding_manager(&self, project_path: &str) -> Option<Arc<EmbeddingManager>> {
        let managers = self.embedding_managers.read().await;
        managers.get(project_path).cloned()
    }

    /// Get the HNSW index for a project directory, if one has been created.
    pub async fn get_hnsw_index(&self, project_path: &str) -> Option<Arc<HnswIndex>> {
        let indexes = self.hnsw_indexes.read().await;
        indexes.get(project_path).cloned()
    }

    /// Remove a directory from the manager, aborting any active indexer
    /// and clearing its cached status.
    pub async fn remove_directory(&self, project_path: &str) {
        self.abort_indexer(project_path).await;
        {
            let mut map = self.statuses.write().await;
            map.remove(project_path);
        }
        {
            let mut services = self.embedding_services.write().await;
            services.remove(project_path);
        }
        {
            let mut managers = self.embedding_managers.write().await;
            managers.remove(project_path);
        }
        {
            let mut indexes = self.hnsw_indexes.write().await;
            indexes.remove(project_path);
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Create a debounced file watcher + mpsc channel for a project directory.
    ///
    /// Returns `(sender, receiver, optional_debouncer, overflow_flag)`.
    /// The sender is kept alongside the `IndexerEntry` so that external
    /// callers can also push file-change notifications.  The overflow flag
    /// is set to `true` whenever `try_send` fails (channel full) so the
    /// incremental loop can trigger a catch-up sync.
    fn create_file_watcher(
        project_path: &str,
    ) -> (
        tokio::sync::mpsc::Sender<PathBuf>,
        tokio::sync::mpsc::Receiver<PathBuf>,
        Option<notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>>,
        Arc<AtomicBool>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::channel::<PathBuf>(4096);
        let overflow_flag = Arc::new(AtomicBool::new(false));

        let watcher = {
            let tx_clone = tx.clone();
            let overflow = Arc::clone(&overflow_flag);
            let root = PathBuf::from(project_path);
            match notify_debouncer_mini::new_debouncer(
                Duration::from_millis(200),
                move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                    if let Ok(events) = events {
                        for event in events {
                            // Do NOT filter by is_file() here: deletion events
                            // produce paths that no longer exist on disk, so
                            // is_file() would be false.  The incremental
                            // indexer handles non-file paths gracefully.
                            if let Err(e) = tx_clone.try_send(event.path) {
                                // Set the overflow flag so the incremental loop
                                // can trigger a catch-up sync later.
                                overflow.store(true, std::sync::atomic::Ordering::Release);
                                tracing::warn!(
                                    path = %e.into_inner().display(),
                                    "index watcher: channel full, event dropped — catch-up sync will reconcile"
                                );
                            }
                        }
                    }
                },
            ) {
                Ok(mut debouncer) => {
                    if let Err(e) = debouncer
                        .watcher()
                        .watch(&root, notify::RecursiveMode::Recursive)
                    {
                        warn!(
                            error = %e,
                            project = %project_path,
                            "index manager: failed to watch directory for changes"
                        );
                    }
                    Some(debouncer)
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        project = %project_path,
                        "index manager: failed to create file watcher"
                    );
                    None
                }
            }
        };

        (tx, rx, watcher, overflow_flag)
    }

    /// Start an incremental-only file watcher for a project that already has
    /// an index.  Skips Phase 1 (full index) and Phase 1b (embedding).
    ///
    /// Called from `ensure_indexed` when the index already exists.
    async fn start_incremental_watcher(&self, project_path: &str) {
        // Guard: if an indexer is already running for this path, skip.
        {
            let indexers = self.active_indexers.read().await;
            if indexers.contains_key(project_path) {
                return;
            }
        }

        let (change_tx, change_rx, watcher, overflow_flag) =
            Self::create_file_watcher(project_path);

        let project_root = PathBuf::from(project_path);
        let index_store = self.index_store.clone();

        // Get embedding-related services (already restored by ensure_indexed).
        let embedding_svc = {
            let embeds = self.embedding_services.read().await;
            embeds.get(project_path).cloned()
        };
        let embedding_mgr = {
            let managers = self.embedding_managers.read().await;
            managers.get(project_path).cloned()
        };
        let hnsw_idx = {
            let indexes = self.hnsw_indexes.read().await;
            indexes.get(project_path).cloned()
        };

        // Build batch callback for incremental status refresh.
        let pp_for_batch = project_path.to_string();
        let statuses_for_batch = self.statuses.clone();
        let app_for_batch = {
            let guard = self.app_handle.read().await;
            guard.clone()
        };
        let store_for_batch = self.index_store.clone();
        let provider_name_for_batch = {
            let managers = self.embedding_managers.read().await;
            managers
                .get(project_path)
                .map(|m| m.display_name().to_string())
                .unwrap_or_default()
        };
        let batch_cb: BatchCompleteCallback = Arc::new(move || {
            if let Ok(summary) = store_for_batch.get_project_summary(&pp_for_batch) {
                let prev_lsp = statuses_for_batch
                    .try_read()
                    .ok()
                    .and_then(|map| map.get(&pp_for_batch).map(|e| e.lsp_enrichment.clone()))
                    .unwrap_or_else(|| "none".to_string());
                let event = IndexStatusEvent {
                    project_path: pp_for_batch.clone(),
                    status: "indexed".to_string(),
                    indexed_files: summary.total_files,
                    total_files: summary.total_files,
                    error_message: None,
                    total_symbols: summary.total_symbols,
                    embedding_chunks: summary.embedding_chunks,
                    embedding_provider_name: Some(provider_name_for_batch.clone()),
                    lsp_enrichment: prev_lsp,
                };
                if let Ok(mut map) = statuses_for_batch.try_write() {
                    map.insert(pp_for_batch.clone(), event.clone());
                }
                if let Some(ref app) = app_for_batch {
                    let _ = app.emit(INDEX_PROGRESS_EVENT, &event);
                }
            }
        });

        let mut indexer = BackgroundIndexer::new(project_root, index_store)
            .with_change_receiver(change_rx)
            .with_channel_overflow_flag(overflow_flag)
            .with_batch_callback(batch_cb);
        if let Some(svc) = embedding_svc {
            indexer = indexer.with_embedding_service(svc);
        }
        if let Some(mgr) = embedding_mgr {
            indexer = indexer.with_embedding_manager(mgr);
        }
        if let Some(idx) = hnsw_idx {
            indexer = indexer.with_hnsw_index(idx);
        }

        let handle = indexer.start_watch_with_catchup().await;

        let mut indexers = self.active_indexers.write().await;
        indexers.insert(
            project_path.to_string(),
            IndexerEntry {
                handle,
                change_tx,
                _watcher: watcher,
            },
        );

        info!(
            project = %project_path,
            "index manager: started incremental watcher for existing index"
        );
    }

    /// Load the persisted embedding configuration from the `settings` table.
    ///
    /// Returns `None` if no config is stored or if deserialization fails.
    fn load_persisted_embedding_config(&self) -> Option<PersistedEmbeddingConfig> {
        match self.index_store.get_setting(EMBEDDING_CONFIG_SETTING_KEY) {
            Ok(Some(json)) => match serde_json::from_str::<PersistedEmbeddingConfig>(&json) {
                Ok(config) => Some(config),
                Err(e) => {
                    warn!(
                        error = %e,
                        "index manager: failed to parse persisted embedding config"
                    );
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                warn!(
                    error = %e,
                    "index manager: failed to read embedding config from DB"
                );
                None
            }
        }
    }

    /// Build an `EmbeddingManager` based on the persisted DB configuration.
    ///
    /// If the DB has a cloud provider configured (Qwen, GLM, OpenAI, Ollama),
    /// this will read the API key from the OS keyring and build the
    /// corresponding provider.  On any failure it falls back to TF-IDF.
    ///
    /// Returns `(manager, is_tfidf)`.
    fn build_embedding_manager_from_config(
        &self,
        embedding_svc: Arc<EmbeddingService>,
    ) -> (Arc<EmbeddingManager>, bool) {
        let persisted = self.load_persisted_embedding_config();

        let persisted = match persisted {
            Some(c) if c.provider != EmbeddingProviderType::TfIdf => c,
            _ => {
                // No config or TF-IDF configured → default path
                return self.build_tfidf_manager(embedding_svc);
            }
        };

        // Resolve API key from the OS keyring for cloud providers.
        let api_key: Option<String> = match Self::embedding_keyring_alias(persisted.provider) {
            Some(alias) => match KeyringService::new().get_api_key(alias) {
                Ok(Some(key)) if !key.is_empty() => Some(key),
                Ok(_) => {
                    warn!(
                        provider = ?persisted.provider,
                        "index manager: API key is empty or missing, falling back to TF-IDF"
                    );
                    return self.build_tfidf_manager(embedding_svc);
                }
                Err(e) => {
                    warn!(
                        provider = ?persisted.provider,
                        error = %e,
                        "index manager: failed to get API key, falling back to TF-IDF"
                    );
                    return self.build_tfidf_manager(embedding_svc);
                }
            },
            None => None, // Local providers (Ollama) don't need keys
        };

        // Build primary provider config.
        let mut primary_config = EmbeddingProviderConfig::new(persisted.provider);
        primary_config.model = persisted.model.clone();
        primary_config.api_key = api_key;
        primary_config.base_url = persisted.base_url.clone();
        primary_config.dimension = persisted.dimension;
        primary_config.batch_size = persisted.batch_size;
        // Resolve proxy configuration from the database settings.
        primary_config.proxy = self.resolve_embedding_proxy(persisted.provider);

        // Build optional fallback config (TF-IDF fallback is common).
        let fallback_config = persisted
            .fallback_provider
            .map(|fb_type| EmbeddingProviderConfig::new(fb_type));

        let manager_config = EmbeddingManagerConfig {
            primary: primary_config,
            fallback: fallback_config,
            cache_enabled: true,
            cache_max_entries: 10_000,
        };

        match EmbeddingManager::from_config(manager_config) {
            Ok(mgr) => {
                info!(
                    provider = ?persisted.provider,
                    model = %persisted.model,
                    "index manager: using cloud embedding provider from DB config"
                );
                (Arc::new(mgr), false)
            }
            Err(e) => {
                warn!(
                    provider = ?persisted.provider,
                    error = %e,
                    "index manager: failed to create cloud provider, falling back to TF-IDF"
                );
                self.build_tfidf_manager(embedding_svc)
            }
        }
    }

    /// Build a TF-IDF–based `EmbeddingManager` from the given service.
    fn build_tfidf_manager(
        &self,
        embedding_svc: Arc<EmbeddingService>,
    ) -> (Arc<EmbeddingManager>, bool) {
        let provider = TfIdfEmbeddingProvider::new(Arc::clone(&embedding_svc));
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: true,
            cache_max_entries: 10_000,
        };
        (
            Arc::new(EmbeddingManager::new(Box::new(provider), None, config)),
            true,
        )
    }

    /// Resolve the proxy configuration for a given embedding provider type.
    ///
    /// Reads the per-provider proxy strategy from the database and resolves
    /// it against the global proxy config (if any).
    fn resolve_embedding_proxy(&self, provider: EmbeddingProviderType) -> Option<ProxyConfig> {
        let alias = match provider {
            EmbeddingProviderType::OpenAI => "embedding_openai",
            EmbeddingProviderType::Qwen => "embedding_qwen",
            EmbeddingProviderType::Glm => "embedding_glm",
            EmbeddingProviderType::Ollama => "embedding_ollama",
            EmbeddingProviderType::TfIdf => return None,
        };
        let keyring = KeyringService::new();
        let db = Database::from_pool(self.db_pool.clone());
        resolve_provider_proxy(&keyring, &db, alias)
    }

    /// Return the keyring alias for a given embedding provider type.
    fn embedding_keyring_alias(provider: EmbeddingProviderType) -> Option<&'static str> {
        match provider {
            EmbeddingProviderType::Qwen => Some("qwen_embedding"),
            EmbeddingProviderType::Glm => Some("glm_embedding"),
            EmbeddingProviderType::OpenAI => Some("openai_embedding"),
            EmbeddingProviderType::TfIdf | EmbeddingProviderType::Ollama => None,
        }
    }

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

    /// Compute a project hash (SHA-256 truncated to 16 hex chars) for HNSW
    /// index directory naming.
    fn project_hash(project_path: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(project_path.as_bytes());
        let result = format!("{:x}", hasher.finalize());
        result[..16].to_string()
    }

    /// Get the HNSW index directory for a project.
    /// Stored under `~/.plan-cascade/hnsw_indexes/<project-hash>/`.
    fn hnsw_index_dir(project_path: &str) -> std::path::PathBuf {
        let hash = Self::project_hash(project_path);
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".plan-cascade")
            .join("hnsw_indexes")
            .join(hash)
    }

    /// Get or create an HNSW index for a project, attempting to load from disk first.
    ///
    /// If no disk files exist and embeddings are present in SQLite, rebuilds
    /// the HNSW index from the stored embeddings.
    async fn get_or_create_hnsw(&self, project_path: &str, dimension: usize) -> Arc<HnswIndex> {
        // Check if we already have one
        {
            let indexes = self.hnsw_indexes.read().await;
            if let Some(idx) = indexes.get(project_path) {
                return Arc::clone(idx);
            }
        }

        let index_dir = Self::hnsw_index_dir(project_path);
        let hnsw = Arc::new(HnswIndex::new(&index_dir, dimension));

        // Try loading from disk
        if hnsw.load_from_disk().await {
            info!(
                project = %project_path,
                "index manager: HNSW loaded from disk"
            );
        } else {
            // Try rebuilding from SQLite embeddings
            match self.rebuild_hnsw_from_store(project_path, &hnsw).await {
                Ok(count) if count > 0 => {
                    info!(
                        project = %project_path,
                        vectors = count,
                        "index manager: HNSW rebuilt from SQLite"
                    );
                    // Save the rebuilt index to disk
                    if let Err(e) = hnsw.save_to_disk().await {
                        warn!(
                            project = %project_path,
                            error = %e,
                            "index manager: failed to save rebuilt HNSW to disk"
                        );
                    }
                }
                Ok(_) => {
                    // No embeddings yet — initialize empty
                    hnsw.initialize().await;
                    info!(
                        project = %project_path,
                        "index manager: HNSW initialized empty (no embeddings yet)"
                    );
                }
                Err(e) => {
                    warn!(
                        project = %project_path,
                        error = %e,
                        "index manager: HNSW rebuild from SQLite failed, initializing empty"
                    );
                    hnsw.initialize().await;
                }
            }
        }

        let mut indexes = self.hnsw_indexes.write().await;
        indexes.insert(project_path.to_string(), Arc::clone(&hnsw));
        hnsw
    }

    /// Rebuild HNSW index from all embeddings stored in SQLite.
    ///
    /// Returns the number of vectors inserted.
    async fn rebuild_hnsw_from_store(
        &self,
        project_path: &str,
        hnsw: &HnswIndex,
    ) -> Result<usize, String> {
        let vectors = self
            .index_store
            .get_all_embedding_ids_and_vectors(project_path)
            .map_err(|e| format!("failed to get embeddings: {}", e))?;

        if vectors.is_empty() {
            return Ok(0);
        }

        // Infer actual dimension from the first vector.
        // Mixed dimensions can occur when the embedding provider changes —
        // rebuild_from_vectors filters mismatched vectors internally.
        let actual_dim = vectors[0].1.len();
        let mismatched_count = vectors
            .iter()
            .filter(|(_, v)| v.len() != actual_dim)
            .count();
        if mismatched_count > 0 {
            warn!(
                expected_dim = actual_dim,
                mismatched = mismatched_count,
                total = vectors.len(),
                "index manager: found embeddings with mismatched dimensions, \
                 they will be filtered during HNSW rebuild"
            );
        }

        hnsw.set_dimension(actual_dim);

        // Atomically rebuild — concurrent searches see old or new, never empty
        hnsw.rebuild_from_vectors(&vectors).await?;

        Ok(vectors.len() - mismatched_count)
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
    use crate::services::orchestrator::index_store::IndexStore;
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
        assert_eq!(status.status, "idle");
        assert_eq!(status.indexed_files, 0);
        assert_eq!(status.total_files, 0);
        assert!(status.error_message.is_none());
    }

    // -----------------------------------------------------------------------
    // ensure_indexed: spawns indexer when no index exists
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // ensure_indexed restores vocabulary from SQLite (story-004)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn ensure_indexed_loads_vocabulary_from_sqlite() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("app.py"), "def run():\n    pass\n").expect("write");

        let pool = test_pool();
        let mgr = IndexManager::new(pool.clone());
        let project_path = dir.path().to_string_lossy().to_string();

        // First: index the project (this creates the index + embedding)
        mgr.ensure_indexed(&project_path).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // Verify the embedding service is ready after indexing
        let emb = mgr.get_embedding_service(&project_path).await;
        assert!(
            emb.is_some(),
            "should have embedding service after indexing"
        );
        // The embedding service should be ready (vocab built during embedding pass)
        // Note: it may or may not be ready depending on how fast the background
        // indexer ran. The key test is the second call below.

        // Save a vocab manually (simulating what the embedding pass does)
        let vocab_json =
            r#"{"token_to_idx":{"def":0,"run":1,"pass":2},"idf":[1.0,1.0,1.0],"num_docs":1}"#;
        let store = IndexStore::new(pool.clone());
        store.save_vocabulary(&project_path, vocab_json).unwrap();

        // Create a fresh IndexManager (simulating app restart)
        let mgr2 = IndexManager::new(pool);
        mgr2.ensure_indexed(&project_path).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // The embedding service should have restored the vocabulary
        let emb2 = mgr2.get_embedding_service(&project_path).await;
        assert!(
            emb2.is_some(),
            "should have embedding service after restore"
        );
        assert!(
            emb2.unwrap().is_ready(),
            "embedding service should be ready after vocab restore from SQLite"
        );
    }

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
                    embedding_provider_name: None,
                    lsp_enrichment: "none".to_string(),
                },
            );
        }

        let status = mgr.get_status(project_path).await;
        assert_eq!(status.status, "indexing");
        assert_eq!(status.indexed_files, 5);
        assert_eq!(status.total_files, 10);
    }

    // -----------------------------------------------------------------------
    // story-006: EmbeddingManager lifecycle integration
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn get_embedding_manager_returns_none_for_unknown_project() {
        let mgr = IndexManager::new(test_pool());
        let result = mgr.get_embedding_manager("/nonexistent").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn ensure_indexed_creates_embedding_manager() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("main.py"), "x = 1\n").expect("write");

        let mgr = IndexManager::new(test_pool());
        let project_path = dir.path().to_string_lossy().to_string();

        mgr.ensure_indexed(&project_path).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // After indexing, both embedding service and embedding manager should exist.
        let emb_svc = mgr.get_embedding_service(&project_path).await;
        assert!(
            emb_svc.is_some(),
            "should have embedding service after indexing"
        );

        let emb_mgr = mgr.get_embedding_manager(&project_path).await;
        assert!(
            emb_mgr.is_some(),
            "should have embedding manager after indexing"
        );
    }

    #[tokio::test]
    async fn start_indexing_creates_embedding_manager() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("test.py"), "y = 2\n").expect("write");

        let mgr = IndexManager::new(test_pool());
        let project_path = dir.path().to_string_lossy().to_string();

        mgr.start_indexing(&project_path).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let emb_mgr = mgr.get_embedding_manager(&project_path).await;
        assert!(
            emb_mgr.is_some(),
            "should have embedding manager after start_indexing"
        );

        // Verify the manager's primary provider is TF-IDF.
        let mgr_ref = emb_mgr.unwrap();
        assert_eq!(
            mgr_ref.provider_type(),
            EmbeddingProviderType::TfIdf,
            "manager primary provider should be TF-IDF"
        );
    }

    #[tokio::test]
    async fn ensure_indexed_restores_vocabulary_visible_through_manager() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("app.py"), "def run():\n    pass\n").expect("write");

        let pool = test_pool();
        let mgr = IndexManager::new(pool.clone());
        let project_path = dir.path().to_string_lossy().to_string();

        // First: index the project
        mgr.ensure_indexed(&project_path).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // Save a vocab manually (simulating what the embedding pass does)
        let vocab_json =
            r#"{"token_to_idx":{"def":0,"run":1,"pass":2},"idf":[1.0,1.0,1.0],"num_docs":1}"#;
        let store = IndexStore::new(pool.clone());
        store.save_vocabulary(&project_path, vocab_json).unwrap();

        // Create a fresh IndexManager (simulating app restart)
        let mgr2 = IndexManager::new(pool);
        mgr2.ensure_indexed(&project_path).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // The embedding manager should exist and its TF-IDF provider should
        // be ready (since the vocabulary was restored through the shared
        // EmbeddingService).
        let emb_mgr = mgr2.get_embedding_manager(&project_path).await;
        assert!(
            emb_mgr.is_some(),
            "should have embedding manager after restore"
        );

        // Also verify the raw embedding service is ready
        let emb_svc = mgr2.get_embedding_service(&project_path).await;
        assert!(
            emb_svc.unwrap().is_ready(),
            "embedding service should be ready after vocab restore"
        );
    }

    #[tokio::test]
    async fn trigger_reindex_preserves_embedding_manager() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("a.py"), "a = 1\n").expect("write");

        let mgr = IndexManager::new(test_pool());
        let project_path = dir.path().to_string_lossy().to_string();

        // Index first.
        mgr.ensure_indexed(&project_path).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let mgr_before = mgr.get_embedding_manager(&project_path).await;
        assert!(mgr_before.is_some(), "should have manager after indexing");

        // Add a file and trigger reindex.
        fs::write(dir.path().join("b.py"), "b = 2\n").expect("write");
        mgr.trigger_reindex(&project_path).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Manager should still exist after reindex (reused via or_insert_with).
        let mgr_after = mgr.get_embedding_manager(&project_path).await;
        assert!(
            mgr_after.is_some(),
            "should have manager after trigger_reindex"
        );
    }
}
