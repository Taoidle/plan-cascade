//! Memory Commands
//!
//! Tauri commands for managing project memories (cross-session persistent knowledge).

use serde::Serialize;
use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::memory::extraction::run_session_extraction;
use crate::services::memory::maintenance::MemoryMaintenance;
use crate::services::memory::query_policy_v2::{memory_query_tuning_v2, MemoryQueryPresetV2};
use crate::services::memory::query_v2::{
    list_memory_entries_v2 as list_memory_entries_unified_v2,
    list_pending_memory_candidates_v2 as list_pending_memory_candidates_unified_v2,
    memory_stats_v2 as memory_stats_unified_v2,
    query_memory_entries_v2 as query_memory_entries_unified_v2,
    review_memory_candidates_v2 as review_memory_candidates_unified_v2, MemoryReviewCandidateV2,
    MemoryReviewDecisionV2, MemoryReviewSummaryV2, MemoryScopeV2, MemoryStatusV2,
    UnifiedMemoryQueryRequestV2, UnifiedMemoryQueryResultV2,
};
use crate::services::memory::retrieval::{MemorySearchIntent, MemorySearchResultV2};
use crate::services::memory::store::{
    build_session_project_path, MemoryCategory, MemoryEntry, MemorySearchResult, MemoryStats,
    MemoryUpdate, NewMemoryEntry, UpsertResult, GLOBAL_PROJECT_PATH,
};
use crate::state::AppState;

const EXTRACTION_SESSION_MARKER_PREFIX: &str = "memory_extracted_session:";
const DEFAULT_SESSION_SCOPE_TTL_DAYS: i64 = 14;

fn env_flag_enabled(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn resolve_memory_project_path(
    project_path: &str,
    scope: Option<&str>,
    session_id: Option<&str>,
) -> Result<String, String> {
    let resolved_scope = scope.unwrap_or("project").trim().to_ascii_lowercase();

    match resolved_scope.as_str() {
        "project" => {
            let trimmed = project_path.trim();
            if trimmed.is_empty() {
                Err("project_path is required for project scope".to_string())
            } else {
                Ok(trimmed.to_string())
            }
        }
        "global" => Ok(GLOBAL_PROJECT_PATH.to_string()),
        "session" => {
            let sid = session_id.unwrap_or("").trim();
            if sid.is_empty() {
                return Err("session_id is required for session scope".to_string());
            }
            build_session_project_path(sid)
                .ok_or_else(|| format!("Invalid session_id for memory scope: {}", sid))
        }
        other => Err(format!("Invalid memory scope: {}", other)),
    }
}

fn parse_memory_intent(intent: Option<&str>) -> MemorySearchIntent {
    match intent
        .unwrap_or("default")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "bugfix" => MemorySearchIntent::Bugfix,
        "refactor" => MemorySearchIntent::Refactor,
        "qa" => MemorySearchIntent::Qa,
        "docs" => MemorySearchIntent::Docs,
        _ => MemorySearchIntent::Default,
    }
}

fn parse_memory_scopes_v2(scopes: Option<&[String]>) -> Vec<MemoryScopeV2> {
    scopes
        .unwrap_or(&[])
        .iter()
        .filter_map(|scope| MemoryScopeV2::from_str(scope))
        .collect()
}

fn parse_memory_statuses_v2(statuses: Option<&[String]>) -> Vec<MemoryStatusV2> {
    statuses
        .unwrap_or(&[])
        .iter()
        .filter_map(|status| MemoryStatusV2::from_str(status))
        .collect()
}

fn parse_memory_review_decision_v2(value: &str) -> Result<MemoryReviewDecisionV2, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "approve" | "approved" | "active" => Ok(MemoryReviewDecisionV2::Approve),
        "reject" | "rejected" => Ok(MemoryReviewDecisionV2::Reject),
        "archive" | "archived" => Ok(MemoryReviewDecisionV2::Archive),
        _ => Err(format!("Invalid review decision: {}", value)),
    }
}

