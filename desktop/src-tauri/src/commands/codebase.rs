//! Codebase Index Management Commands
//!
//! Provides Tauri commands for browsing, inspecting, and managing
//! workspace codebase indexes from the frontend Codebase panel.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{Emitter, State};

use crate::models::response::CommandResponse;
use crate::services::orchestrator::hybrid_search::{HybridSearchEngine, SearchChannel};
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

/// Request for codebase search v2.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodebaseSearchV2Request {
    pub project_path: String,
    pub query: String,
    #[serde(default)]
    pub modes: Vec<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub include_snippet: Option<bool>,
    pub filters: Option<CodebaseSearchFilters>,
}

/// Optional filters for codebase search v2.
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

/// Search hit for codebase search v2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub file_path: String,
    pub symbol_name: Option<String>,
    pub snippet: Option<String>,
    pub similarity: Option<f32>,
    pub score: f64,
    pub score_breakdown: Vec<SearchChannelScore>,
}

/// Response payload for codebase search v2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSearchResponse {
    pub hits: Vec<SearchHit>,
    pub total: usize,
    pub semantic_degraded: bool,
    pub semantic_error: Option<String>,
}

/// File excerpt response for preview panel integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileExcerptResult {
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub total_lines: usize,
    pub content: String,
}

/// Context item for cross-mode context handoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItem {
    pub r#type: String,
    pub project_path: String,
    pub file_path: String,
    pub symbol_name: Option<String>,
    pub snippet: Option<String>,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub score: Option<f64>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodebaseContextAddedEvent {
    pub target_mode: String,
    pub items: Vec<ContextItem>,
}

fn resolve_project_file_path(project_path: &str, file_path: &str) -> Result<PathBuf, String> {
    let project_root =
        std::fs::canonicalize(project_path).map_err(|e| format!("Invalid project path: {}", e))?;

    let candidate = if Path::new(file_path).is_absolute() {
        PathBuf::from(file_path)
    } else {
        project_root.join(file_path)
    };

    let resolved =
        std::fs::canonicalize(&candidate).map_err(|e| format!("Invalid file path: {}", e))?;

    if !resolved.starts_with(&project_root) {
        return Err("File path escapes project root".to_string());
    }

    Ok(resolved)
}

