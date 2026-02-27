//! Codebase Index Management Commands
//!
//! Provides Tauri commands for browsing, inspecting, and managing
//! workspace codebase indexes from the frontend Codebase panel.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::orchestrator::embedding_service::SemanticSearchResult;
use crate::services::orchestrator::index_manager::IndexStatusEvent;
use crate::services::orchestrator::index_store::{
    EmbeddingMetadata, FileIndexRow, IndexedProjectEntry, LanguageBreakdown, ProjectIndexSummary,
};

use super::standalone::StandaloneState;

/// Composite detail for a codebase project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseProjectDetail {
    pub project_path: String,
    pub summary: ProjectIndexSummary,
    pub languages: Vec<LanguageBreakdown>,
    pub embedding_metadata: Vec<EmbeddingMetadata>,
    pub status: IndexStatusEvent,
}

/// Paginated file listing result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseFileListResult {
    pub files: Vec<FileIndexRow>,
    pub total: usize,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// List all indexed projects with file counts.
#[tauri::command]
pub async fn codebase_list_projects(
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<Vec<IndexedProjectEntry>>, String> {
    let mgr_lock = standalone_state.index_manager.read().await;
    let mgr = match &*mgr_lock {
        Some(mgr) => mgr,
        None => return Ok(CommandResponse::ok(vec![])),
    };

    match mgr.index_store().list_indexed_projects() {
        Ok(projects) => Ok(CommandResponse::ok(projects)),
        Err(e) => Ok(CommandResponse::err(format!(
            "Failed to list indexed projects: {}",
            e
        ))),
    }
}

/// Get detailed information about a specific project's codebase index.
#[tauri::command]
pub async fn codebase_get_project_detail(
    project_path: String,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<CodebaseProjectDetail>, String> {
    let mgr_lock = standalone_state.index_manager.read().await;
    let mgr = match &*mgr_lock {
        Some(mgr) => mgr,
        None => {
            return Ok(CommandResponse::err(
                "IndexManager not initialized".to_string(),
            ))
        }
    };

    let index_store = mgr.index_store();

    let summary = match index_store.get_project_summary(&project_path) {
        Ok(s) => s,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to get project summary: {}",
                e
            )))
        }
    };

    let languages = index_store
        .get_language_breakdown(&project_path)
        .unwrap_or_default();

    let embedding_metadata = index_store
        .get_embedding_metadata(&project_path)
        .unwrap_or_default();

    let status = mgr.get_status(&project_path).await;

    Ok(CommandResponse::ok(CodebaseProjectDetail {
        project_path,
        summary,
        languages,
        embedding_metadata,
        status,
    }))
}

