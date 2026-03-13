use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::embedding_manager::EmbeddingManager;
use super::hnsw_index::HnswIndex;
use super::hybrid_search::{HybridSearchEngine, SearchChannel};
use super::index_store::IndexStore;

/// Canonical request shape shared by IPC and the CodebaseSearch tool.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodebaseSearchRequest {
    pub project_path: String,
    pub query: String,
    #[serde(default)]
    pub modes: Vec<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub include_snippet: Option<bool>,
    pub filters: Option<CodebaseSearchFilters>,
}

/// Optional filters for codebase search.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodebaseSearchFilters {
    pub component: Option<String>,
    pub language: Option<String>,
    pub file_path_prefix: Option<String>,
}

/// Single channel score contribution for a search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchChannelScore {
    pub channel: String,
    pub rank: usize,
    pub score: f64,
}

/// Search hit returned by codebase search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub file_path: String,
    pub symbol_name: Option<String>,
    pub snippet: Option<String>,
    pub similarity: Option<f32>,
    pub score: f64,
    pub score_breakdown: Vec<SearchChannelScore>,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub component: Option<String>,
    pub language: Option<String>,
    pub channels: Vec<String>,
    pub query_id: String,
}

/// Search diagnostics for UI/tool troubleshooting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSearchDiagnostics {
    pub query_id: String,
    pub active_channels: Vec<String>,
    pub semantic_degraded: bool,
    pub semantic_error: Option<String>,
    pub provider_display: Option<String>,
    pub embedding_dimension: usize,
    pub hnsw_used: bool,
    pub hnsw_vector_count: usize,
}

/// Canonical search response shape shared by IPC and tool code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSearchResponse {
    pub hits: Vec<SearchHit>,
    pub total: usize,
    pub semantic_degraded: bool,
    pub semantic_error: Option<String>,
    pub query_id: String,
    pub diagnostics: Option<CodeSearchDiagnostics>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RequestedSearchMode {
    Hybrid,
    Symbol,
    Path,
    Semantic,
}

impl RequestedSearchMode {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "hybrid" => Some(Self::Hybrid),
            "symbol" => Some(Self::Symbol),
            "path" => Some(Self::Path),
            "semantic" => Some(Self::Semantic),
            _ => None,
        }
    }
}

fn parse_requested_modes(raw_modes: &[String]) -> Result<HashSet<RequestedSearchMode>, String> {
    if raw_modes.is_empty() {
        let mut defaults = HashSet::new();
        defaults.insert(RequestedSearchMode::Hybrid);
        return Ok(defaults);
    }

    let mut parsed = HashSet::new();
    for raw in raw_modes {
        let mode_str = raw.trim();
        let mode = RequestedSearchMode::parse(mode_str).ok_or_else(|| {
            format!(
                "Invalid mode '{}'. Use one of: hybrid, symbol, path, semantic.",
                if mode_str.is_empty() {
                    "<empty>"
                } else {
                    mode_str
                }
            )
        })?;
        parsed.insert(mode);
    }
    Ok(parsed)
}

fn channel_matches_modes(
    channel: SearchChannel,
    requested_modes: &HashSet<RequestedSearchMode>,
) -> bool {
    if requested_modes.contains(&RequestedSearchMode::Hybrid) {
        return true;
    }
    match channel {
        SearchChannel::Symbol => requested_modes.contains(&RequestedSearchMode::Symbol),
        SearchChannel::FilePath => requested_modes.contains(&RequestedSearchMode::Path),
        SearchChannel::Semantic => requested_modes.contains(&RequestedSearchMode::Semantic),
    }
}

/// Shared search service used by Tauri command and CodebaseSearch tool.
pub struct CodebaseSearchService {
    index_store: Arc<IndexStore>,
    embedding_manager: Option<Arc<EmbeddingManager>>,
    hnsw_index: Option<Arc<HnswIndex>>,
}