fn open_file_in_editor(
    path: &Path,
    line: Option<usize>,
    column: Option<usize>,
) -> Result<(), String> {
    let file = path
        .to_str()
        .ok_or_else(|| "Failed to encode file path".to_string())?;
    let line = line.unwrap_or(1);
    let column = column.unwrap_or(1);

    if let Ok(editor) = std::env::var("EDITOR") {
        let editor_lc = editor.to_lowercase();
        let mut cmd = Command::new(editor.as_str());
        if editor_lc.contains("code") || editor_lc.contains("cursor") {
            cmd.arg("--goto")
                .arg(format!("{}:{}:{}", file, line, column));
        } else {
            cmd.arg(file);
        }
        cmd.spawn()
            .map_err(|e| format!("Failed to launch editor {}: {}", editor, e))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(file)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(file)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", file])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Unsupported platform for opening files".to_string())
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

/// Perform a multi-channel codebase search with mode filtering and pagination.
#[tauri::command]
pub async fn codebase_search_v2(
    request: CodebaseSearchV2Request,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<CodeSearchResponse>, String> {
    if request.project_path.trim().is_empty() {
        return Ok(CommandResponse::err("project_path is empty"));
    }
    if request.query.trim().is_empty() {
        return Ok(CommandResponse::err("query is empty"));
    }

    let mgr_lock = standalone_state.index_manager.read().await;
    let mgr = match &*mgr_lock {
        Some(mgr) => mgr,
        None => return Ok(CommandResponse::err("IndexManager not initialized")),
    };

    let index_store = mgr.index_store();
    let mut engine = HybridSearchEngine::with_defaults(
        mgr.index_store_arc(),
        mgr.get_embedding_manager(&request.project_path).await,
    );
    if let Some(hnsw) = mgr.get_hnsw_index(&request.project_path).await {
        engine.set_hnsw_index(hnsw);
    }

    let outcome = match engine.search(&request.query, &request.project_path).await {
        Ok(v) => v,
        Err(e) => return Ok(CommandResponse::err(format!("Search failed: {}", e))),
    };

    let parsed_modes = match parse_requested_modes(&request.modes) {
        Ok(modes) => modes,
        Err(e) => return Ok(CommandResponse::err(e)),
    };
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
        match index_store.list_project_files(&request.project_path, Some(lang), None, 0, 100_000) {
            Ok((files, _)) => Some(files.into_iter().map(|f| f.file_path).collect()),
            Err(_) => Some(HashSet::new()),
        }
    } else {
        None
    };
    let allowed_component_paths: Option<HashSet<String>> = if let Some(component) = component_filter
    {
        match index_store.query_files_by_component(&request.project_path, component) {
            Ok(files) => Some(files.into_iter().map(|f| f.file_path).collect()),
            Err(_) => Some(HashSet::new()),
        }
    } else {
        None
    };

    let mut hits: Vec<SearchHit> = outcome
        .results
        .into_iter()
        .filter(|result| {
            // Keep hits when any contributing channel is requested.
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
        .map(|result| SearchHit {
            file_path: result.file_path,
            symbol_name: result.symbol_name,
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
        })
        .collect();

    let total = hits.len();
    let offset = request.offset.unwrap_or(0).min(total);
    let limit = request.limit.unwrap_or(20).clamp(1, 100);
    hits = hits.into_iter().skip(offset).take(limit).collect();

    Ok(CommandResponse::ok(CodeSearchResponse {
        hits,
        total,
        semantic_degraded: outcome.semantic_degraded,
        semantic_error: outcome.semantic_error,
    }))
}

/// Fetch a safe, line-bounded file excerpt under the project root.
#[tauri::command]
pub async fn codebase_get_file_excerpt(
    project_path: String,
    file_path: String,
    line_start: usize,
    line_end: usize,
) -> Result<CommandResponse<FileExcerptResult>, String> {
    let resolved = match resolve_project_file_path(&project_path, &file_path) {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let raw = match std::fs::read_to_string(&resolved) {
        Ok(v) => v,
        Err(e) => return Ok(CommandResponse::err(format!("Failed to read file: {}", e))),
    };

    let lines: Vec<&str> = raw.lines().collect();
    let total_lines = lines.len();
    if total_lines == 0 {
        return Ok(CommandResponse::ok(FileExcerptResult {
            file_path,
            line_start: 1,
            line_end: 1,
            total_lines: 0,
            content: String::new(),
        }));
    }

    let start = line_start.max(1).min(total_lines);
    let end = line_end.max(start).min(total_lines);

    let content = lines[(start - 1)..end].join("\n");
    Ok(CommandResponse::ok(FileExcerptResult {
        file_path,
        line_start: start,
        line_end: end,
        total_lines,
        content,
    }))
}

/// Open a project file in the user's editor.
#[tauri::command]
pub async fn codebase_open_in_editor(
    project_path: String,
    file_path: String,
    line: Option<usize>,
    column: Option<usize>,
) -> Result<CommandResponse<bool>, String> {
    let resolved = match resolve_project_file_path(&project_path, &file_path) {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    match open_file_in_editor(&resolved, line, column) {
        Ok(()) => Ok(CommandResponse::ok(true)),
        Err(e) => Ok(CommandResponse::err(e)),
    }
}

/// Emit a cross-mode context handoff event for Chat/Plan/Task integration.
#[tauri::command]
pub async fn codebase_add_context(
    target_mode: String,
    items: Vec<ContextItem>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<usize>, String> {
    let payload = CodebaseContextAddedEvent {
        target_mode,
        items: items.clone(),
    };
    let _ = app_handle.emit("codebase-context-added", &payload);
    Ok(CommandResponse::ok(items.len()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_requested_modes_defaults_to_hybrid() {
        let parsed = parse_requested_modes(&[]).expect("modes should parse");
        assert!(parsed.contains(&RequestedSearchMode::Hybrid));
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn parse_requested_modes_accepts_v2_modes_only() {
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
    fn parse_requested_modes_rejects_legacy_aliases() {
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
