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

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::commands::knowledge::KnowledgeState;
use crate::services::knowledge::context_provider::{
    KnowledgeContextConfig, KnowledgeContextProvider,
};
use crate::services::memory::retrieval::search_memories;
use crate::services::memory::store::MemoryCategory;
use crate::services::memory::MemorySearchRequest;
use crate::services::skills::config::load_skills_config;
use crate::services::skills::discovery::discover_all_skills;
use crate::services::skills::index::build_index;
use crate::services::skills::injector::inject_skill_summaries;
use crate::services::skills::model::{InjectionPhase, SelectionPolicy, SkillMatch};
use crate::services::skills::select::select_skills_for_session;
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
    pub selected_documents: Vec<String>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsSourceConfig {
    pub enabled: bool,
    /// Specific skill IDs the user selected.
    /// Empty = use automatic selection logic (backward-compatible).
    #[serde(default)]
    pub selected_skill_ids: Vec<String>,
}

/// Aggregated domain context from all sources.
#[derive(Debug, Clone, Default)]
pub struct EnrichedContext {
    pub knowledge_block: String,
    pub memory_block: String,
    pub skills_block: String,
    pub skill_expertise: Vec<String>,
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
async fn ensure_knowledge_initialized(
    knowledge_state: &KnowledgeState,
    app_state: &AppState,
) {
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
            tracing::warn!("[ContextSource] Failed to access database for knowledge init: {}", e);
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
        tracing::warn!("[ContextSource] Knowledge pipeline initialization failed: {}", e);
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
    let knowledge_block = if config
        .knowledge
        .as_ref()
        .map_or(false, |k| k.enabled)
    {
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
        )
        .await
    } else {
        String::new()
    };

    let (skills_block, skill_expertise) =
        if config.skills.as_ref().map_or(false, |s| s.enabled) {
            let s = config.skills.as_ref().unwrap();
            select_skills_for_task_filtered(
                app_state,
                project_path,
                query,
                phase,
                &s.selected_skill_ids,
            )
            .await
        } else {
            (String::new(), vec![])
        };

