//! Codebase Index Management Commands
//!
//! Provides Tauri commands for browsing, inspecting, and managing
//! workspace codebase indexes from the frontend Codebase panel.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{Emitter, State};

use crate::models::response::CommandResponse;
use crate::services::orchestrator::codebase_search_service::CodebaseSearchService;
use crate::services::orchestrator::index_manager::IndexStatusEvent;
use crate::services::orchestrator::index_store::{
    EmbeddingMetadata, FileIndexRow, IndexedProjectEntry, LanguageBreakdown, ProjectIndexSummary,
};
use crate::services::workflow_kernel::{HandoffContextBundle, WorkflowKernelState, WorkflowMode};

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

/// Indexed project list entry with live status snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedProjectStatusEntry {
    pub project_path: String,
    pub file_count: usize,
    pub last_indexed_at: Option<String>,
    pub status: IndexStatusEvent,
}

/// Paginated file listing result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseFileListResult {
    pub files: Vec<FileIndexRow>,
    pub total: usize,
}

pub use crate::services::orchestrator::codebase_search_service::{
    CodeSearchDiagnostics, CodeSearchResponse, CodebaseSearchFilters,
    CodebaseSearchRequest as CodebaseSearchV2Request, SearchChannelScore, SearchHit,
};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseContextAppendResult {
    pub appended_count: usize,
    pub context_ref_ids: Vec<String>,
    pub session_id: String,
    pub target_mode: String,
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

fn parse_workflow_mode(value: &str) -> Result<WorkflowMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "chat" => Ok(WorkflowMode::Chat),
        "plan" => Ok(WorkflowMode::Plan),
        "task" => Ok(WorkflowMode::Task),
        _ => Err("mode_mismatch: target_mode must be one of: chat, plan, task".to_string()),
    }
}

