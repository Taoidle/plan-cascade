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
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
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

/// Filename for the HNSW metadata sidecar (JSON).
const HNSW_META_FILENAME: &str = "embeddings.hnsw.meta.json";

/// Metadata written alongside the HNSW index files so that dimension
/// mismatches (e.g. after switching embedding provider) can be detected
/// and trigger an automatic rebuild.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HnswMetadata {
    dimension: usize,
    vector_count: usize,
    /// HNSW data IDs marked as stale (soft-deleted).
    /// Persisted so stale filtering survives app restart.
    #[serde(default)]
    stale_ids: Vec<usize>,
}

/// Wrapper around `hnsw_rs::Hnsw` providing thread-safe, async-friendly
/// approximate nearest neighbor search with disk persistence and soft-delete.
pub struct HnswIndex {
    /// Directory where HNSW sidecar files are stored.
    index_dir: PathBuf,
    /// Embedding vector dimension.  Stored as `AtomicUsize` so that it can be
    /// updated from `rebuild_hnsw_from_store` (which infers the real dimension
    /// from stored vectors) without requiring `&mut self`.
    dimension: AtomicUsize,
    /// The HNSW index wrapped for concurrent access.
    /// `None` means the index has not been built yet.
    inner: RwLock<Option<Arc<HnswInner>>>,
    /// Set of data IDs marked as stale (soft-deleted).
    stale_ids: RwLock<HashSet<usize>>,
    /// Total number of vectors inserted (including stale ones).
    count: AtomicUsize,
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
            dimension: AtomicUsize::new(dimension),
            inner: RwLock::new(None),
            stale_ids: RwLock::new(HashSet::new()),
            count: AtomicUsize::new(0),
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
        self.count.store(0, Ordering::Relaxed);
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

        // Check metadata sidecar for dimension mismatch
        let meta_file = self.index_dir.join(HNSW_META_FILENAME);
        let current_dim = self.dimension();
        let mut loaded_meta: Option<HnswMetadata> = None;
        if meta_file.exists() {
            if let Ok(meta_json) = std::fs::read_to_string(&meta_file) {
                if let Ok(meta) = serde_json::from_str::<HnswMetadata>(&meta_json) {
                    if current_dim > 0 && meta.dimension != current_dim {
                        warn!(
                            stored_dim = meta.dimension,
                            expected_dim = current_dim,
                            "HNSW load_from_disk: dimension mismatch, deleting stale index"
                        );
                        let _ = std::fs::remove_file(&graph_file);
                        let _ = std::fs::remove_file(&data_file);
                        let _ = std::fs::remove_file(&meta_file);
                        return false;
                    }
                    // If current dimension is 0 (unknown), restore from metadata
                    if current_dim == 0 && meta.dimension > 0 {
                        self.set_dimension(meta.dimension);
                    }
                    loaded_meta = Some(meta);
                }
            }
        } else if current_dim > 0 {
            // No meta file (pre-P0-1 index).  We cannot verify the stored
            // dimension, so delete the stale files and force a rebuild.
            warn!(
                expected_dim = current_dim,
                "HNSW load_from_disk: no metadata file found (legacy index), \
                 deleting to force rebuild with correct dimensions"
            );
            let _ = std::fs::remove_file(&graph_file);
            let _ = std::fs::remove_file(&data_file);
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
                self.count.store(nb_point, Ordering::Relaxed);
                let mut stale = self.stale_ids.write().await;
                stale.clear();
                if let Some(ref meta) = loaded_meta {
                    for &id in &meta.stale_ids {
                        stale.insert(id);
                    }
                    if !stale.is_empty() {
                        info!(
                            stale_count = stale.len(),
                            "HNSW load_from_disk: restored stale IDs from metadata"
                        );
                    }
                }
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
        let dimension = self.dimension();
        let vector_count = self.count.load(Ordering::Relaxed);
        let stale_snapshot: Vec<usize> = {
            let stale = self.stale_ids.read().await;
            stale.iter().copied().collect()
        };

        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&index_dir)
                .map_err(|e| format!("failed to create HNSW dir: {}", e))?;

            inner
                .hnsw
                .file_dump(&index_dir, HNSW_BASENAME)
                .map_err(|e| format!("HNSW file_dump failed: {}", e))?;