    EnrichedContext {
        knowledge_block,
        memory_block,
        skills_block,
        skill_expertise,
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
        )
        .await
    } else {
        String::new()
    };

    let (skills_block, skill_expertise) =
        if config.skills.as_ref().map_or(false, |s| s.enabled) {
            let s = config.skills.as_ref().unwrap();
            select_skills_for_task_filtered(
                app_state,
                project_path,
                query,
                phase,
                &s.selected_skill_ids,
            )
            .await
        } else {
            (String::new(), vec![])
        };

    EnrichedContext {
        knowledge_block: String::new(), // Knowledge handled via SearchKnowledge tool
        memory_block,
        skills_block,
        skill_expertise,
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
/// When `collection_ids` or `document_ids` are non-empty, they are passed to
/// the `KnowledgeContextConfig` so the pipeline filters accordingly.
/// Empty vectors are treated as "no filter" (query all).
pub async fn query_knowledge_for_task_filtered(
    knowledge_state: &KnowledgeState,
    project_id: &str,
    query: &str,
    collection_ids: &[String],
    document_ids: &[String],
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
        document_ids: if document_ids.is_empty() {
            None
        } else {
            Some(document_ids.to_vec())
        },
        ..KnowledgeContextConfig::default()
    };

    match provider
        .query_for_context(project_id, query, &config)
        .await
    {
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
    let request = MemorySearchRequest {
        project_path: project_path.to_string(),
        query: query.to_string(),
        categories: None,
        top_k: 10,
        min_importance: 0.3,
    };

    let entries = match app_state
        .with_memory_store(|store| search_memories(store, &request))
        .await
    {
        Ok(results) => results
            .into_iter()
            .map(|r| r.entry)
            .collect::<Vec<_>>(),
        Err(_) => return String::new(),
    };

    if entries.is_empty() {
        return String::new();
    }

    build_memory_section(Some(&entries))
}

/// Query Project Memory with optional category/ID filtering.
///
/// - `selected_memory_ids` non-empty → fetch those specific entries (exact injection).
/// - `selected_categories` non-empty → search within those categories only.
/// - Both empty + enabled → search all categories (current behavior).
/// Results are merged and deduplicated.
pub async fn query_memories_for_task_filtered(
    app_state: &AppState,
    project_path: &str,
    query: &str,
    selected_categories: &[String],
    selected_memory_ids: &[String],
) -> String {
    let mut entries = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // 1. If specific IDs requested, fetch them directly
    if !selected_memory_ids.is_empty() {
        let ids = selected_memory_ids.to_vec();
        if let Ok(fetched) = app_state
            .with_memory_store(|store| {
                let mut results = Vec::new();
                for id in &ids {
                    if let Ok(Some(entry)) = store.get_memory(id) {
                        results.push(entry);
                    }
                }
                Ok(results)
            })
            .await
        {
            for entry in fetched {
                seen_ids.insert(entry.id.clone());
                entries.push(entry);
            }
        }
    }

    // 2. If categories selected (or no specific IDs either → search all), do a semantic search
    if selected_memory_ids.is_empty() || !selected_categories.is_empty() {
        let categories = if selected_categories.is_empty() {
            None
        } else {
            let cats: Vec<MemoryCategory> = selected_categories
                .iter()
                .filter_map(|s| MemoryCategory::from_str(s).ok())
                .collect();
            if cats.is_empty() { None } else { Some(cats) }
        };

        let request = MemorySearchRequest {
            project_path: project_path.to_string(),
            query: query.to_string(),
            categories,
            top_k: 10,
            min_importance: 0.3,
        };

        if let Ok(results) = app_state
            .with_memory_store(|store| search_memories(store, &request))
            .await
        {
            for r in results {
                if seen_ids.insert(r.entry.id.clone()) {
                    entries.push(r.entry);
                }
            }
        }
    }

    if entries.is_empty() {
        return String::new();
    }

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
    let project_root = Path::new(project_path);

    // Load config
    let config_path = project_root.join("external-skills.json");
    let config = load_skills_config(&config_path).unwrap_or_default();

    // Discover skills
    let discovered = match discover_all_skills(project_root, &config, None) {
        Ok(d) => d,
        Err(_) => return (String::new(), vec![]),
    };

    // Build index
    let mut index = match build_index(discovered) {
        Ok(idx) => idx,
        Err(_) => return (String::new(), vec![]),
    };

    // Apply persisted disabled state from the database
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
        index = crate::services::skills::model::SkillIndex::new(docs);
    }

    let policy = SelectionPolicy::default();
    let matches: Vec<SkillMatch> =
        select_skills_for_session(&index, project_root, query, &phase, &policy);

    if matches.is_empty() {
        return (String::new(), vec![]);
    }

    // Collect expertise items from matched skill names
    let expertise: Vec<String> = matches
        .iter()
        .map(|m| format!("{} best practices", m.skill.name))
        .collect();

    let block = inject_skill_summaries(&matches, &policy);
    (block, expertise)
}

/// Select Skills with optional user-specified ID filtering.
///
/// - `selected_skill_ids` non-empty → only include those skills (UserForced reason).
/// - `selected_skill_ids` empty → use automatic selection logic (current behavior).
pub async fn select_skills_for_task_filtered(
    app_state: &AppState,
    project_path: &str,
    query: &str,
    phase: InjectionPhase,
    selected_skill_ids: &[String],
) -> (String, Vec<String>) {
    if selected_skill_ids.is_empty() {
        // Fall back to automatic selection
        return select_skills_for_task(app_state, project_path, query, phase).await;
    }

    let project_root = Path::new(project_path);

    // Load config & discover
    let config_path = project_root.join("external-skills.json");
    let config = load_skills_config(&config_path).unwrap_or_default();
    let discovered = match discover_all_skills(project_root, &config, None) {
        Ok(d) => d,
        Err(_) => return (String::new(), vec![]),
    };
    let index = match build_index(discovered) {
        Ok(idx) => idx,
        Err(_) => return (String::new(), vec![]),
    };

    // Filter to user-selected IDs
    let id_set: std::collections::HashSet<&str> =
        selected_skill_ids.iter().map(|s| s.as_str()).collect();
    let matches: Vec<SkillMatch> = index
        .skills()
        .iter()
        .filter(|doc| id_set.contains(doc.id.as_str()))
        .map(|doc| SkillMatch {
            score: 1.0,
            match_reason: crate::services::skills::model::MatchReason::UserForced,
            skill: doc.to_summary(false),
        })
        .collect();

    if matches.is_empty() {
        return (String::new(), vec![]);
    }

    let expertise: Vec<String> = matches
        .iter()
        .map(|m| format!("{} best practices", m.skill.name))
        .collect();

    let policy = SelectionPolicy::default();
    let block = inject_skill_summaries(&matches, &policy);
    (block, expertise)
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
