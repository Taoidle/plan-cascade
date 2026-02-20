//! Hybrid Search Engine with Reciprocal Rank Fusion (RRF)
//!
//! Combines multiple search channels (keyword-based and semantic) using RRF
//! to produce a single ranked result list. Each result carries score provenance
//! so callers can explain why a result was ranked where it was.
//!
//! ## RRF Formula
//!
//! For each document `d`, the fused score is:
//!
//! ```text
//! score(d) = Σ 1 / (k + rank_i(d))
//! ```
//!
//! where `k` is a constant (default 60) and `rank_i(d)` is the 1-based rank of
//! `d` in channel `i`. Documents not found in a channel receive no contribution
//! from that channel (rather than a penalty).
//!
//! ## Deterministic Tie-Breaking
//!
//! Results with equal RRF scores are sorted by `file_path` ascending to ensure
//! deterministic ordering across runs.
//!
//! ## Design Decision (ADR — story-014)
//!
//! The hybrid engine is decoupled from `ToolExecutor` so it can be reused from
//! both the `CodebaseSearch` tool and any future standalone semantic search command.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing;

use super::embedding_manager::EmbeddingManager;
use super::hnsw_index::HnswIndex;
use super::index_store::IndexStore;
use crate::utils::error::{AppError, AppResult};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Identifies which search channel produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SearchChannel {
    /// Symbol name search (IndexStore::query_symbols)
    Symbol,
    /// File path substring search (IndexStore::query_files_by_path)
    FilePath,
    /// Semantic vector similarity search (EmbeddingManager + IndexStore::semantic_search)
    Semantic,
}

impl std::fmt::Display for SearchChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchChannel::Symbol => write!(f, "symbol"),
            SearchChannel::FilePath => write!(f, "file_path"),
            SearchChannel::Semantic => write!(f, "semantic"),
        }
    }
}

/// A single channel's contribution to a fused result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelContribution {
    /// Which search channel this contribution comes from.
    pub channel: SearchChannel,
    /// The 1-based rank within this channel's result list.
    pub rank: usize,
    /// The RRF score contribution: `1.0 / (k + rank)`.
    pub rrf_contribution: f64,
}

/// A hybrid search result after RRF fusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchResult {
    /// The file path that was matched.
    pub file_path: String,
    /// The combined RRF score across all channels.
    pub score: f64,
    /// Provenance: which channels contributed and their individual ranks.
    pub provenance: Vec<ChannelContribution>,
    /// Optional: the matching symbol name (when found via Symbol channel).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,
    /// Optional: the matching chunk text (when found via Semantic channel).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_text: Option<String>,
    /// Optional: the semantic similarity score from the Semantic channel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_similarity: Option<f32>,
}

/// Outcome of a hybrid search, wrapping results with degradation metadata.
///
/// When the semantic channel fails (e.g. embedding provider unavailable), the
/// search still returns keyword-only results but sets `semantic_degraded = true`
/// so callers can inform the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchOutcome {
    /// The fused search results.
    pub results: Vec<HybridSearchResult>,
    /// `true` when the semantic channel was skipped or failed.
    pub semantic_degraded: bool,
    /// Human-readable reason when `semantic_degraded` is `true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_error: Option<String>,

    /// Embedding provider display name (e.g., "OpenAI (text-embedding-3-small)").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_display: Option<String>,
    /// Embedding vector dimension (0 means unknown or not configured).
    #[serde(default)]
    pub embedding_dimension: usize,
    /// Number of vectors in the HNSW index.
    #[serde(default)]
    pub hnsw_vector_count: usize,
    /// Whether HNSW was used for semantic search.
    #[serde(default)]
    pub hnsw_used: bool,
    /// Which search channels actually returned results.
    #[serde(default)]
    pub active_channels: Vec<SearchChannel>,
}

/// Configuration for the hybrid search engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchConfig {
    /// RRF constant `k`. Higher values reduce the influence of high-ranked
    /// items relative to lower-ranked ones. Default: 60.
    pub rrf_k: f64,
    /// Maximum number of results to return.
    pub max_results: usize,
    /// Maximum number of results to fetch from each channel before fusion.
    pub channel_max_results: usize,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            rrf_k: 60.0,
            max_results: 20,
            channel_max_results: 50,
        }
    }
}

// ---------------------------------------------------------------------------
// HybridSearchEngine
// ---------------------------------------------------------------------------

/// Hybrid search engine that fuses keyword and semantic search channels via RRF.
///
/// Holds references to the index store and (optionally) the embedding manager.
/// When no embedding manager is configured, the semantic channel is skipped.
///
/// When an `HnswIndex` is provided and ready, the semantic channel uses O(log n)
/// approximate nearest neighbor search instead of O(n) brute-force scan.
pub struct HybridSearchEngine {
    index_store: Arc<IndexStore>,
    embedding_manager: Option<Arc<EmbeddingManager>>,
    /// Optional HNSW index for fast approximate nearest neighbor search.
    /// When present and ready, `search_semantic` uses HNSW instead of brute-force.
    hnsw_index: Option<Arc<HnswIndex>>,
    config: HybridSearchConfig,
}

impl HybridSearchEngine {
    /// Create a new `HybridSearchEngine`.
    ///
    /// # Arguments
    ///
    /// * `index_store` - The SQLite-backed index store for keyword searches.
    /// * `embedding_manager` - Optional embedding manager for semantic search.
    /// * `config` - Configuration parameters (RRF k, max results, etc.).
    pub fn new(
        index_store: Arc<IndexStore>,
        embedding_manager: Option<Arc<EmbeddingManager>>,
        config: HybridSearchConfig,
    ) -> Self {
        Self {
            index_store,
            embedding_manager,
            hnsw_index: None,
            config,
        }
    }

    /// Create a new `HybridSearchEngine` with default configuration.
    pub fn with_defaults(
        index_store: Arc<IndexStore>,
        embedding_manager: Option<Arc<EmbeddingManager>>,
    ) -> Self {
        Self::new(index_store, embedding_manager, HybridSearchConfig::default())
    }

    /// Set the HNSW index for fast approximate nearest neighbor search.
    ///
    /// When set and ready, the semantic channel will use HNSW search (O(log n))
    /// instead of brute-force cosine similarity scan (O(n)).
    pub fn set_hnsw_index(&mut self, hnsw: Arc<HnswIndex>) {
        self.hnsw_index = Some(hnsw);
    }