            // Write metadata sidecar for dimension and stale ID tracking
            let meta = HnswMetadata {
                dimension,
                vector_count,
                stale_ids: stale_snapshot,
            };
            if let Ok(json) = serde_json::to_string_pretty(&meta) {
                let meta_path = index_dir.join(HNSW_META_FILENAME);
                if let Err(e) = std::fs::write(&meta_path, json) {
                    warn!(error = %e, "failed to write HNSW metadata sidecar");
                }
            }

            Ok(())
        })
        .await
        .map_err(|e| format!("spawn_blocking panicked: {}", e))?
    }

    /// Insert a single vector into the index.
    ///
    /// The `id` is the data ID (typically the SQLite row ID or a sequential
    /// counter) used to identify the vector in search results.
    ///
    /// Returns `true` if the insert succeeded, `false` if the index is not
    /// initialized or the embedding dimension does not match.
    pub async fn insert(&self, id: usize, embedding: &[f32]) -> bool {
        // Dimension guard: refuse to insert when the embedding dimension
        // doesn't match the index dimension.  Without this check, `hnsw_rs`
        // (via `anndists`) would panic with an assertion failure.
        let idx_dim = self.dimension();
        if idx_dim > 0 && embedding.len() != idx_dim {
            warn!(
                embedding_dim = embedding.len(),
                index_dim = idx_dim,
                "HNSW insert: dimension mismatch, skipping insert"
            );
            return false;
        }

        let guard = self.inner.read().await;
        if let Some(inner) = guard.as_ref() {
            let data = embedding.to_vec();
            inner.hnsw.insert_slice((&data, id));
            self.count.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Insert multiple vectors into the index.
    ///
    /// Returns `true` if all inserts succeeded, `false` if the index is not
    /// initialized or the embedding dimension does not match.
    pub async fn batch_insert(&self, items: &[(usize, Vec<f32>)]) -> bool {
        if items.is_empty() {
            return true;
        }

        // Dimension guard: check the first item's dimension against the index.
        let idx_dim = self.dimension();
        if idx_dim > 0 {
            if let Some((_, first_emb)) = items.first() {
                if first_emb.len() != idx_dim {
                    warn!(
                        embedding_dim = first_emb.len(),
                        index_dim = idx_dim,
                        count = items.len(),
                        "HNSW batch_insert: dimension mismatch, skipping batch"
                    );
                    return false;
                }
            }
        }

        let guard = self.inner.read().await;
        if let Some(inner) = guard.as_ref() {
            for (id, embedding) in items {
                inner.hnsw.insert_slice((embedding, *id));
            }
            self.count.fetch_add(items.len(), Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Search for the `top_k` nearest neighbors of `query`.
    ///
    /// Returns a vector of `(data_id, distance)` pairs sorted by distance
    /// ascending.  Stale IDs are filtered out.
    pub async fn search(&self, query: &[f32], top_k: usize) -> Vec<(usize, f32)> {
        // Dimension guard: refuse to search when the query vector dimension
        // doesn't match the index dimension.  Without this check, `hnsw_rs`
        // (via `anndists`) would panic with an assertion failure.
        let idx_dim = self.dimension();
        if idx_dim > 0 && query.len() != idx_dim {
            warn!(
                query_dim = query.len(),
                index_dim = idx_dim,
                "HNSW search: dimension mismatch, skipping search"
            );
            return Vec::new();
        }

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
        self.count.load(Ordering::Relaxed)
    }

    /// Returns the number of stale IDs.
    pub async fn get_stale_count(&self) -> usize {
        let stale = self.stale_ids.read().await;
        stale.len()
    }

    /// Returns true if the stale ratio exceeds the rebuild threshold (10%).
    pub async fn needs_rebuild(&self) -> bool {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            return false;
        }
        let stale = self.stale_ids.read().await;
        (stale.len() as f64 / count as f64) > 0.10
    }

    /// Reset the index to empty state (for rebuild).
    pub async fn reset(&self) {
        self.initialize().await;
    }

    /// Atomically rebuild the HNSW index from the given vectors.
    ///
    /// Builds a new Hnsw graph in `spawn_blocking` (CPU-bound), then swaps it
    /// into `inner` under a single write lock.  Concurrent `search()` calls
    /// always see either the old or the new index, never an empty one.
    pub async fn rebuild_from_vectors(
        &self,
        vectors: &[(usize, Vec<f32>)],
    ) -> Result<(), String> {
        if vectors.is_empty() {
            self.initialize().await;
            return Ok(());
        }

        let actual_dim = vectors[0].1.len();

        // Filter vectors to only include those matching the expected dimension.
        // Mixed dimensions can occur when the embedding provider or its config
        // changes between indexing runs, leaving stale embeddings in SQLite.
        let filtered: Vec<(usize, Vec<f32>)> = vectors
            .iter()
            .filter(|(id, emb)| {
                if emb.len() != actual_dim {
                    warn!(
                        id = id,
                        expected_dim = actual_dim,
                        actual_dim = emb.len(),
                        "HNSW rebuild: filtering out vector with mismatched dimension"
                    );
                    false
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        if filtered.is_empty() {
            self.initialize().await;
            return Ok(());
        }

        let filtered_count = filtered.len();

        let new_inner = tokio::task::spawn_blocking(move || {
            // Use catch_unwind as an additional safety net — hnsw_rs / anndists
            // can panic on unexpected input rather than returning errors.
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let hnsw = Hnsw::<f32, DistCosine>::new(
                    MAX_NB_CONNECTION,
                    filtered.len().max(DEFAULT_MAX_ELEMENTS),
                    MAX_LAYER,
                    EF_CONSTRUCTION,
                    DistCosine,
                );
                for (id, embedding) in &filtered {
                    hnsw.insert_slice((embedding, *id));
                }
                Arc::new(HnswInner { hnsw })
            }))
        })
        .await
        .map_err(|e| format!("spawn_blocking panicked: {}", e))?
        .map_err(|_| {
            "HNSW rebuild panicked during vector insertion \
             (possible dimension mismatch in hnsw_rs)"
                .to_string()
        })?;

        // Atomic swap — concurrent searches see old or new, never empty
        {
            let mut guard = self.inner.write().await;
            *guard = Some(new_inner);
        }
        self.dimension.store(actual_dim, Ordering::Relaxed);
        self.count.store(filtered_count, Ordering::Relaxed);
        {
            let mut stale = self.stale_ids.write().await;
            stale.clear();
        }

        Ok(())
    }

    /// Get the index directory path.
    pub fn index_dir(&self) -> &Path {
        &self.index_dir
    }

    /// Get the embedding dimension.
    pub fn dimension(&self) -> usize {
        self.dimension.load(Ordering::Relaxed)
    }

    /// Update the embedding dimension.
    ///
    /// Used when the actual dimension is inferred from stored vectors (e.g.
    /// during `rebuild_hnsw_from_store`) and the initial dimension was unknown.
    pub fn set_dimension(&self, dim: usize) {
        self.dimension.store(dim, Ordering::Relaxed);
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

    // -----------------------------------------------------------------------
    // dimension mismatch triggers rebuild on load
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn load_from_disk_returns_false_on_dimension_mismatch() {
        let dir = tempdir().expect("tempdir");
        let index_dir = dir.path().join("hnsw_dim_mismatch");
        let dim_old = 128;

        // Build and save with dim=128
        {
            let idx = HnswIndex::new(&index_dir, dim_old);
            idx.initialize().await;
            for i in 0..5 {
                idx.insert(i, &make_embedding(dim_old, i)).await;
            }
            idx.save_to_disk().await.expect("save should succeed");
        }

        // Try to load with dim=768 — should detect mismatch and return false
        {
            let idx2 = HnswIndex::new(&index_dir, 768);
            assert!(
                !idx2.load_from_disk().await,
                "load should return false on dimension mismatch"
            );
            assert!(!idx2.is_ready().await);
        }
    }

    // -----------------------------------------------------------------------
    // set_dimension updates dimension after rebuild
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn set_dimension_updates_after_rebuild() {
        let dir = tempdir().expect("tempdir");
        let dim = 64;
        let idx = HnswIndex::new(dir.path().join("hnsw"), 0); // start with unknown dim
        assert_eq!(idx.dimension(), 0);

        idx.set_dimension(dim);
        assert_eq!(idx.dimension(), dim);
    }

    // -----------------------------------------------------------------------
    // metadata sidecar restores dimension when loading with dim=0
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn load_from_disk_restores_dimension_from_metadata() {
        let dir = tempdir().expect("tempdir");
        let index_dir = dir.path().join("hnsw_meta_restore");
        let dim = 32;

        // Build, save (writes metadata sidecar)
        {
            let idx = HnswIndex::new(&index_dir, dim);
            idx.initialize().await;
            for i in 0..3 {
                idx.insert(i, &make_embedding(dim, i)).await;
            }
            idx.save_to_disk().await.expect("save");
        }

        // Load with dim=0 — should restore dimension from metadata
        {
            let idx2 = HnswIndex::new(&index_dir, 0);
            assert!(idx2.load_from_disk().await, "should load successfully");
            assert_eq!(
                idx2.dimension(),
                dim,
                "dimension should be restored from metadata"
            );
        }
    }

    // -----------------------------------------------------------------------
    // save/load preserves stale IDs across restart
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn save_load_preserves_stale_ids() {
        let dir = tempdir().expect("tempdir");
        let index_dir = dir.path().join("hnsw_stale_persist");
        let dim = 16;

        // Build index, mark some IDs as stale, save
        {
            let idx = HnswIndex::new(&index_dir, dim);
            idx.initialize().await;
            for i in 0..5 {
                idx.insert(i, &make_embedding(dim, i)).await;
            }
            idx.mark_stale(1).await;
            idx.mark_stale(3).await;
            assert_eq!(idx.get_stale_count().await, 2);
            idx.save_to_disk().await.expect("save should succeed");
        }

        // Load in a fresh HnswIndex — stale IDs should be restored
        {
            let idx2 = HnswIndex::new(&index_dir, dim);
            assert!(idx2.load_from_disk().await, "should load successfully");
            assert_eq!(
                idx2.get_stale_count().await,
                2,
                "stale IDs should survive save/load"
            );

            // Search should filter out the stale IDs
            let results = idx2.search(&make_embedding(dim, 1), 10).await;
            let result_ids: Vec<usize> = results.iter().map(|(id, _)| *id).collect();
            assert!(
                !result_ids.contains(&1),
                "stale ID 1 should be filtered from search"
            );
            assert!(
                !result_ids.contains(&3),
                "stale ID 3 should be filtered from search"
            );
        }
    }

    // -----------------------------------------------------------------------
    // rebuild_from_vectors atomically replaces the index
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn rebuild_from_vectors_atomic() {
        let dir = tempdir().expect("tempdir");
        let dim = 8;
        let idx = HnswIndex::new(dir.path().join("hnsw_rebuild"), dim);
        idx.initialize().await;

        // Insert initial vectors and mark one stale
        for i in 0..5 {
            idx.insert(i, &make_embedding(dim, i)).await;
        }
        idx.mark_stale(2).await;
        assert_eq!(idx.get_count().await, 5);
        assert_eq!(idx.get_stale_count().await, 1);

        // Rebuild with a different set of vectors
        let new_vectors: Vec<(usize, Vec<f32>)> = (10..13)
            .map(|i| (i, make_embedding(dim, i)))
            .collect();
        idx.rebuild_from_vectors(&new_vectors)
            .await
            .expect("rebuild should succeed");

        // Count should reflect new vectors, stale should be cleared
        assert_eq!(idx.get_count().await, 3);
        assert_eq!(idx.get_stale_count().await, 0);

        // Search should find new vectors
        let results = idx.search(&make_embedding(dim, 10), 5).await;
        let result_ids: Vec<usize> = results.iter().map(|(id, _)| *id).collect();
        assert!(
            result_ids.contains(&10),
            "new vector 10 should be searchable"
        );

        // Old vectors should not be found
        assert!(
            !result_ids.contains(&0),
            "old vector 0 should not be in rebuilt index"
        );
    }

    // -----------------------------------------------------------------------
    // rebuild_from_vectors with empty input initializes empty index
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn rebuild_from_vectors_empty() {
        let dir = tempdir().expect("tempdir");
        let dim = 8;
        let idx = HnswIndex::new(dir.path().join("hnsw_rebuild_empty"), dim);
        idx.initialize().await;

        for i in 0..3 {
            idx.insert(i, &make_embedding(dim, i)).await;
        }
        assert_eq!(idx.get_count().await, 3);

        // Rebuild with empty vectors should reset to empty
        idx.rebuild_from_vectors(&[])
            .await
            .expect("rebuild with empty should succeed");
        assert_eq!(idx.get_count().await, 0);
        assert!(idx.is_ready().await, "index should still be initialized");
    }
}
