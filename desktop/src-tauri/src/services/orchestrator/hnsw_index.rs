//! HNSW Vector Index
//!
//! Wraps the `hnsw_rs` crate to provide O(log n) approximate nearest neighbor
//! search for embedding vectors.  The index is treated as a **derived cache**
//! (ADR-004): SQLite `file_embeddings` is the source of truth, and the HNSW
//! files can be deleted and rebuilt at any time.
//!
//! ## Thread Safety
//!
//! The inner `Hnsw` is wrapped in `Arc` and accessed via `RwLock` so that
//! readers (search) can proceed concurrently while writers (insert, rebuild)
//! hold exclusive access.  CPU-bound HNSW operations are offloaded to
//! `tokio::task::spawn_blocking`.
//!
//! ## Persistence
//!
//! The index is persisted as two sidecar files next to the SQLite database:
//! - `<index_dir>/embeddings.hnsw.graph`
//! - `<index_dir>/embeddings.hnsw.data`
//!
//! ## Soft-Delete Pattern
//!
//! Since `hnsw_rs` does not support point deletion, stale IDs are tracked in a
//! `HashSet<usize>` and filtered from search results.  When the stale fraction
//! exceeds 10%, a full rebuild from SQLite is triggered.

use hnsw_rs::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// HNSW tuning parameters.
const MAX_NB_CONNECTION: usize = 24;
const MAX_LAYER: usize = 16;
const EF_CONSTRUCTION: usize = 200;
const EF_SEARCH: usize = 64;

/// Default maximum number of elements for initial index creation.
/// The index can grow beyond this but performance may degrade.
const DEFAULT_MAX_ELEMENTS: usize = 100_000;

/// Basename used for the persisted HNSW files.
const HNSW_BASENAME: &str = "embeddings";

/// Wrapper around `hnsw_rs::Hnsw` providing thread-safe, async-friendly
/// approximate nearest neighbor search with disk persistence and soft-delete.
pub struct HnswIndex {
    /// Directory where HNSW sidecar files are stored.
    index_dir: PathBuf,
    /// Embedding vector dimension.
    dimension: usize,
    /// The HNSW index wrapped for concurrent access.
    /// `None` means the index has not been built yet.
    inner: RwLock<Option<Arc<HnswInner>>>,
    /// Set of data IDs marked as stale (soft-deleted).
    stale_ids: RwLock<HashSet<usize>>,
    /// Total number of vectors inserted (including stale ones).
    count: RwLock<usize>,
}

/// Newtype wrapper so we can send the HNSW across threads.
///
/// The `'static` lifetime is safe here because:
/// - When created via `Hnsw::new()`, all data is owned.
/// - When loaded from disk, `hnsw_rs` reads data into owned memory.
/// - The `HnswIo` used for loading is leaked (via `Box::leak`) to satisfy
///   the borrow checker, as `hnsw_rs` returns `Hnsw<'a, ...>` borrowing
///   from the `HnswIo`.
struct HnswInner {
    hnsw: Hnsw<'static, f32, DistCosine>,
}

// SAFETY: hnsw_rs::Hnsw<'static, f32, DistCosine> uses Arc-based internal
// storage and is safe to share across threads.
unsafe impl Send for HnswInner {}
unsafe impl Sync for HnswInner {}

impl HnswIndex {
    /// Create a new, empty HNSW index.
    ///
    /// The index directory will be created if it does not exist.
    pub fn new(index_dir: impl AsRef<Path>, dimension: usize) -> Self {
        Self {
            index_dir: index_dir.as_ref().to_path_buf(),
            dimension,
            inner: RwLock::new(None),
            stale_ids: RwLock::new(HashSet::new()),
            count: RwLock::new(0),
        }
    }