impl CodebaseSearchService {
    pub fn new(
        index_store: Arc<IndexStore>,
        embedding_manager: Option<Arc<EmbeddingManager>>,
        hnsw_index: Option<Arc<HnswIndex>>,
    ) -> Self {
        Self {
            index_store,
            embedding_manager,
            hnsw_index,
        }
    }

    pub async fn search(
        &self,
        request: CodebaseSearchRequest,
    ) -> Result<CodeSearchResponse, String> {
        if request.project_path.trim().is_empty() {
            return Err("project_path is empty".to_string());
        }
        if request.query.trim().is_empty() {
            return Err("query is empty".to_string());
        }

        let resolved_project_path = self
            .index_store
            .resolve_equivalent_project_path(&request.project_path)
            .unwrap_or_else(|_| request.project_path.clone());

        let mut engine = HybridSearchEngine::with_defaults(
            Arc::clone(&self.index_store),
            self.embedding_manager.clone(),
        );
        if let Some(hnsw) = self.hnsw_index.clone() {
            engine.set_hnsw_index(hnsw);
        }

        let outcome = engine
            .search(&request.query, &resolved_project_path)
            .await
            .map_err(|e| format!("Search failed: {}", e))?;

        let parsed_modes = parse_requested_modes(&request.modes)?;
        let include_snippet = request.include_snippet.unwrap_or(true);
        let filters = request.filters.unwrap_or_default();
        let component_filter = filters
            .component
            .as_ref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty());
        let path_prefix_filter = filters
            .file_path_prefix
            .as_ref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty());

        let allowed_language_paths: Option<HashSet<String>> = if let Some(lang) = filters
                .language
                .as_ref()
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
        {
            match self.index_store.list_project_files(
                &resolved_project_path,
                Some(lang),
                None,
                0,
                100_000,
            ) {
                Ok((files, _)) => Some(files.into_iter().map(|f| f.file_path).collect()),
                Err(_) => Some(HashSet::new()),
            }
        } else {
            None
        };

        let allowed_component_paths: Option<HashSet<String>> =
            if let Some(component) = component_filter {
                match self
                    .index_store
                    .query_files_by_component(&resolved_project_path, component)
                {
                    Ok(files) => Some(files.into_iter().map(|f| f.file_path).collect()),
                    Err(_) => Some(HashSet::new()),
                }
            } else {
                None
            };

        let query_id = uuid::Uuid::new_v4().to_string();
        let mut file_meta_cache: HashMap<String, (Option<String>, Option<String>)> = HashMap::new();
        let mut symbol_line_cache: HashMap<(String, String), (Option<usize>, Option<usize>)> =
            HashMap::new();

        let mut hits: Vec<SearchHit> = outcome
            .results
            .into_iter()
            .filter(|result| {
                if !result
                    .provenance
                    .iter()
                    .any(|p| channel_matches_modes(p.channel, &parsed_modes))
                {
                    return false;
                }

                if let Some(ref allowed) = allowed_component_paths {
                    if !allowed.contains(&result.file_path) {
                        return false;
                    }
                }

                if let Some(prefix) = path_prefix_filter {
                    if !result.file_path.starts_with(prefix) {
                        return false;
                    }
                }

                if let Some(ref allowed) = allowed_language_paths {
                    if !allowed.contains(&result.file_path) {
                        return false;
                    }
                }

                true
            })
            .map(|result| {
                let file_path = result.file_path;
                let symbol_name = result.symbol_name;

                let (component, language) = if let Some(found) = file_meta_cache.get(&file_path) {
                    found.clone()
                } else {
                    let metadata = match self
                        .index_store
                        .query_files_by_path(&resolved_project_path, &file_path)
                    {
                        Ok(rows) => rows
                            .into_iter()
                            .find(|row| row.file_path == file_path)
                            .map(|row| (Some(row.component), Some(row.language)))
                            .unwrap_or((None, None)),
                        Err(_) => (None, None),
                    };
                    file_meta_cache.insert(file_path.clone(), metadata.clone());
                    metadata
                };

                let (line_start, line_end) = if let Some(symbol) = symbol_name.as_ref() {
                    let key = (file_path.clone(), symbol.clone());
                    if let Some(lines) = symbol_line_cache.get(&key) {
                        *lines
                    } else {
                        let resolved = match self
                            .index_store
                            .get_file_symbols(&resolved_project_path, &file_path)
                        {
                            Ok(symbols) => symbols
                                .into_iter()
                                .find(|item| item.name == *symbol)
                                .map(|item| {
                                    let end_line = if item.end_line > 0 {
                                        Some(item.end_line)
                                    } else {
                                        Some(item.line)
                                    };
                                    (Some(item.line), end_line)
                                })
                                .unwrap_or((None, None)),
                            Err(_) => (None, None),
                        };
                        symbol_line_cache.insert(key, resolved);
                        resolved
                    }
                } else {
                    (None, None)
                };

                let channels: Vec<String> = result
                    .provenance
                    .iter()
                    .map(|p| p.channel.to_string())
                    .collect();

                SearchHit {
                    file_path,
                    symbol_name,
                    snippet: if include_snippet {
                        result.chunk_text
                    } else {
                        None
                    },
                    similarity: result.semantic_similarity,
                    score: result.score,
                    score_breakdown: result
                        .provenance
                        .into_iter()
                        .map(|p| SearchChannelScore {
                            channel: p.channel.to_string(),
                            rank: p.rank,
                            score: p.rrf_contribution,
                        })
                        .collect(),
                    line_start,
                    line_end,
                    component,
                    language,
                    channels,
                    query_id: query_id.clone(),
                }
            })
            .collect();

        let total = hits.len();
        let offset = request.offset.unwrap_or(0).min(total);
        let limit = request.limit.unwrap_or(20).clamp(1, 100);
        hits = hits.into_iter().skip(offset).take(limit).collect();

        let semantic_degraded = outcome.semantic_degraded;
        let semantic_error = outcome.semantic_error.clone();
        let provider_display = outcome.provider_display.clone();
        let embedding_dimension = outcome.embedding_dimension;
        let hnsw_used = outcome.hnsw_used;
        let hnsw_vector_count = outcome.hnsw_vector_count;
        let active_channels: Vec<String> = outcome
            .active_channels
            .iter()
            .map(|channel| channel.to_string())
            .collect();

        Ok(CodeSearchResponse {
            hits,
            total,
            semantic_degraded,
            semantic_error: semantic_error.clone(),
            query_id: query_id.clone(),
            diagnostics: Some(CodeSearchDiagnostics {
                query_id,
                active_channels,
                semantic_degraded,
                semantic_error,
                provider_display,
                embedding_dimension,
                hnsw_used,
                hnsw_vector_count,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_modes_defaults_to_hybrid() {
        let parsed = parse_requested_modes(&[]).expect("modes should parse");
        assert!(parsed.contains(&RequestedSearchMode::Hybrid));
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn parse_modes_accepts_v2_modes_only() {
        let parsed = parse_requested_modes(&[
            "symbol".to_string(),
            "path".to_string(),
            "semantic".to_string(),
        ])
        .expect("valid v2 modes should parse");
        assert!(parsed.contains(&RequestedSearchMode::Symbol));
        assert!(parsed.contains(&RequestedSearchMode::Path));
        assert!(parsed.contains(&RequestedSearchMode::Semantic));
        assert!(!parsed.contains(&RequestedSearchMode::Hybrid));
    }

    #[test]
    fn parse_modes_rejects_legacy_aliases() {
        let err = parse_requested_modes(&["all".to_string()]).expect_err("legacy mode must fail");
        assert!(err.contains("Invalid mode 'all'"));
    }

    #[test]
    fn channel_matching_hybrid_matches_all_channels() {
        let modes = parse_requested_modes(&["hybrid".to_string()]).expect("hybrid should parse");
        assert!(channel_matches_modes(SearchChannel::Symbol, &modes));
        assert!(channel_matches_modes(SearchChannel::FilePath, &modes));
        assert!(channel_matches_modes(SearchChannel::Semantic, &modes));
    }
}
