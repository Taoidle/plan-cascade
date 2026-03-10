//! Task Mode Domain Context Provider
//!
//! Aggregates domain knowledge from Knowledge Base (RAG), Project Memory,
//! and Skills into formatted context blocks for injection into Task Mode
//! workflow stages (requirement analysis, architecture review, PRD generation,
//! story execution).
//!
//! All query functions follow graceful degradation: on any error, they log a
//! warning and return empty strings so that the LLM never sees error messages —
//! it simply receives less context, preserving the original behavior.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::services::llm::types::ProviderConfig;
use crate::commands::knowledge::KnowledgeState;
use crate::services::knowledge::context_provider::{
    KnowledgeContextConfig, KnowledgeContextProvider,
};
use crate::services::knowledge::pipeline::ScopedDocumentRef;
use crate::services::memory::query_policy_v2::tuning_for_task_context_v2;
use crate::services::memory::query_v2::{
    query_memory_entries_v2 as query_memory_entries_unified_v2, MemoryScopeV2, MemoryStatusV2,
    UnifiedMemoryQueryRequestV2,
};
use crate::services::memory::store::MemoryCategory;
use crate::services::skills::config::load_skills_config;
use crate::services::skills::discovery::discover_all_skills;
use crate::services::skills::generator::SkillGeneratorStore;
use crate::services::skills::index::build_index;
use crate::services::skills::injector::{inject_skill_summaries, inject_skills};
use crate::services::skills::model::{
    GeneratedSkillRecord, InjectionPhase, SelectionPolicy, SkillDocument, SkillIndex, SkillMatch,
    SkillReviewStatus, SkillSource,
};
use crate::services::skills::router::SkillRerankDiagnostics;
use crate::services::skills::router::rerank_skill_matches;
use crate::services::skills::select::{
    select_skill_candidates_for_session, select_skills_for_session,
};
use crate::services::tools::definitions::get_tool_definitions_from_registry;
use crate::services::tools::system_prompt::build_memory_section;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Context Source Configuration (user-controlled)
// ---------------------------------------------------------------------------

/// User-controlled configuration for which context sources to query.
/// Passed from the frontend to conditionally inject domain knowledge.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextSourceConfig {
    /// The project ID used for knowledge base queries (e.g. "default" or a UUID).
    /// Falls back to `"default"` when not provided (backward-compatible).
    #[serde(default = "default_project_id")]
    pub project_id: String,
    pub knowledge: Option<KnowledgeSourceConfig>,
    pub memory: Option<MemorySourceConfig>,
    pub skills: Option<SkillsSourceConfig>,
}