fn validate_context_items(items: &[ContextItem]) -> Result<(), String> {
    let first_project = items
        .first()
        .map(|item| item.project_path.as_str())
        .unwrap_or_default();

    for (idx, item) in items.iter().enumerate() {
        if item.project_path.trim().is_empty() {
            return Err(format!(
                "context_validation_failed: items[{idx}].project_path is empty"
            ));
        }
        if item.file_path.trim().is_empty() {
            return Err(format!(
                "context_validation_failed: items[{idx}].file_path is empty"
            ));
        }
        if let Some(line_start) = item.line_start {
            if line_start == 0 {
                return Err(format!(
                    "context_validation_failed: items[{idx}].line_start must be >= 1"
                ));
            }
            if let Some(line_end) = item.line_end {
                if line_end < line_start {
                    return Err(format!(
                        "context_validation_failed: items[{idx}] line_end must be >= line_start"
                    ));
                }
            }
        }
        if item.project_path != first_project {
            return Err(
                "context_validation_failed: mixed project_path values are not supported"
                    .to_string(),
            );
        }
    }

    Ok(())
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

/// List all indexed projects with status snapshots.
#[tauri::command]
pub async fn codebase_list_projects_v2(
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<Vec<IndexedProjectStatusEntry>>, String> {
    let mgr_lock = standalone_state.index_manager.read().await;
    let mgr = match &*mgr_lock {
        Some(mgr) => mgr,
        None => return Ok(CommandResponse::ok(vec![])),
    };

    let projects = match mgr.index_store().list_indexed_projects() {
        Ok(projects) => projects,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to list indexed projects: {}",
                e
            )))
        }
    };

    let mut entries = Vec::with_capacity(projects.len());
    for project in projects {
        let status = mgr.get_status(&project.project_path).await;
        entries.push(IndexedProjectStatusEntry {
            project_path: project.project_path,
            file_count: project.file_count,
            last_indexed_at: project.last_indexed_at,
            status,
        });
    }

    Ok(CommandResponse::ok(entries))
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
    let mgr_lock = standalone_state.index_manager.read().await;
    let mgr = match &*mgr_lock {
        Some(mgr) => mgr,
        None => return Ok(CommandResponse::err("IndexManager not initialized")),
    };

    let project_path = request.project_path.clone();
    let search_service = CodebaseSearchService::new(
        mgr.index_store_arc(),
        mgr.get_embedding_manager(&project_path).await,
        mgr.get_hnsw_index(&project_path).await,
    );
    match search_service.search(request).await {
        Ok(response) => Ok(CommandResponse::ok(response)),
        Err(error) => Ok(CommandResponse::err(error)),
    }
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
    session_id: Option<String>,
    workflow_state: State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<CodebaseContextAppendResult>, String> {
    if items.is_empty() {
        return Ok(CommandResponse::err(
            "context_validation_failed: items cannot be empty",
        ));
    }
    if let Err(err) = validate_context_items(&items) {
        return Ok(CommandResponse::err(err));
    }

    let target_mode_parsed = match parse_workflow_mode(&target_mode) {
        Ok(mode) => mode,
        Err(err) => return Ok(CommandResponse::err(err)),
    };

    let resolved_session_id = match session_id.as_deref().map(str::trim) {
        Some(existing) if !existing.is_empty() => existing.to_string(),
        _ => match workflow_state
            .open_session(
                Some(target_mode_parsed),
                Some(HandoffContextBundle::default()),
            )
            .await
        {
            Ok(session) => session.session_id,
            Err(e) => {
                return Ok(CommandResponse::err(format!(
                    "session_not_found: unable to create workflow session: {}",
                    e
                )));
            }
        },
    };

    let context_ref_ids: Vec<String> = items
        .iter()
        .map(|item| {
            format!(
                "ctx:{}:{}:{}:{}",
                uuid::Uuid::new_v4(),
                item.project_path,
                item.file_path,
                item.symbol_name.clone().unwrap_or_default()
            )
        })
        .collect();

    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "source".to_string(),
        serde_json::Value::String("codebase".to_string()),
    );
    metadata.insert(
        "target_mode".to_string(),
        serde_json::Value::String(target_mode.clone()),
    );
    metadata.insert(
        "items".to_string(),
        serde_json::to_value(&items).unwrap_or(serde_json::Value::Array(vec![])),
    );
    metadata.insert(
        "context_ref_ids".to_string(),
        serde_json::to_value(&context_ref_ids).unwrap_or(serde_json::Value::Array(vec![])),
    );

    let artifact_refs: Vec<String> = items
        .iter()
        .map(|item| {
            format!(
                "{}:{}{}",
                item.project_path,
                item.file_path,
                item.symbol_name
                    .as_ref()
                    .map(|symbol| format!("#{}", symbol))
                    .unwrap_or_default()
            )
        })
        .collect();

    let mut context_sources = vec!["codebase".to_string()];
    for project in items.iter().map(|item| item.project_path.clone()) {
        let source = format!("codebase:{}", project);
        if !context_sources.contains(&source) {
            context_sources.push(source);
        }
    }

    let handoff = HandoffContextBundle {
        conversation_context: Vec::new(),
        artifact_refs,
        context_sources,
        metadata,
    };

    if let Err(e) = workflow_state
        .append_context_items(&resolved_session_id, target_mode_parsed, handoff)
        .await
    {
        let msg = if e.contains("not found") {
            format!("session_not_found: {}", e)
        } else {
            e
        };
        return Ok(CommandResponse::err(msg));
    }

    // Compatibility event (deprecated). Actual integration is now session-backed.
    let payload = CodebaseContextAddedEvent {
        target_mode,
        items: items.clone(),
    };
    let _ = app_handle.emit("codebase-context-added", &payload);

    Ok(CommandResponse::ok(CodebaseContextAppendResult {
        appended_count: items.len(),
        context_ref_ids,
        session_id: resolved_session_id,
        target_mode: payload.target_mode,
    }))
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
    fn validate_context_items_rejects_mixed_project_paths() {
        let items = vec![
            ContextItem {
                r#type: "search_result".to_string(),
                project_path: "/a".to_string(),
                file_path: "src/a.rs".to_string(),
                symbol_name: None,
                snippet: None,
                line_start: Some(1),
                line_end: Some(2),
                score: None,
                metadata: None,
            },
            ContextItem {
                r#type: "search_result".to_string(),
                project_path: "/b".to_string(),
                file_path: "src/b.rs".to_string(),
                symbol_name: None,
                snippet: None,
                line_start: Some(1),
                line_end: Some(2),
                score: None,
                metadata: None,
            },
        ];

        let err = validate_context_items(&items).expect_err("mixed projects must fail");
        assert!(err.contains("mixed project_path values"));
    }

    #[test]
    fn validate_context_items_rejects_invalid_line_ranges() {
        let items = vec![ContextItem {
            r#type: "search_result".to_string(),
            project_path: "/a".to_string(),
            file_path: "src/a.rs".to_string(),
            symbol_name: None,
            snippet: None,
            line_start: Some(10),
            line_end: Some(3),
            score: None,
            metadata: None,
        }];

        let err = validate_context_items(&items).expect_err("invalid line range must fail");
        assert!(err.contains("line_end must be >= line_start"));
    }
}