/// List files for a project with optional language filter and pagination.
#[tauri::command]
pub async fn codebase_list_files(
    project_path: String,
    language_filter: Option<String>,
    search_pattern: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<CodebaseFileListResult>, String> {
    let mgr_lock = standalone_state.index_manager.read().await;
    let mgr = match &*mgr_lock {
        Some(mgr) => mgr,
        None => {
            return Ok(CommandResponse::err(
                "IndexManager not initialized".to_string(),
            ))
        }
    };

    let off = offset.unwrap_or(0);
    let lim = limit.unwrap_or(50);

    match mgr.index_store().list_project_files(
        &project_path,
        language_filter.as_deref(),
        search_pattern.as_deref(),
        off,
        lim,
    ) {
        Ok((files, total)) => Ok(CommandResponse::ok(CodebaseFileListResult { files, total })),
        Err(e) => Ok(CommandResponse::err(format!(
            "Failed to list project files: {}",
            e
        ))),
    }
}

/// Delete a project's codebase index.
///
/// Removes the in-memory state via `IndexManager::remove_directory` and
/// then deletes the persisted SQLite rows via `IndexStore::delete_project_index`.
#[tauri::command]
pub async fn codebase_delete_project(
    project_path: String,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<usize>, String> {
    let mgr_lock = standalone_state.index_manager.read().await;
    let mgr = match &*mgr_lock {
        Some(mgr) => mgr,
        None => {
            return Ok(CommandResponse::err(
                "IndexManager not initialized".to_string(),
            ))
        }
    };

    // Remove in-memory state (abort indexer, clear caches)
    mgr.remove_directory(&project_path).await;

    // Delete persisted index data
    match mgr.index_store().delete_project_index(&project_path) {
        Ok(deleted) => Ok(CommandResponse::ok(deleted)),
        Err(e) => Ok(CommandResponse::err(format!(
            "Failed to delete project index: {}",
            e
        ))),
    }
}

/// Perform a semantic search over a project's indexed embeddings.
///
/// Delegates to the same logic as the `semantic_search` command in standalone.rs
/// but accepts an explicit `project_path` (no fallback to working directory).
#[tauri::command]
pub async fn codebase_search(
    project_path: String,
    query: String,
    top_k: Option<usize>,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<Vec<SemanticSearchResult>>, String> {
    if query.trim().is_empty() {
        return Ok(CommandResponse::err("Query string is empty"));
    }

    let mgr_lock = standalone_state.index_manager.read().await;
    let mgr = match &*mgr_lock {
        Some(mgr) => mgr,
        None => {
            return Ok(CommandResponse::err(
                "Semantic search not available: IndexManager not initialized.",
            ))
        }
    };

    let index_store = mgr.index_store();

    let embedding_count = match index_store.count_embeddings(&project_path) {
        Ok(count) => count,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to count embeddings: {}",
                e
            )))
        }
    };

    if embedding_count == 0 {
        return Ok(CommandResponse::ok(vec![]));
    }

    let k = top_k.unwrap_or(10);

    // Use EmbeddingManager if available
    if let Some(emb_mgr) = mgr.get_embedding_manager(&project_path).await {
        match emb_mgr.embed_query(&query).await {
            Ok(query_embedding) if !query_embedding.is_empty() => {
                // Try HNSW search first, fall back to brute-force
                if let Some(hnsw) = mgr.get_hnsw_index(&project_path).await {
                    if hnsw.is_ready().await {
                        let hnsw_hits = hnsw.search(&query_embedding, k).await;
                        if !hnsw_hits.is_empty() {
                            let rowids: Vec<usize> =
                                hnsw_hits.iter().map(|(id, _)| *id).collect();
                            match index_store.get_embeddings_by_rowids(&rowids) {
                                Ok(metadata) => {
                                    let results: Vec<SemanticSearchResult> = hnsw_hits
                                        .into_iter()
                                        .filter_map(|(id, distance)| {
                                            metadata.get(&id).map(
                                                |(file_path, chunk_index, chunk_text)| {
                                                    SemanticSearchResult {
                                                        file_path: file_path.clone(),
                                                        chunk_index: *chunk_index,
                                                        chunk_text: chunk_text.clone(),
                                                        similarity: 1.0 - distance,
                                                    }
                                                },
                                            )
                                        })
                                        .collect();
                                    return Ok(CommandResponse::ok(results));
                                }
                                Err(e) => {
                                    return Ok(CommandResponse::err(format!(
                                        "HNSW search failed to fetch metadata: {}",
                                        e
                                    )))
                                }
                            }
                        }
                    }
                }
                // Fall back to brute-force SQLite search
                match index_store.semantic_search(&query_embedding, &project_path, k) {
                    Ok(results) => return Ok(CommandResponse::ok(results)),
                    Err(e) => {
                        return Ok(CommandResponse::err(format!(
                            "Brute-force semantic search failed: {}",
                            e
                        )))
                    }
                }
            }
            Ok(_) => {
                return Ok(CommandResponse::err(
                    "Embedding provider returned empty vector for query.",
                ))
            }
            Err(e) => {
                return Ok(CommandResponse::err(format!(
                    "Failed to embed query: {}",
                    e
                )))
            }
        }
    }

    // No embedding manager — rebuild a temporary TF-IDF vocabulary
    let embedding_service =
        crate::services::orchestrator::embedding_service::EmbeddingService::new();

    let all_chunks = match index_store.get_embeddings_for_project(&project_path) {
        Ok(chunks) => chunks,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to get embeddings: {}",
                e
            )))
        }
    };

    let chunk_texts: Vec<&str> = all_chunks
        .iter()
        .map(|(_, _, text, _)| text.as_str())
        .collect();
    embedding_service.build_vocabulary(&chunk_texts);

    let query_embedding = embedding_service.embed_text(&query);
    if query_embedding.is_empty() {
        return Ok(CommandResponse::ok(vec![]));
    }

    match index_store.semantic_search(&query_embedding, &project_path, k) {
        Ok(results) => Ok(CommandResponse::ok(results)),
        Err(e) => Ok(CommandResponse::err(format!(
            "Semantic search failed: {}",
            e
        ))),
    }
}

/// Trigger LLM-based component classification for a project.
///
/// Uses the configured LLM provider to analyze directory structure and
/// derive meaningful component names.  Falls back to heuristic (top-level
/// directory names) when no LLM is available.
#[tauri::command]
pub async fn classify_codebase_components(
    project_path: String,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<ClassifyComponentsResult>, String> {
    let mgr_lock = standalone_state.index_manager.read().await;
    let mgr = match &*mgr_lock {
        Some(mgr) => mgr,
        None => {
            return Ok(CommandResponse::err(
                "IndexManager not initialized".to_string(),
            ))
        }
    };

    let result = mgr.classify_components(&project_path).await;

    Ok(CommandResponse::ok(ClassifyComponentsResult {
        source: match result.source {
            crate::services::orchestrator::component_classifier::ClassificationSource::Llm => {
                "llm".to_string()
            }
            crate::services::orchestrator::component_classifier::ClassificationSource::Heuristic => {
                "heuristic".to_string()
            }
        },
        mappings_count: result.mappings.len(),
        files_updated: result.files_updated,
    }))
}

/// Result of a component classification operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyComponentsResult {
    pub source: String,
    pub mappings_count: usize,
    pub files_updated: usize,
}