/// Search project memories by semantic similarity and keyword match
#[tauri::command]
pub async fn search_project_memories(
    project_path: String,
    query: String,
    categories: Option<Vec<String>>,
    top_k: Option<usize>,
    scope: Option<String>,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<MemorySearchResult>>, String> {
    eprintln!(
        "[memory] Deprecated command 'search_project_memories' called; routing to unified query_v2"
    );

    let effective_project_path =
        match resolve_memory_project_path(&project_path, scope.as_deref(), session_id.as_deref()) {
            Ok(path) => path,
            Err(e) => return Ok(CommandResponse::err(e)),
        };

    let parsed_categories = categories.as_ref().map(|cats| {
        cats.iter()
            .filter_map(|c| MemoryCategory::from_str(c).ok())
            .collect::<Vec<_>>()
    });

    let parsed_scope = match scope.as_deref() {
        Some(value) => match MemoryScopeV2::from_str(value) {
            Some(scope) => vec![scope],
            None => {
                return Ok(CommandResponse::err(format!(
                    "Invalid memory scope: {}",
                    value
                )))
            }
        },
        None => vec![],
    };
    if matches!(parsed_scope.first(), Some(MemoryScopeV2::Session))
        && session_id.as_deref().unwrap_or("").trim().is_empty()
    {
        return Ok(CommandResponse::err(
            "session_id is required for session scope".to_string(),
        ));
    }

    let tuning = memory_query_tuning_v2(MemoryQueryPresetV2::CommandSearch);
    let request = UnifiedMemoryQueryRequestV2 {
        project_path: effective_project_path,
        query,
        scopes: parsed_scope,
        categories: parsed_categories.unwrap_or_default(),
        include_ids: vec![],
        exclude_ids: vec![],
        session_id,
        top_k_total: top_k.unwrap_or(tuning.top_k_total),
        min_importance: tuning.min_importance,
        per_scope_budget: tuning.per_scope_budget,
        intent: MemorySearchIntent::Default,
        enable_semantic: true,
        enable_lexical: true,
        statuses: vec![MemoryStatusV2::Active],
    };

    let store = match state.get_memory_store_arc().await {
        Ok(store) => store,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match query_memory_entries_unified_v2(store.as_ref(), &request).await {
        Ok(results) => Ok(CommandResponse::ok(
            results
                .results
                .into_iter()
                .map(|row| MemorySearchResult {
                    entry: row.entry,
                    relevance_score: row.relevance_score,
                })
                .collect(),
        )),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Search project memories with explainable scoring and intent-aware weighting.
#[tauri::command]
pub async fn search_project_memories_v2(
    project_path: String,
    query: String,
    categories: Option<Vec<String>>,
    top_k: Option<usize>,
    scope: Option<String>,
    session_id: Option<String>,
    intent: Option<String>,
    enable_semantic: Option<bool>,
    enable_lexical: Option<bool>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<MemorySearchResultV2>>, String> {
    let trimmed_project_path = project_path.trim().to_string();
    if trimmed_project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required".to_string()));
    }
    let parsed_scope = match scope.as_deref() {
        Some(value) => match MemoryScopeV2::from_str(value) {
            Some(scope) => vec![scope],
            None => {
                return Ok(CommandResponse::err(format!(
                    "Invalid memory scope: {}",
                    value
                )))
            }
        },
        None => vec![],
    };
    if matches!(parsed_scope.first(), Some(MemoryScopeV2::Session))
        && session_id.as_deref().unwrap_or("").trim().is_empty()
    {
        return Ok(CommandResponse::err(
            "session_id is required for session scope".to_string(),
        ));
    }

    let parsed_categories: Vec<MemoryCategory> = categories
        .as_ref()
        .map(|cats| {
            cats.iter()
                .filter_map(|c| MemoryCategory::from_str(c).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let tuning = memory_query_tuning_v2(MemoryQueryPresetV2::CommandSearch);
    let request = UnifiedMemoryQueryRequestV2 {
        project_path: trimmed_project_path,
        query,
        scopes: parsed_scope,
        categories: parsed_categories,
        include_ids: vec![],
        exclude_ids: vec![],
        session_id,
        top_k_total: top_k.unwrap_or(tuning.top_k_total),
        min_importance: tuning.min_importance,
        per_scope_budget: tuning.per_scope_budget,
        intent: parse_memory_intent(intent.as_deref()),
        enable_semantic: enable_semantic.unwrap_or(true),
        enable_lexical: enable_lexical.unwrap_or(true),
        statuses: vec![MemoryStatusV2::Active],
    };

    let store = match state.get_memory_store_arc().await {
        Ok(store) => store,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match query_memory_entries_unified_v2(store.as_ref(), &request).await {
        Ok(results) => Ok(CommandResponse::ok(results.results)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List project memories with optional category filter and pagination
#[tauri::command]
pub async fn list_project_memories(
    project_path: String,
    category: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    scope: Option<String>,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<MemoryEntry>>, String> {
    let effective_project_path =
        match resolve_memory_project_path(&project_path, scope.as_deref(), session_id.as_deref()) {
            Ok(path) => path,
            Err(e) => return Ok(CommandResponse::err(e)),
        };

    let parsed_category = category
        .as_ref()
        .and_then(|c| MemoryCategory::from_str(c).ok());

    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(50);

    match state
        .with_memory_store(|store| {
            store.list_memories(&effective_project_path, parsed_category, offset, limit)
        })
        .await
    {
        Ok(memories) => Ok(CommandResponse::ok(memories)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Add a new project memory
#[tauri::command]
pub async fn add_project_memory(
    project_path: String,
    category: String,
    content: String,
    keywords: Vec<String>,
    importance: Option<f32>,
    scope: Option<String>,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryEntry>, String> {
    let effective_project_path =
        match resolve_memory_project_path(&project_path, scope.as_deref(), session_id.as_deref()) {
            Ok(path) => path,
            Err(e) => return Ok(CommandResponse::err(e)),
        };

    let parsed_category = match MemoryCategory::from_str(&category) {
        Ok(c) => c,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let entry = NewMemoryEntry {
        project_path: effective_project_path,
        category: parsed_category,
        content,
        keywords,
        importance: importance.unwrap_or(0.5),
        source_session_id: session_id,
        source_context: None,
    };

    match state
        .with_memory_store(|store| store.add_memory(entry))
        .await
    {
        Ok(memory) => Ok(CommandResponse::ok(memory)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update an existing project memory
#[tauri::command]
pub async fn update_project_memory(
    id: String,
    content: Option<String>,
    category: Option<String>,
    importance: Option<f32>,
    keywords: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryEntry>, String> {
    let parsed_category = category
        .as_ref()
        .map(|c| MemoryCategory::from_str(c))
        .transpose()
        .map_err(|e| e.to_string());

    let parsed_category = match parsed_category {
        Ok(c) => c,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let updates = MemoryUpdate {
        content,
        category: parsed_category,
        importance,
        keywords,
    };

    match state
        .with_memory_store(|store| store.update_memory(&id, updates))
        .await
    {
        Ok(memory) => Ok(CommandResponse::ok(memory)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete a specific project memory
#[tauri::command]
pub async fn delete_project_memory(
    id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String> {
    match state
        .with_memory_store(|store| store.delete_memory(&id))
        .await
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Clear all memories for a project
#[tauri::command]
pub async fn clear_project_memories(
    project_path: String,
    scope: Option<String>,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<usize>, String> {
    let effective_project_path =
        match resolve_memory_project_path(&project_path, scope.as_deref(), session_id.as_deref()) {
            Ok(path) => path,
            Err(e) => return Ok(CommandResponse::err(e)),
        };

    match state
        .with_memory_store(|store| store.clear_project_memories(&effective_project_path))
        .await
    {
        Ok(count) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Clear all memories for a session scope.
#[tauri::command]
pub async fn clear_session_memories(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<usize>, String> {
    match state
        .with_memory_store(|store| store.clear_session_memories(&session_id))
        .await
    {
        Ok(count) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Cleanup expired session-scope memories based on TTL days (default: 14).
#[tauri::command]
pub async fn cleanup_expired_session_memories_v2(
    ttl_days: Option<i64>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<usize>, String> {
    let effective_ttl = ttl_days.unwrap_or(DEFAULT_SESSION_SCOPE_TTL_DAYS).max(1);
    match state
        .with_memory_store(|store| store.cleanup_expired_session_memories(effective_ttl))
        .await
    {
        Ok(count) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get memory statistics for a project
#[tauri::command]
pub async fn get_memory_stats(
    project_path: String,
    scope: Option<String>,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryStats>, String> {
    let trimmed_project_path = project_path.trim().to_string();
    if trimmed_project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required".to_string()));
    }
    let scopes = match scope.as_deref() {
        Some(value) => match MemoryScopeV2::from_str(value) {
            Some(scope) => vec![scope],
            None => {
                return Ok(CommandResponse::err(format!(
                    "Invalid memory scope: {}",
                    value
                )))
            }
        },
        None => vec![],
    };
    if matches!(scopes.first(), Some(MemoryScopeV2::Session))
        && session_id.as_deref().unwrap_or("").trim().is_empty()
    {
        return Ok(CommandResponse::err(
            "session_id is required for session scope".to_string(),
        ));
    }
    let tuning = memory_query_tuning_v2(MemoryQueryPresetV2::CommandStats);
    let request = UnifiedMemoryQueryRequestV2 {
        project_path: trimmed_project_path,
        query: String::new(),
        scopes,
        categories: vec![],
        include_ids: vec![],
        exclude_ids: vec![],
        session_id,
        top_k_total: tuning.top_k_total,
        min_importance: tuning.min_importance,
        per_scope_budget: tuning.per_scope_budget,
        intent: MemorySearchIntent::Default,
        enable_semantic: false,
        enable_lexical: false,
        statuses: vec![MemoryStatusV2::Active],
    };
    match state
        .with_memory_store(|store| memory_stats_unified_v2(store, &request))
        .await
    {
        Ok(stats) => Ok(CommandResponse::ok(stats)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Unified V2 memory query across project/global/session scopes.
#[tauri::command]
pub async fn query_memory_entries_v2(
    project_path: String,
    query: String,
    categories: Option<Vec<String>>,
    scopes: Option<Vec<String>>,
    include_ids: Option<Vec<String>>,
    exclude_ids: Option<Vec<String>>,
    statuses: Option<Vec<String>>,
    session_id: Option<String>,
    top_k_total: Option<usize>,
    min_importance: Option<f32>,
    per_scope_budget: Option<usize>,
    intent: Option<String>,
    enable_semantic: Option<bool>,
    enable_lexical: Option<bool>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<UnifiedMemoryQueryResultV2>, String> {
    let trimmed_project_path = project_path.trim().to_string();
    if trimmed_project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required".to_string()));
    }
    let parsed_categories: Vec<MemoryCategory> = categories
        .as_ref()
        .map(|cats| {
            cats.iter()
                .filter_map(|cat| MemoryCategory::from_str(cat).ok())
                .collect()
        })
        .unwrap_or_default();
    let tuning = memory_query_tuning_v2(MemoryQueryPresetV2::CommandQuery);
    let request = UnifiedMemoryQueryRequestV2 {
        project_path: trimmed_project_path,
        query,
        scopes: parse_memory_scopes_v2(scopes.as_deref()),
        categories: parsed_categories,
        include_ids: include_ids.unwrap_or_default(),
        exclude_ids: exclude_ids.unwrap_or_default(),
        session_id,
        top_k_total: top_k_total.unwrap_or(tuning.top_k_total),
        min_importance: min_importance.unwrap_or(tuning.min_importance),
        per_scope_budget: per_scope_budget.unwrap_or(tuning.per_scope_budget),
        intent: parse_memory_intent(intent.as_deref()),
        enable_semantic: enable_semantic.unwrap_or(true),
        enable_lexical: enable_lexical.unwrap_or(true),
        statuses: parse_memory_statuses_v2(statuses.as_deref()),
    };
    let store = match state.get_memory_store_arc().await {
        Ok(store) => store,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    match query_memory_entries_unified_v2(store.as_ref(), &request).await {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Unified V2 memory listing across scopes.
#[tauri::command]
pub async fn list_memory_entries_v2(
    project_path: String,
    categories: Option<Vec<String>>,
    scopes: Option<Vec<String>>,
    statuses: Option<Vec<String>>,
    session_id: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<MemoryEntry>>, String> {
    let trimmed_project_path = project_path.trim().to_string();
    if trimmed_project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required".to_string()));
    }
    let tuning = memory_query_tuning_v2(MemoryQueryPresetV2::CommandList);
    let parsed_categories: Vec<MemoryCategory> = categories
        .as_ref()
        .map(|cats| {
            cats.iter()
                .filter_map(|cat| MemoryCategory::from_str(cat).ok())
                .collect()
        })
        .unwrap_or_default();
    let page_offset = offset.unwrap_or(0);
    let page_limit = limit.unwrap_or(50).max(1);
    let request = UnifiedMemoryQueryRequestV2 {
        project_path: trimmed_project_path,
        query: String::new(),
        scopes: parse_memory_scopes_v2(scopes.as_deref()),
        categories: parsed_categories,
        include_ids: vec![],
        exclude_ids: vec![],
        session_id,
        top_k_total: (page_offset + page_limit).max(tuning.top_k_total),
        min_importance: tuning.min_importance,
        per_scope_budget: tuning.per_scope_budget,
        intent: MemorySearchIntent::Default,
        enable_semantic: false,
        enable_lexical: false,
        statuses: parse_memory_statuses_v2(statuses.as_deref()),
    };
    let store = match state.get_memory_store_arc().await {
        Ok(store) => store,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };
    match list_memory_entries_unified_v2(store.as_ref(), request).await {
        Ok(mut rows) => {
            if page_offset >= rows.len() {
                return Ok(CommandResponse::ok(vec![]));
            }
            let end = (page_offset + page_limit).min(rows.len());
            rows = rows[page_offset..end].to_vec();
            Ok(CommandResponse::ok(rows))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Unified V2 memory stats across scopes.
#[tauri::command]
pub async fn memory_stats_v2(
    project_path: String,
    categories: Option<Vec<String>>,
    scopes: Option<Vec<String>>,
    statuses: Option<Vec<String>>,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryStats>, String> {
    let trimmed_project_path = project_path.trim().to_string();
    if trimmed_project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required".to_string()));
    }
    let parsed_categories: Vec<MemoryCategory> = categories
        .as_ref()
        .map(|cats| {
            cats.iter()
                .filter_map(|cat| MemoryCategory::from_str(cat).ok())
                .collect()
        })
        .unwrap_or_default();
    let tuning = memory_query_tuning_v2(MemoryQueryPresetV2::CommandStats);
    let request = UnifiedMemoryQueryRequestV2 {
        project_path: trimmed_project_path,
        query: String::new(),
        scopes: parse_memory_scopes_v2(scopes.as_deref()),
        categories: parsed_categories,
        include_ids: vec![],
        exclude_ids: vec![],
        session_id,
        top_k_total: tuning.top_k_total,
        min_importance: tuning.min_importance,
        per_scope_budget: tuning.per_scope_budget,
        intent: MemorySearchIntent::Default,
        enable_semantic: false,
        enable_lexical: false,
        statuses: parse_memory_statuses_v2(statuses.as_deref()),
    };
    match state
        .with_memory_store(|store| memory_stats_unified_v2(store, &request))
        .await
    {
        Ok(stats) => Ok(CommandResponse::ok(stats)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List pending-review memories in V2 governance flow.
#[tauri::command]
pub async fn list_pending_memory_candidates_v2(
    project_path: String,
    scopes: Option<Vec<String>>,
    session_id: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<MemoryReviewCandidateV2>>, String> {
    let trimmed_project_path = project_path.trim().to_string();
    if trimmed_project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required".to_string()));
    }
    let normalized_scopes = parse_memory_scopes_v2(scopes.as_deref());
    match state
        .with_memory_store(|store| {
            list_pending_memory_candidates_unified_v2(
                store,
                &trimmed_project_path,
                session_id.as_deref(),
                &normalized_scopes,
                limit.unwrap_or(200),
            )
        })
        .await
    {
        Ok(rows) => Ok(CommandResponse::ok(rows)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Review pending memory candidates in V2 governance flow.
#[tauri::command]
pub async fn review_memory_candidates_v2(
    memory_ids: Vec<String>,
    decision: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryReviewSummaryV2>, String> {
    let parsed_decision = match parse_memory_review_decision_v2(&decision) {
        Ok(value) => value,
        Err(e) => return Ok(CommandResponse::err(e)),
    };
    match state
        .with_memory_store(|store| {
            review_memory_candidates_unified_v2(store, &memory_ids, parsed_decision)
        })
        .await
    {
        Ok(summary) => Ok(CommandResponse::ok(summary)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Result of running maintenance operations
#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceResult {
    pub decayed_count: usize,
    pub pruned_count: usize,
    pub compacted_count: usize,
}

/// Run memory maintenance: decay, prune, and compact.
///
/// Called fire-and-forget when the memory dialog opens. Gracefully
/// handles failures in each step (returns 0 for that step).
#[tauri::command]
pub async fn run_memory_maintenance(
    project_path: String,
    scope: Option<String>,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MaintenanceResult>, String> {
    let effective_project_path =
        match resolve_memory_project_path(&project_path, scope.as_deref(), session_id.as_deref()) {
            Ok(path) => path,
            Err(e) => return Ok(CommandResponse::err(e)),
        };

    let decayed_count = state
        .with_memory_store(|store| {
            MemoryMaintenance::decay_memories(store, &effective_project_path)
        })
        .await
        .unwrap_or(0);

    let pruned_count = state
        .with_memory_store(|store| {
            MemoryMaintenance::prune_memories(store, &effective_project_path, 0.05)
        })
        .await
        .unwrap_or(0);

    let compacted_count = state
        .with_memory_store(|store| {
            MemoryMaintenance::compact_memories(store, &effective_project_path)
        })
        .await
        .unwrap_or(0);

    Ok(CommandResponse::ok(MaintenanceResult {
        decayed_count,
        pruned_count,
        compacted_count,
    }))
}

/// Result of automatic memory extraction from a session
#[derive(Debug, Clone, Serialize)]
pub struct MemoryExtractionResult {
    pub extracted_count: usize,
    pub inserted_count: usize,
    pub merged_count: usize,
    pub skipped_count: usize,
}

/// Extract memories from a completed session using LLM analysis.
///
/// Called by the frontend after a session completes. Uses the configured
/// LLM provider to analyze the conversation and extract persistent memories.
/// Silently returns zero results if no provider is configured or on any error.
#[tauri::command]
pub async fn extract_session_memories(
    project_path: String,
    task_description: String,
    conversation_summary: String,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryExtractionResult>, String> {
    let zero_result = MemoryExtractionResult {
        extracted_count: 0,
        inserted_count: 0,
        merged_count: 0,
        skipped_count: 0,
    };

    if !env_flag_enabled("UNIFIED_SESSION_EXTRACTION", true) {
        eprintln!(
            "[memory-extraction] Skipped: UNIFIED_SESSION_EXTRACTION is disabled (source=chat_frontend)"
        );
        return Ok(CommandResponse::ok(zero_result));
    }

    if let Some(ref sid) = session_id {
        let marker_key = extraction_marker_key(sid);
        let already_extracted = state
            .with_database(|db| Ok(db.get_setting(&marker_key)?.is_some()))
            .await
            .unwrap_or(false);
        if already_extracted {
            eprintln!(
                "[memory-extraction] Skipped: session already extracted (session_id={})",
                sid
            );
            return Ok(CommandResponse::ok(zero_result));
        }
    }

    // Skip short conversations
    if conversation_summary.len() < 50 {
        eprintln!(
            "[memory-extraction] Skipped: conversation too short ({} chars < 50)",
            conversation_summary.len()
        );
        return Ok(CommandResponse::ok(zero_result));
    }

    // Resolve LLM provider from app config + keyring
    let provider = match resolve_extraction_provider(&state).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "[memory-extraction] Provider resolution failed (source=chat_frontend): {}",
                e
            );
            return Ok(CommandResponse::ok(zero_result));
        }
    };

    // Load existing memories to avoid duplicates
    let existing_memories = state
        .with_memory_store(|store| store.list_memories(&project_path, None, 0, 200))
        .await
        .unwrap_or_default();

    let entries = match run_session_extraction(
        provider.as_ref(),
        &project_path,
        &task_description,
        &[], // files_read not available from frontend trigger
        &[], // key_findings not available from frontend trigger
        &conversation_summary,
        session_id.as_deref(),
        &existing_memories,
    )
    .await
    {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!(
                "[memory-extraction] Session extraction failed (source=chat_frontend): {}",
                e
            );
            return Ok(CommandResponse::ok(zero_result));
        }
    };

    let extracted_count = entries.len();
    if extracted_count == 0 {
        mark_session_extraction_done(&state, session_id.as_deref()).await;
        eprintln!(
            "[memory-extraction] LLM response parsed but yielded 0 entries (source=chat_frontend)"
        );
        return Ok(CommandResponse::ok(zero_result));
    }

    // Upsert each memory entry
    let mut inserted_count = 0usize;
    let mut merged_count = 0usize;
    let mut skipped_count = 0usize;

    for entry in entries {
        match state
            .with_memory_store(|store| store.upsert_memory(entry.clone()))
            .await
        {
            Ok(UpsertResult::Inserted(_)) => inserted_count += 1,
            Ok(UpsertResult::Merged { .. }) => merged_count += 1,
            Ok(UpsertResult::Skipped { .. }) => skipped_count += 1,
            Err(e) => {
                eprintln!(
                    "[memory-extraction] Upsert failed (source=chat_frontend): {}",
                    e
                );
                skipped_count += 1;
            }
        }
    }

    eprintln!(
        "[memory-extraction] Done (source=chat_frontend): extracted={}, inserted={}, merged={}, skipped={}",
        extracted_count,
        inserted_count,
        merged_count,
        skipped_count
    );

    mark_session_extraction_done(&state, session_id.as_deref()).await;

    Ok(CommandResponse::ok(MemoryExtractionResult {
        extracted_count,
        inserted_count,
        merged_count,
        skipped_count,
    }))
}

/// Resolve the LLM provider for memory extraction from app settings.
///
/// Uses the default_provider/default_model from AppConfig and retrieves
/// the API key from the OS keyring. Returns an error if no provider is
/// configured or no API key is found (except for Ollama).
async fn resolve_extraction_provider(
    state: &AppState,
) -> Result<Box<dyn crate::services::llm::provider::LlmProvider>, String> {
    use crate::commands::standalone::{get_api_key_with_aliases, normalize_provider_name};
    use crate::services::llm::types::{ProviderConfig, ProviderType};
    use crate::storage::KeyringService;

    let app_config = state
        .get_config()
        .await
        .map_err(|e| format!("Config not initialized: {}", e))?;

    let canonical = normalize_provider_name(&app_config.default_provider)
        .ok_or_else(|| format!("Unknown provider: {}", app_config.default_provider))?;

    let provider_type = match canonical {
        "anthropic" => ProviderType::Anthropic,
        "openai" => ProviderType::OpenAI,
        "deepseek" => ProviderType::DeepSeek,
        "glm" => ProviderType::Glm,
        "qwen" => ProviderType::Qwen,
        "minimax" => ProviderType::Minimax,
        "ollama" => ProviderType::Ollama,
        _ => return Err(format!("Unsupported provider: {}", canonical)),
    };

    let keyring = KeyringService::new();
    let api_key = get_api_key_with_aliases(&keyring, canonical)
        .map_err(|e| format!("Keyring error: {}", e))?;

    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return Err("No API key configured".into());
    }

    // Resolve base_url from database settings
    let resolved_base_url = {
        let key = format!("provider_{}_base_url", canonical);
        state
            .with_database(|db| db.get_setting(&key))
            .await
            .ok()
            .flatten()
            .filter(|u| !u.is_empty())
    };

    // Resolve proxy settings
    let proxy = state
        .with_database(|db| {
            Ok(crate::commands::proxy::resolve_provider_proxy(
                &keyring, db, canonical,
            ))
        })
        .await
        .ok()
        .flatten();

    let resolved_model = {
        let model = app_config.model_for_provider(canonical);
        if model.is_empty() {
            app_config.default_model.clone()
        } else {
            model
        }
    };

    let config = ProviderConfig {
        provider: provider_type.clone(),
        api_key,
        base_url: resolved_base_url,
        model: resolved_model,
        max_tokens: 2048,
        temperature: 0.3,
        proxy,
        ..Default::default()
    };

    Ok(create_extraction_provider(config))
}

/// Create an LLM provider instance from a ProviderConfig.
fn create_extraction_provider(
    config: crate::services::llm::types::ProviderConfig,
) -> Box<dyn crate::services::llm::provider::LlmProvider> {
    use crate::services::llm::types::ProviderType;
    use crate::services::llm::*;

    match config.provider {
        ProviderType::Anthropic => Box::new(AnthropicProvider::new(config)),
        ProviderType::OpenAI => Box::new(OpenAIProvider::new(config)),
        ProviderType::DeepSeek => Box::new(DeepSeekProvider::new(config)),
        ProviderType::Glm => Box::new(GlmProvider::new(config)),
        ProviderType::Qwen => Box::new(QwenProvider::new(config)),
        ProviderType::Minimax => Box::new(MinimaxProvider::new(config)),
        ProviderType::Ollama => Box::new(OllamaProvider::new(config)),
    }
}

fn extraction_marker_key(session_id: &str) -> String {
    format!("{}{}", EXTRACTION_SESSION_MARKER_PREFIX, session_id)
}

async fn mark_session_extraction_done(state: &AppState, session_id: Option<&str>) {
    let Some(session_id) = session_id else {
        return;
    };
    let marker_key = extraction_marker_key(session_id);
    if let Err(e) = state
        .with_database(|db| db.set_setting(&marker_key, "1"))
        .await
    {
        eprintln!(
            "[memory-extraction] Failed to persist extraction marker for session {}: {}",
            session_id, e
        );
    }
}