    /// Builder-style setter for the HNSW index.
    pub fn with_hnsw_index(mut self, hnsw: Option<Arc<HnswIndex>>) -> Self {
        self.hnsw_index = hnsw;
        self
    }

    /// Returns a reference to the current configuration.
    pub fn config(&self) -> &HybridSearchConfig {
        &self.config
    }

    /// Perform a hybrid search across all available channels.
    ///
    /// Runs keyword channels (symbols, file paths) and the semantic channel
    /// (if an embedding manager is configured), then fuses results using RRF.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query string.
    /// * `project_path` - The project path to search within.
    ///
    /// # Returns
    ///
    /// A vector of `HybridSearchResult` sorted by RRF score descending, then
    /// file path ascending for deterministic tie-breaking.
    pub async fn search(
        &self,
        query: &str,
        project_path: &str,
    ) -> AppResult<HybridSearchOutcome> {
        let mut channel_results: Vec<(SearchChannel, Vec<ChannelEntry>)> = Vec::new();
        let mut semantic_degraded = false;
        let mut semantic_error: Option<String> = None;

        // --- Channel 1: Symbol search ---
        let symbol_entries = self.search_symbols(query)?;
        if !symbol_entries.is_empty() {
            channel_results.push((SearchChannel::Symbol, symbol_entries));
        }

        // --- Channel 2: File path search ---
        let file_entries = self.search_file_paths(query, project_path)?;
        if !file_entries.is_empty() {
            channel_results.push((SearchChannel::FilePath, file_entries));
        }

        // --- Channel 3: Semantic search ---
        if let Some(ref emb_mgr) = self.embedding_manager {
            match self
                .search_semantic(query, project_path, emb_mgr)
                .await
            {
                Ok(semantic_entries) if !semantic_entries.is_empty() => {
                    channel_results.push((SearchChannel::Semantic, semantic_entries));
                }
                Ok(_) => {
                    // No semantic results; skip this channel silently
                }
                Err(e) => {
                    // Log and skip — semantic failure should not block keyword results
                    tracing::warn!("Hybrid search: semantic channel failed: {}", e);
                    semantic_degraded = true;
                    semantic_error = Some(format!("{}", e));
                }
            }
        } else {
            // No embedding manager configured — semantic unavailable
            semantic_degraded = true;
            semantic_error = Some("No embedding provider configured".to_string());
        }

        // --- Collect metadata ---
        let active_channels: Vec<SearchChannel> =
            channel_results.iter().map(|(ch, _)| *ch).collect();

        let (provider_display, embedding_dimension) =
            if let Some(ref emb_mgr) = self.embedding_manager {
                (
                    Some(emb_mgr.display_name().to_string()),
                    emb_mgr.dimension(),
                )
            } else {
                (None, 0)
            };

        let (hnsw_used, hnsw_vector_count) = if let Some(ref hnsw) = self.hnsw_index {
            if hnsw.is_ready().await {
                (true, hnsw.get_count().await)
            } else {
                (false, 0)
            }
        } else {
            (false, 0)
        };

        // --- RRF Fusion ---
        let fused = self.fuse_rrf(&channel_results);

        Ok(HybridSearchOutcome {
            results: fused,
            semantic_degraded,
            semantic_error,
            provider_display,
            embedding_dimension,
            hnsw_vector_count,
            hnsw_used,
            active_channels,
        })
    }

    // -----------------------------------------------------------------------
    // Channel implementations
    // -----------------------------------------------------------------------

    /// Run the symbol search channel.
    ///
    /// Attempts FTS5 BM25-ranked search first. Falls back to LIKE-based search
    /// if FTS returns empty results or encounters an error (graceful degradation).
    fn search_symbols(&self, query: &str) -> AppResult<Vec<ChannelEntry>> {
        // Try FTS5 first for BM25-ranked results
        match self.index_store.fts_search_symbols(query, self.config.channel_max_results) {
            Ok(fts_results) if !fts_results.is_empty() => {
                let entries: Vec<ChannelEntry> = fts_results
                    .into_iter()
                    .map(|sym| ChannelEntry {
                        file_path: sym.file_path,
                        symbol_name: Some(sym.symbol_name),
                        chunk_text: None,
                        semantic_similarity: None,
                    })
                    .collect();
                return Ok(entries);
            }
            Err(e) => {
                tracing::warn!("FTS5 symbol search failed, falling back to LIKE: {}", e);
            }
            Ok(_) => {
                // FTS returned empty; fall through to LIKE
            }
        }

        // Fallback: LIKE-based search
        let pattern = format!("%{}%", query);
        let symbols = self.index_store.query_symbols(&pattern)?;

        let entries: Vec<ChannelEntry> = symbols
            .into_iter()
            .take(self.config.channel_max_results)
            .map(|sym| ChannelEntry {
                file_path: sym.file_path,
                symbol_name: Some(sym.symbol_name),
                chunk_text: None,
                semantic_similarity: None,
            })
            .collect();

        Ok(entries)
    }

    /// Run the file path search channel.
    ///
    /// Attempts FTS5 BM25-ranked search first. Falls back to LIKE-based search
    /// if FTS returns empty results or encounters an error (graceful degradation).
    fn search_file_paths(
        &self,
        query: &str,
        project_path: &str,
    ) -> AppResult<Vec<ChannelEntry>> {
        // Try FTS5 first for BM25-ranked results
        match self.index_store.fts_search_files(query, project_path, self.config.channel_max_results) {
            Ok(fts_results) if !fts_results.is_empty() => {
                let entries: Vec<ChannelEntry> = fts_results
                    .into_iter()
                    .map(|f| ChannelEntry {
                        file_path: f.file_path,
                        symbol_name: None,
                        chunk_text: None,
                        semantic_similarity: None,
                    })
                    .collect();
                return Ok(entries);
            }
            Err(e) => {
                tracing::warn!("FTS5 filepath search failed, falling back to LIKE: {}", e);
            }
            Ok(_) => {
                // FTS returned empty; fall through to LIKE
            }
        }

        // Fallback: LIKE-based search
        let pattern = format!("%{}%", query);
        let files = self.index_store.query_files_by_path(project_path, &pattern)?;

        let entries: Vec<ChannelEntry> = files
            .into_iter()
            .take(self.config.channel_max_results)
            .map(|f| ChannelEntry {
                file_path: f.file_path,
                symbol_name: None,
                chunk_text: None,
                semantic_similarity: None,
            })
            .collect();

        Ok(entries)
    }

