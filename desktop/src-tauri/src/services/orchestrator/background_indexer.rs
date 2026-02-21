//! Background Indexing with File Watcher Integration
//!
//! Runs file indexing in a background tokio task so that the main execution
//! thread is never blocked.  On start a full inventory is built, and afterwards
//! the indexer listens on an optional `mpsc` channel for incremental updates
//! triggered by file-watcher events.

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::analysis_index::{
    build_file_inventory, detect_component, extract_symbols_from_str, is_test_path, AnalysisLimits,
};
use super::embedding_manager::EmbeddingManager;
use super::embedding_provider::EmbeddingProviderType;
use super::embedding_provider_tfidf::TfIdfEmbeddingProvider;
use super::embedding_service::{embedding_to_bytes, EmbeddingService};
use super::hnsw_index::HnswIndex;
use super::index_store::IndexStore;
use super::tree_sitter_parser;

/// Callback type for reporting indexing progress.
///
/// Called with `(indexed_so_far, total_files)` during a full index pass.
pub type IndexProgressCallback = Arc<dyn Fn(usize, usize) + Send + Sync>;

/// Statistics from a managed embedding pass.
///
/// Used by `IndexManager` to determine the final status (e.g.
/// `"indexed"` vs `"indexed_no_embedding"` vs `"error"`).
#[derive(Debug, Default)]
pub struct EmbeddingPassStats {
    /// Number of embedding chunks successfully stored in SQLite.
    pub stored_chunks: usize,
    /// Number of files where embedding failed (skipped, still counted).
    pub failed_files: usize,
    /// Total number of files that were considered for embedding.
    pub total_files: usize,
}

impl EmbeddingPassStats {
    /// Returns `true` if at least one file failed during the embedding pass.
    pub fn has_failures(&self) -> bool {
        self.failed_files > 0
    }
}

/// Result of an incremental index operation for a single file-change event.
enum IncrementalResult {
    /// The file was re-indexed because its content hash changed.
    /// Carries the already-read content, detected language, and normalized
    /// relative path so that downstream embedding can reuse them without
    /// additional disk reads.
    Updated {
        content: String,
        language: String,
        rel_path: String,
    },
    /// The file was deleted from disk and its index entry was cleaned up.
    Deleted { rel_path: String },
    /// A directory was deleted; all child file indexes were cleaned up.
    DirectoryDeleted { child_rel_paths: Vec<String> },
    /// No action was needed (file unchanged or event was irrelevant).
    Skipped,
}

/// Background file indexer that populates and maintains the SQLite index.
///
/// Usage:
/// ```ignore
/// let indexer = BackgroundIndexer::new(project_root, index_store)
///     .with_change_receiver(rx)
///     .with_progress_callback(Arc::new(|done, total| println!("{done}/{total}")));
/// let handle = indexer.start().await;
/// ```
pub struct BackgroundIndexer {
    project_root: PathBuf,
    index_store: Arc<IndexStore>,
    embedding_service: Option<Arc<EmbeddingService>>,
    embedding_manager: Option<Arc<EmbeddingManager>>,
    hnsw_index: Option<Arc<HnswIndex>>,
    change_rx: Option<tokio::sync::mpsc::Receiver<PathBuf>>,
    progress_callback: Option<IndexProgressCallback>,
    /// Shared flag set by the file watcher when the mpsc channel overflows.
    /// The incremental loop checks this after each batch and triggers a
    /// catch-up sync when set.
    channel_overflow: Option<Arc<AtomicBool>>,
}

impl BackgroundIndexer {
    /// Create a new background indexer for the given project root.
    pub fn new(project_root: PathBuf, index_store: Arc<IndexStore>) -> Self {
        Self {
            project_root,
            index_store,
            embedding_service: None,
            embedding_manager: None,
            hnsw_index: None,
            change_rx: None,
            progress_callback: None,
            channel_overflow: None,
        }
    }

    /// Attach an embedding service for generating vector embeddings after indexing.
    ///
    /// When set, the indexer will chunk file contents and generate TF-IDF
    /// embeddings stored in the `file_embeddings` table.
    pub fn with_embedding_service(mut self, svc: Arc<EmbeddingService>) -> Self {
        self.embedding_service = Some(svc);
        self
    }

    /// Attach an `EmbeddingManager` for provider-aware embedding with
    /// automatic fallback and caching.
    ///
    /// When set, the indexer will use the manager instead of the direct
    /// `EmbeddingService` for both full and incremental embedding passes.
    /// If both `embedding_manager` and `embedding_service` are set, the
    /// manager takes precedence.
    pub fn with_embedding_manager(mut self, mgr: Arc<EmbeddingManager>) -> Self {
        self.embedding_manager = Some(mgr);
        self
    }

    /// Attach an HNSW index for O(log n) approximate nearest neighbor search.
    ///
    /// When set, the indexer will insert embeddings into the HNSW index
    /// during the embedding pass and save the index to disk afterwards.
    pub fn with_hnsw_index(mut self, idx: Arc<HnswIndex>) -> Self {
        self.hnsw_index = Some(idx);
        self
    }

    /// Attach a channel receiver for incremental file-change notifications.
    ///
    /// Each received `PathBuf` is an absolute path to a file that was
    /// modified (or created).  The indexer will re-index that file only if
    /// its content hash has changed since the last index.
    pub fn with_change_receiver(mut self, rx: tokio::sync::mpsc::Receiver<PathBuf>) -> Self {
        self.change_rx = Some(rx);
        self
    }

    /// Attach an optional progress callback that will be invoked during full indexing.
    ///
    /// The callback receives `(indexed_so_far, total_files)` and is called every
    /// 10 files, plus once at completion with `(total, total)`.
    pub fn with_progress_callback(mut self, cb: IndexProgressCallback) -> Self {
        self.progress_callback = Some(cb);
        self
    }