    /// Initialize the index with an empty HNSW graph.
    pub async fn initialize(&self) {
        let hnsw = Hnsw::<f32, DistCosine>::new(
            MAX_NB_CONNECTION,
            DEFAULT_MAX_ELEMENTS,
            MAX_LAYER,
            EF_CONSTRUCTION,
            DistCosine,
        );
        let mut guard = self.inner.write().await;
        *guard = Some(Arc::new(HnswInner { hnsw }));
        let mut count = self.count.write().await;
        *count = 0;
        let mut stale = self.stale_ids.write().await;
        stale.clear();
    }

    /// Try to load the index from disk.
    ///
    /// Returns `true` if loaded successfully, `false` if files do not exist
    /// or loading fails.
    ///
    /// Note: loading is done synchronously because `HnswIo` borrows data
    /// that must outlive the returned `Hnsw`.  Loading is fast (memory-mapped)
    /// and happens only once at startup.
    pub async fn load_from_disk(&self) -> bool {
        let graph_file = self.index_dir.join(format!("{}.hnsw.graph", HNSW_BASENAME));
        let data_file = self.index_dir.join(format!("{}.hnsw.data", HNSW_BASENAME));

        if !graph_file.exists() || !data_file.exists() {
            debug!(
                dir = %self.index_dir.display(),
                "HNSW load_from_disk: files not found"
            );
            return false;
        }

        // Check that the files are non-empty (hnsw_rs can panic on empty/corrupt files)
        let graph_ok = std::fs::metadata(&graph_file)
            .map(|m| m.len() > 0)
            .unwrap_or(false);
        let data_ok = std::fs::metadata(&data_file)
            .map(|m| m.len() > 0)
            .unwrap_or(false);
        if !graph_ok || !data_ok {
            warn!(
                dir = %self.index_dir.display(),
                "HNSW load_from_disk: files exist but are empty or unreadable"
            );
            return false;
        }

        // Leak the HnswIo so the returned Hnsw can have a 'static lifetime.
        // This is a small fixed-size struct (~100 bytes) leaked once per load.
        let index_dir = self.index_dir.clone();

        // Use catch_unwind because hnsw_rs can panic on corrupt data instead
        // of returning an error (e.g., null pointer in slice::from_raw_parts).
        let load_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let io = Box::leak(Box::new(HnswIo::new(&index_dir, HNSW_BASENAME)));
            let result: Result<Hnsw<'static, f32, DistCosine>, _> =
                io.load_hnsw_with_dist(DistCosine);
            result
        }));

        match load_result {
            Ok(Ok(hnsw)) => {
                let nb_point = hnsw.get_nb_point();
                let mut guard = self.inner.write().await;
                *guard = Some(Arc::new(HnswInner { hnsw }));
                let mut count = self.count.write().await;
                *count = nb_point;
                let mut stale = self.stale_ids.write().await;
                stale.clear();
                info!(
                    dir = %self.index_dir.display(),
                    points = nb_point,
                    "HNSW loaded from disk"
                );
                true
            }
            Ok(Err(e)) => {
                warn!(
                    dir = %self.index_dir.display(),
                    error = %e,
                    "HNSW load_from_disk failed"
                );
                false
            }
            Err(_panic) => {
                warn!(
                    dir = %self.index_dir.display(),
                    "HNSW load_from_disk panicked (corrupt index files), will rebuild"
                );
                // Delete corrupt files so next attempt doesn't fail again
                let _ = std::fs::remove_file(&graph_file);
                let _ = std::fs::remove_file(&data_file);
                false
            }
        }
    }

    /// Save the index to disk.
    ///
    /// Creates the index directory if it does not exist.
    pub async fn save_to_disk(&self) -> Result<(), String> {
        let guard = self.inner.read().await;
        let inner = match guard.as_ref() {
            Some(inner) => Arc::clone(inner),
            None => return Err("HNSW index not initialized".to_string()),
        };
        drop(guard);

        let index_dir = self.index_dir.clone();

        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&index_dir)
                .map_err(|e| format!("failed to create HNSW dir: {}", e))?;

            inner
                .hnsw
                .file_dump(&index_dir, HNSW_BASENAME)
                .map_err(|e| format!("HNSW file_dump failed: {}", e))?;

            Ok(())
        })
        .await
        .map_err(|e| format!("spawn_blocking panicked: {}", e))?
    }

    /// Insert a single vector into the index.
    ///
    /// The `id` is the data ID (typically the SQLite row ID or a sequential
    /// counter) used to identify the vector in search results.
    pub async fn insert(&self, id: usize, embedding: &[f32]) {
        let guard = self.inner.read().await;
        if let Some(inner) = guard.as_ref() {
            let data = embedding.to_vec();
            inner.hnsw.insert_slice((&data, id));
            drop(guard);
            let mut count = self.count.write().await;
            *count += 1;
        }
    }

    /// Insert multiple vectors into the index.
    pub async fn batch_insert(&self, items: &[(usize, Vec<f32>)]) {
        if items.is_empty() {
            return;
        }
        let guard = self.inner.read().await;
        if let Some(inner) = guard.as_ref() {
            for (id, embedding) in items {
                inner.hnsw.insert_slice((embedding, *id));
            }
            drop(guard);
            let mut count = self.count.write().await;
            *count += items.len();
        }
    }

    /// Search for the `top_k` nearest neighbors of `query`.
    ///
    /// Returns a vector of `(data_id, distance)` pairs sorted by distance
    /// ascending.  Stale IDs are filtered out.
    pub async fn search(&self, query: &[f32], top_k: usize) -> Vec<(usize, f32)> {
        let guard = self.inner.read().await;
        let inner = match guard.as_ref() {
            Some(inner) => Arc::clone(inner),
            None => return Vec::new(),
        };
        drop(guard);

        let stale = self.stale_ids.read().await;
        let stale_snapshot: HashSet<usize> = stale.clone();
        drop(stale);

        let query_vec = query.to_vec();

        let result: Result<Vec<(usize, f32)>, _> = tokio::task::spawn_blocking(move || {
            // Request extra results to compensate for stale ID filtering
            let ef = EF_SEARCH.max(top_k * 2);
            let request_k = top_k + stale_snapshot.len();
            let neighbours = inner.hnsw.search(&query_vec, request_k, ef);

            let mut results: Vec<(usize, f32)> = neighbours
                .into_iter()
                .filter(|n| !stale_snapshot.contains(&n.d_id))
                .map(|n| (n.d_id, n.distance))
                .collect();

            results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            results.truncate(top_k);
            results
        })
        .await;

        result.unwrap_or_default()
    }

    /// Mark a data ID as stale (soft-delete).
    ///
    /// The ID will be filtered from future search results.
    pub async fn mark_stale(&self, id: usize) {
        let mut stale = self.stale_ids.write().await;
        stale.insert(id);
    }

    /// Returns `true` if the index has been initialized and contains data.
    pub async fn is_ready(&self) -> bool {
        let guard = self.inner.read().await;
        guard.is_some()
    }

    /// Returns the number of vectors in the index (including stale ones).
    pub async fn get_count(&self) -> usize {
        let count = self.count.read().await;
        *count
    }

    /// Returns the number of stale IDs.
    pub async fn get_stale_count(&self) -> usize {
        let stale = self.stale_ids.read().await;
        stale.len()
    }

    /// Returns true if the stale ratio exceeds the rebuild threshold (10%).
    pub async fn needs_rebuild(&self) -> bool {
        let count = self.count.read().await;
        let stale = self.stale_ids.read().await;
        if *count == 0 {
            return false;
        }
        (stale.len() as f64 / *count as f64) > 0.10
    }

    /// Reset the index to empty state (for rebuild).
    pub async fn reset(&self) {
        self.initialize().await;
    }

    /// Get the index directory path.
    pub fn index_dir(&self) -> &Path {
        &self.index_dir
    }

    /// Get the embedding dimension.
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Generate a random-ish embedding vector of the given dimension.
    /// Uses a simple deterministic scheme based on seed for reproducibility.
    fn make_embedding(dim: usize, seed: usize) -> Vec<f32> {
        let mut v = Vec::with_capacity(dim);
        for i in 0..dim {
            // Simple deterministic "random" values
            let val = ((seed * 7 + i * 13) % 1000) as f32 / 1000.0;
            v.push(val);
        }
        // L2 normalize for cosine distance
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
        v
    }

    // -----------------------------------------------------------------------
    // new creates empty index (not yet initialized)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn new_creates_uninitialized_index() {
        let dir = tempdir().expect("tempdir");
        let idx = HnswIndex::new(dir.path().join("hnsw"), 128);
        assert!(!idx.is_ready().await);
    }

    // -----------------------------------------------------------------------
    // initialize creates ready index
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn initialize_creates_ready_index() {
        let dir = tempdir().expect("tempdir");
        let idx = HnswIndex::new(dir.path().join("hnsw"), 128);
        idx.initialize().await;
        assert!(idx.is_ready().await);
        assert_eq!(idx.get_count().await, 0);
    }

    // -----------------------------------------------------------------------
    // empty index returns empty results
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn empty_index_returns_empty_results() {
        let dir = tempdir().expect("tempdir");
        let idx = HnswIndex::new(dir.path().join("hnsw"), 3);
        idx.initialize().await;

        let query = vec![1.0, 0.0, 0.0];
        let results = idx.search(&query, 10).await;
        assert!(results.is_empty());
    }

    // -----------------------------------------------------------------------
    // insert + search returns correct nearest neighbor
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn insert_and_search_returns_nearest_neighbor() {
        let dir = tempdir().expect("tempdir");
        let dim = 64;
        let idx = HnswIndex::new(dir.path().join("hnsw"), dim);
        idx.initialize().await;

        // Insert 100 random vectors
        for i in 0..100 {
            let emb = make_embedding(dim, i);
            idx.insert(i, &emb).await;
        }

        // Search for the exact vector we inserted at id=42
        let query = make_embedding(dim, 42);
        let results = idx.search(&query, 1).await;

        assert!(!results.is_empty(), "should return at least one result");
        assert_eq!(
            results[0].0, 42,
            "top-1 should be the exact match (id=42)"
        );
        assert!(
            results[0].1 < 0.01,
            "distance to self should be near zero, got {}",
            results[0].1
        );
    }

    // -----------------------------------------------------------------------
    // search top-k returns multiple results sorted by distance
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn search_returns_top_k_sorted_by_distance() {
        let dir = tempdir().expect("tempdir");
        let dim = 32;
        let idx = HnswIndex::new(dir.path().join("hnsw"), dim);
        idx.initialize().await;

        for i in 0..50 {
            let emb = make_embedding(dim, i);
            idx.insert(i, &emb).await;
        }

        let query = make_embedding(dim, 25);
        let results = idx.search(&query, 5).await;

        assert_eq!(results.len(), 5, "should return exactly 5 results");
        // Results should be sorted by distance ascending
        for i in 1..results.len() {
            assert!(
                results[i].1 >= results[i - 1].1,
                "results should be sorted by distance"
            );
        }
    }

    // -----------------------------------------------------------------------
    // save_to_disk + load_from_disk roundtrips successfully
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn save_load_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let index_dir = dir.path().join("hnsw_save_load");
        let dim = 32;

        // Build and save
        {
            let idx = HnswIndex::new(&index_dir, dim);
            idx.initialize().await;

            for i in 0..20 {
                let emb = make_embedding(dim, i);
                idx.insert(i, &emb).await;
            }

            idx.save_to_disk().await.expect("save should succeed");
        }

        // Load into a new index and verify search works
        {
            let idx2 = HnswIndex::new(&index_dir, dim);
            assert!(idx2.load_from_disk().await, "load should succeed");
            assert!(idx2.is_ready().await);

            // Search for vector 10
            let query = make_embedding(dim, 10);
            let results = idx2.search(&query, 1).await;

            assert!(!results.is_empty());
            assert_eq!(results[0].0, 10, "should find the same vector after reload");
            assert!(results[0].1 < 0.01, "distance should be near zero");
        }
    }

    // -----------------------------------------------------------------------
    // mark_stale filters IDs from search results
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn mark_stale_filters_from_results() {
        let dir = tempdir().expect("tempdir");
        let dim = 32;
        let idx = HnswIndex::new(dir.path().join("hnsw"), dim);
        idx.initialize().await;

        for i in 0..10 {
            let emb = make_embedding(dim, i);
            idx.insert(i, &emb).await;
        }

        // Mark id=5 as stale
        idx.mark_stale(5).await;

        // Search for vector 5 — it should NOT appear in results
        let query = make_embedding(dim, 5);
        let results = idx.search(&query, 10).await;

        let ids: Vec<usize> = results.iter().map(|r| r.0).collect();
        assert!(
            !ids.contains(&5),
            "stale ID should be filtered from results"
        );
        assert_eq!(results.len(), 9, "should return 9 results (10 - 1 stale)");
    }

    // -----------------------------------------------------------------------
    // batch_insert works correctly
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn batch_insert_works() {
        let dir = tempdir().expect("tempdir");
        let dim = 16;
        let idx = HnswIndex::new(dir.path().join("hnsw"), dim);
        idx.initialize().await;

        let items: Vec<(usize, Vec<f32>)> = (0..30).map(|i| (i, make_embedding(dim, i))).collect();

        idx.batch_insert(&items).await;
        assert_eq!(idx.get_count().await, 30);

        let query = make_embedding(dim, 15);
        let results = idx.search(&query, 1).await;
        assert_eq!(results[0].0, 15);
    }

    // -----------------------------------------------------------------------
    // needs_rebuild returns true when stale ratio > 10%
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn needs_rebuild_threshold() {
        let dir = tempdir().expect("tempdir");
        let dim = 8;
        let idx = HnswIndex::new(dir.path().join("hnsw"), dim);
        idx.initialize().await;

        // Insert 100 vectors
        for i in 0..100 {
            let emb = make_embedding(dim, i);
            idx.insert(i, &emb).await;
        }

        assert!(!idx.needs_rebuild().await, "no stale IDs yet");

        // Mark 10 as stale (exactly 10%)
        for i in 0..10 {
            idx.mark_stale(i).await;
        }
        assert!(!idx.needs_rebuild().await, "10% should not trigger");

        // Mark one more to exceed 10%
        idx.mark_stale(10).await;
        assert!(idx.needs_rebuild().await, "11% should trigger rebuild");
    }

    // -----------------------------------------------------------------------
    // reset clears the index
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn reset_clears_index() {
        let dir = tempdir().expect("tempdir");
        let dim = 8;
        let idx = HnswIndex::new(dir.path().join("hnsw"), dim);
        idx.initialize().await;

        for i in 0..10 {
            idx.insert(i, &make_embedding(dim, i)).await;
        }
        idx.mark_stale(0).await;

        assert_eq!(idx.get_count().await, 10);
        assert_eq!(idx.get_stale_count().await, 1);

        idx.reset().await;

        assert_eq!(idx.get_count().await, 0);
        assert_eq!(idx.get_stale_count().await, 0);
        assert!(idx.is_ready().await);
    }

    // -----------------------------------------------------------------------
    // load_from_disk returns false when no files exist
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn load_from_disk_returns_false_when_no_files() {
        let dir = tempdir().expect("tempdir");
        let idx = HnswIndex::new(dir.path().join("nonexistent"), 32);
        assert!(!idx.load_from_disk().await);
        assert!(!idx.is_ready().await);
    }

    // -----------------------------------------------------------------------
    // not-ready index search returns empty
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn search_on_uninitialized_returns_empty() {
        let dir = tempdir().expect("tempdir");
        let idx = HnswIndex::new(dir.path().join("hnsw"), 32);
        // Don't initialize
        let results = idx.search(&[1.0, 0.0, 0.0], 5).await;
        assert!(results.is_empty());
    }
}