fn default_project_id() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSourceConfig {
    pub enabled: bool,
    #[serde(default)]
    pub selected_collections: Vec<String>,
    #[serde(default)]
    pub selected_documents: Vec<ScopedDocumentRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySourceConfig {
    pub enabled: bool,
    /// Selected memory categories to filter by (e.g. "preference", "convention").
    /// Empty = search all categories (backward-compatible).
    #[serde(default)]
    pub selected_categories: Vec<String>,
    /// Specific memory entry IDs to include.
    /// Empty = no per-item filtering (backward-compatible).
    #[serde(default)]
    pub selected_memory_ids: Vec<String>,
    /// Specific memory entry IDs to exclude from injection.
    /// Empty = no exclusion filtering.
    #[serde(default)]
    pub excluded_memory_ids: Vec<String>,
    /// Enabled memory scopes: project/global/session.
    /// Empty = default to project + global (+ session when session_id is present).
    #[serde(default)]
    pub selected_scopes: Vec<String>,
    /// Session id used when querying session-scoped memories.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Allowed memory statuses for retrieval.
    /// Empty means default to `active`, unless review_mode opts into diagnostics.
    #[serde(default)]
    pub statuses: Vec<String>,
    /// Review retrieval mode:
    /// - `active_only` (default): inject only active memories
    /// - `include_pending_review`: include pending_review for diagnostics
    #[serde(default)]
    pub review_mode: Option<String>,
    /// Optional frontend-provided memory selection mode.
    /// - `auto_exclude`: auto retrieval + excluded ids
    /// - `only_selected`: exact include ids only
    #[serde(default)]
    pub selection_mode: Option<MemorySelectionMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySelectionMode {
    AutoExclude,
    OnlySelected,
}

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

/// Resolve effective memory statuses from explicit statuses + review mode.
///
/// Rules:
/// - Explicit `statuses` has highest priority.
/// - Empty `statuses` falls back to review mode:
///   - `include_pending_review` / `diagnostic` => active + pending_review
///   - otherwise => active only
/// - `MEMORY_REVIEW_MODE_RESOLVER=0` disables review-mode expansion and keeps
///   backward-compatible active-only default unless explicit statuses are given.
pub fn resolve_memory_statuses(
    selected_statuses: &[String],
    review_mode: Option<&str>,
) -> Vec<MemoryStatusV2> {
    let mut parsed_statuses: Vec<MemoryStatusV2> = selected_statuses
        .iter()
        .filter_map(|status| MemoryStatusV2::from_str(status))
        .collect();

    if !parsed_statuses.is_empty() {
        return parsed_statuses;
    }

    if env_flag_enabled("MEMORY_REVIEW_MODE_RESOLVER", true)
        && matches!(
            review_mode
                .unwrap_or("active_only")
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "include_pending_review" | "diagnostic"
        )
    {
        parsed_statuses.push(MemoryStatusV2::Active);
        parsed_statuses.push(MemoryStatusV2::PendingReview);
        return parsed_statuses;
    }

    vec![MemoryStatusV2::Active]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsSourceConfig {
    pub enabled: bool,
    /// Specific skill IDs the user selected.
    /// Empty = use automatic selection logic (backward-compatible).
    #[serde(default)]
    pub selected_skill_ids: Vec<String>,
    /// Session-pinned command-invoked skills.
    #[serde(default)]
    pub invoked_skill_ids: Vec<String>,
    /// Skill selection mode.
    /// `auto` = use lexical/detection policy.
    /// `explicit` = strictly respect `selected_skill_ids`.
    #[serde(default)]
    pub selection_mode: SkillSelectionMode,
    /// Review filter for generated skills.
    #[serde(default)]
    pub review_filter: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkillSelectionMode {
    #[default]
    Auto,
    Explicit,
}

#[derive(Debug, Clone, Default)]
pub struct EffectiveSkillPlan {
    pub matches: Vec<SkillMatch>,
    pub skills_block: String,
    pub skill_expertise: Vec<String>,
    pub blocked_tools: Vec<String>,
    pub selection_reason: String,
    pub selection_origin: String,
    pub hierarchy_matches: Vec<String>,
    pub why_not_selected: Vec<NonSelectedSkillDiagnostic>,
    pub router_diagnostics: SkillRerankDiagnostics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NonSelectedSkillDiagnostic {
    pub skill_id: String,
    pub skill_name: String,
    pub reason: String,
    pub source_type: String,
    pub path: String,
    pub review_status: Option<SkillReviewStatus>,
}

/// Aggregated domain context from all sources.
#[derive(Debug, Clone, Default)]
pub struct EnrichedContext {
    pub knowledge_block: String,
    pub memory_block: String,
    pub skills_block: String,
    pub skill_expertise: Vec<String>,
    pub selected_skills: Vec<SkillMatch>,
    pub blocked_tools: Vec<String>,
    pub skill_selection_reason: String,
}

// ---------------------------------------------------------------------------
// Knowledge pipeline initialization helper
// ---------------------------------------------------------------------------

/// Ensure the knowledge pipeline is initialized before querying.
///
/// This is needed because `execute_standalone` and task mode commands do not
/// call the `ensure_initialized` function from `commands/knowledge.rs` (which
/// is private to that module). Without this, `get_pipeline()` returns `Err`
/// and knowledge queries silently return empty strings.
async fn ensure_knowledge_initialized(knowledge_state: &KnowledgeState, app_state: &AppState) {
    if knowledge_state.is_initialized().await {
        return;
    }
    tracing::info!("[ContextSource] Knowledge pipeline not initialized, attempting init...");
    let db = match app_state
        .with_database(|db| Ok(std::sync::Arc::new(db.clone())))
        .await
    {
        Ok(db) => db,
        Err(e) => {
            tracing::warn!(
                "[ContextSource] Failed to access database for knowledge init: {}",
                e
            );
            return;
        }
    };

    let (emb_config, is_tfidf) = match app_state
        .with_keyring(|keyring| {
            let (config, _dim, is_tfidf) =
                crate::services::orchestrator::embedding_config_builder::build_embedding_config_from_settings(
                    &db, keyring,
                );
            Ok((config, is_tfidf))
        })
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("[ContextSource] Failed to build embedding config: {}", e);
            return;
        }
    };

    if let Err(e) = knowledge_state
        .initialize_with_config(db, emb_config, is_tfidf)
        .await
    {
        tracing::warn!(
            "[ContextSource] Knowledge pipeline initialization failed: {}",
            e
        );
    } else {
        tracing::info!("[ContextSource] Knowledge pipeline initialized successfully");
    }
}

// ---------------------------------------------------------------------------
// Conditional query entry point
// ---------------------------------------------------------------------------

/// Query domain context based on user-selected source configuration.
///
/// Only queries sources that the user has explicitly enabled.
/// Returns `EnrichedContext::default()` (all empty) when nothing is selected.
pub async fn query_selected_context(
    config: &ContextSourceConfig,
    knowledge_state: &KnowledgeState,
    app_state: &AppState,
    project_path: &str,
    query: &str,
    phase: InjectionPhase,
    provider_config: Option<&ProviderConfig>,
) -> EnrichedContext {
    tracing::info!(
        "[ContextSource] query_selected_context called — knowledge={:?}, memory={:?}, skills={:?}, project_id={}",
        config.knowledge.as_ref().map(|k| k.enabled),
        config.memory.as_ref().map(|m| m.enabled),
        config.skills.as_ref().map(|s| s.enabled),
        config.project_id,
    );
    let pid = if config.project_id.is_empty() {
        "default"
    } else {
        &config.project_id
    };
    let knowledge_block =
        if config.knowledge.as_ref().map_or(false, |k| k.enabled) {
            let k = config.knowledge.as_ref().unwrap();
            tracing::info!(
            "[ContextSource] Querying knowledge — project_id={}, collections={:?}, documents={:?}",
            pid, k.selected_collections, k.selected_documents,
        );
            // Ensure knowledge pipeline is initialized before querying
            ensure_knowledge_initialized(knowledge_state, app_state).await;
            let block = query_knowledge_for_task_filtered(
                knowledge_state,
                pid,
                query,
                &k.selected_collections,
                &k.selected_documents,
            )
            .await;
            tracing::info!(
                "[ContextSource] Knowledge query result: {} chars",
                block.len(),
            );
            block
        } else {
            String::new()
        };

    let memory_block = if config.memory.as_ref().map_or(false, |m| m.enabled) {
        let m = config.memory.as_ref().unwrap();
        query_memories_for_task_filtered(
            app_state,
            project_path,
            query,
            &m.selected_categories,
            &m.selected_memory_ids,
            &m.excluded_memory_ids,
            &m.selected_scopes,
            m.session_id.as_deref(),
            &m.statuses,
            m.review_mode.as_deref(),
        )
        .await
    } else {
        String::new()
    };

    let (skills_block, skill_expertise, selected_skills, blocked_tools, skill_selection_reason) =
        if config.skills.as_ref().map_or(false, |s| s.enabled) {
            let s = config.skills.as_ref().unwrap();
            let effective = resolve_effective_skills(
                app_state,
                project_path,
                query,
                phase,
                &s.selected_skill_ids,
                &s.invoked_skill_ids,
                s.selection_mode,
                s.review_filter.as_deref(),
                true,
                provider_config,
            )
            .await;
            (
                effective.skills_block,
                effective.skill_expertise,
                effective.matches,
                effective.blocked_tools,
                effective.selection_reason,
            )
        } else {
            (String::new(), vec![], vec![], vec![], String::new())
        };

    EnrichedContext {
        knowledge_block,
        memory_block,
        skills_block,
        skill_expertise,
        selected_skills,
        blocked_tools,
        skill_selection_reason,
    }
}

/// Like `query_selected_context` but skips knowledge (handled via tool).
///
/// Used when knowledge is on-demand via SearchKnowledge tool.
/// Only queries Memory + Skills; `knowledge_block` is always empty.
pub async fn query_selected_context_without_knowledge(
    config: &ContextSourceConfig,
    app_state: &AppState,
    project_path: &str,
    query: &str,
    phase: InjectionPhase,
    provider_config: Option<&ProviderConfig>,
) -> EnrichedContext {
    tracing::info!(
        "[ContextSource] query_selected_context_without_knowledge — memory={:?}, skills={:?}, project_id={}",
        config.memory.as_ref().map(|m| m.enabled),
        config.skills.as_ref().map(|s| s.enabled),
        config.project_id,
    );

    let memory_block = if config.memory.as_ref().map_or(false, |m| m.enabled) {
        let m = config.memory.as_ref().unwrap();
        query_memories_for_task_filtered(
            app_state,
            project_path,
            query,
            &m.selected_categories,
            &m.selected_memory_ids,
            &m.excluded_memory_ids,
            &m.selected_scopes,
            m.session_id.as_deref(),
            &m.statuses,
            m.review_mode.as_deref(),
        )
        .await
    } else {
        String::new()
    };

    let (skills_block, skill_expertise, selected_skills, blocked_tools, skill_selection_reason) =
        if config.skills.as_ref().map_or(false, |s| s.enabled) {
            let s = config.skills.as_ref().unwrap();
            let effective = resolve_effective_skills(
                app_state,
                project_path,
                query,
                phase,
                &s.selected_skill_ids,
                &s.invoked_skill_ids,
                s.selection_mode,
                s.review_filter.as_deref(),
                true,
                provider_config,
            )
            .await;
            (
                effective.skills_block,
                effective.skill_expertise,
                effective.matches,
                effective.blocked_tools,
                effective.selection_reason,
            )
        } else {
            (String::new(), vec![], vec![], vec![], String::new())
        };

    EnrichedContext {
        knowledge_block: String::new(), // Knowledge handled via SearchKnowledge tool
        memory_block,
        skills_block,
        skill_expertise,
        selected_skills,
        blocked_tools,
        skill_selection_reason,
    }
}

/// Re-export ensure_knowledge_initialized for use by standalone/task_mode commands.
pub async fn ensure_knowledge_initialized_public(
    knowledge_state: &KnowledgeState,
    app_state: &AppState,
) {
    ensure_knowledge_initialized(knowledge_state, app_state).await;
}

/// Query the Knowledge Base and return a formatted markdown context block.
///
/// Returns an empty string if the knowledge pipeline is not initialized or
/// the query fails for any reason.
pub async fn query_knowledge_for_task(
    knowledge_state: &KnowledgeState,
    project_id: &str,
    query: &str,
) -> String {
    let pipeline = match knowledge_state.get_pipeline().await {
        Ok(p) => p,
        Err(_) => return String::new(),
    };

    let provider = KnowledgeContextProvider::new(pipeline);
    let config = KnowledgeContextConfig::default();

    match provider.query_for_context(project_id, query, &config).await {
        Ok(chunks) => KnowledgeContextProvider::format_context_block(&chunks),
        Err(e) => {
            tracing::warn!("Knowledge query for task mode failed: {}", e);
            String::new()
        }
    }
}

/// Query the Knowledge Base with optional collection/document filtering.
///
/// When `collection_ids` or `document_refs` are non-empty, they are passed to
/// the `KnowledgeContextConfig` so the pipeline filters accordingly.
/// Empty vectors are treated as "no filter" (query all).
pub async fn query_knowledge_for_task_filtered(
    knowledge_state: &KnowledgeState,
    project_id: &str,
    query: &str,
    collection_ids: &[String],
    document_refs: &[ScopedDocumentRef],
) -> String {
    let pipeline = match knowledge_state.get_pipeline().await {
        Ok(p) => p,
        Err(_) => return String::new(),
    };

    let provider = KnowledgeContextProvider::new(pipeline);
    let config = KnowledgeContextConfig {
        collection_ids: if collection_ids.is_empty() {
            None
        } else {
            Some(collection_ids.to_vec())
        },
        document_refs: if document_refs.is_empty() {
            None
        } else {
            Some(document_refs.to_vec())
        },
        ..KnowledgeContextConfig::default()
    };

    match provider.query_for_context(project_id, query, &config).await {
        Ok(chunks) => KnowledgeContextProvider::format_context_block(&chunks),
        Err(e) => {
            tracing::warn!("Knowledge filtered query for task mode failed: {}", e);
            String::new()
        }
    }
}

/// Query Project Memory and return a formatted markdown section.
///
/// Returns an empty string if the memory store is not initialized or
/// the query yields no results.
pub async fn query_memories_for_task(
    app_state: &AppState,
    project_path: &str,
    query: &str,
) -> String {
    query_memories_for_task_filtered(
        app_state,
        project_path,
        query,
        &[],
        &[],
        &[],
        &[],
        None,
        &[],
        None,
    )
    .await
}

/// Query Project Memory with optional category/ID filtering.
///
/// - `selected_memory_ids` non-empty → fetch those specific entries (exact injection).
/// - `excluded_memory_ids` removes entries from both direct-id and semantic results.
/// - `selected_categories` non-empty → search within those categories only.
/// - `selected_scopes` controls which scope namespaces are queried.
/// - Empty scopes → project + global (+ session when session_id is provided).
/// Results are merged and deduplicated.
pub async fn query_memories_for_task_filtered(
    app_state: &AppState,
    project_path: &str,
    query: &str,
    selected_categories: &[String],
    selected_memory_ids: &[String],
    excluded_memory_ids: &[String],
    selected_scopes: &[String],
    session_id: Option<&str>,
    selected_statuses: &[String],
    review_mode: Option<&str>,
) -> String {
    let store = match app_state.get_memory_store_arc().await {
        Ok(store) => store,
        Err(_) => return String::new(),
    };

    let parsed_categories: Vec<MemoryCategory> = selected_categories
        .iter()
        .filter_map(|category| MemoryCategory::from_str(category).ok())
        .collect();
    let parsed_scopes: Vec<MemoryScopeV2> = selected_scopes
        .iter()
        .filter_map(|scope| MemoryScopeV2::from_str(scope))
        .collect();
    let parsed_statuses = resolve_memory_statuses(selected_statuses, review_mode);
    let should_query = selected_memory_ids.is_empty() || !parsed_categories.is_empty();
    let query_text = if should_query {
        query.to_string()
    } else {
        String::new()
    };
    let tuning = tuning_for_task_context_v2(!selected_memory_ids.is_empty());
    let request = UnifiedMemoryQueryRequestV2 {
        project_path: project_path.to_string(),
        query: query_text,
        scopes: parsed_scopes,
        categories: parsed_categories,
        include_ids: selected_memory_ids.to_vec(),
        exclude_ids: excluded_memory_ids.to_vec(),
        session_id: session_id.map(|sid| sid.to_string()),
        top_k_total: tuning.top_k_total,
        min_importance: tuning.min_importance,
        per_scope_budget: tuning.per_scope_budget,
        intent: crate::services::memory::retrieval::MemorySearchIntent::Default,
        enable_semantic: true,
        enable_lexical: true,
        statuses: parsed_statuses,
    };

    let queried = match query_memory_entries_unified_v2(store.as_ref(), &request).await {
        Ok(rows) => rows,
        Err(_) => return String::new(),
    };
    if queried.results.is_empty() {
        return String::new();
    }

    let entries = queried
        .results
        .into_iter()
        .map(|row| row.entry)
        .collect::<Vec<_>>();
    build_memory_section(Some(&entries))
}

/// Select applicable Skills and return a formatted block plus expertise items.
///
/// Returns `("", vec![])` if skill discovery or selection fails.
pub async fn select_skills_for_task(
    app_state: &AppState,
    project_path: &str,
    query: &str,
    phase: InjectionPhase,
) -> (String, Vec<String>) {
    select_skills_for_task_filtered(app_state, project_path, query, phase, &[]).await
}

/// Select matched skills with optional user-specified ID filtering.
///
/// - `selected_skill_ids` non-empty → only include those skills (UserForced reason).
/// - `selected_skill_ids` empty → use automatic selection logic.
pub async fn select_skill_matches_for_task_filtered(
    app_state: &AppState,
    project_path: &str,
    query: &str,
    phase: InjectionPhase,
    selected_skill_ids: &[String],
) -> Vec<SkillMatch> {
    resolve_effective_skills(
        app_state,
        project_path,
        query,
        phase,
        selected_skill_ids,
        &[],
        if selected_skill_ids.is_empty() {
            SkillSelectionMode::Auto
        } else {
            SkillSelectionMode::Explicit
        },
        None,
        true,
        None,
    )
    .await
    .matches
}

/// Select Skills with optional user-specified ID filtering.
///
/// - `selected_skill_ids` non-empty → only include those skills (UserForced reason).
/// - `selected_skill_ids` empty → use automatic selection logic.
pub async fn select_skills_for_task_filtered(
    app_state: &AppState,
    project_path: &str,
    query: &str,
    phase: InjectionPhase,
    selected_skill_ids: &[String],
) -> (String, Vec<String>) {
    let effective = resolve_effective_skills(
        app_state,
        project_path,
        query,
        phase,
        selected_skill_ids,
        &[],
        if selected_skill_ids.is_empty() {
            SkillSelectionMode::Auto
        } else {
            SkillSelectionMode::Explicit
        },
        None,
        true,
        None,
    )
    .await;
    (effective.skills_block, effective.skill_expertise)
}

fn normalized_allowed_tools_from_skill_matches(matches: &[SkillMatch]) -> Option<HashSet<String>> {
    let mut allowed: Option<HashSet<String>> = None;

    for skill_match in matches {
        if !skill_match.skill.enabled || skill_match.skill.allowed_tools.is_empty() {
            continue;
        }
        let current = skill_match
            .skill
            .allowed_tools
            .iter()
            .map(|tool| tool.trim().to_ascii_lowercase())
            .filter(|tool| !tool.is_empty())
            .collect::<HashSet<_>>();
        if current.is_empty() {
            continue;
        }

        allowed = Some(match allowed.take() {
            Some(existing) => existing
                .intersection(&current)
                .cloned()
                .collect::<HashSet<_>>(),
            None => current,
        });
    }

    let Some(mut allowed) = allowed else {
        return None;
    };

    // Minimum safe exploration set.
    for safe_tool in ["read", "ls", "glob", "grep", "cwd"] {
        allowed.insert(safe_tool.to_string());
    }
    Some(allowed)
}

pub fn derive_blocked_tools_from_skill_policy(matches: &[SkillMatch]) -> Vec<String> {
    let Some(allowed) = normalized_allowed_tools_from_skill_matches(matches) else {
        return vec![];
    };

    let mut blocked = get_tool_definitions_from_registry()
        .into_iter()
        .filter_map(|tool| {
            let normalized = tool.name.trim().to_ascii_lowercase();
            if allowed.contains(&normalized) {
                None
            } else {
                Some(tool.name)
            }
        })
        .collect::<Vec<_>>();
    blocked.sort();
    blocked.dedup();
    blocked
}

fn select_skill_matches_by_ids(
    index: &SkillIndex,
    selected_skill_ids: &[String],
) -> Vec<SkillMatch> {
    let id_set: HashSet<&str> = selected_skill_ids.iter().map(|s| s.as_str()).collect();
    index
        .skills()
        .iter()
        .filter(|doc| doc.enabled && id_set.contains(doc.id.as_str()))
        .map(|doc| SkillMatch {
            score: 1.0,
            match_reason: crate::services::skills::model::MatchReason::UserForced,
            skill: doc.to_summary(false),
        })
        .collect()
}

fn merge_skill_matches(primary: Vec<SkillMatch>, supplemental: Vec<SkillMatch>) -> Vec<SkillMatch> {
    let mut merged = primary;
    let mut seen = merged
        .iter()
        .map(|skill| skill.skill.id.clone())
        .collect::<HashSet<_>>();
    for skill in supplemental {
        if seen.insert(skill.skill.id.clone()) {
            merged.push(skill);
        }
    }
    merged
}

fn filter_skill_matches_by_review(matches: Vec<SkillMatch>, index: &SkillIndex, include_pending_review: bool) -> Vec<SkillMatch> {
    matches
        .into_iter()
        .filter(|skill_match| {
            let Some(doc) = index.skills().iter().find(|doc| doc.id == skill_match.skill.id) else {
                return false;
            };
            match doc.review_status {
                None | Some(SkillReviewStatus::Approved) => true,
                Some(SkillReviewStatus::PendingReview) => include_pending_review,
                Some(SkillReviewStatus::Rejected | SkillReviewStatus::Archived) => false,
            }
        })
        .collect()
}

pub async fn hydrate_skill_matches_by_ids(
    app_state: &AppState,
    project_path: &str,
    skill_ids: &[String],
) -> Vec<SkillMatch> {
    if skill_ids.is_empty() {
        return Vec::new();
    }
    let Some(index) = build_unified_skill_index_for_task(app_state, project_path).await else {
        return Vec::new();
    };
    select_skill_matches_by_ids(&index, skill_ids)
}

fn build_skill_block_from_matches(
    index: &SkillIndex,
    matches: &[SkillMatch],
    policy: &SelectionPolicy,
) -> String {
    if matches.is_empty() {
        return String::new();
    }

    let docs = matches
        .iter()
        .filter_map(|m| index.skills().iter().find(|d| d.id == m.skill.id))
        .collect::<Vec<_>>();

    if docs.len() == matches.len() {
        return inject_skills(matches, docs.as_slice(), policy);
    }

    // Fallback path for partial index misses.
    inject_skill_summaries(matches, policy)
}

fn project_relative_skill_path(project_root: &Path, path: &Path) -> String {
    match path.strip_prefix(project_root) {
        Ok(relative) => relative.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

fn skill_matches_phase(doc: &SkillDocument, phase: InjectionPhase) -> bool {
    doc.inject_into.contains(&InjectionPhase::Always) || doc.inject_into.contains(&phase)
}

fn skill_source_label(source: &SkillSource) -> String {
    match source {
        SkillSource::Builtin => "builtin".to_string(),
        SkillSource::External { .. } => "external".to_string(),
        SkillSource::User => "user".to_string(),
        SkillSource::ProjectLocal => "project_local".to_string(),
        SkillSource::Generated => "generated".to_string(),
    }
}

fn collect_hierarchy_matches(project_root: &Path, index: &SkillIndex, matches: &[SkillMatch]) -> Vec<String> {
    let mut hierarchy = BTreeSet::new();

    for skill_match in matches {
        let Some(doc) = index.skills().iter().find(|doc| doc.id == skill_match.skill.id) else {
            continue;
        };

        let relative = project_relative_skill_path(project_root, &doc.path);
        if relative.starts_with(".skills/") || relative.ends_with(".md") {
            hierarchy.insert(relative);
        }
    }

    hierarchy.into_iter().collect()
}

fn collect_why_not_selected(
    project_root: &Path,
    index: &SkillIndex,
    phase: InjectionPhase,
    final_matches: &[SkillMatch],
    pre_review_matches: &[SkillMatch],
    selected_skill_ids: &[String],
    invoked_skill_ids: &[String],
    force_user_selected: bool,
    include_pending_review: bool,
) -> Vec<NonSelectedSkillDiagnostic> {
    let final_ids = final_matches
        .iter()
        .map(|skill| skill.skill.id.as_str())
        .collect::<HashSet<_>>();
    let pre_review_ids = pre_review_matches
        .iter()
        .map(|skill| skill.skill.id.as_str())
        .collect::<HashSet<_>>();
    let selected_ids = selected_skill_ids
        .iter()
        .map(|value| value.as_str())
        .collect::<HashSet<_>>();
    let invoked_ids = invoked_skill_ids
        .iter()
        .map(|value| value.as_str())
        .collect::<HashSet<_>>();

    let mut diagnostics = index
        .skills()
        .iter()
        .filter(|doc| !final_ids.contains(doc.id.as_str()))
        .map(|doc| {
            let reason = if !doc.enabled {
                "disabled"
            } else if !skill_matches_phase(doc, phase.clone()) {
                "phase_mismatch"
            } else {
                match doc.review_status {
                    Some(SkillReviewStatus::PendingReview) if !include_pending_review => "pending_review",
                    Some(SkillReviewStatus::Rejected) => "rejected",
                    Some(SkillReviewStatus::Archived) => "archived",
                    _ if force_user_selected
                        && !selected_ids.contains(doc.id.as_str())
                        && !invoked_ids.contains(doc.id.as_str()) =>
                    {
                        "not_in_explicit_selection"
                    }
                    _ if !pre_review_ids.contains(doc.id.as_str()) => "unmatched",
                    _ => "filtered_out",
                }
            };

            NonSelectedSkillDiagnostic {
                skill_id: doc.id.clone(),
                skill_name: doc.name.clone(),
                reason: reason.to_string(),
                source_type: skill_source_label(&doc.source),
                path: project_relative_skill_path(project_root, &doc.path),
                review_status: doc.review_status.clone(),
            }
        })
        .collect::<Vec<_>>();

    diagnostics.sort_by(|left, right| {
        left.reason
            .cmp(&right.reason)
            .then_with(|| left.skill_name.cmp(&right.skill_name))
    });
    diagnostics
}

pub async fn resolve_effective_skills(
    app_state: &AppState,
    project_path: &str,
    query: &str,
    phase: InjectionPhase,
    selected_skill_ids: &[String],
    invoked_skill_ids: &[String],
    selection_mode: SkillSelectionMode,
    review_filter: Option<&str>,
    enforce_user_selection: bool,
    provider_config: Option<&ProviderConfig>,
) -> EffectiveSkillPlan {
    let project_root = Path::new(project_path);
    let index = match build_unified_skill_index_for_task(app_state, project_path).await {
        Some(idx) => idx,
        None => {
            return EffectiveSkillPlan {
                selection_reason: "skills_index_unavailable".to_string(),
                selection_origin: "none".to_string(),
                ..EffectiveSkillPlan::default()
            }
        }
    };

    let force_user_selected = enforce_user_selection
        && (selection_mode == SkillSelectionMode::Explicit || !selected_skill_ids.is_empty());
    let include_pending_review = matches!(
        review_filter
            .unwrap_or("approved_only")
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "include_pending_review" | "diagnostic"
    );

    let command_invoked_matches = filter_skill_matches_by_review(
        select_skill_matches_by_ids(&index, invoked_skill_ids),
        &index,
        include_pending_review,
    );
    let mut router_diagnostics = SkillRerankDiagnostics::default();
    let (pre_review_matches, selection_reason, selection_origin) = if force_user_selected {
        let pre_review_matches = merge_skill_matches(
            select_skill_matches_by_ids(&index, selected_skill_ids),
            command_invoked_matches.clone(),
        );
        let selection_reason = if !invoked_skill_ids.is_empty() {
            "skills_explicit_and_command_invoked".to_string()
        } else {
            "skills_user_selected".to_string()
        };
        let selection_origin = if !invoked_skill_ids.is_empty() {
            "mixed".to_string()
        } else {
            "explicit".to_string()
        };
        (pre_review_matches, selection_reason, selection_origin)
    } else {
        let policy = SelectionPolicy::default();
        let recalled_candidates = filter_skill_matches_by_review(
            select_skill_candidates_for_session(
                &index,
                project_root,
                query,
                &phase,
                &policy,
                10,
            ),
            &index,
            include_pending_review,
        );
        let deterministic_auto =
            filter_skill_matches_by_review(
                select_skills_for_session(&index, project_root, query, &phase, &policy),
                &index,
                include_pending_review,
            );
        let deterministic_auto_empty = deterministic_auto.is_empty();
        let rerank_result = rerank_skill_matches(
            project_root,
            query,
            &phase,
            &recalled_candidates,
            provider_config,
        )
        .await;
        router_diagnostics = rerank_result.1;
        let mut auto_matches = match rerank_result.0 {
            Some(matches) => matches,
            None => deterministic_auto,
        };
        if auto_matches.is_empty() && !recalled_candidates.is_empty() && deterministic_auto_empty {
            auto_matches = recalled_candidates
                .into_iter()
                .take(policy.top_k)
                .collect();
        }
        let pre_review_matches = merge_skill_matches(command_invoked_matches.clone(), auto_matches);
        let rerank_succeeded = router_diagnostics.skill_router_used
            && router_diagnostics.skill_router_fallback_reason.is_none()
            && !router_diagnostics.skill_router_selected_ids.is_empty();
        let rerank_fell_back = router_diagnostics.skill_router_fallback_reason.is_some();
        let selection_reason = if !invoked_skill_ids.is_empty() {
            "skills_command_invoked".to_string()
        } else if rerank_succeeded {
            "skills_auto_llm_rerank".to_string()
        } else if rerank_fell_back {
            "skills_auto_llm_fallback".to_string()
        } else if pre_review_matches.is_empty() {
            "skills_no_match".to_string()
        } else {
            "skills_auto_match".to_string()
        };
        let selection_origin = if !invoked_skill_ids.is_empty() {
            "command_invoked".to_string()
        } else if rerank_succeeded {
            "auto_llm".to_string()
        } else if rerank_fell_back {
            "auto_fallback".to_string()
        } else if pre_review_matches.is_empty() {
            "none".to_string()
        } else {
            "auto".to_string()
        };
        (pre_review_matches, selection_reason, selection_origin)
    };
    let matches = filter_skill_matches_by_review(pre_review_matches.clone(), &index, include_pending_review);
    let selection_reason = if !force_user_selected
        && matches.is_empty()
        && !invoked_skill_ids.is_empty()
        && selection_origin == "command_invoked"
    {
        "skills_command_invoked_unavailable".to_string()
    } else if !force_user_selected
        && matches.is_empty()
        && (selection_origin == "auto" || selection_origin == "auto_llm")
    {
        "skills_no_match".to_string()
    } else {
        selection_reason
    };
    let selection_origin = if !force_user_selected
        && matches.is_empty()
        && selection_origin == "auto"
    {
        "none".to_string()
    } else {
        selection_origin
    };

    let policy = SelectionPolicy::default();
    let skills_block = build_skill_block_from_matches(&index, &matches, &policy);
    let skill_expertise = matches
        .iter()
        .map(|m| format!("{} best practices", m.skill.name))
        .collect::<Vec<_>>();
    let blocked_tools = derive_blocked_tools_from_skill_policy(&matches);
    let hierarchy_matches = collect_hierarchy_matches(project_root, &index, &matches);
    let why_not_selected = collect_why_not_selected(
        project_root,
        &index,
        phase,
        &matches,
        &pre_review_matches,
        selected_skill_ids,
        invoked_skill_ids,
        force_user_selected,
        include_pending_review,
    );

    EffectiveSkillPlan {
        matches,
        skills_block,
        skill_expertise,
        blocked_tools,
        selection_reason,
        selection_origin,
        hierarchy_matches,
        why_not_selected,
        router_diagnostics,
    }
}

pub async fn build_unified_skill_index_for_task(
    app_state: &AppState,
    project_path: &str,
) -> Option<SkillIndex> {
    let project_root = Path::new(project_path);

    // Load config
    let config_path = project_root.join("external-skills.json");
    let config = load_skills_config(&config_path).unwrap_or_default();
    let plan_cascade_dir = crate::utils::paths::plan_cascade_dir().ok();

    // Discover and build file-based index
    let discovered =
        discover_all_skills(project_root, &config, plan_cascade_dir.as_deref()).ok()?;
    let mut index = build_index(discovered).ok()?;

    // Apply persisted disabled state from the database for file-based skills.
    let disabled_ids: std::collections::HashSet<String> = app_state
        .with_database(|db| {
            let rows = db.get_settings_by_prefix("skill_disabled:")?;
            Ok(rows
                .into_iter()
                .map(|(key, _)| {
                    key.strip_prefix("skill_disabled:")
                        .unwrap_or(&key)
                        .to_string()
                })
                .collect())
        })
        .await
        .unwrap_or_default();

    if !disabled_ids.is_empty() {
        let mut docs = index.skills().to_vec();
        for doc in &mut docs {
            if disabled_ids.contains(&doc.id) {
                doc.enabled = false;
            }
        }
        index = SkillIndex::new(docs);
    }

    let mut docs = index.skills().to_vec();
    let generated_records = app_state
        .with_database(|db| {
            let store = SkillGeneratorStore::new(std::sync::Arc::new(db.clone()));
            store.list_generated_skills(project_path, false)
        })
        .await
        .unwrap_or_default();

    docs.extend(
        generated_records
            .into_iter()
            .map(generated_record_to_skill_document),
    );

    Some(SkillIndex::new(docs))
}

fn generated_record_to_skill_document(record: GeneratedSkillRecord) -> SkillDocument {
    SkillDocument {
        id: record.id.clone(),
        name: record.name,
        description: record.description,
        version: None,
        tags: record.tags,
        body: record.body,
        path: std::path::PathBuf::from(format!("generated://{}", record.id)),
        hash: format!("generated-{}", record.id),
        last_modified: None,
        user_invocable: false,
        allowed_tools: vec![],
        license: None,
        metadata: std::collections::HashMap::new(),
        hooks: None,
        source: SkillSource::Generated,
        priority: 0,
        detect: None,
        inject_into: vec![InjectionPhase::Always],
        enabled: record.enabled,
        review_status: Some(record.review_status),
        review_notes: record.review_notes,
        reviewed_at: record.reviewed_at,
    }
}

/// Merge exploration context, knowledge block, and memory block into a single
/// enriched context string.
///
/// Returns `None` if all inputs are empty/absent.
pub fn merge_enriched_context(
    exploration_context: Option<&str>,
    knowledge_block: &str,
    memory_block: &str,
) -> Option<String> {
    let has_exploration = exploration_context.map_or(false, |s| !s.is_empty());
    let has_knowledge = !knowledge_block.is_empty();
    let has_memory = !memory_block.is_empty();

    if !has_exploration && !has_knowledge && !has_memory {
        return None;
    }

    let mut merged = String::new();

    if let Some(ctx) = exploration_context {
        if !ctx.is_empty() {
            merged.push_str(ctx);
        }
    }

    if has_knowledge {
        if !merged.is_empty() {
            merged.push_str("\n\n");
        }
        merged.push_str(knowledge_block);
    }

    if has_memory {
        if !merged.is_empty() {
            merged.push_str("\n\n");
        }
        merged.push_str(memory_block);
    }

    Some(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_memory_statuses_defaults_to_active_only() {
        let statuses = resolve_memory_statuses(&[], None);
        assert_eq!(statuses, vec![MemoryStatusV2::Active]);
    }

    #[test]
    fn resolve_memory_statuses_respects_review_mode_when_no_explicit_statuses() {
        let statuses = resolve_memory_statuses(&[], Some("include_pending_review"));
        assert_eq!(
            statuses,
            vec![MemoryStatusV2::Active, MemoryStatusV2::PendingReview]
        );
    }

    #[test]
    fn resolve_memory_statuses_explicit_statuses_override_review_mode() {
        let statuses = resolve_memory_statuses(
            &[String::from("archived"), String::from("rejected")],
            Some("include_pending_review"),
        );
        assert_eq!(
            statuses,
            vec![MemoryStatusV2::Archived, MemoryStatusV2::Rejected]
        );
    }
}
