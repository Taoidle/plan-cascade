//! Background Indexing with File Watcher Integration
//!
//! Runs file indexing in a background tokio task so that the main execution
//! thread is never blocked.  On start a full inventory is built, and afterwards
//! the indexer listens on an optional `mpsc` channel for incremental updates
//! triggered by file-watcher events.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::analysis_index::{build_file_inventory, extract_symbols, AnalysisLimits};
use super::index_store::IndexStore;

/// Callback type for reporting indexing progress.
///
/// Called with `(indexed_so_far, total_files)` during a full index pass.
pub type IndexProgressCallback = Arc<dyn Fn(usize, usize) + Send + Sync>;

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
    change_rx: Option<tokio::sync::mpsc::Receiver<PathBuf>>,
    progress_callback: Option<IndexProgressCallback>,
}

impl BackgroundIndexer {
    /// Create a new background indexer for the given project root.
    pub fn new(project_root: PathBuf, index_store: Arc<IndexStore>) -> Self {
        Self {
            project_root,
            index_store,
            change_rx: None,
            progress_callback: None,
        }
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

    /// Spawn the background indexing task and return its `JoinHandle`.
    ///
    /// The task:
    /// 1. Performs a full index of the project on start.
    /// 2. Enters a loop listening for incremental change events (if a
    ///    receiver was provided).
    ///
    /// Errors during indexing are logged but never propagated; the task
    /// keeps running so that future change events are still processed.
    pub async fn start(self) -> tokio::task::JoinHandle<()> {
        let project_root = self.project_root;
        let index_store = self.index_store;
        let change_rx = self.change_rx;
        let progress_callback = self.progress_callback;

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

            // --- Phase 2: Incremental updates ---
            if let Some(mut rx) = change_rx {
                debug!("background indexer: listening for incremental changes");
                while let Some(changed_path) = rx.recv().await {
                    if let Err(e) =
                        run_incremental_index(&project_root, &index_store, &changed_path)
                    {
                        warn!(
                            path = %changed_path.display(),
                            error = %e,
                            "background indexer: incremental index failed"
                        );
                    }
                }
                debug!("background indexer: change channel closed, stopping");
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

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

    info!(
        files = total_files,
        "background indexer: full index stored"
    );
    Ok(())
}

/// Re-index a single file if its content hash is stale.
fn run_incremental_index(
    project_root: &Path,
    index_store: &IndexStore,
    changed_path: &Path,
) -> Result<(), String> {
    let rel = changed_path
        .strip_prefix(project_root)
        .map_err(|_| format!("path {:?} is not under project root", changed_path))?;

    let rel_str = rel.to_string_lossy().replace('\\', "/");
    let project_path = project_root.to_string_lossy().to_string();

    // If the file no longer exists we skip silently (it was deleted).
    if !changed_path.is_file() {
        debug!(path = %rel_str, "background indexer: skipping non-file path");
        return Ok(());
    }

    let content_hash = compute_content_hash(changed_path);

    let stale = index_store
        .is_index_stale(&project_path, &rel_str, &content_hash)
        .map_err(|e| e.to_string())?;

    if !stale {
        debug!(path = %rel_str, "background indexer: file unchanged, skipping");
        return Ok(());
    }

    // Build a minimal inventory item for this single file.
    let metadata = std::fs::metadata(changed_path).map_err(|e| e.to_string())?;
    let ext = changed_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    let language = detect_language_simple(ext.as_deref());

    let limits = AnalysisLimits::default();
    let symbols = extract_symbols(changed_path, &language, limits.max_symbols_per_file);

    let line_count = estimate_line_count_simple(changed_path, metadata.len());

    let item = super::analysis_index::FileInventoryItem {
        path: rel_str.clone(),
        component: String::new(), // component detection is non-critical for incremental
        language,
        extension: ext,
        size_bytes: metadata.len(),
        line_count,
        is_test: false,
        symbols,
    };

    index_store
        .upsert_file_index(&project_path, &item, &content_hash)
        .map_err(|e| e.to_string())?;

    debug!(path = %rel_str, "background indexer: incremental index updated");
    Ok(())
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

/// Estimate the number of lines in a file without loading very large files.
fn estimate_line_count_simple(path: &Path, file_size: u64) -> usize {
    if file_size > 2_000_000 {
        return 0;
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            if bytes.is_empty() {
                0
            } else {
                bytes.iter().filter(|&&b| b == b'\n').count() + 1
            }
        }
        Err(_) => 0,
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
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub struct Config;\n",
        )
        .expect("write");

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
        fs::write(
            dir.path().join("main.py"),
            "def entry():\n    pass\n",
        )
        .expect("write");

        let store = test_store();
        let indexer = BackgroundIndexer::new(dir.path().to_path_buf(), store.clone());

        let handle = indexer.start().await;
        // The task should complete since there is no change_rx
        handle.await.expect("task should complete");

        let project_path = dir.path().to_string_lossy().to_string();
        let summary = store.get_project_summary(&project_path).expect("summary");
        assert_eq!(summary.total_files, 1);
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
}