    /// Run the semantic search channel.
    ///
    /// When an HNSW index is available and ready, uses O(log n) approximate
    /// nearest neighbor search.  Falls back to O(n) brute-force cosine
    /// similarity scan when HNSW is absent or not ready.
    async fn search_semantic(
        &self,
        query: &str,
        project_path: &str,
        emb_mgr: &EmbeddingManager,
    ) -> AppResult<Vec<ChannelEntry>> {
        // Embed the query
        let query_embedding = emb_mgr
            .embed_query(query)
            .await
            .map_err(|e| AppError::internal(format!("Embedding query failed: {}", e)))?;

        if query_embedding.is_empty() {
            return Ok(Vec::new());
        }

        // Try HNSW search first (O(log n))
        if let Some(ref hnsw) = self.hnsw_index {
            if hnsw.is_ready().await {
                tracing::debug!("Hybrid search: using HNSW for semantic channel");
                return self
                    .search_semantic_hnsw(
                        &query_embedding,
                        hnsw,
                        self.config.channel_max_results,
                    )
                    .await;
            } else {
                tracing::debug!("Hybrid search: HNSW not ready, falling back to brute-force");
            }
        }

        // Fallback: brute-force cosine similarity scan (O(n))
        let results = self.index_store.semantic_search(
            &query_embedding,
            project_path,
            self.config.channel_max_results,
        )?;

        let entries: Vec<ChannelEntry> = results
            .into_iter()
            .map(|r| ChannelEntry {
                file_path: r.file_path,
                symbol_name: None,
                chunk_text: Some(r.chunk_text),
                semantic_similarity: Some(r.similarity),
            })
            .collect();

        Ok(entries)
    }

    /// Perform semantic search using the HNSW index.
    ///
    /// 1. Searches HNSW for nearest neighbor IDs (data_id = SQLite ROWID).
    /// 2. Fetches chunk metadata (file_path, chunk_text) from SQLite by ROWID.
    /// 3. Converts HNSW distance to similarity score.
    async fn search_semantic_hnsw(
        &self,
        query_embedding: &[f32],
        hnsw: &HnswIndex,
        top_k: usize,
    ) -> AppResult<Vec<ChannelEntry>> {
        let hnsw_results = hnsw.search(query_embedding, top_k).await;

        if hnsw_results.is_empty() {
            return Ok(Vec::new());
        }

        // Fetch chunk metadata from SQLite for the matched ROWID values
        let rowids: Vec<usize> = hnsw_results.iter().map(|(id, _)| *id).collect();
        let metadata = self.index_store.get_embeddings_by_rowids(&rowids)?;

        let entries: Vec<ChannelEntry> = hnsw_results
            .into_iter()
            .filter_map(|(id, distance)| {
                metadata.get(&id).map(|(file_path, _chunk_index, chunk_text)| {
                    // Convert DistCosine distance to similarity:
                    // DistCosine distance = 1 - cosine_similarity
                    // so similarity = 1 - distance
                    let similarity = 1.0 - distance;
                    ChannelEntry {
                        file_path: file_path.clone(),
                        symbol_name: None,
                        chunk_text: Some(chunk_text.clone()),
                        semantic_similarity: Some(similarity),
                    }
                })
            })
            .collect();

        Ok(entries)
    }

    // -----------------------------------------------------------------------
    // RRF Fusion
    // -----------------------------------------------------------------------

    /// Fuse channel results using Reciprocal Rank Fusion.
    ///
    /// For each document, computes:
    ///   score(d) = Σ 1 / (k + rank_i(d))
    /// where rank_i(d) is the 1-based rank in channel i.
    ///
    /// Returns results sorted by score descending, then file_path ascending.
    fn fuse_rrf(
        &self,
        channel_results: &[(SearchChannel, Vec<ChannelEntry>)],
    ) -> Vec<HybridSearchResult> {
        // Accumulate scores and provenance per file_path
        let mut score_map: HashMap<String, FusionAccumulator> = HashMap::new();

        for (channel, entries) in channel_results {
            for (idx, entry) in entries.iter().enumerate() {
                let rank = idx + 1; // 1-based rank
                let rrf_contribution = 1.0 / (self.config.rrf_k + rank as f64);

                let acc = score_map
                    .entry(entry.file_path.clone())
                    .or_insert_with(|| FusionAccumulator {
                        file_path: entry.file_path.clone(),
                        score: 0.0,
                        provenance: Vec::new(),
                        symbol_name: None,
                        chunk_text: None,
                        semantic_similarity: None,
                    });

                acc.score += rrf_contribution;
                acc.provenance.push(ChannelContribution {
                    channel: *channel,
                    rank,
                    rrf_contribution,
                });

                // Merge optional metadata from the first occurrence per channel
                if entry.symbol_name.is_some() && acc.symbol_name.is_none() {
                    acc.symbol_name = entry.symbol_name.clone();
                }
                if entry.chunk_text.is_some() && acc.chunk_text.is_none() {
                    acc.chunk_text = entry.chunk_text.clone();
                }
                if entry.semantic_similarity.is_some() && acc.semantic_similarity.is_none() {
                    acc.semantic_similarity = entry.semantic_similarity;
                }
            }
        }

        // Collect and sort: RRF score descending, then file_path ascending
        let mut results: Vec<HybridSearchResult> = score_map
            .into_values()
            .map(|acc| HybridSearchResult {
                file_path: acc.file_path,
                score: acc.score,
                provenance: acc.provenance,
                symbol_name: acc.symbol_name,
                chunk_text: acc.chunk_text,
                semantic_similarity: acc.semantic_similarity,
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.file_path.cmp(&b.file_path))
        });

        results.truncate(self.config.max_results);

        results
    }
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// Internal representation of a single channel's result entry.
#[derive(Debug, Clone)]
struct ChannelEntry {
    file_path: String,
    symbol_name: Option<String>,
    chunk_text: Option<String>,
    semantic_similarity: Option<f32>,
}

/// Accumulator used during RRF fusion to aggregate scores across channels.
struct FusionAccumulator {
    file_path: String,
    score: f64,
    provenance: Vec<ChannelContribution>,
    symbol_name: Option<String>,
    chunk_text: Option<String>,
    semantic_similarity: Option<f32>,
}

// ---------------------------------------------------------------------------
// Standalone RRF utility function
// ---------------------------------------------------------------------------