    /// Attach a shared overflow flag from the file watcher channel.
    ///
    /// When the watcher's `try_send` fails (channel full), it sets this flag
    /// to `true`.  The incremental loop checks the flag after each batch and
    /// triggers a lightweight catch-up sync to reconcile missed events.
    pub fn with_channel_overflow_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.channel_overflow = Some(flag);
        self
    }

    /// Spawn the background indexing task and return its `JoinHandle`.
    ///
    /// The task:
    /// 1. Performs a full index of the project on start.
    /// 2. Enters a loop listening for incremental change events (if a
    ///    receiver was provided).
    ///
    /// Errors during indexing are logged but never propagated; the task
    /// keeps running so that future change events are still processed.
    pub async fn start(self) -> tokio::task::JoinHandle<Option<EmbeddingPassStats>> {
        let project_root = self.project_root;
        let index_store = self.index_store;
        let embedding_service = self.embedding_service;
        let embedding_manager = self.embedding_manager;
        let hnsw_index: Option<Arc<HnswIndex>> = self.hnsw_index;
        let change_rx = self.change_rx;
        let progress_callback = self.progress_callback;
        let channel_overflow = self.channel_overflow;

        tokio::spawn(async move {
            // --- Phase 1: Full index ---
            info!(
                project = %project_root.display(),
                "background indexer: starting full index"
            );
            if let Err(e) = run_full_index(&project_root, &index_store, progress_callback.as_ref())
            {
                warn!(
                    error = %e,
                    "background indexer: full index failed"
                );
            } else {
                info!("background indexer: full index complete");
            }

            // --- Phase 1b: Generate embeddings ---
            // Prefer EmbeddingManager over direct EmbeddingService when both are set.
            let embedding_stats: Option<EmbeddingPassStats> = if let Some(ref emb_mgr) = embedding_manager {
                info!("background indexer: starting embedding generation (via EmbeddingManager)");
                match run_embedding_pass_managed(&project_root, &index_store, emb_mgr, hnsw_index.as_ref()).await {
                    Ok(stats) => {
                        if stats.has_failures() {
                            warn!(
                                failed = stats.failed_files,
                                total = stats.total_files,
                                "background indexer: embedding generation (managed) complete with partial failures"
                            );
                        } else {
                            info!("background indexer: embedding generation (managed) complete");
                        }
                        Some(stats)
                    }
                    Err(e) => {
                        warn!(
                            error = %e,
                            "background indexer: embedding generation (managed) failed"
                        );
                        None
                    }
                }
            } else if let Some(ref emb_svc) = embedding_service {
                info!("background indexer: starting embedding generation");
                if let Err(e) = run_embedding_pass(&project_root, &index_store, emb_svc, hnsw_index.as_ref()).await {
                    warn!(
                        error = %e,
                        "background indexer: embedding generation failed"
                    );
                } else {
                    info!("background indexer: embedding generation complete");
                }
                None // Legacy path does not return stats
            } else {
                None
            };

            // --- Phase 2: Incremental updates ---
            if let Some(mut rx) = change_rx {
                let gitignore = build_gitignore_matcher(&project_root);
                run_incremental_loop(
                    &mut rx,
                    &project_root,
                    &index_store,
                    embedding_manager.as_ref(),
                    embedding_service.as_ref(),
                    hnsw_index.as_ref(),
                    gitignore,
                    channel_overflow.as_ref().map(|f| f.as_ref()),
                )
                .await;
            }

            embedding_stats
        })
    }

    /// Spawn a background task that ONLY listens for incremental changes.
    ///
    /// Skips Phase 1 (full index) and Phase 1b (full embedding).
    /// Used when restoring an already-indexed project after app restart.
    pub async fn start_watch_only(self) -> tokio::task::JoinHandle<()> {
        let project_root = self.project_root;
        let index_store = self.index_store;
        let embedding_manager = self.embedding_manager;
        let embedding_service = self.embedding_service;
        let hnsw_index = self.hnsw_index;
        let change_rx = self.change_rx;
        let channel_overflow = self.channel_overflow;

        tokio::spawn(async move {
            if let Some(mut rx) = change_rx {
                debug!(
                    project = %project_root.display(),
                    "background indexer: starting watch-only incremental loop"
                );
                let gitignore = build_gitignore_matcher(&project_root);
                run_incremental_loop(
                    &mut rx,
                    &project_root,
                    &index_store,
                    embedding_manager.as_ref(),
                    embedding_service.as_ref(),
                    hnsw_index.as_ref(),
                    gitignore,
                    channel_overflow.as_ref().map(|f| f.as_ref()),
                )
                .await;
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a comprehensive `Gitignore` matcher for incremental indexing.
///
/// Respects the same gitignore hierarchy as the full index (`WalkBuilder`):
/// - Global gitignore (`~/.config/git/ignore` or `$XDG_CONFIG_HOME/git/ignore`)
/// - `.git/info/exclude`
/// - Root `.gitignore`
/// - Subdirectory `.gitignore` files (e.g. `src/.gitignore`)
///
/// Returns `Gitignore::empty()` if building fails.
fn build_gitignore_matcher(project_root: &Path) -> Gitignore {
    let mut builder = GitignoreBuilder::new(project_root);

    // 1. Global gitignore
    if let Some(global_path) = resolve_global_gitignore() {
        builder.add(global_path);
    }

    // 2. .git/info/exclude
    builder.add(project_root.join(".git/info/exclude"));

    // 3. Root .gitignore
    builder.add(project_root.join(".gitignore"));

    // 4. Subdirectory .gitignore files — walk the tree using the `ignore` crate
    //    itself so that already-ignored directories are skipped efficiently.
    let walker = WalkBuilder::new(project_root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .max_depth(Some(15))
        .build();

    for entry in walker {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.is_file()
            && path.file_name().and_then(|n| n.to_str()) == Some(".gitignore")
            && path.parent() != Some(project_root)
        {
            builder.add(path);
        }
    }

    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

/// Resolve the global gitignore path.
///
/// Checks `$XDG_CONFIG_HOME/git/ignore` first, then `~/.config/git/ignore`.
fn resolve_global_gitignore() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg).join("git/ignore");
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".config/git/ignore");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Check whether a path should be ignored according to the gitignore matcher.
fn is_ignored_by_gitignore(gitignore: &Gitignore, project_root: &Path, path: &Path) -> bool {
    let rel_path = path.strip_prefix(project_root).unwrap_or(path);
    gitignore
        .matched_path_or_any_parents(rel_path, path.is_dir())
        .is_ignore()
}

/// Shared Phase-2 incremental loop used by both `start()` and `start_watch_only()`.
///
/// Uses a batch-drain pattern: waits for the first event, then drains all
/// immediately available events before processing.  This provides natural
/// deduplication for bursts (e.g. `git checkout`) and enables `.gitignore`
/// change detection within each batch.
async fn run_incremental_loop(
    rx: &mut tokio::sync::mpsc::Receiver<PathBuf>,
    project_root: &Path,
    index_store: &IndexStore,
    embedding_manager: Option<&Arc<EmbeddingManager>>,
    embedding_service: Option<&Arc<EmbeddingService>>,
    hnsw_index: Option<&Arc<HnswIndex>>,
    mut gitignore: Gitignore,
    channel_overflow: Option<&AtomicBool>,
) {
    let project_path = project_root.to_string_lossy().to_string();

    debug!("background indexer: listening for incremental changes");
    loop {
        // Wait for the first event (blocks until a path arrives or channel closes).
        let first = match rx.recv().await {
            Some(path) => path,
            None => break,
        };

        // Drain all immediately available events into a batch.
        let mut batch = vec![first];
        while let Ok(path) = rx.try_recv() {
            batch.push(path);
        }

        // Detect .gitignore changes in this batch and rebuild the matcher.
        let gitignore_changed = batch.iter().any(|p| {
            p.file_name().and_then(|n| n.to_str()) == Some(".gitignore")
        });
        if gitignore_changed {
            gitignore = build_gitignore_matcher(project_root);
            info!("background indexer: .gitignore changed, rebuilt matcher");
            apply_gitignore_retroactively(project_root, index_store, &gitignore, hnsw_index).await;
        }

        // Deduplicate paths within the batch.
        let unique: HashSet<PathBuf> = batch.into_iter().collect();

        for changed_path in unique {
            if is_ignored_by_gitignore(&gitignore, project_root, &changed_path) {
                debug!(path = %changed_path.display(), "background indexer: skipping gitignored path");
                continue;
            }
            match run_incremental_index(project_root, index_store, &changed_path) {
                Ok(IncrementalResult::Deleted { rel_path }) => {
                    // Mark HNSW entries stale, then delete embeddings from SQLite.
                    if let Some(hnsw) = hnsw_index {
                        if hnsw.is_ready().await {
                            if let Ok(rowids) =
                                index_store.get_embedding_rowids_for_file(&project_path, &rel_path)
                            {
                                for rowid in rowids {
                                    hnsw.mark_stale(rowid).await;
                                }
                            }
                        }
                    }
                    let _ = index_store.delete_embeddings_for_file(&project_path, &rel_path);
                }
                Ok(IncrementalResult::DirectoryDeleted { child_rel_paths }) => {
                    for rel_path in &child_rel_paths {
                        if let Some(hnsw) = hnsw_index {
                            if hnsw.is_ready().await {
                                if let Ok(rowids) =
                                    index_store.get_embedding_rowids_for_file(&project_path, rel_path)
                                {
                                    for rowid in rowids {
                                        hnsw.mark_stale(rowid).await;
                                    }
                                }
                            }
                        }
                        let _ = index_store.delete_embeddings_for_file(&project_path, rel_path);
                    }
                }
                Ok(IncrementalResult::Skipped) => {}
                Ok(IncrementalResult::Updated {
                    content,
                    language,
                    rel_path,
                }) => {
                    if let Some(emb_mgr) = embedding_manager {
                        if let Err(e) = run_incremental_embedding_managed_with_content(
                            project_root,
                            index_store,
                            emb_mgr,
                            &changed_path,
                            &content,
                            &language,
                            &rel_path,
                            hnsw_index,
                        )
                        .await
                        {
                            warn!(
                                path = %changed_path.display(),
                                error = %e,
                                "background indexer: incremental embedding (managed) failed"
                            );
                        }
                    } else if let Some(emb_svc) = embedding_service {
                        if let Err(e) = run_incremental_embedding_with_content(
                            project_root,
                            index_store,
                            emb_svc,
                            &changed_path,
                            &content,
                            &language,
                            &rel_path,
                            hnsw_index,
                        )
                        .await
                        {
                            warn!(
                                path = %changed_path.display(),
                                error = %e,
                                "background indexer: incremental embedding failed"
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        path = %changed_path.display(),
                        error = %e,
                        "background indexer: incremental index failed"
                    );
                }
            }
        }

        // Check channel overflow flag after processing each batch.
        if let Some(flag) = channel_overflow {
            if flag.swap(false, Ordering::AcqRel) {
                info!("background indexer: channel overflow detected, running catch-up sync");
                run_catchup_sync(
                    project_root,
                    index_store,
                    &gitignore,
                    embedding_manager,
                    embedding_service,
                    hnsw_index,
                )
                .await;
            }
        }
    }
    debug!("background indexer: change channel closed, stopping");
}

/// Remove index entries for files that are now matched by an updated gitignore.
///
/// Called after `.gitignore` changes to clean up entries that should no longer
/// be indexed.
async fn apply_gitignore_retroactively(
    project_root: &Path,
    index_store: &IndexStore,
    gitignore: &Gitignore,
    hnsw_index: Option<&Arc<HnswIndex>>,
) {
    let project_path = project_root.to_string_lossy().to_string();
    let indexed_paths = match index_store.get_indexed_file_paths(&project_path) {
        Ok(paths) => paths,
        Err(e) => {
            warn!(error = %e, "background indexer: failed to get indexed paths for gitignore cleanup");
            return;
        }
    };

    let mut removed = 0usize;
    for rel_path in &indexed_paths {
        let abs_path = project_root.join(rel_path);
        if is_ignored_by_gitignore(gitignore, project_root, &abs_path) {
            // Mark HNSW entries stale
            if let Some(hnsw) = hnsw_index {
                if hnsw.is_ready().await {
                    if let Ok(rowids) =
                        index_store.get_embedding_rowids_for_file(&project_path, rel_path)
                    {
                        for rowid in rowids {
                            hnsw.mark_stale(rowid).await;
                        }
                    }
                }
            }
            let _ = index_store.delete_embeddings_for_file(&project_path, rel_path);
            let _ = index_store.delete_file_index(&project_path, rel_path);
            removed += 1;
        }
    }

    if removed > 0 {
        info!(
            removed_files = removed,
            "background indexer: removed newly-ignored files from index"
        );
    }
}

/// Lightweight catch-up sync after channel overflow.
///
/// Reconciles the file index with the current state of the file system:
/// 1. Remove entries for files that no longer exist or are now gitignored
/// 2. Discover and index new/changed files
///
/// Also updates embeddings for changed/new files so they don't remain stale
/// indefinitely after a channel overflow event.
async fn run_catchup_sync(
    project_root: &Path,
    index_store: &IndexStore,
    gitignore: &Gitignore,
    embedding_manager: Option<&Arc<EmbeddingManager>>,
    embedding_service: Option<&Arc<EmbeddingService>>,
    hnsw_index: Option<&Arc<HnswIndex>>,
) {
    let project_path = project_root.to_string_lossy().to_string();

    // 1. Check existing index entries: remove deleted / gitignored files,
    //    update files whose hash changed.
    if let Ok(indexed_paths) = index_store.get_indexed_file_paths(&project_path) {
        let mut removed = 0usize;
        let mut updated = 0usize;

        for rel_path in &indexed_paths {
            let abs_path = project_root.join(rel_path);

            // Remove if deleted or now gitignored
            if !abs_path.is_file() || is_ignored_by_gitignore(gitignore, project_root, &abs_path) {
                let _ = index_store.delete_file_index(&project_path, rel_path);
                let _ = index_store.delete_embeddings_for_file(&project_path, rel_path);
                removed += 1;
                continue;
            }

            // Check hash staleness and re-index if changed
            if let Ok(IncrementalResult::Updated {
                content,
                language,
                rel_path,
            }) = run_incremental_index(project_root, index_store, &abs_path)
            {
                updated += 1;
                // Also update embedding for this file
                if let Some(emb_mgr) = embedding_manager {
                    let _ = run_incremental_embedding_managed_with_content(
                        project_root,
                        index_store,
                        emb_mgr,
                        &abs_path,
                        &content,
                        &language,
                        &rel_path,
                        hnsw_index,
                    )
                    .await;
                } else if let Some(emb_svc) = embedding_service {
                    let _ = run_incremental_embedding_with_content(
                        project_root,
                        index_store,
                        emb_svc,
                        &abs_path,
                        &content,
                        &language,
                        &rel_path,
                        hnsw_index,
                    )
                    .await;
                }
            }
        }

        if removed > 0 || updated > 0 {
            info!(
                removed = removed,
                updated = updated,
                "background indexer: catch-up sync reconciled existing entries"
            );
        }
    }

    // 2. Discover new files by scanning the file system.
    if let Ok(inventory) = build_file_inventory(project_root, &[]) {
        let mut new_files = 0usize;
        for item in &inventory.items {
            let abs_path = project_root.join(&item.path);
            if is_ignored_by_gitignore(gitignore, project_root, &abs_path) {
                continue;
            }
            let content_hash = compute_content_hash(&abs_path);
            let is_stale = index_store
                .is_index_stale(&project_path, &item.path, &content_hash)
                .unwrap_or(true);
            if is_stale {
                if let Err(e) = index_store.upsert_file_index(&project_path, item, &content_hash) {
                    warn!(
                        file = %item.path,
                        error = %e,
                        "background indexer: catch-up sync failed to upsert file"
                    );
                } else {
                    new_files += 1;
                    // Embed the newly indexed file
                    if abs_path.is_file() && item.size_bytes <= MAX_EMBEDDABLE_FILE_SIZE {
                        if let Ok(content) = std::fs::read_to_string(&abs_path) {
                            if let Some(emb_mgr) = embedding_manager {
                                let _ = run_incremental_embedding_managed_with_content(
                                    project_root,
                                    index_store,
                                    emb_mgr,
                                    &abs_path,
                                    &content,
                                    &item.language,
                                    &item.path,
                                    hnsw_index,
                                )
                                .await;
                            } else if let Some(emb_svc) = embedding_service {
                                let _ = run_incremental_embedding_with_content(
                                    project_root,
                                    index_store,
                                    emb_svc,
                                    &abs_path,
                                    &content,
                                    &item.language,
                                    &item.path,
                                    hnsw_index,
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }

        if new_files > 0 {
            info!(
                new_files = new_files,
                "background indexer: catch-up sync discovered new files"
            );
        }
    }

    info!("background indexer: catch-up sync complete");
}

/// Run a full index of every file under `project_root`.
///
/// If a `progress_callback` is provided it is invoked every 10 files with
/// `(indexed_so_far, total_files)` and once at the end with `(total, total)`.
fn run_full_index(
    project_root: &Path,
    index_store: &IndexStore,
    progress_callback: Option<&IndexProgressCallback>,
) -> Result<(), String> {
    let inventory = build_file_inventory(project_root, &[]).map_err(|e| e.to_string())?;

    let project_path = project_root.to_string_lossy().to_string();
    let total_files = inventory.items.len();

    for (i, item) in inventory.items.iter().enumerate() {
        let abs_path = project_root.join(&item.path);
        let content_hash = compute_content_hash(&abs_path);
        if let Err(e) = index_store.upsert_file_index(&project_path, item, &content_hash) {
            warn!(
                file = %item.path,
                error = %e,
                "background indexer: failed to upsert file"
            );
        }

        // Report progress every 10 files
        let indexed_so_far = i + 1;
        if let Some(cb) = progress_callback {
            if indexed_so_far % 10 == 0 {
                cb(indexed_so_far, total_files);
            }
        }
    }

    // Final progress report
    if let Some(cb) = progress_callback {
        cb(total_files, total_files);
    }

    // --- Stale entry cleanup ---
    // Remove index entries for files that no longer exist in the project.
    {
        use std::collections::HashSet;

        let current_paths: HashSet<&str> =
            inventory.items.iter().map(|item| item.path.as_str()).collect();

        if let Ok(indexed_paths) = index_store.get_indexed_file_paths(&project_path) {
            let mut stale_count = 0usize;
            for stale_path in &indexed_paths {
                if !current_paths.contains(stale_path.as_str()) {
                    let _ = index_store.delete_file_index(&project_path, stale_path);
                    let _ = index_store.delete_embeddings_for_file(&project_path, stale_path);
                    stale_count += 1;
                }
            }
            if stale_count > 0 {
                info!(
                    stale_files = stale_count,
                    "background indexer: cleaned up stale entries"
                );
            }
        }
    }

    info!(files = total_files, "background indexer: full index stored");
    Ok(())
}

/// Re-index a single file if its content hash is stale, or clean up its
/// index entry when the file has been deleted from disk.
fn run_incremental_index(
    project_root: &Path,
    index_store: &IndexStore,
    changed_path: &Path,
) -> Result<IncrementalResult, String> {
    let rel = changed_path
        .strip_prefix(project_root)
        .map_err(|_| format!("path {:?} is not under project root", changed_path))?;

    let rel_str = rel.to_string_lossy().replace('\\', "/");
    let project_path = project_root.to_string_lossy().to_string();

    // If the path no longer exists on disk, handle deletion.
    if !changed_path.is_file() {
        // Try exact file match first (single file deleted).
        let deleted = index_store
            .delete_file_index(&project_path, &rel_str)
            .map_err(|e| e.to_string())?;
        if deleted > 0 {
            debug!(path = %rel_str, "background indexer: file deleted, cleaned up index");
            return Ok(IncrementalResult::Deleted { rel_path: rel_str });
        }

        // If path doesn't exist at all, it may be a deleted directory.
        // Find and clean up all child file indexes under this prefix.
        if !changed_path.exists() {
            let child_paths = index_store
                .get_indexed_files_under_prefix(&project_path, &rel_str)
                .map_err(|e| e.to_string())?;
            if !child_paths.is_empty() {
                for child in &child_paths {
                    let _ = index_store.delete_file_index(&project_path, child);
                }
                debug!(
                    path = %rel_str,
                    children = child_paths.len(),
                    "background indexer: directory deleted, cleaned up child file indexes"
                );
                return Ok(IncrementalResult::DirectoryDeleted {
                    child_rel_paths: child_paths,
                });
            }
        }

        return Ok(IncrementalResult::Skipped);
    }

    // Single disk read: read bytes once and derive hash + content from them.
    let bytes = std::fs::read(changed_path).map_err(|e| e.to_string())?;
    let content_hash = {
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        format!("{:x}", hasher.finalize())
    };

    let stale = index_store
        .is_index_stale(&project_path, &rel_str, &content_hash)
        .map_err(|e| e.to_string())?;

    if !stale {
        debug!(path = %rel_str, "background indexer: file unchanged, skipping");
        return Ok(IncrementalResult::Skipped);
    }

    // Convert the already-read bytes to UTF-8 string.
    let content = String::from_utf8(bytes)
        .map_err(|_| format!("non-UTF-8 file: {:?}", changed_path))?;
    let file_size = content.len() as u64;

    let ext = changed_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    let language = detect_language_simple(ext.as_deref());

    let limits = AnalysisLimits::default();
    let symbols = extract_symbols_from_str(&content, &language, limits.max_symbols_per_file);

    // Compute line count from already-read content instead of re-reading from disk.
    let line_count = if content.is_empty() {
        0
    } else {
        content.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1
    };

    let item = super::analysis_index::FileInventoryItem {
        path: rel_str.clone(),
        component: detect_component(&rel_str),
        language: language.clone(),
        extension: ext,
        size_bytes: file_size,
        line_count,
        is_test: is_test_path(&rel_str),
        symbols,
    };

    index_store
        .upsert_file_index(&project_path, &item, &content_hash)
        .map_err(|e| e.to_string())?;

    debug!(path = %rel_str, "background indexer: incremental index updated");
    Ok(IncrementalResult::Updated {
        content,
        language,
        rel_path: rel_str,
    })
}

/// Compute a SHA-256 content hash for the file at `path`.
///
/// Returns a hex-encoded hash string.  If the file cannot be read the
/// function returns a placeholder so that the caller can still proceed.
fn compute_content_hash(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => {
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            format!("{:x}", hasher.finalize())
        }
        Err(_) => "unreadable".to_string(),
    }
}

/// Minimal language detection mirroring `analysis_index::detect_language`.
fn detect_language_simple(ext: Option<&str>) -> String {
    match ext {
        Some("py") => "python",
        Some("rs") => "rust",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("go") => "go",
        Some("java") => "java",
        Some("json") | Some("toml") | Some("yaml") | Some("yml") => "config",
        Some("md") => "markdown",
        _ => "other",
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Chunking — splits source files into semantic pieces for embedding
// ---------------------------------------------------------------------------

/// Maximum number of lines per chunk for fixed-size windowing.
const CHUNK_MAX_LINES: usize = 60;
/// Overlap lines between consecutive fixed-size windows.
const CHUNK_OVERLAP_LINES: usize = 10;
/// Maximum file size (in bytes) to attempt embedding (skip very large files).
const MAX_EMBEDDABLE_FILE_SIZE: u64 = 500_000;

/// A chunk of source code with its origin information.
#[derive(Debug, Clone)]
pub struct FileChunk {
    pub index: usize,
    pub text: String,
}

/// Split file content into semantic chunks.
///
/// For languages supported by tree-sitter, chunks are aligned to symbol
/// boundaries (functions, classes, structs, etc.).  For unsupported languages,
/// a fixed-size sliding window is used.
pub fn chunk_file_content(content: &str, language: &str) -> Vec<FileChunk> {
    if content.is_empty() {
        return Vec::new();
    }

    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    if tree_sitter_parser::is_language_supported(language) {
        let symbols = tree_sitter_parser::parse_symbols(content, language, 200);
        if !symbols.is_empty() {
            return chunk_by_symbols(&lines, &symbols);
        }
    }

    // Fallback: fixed-size windows
    chunk_by_window(&lines)
}

/// Chunk by symbol boundaries.
///
/// Each top-level symbol (or group of small adjacent symbols) becomes a chunk.
/// Gaps between symbols are merged into the preceding or following chunk.
fn chunk_by_symbols(
    lines: &[&str],
    symbols: &[super::analysis_index::SymbolInfo],
) -> Vec<FileChunk> {
    // Collect (start_line, end_line) ranges for top-level symbols only (no parent).
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for sym in symbols {
        if sym.parent.is_some() {
            continue; // Skip nested symbols — they are inside a parent range
        }
        let start = sym.line.saturating_sub(1); // convert 1-based to 0-based
        let end = if sym.end_line > 0 {
            sym.end_line.min(lines.len())
        } else {
            // If no end_line, estimate: go until next symbol or +30 lines
            (start + 30).min(lines.len())
        };
        ranges.push((start, end));
    }

    if ranges.is_empty() {
        return chunk_by_window(lines);
    }

    // Sort ranges by start line
    ranges.sort_by_key(|r| r.0);

    // Merge overlapping/adjacent ranges and build chunks
    let mut chunks = Vec::new();
    let mut current_start = 0usize; // Include file header (imports, etc.)

    for (sym_start, sym_end) in &ranges {
        let sym_start = *sym_start;
        let sym_end = *sym_end;

        // If there is a gap between the current position and this symbol, include
        // it in the chunk (it might be comments, imports, etc.).
        if sym_start > current_start + CHUNK_MAX_LINES {
            // The gap is large — emit the gap as its own chunk
            let text: String = lines[current_start..sym_start].join("\n");
            if !text.trim().is_empty() {
                chunks.push(FileChunk {
                    index: chunks.len(),
                    text,
                });
            }
            current_start = sym_start;
        }

        let chunk_end = sym_end.min(lines.len());
        if chunk_end > current_start {
            let text: String = lines[current_start..chunk_end].join("\n");
            if !text.trim().is_empty() {
                chunks.push(FileChunk {
                    index: chunks.len(),
                    text,
                });
            }
            current_start = chunk_end;
        }
    }

    // Remaining tail
    if current_start < lines.len() {
        let text: String = lines[current_start..].join("\n");
        if !text.trim().is_empty() {
            chunks.push(FileChunk {
                index: chunks.len(),
                text,
            });
        }
    }

    // If we somehow produced zero chunks, fall back to window chunking
    if chunks.is_empty() {
        return chunk_by_window(lines);
    }

    chunks
}

/// Chunk by fixed-size sliding window.
fn chunk_by_window(lines: &[&str]) -> Vec<FileChunk> {
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < lines.len() {
        let end = (start + CHUNK_MAX_LINES).min(lines.len());
        let text: String = lines[start..end].join("\n");
        if !text.trim().is_empty() {
            chunks.push(FileChunk {
                index: chunks.len(),
                text,
            });
        }
        if end >= lines.len() {
            break;
        }
        start = end.saturating_sub(CHUNK_OVERLAP_LINES);
    }

    chunks
}

// ---------------------------------------------------------------------------
// Embedding pass — runs after full index to generate TF-IDF vectors
// ---------------------------------------------------------------------------

/// Run an embedding pass over all indexed files in the project.
///
/// 1. Reads each file under `project_root`
/// 2. Chunks the content using `chunk_file_content`
/// 3. Builds the TF-IDF vocabulary from all chunks
/// 4. Generates and stores embedding vectors
async fn run_embedding_pass(
    project_root: &Path,
    index_store: &IndexStore,
    embedding_service: &EmbeddingService,
    hnsw_index: Option<&Arc<HnswIndex>>,
) -> Result<(), String> {
    let project_path = project_root.to_string_lossy().to_string();

    // Collect all chunks first (to build vocabulary from the full corpus)
    let inventory = build_file_inventory(project_root, &[]).map_err(|e| e.to_string())?;

    let mut all_chunks: Vec<(String, FileChunk)> = Vec::new(); // (relative_path, chunk)

    for item in &inventory.items {
        if item.size_bytes > MAX_EMBEDDABLE_FILE_SIZE {
            continue;
        }
        let abs_path = project_root.join(&item.path);
        let content = match std::fs::read_to_string(&abs_path) {
            Ok(c) => c,
            Err(_) => continue, // skip unreadable files
        };
        let chunks = chunk_file_content(&content, &item.language);
        for chunk in chunks {
            all_chunks.push((item.path.clone(), chunk));
        }
    }

    if all_chunks.is_empty() {
        info!("background indexer: no chunks to embed");
        return Ok(());
    }

    // Build vocabulary from all chunk texts
    let texts: Vec<&str> = all_chunks.iter().map(|(_, c)| c.text.as_str()).collect();
    embedding_service.build_vocabulary(&texts);

    // Generate embeddings and store them per-file using replace_file_embeddings_tfidf
    // to prevent orphan chunks when a file's chunk count decreases.
    let embeddings = embedding_service.embed_batch(&texts);
    let mut stored = 0usize;

    // Group chunks and embeddings by file path
    let mut file_groups: HashMap<&str, Vec<(usize, &FileChunk, &Vec<f32>)>> = HashMap::new();
    for (i, ((rel_path, chunk), embedding)) in
        all_chunks.iter().zip(embeddings.iter()).enumerate()
    {
        file_groups
            .entry(rel_path.as_str())
            .or_default()
            .push((i, chunk, embedding));
    }

    for (rel_path, group) in &file_groups {
        let embedding_bytes: Vec<Vec<u8>> =
            group.iter().map(|(_, _, emb)| embedding_to_bytes(emb)).collect();
        let batch: Vec<(i64, &str, &[u8])> = group
            .iter()
            .zip(embedding_bytes.iter())
            .map(|((_, chunk, _), bytes)| {
                (chunk.index as i64, chunk.text.as_str(), bytes.as_slice())
            })
            .collect();

        match index_store.replace_file_embeddings_tfidf(&project_path, rel_path, &batch) {
            Ok(()) => {
                stored += batch.len();
            }
            Err(e) => {
                warn!(
                    file = %rel_path,
                    error = %e,
                    "background indexer: failed to store embeddings"
                );
            }
        }
    }

    info!(
        chunks = stored,
        total = all_chunks.len(),
        "background indexer: embeddings stored"
    );

    // Rebuild HNSW index from SQLite after full embedding pass.
    // This is more efficient than inserting one-by-one during the loop,
    // because we can batch-insert all embeddings at once.
    if let Some(hnsw) = hnsw_index {
        if let Err(e) = rebuild_hnsw_after_embedding(index_store, &project_path, hnsw).await {
            warn!(
                error = %e,
                "background indexer: HNSW rebuild after embedding pass failed"
            );
        }
    }

    // Persist the vocabulary to SQLite so it survives app restart
    if let Some(vocab_json) = embedding_service.export_vocabulary() {
        if let Err(e) = index_store.save_vocabulary(&project_path, &vocab_json) {
            warn!(
                error = %e,
                "background indexer: failed to save vocabulary to SQLite"
            );
        } else {
            info!("background indexer: vocabulary saved to SQLite");
        }
    }

    Ok(())
}

/// Re-embed a single changed file.
///
/// Retained for catch-up sync and other scenarios where the file content is
/// not already available in memory.
#[allow(dead_code)]
async fn run_incremental_embedding(
    project_root: &Path,
    index_store: &IndexStore,
    embedding_service: &EmbeddingService,
    changed_path: &Path,
    hnsw_index: Option<&Arc<HnswIndex>>,
) -> Result<(), String> {
    if !changed_path.is_file() {
        return Ok(());
    }

    let rel = changed_path
        .strip_prefix(project_root)
        .map_err(|_| format!("path {:?} is not under project root", changed_path))?;
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    let project_path = project_root.to_string_lossy().to_string();

    let metadata = std::fs::metadata(changed_path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_EMBEDDABLE_FILE_SIZE {
        return Ok(());
    }

    let content = std::fs::read_to_string(changed_path).map_err(|e| e.to_string())?;
    let ext = changed_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    let language = detect_language_simple(ext.as_deref());

    // Mark old embeddings for this file as stale in HNSW before deleting from SQLite.
    // We need to capture the ROWIDs before deleting.
    if let Some(hnsw) = hnsw_index {
        if hnsw.is_ready().await {
            if let Ok(rowids) = index_store.get_embedding_rowids_for_file(&project_path, &rel_str)
            {
                for rowid in rowids {
                    hnsw.mark_stale(rowid).await;
                }
            }
        }
    }

    // Delete old embeddings for this file
    let _ = index_store.delete_embeddings_for_file(&project_path, &rel_str);

    if !embedding_service.is_ready() {
        // Vocabulary not in memory — try to restore from SQLite before giving up.
        match index_store.load_vocabulary(&project_path) {
            Ok(Some(json)) => {
                if let Err(e) = embedding_service.import_vocabulary(&json) {
                    warn!(
                        error = %e,
                        "background indexer: failed to import vocabulary for incremental embedding"
                    );
                    return Ok(());
                }
                info!(
                    "background indexer: restored vocabulary from SQLite for incremental embedding"
                );
            }
            _ => {
                // No vocabulary in DB either — skip. It will be built on the next full pass.
                return Ok(());
            }
        }
    }

    let chunks = chunk_file_content(&content, &language);
    for chunk in &chunks {
        let embedding = embedding_service.embed_text(&chunk.text);
        let bytes = embedding_to_bytes(&embedding);
        if let Err(e) = index_store.upsert_chunk_embedding(
            &project_path,
            &rel_str,
            chunk.index as i64,
            &chunk.text,
            &bytes,
        ) {
            warn!(
                file = %rel_str,
                chunk = chunk.index,
                error = %e,
                "background indexer: incremental embedding failed"
            );
        } else {
            // Insert new embedding into HNSW
            if let Some(hnsw) = hnsw_index {
                if hnsw.is_ready().await {
                    if let Ok(Some(rowid)) = index_store.get_embedding_rowid_for_chunk(
                        &project_path,
                        &rel_str,
                        chunk.index as i64,
                    ) {
                        hnsw.insert(rowid, &embedding).await;
                    }
                }
            }
        }
    }

    // Check if HNSW needs a full rebuild due to stale ID accumulation (>10%)
    if let Some(hnsw) = hnsw_index {
        if hnsw.is_ready().await && hnsw.needs_rebuild().await {
            info!(
                "background indexer: HNSW stale ratio exceeded 10%, triggering full rebuild"
            );
            if let Err(e) = rebuild_hnsw_after_embedding(index_store, &project_path, hnsw).await {
                warn!(
                    error = %e,
                    "background indexer: periodic HNSW rebuild failed"
                );
            }
        } else if hnsw.is_ready().await {
            // Just save incrementally
            if let Err(e) = hnsw.save_to_disk().await {
                warn!(
                    error = %e,
                    "background indexer: failed to save HNSW after incremental embedding"
                );
            }
        }
    }

    debug!(
        path = %rel_str,
        chunks = chunks.len(),
        "background indexer: incremental embedding updated"
    );
    Ok(())
}

/// Re-embed a single changed file using already-read content.
///
/// Avoids redundant disk I/O by accepting the file content, language, and
/// relative path that were already computed during the incremental index step.
///
/// Uses the "generate-then-swap" pattern for transactional safety.
async fn run_incremental_embedding_with_content(
    project_root: &Path,
    index_store: &IndexStore,
    embedding_service: &EmbeddingService,
    _changed_path: &Path,
    content: &str,
    language: &str,
    rel_str: &str,
    hnsw_index: Option<&Arc<HnswIndex>>,
) -> Result<(), String> {
    let project_path = project_root.to_string_lossy().to_string();

    if content.len() as u64 > MAX_EMBEDDABLE_FILE_SIZE {
        return Ok(());
    }

    // Mark old HNSW entries as stale.
    if let Some(hnsw) = hnsw_index {
        if hnsw.is_ready().await {
            if let Ok(rowids) = index_store.get_embedding_rowids_for_file(&project_path, rel_str) {
                for rowid in rowids {
                    hnsw.mark_stale(rowid).await;
                }
            }
        }
    }

    if !embedding_service.is_ready() {
        match index_store.load_vocabulary(&project_path) {
            Ok(Some(json)) => {
                if let Err(e) = embedding_service.import_vocabulary(&json) {
                    warn!(
                        error = %e,
                        "background indexer: failed to import vocabulary for incremental embedding"
                    );
                    return Ok(());
                }
                info!(
                    "background indexer: restored vocabulary from SQLite for incremental embedding"
                );
            }
            _ => return Ok(()),
        }
    }

    // Generate all embeddings first (outside any transaction).
    let chunks = chunk_file_content(content, language);
    if chunks.is_empty() {
        return Ok(());
    }

    let embeddings: Vec<Vec<f32>> = chunks
        .iter()
        .map(|chunk| embedding_service.embed_text(&chunk.text))
        .collect();

    // Build the batch for the atomic replace.
    let embedding_bytes: Vec<Vec<u8>> = embeddings.iter().map(|e| embedding_to_bytes(e)).collect();
    let batch: Vec<(i64, &str, &[u8])> = chunks
        .iter()
        .zip(embedding_bytes.iter())
        .map(|(chunk, bytes)| (chunk.index as i64, chunk.text.as_str(), bytes.as_slice()))
        .collect();

    // Atomic delete-then-insert in a single transaction.
    if let Err(e) = index_store.replace_file_embeddings_tfidf(&project_path, rel_str, &batch) {
        warn!(
            file = %rel_str,
            error = %e,
            "background indexer: transactional embedding replace failed"
        );
        return Err(e.to_string());
    }

    // Insert new embeddings into HNSW.
    if let Some(hnsw) = hnsw_index {
        if hnsw.is_ready().await {
            for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
                if let Ok(Some(rowid)) = index_store.get_embedding_rowid_for_chunk(
                    &project_path,
                    rel_str,
                    chunk.index as i64,
                ) {
                    hnsw.insert(rowid, embedding).await;
                }
            }
        }
    }

    // Check if HNSW needs a full rebuild due to stale ID accumulation (>10%)
    if let Some(hnsw) = hnsw_index {
        if hnsw.is_ready().await && hnsw.needs_rebuild().await {
            info!(
                "background indexer: HNSW stale ratio exceeded 10%, triggering full rebuild"
            );
            if let Err(e) = rebuild_hnsw_after_embedding(index_store, &project_path, hnsw).await {
                warn!(
                    error = %e,
                    "background indexer: periodic HNSW rebuild failed"
                );
            }
        } else if hnsw.is_ready().await {
            if let Err(e) = hnsw.save_to_disk().await {
                warn!(
                    error = %e,
                    "background indexer: failed to save HNSW after incremental embedding"
                );
            }
        }
    }

    debug!(
        path = %rel_str,
        chunks = chunks.len(),
        "background indexer: incremental embedding updated"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// EmbeddingManager-based embedding pass (story-005)
// ---------------------------------------------------------------------------

/// Run an embedding pass using the `EmbeddingManager` dispatch layer.
///
/// This is the provider-aware replacement for `run_embedding_pass`. It:
/// 1. Collects file chunks the same way as the legacy path
/// 2. If the primary provider is TF-IDF, builds vocabulary before embedding
/// 3. Uses `EmbeddingManager::embed_documents()` for batch embedding
/// 4. Stores embeddings in IndexStore
/// 5. Persists TF-IDF vocabulary if applicable
async fn run_embedding_pass_managed(
    project_root: &Path,
    index_store: &IndexStore,
    manager: &EmbeddingManager,
    hnsw_index: Option<&Arc<HnswIndex>>,
) -> Result<EmbeddingPassStats, String> {
    let project_path = project_root.to_string_lossy().to_string();

    // Collect all chunks first, grouped by file for per-file fault tolerance.
    let inventory = build_file_inventory(project_root, &[]).map_err(|e| e.to_string())?;

    // Group chunks by relative file path
    let mut file_chunks: HashMap<String, Vec<FileChunk>> = HashMap::new();
    let mut all_texts_for_vocab: Vec<String> = Vec::new();

    for item in &inventory.items {
        if item.size_bytes > MAX_EMBEDDABLE_FILE_SIZE {
            continue;
        }
        let abs_path = project_root.join(&item.path);
        let content = match std::fs::read_to_string(&abs_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let chunks = chunk_file_content(&content, &item.language);
        for chunk in &chunks {
            all_texts_for_vocab.push(chunk.text.clone());
        }
        if !chunks.is_empty() {
            file_chunks.insert(item.path.clone(), chunks);
        }
    }

    if file_chunks.is_empty() {
        info!("background indexer: no chunks to embed (managed)");
        return Ok(EmbeddingPassStats::default());
    }

    // If primary provider is TF-IDF, build vocabulary from the full corpus
    // (TF-IDF needs all texts to compute IDF weights correctly).
    if manager.provider_type() == EmbeddingProviderType::TfIdf {
        let refs: Vec<&str> = all_texts_for_vocab.iter().map(|s| s.as_str()).collect();
        let primary = manager.primary_provider();
        if let Some(tfidf) = primary.as_any().downcast_ref::<TfIdfEmbeddingProvider>() {
            tfidf.build_vocabulary(&refs);
            debug!(
                chunks = refs.len(),
                "background indexer: TF-IDF vocabulary built via manager"
            );
        }
    }

    let mut stats = EmbeddingPassStats {
        stored_chunks: 0,
        failed_files: 0,
        total_files: file_chunks.len(),
    };

    // Embed per-file: each file can fail independently without losing others.
    // Uses replace_file_embeddings() to atomically delete old + insert new,
    // preventing orphan chunks when a file's chunk count decreases.
    for (rel_path, chunks) in &file_chunks {
        let file_texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();

        match manager.embed_documents(&file_texts).await {
            Ok(embeddings) => {
                let embedding_bytes: Vec<Vec<u8>> =
                    embeddings.iter().map(|e| embedding_to_bytes(e)).collect();
                let batch: Vec<(i64, &str, &[u8])> = chunks
                    .iter()
                    .zip(embedding_bytes.iter())
                    .map(|(chunk, bytes)| {
                        (chunk.index as i64, chunk.text.as_str(), bytes.as_slice())
                    })
                    .collect();

                let provider_type_str =
                    format!("{:?}", manager.provider_type()).to_lowercase();
                let provider_model = manager.display_name().to_string();
                let dim = embeddings.first().map(|e| e.len() as i64).unwrap_or(0);

                match index_store.replace_file_embeddings(
                    &project_path,
                    rel_path,
                    &batch,
                    &provider_type_str,
                    &provider_model,
                    dim,
                ) {
                    Ok(()) => {
                        stats.stored_chunks += batch.len();
                    }
                    Err(e) => {
                        warn!(
                            file = %rel_path,
                            error = %e,
                            "background indexer: failed to store embeddings (managed)"
                        );
                        stats.failed_files += 1;
                    }
                }
            }
            Err(e) => {
                warn!(
                    file = %rel_path,
                    error = %e,
                    "background indexer: embedding failed for file, skipping"
                );
                stats.failed_files += 1;
            }
        }
    }

    info!(
        stored = stats.stored_chunks,
        failed_files = stats.failed_files,
        total_files = stats.total_files,
        "background indexer: per-file embedding pass complete (managed)"
    );

    // Rebuild HNSW index from SQLite after full embedding pass
    if let Some(hnsw) = hnsw_index {
        if let Err(e) = rebuild_hnsw_after_embedding(index_store, &project_path, hnsw).await {
            warn!(
                error = %e,
                "background indexer: HNSW rebuild after managed embedding pass failed"
            );
        }
    }

    // Persist TF-IDF vocabulary to SQLite if applicable
    if manager.provider_type() == EmbeddingProviderType::TfIdf {
        let primary = manager.primary_provider();
        if let Some(tfidf) = primary.as_any().downcast_ref::<TfIdfEmbeddingProvider>() {
            if let Some(vocab_json) = tfidf.export_vocabulary() {
                if let Err(e) = index_store.save_vocabulary(&project_path, &vocab_json) {
                    warn!(
                        error = %e,
                        "background indexer: failed to save vocabulary to SQLite (managed)"
                    );
                } else {
                    info!("background indexer: vocabulary saved to SQLite (managed)");
                }
            }
        }
    }

    Ok(stats)
}

/// Re-embed a single changed file using the `EmbeddingManager`.
///
/// Provider-aware replacement for `run_incremental_embedding`. Handles
/// TF-IDF vocabulary restoration from SQLite when the provider is not ready.
///
/// Retained for catch-up sync and other scenarios where the file content is
/// not already available in memory.
#[allow(dead_code)]
async fn run_incremental_embedding_managed(
    project_root: &Path,
    index_store: &IndexStore,
    manager: &EmbeddingManager,
    changed_path: &Path,
    hnsw_index: Option<&Arc<HnswIndex>>,
) -> Result<(), String> {
    if !changed_path.is_file() {
        return Ok(());
    }

    let rel = changed_path
        .strip_prefix(project_root)
        .map_err(|_| format!("path {:?} is not under project root", changed_path))?;
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    let project_path = project_root.to_string_lossy().to_string();

    let metadata = std::fs::metadata(changed_path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_EMBEDDABLE_FILE_SIZE {
        return Ok(());
    }

    let content = std::fs::read_to_string(changed_path).map_err(|e| e.to_string())?;
    let ext = changed_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    let language = detect_language_simple(ext.as_deref());

    // Mark old embeddings for this file as stale in HNSW before deleting from SQLite
    if let Some(hnsw) = hnsw_index {
        if hnsw.is_ready().await {
            if let Ok(rowids) = index_store.get_embedding_rowids_for_file(&project_path, &rel_str)
            {
                for rowid in rowids {
                    hnsw.mark_stale(rowid).await;
                }
            }
        }
    }

    // Delete old embeddings for this file
    let _ = index_store.delete_embeddings_for_file(&project_path, &rel_str);

    // If primary provider is TF-IDF, ensure vocabulary is loaded
    if manager.provider_type() == EmbeddingProviderType::TfIdf {
        let primary = manager.primary_provider();
        if let Some(tfidf) = primary.as_any().downcast_ref::<TfIdfEmbeddingProvider>() {
            if !tfidf.is_ready() {
                // Try to restore vocabulary from SQLite
                match index_store.load_vocabulary(&project_path) {
                    Ok(Some(json)) => {
                        if let Err(e) = tfidf.import_vocabulary(&json) {
                            warn!(
                                error = %e,
                                "background indexer: failed to import vocabulary for incremental embedding (managed)"
                            );
                            return Ok(());
                        }
                        info!(
                            "background indexer: restored vocabulary from SQLite for incremental embedding (managed)"
                        );
                    }
                    _ => {
                        // No vocabulary in DB — skip. It will be built on the next full pass.
                        return Ok(());
                    }
                }
            }
        }
    }

    let chunks = chunk_file_content(&content, &language);
    if chunks.is_empty() {
        return Ok(());
    }

    let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();

    let embeddings = manager
        .embed_documents(&texts)
        .await
        .map_err(|e| format!("embedding manager incremental embed failed: {}", e))?;

    for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
        let bytes = embedding_to_bytes(embedding);
        if let Err(e) = index_store.upsert_chunk_embedding(
            &project_path,
            &rel_str,
            chunk.index as i64,
            &chunk.text,
            &bytes,
        ) {
            warn!(
                file = %rel_str,
                chunk = chunk.index,
                error = %e,
                "background indexer: incremental embedding failed (managed)"
            );
        } else {
            // Insert new embedding into HNSW
            if let Some(hnsw) = hnsw_index {
                if hnsw.is_ready().await {
                    if let Ok(Some(rowid)) = index_store.get_embedding_rowid_for_chunk(
                        &project_path,
                        &rel_str,
                        chunk.index as i64,
                    ) {
                        hnsw.insert(rowid, embedding).await;
                    }
                }
            }
        }
    }

    // Check if HNSW needs a full rebuild due to stale ID accumulation (>10%)
    if let Some(hnsw) = hnsw_index {
        if hnsw.is_ready().await && hnsw.needs_rebuild().await {
            info!(
                "background indexer: HNSW stale ratio exceeded 10% (managed), triggering full rebuild"
            );
            if let Err(e) = rebuild_hnsw_after_embedding(index_store, &project_path, hnsw).await {
                warn!(
                    error = %e,
                    "background indexer: periodic HNSW rebuild failed (managed)"
                );
            }
        } else if hnsw.is_ready().await {
            // Just save incrementally
            if let Err(e) = hnsw.save_to_disk().await {
                warn!(
                    error = %e,
                    "background indexer: failed to save HNSW after incremental embedding (managed)"
                );
            }
        }
    }

    debug!(
        path = %rel_str,
        chunks = chunks.len(),
        "background indexer: incremental embedding updated (managed)"
    );
    Ok(())
}

/// Re-embed a single changed file using the `EmbeddingManager` and already-read content.
///
/// Avoids redundant disk I/O by accepting the file content, language, and
/// relative path that were already computed during the incremental index step.
///
/// Uses the "generate-then-swap" pattern: embeddings are computed first
/// (outside any transaction), then atomically replaced in SQLite via
/// `replace_file_embeddings`.  If the API call fails the old embeddings
/// remain intact.
async fn run_incremental_embedding_managed_with_content(
    project_root: &Path,
    index_store: &IndexStore,
    manager: &EmbeddingManager,
    _changed_path: &Path,
    content: &str,
    language: &str,
    rel_str: &str,
    hnsw_index: Option<&Arc<HnswIndex>>,
) -> Result<(), String> {
    let project_path = project_root.to_string_lossy().to_string();

    if content.len() as u64 > MAX_EMBEDDABLE_FILE_SIZE {
        return Ok(());
    }

    // Mark old HNSW entries as stale (safe even if the API call later fails —
    // a periodic HNSW rebuild will reconcile).
    if let Some(hnsw) = hnsw_index {
        if hnsw.is_ready().await {
            if let Ok(rowids) = index_store.get_embedding_rowids_for_file(&project_path, rel_str) {
                for rowid in rowids {
                    hnsw.mark_stale(rowid).await;
                }
            }
        }
    }

    // If primary provider is TF-IDF, ensure vocabulary is loaded
    if manager.provider_type() == EmbeddingProviderType::TfIdf {
        let primary = manager.primary_provider();
        if let Some(tfidf) = primary.as_any().downcast_ref::<TfIdfEmbeddingProvider>() {
            if !tfidf.is_ready() {
                match index_store.load_vocabulary(&project_path) {
                    Ok(Some(json)) => {
                        if let Err(e) = tfidf.import_vocabulary(&json) {
                            warn!(
                                error = %e,
                                "background indexer: failed to import vocabulary for incremental embedding (managed)"
                            );
                            return Ok(());
                        }
                        info!(
                            "background indexer: restored vocabulary from SQLite for incremental embedding (managed)"
                        );
                    }
                    _ => return Ok(()),
                }
            }
        }
    }

    let chunks = chunk_file_content(content, language);
    if chunks.is_empty() {
        return Ok(());
    }

    let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();

    // Generate embeddings OUTSIDE the transaction (may call external API).
    let embeddings = manager
        .embed_documents(&texts)
        .await
        .map_err(|e| format!("embedding manager incremental embed failed: {}", e))?;

    // Build the batch for the atomic replace.
    let embedding_bytes: Vec<Vec<u8>> = embeddings.iter().map(|e| embedding_to_bytes(e)).collect();
    let batch: Vec<(i64, &str, &[u8])> = chunks
        .iter()
        .zip(embedding_bytes.iter())
        .map(|(chunk, bytes)| (chunk.index as i64, chunk.text.as_str(), bytes.as_slice()))
        .collect();

    // Determine provider metadata for the stored embeddings.
    let provider_type_str = format!("{:?}", manager.provider_type()).to_lowercase();
    let provider_model = manager.display_name().to_string();
    let dim = embeddings.first().map(|e| e.len() as i64).unwrap_or(0);

    // Atomic delete-then-insert in a single transaction.
    if let Err(e) = index_store.replace_file_embeddings(
        &project_path,
        rel_str,
        &batch,
        &provider_type_str,
        &provider_model,
        dim,
    ) {
        warn!(
            file = %rel_str,
            error = %e,
            "background indexer: transactional embedding replace failed (managed)"
        );
        return Err(e.to_string());
    }

    // Insert new embeddings into HNSW.
    if let Some(hnsw) = hnsw_index {
        if hnsw.is_ready().await {
            for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
                if let Ok(Some(rowid)) = index_store.get_embedding_rowid_for_chunk(
                    &project_path,
                    rel_str,
                    chunk.index as i64,
                ) {
                    hnsw.insert(rowid, embedding).await;
                }
            }
        }
    }

    // Check if HNSW needs a full rebuild due to stale ID accumulation (>10%)
    if let Some(hnsw) = hnsw_index {
        if hnsw.is_ready().await && hnsw.needs_rebuild().await {
            info!(
                "background indexer: HNSW stale ratio exceeded 10% (managed), triggering full rebuild"
            );
            if let Err(e) = rebuild_hnsw_after_embedding(index_store, &project_path, hnsw).await {
                warn!(
                    error = %e,
                    "background indexer: periodic HNSW rebuild failed (managed)"
                );
            }
        } else if hnsw.is_ready().await {
            if let Err(e) = hnsw.save_to_disk().await {
                warn!(
                    error = %e,
                    "background indexer: failed to save HNSW after incremental embedding (managed)"
                );
            }
        }
    }

    debug!(
        path = %rel_str,
        chunks = chunks.len(),
        "background indexer: incremental embedding updated (managed)"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// HNSW helpers
// ---------------------------------------------------------------------------

/// Rebuild the HNSW index from all embeddings in SQLite after a full embedding pass.
///
/// Resets the HNSW index, batch-inserts all embeddings, and saves to disk.
/// This is more efficient than inserting one-by-one during the embedding loop.
async fn rebuild_hnsw_after_embedding(
    index_store: &IndexStore,
    project_path: &str,
    hnsw: &Arc<HnswIndex>,
) -> Result<(), String> {
    let all_embeddings = index_store
        .get_all_embedding_ids_and_vectors(project_path)
        .map_err(|e| format!("failed to get embeddings for HNSW rebuild: {}", e))?;

    if all_embeddings.is_empty() {
        debug!("background indexer: no embeddings to insert into HNSW");
        return Ok(());
    }

    // Infer actual dimension from the first vector and update the index
    let actual_dim = all_embeddings[0].1.len();
    hnsw.set_dimension(actual_dim);

    // Reset the HNSW index to clear any stale data from a previous pass
    hnsw.reset().await;

    // Batch insert all embeddings
    hnsw.batch_insert(&all_embeddings).await;

    // Save to disk
    hnsw.save_to_disk().await.map_err(|e| {
        format!("failed to save HNSW index to disk: {}", e)
    })?;

    info!(
        vectors = all_embeddings.len(),
        dimension = actual_dim,
        "background indexer: HNSW index rebuilt and saved to disk"
    );

    Ok(())
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

    /// Helper: create an in-memory IndexStore for testing.
    fn test_store() -> Arc<IndexStore> {
        let db = Database::new_in_memory().expect("in-memory db");
        Arc::new(IndexStore::new(db.pool().clone()))
    }

    // -----------------------------------------------------------------------
    // compute_content_hash
    // -----------------------------------------------------------------------

    #[test]
    fn content_hash_is_deterministic() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("hello.txt");
        fs::write(&file, "hello world").expect("write");

        let h1 = compute_content_hash(&file);
        let h2 = compute_content_hash(&file);
        assert_eq!(h1, h2);
        assert_ne!(h1, "unreadable");
    }

    #[test]
    fn content_hash_changes_when_file_changes() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("data.txt");

        fs::write(&file, "version1").expect("write");
        let h1 = compute_content_hash(&file);

        fs::write(&file, "version2").expect("write");
        let h2 = compute_content_hash(&file);

        assert_ne!(h1, h2);
    }

    #[test]
    fn content_hash_returns_placeholder_for_missing_file() {
        let h = compute_content_hash(Path::new("/nonexistent/file.txt"));
        assert_eq!(h, "unreadable");
    }

    // -----------------------------------------------------------------------
    // Full index populates the store
    // -----------------------------------------------------------------------

    #[test]
    fn full_index_populates_store() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main() {\n    println!(\"hi\");\n}\n",
        )
        .expect("write");
        fs::write(dir.path().join("src/lib.rs"), "pub struct Config;\n").expect("write");

        let store = test_store();
        run_full_index(dir.path(), &store, None).expect("full index");

        let project_path = dir.path().to_string_lossy().to_string();
        let summary = store.get_project_summary(&project_path).expect("summary");
        assert_eq!(summary.total_files, 2, "expected 2 indexed files");
    }

    // -----------------------------------------------------------------------
    // Incremental index only re-indexes changed files
    // -----------------------------------------------------------------------

    #[test]
    fn incremental_index_skips_unchanged_files() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("app.py");
        fs::write(&file, "def main():\n    pass\n").expect("write");

        let store = test_store();

        // Full index first
        run_full_index(dir.path(), &store, None).expect("full index");

        let project_path = dir.path().to_string_lossy().to_string();

        // Record the hash before incremental
        let hash_before = compute_content_hash(&file);
        let stale_before = store
            .is_index_stale(&project_path, "app.py", &hash_before)
            .expect("stale check");
        assert!(!stale_before, "file should NOT be stale after full index");

        // Incremental on the same unchanged file should be a no-op
        run_incremental_index(dir.path(), &store, &file).expect("incremental");

        // Still exactly one file in the index
        let summary = store.get_project_summary(&project_path).expect("summary");
        assert_eq!(summary.total_files, 1);
    }

    #[test]
    fn incremental_index_updates_changed_file() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("lib.py");
        fs::write(&file, "def old_func():\n    pass\n").expect("write");

        let store = test_store();
        run_full_index(dir.path(), &store, None).expect("full index");

        let project_path = dir.path().to_string_lossy().to_string();
        let symbols_v1 = store
            .get_file_symbols(&project_path, "lib.py")
            .expect("symbols");
        assert_eq!(symbols_v1.len(), 1);
        assert_eq!(symbols_v1[0].name, "old_func");

        // Modify the file
        fs::write(&file, "def new_func():\n    pass\n").expect("write");

        // Incremental should detect the change and re-index
        run_incremental_index(dir.path(), &store, &file).expect("incremental");

        let symbols_v2 = store
            .get_file_symbols(&project_path, "lib.py")
            .expect("symbols");
        assert_eq!(symbols_v2.len(), 1);
        assert_eq!(symbols_v2[0].name, "new_func");
    }

    #[test]
    fn incremental_index_handles_deleted_file() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("temp.py");
        fs::write(&file, "x = 1\n").expect("write");

        let store = test_store();
        run_full_index(dir.path(), &store, None).expect("full index");

        // Delete the file
        fs::remove_file(&file).expect("remove");

        // Incremental should succeed (skip gracefully)
        let result = run_incremental_index(dir.path(), &store, &file);
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Progress callback
    // -----------------------------------------------------------------------

    #[test]
    fn progress_callback_is_invoked_during_full_index() {
        let dir = tempdir().expect("tempdir");
        // Create 25 files so we get at least two progress callbacks (at 10 and 20)
        // plus the final call at 25.
        for i in 0..25 {
            fs::write(
                dir.path().join(format!("file_{i}.py")),
                format!("x_{i} = {i}\n"),
            )
            .expect("write");
        }

        let store = test_store();
        let calls: Arc<std::sync::Mutex<Vec<(usize, usize)>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let calls_clone = calls.clone();
        let cb: IndexProgressCallback = Arc::new(move |done, total| {
            calls_clone.lock().unwrap().push((done, total));
        });

        run_full_index(dir.path(), &store, Some(&cb)).expect("full index");

        let recorded = calls.lock().unwrap();
        // Should have calls at 10, 20, and final (25)
        assert!(
            recorded.len() >= 3,
            "expected at least 3 progress calls, got {}",
            recorded.len()
        );
        // First periodic call should be (10, 25)
        assert_eq!(recorded[0], (10, 25));
        // Second periodic call should be (20, 25)
        assert_eq!(recorded[1], (20, 25));
        // Last call should be (total, total)
        let last = recorded.last().unwrap();
        assert_eq!(last.0, last.1, "final callback should have done == total");
        assert_eq!(last.1, 25);
    }

    #[test]
    fn progress_callback_not_called_for_empty_project() {
        let dir = tempdir().expect("tempdir");
        // No files created - empty directory

        let store = test_store();
        let calls: Arc<std::sync::Mutex<Vec<(usize, usize)>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let calls_clone = calls.clone();
        let cb: IndexProgressCallback = Arc::new(move |done, total| {
            calls_clone.lock().unwrap().push((done, total));
        });

        run_full_index(dir.path(), &store, Some(&cb)).expect("full index");

        let recorded = calls.lock().unwrap();
        // Final callback with (0, 0) is expected
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0], (0, 0));
    }

    #[tokio::test]
    async fn start_invokes_progress_callback() {
        let dir = tempdir().expect("tempdir");
        for i in 0..15 {
            fs::write(
                dir.path().join(format!("mod_{i}.py")),
                format!("val_{i} = {i}\n"),
            )
            .expect("write");
        }

        let store = test_store();
        let calls: Arc<std::sync::Mutex<Vec<(usize, usize)>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let calls_clone = calls.clone();
        let cb: IndexProgressCallback = Arc::new(move |done, total| {
            calls_clone.lock().unwrap().push((done, total));
        });

        let indexer = BackgroundIndexer::new(dir.path().to_path_buf(), store.clone())
            .with_progress_callback(cb);

        let handle = indexer.start().await;
        handle.await.expect("task should complete");

        let recorded = calls.lock().unwrap();
        // At least one periodic call (at 10) plus the final call
        assert!(
            recorded.len() >= 2,
            "expected at least 2 progress calls, got {}",
            recorded.len()
        );
        // Last call should be (total, total)
        let last = recorded.last().unwrap();
        assert_eq!(last.0, last.1);
    }

    // -----------------------------------------------------------------------
    // BackgroundIndexer::start spawns a non-blocking task
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn start_completes_full_index_in_background() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("main.py"), "def entry():\n    pass\n").expect("write");

        let store = test_store();
        let indexer = BackgroundIndexer::new(dir.path().to_path_buf(), store.clone());

        let handle = indexer.start().await;
        // The task should complete since there is no change_rx
        handle.await.expect("task should complete");

        let project_path = dir.path().to_string_lossy().to_string();
        let summary = store.get_project_summary(&project_path).expect("summary");
        assert_eq!(summary.total_files, 1);
    }

    // -----------------------------------------------------------------------
    // Chunking tests (story-003)
    // -----------------------------------------------------------------------

    #[test]
    fn chunk_by_window_basic() {
        let content = (0..100)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let lines: Vec<&str> = content.lines().collect();
        let chunks = chunk_by_window(&lines);

        assert!(!chunks.is_empty());
        // First chunk should have CHUNK_MAX_LINES lines
        let first_lines: Vec<&str> = chunks[0].text.lines().collect();
        assert_eq!(first_lines.len(), CHUNK_MAX_LINES);
        // Chunk indices should be sequential
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
        }
    }

    #[test]
    fn chunk_by_window_small_file() {
        let content = "line1\nline2\nline3";
        let lines: Vec<&str> = content.lines().collect();
        let chunks = chunk_by_window(&lines);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, content);
    }

    #[test]
    fn chunk_file_content_empty() {
        let chunks = chunk_file_content("", "rust");
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_file_content_unsupported_language() {
        let content = (0..100)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_file_content(&content, "unknown");
        // Should fall back to window chunking
        assert!(!chunks.is_empty());
    }

    #[test]
    fn chunk_file_content_rust_with_symbols() {
        let content = r#"
use std::collections::HashMap;

/// A configuration struct.
pub struct Config {
    name: String,
    value: i32,
}

impl Config {
    pub fn new(name: String) -> Self {
        Self { name, value: 0 }
    }

    pub fn set_value(&mut self, v: i32) {
        self.value = v;
    }
}

/// Process the config.
pub fn process_config(config: &Config) -> String {
    format!("Config: {}", config.name)
}
"#;
        let chunks = chunk_file_content(content, "rust");
        assert!(!chunks.is_empty(), "should produce at least one chunk");
        // All original content should be represented across chunks
        let total_text: String = chunks
            .iter()
            .map(|c| c.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(total_text.contains("Config"));
        assert!(total_text.contains("process_config"));
    }

    // -----------------------------------------------------------------------
    // Embedding pass integration test
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn embedding_pass_stores_embeddings() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main() {\n    println!(\"hello\");\n}\n\npub fn helper() {\n    // do stuff\n}\n",
        )
        .expect("write");
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub struct Config {\n    name: String,\n}\n",
        )
        .expect("write");

        let store = test_store();
        // First run full index so files are in file_index table
        run_full_index(dir.path(), &store, None).expect("full index");

        let emb_svc = EmbeddingService::new();
        run_embedding_pass(dir.path(), &store, &emb_svc, None)
            .await
            .expect("embedding pass");

        let project_path = dir.path().to_string_lossy().to_string();
        let count = store.count_embeddings(&project_path).expect("count");
        assert!(
            count > 0,
            "should have stored at least one embedding, got {}",
            count
        );

        let embeddings = store
            .get_embeddings_for_project(&project_path)
            .expect("get");
        assert!(!embeddings.is_empty());
        // Each embedding should have non-empty bytes
        for (_, _, _, bytes) in &embeddings {
            assert!(!bytes.is_empty(), "embedding bytes should not be empty");
        }
    }

    // -----------------------------------------------------------------------
    // Embedding pass saves vocabulary to SQLite (story-003)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn embedding_pass_saves_vocabulary_to_sqlite() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main() {\n    println!(\"hello\");\n}\n\npub fn helper() {\n    // stuff\n}\n",
        )
        .expect("write");
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub struct Config {\n    name: String,\n}\n",
        )
        .expect("write");

        let store = test_store();
        run_full_index(dir.path(), &store, None).expect("full index");

        let emb_svc = EmbeddingService::new();
        run_embedding_pass(dir.path(), &store, &emb_svc, None)
            .await
            .expect("embedding pass");

        let project_path = dir.path().to_string_lossy().to_string();
        let vocab_json = store.load_vocabulary(&project_path).expect("load vocab");
        assert!(
            vocab_json.is_some(),
            "vocabulary should be saved after embedding pass"
        );

        // Verify the saved vocabulary can be imported into a fresh service
        let fresh_svc = EmbeddingService::new();
        fresh_svc
            .import_vocabulary(&vocab_json.unwrap())
            .expect("import should succeed");
        assert!(
            fresh_svc.is_ready(),
            "fresh service should be ready after import"
        );

        // Verify the imported service produces valid embeddings
        let vec = fresh_svc.embed_text("pub fn main()");
        assert!(!vec.is_empty(), "embed_text should produce a vector");
    }

    // -----------------------------------------------------------------------
    // Incremental embedding restores vocabulary from DB (story-004)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn incremental_embedding_restores_vocab_from_db_when_not_ready() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main() {\n    println!(\"hello\");\n}\n",
        )
        .expect("write");
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub struct Config {\n    name: String,\n}\n",
        )
        .expect("write");

        let store = test_store();
        let project_path = dir.path().to_string_lossy().to_string();

        // First: full index + embedding pass to build and save vocabulary
        run_full_index(dir.path(), &store, None).expect("full index");
        let emb_svc1 = EmbeddingService::new();
        run_embedding_pass(dir.path(), &store, &emb_svc1, None)
            .await
            .expect("embedding pass");

        // Verify vocab was saved
        assert!(
            store.load_vocabulary(&project_path).unwrap().is_some(),
            "vocabulary should be saved"
        );

        // Create a FRESH embedding service (simulating app restart — vocab not in memory)
        let emb_svc2 = EmbeddingService::new();
        assert!(!emb_svc2.is_ready(), "fresh service should not be ready");

        // Modify a file
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main_v2() {\n    println!(\"updated\");\n}\n",
        )
        .expect("write");

        // Incremental embedding should restore vocab from DB and proceed
        let changed_path = dir.path().join("src/main.rs");
        run_incremental_embedding(dir.path(), &store, &emb_svc2, &changed_path, None)
            .await
            .expect("incremental embedding");

        // Verify the service is now ready (vocab was restored)
        assert!(
            emb_svc2.is_ready(),
            "service should be ready after incremental restores vocab"
        );

        // Verify embeddings were stored for the changed file
        let embeddings = store.get_embeddings_for_project(&project_path).unwrap();
        let main_embeddings: Vec<_> = embeddings
            .iter()
            .filter(|(path, _, _, _)| path.contains("main.rs"))
            .collect();
        assert!(
            !main_embeddings.is_empty(),
            "should have embeddings for main.rs after incremental"
        );
    }

    #[tokio::test]
    async fn incremental_embedding_skips_when_no_vocab_in_db() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("test.py"), "x = 1\n").expect("write");

        let store = test_store();

        // Full index but NO embedding pass — so no vocab in DB
        run_full_index(dir.path(), &store, None).expect("full index");

        let emb_svc = EmbeddingService::new();
        assert!(!emb_svc.is_ready());

        // Incremental embedding should skip gracefully (no panic, no error)
        let changed_path = dir.path().join("test.py");
        let result =
            run_incremental_embedding(dir.path(), &store, &emb_svc, &changed_path, None).await;
        assert!(result.is_ok(), "should succeed even without vocab in DB");
        assert!(!emb_svc.is_ready(), "service should remain not-ready");
    }

    #[tokio::test]
    async fn start_processes_incremental_changes() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("service.py");
        fs::write(&file, "def serve():\n    pass\n").expect("write");

        let store = test_store();
        let (tx, rx) = tokio::sync::mpsc::channel::<PathBuf>(16);

        let indexer = BackgroundIndexer::new(dir.path().to_path_buf(), store.clone())
            .with_change_receiver(rx);

        let handle = indexer.start().await;

        // Modify the file and send the change event
        fs::write(&file, "def serve_v2():\n    pass\n").expect("write");
        tx.send(file.clone()).await.expect("send");

        // Drop the sender to close the channel so the task exits
        drop(tx);
        handle.await.expect("task should complete");

        let project_path = dir.path().to_string_lossy().to_string();
        let symbols = store
            .get_file_symbols(&project_path, "service.py")
            .expect("symbols");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "serve_v2");
    }

    // -----------------------------------------------------------------------
    // EmbeddingManager integration tests (story-005)
    // -----------------------------------------------------------------------

    /// Helper: create an EmbeddingManager with TF-IDF as primary provider.
    fn test_tfidf_manager() -> Arc<EmbeddingManager> {
        use crate::services::orchestrator::embedding_manager::EmbeddingManagerConfig;
        use crate::services::orchestrator::embedding_provider::{
            EmbeddingProviderConfig, EmbeddingProviderType,
        };

        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        Arc::new(EmbeddingManager::from_config(config).expect("create manager"))
    }

    #[tokio::test]
    async fn managed_embedding_pass_stores_embeddings() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main() {\n    println!(\"hello\");\n}\n\npub fn helper() {\n    // do stuff\n}\n",
        )
        .expect("write");
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub struct Config {\n    name: String,\n}\n",
        )
        .expect("write");

        let store = test_store();
        run_full_index(dir.path(), &store, None).expect("full index");

        let manager = test_tfidf_manager();
        run_embedding_pass_managed(dir.path(), &store, &manager, None)
            .await
            .expect("managed embedding pass");

        let project_path = dir.path().to_string_lossy().to_string();
        let count = store.count_embeddings(&project_path).expect("count");
        assert!(
            count > 0,
            "should have stored at least one embedding via manager, got {}",
            count
        );

        let embeddings = store
            .get_embeddings_for_project(&project_path)
            .expect("get");
        assert!(!embeddings.is_empty());
        for (_, _, _, bytes) in &embeddings {
            assert!(!bytes.is_empty(), "embedding bytes should not be empty");
        }
    }

    #[tokio::test]
    async fn managed_embedding_pass_saves_vocabulary_to_sqlite() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main() {\n    println!(\"hello\");\n}\n\npub fn helper() {\n    // stuff\n}\n",
        )
        .expect("write");
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub struct Config {\n    name: String,\n}\n",
        )
        .expect("write");

        let store = test_store();
        run_full_index(dir.path(), &store, None).expect("full index");

        let manager = test_tfidf_manager();
        run_embedding_pass_managed(dir.path(), &store, &manager, None)
            .await
            .expect("managed embedding pass");

        let project_path = dir.path().to_string_lossy().to_string();
        let vocab_json = store.load_vocabulary(&project_path).expect("load vocab");
        assert!(
            vocab_json.is_some(),
            "vocabulary should be saved after managed embedding pass"
        );
    }

    #[tokio::test]
    async fn managed_incremental_embedding_works() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main() {\n    println!(\"hello\");\n}\n",
        )
        .expect("write");
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub struct Config {\n    name: String,\n}\n",
        )
        .expect("write");

        let store = test_store();
        let project_path = dir.path().to_string_lossy().to_string();

        // Full index + managed embedding pass to build and save vocabulary
        run_full_index(dir.path(), &store, None).expect("full index");
        let manager = test_tfidf_manager();
        run_embedding_pass_managed(dir.path(), &store, &manager, None)
            .await
            .expect("managed embedding pass");

        // Verify vocab was saved
        assert!(
            store.load_vocabulary(&project_path).unwrap().is_some(),
            "vocabulary should be saved"
        );

        // Create a FRESH manager (simulating app restart)
        let manager2 = test_tfidf_manager();

        // Verify the fresh manager's TF-IDF provider is not ready
        let primary = manager2.primary_provider();
        let tfidf = primary
            .as_any()
            .downcast_ref::<TfIdfEmbeddingProvider>()
            .expect("should be TfIdfEmbeddingProvider");
        assert!(!tfidf.is_ready(), "fresh manager should not be ready");

        // Modify a file
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main_v2() {\n    println!(\"updated\");\n}\n",
        )
        .expect("write");

        // Incremental embedding should restore vocab from DB and proceed
        let changed_path = dir.path().join("src/main.rs");
        run_incremental_embedding_managed(dir.path(), &store, &manager2, &changed_path, None)
            .await
            .expect("managed incremental embedding");

        // Verify the provider is now ready (vocab was restored)
        assert!(
            tfidf.is_ready(),
            "TF-IDF provider should be ready after incremental restores vocab"
        );

        // Verify embeddings were stored for the changed file
        let embeddings = store.get_embeddings_for_project(&project_path).unwrap();
        let main_embeddings: Vec<_> = embeddings
            .iter()
            .filter(|(path, _, _, _)| path.contains("main.rs"))
            .collect();
        assert!(
            !main_embeddings.is_empty(),
            "should have embeddings for main.rs after managed incremental"
        );
    }

    #[tokio::test]
    async fn managed_incremental_embedding_skips_when_no_vocab_in_db() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("test.py"), "x = 1\n").expect("write");

        let store = test_store();

        // Full index but NO embedding pass — so no vocab in DB
        run_full_index(dir.path(), &store, None).expect("full index");

        let manager = test_tfidf_manager();
        let primary = manager.primary_provider();
        let tfidf = primary
            .as_any()
            .downcast_ref::<TfIdfEmbeddingProvider>()
            .expect("TfIdfEmbeddingProvider");
        assert!(!tfidf.is_ready());

        // Incremental embedding should skip gracefully (no panic, no error)
        let changed_path = dir.path().join("test.py");
        let result =
            run_incremental_embedding_managed(dir.path(), &store, &manager, &changed_path, None)
                .await;
        assert!(result.is_ok(), "should succeed even without vocab in DB");
        assert!(
            !tfidf.is_ready(),
            "TF-IDF provider should remain not-ready"
        );
    }

    #[tokio::test]
    async fn start_with_embedding_manager_generates_embeddings() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main() {\n    println!(\"hello\");\n}\n",
        )
        .expect("write");

        let store = test_store();
        let manager = test_tfidf_manager();

        let indexer = BackgroundIndexer::new(dir.path().to_path_buf(), store.clone())
            .with_embedding_manager(manager);

        let handle = indexer.start().await;
        handle.await.expect("task should complete");

        let project_path = dir.path().to_string_lossy().to_string();
        let count = store.count_embeddings(&project_path).expect("count");
        assert!(
            count > 0,
            "should have stored embeddings via manager, got {}",
            count
        );
    }

    #[tokio::test]
    async fn embedding_manager_takes_precedence_over_service() {
        // When both embedding_service and embedding_manager are set,
        // the manager should be used (and produce embeddings).
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main() {\n    println!(\"hello\");\n}\n",
        )
        .expect("write");

        let store = test_store();
        let emb_svc = Arc::new(EmbeddingService::new());
        let manager = test_tfidf_manager();

        let indexer = BackgroundIndexer::new(dir.path().to_path_buf(), store.clone())
            .with_embedding_service(emb_svc.clone())
            .with_embedding_manager(manager);

        let handle = indexer.start().await;
        handle.await.expect("task should complete");

        let project_path = dir.path().to_string_lossy().to_string();
        let count = store.count_embeddings(&project_path).expect("count");
        assert!(
            count > 0,
            "should have stored embeddings via manager, got {}",
            count
        );

        // The direct EmbeddingService should NOT have been used (vocab not built)
        assert!(
            !emb_svc.is_ready(),
            "direct EmbeddingService should not have been used when manager is set"
        );
    }

    // -----------------------------------------------------------------------
    // Story-005 Integration Tests: HNSW + brute-force comparison
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn hnsw_results_match_brute_force_semantic_search() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        // Create several files with distinct content for meaningful embeddings
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn main() {\n    println!(\"hello world\");\n    run_server();\n}\n",
        )
        .expect("write");
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub struct Config {\n    name: String,\n    port: u16,\n}\n\nimpl Config {\n    pub fn new() -> Self {\n        Self { name: String::new(), port: 8080 }\n    }\n}\n",
        )
        .expect("write");
        fs::write(
            dir.path().join("src/server.rs"),
            "pub fn run_server() {\n    let config = Config::new();\n    start_listener(config.port);\n}\n\nfn start_listener(port: u16) {\n    // bind to port\n}\n",
        )
        .expect("write");

        let store = test_store();
        let project_path = dir.path().to_string_lossy().to_string();

        // Full index + embedding pass
        run_full_index(dir.path(), &store, None).expect("full index");
        let emb_svc = EmbeddingService::new();
        run_embedding_pass(dir.path(), &store, &emb_svc, None)
            .await
            .expect("embedding pass");

        // Build HNSW from the stored embeddings
        let hnsw_dir = dir.path().join("hnsw");
        let all_embeddings = store
            .get_all_embedding_ids_and_vectors(&project_path)
            .expect("get all embeddings");
        assert!(!all_embeddings.is_empty(), "should have embeddings");

        // Determine dimension from first embedding
        let dimension = all_embeddings[0].1.len();
        let hnsw = Arc::new(HnswIndex::new(&hnsw_dir, dimension));
        hnsw.initialize().await;
        hnsw.batch_insert(&all_embeddings).await;

        // Generate a query embedding
        let query_text = "pub fn main";
        let query_embedding = emb_svc.embed_text(query_text);
        assert!(!query_embedding.is_empty(), "query embedding should not be empty");

        let top_k = all_embeddings.len(); // Request all results to measure recall

        // Brute-force search via IndexStore
        let brute_results = store
            .semantic_search(&query_embedding, &project_path, top_k)
            .expect("brute force search");

        // HNSW search
        let hnsw_results = hnsw.search(&query_embedding, top_k).await;

        // Both should return results
        assert!(
            !brute_results.is_empty(),
            "brute force should return results"
        );
        assert!(
            !hnsw_results.is_empty(),
            "HNSW should return results"
        );

        // Both should return the same number of results
        assert_eq!(
            hnsw_results.len(),
            brute_results.len(),
            "HNSW and brute-force should return same number of results"
        );

        // Check recall: all brute-force files should appear in HNSW results.
        // Since HNSW is approximate, we check that recall is >= 95%.
        let hnsw_rowids: Vec<usize> = hnsw_results.iter().map(|(id, _)| *id).collect();
        let hnsw_metadata = store.get_embeddings_by_rowids(&hnsw_rowids).expect("metadata");
        let hnsw_files: std::collections::HashSet<&String> = hnsw_metadata
            .values()
            .map(|(fp, _, _)| fp)
            .collect();

        let brute_files: std::collections::HashSet<&String> = brute_results
            .iter()
            .map(|r| &r.file_path)
            .collect();

        // All files found by brute-force should also appear in HNSW results
        let recall_files = brute_files
            .iter()
            .filter(|f| hnsw_files.contains(*f))
            .count();
        let total_files = brute_files.len();

        assert!(
            recall_files == total_files,
            "HNSW should find all the same files as brute-force. \
             Found {}/{} files. HNSW files: {:?}, brute files: {:?}",
            recall_files,
            total_files,
            hnsw_files,
            brute_files,
        );

        // Verify HNSW distances are valid (non-negative, within expected range)
        for (id, distance) in &hnsw_results {
            assert!(
                *distance >= 0.0 && *distance <= 2.0,
                "HNSW cosine distance for id {} should be in [0, 2], got {}",
                id,
                distance
            );
        }

        // Verify brute-force similarities are valid
        for result in &brute_results {
            assert!(
                result.similarity >= -1.0 && result.similarity <= 1.0,
                "brute-force similarity for {} should be in [-1, 1], got {}",
                result.file_path,
                result.similarity
            );
        }
    }

    // -----------------------------------------------------------------------
    // Story-005: Stale ID exclusion after incremental file update
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn incremental_update_marks_old_embeddings_stale_in_hnsw() {
        // This test directly verifies the stale ID tracking mechanism:
        // 1. Build embeddings and HNSW index
        // 2. Manually mark old rowids as stale (simulating what run_incremental_embedding does)
        // 3. Verify stale IDs are excluded from search results
        // 4. Verify new embeddings appear in search results

        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        fs::write(
            dir.path().join("src/main.rs"),
            "pub fn old_function() {\n    println!(\"old code\");\n}\n",
        )
        .expect("write");
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub struct Config {\n    name: String,\n}\n",
        )
        .expect("write");

        let store = test_store();
        let project_path = dir.path().to_string_lossy().to_string();

        // Full index + embedding pass
        run_full_index(dir.path(), &store, None).expect("full index");
        let emb_svc = EmbeddingService::new();
        run_embedding_pass(dir.path(), &store, &emb_svc, None)
            .await
            .expect("embedding pass");

        // Build HNSW index from stored embeddings
        let hnsw_dir = dir.path().join("hnsw");
        let all_embeddings = store
            .get_all_embedding_ids_and_vectors(&project_path)
            .expect("get embeddings");
        assert!(!all_embeddings.is_empty(), "should have embeddings");
        let dimension = all_embeddings[0].1.len();
        let hnsw = Arc::new(HnswIndex::new(&hnsw_dir, dimension));
        hnsw.initialize().await;
        hnsw.batch_insert(&all_embeddings).await;

        // Identify which rowids belong to main.rs
        let all_rowids: Vec<usize> = all_embeddings.iter().map(|(id, _)| *id).collect();
        let all_metadata = store.get_embeddings_by_rowids(&all_rowids).expect("metadata");
        let main_rs_rowids: Vec<usize> = all_metadata
            .iter()
            .filter(|(_, (fp, _, _))| fp.contains("main.rs"))
            .map(|(id, _)| *id)
            .collect();
        assert!(
            !main_rs_rowids.is_empty(),
            "should have embeddings for main.rs"
        );

        // Verify main.rs embeddings appear in search results before marking stale
        let query = emb_svc.embed_text("pub fn old_function");
        let results_before = hnsw.search(&query, all_embeddings.len()).await;
        let before_ids: Vec<usize> = results_before.iter().map(|(id, _)| *id).collect();
        assert!(
            main_rs_rowids.iter().any(|id| before_ids.contains(id)),
            "main.rs embeddings should appear in search before marking stale"
        );

        // Mark main.rs embeddings as stale (simulating incremental update)
        for rowid in &main_rs_rowids {
            hnsw.mark_stale(*rowid).await;
        }

        let stale_count = hnsw.get_stale_count().await;
        assert!(
            stale_count > 0,
            "should have stale IDs after marking, got {}",
            stale_count
        );
        assert_eq!(
            stale_count,
            main_rs_rowids.len(),
            "stale count should match number of main.rs embeddings"
        );

        // Verify main.rs embeddings are filtered from search results
        let results_after = hnsw.search(&query, all_embeddings.len()).await;
        let after_ids: Vec<usize> = results_after.iter().map(|(id, _)| *id).collect();
        assert!(
            !main_rs_rowids.iter().any(|id| after_ids.contains(id)),
            "main.rs embeddings should be filtered from search after marking stale"
        );

        // Verify that non-stale embeddings (lib.rs) still appear
        let lib_rs_rowids: Vec<usize> = all_metadata
            .iter()
            .filter(|(_, (fp, _, _))| fp.contains("lib.rs"))
            .map(|(id, _)| *id)
            .collect();
        assert!(
            lib_rs_rowids.iter().any(|id| after_ids.contains(id)),
            "lib.rs embeddings should still appear in search (not stale)"
        );

        // Insert new embeddings for updated main.rs and verify they appear
        let new_text = "pub fn new_function";
        let new_embedding = emb_svc.embed_text(new_text);
        let new_id = 9999; // Use a distinct ID
        hnsw.insert(new_id, &new_embedding).await;

        let results_with_new = hnsw.search(&new_embedding, 1).await;
        assert!(!results_with_new.is_empty(), "should find the new embedding");
        assert_eq!(
            results_with_new[0].0, new_id,
            "top result should be the newly inserted embedding"
        );
    }

    // -----------------------------------------------------------------------
    // Story-005: Periodic rebuild triggered when stale ratio exceeds 10%
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn periodic_rebuild_triggered_when_stale_threshold_exceeded() {
        let dir = tempdir().expect("tempdir");
        // Create many small files so we have enough embeddings to test the 10% threshold
        fs::create_dir_all(dir.path().join("src")).expect("mkdir");
        for i in 0..12 {
            fs::write(
                dir.path().join(format!("src/mod_{}.rs", i)),
                format!(
                    "pub fn function_{}() {{\n    println!(\"hello from {}\");\n}}\n",
                    i, i
                ),
            )
            .expect("write");
        }

        let store = test_store();
        let project_path = dir.path().to_string_lossy().to_string();

        // Full index + embedding pass
        run_full_index(dir.path(), &store, None).expect("full index");
        let emb_svc = EmbeddingService::new();
        run_embedding_pass(dir.path(), &store, &emb_svc, None)
            .await
            .expect("embedding pass");

        // Build HNSW
        let hnsw_dir = dir.path().join("hnsw");
        let all_embeddings = store
            .get_all_embedding_ids_and_vectors(&project_path)
            .expect("get embeddings");
        assert!(
            all_embeddings.len() >= 10,
            "need at least 10 embeddings for threshold test, got {}",
            all_embeddings.len()
        );
        let dimension = all_embeddings[0].1.len();
        let hnsw = Arc::new(HnswIndex::new(&hnsw_dir, dimension));
        hnsw.initialize().await;
        hnsw.batch_insert(&all_embeddings).await;

        let total_before = hnsw.get_count().await;
        assert!(!hnsw.needs_rebuild().await, "should not need rebuild initially");

        // Mark >10% as stale manually
        let stale_target = (total_before as f64 * 0.11).ceil() as usize;
        for i in 0..stale_target {
            if i < all_embeddings.len() {
                hnsw.mark_stale(all_embeddings[i].0).await;
            }
        }

        assert!(
            hnsw.needs_rebuild().await,
            "should need rebuild after marking >10% as stale"
        );

        // Simulate what the incremental function does: check and rebuild
        if hnsw.needs_rebuild().await {
            rebuild_hnsw_after_embedding(&store, &project_path, &hnsw)
                .await
                .expect("rebuild should succeed");
        }

        // After rebuild: stale count should be 0, count should be restored
        assert_eq!(
            hnsw.get_stale_count().await,
            0,
            "stale count should be 0 after rebuild"
        );
        assert!(
            hnsw.get_count().await > 0,
            "should have vectors after rebuild"
        );
        assert!(
            !hnsw.needs_rebuild().await,
            "should not need rebuild after fresh rebuild"
        );

        // Verify search still works after rebuild
        let query = emb_svc.embed_text("pub fn function_5");
        let results = hnsw.search(&query, 5).await;
        assert!(
            !results.is_empty(),
            "search should return results after rebuild"
        );
    }

    // -----------------------------------------------------------------------
    // Story-005: trigger_reindex creates fresh HNSW index (IndexManager test)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn trigger_reindex_creates_fresh_hnsw() {
        let dim = 16;
        let hnsw_dir = tempdir().expect("tempdir");
        let hnsw = Arc::new(HnswIndex::new(hnsw_dir.path().join("hnsw"), dim));
        hnsw.initialize().await;

        // Insert some vectors and mark some as stale
        for i in 0..20 {
            let mut v = vec![0.0f32; dim];
            v[i % dim] = 1.0;
            hnsw.insert(i, &v).await;
        }
        for i in 0..5 {
            hnsw.mark_stale(i).await;
        }

        assert_eq!(hnsw.get_count().await, 20);
        assert_eq!(hnsw.get_stale_count().await, 5);
        assert!(hnsw.needs_rebuild().await); // 5/20 = 25% > 10%

        // Reset (simulating what trigger_reindex does: discard old, create fresh)
        hnsw.reset().await;

        assert_eq!(hnsw.get_count().await, 0);
        assert_eq!(hnsw.get_stale_count().await, 0);
        assert!(hnsw.is_ready().await);
        assert!(!hnsw.needs_rebuild().await);

        // Re-insert to verify the fresh index works
        for i in 0..10 {
            let mut v = vec![0.0f32; dim];
            v[i % dim] = 1.0;
            hnsw.insert(i, &v).await;
        }
        assert_eq!(hnsw.get_count().await, 10);

        let mut query = vec![0.0f32; dim];
        query[3] = 1.0;
        let results = hnsw.search(&query, 1).await;
        assert!(!results.is_empty(), "search should work on fresh index");
        assert_eq!(results[0].0, 3, "should find the matching vector");
    }

    // -----------------------------------------------------------------------
    // Gitignore filtering tests
    // -----------------------------------------------------------------------

    #[test]
    fn gitignore_filtering_skips_ignored_paths() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // Write a .gitignore that ignores node_modules/ and dist/
        fs::write(root.join(".gitignore"), "node_modules/\ndist/\n").expect("write gitignore");

        // Create matching and non-matching files
        fs::create_dir_all(root.join("node_modules/pkg")).expect("mkdir");
        fs::write(root.join("node_modules/pkg/index.js"), "module.exports = {}").expect("write");
        fs::create_dir_all(root.join("dist")).expect("mkdir");
        fs::write(root.join("dist/bundle.js"), "var a=1;").expect("write");
        fs::create_dir_all(root.join("src")).expect("mkdir");
        fs::write(root.join("src/main.rs"), "fn main() {}").expect("write");

        let matcher = build_gitignore_matcher(root);

        assert!(
            is_ignored_by_gitignore(&matcher, root, &root.join("node_modules/pkg/index.js")),
            "node_modules file should be ignored"
        );
        assert!(
            is_ignored_by_gitignore(&matcher, root, &root.join("dist/bundle.js")),
            "dist file should be ignored"
        );
        assert!(
            !is_ignored_by_gitignore(&matcher, root, &root.join("src/main.rs")),
            "src file should NOT be ignored"
        );
    }

    #[test]
    fn gitignore_filtering_handles_missing_gitignore() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // No .gitignore file — matcher should not ignore anything
        fs::create_dir_all(root.join("src")).expect("mkdir");
        fs::write(root.join("src/main.rs"), "fn main() {}").expect("write");
        fs::create_dir_all(root.join("node_modules/pkg")).expect("mkdir");
        fs::write(root.join("node_modules/pkg/index.js"), "module.exports = {}").expect("write");

        let matcher = build_gitignore_matcher(root);

        assert!(
            !is_ignored_by_gitignore(&matcher, root, &root.join("src/main.rs")),
            "should not ignore when no .gitignore exists"
        );
        assert!(
            !is_ignored_by_gitignore(&matcher, root, &root.join("node_modules/pkg/index.js")),
            "should not ignore when no .gitignore exists"
        );
    }

    #[tokio::test]
    async fn start_watch_only_skips_gitignored_incremental_changes() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // Write a .gitignore that ignores node_modules/
        fs::write(root.join(".gitignore"), "node_modules/\n").expect("write gitignore");

        // Pre-create files so incremental indexing has something to work with
        fs::write(root.join("main.py"), "def main():\n    pass\n").expect("write");
        fs::create_dir_all(root.join("node_modules")).expect("mkdir");
        fs::write(root.join("node_modules/foo.js"), "var x = 1;").expect("write");

        let store = test_store();
        let (tx, rx) = tokio::sync::mpsc::channel::<PathBuf>(16);

        // Use start_watch_only to test only the incremental loop (no full index)
        let indexer = BackgroundIndexer::new(root.to_path_buf(), store.clone())
            .with_change_receiver(rx);

        let handle = indexer.start_watch_only().await;

        // Send an ignored path — it should be skipped
        tx.send(root.join("node_modules/foo.js")).await.expect("send");

        // Send a normal file change
        tx.send(root.join("main.py")).await.expect("send");

        // Close the channel so the task exits
        drop(tx);
        handle.await.expect("task should complete");

        let project_path = root.to_string_lossy().to_string();

        // main.py should be indexed
        let symbols = store.get_file_symbols(&project_path, "main.py").expect("symbols");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "main");

        // node_modules/foo.js should NOT be in the index
        let nm_symbols = store
            .get_file_symbols(&project_path, "node_modules/foo.js")
            .expect("symbols");
        assert!(
            nm_symbols.is_empty(),
            "gitignored file should not be indexed"
        );
    }
}