/// Compute RRF scores for a set of ranked lists.
///
/// This is a standalone utility function that can be used independently of
/// `HybridSearchEngine` for custom fusion scenarios.
///
/// # Arguments
///
/// * `ranked_lists` - A slice of ranked lists, where each list is a Vec of
///   document identifiers in rank order (best first).
/// * `k` - The RRF constant (typically 60).
///
/// # Returns
///
/// A Vec of `(document_id, rrf_score)` tuples sorted by score descending,
/// then document_id ascending for deterministic tie-breaking.
pub fn compute_rrf_scores(ranked_lists: &[Vec<String>], k: f64) -> Vec<(String, f64)> {
    let mut scores: HashMap<String, f64> = HashMap::new();

    for list in ranked_lists {
        for (idx, doc_id) in list.iter().enumerate() {
            let rank = idx + 1;
            let contribution = 1.0 / (k + rank as f64);
            *scores.entry(doc_id.clone()).or_insert(0.0) += contribution;
        }
    }

    let mut result: Vec<(String, f64)> = scores.into_iter().collect();
    result.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // RRF Calculation Tests
    // =====================================================================

    #[test]
    fn rrf_single_list_single_item() {
        let lists = vec![vec!["doc_a".to_string()]];
        let results = compute_rrf_scores(&lists, 60.0);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "doc_a");
        // rank=1, score = 1/(60+1) = 1/61
        let expected = 1.0 / 61.0;
        assert!((results[0].1 - expected).abs() < 1e-10);
    }

    #[test]
    fn rrf_single_list_multiple_items() {
        let lists = vec![vec![
            "doc_a".to_string(),
            "doc_b".to_string(),
            "doc_c".to_string(),
        ]];
        let results = compute_rrf_scores(&lists, 60.0);

        assert_eq!(results.len(), 3);
        // doc_a: rank 1 -> 1/61
        // doc_b: rank 2 -> 1/62
        // doc_c: rank 3 -> 1/63
        assert_eq!(results[0].0, "doc_a");
        assert_eq!(results[1].0, "doc_b");
        assert_eq!(results[2].0, "doc_c");

        assert!((results[0].1 - 1.0 / 61.0).abs() < 1e-10);
        assert!((results[1].1 - 1.0 / 62.0).abs() < 1e-10);
        assert!((results[2].1 - 1.0 / 63.0).abs() < 1e-10);
    }

    #[test]
    fn rrf_two_lists_with_overlap() {
        // List 1: [A, B, C] — A is rank 1, B rank 2, C rank 3
        // List 2: [B, D, A] — B is rank 1, D rank 2, A rank 3
        let lists = vec![
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
            vec!["B".to_string(), "D".to_string(), "A".to_string()],
        ];
        let results = compute_rrf_scores(&lists, 60.0);

        // Expected scores:
        // A: 1/61 + 1/63
        // B: 1/62 + 1/61
        // C: 1/63
        // D: 1/62
        let score_a = 1.0 / 61.0 + 1.0 / 63.0;
        let score_b = 1.0 / 62.0 + 1.0 / 61.0;
        let score_c = 1.0 / 63.0;
        let score_d = 1.0 / 62.0;

        // B > A > D > C
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].0, "B");
        assert!((results[0].1 - score_b).abs() < 1e-10);

        assert_eq!(results[1].0, "A");
        assert!((results[1].1 - score_a).abs() < 1e-10);

        assert_eq!(results[2].0, "D");
        assert!((results[2].1 - score_d).abs() < 1e-10);

        assert_eq!(results[3].0, "C");
        assert!((results[3].1 - score_c).abs() < 1e-10);
    }

    #[test]
    fn rrf_three_lists_all_overlapping() {
        // All three lists contain "X" at different ranks
        let lists = vec![
            vec!["X".to_string(), "Y".to_string()],
            vec!["Y".to_string(), "X".to_string()],
            vec!["X".to_string(), "Z".to_string()],
        ];
        let results = compute_rrf_scores(&lists, 60.0);

        // X: 1/61 + 1/62 + 1/61 = 2/61 + 1/62
        // Y: 1/62 + 1/61
        // Z: 1/62
        let score_x = 1.0 / 61.0 + 1.0 / 62.0 + 1.0 / 61.0;
        let score_y = 1.0 / 62.0 + 1.0 / 61.0;
        let score_z = 1.0 / 62.0;

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, "X");
        assert!((results[0].1 - score_x).abs() < 1e-10);

        assert_eq!(results[1].0, "Y");
        assert!((results[1].1 - score_y).abs() < 1e-10);

        assert_eq!(results[2].0, "Z");
        assert!((results[2].1 - score_z).abs() < 1e-10);
    }

    #[test]
    fn rrf_empty_lists() {
        let lists: Vec<Vec<String>> = vec![];
        let results = compute_rrf_scores(&lists, 60.0);
        assert!(results.is_empty());
    }

    #[test]
    fn rrf_single_empty_list() {
        let lists: Vec<Vec<String>> = vec![vec![]];
        let results = compute_rrf_scores(&lists, 60.0);
        assert!(results.is_empty());
    }

    #[test]
    fn rrf_custom_k_value() {
        let lists = vec![vec!["A".to_string()]];
        let results = compute_rrf_scores(&lists, 10.0);

        // rank=1, k=10, score = 1/(10+1) = 1/11
        assert_eq!(results.len(), 1);
        assert!((results[0].1 - 1.0 / 11.0).abs() < 1e-10);
    }

    #[test]
    fn rrf_k_zero() {
        // k=0 makes the formula score = 1/rank
        let lists = vec![vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
        ]];
        let results = compute_rrf_scores(&lists, 0.0);

        assert!((results[0].1 - 1.0).abs() < 1e-10); // 1/1
        assert!((results[1].1 - 0.5).abs() < 1e-10); // 1/2
        assert!((results[2].1 - 1.0 / 3.0).abs() < 1e-10); // 1/3
    }

    // =====================================================================
    // Deterministic Ordering Tests
    // =====================================================================

    #[test]
    fn rrf_deterministic_tiebreaking_by_path() {
        // Two documents with exactly equal scores (same rank in one list each)
        let lists = vec![
            vec!["B_file".to_string()],
            vec!["A_file".to_string()],
        ];
        let results = compute_rrf_scores(&lists, 60.0);

        // Both have score 1/61. Tie broken by alphabetical order.
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "A_file");
        assert_eq!(results[1].0, "B_file");
        assert!((results[0].1 - results[1].1).abs() < 1e-10);
    }

    #[test]
    fn rrf_deterministic_ordering_is_stable() {
        // Run the same fusion multiple times and verify ordering is identical
        let lists = vec![
            vec![
                "zebra.rs".to_string(),
                "alpha.rs".to_string(),
                "middle.rs".to_string(),
            ],
            vec![
                "alpha.rs".to_string(),
                "zebra.rs".to_string(),
                "beta.rs".to_string(),
            ],
        ];

        let r1 = compute_rrf_scores(&lists, 60.0);
        let r2 = compute_rrf_scores(&lists, 60.0);
        let r3 = compute_rrf_scores(&lists, 60.0);

        // All runs must produce identical ordering
        for i in 0..r1.len() {
            assert_eq!(r1[i].0, r2[i].0);
            assert_eq!(r1[i].0, r3[i].0);
            assert!((r1[i].1 - r2[i].1).abs() < 1e-10);
            assert!((r1[i].1 - r3[i].1).abs() < 1e-10);
        }
    }

    #[test]
    fn rrf_many_tied_entries_sorted_alphabetically() {
        // All items appear once at rank 1 in separate lists => all tied
        let lists = vec![
            vec!["delta.rs".to_string()],
            vec!["alpha.rs".to_string()],
            vec!["charlie.rs".to_string()],
            vec!["bravo.rs".to_string()],
        ];
        let results = compute_rrf_scores(&lists, 60.0);

        // All have score 1/61. Should be sorted alphabetically.
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].0, "alpha.rs");
        assert_eq!(results[1].0, "bravo.rs");
        assert_eq!(results[2].0, "charlie.rs");
        assert_eq!(results[3].0, "delta.rs");
    }

    // =====================================================================
    // Single-Channel Scenario Tests
    // =====================================================================

    #[test]
    fn rrf_only_one_list_preserves_original_order() {
        let lists = vec![vec![
            "first.rs".to_string(),
            "second.rs".to_string(),
            "third.rs".to_string(),
        ]];
        let results = compute_rrf_scores(&lists, 60.0);

        // Original order should be preserved (descending score = ascending rank)
        assert_eq!(results[0].0, "first.rs");
        assert_eq!(results[1].0, "second.rs");
        assert_eq!(results[2].0, "third.rs");
        assert!(results[0].1 > results[1].1);
        assert!(results[1].1 > results[2].1);
    }

    #[test]
    fn rrf_disjoint_lists() {
        // Two lists with no overlap
        let lists = vec![
            vec!["A".to_string(), "B".to_string()],
            vec!["C".to_string(), "D".to_string()],
        ];
        let results = compute_rrf_scores(&lists, 60.0);

        assert_eq!(results.len(), 4);
        // A and C both at rank 1 in their respective lists -> same score
        // B and D both at rank 2 in their respective lists -> same score
        // Ties broken alphabetically: A before C, B before D
        assert_eq!(results[0].0, "A");
        assert_eq!(results[1].0, "C");
        assert_eq!(results[2].0, "B");
        assert_eq!(results[3].0, "D");
    }

    // =====================================================================
    // Provenance Tracking Tests (using HybridSearchEngine internals)
    // =====================================================================

    #[test]
    fn fuse_rrf_tracks_provenance() {
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        let engine = HybridSearchEngine::new(
            store,
            None,
            HybridSearchConfig {
                rrf_k: 60.0,
                max_results: 10,
                channel_max_results: 50,
            },
        );

        let channel_results = vec![
            (
                SearchChannel::Symbol,
                vec![
                    ChannelEntry {
                        file_path: "src/main.rs".to_string(),
                        symbol_name: Some("main".to_string()),
                        chunk_text: None,
                        semantic_similarity: None,
                    },
                    ChannelEntry {
                        file_path: "src/lib.rs".to_string(),
                        symbol_name: Some("init".to_string()),
                        chunk_text: None,
                        semantic_similarity: None,
                    },
                ],
            ),
            (
                SearchChannel::Semantic,
                vec![
                    ChannelEntry {
                        file_path: "src/main.rs".to_string(),
                        symbol_name: None,
                        chunk_text: Some("fn main() { ... }".to_string()),
                        semantic_similarity: Some(0.95),
                    },
                    ChannelEntry {
                        file_path: "src/utils.rs".to_string(),
                        symbol_name: None,
                        chunk_text: Some("utility functions".to_string()),
                        semantic_similarity: Some(0.80),
                    },
                ],
            ),
        ];

        let results = engine.fuse_rrf(&channel_results);

        // src/main.rs should have the highest score (appears in both channels)
        assert_eq!(results[0].file_path, "src/main.rs");

        // Verify provenance for src/main.rs
        assert_eq!(results[0].provenance.len(), 2);
        let symbol_contrib = results[0]
            .provenance
            .iter()
            .find(|p| p.channel == SearchChannel::Symbol)
            .expect("should have Symbol provenance");
        assert_eq!(symbol_contrib.rank, 1);
        assert!((symbol_contrib.rrf_contribution - 1.0 / 61.0).abs() < 1e-10);

        let semantic_contrib = results[0]
            .provenance
            .iter()
            .find(|p| p.channel == SearchChannel::Semantic)
            .expect("should have Semantic provenance");
        assert_eq!(semantic_contrib.rank, 1);
        assert!((semantic_contrib.rrf_contribution - 1.0 / 61.0).abs() < 1e-10);

        // Verify merged metadata
        assert_eq!(results[0].symbol_name.as_deref(), Some("main"));
        assert_eq!(
            results[0].chunk_text.as_deref(),
            Some("fn main() { ... }")
        );
        assert_eq!(results[0].semantic_similarity, Some(0.95));

        // src/lib.rs should only have Symbol provenance
        let lib_result = results
            .iter()
            .find(|r| r.file_path == "src/lib.rs")
            .expect("should have src/lib.rs");
        assert_eq!(lib_result.provenance.len(), 1);
        assert_eq!(lib_result.provenance[0].channel, SearchChannel::Symbol);
        assert_eq!(lib_result.provenance[0].rank, 2);

        // src/utils.rs should only have Semantic provenance
        let utils_result = results
            .iter()
            .find(|r| r.file_path == "src/utils.rs")
            .expect("should have src/utils.rs");
        assert_eq!(utils_result.provenance.len(), 1);
        assert_eq!(utils_result.provenance[0].channel, SearchChannel::Semantic);
        assert_eq!(utils_result.provenance[0].rank, 2);
    }

    #[test]
    fn fuse_rrf_respects_max_results() {
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        let engine = HybridSearchEngine::new(
            store,
            None,
            HybridSearchConfig {
                rrf_k: 60.0,
                max_results: 3,
                channel_max_results: 50,
            },
        );

        // Create a list with 10 entries
        let entries: Vec<ChannelEntry> = (0..10)
            .map(|i| ChannelEntry {
                file_path: format!("src/file_{:02}.rs", i),
                symbol_name: Some(format!("sym_{}", i)),
                chunk_text: None,
                semantic_similarity: None,
            })
            .collect();

        let channel_results = vec![(SearchChannel::Symbol, entries)];
        let results = engine.fuse_rrf(&channel_results);

        // Should be truncated to max_results = 3
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn fuse_rrf_empty_channels() {
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        let engine = HybridSearchEngine::with_defaults(store, None);
        let channel_results: Vec<(SearchChannel, Vec<ChannelEntry>)> = vec![];
        let results = engine.fuse_rrf(&channel_results);

        assert!(results.is_empty());
    }

    #[test]
    fn fuse_rrf_single_channel_preserves_rank_order() {
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        let engine = HybridSearchEngine::with_defaults(store, None);

        let entries = vec![
            ChannelEntry {
                file_path: "first.rs".to_string(),
                symbol_name: None,
                chunk_text: Some("first chunk".to_string()),
                semantic_similarity: Some(0.99),
            },
            ChannelEntry {
                file_path: "second.rs".to_string(),
                symbol_name: None,
                chunk_text: Some("second chunk".to_string()),
                semantic_similarity: Some(0.85),
            },
            ChannelEntry {
                file_path: "third.rs".to_string(),
                symbol_name: None,
                chunk_text: Some("third chunk".to_string()),
                semantic_similarity: Some(0.70),
            },
        ];

        let channel_results = vec![(SearchChannel::Semantic, entries)];
        let results = engine.fuse_rrf(&channel_results);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].file_path, "first.rs");
        assert_eq!(results[1].file_path, "second.rs");
        assert_eq!(results[2].file_path, "third.rs");
        assert!(results[0].score > results[1].score);
        assert!(results[1].score > results[2].score);
    }

    // =====================================================================
    // Integration-style Tests (with IndexStore)
    // =====================================================================

    #[test]
    fn search_symbols_channel_uses_index_store() {
        use super::super::analysis_index::{FileInventoryItem, SymbolInfo, SymbolKind};
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        // Populate the index
        let item = FileInventoryItem {
            path: "src/controller.rs".to_string(),
            component: "backend".to_string(),
            language: "rust".to_string(),
            extension: Some("rs".to_string()),
            size_bytes: 1024,
            line_count: 50,
            is_test: false,
            symbols: vec![
                SymbolInfo::basic("UserController".to_string(), SymbolKind::Struct, 5),
                SymbolInfo::basic("handle_request".to_string(), SymbolKind::Function, 15),
            ],
        };
        store
            .upsert_file_index("/project", &item, "hash1")
            .unwrap();

        let engine = HybridSearchEngine::with_defaults(store, None);
        let results = engine.search_symbols("Controller").unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "src/controller.rs");
        assert_eq!(results[0].symbol_name.as_deref(), Some("UserController"));
    }

    #[test]
    fn search_file_paths_channel_uses_index_store() {
        use super::super::analysis_index::FileInventoryItem;
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        // Populate the index
        let items = vec![
            FileInventoryItem {
                path: "src/services/auth.rs".to_string(),
                component: "backend".to_string(),
                language: "rust".to_string(),
                extension: Some("rs".to_string()),
                size_bytes: 512,
                line_count: 30,
                is_test: false,
                symbols: vec![],
            },
            FileInventoryItem {
                path: "src/services/user.rs".to_string(),
                component: "backend".to_string(),
                language: "rust".to_string(),
                extension: Some("rs".to_string()),
                size_bytes: 768,
                line_count: 45,
                is_test: false,
                symbols: vec![],
            },
            FileInventoryItem {
                path: "src/models/user.rs".to_string(),
                component: "backend".to_string(),
                language: "rust".to_string(),
                extension: Some("rs".to_string()),
                size_bytes: 256,
                line_count: 20,
                is_test: false,
                symbols: vec![],
            },
        ];

        for (i, item) in items.iter().enumerate() {
            store
                .upsert_file_index("/project", item, &format!("hash{}", i))
                .unwrap();
        }

        let engine = HybridSearchEngine::with_defaults(store, None);
        let results = engine.search_file_paths("user", "/project").unwrap();

        assert_eq!(results.len(), 2);
        // Both should contain "user" in the path
        for entry in &results {
            assert!(entry.file_path.contains("user"));
        }
    }

    // =====================================================================
    // HybridSearchResult Type Tests
    // =====================================================================

    #[test]
    fn hybrid_result_serialization_roundtrip() {
        let result = HybridSearchResult {
            file_path: "src/main.rs".to_string(),
            score: 0.032786885,
            provenance: vec![
                ChannelContribution {
                    channel: SearchChannel::Symbol,
                    rank: 1,
                    rrf_contribution: 1.0 / 61.0,
                },
                ChannelContribution {
                    channel: SearchChannel::Semantic,
                    rank: 3,
                    rrf_contribution: 1.0 / 63.0,
                },
            ],
            symbol_name: Some("main".to_string()),
            chunk_text: None,
            semantic_similarity: Some(0.92),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: HybridSearchResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.file_path, "src/main.rs");
        assert_eq!(deserialized.provenance.len(), 2);
        assert_eq!(deserialized.symbol_name.as_deref(), Some("main"));
        assert!(deserialized.chunk_text.is_none());
        assert_eq!(deserialized.semantic_similarity, Some(0.92));
    }

    #[test]
    fn search_channel_display() {
        assert_eq!(format!("{}", SearchChannel::Symbol), "symbol");
        assert_eq!(format!("{}", SearchChannel::FilePath), "file_path");
        assert_eq!(format!("{}", SearchChannel::Semantic), "semantic");
    }

    // =====================================================================
    // Config Tests
    // =====================================================================

    #[test]
    fn default_config_values() {
        let config = HybridSearchConfig::default();
        assert!((config.rrf_k - 60.0).abs() < 1e-10);
        assert_eq!(config.max_results, 20);
        assert_eq!(config.channel_max_results, 50);
    }

    #[test]
    fn config_serialization_roundtrip() {
        let config = HybridSearchConfig {
            rrf_k: 42.0,
            max_results: 15,
            channel_max_results: 30,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: HybridSearchConfig = serde_json::from_str(&json).unwrap();

        assert!((deserialized.rrf_k - 42.0).abs() < 1e-10);
        assert_eq!(deserialized.max_results, 15);
        assert_eq!(deserialized.channel_max_results, 30);
    }

    // =====================================================================
    // Rank Improvement Tests (multi-channel boosts results)
    // =====================================================================

    #[test]
    fn multi_channel_presence_boosts_score() {
        // A document appearing in multiple channels should rank higher
        // than one appearing in only one channel, even at a lower rank.
        let lists = vec![
            // Channel 1: [A at rank 1, B at rank 2]
            vec!["A".to_string(), "B".to_string()],
            // Channel 2: [B at rank 1, C at rank 2]
            vec!["B".to_string(), "C".to_string()],
        ];
        let results = compute_rrf_scores(&lists, 60.0);

        // B appears in both channels (rank 2 + rank 1)
        // A appears in only channel 1 (rank 1)
        // C appears in only channel 2 (rank 2)
        let score_a = 1.0 / 61.0; // rank 1 in one channel
        let score_b = 1.0 / 62.0 + 1.0 / 61.0; // rank 2 + rank 1
        let score_c = 1.0 / 62.0; // rank 2 in one channel

        // B should beat A because multi-channel presence gives it a higher fused score
        assert_eq!(results[0].0, "B");
        assert!((results[0].1 - score_b).abs() < 1e-10);

        assert_eq!(results[1].0, "A");
        assert!((results[1].1 - score_a).abs() < 1e-10);

        assert_eq!(results[2].0, "C");
        assert!((results[2].1 - score_c).abs() < 1e-10);
    }

    #[test]
    fn cross_channel_fusion_improves_relevance() {
        // Simulate: query "auth middleware"
        // Symbol channel finds: [auth_handler, middleware_fn, validate_token]
        // File path channel finds: [middleware.rs, auth.rs, config.rs]
        // Semantic channel finds: [auth.rs, middleware.rs, utils.rs]
        //
        // After fusion, auth.rs and middleware.rs should rank higher because
        // they appear in multiple channels.
        let lists = vec![
            vec![
                "auth_handler.rs".to_string(),
                "middleware_fn.rs".to_string(),
                "validate_token.rs".to_string(),
            ],
            vec![
                "middleware.rs".to_string(),
                "auth.rs".to_string(),
                "config.rs".to_string(),
            ],
            vec![
                "auth.rs".to_string(),
                "middleware.rs".to_string(),
                "utils.rs".to_string(),
            ],
        ];
        let results = compute_rrf_scores(&lists, 60.0);

        // auth.rs: rank 2 in file_path + rank 1 in semantic = 1/62 + 1/61
        // middleware.rs: rank 1 in file_path + rank 2 in semantic = 1/61 + 1/62
        // These two should be tied and sorted alphabetically
        assert_eq!(results[0].0, "auth.rs");
        assert_eq!(results[1].0, "middleware.rs");

        // Both should score higher than single-channel results
        let single_channel_max = 1.0 / 61.0; // best single-channel score
        assert!(results[0].1 > single_channel_max);
        assert!(results[1].1 > single_channel_max);
    }

    // =====================================================================
    // HNSW Integration Tests (feature-001 story-004)
    // =====================================================================

    #[test]
    fn hybrid_engine_with_hnsw_builder() {
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        // Engine without HNSW
        let engine = HybridSearchEngine::with_defaults(store.clone(), None);
        assert!(engine.hnsw_index.is_none());

        // Engine with HNSW via builder
        let hnsw = Arc::new(HnswIndex::new("/tmp/test_hnsw", 128));
        let engine = HybridSearchEngine::with_defaults(store.clone(), None)
            .with_hnsw_index(Some(hnsw));
        assert!(engine.hnsw_index.is_some());

        // Engine with None HNSW via builder
        let engine = HybridSearchEngine::with_defaults(store, None)
            .with_hnsw_index(None);
        assert!(engine.hnsw_index.is_none());
    }

    #[test]
    fn hybrid_engine_set_hnsw_index() {
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        let mut engine = HybridSearchEngine::with_defaults(store, None);
        assert!(engine.hnsw_index.is_none());

        let hnsw = Arc::new(HnswIndex::new("/tmp/test_hnsw", 128));
        engine.set_hnsw_index(hnsw);
        assert!(engine.hnsw_index.is_some());
    }

    #[tokio::test]
    async fn search_semantic_hnsw_returns_entries() {
        use crate::storage::database::Database;
        use super::super::embedding_service::embedding_to_bytes;
        use tempfile::tempdir;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        // Insert some embeddings into SQLite so we can look them up by ROWID
        let dim = 8;
        let embeddings: Vec<Vec<f32>> = (0..5).map(|i| {
            let mut v = vec![0.0f32; dim];
            v[i % dim] = 1.0;
            v
        }).collect();

        for (i, emb) in embeddings.iter().enumerate() {
            let emb_bytes = embedding_to_bytes(emb);
            store.upsert_chunk_embedding(
                "/test",
                &format!("src/file_{}.rs", i),
                i as i64,
                &format!("chunk text {}", i),
                &emb_bytes,
            ).unwrap();
        }

        // Create HNSW index and insert the same embeddings
        let dir = tempdir().expect("tempdir");
        let hnsw = Arc::new(HnswIndex::new(dir.path().join("hnsw"), dim));
        hnsw.initialize().await;

        // We need to insert using the SQLite ROWIDs
        // Get the ROWIDs from SQLite
        let all_ids = store.get_all_embedding_ids_and_vectors("/test").unwrap();
        for (rowid, vec) in &all_ids {
            hnsw.insert(*rowid, vec).await;
        }

        // Build engine with HNSW
        let engine = HybridSearchEngine::with_defaults(store, None)
            .with_hnsw_index(Some(hnsw.clone()));

        // Search for the first embedding
        let results = engine
            .search_semantic_hnsw(&embeddings[0], &hnsw, 3)
            .await
            .unwrap();

        assert!(!results.is_empty(), "HNSW search should return results");
        // First result should be the closest match
        assert!(
            results[0].semantic_similarity.unwrap() > 0.5,
            "Top result should have high similarity"
        );
        assert!(results[0].chunk_text.is_some(), "Should have chunk text");
    }

    #[tokio::test]
    async fn search_semantic_hnsw_empty_index_returns_empty() {
        use crate::storage::database::Database;
        use tempfile::tempdir;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        let dir = tempdir().expect("tempdir");
        let hnsw = Arc::new(HnswIndex::new(dir.path().join("hnsw"), 8));
        hnsw.initialize().await;

        let engine = HybridSearchEngine::with_defaults(store, None)
            .with_hnsw_index(Some(hnsw.clone()));

        let query = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let results = engine
            .search_semantic_hnsw(&query, &hnsw, 5)
            .await
            .unwrap();

        assert!(results.is_empty(), "Empty HNSW should return empty results");
    }

    // =====================================================================
    // FTS5 Integration Tests (story-004)
    // =====================================================================

    #[test]
    fn search_symbols_uses_fts_when_available() {
        use super::super::analysis_index::{FileInventoryItem, SymbolInfo, SymbolKind};
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        // Populate the index with symbols
        let item = FileInventoryItem {
            path: "src/auth_service.rs".to_string(),
            component: "backend".to_string(),
            language: "rust".to_string(),
            extension: Some("rs".to_string()),
            size_bytes: 1024,
            line_count: 50,
            is_test: false,
            symbols: vec![
                SymbolInfo::basic("auth_handler".to_string(), SymbolKind::Function, 5),
                SymbolInfo::basic("validate_token".to_string(), SymbolKind::Function, 15),
            ],
        };
        store.upsert_file_index("/project", &item, "hash1").unwrap();

        let engine = HybridSearchEngine::with_defaults(store, None);

        // Search for "auth" - should use FTS and find auth_handler
        let results = engine.search_symbols("auth").unwrap();
        assert!(!results.is_empty(), "FTS should find symbols matching 'auth'");
        assert_eq!(results[0].symbol_name.as_deref(), Some("auth_handler"));
    }

    #[test]
    fn search_file_paths_uses_fts_when_available() {
        use super::super::analysis_index::FileInventoryItem;
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        let items = vec![
            FileInventoryItem {
                path: "src/services/auth.rs".to_string(),
                component: "backend".to_string(),
                language: "rust".to_string(),
                extension: Some("rs".to_string()),
                size_bytes: 512,
                line_count: 30,
                is_test: false,
                symbols: vec![],
            },
            FileInventoryItem {
                path: "src/services/user.rs".to_string(),
                component: "backend".to_string(),
                language: "rust".to_string(),
                extension: Some("rs".to_string()),
                size_bytes: 768,
                line_count: 45,
                is_test: false,
                symbols: vec![],
            },
        ];

        for (i, item) in items.iter().enumerate() {
            store.upsert_file_index("/project", item, &format!("hash{}", i)).unwrap();
        }

        let engine = HybridSearchEngine::with_defaults(store, None);

        // Search for "auth" in file paths
        let results = engine.search_file_paths("auth", "/project").unwrap();
        assert!(!results.is_empty(), "FTS should find file paths matching 'auth'");
        assert_eq!(results[0].file_path, "src/services/auth.rs");
    }

    #[test]
    fn search_symbols_falls_back_to_like_on_short_query() {
        use super::super::analysis_index::{FileInventoryItem, SymbolInfo, SymbolKind};
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        let item = FileInventoryItem {
            path: "src/main.rs".to_string(),
            component: "backend".to_string(),
            language: "rust".to_string(),
            extension: Some("rs".to_string()),
            size_bytes: 1024,
            line_count: 50,
            is_test: false,
            symbols: vec![
                SymbolInfo::basic("x".to_string(), SymbolKind::Function, 1),
            ],
        };
        store.upsert_file_index("/project", &item, "hash1").unwrap();

        let engine = HybridSearchEngine::with_defaults(store, None);

        // Very short query ("x") should still return results via LIKE fallback
        let results = engine.search_symbols("x").unwrap();
        // Should find "x" via either FTS or LIKE fallback
        assert!(!results.is_empty(), "Short query should still find results");
    }

    #[test]
    fn fts_results_correctly_fused_with_rrf() {
        use super::super::analysis_index::{FileInventoryItem, SymbolInfo, SymbolKind};
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        // Create files where "auth" appears in both symbol names and file paths
        let item = FileInventoryItem {
            path: "src/auth.rs".to_string(),
            component: "backend".to_string(),
            language: "rust".to_string(),
            extension: Some("rs".to_string()),
            size_bytes: 1024,
            line_count: 50,
            is_test: false,
            symbols: vec![
                SymbolInfo::basic("auth_handler".to_string(), SymbolKind::Function, 5),
            ],
        };
        store.upsert_file_index("/project", &item, "hash1").unwrap();

        let engine = HybridSearchEngine::with_defaults(store, None);

        // "auth" should appear in both symbol and filepath channels
        let symbol_results = engine.search_symbols("auth").unwrap();
        let filepath_results = engine.search_file_paths("auth", "/project").unwrap();

        // Both channels should have results
        assert!(!symbol_results.is_empty(), "Symbol channel should find auth");
        assert!(!filepath_results.is_empty(), "FilePath channel should find auth");

        // src/auth.rs should appear in both, meaning it would get a boosted
        // RRF score during fusion
        assert_eq!(symbol_results[0].file_path, "src/auth.rs");
        assert_eq!(filepath_results[0].file_path, "src/auth.rs");
    }

    #[test]
    fn existing_hybrid_search_tests_still_pass_after_fts_wiring() {
        // This meta-test verifies that modifying search_symbols and
        // search_file_paths to use FTS does not break existing functionality.
        // The test below re-verifies the same assertions as the original
        // search_symbols_channel_uses_index_store test.
        use super::super::analysis_index::{FileInventoryItem, SymbolInfo, SymbolKind};
        use crate::storage::database::Database;

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        let item = FileInventoryItem {
            path: "src/controller.rs".to_string(),
            component: "backend".to_string(),
            language: "rust".to_string(),
            extension: Some("rs".to_string()),
            size_bytes: 1024,
            line_count: 50,
            is_test: false,
            symbols: vec![
                SymbolInfo::basic("user_controller".to_string(), SymbolKind::Struct, 5),
                SymbolInfo::basic("handle_request".to_string(), SymbolKind::Function, 15),
            ],
        };
        store.upsert_file_index("/project", &item, "hash1").unwrap();

        let engine = HybridSearchEngine::with_defaults(store, None);
        let results = engine.search_symbols("user_controller").unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].file_path, "src/controller.rs");
        assert_eq!(results[0].symbol_name.as_deref(), Some("user_controller"));
    }
}
