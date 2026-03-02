//! Memory Retrieval and Ranking
//!
//! Implements the 4-signal ranking algorithm for searching project memories:
//! embedding similarity, keyword overlap, importance, and recency.
//!
//! ## Search Flow
//!
//! 1. Generate TF-IDF embedding for query (reuse EmbeddingService)
//! 2. Retrieve candidate memories from SQLite (filter by project_path, optional category, importance >= min_importance)
//! 3. Compute cosine similarity between query embedding and each candidate
//! 4. Compute keyword overlap (Jaccard coefficient)
//! 5. Apply combined scoring formula
//! 6. Sort by final_score descending, return top_k
//! 7. Optionally bump access_count and last_accessed_at for returned entries

use std::collections::{HashMap, HashSet};

use chrono::{NaiveDateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::services::memory::query_policy_v2::{memory_query_tuning_v2, MemoryQueryPresetV2};
use crate::services::memory::store::{
    bytes_to_embedding, MemoryCategory, MemoryEntry, MemorySearchRequest, MemorySearchResult,
    ProjectMemoryStore, GLOBAL_PROJECT_PATH, SESSION_PROJECT_PATH_PREFIX,
};
use crate::services::orchestrator::embedding_manager::EmbeddingManager;
use crate::services::orchestrator::embedding_provider::EmbeddingProviderType;
use crate::services::orchestrator::embedding_service::cosine_similarity;
use crate::utils::error::AppResult;

#[derive(Debug, Clone)]
struct ScopeFilter {
    scope: String,
    project_path: Option<String>,
    session_id: Option<String>,
}

fn resolve_scope_filter(project_path: &str) -> ScopeFilter {
    if project_path == GLOBAL_PROJECT_PATH {
        ScopeFilter {
            scope: "global".to_string(),
            project_path: None,
            session_id: None,
        }
    } else if let Some(session) = project_path.strip_prefix(SESSION_PROJECT_PATH_PREFIX) {
        ScopeFilter {
            scope: "session".to_string(),
            project_path: None,
            session_id: Some(session.to_string()),
        }
    } else {
        ScopeFilter {
            scope: "project".to_string(),
            project_path: Some(project_path.to_string()),
            session_id: None,
        }
    }
}

fn legacy_project_path_from_scope(
    scope: &str,
    project_path: Option<&str>,
    session_id: Option<&str>,
) -> String {
    match scope {
        "global" => GLOBAL_PROJECT_PATH.to_string(),
        "session" => session_id
            .map(|sid| format!("{}{}", SESSION_PROJECT_PATH_PREFIX, sid))
            .unwrap_or_else(|| GLOBAL_PROJECT_PATH.to_string()),
        _ => project_path.unwrap_or_default().to_string(),
    }
}

/// Ranking mode for memory retrieval.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRankingMode {
    /// Full 4-signal ranking: embedding + keyword + importance + recency.
    Hybrid,
    /// Browse mode for empty/noisy queries: importance + recency only.
    Browse,
}

/// Whether search should mutate access counters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTouchPolicy {
    /// Read-only search. Does not update access counters.
    NoTouch,
    /// Touch all returned results by incrementing access counters.
    TouchReturned,
}

/// Search-time behavior controls.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemorySearchOptions {
    pub ranking_mode: MemoryRankingMode,
    pub touch_policy: MemoryTouchPolicy,
}

impl Default for MemorySearchOptions {
    fn default() -> Self {
        Self {
            ranking_mode: MemoryRankingMode::Hybrid,
            touch_policy: MemoryTouchPolicy::NoTouch,
        }
    }
}

/// Retrieval intent used to tune ranking weights.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySearchIntent {
    Default,
    Bugfix,
    Refactor,
    Qa,
    Docs,
}

impl Default for MemorySearchIntent {
    fn default() -> Self {
        Self::Default
    }
}

/// V2 memory search request with configurable channels and intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchRequestV2 {
    pub project_path: String,
    pub query: String,
    pub categories: Option<Vec<MemoryCategory>>,
    pub top_k: usize,
    pub min_importance: f32,
    #[serde(default)]
    pub intent: MemorySearchIntent,
    #[serde(default = "default_true")]
    pub enable_semantic: bool,
    #[serde(default = "default_true")]
    pub enable_lexical: bool,
}

fn default_true() -> bool {
    true
}

impl Default for MemorySearchRequestV2 {
    fn default() -> Self {
        let tuning = memory_query_tuning_v2(MemoryQueryPresetV2::CommandSearch);
        Self {
            project_path: String::new(),
            query: String::new(),
            categories: None,
            top_k: tuning.top_k_total,
            min_importance: tuning.min_importance,
            intent: MemorySearchIntent::Default,
            enable_semantic: true,
            enable_lexical: true,
        }
    }
}

/// Weighted score components returned for explainability.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryScoreBreakdown {
    pub semantic_similarity: f32,
    pub lexical_overlap: f32,
    pub keyword_overlap: f32,
    pub importance: f32,
    pub recency_score: f32,
    pub source_reliability: f32,
    pub semantic_weight: f32,
    pub lexical_weight: f32,
    pub keyword_weight: f32,
    pub importance_weight: f32,
    pub recency_weight: f32,
    pub reliability_weight: f32,
    pub final_score: f32,
}

/// Search result with explainable scoring details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResultV2 {
    pub entry: crate::services::memory::store::MemoryEntry,
    pub relevance_score: f32,
    pub score_breakdown: MemoryScoreBreakdown,
    pub semantic_channel: String,
    pub degraded: bool,
}

#[derive(Debug, Clone, Copy)]
struct IntentWeights {
    semantic: f32,
    lexical: f32,
    keyword: f32,
    importance: f32,
    recency: f32,
    reliability: f32,
}

impl IntentWeights {
    fn for_intent(intent: MemorySearchIntent) -> Self {
        match intent {
            MemorySearchIntent::Bugfix => Self {
                semantic: 0.35,
                lexical: 0.25,
                keyword: 0.15,
                importance: 0.15,
                recency: 0.05,
                reliability: 0.05,
            },
            MemorySearchIntent::Refactor => Self {
                semantic: 0.32,
                lexical: 0.20,
                keyword: 0.13,
                importance: 0.20,
                recency: 0.05,
                reliability: 0.10,
            },
            MemorySearchIntent::Qa => Self {
                semantic: 0.28,
                lexical: 0.27,
                keyword: 0.18,
                importance: 0.12,
                recency: 0.10,
                reliability: 0.05,
            },
            MemorySearchIntent::Docs => Self {
                semantic: 0.24,
                lexical: 0.30,
                keyword: 0.18,
                importance: 0.12,
                recency: 0.06,
                reliability: 0.10,
            },
            MemorySearchIntent::Default => Self {
                semantic: 0.30,
                lexical: 0.22,
                keyword: 0.15,
                importance: 0.18,
                recency: 0.08,
                reliability: 0.07,
            },
        }
    }
}

/// Relevance scoring formula:
///
///   final_score = w1 * embedding_similarity
///               + w2 * keyword_overlap
///               + w3 * importance
///               + w4 * recency_score
///
/// Where:
///   w1 = 0.40  (semantic relevance)
///   w2 = 0.25  (keyword match)
///   w3 = 0.20  (importance weight)
///   w4 = 0.15  (recency bonus)
///
///   recency_score = 1.0 / (1.0 + days_since_last_access * 0.1)
///   keyword_overlap = |query_keywords ∩ memory_keywords| / |query_keywords ∪ memory_keywords|
pub fn compute_relevance_score(
    embedding_similarity: f32,
    keyword_overlap: f32,
    importance: f32,
    days_since_last_access: f64,
) -> f32 {
    let recency = 1.0 / (1.0 + days_since_last_access as f32 * 0.1);
    0.40 * embedding_similarity + 0.25 * keyword_overlap + 0.20 * importance + 0.15 * recency
}

/// Browse score used when no reliable semantic query is available.
///
/// Recency uses the same decay curve as hybrid scoring.
fn compute_browse_score(importance: f32, days_since_last_access: f64) -> f32 {
    let recency = 1.0 / (1.0 + days_since_last_access as f32 * 0.1);
    0.80 * importance + 0.20 * recency
}

/// Compute Jaccard coefficient between two keyword sets.
///
/// Returns |intersection| / |union|, or 0.0 if both sets are empty.
pub fn keyword_jaccard(query_keywords: &[String], memory_keywords: &[String]) -> f32 {
    if query_keywords.is_empty() && memory_keywords.is_empty() {
        return 0.0;
    }

    let query_set: HashSet<&str> = query_keywords.iter().map(|s| s.as_str()).collect();
    let memory_set: HashSet<&str> = memory_keywords.iter().map(|s| s.as_str()).collect();

    let intersection = query_set.intersection(&memory_set).count();
    let union = query_set.union(&memory_set).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

/// Compute days since a datetime string (SQLite format: "YYYY-MM-DD HH:MM:SS")
fn days_since(datetime_str: &str) -> f64 {
    if let Ok(dt) = NaiveDateTime::parse_from_str(datetime_str, "%Y-%m-%d %H:%M:%S") {
        let now = Utc::now().naive_utc();
        let duration = now.signed_duration_since(dt);
        duration.num_hours() as f64 / 24.0
    } else {
        // If parsing fails, assume it was accessed recently
        0.0
    }
}

/// Basic English stopwords that should not participate in keyword overlap.
fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "but"
            | "by"
            | "can"
            | "do"
            | "for"
            | "from"
            | "had"
            | "has"
            | "have"
            | "how"
            | "in"
            | "into"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "their"
            | "this"
            | "to"
            | "use"
            | "using"
            | "what"
            | "when"
            | "where"
            | "which"
            | "who"
            | "with"
            | "you"
            | "your"
    )
}

/// Extract simple keywords from a query string.
///
/// Splits on whitespace and non-alphanumeric characters, lowercases,
/// filters out short tokens (< 3 chars) and common stopwords.
pub fn extract_query_keywords(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| s.len() >= 3)
        .filter(|s| !is_stopword(s))
        .map(|s| s.to_string())
        .collect()
}

fn normalize_for_dedupe(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn lexical_overlap_score(query_keywords: &[String], content: &str) -> f32 {
    if query_keywords.is_empty() {
        return 0.0;
    }

    let content_lower = content.to_lowercase();
    let mut matched = 0usize;
    for kw in query_keywords {
        if content_lower.contains(kw) {
            matched += 1;
        }
    }

    matched as f32 / query_keywords.len() as f32
}

#[derive(Debug, Clone)]
struct MemoryCandidate {
    entry: MemoryEntry,
    embedding: Option<Vec<f32>>,
}

fn build_category_filter_sql(categories: &Option<Vec<MemoryCategory>>) -> String {
    if let Some(cats) = categories {
        if cats.is_empty() {
            String::new()
        } else {
            let cat_strs: Vec<String> = cats.iter().map(|c| format!("'{}'", c.as_str())).collect();
            format!(" AND category IN ({})", cat_strs.join(","))
        }
    } else {
        String::new()
    }
}

fn candidate_limit(top_k: usize) -> usize {
    top_k.max(1).saturating_mul(6).clamp(30, 300)
}

fn semantic_channel_without_semantic(enable_lexical: bool) -> String {
    if enable_lexical {
        "lexical_only".to_string()
    } else {
        "disabled".to_string()
    }
}

fn provider_channel_name(provider: EmbeddingProviderType) -> &'static str {
    match provider {
        EmbeddingProviderType::TfIdf => "tfidf",
        EmbeddingProviderType::Ollama => "ollama",
        EmbeddingProviderType::Qwen => "qwen",
        EmbeddingProviderType::Glm => "glm",
        EmbeddingProviderType::OpenAI => "openai",
    }
}

fn load_candidates_v2(
    store: &ProjectMemoryStore,
    request: &MemorySearchRequestV2,
) -> AppResult<Vec<MemoryCandidate>> {
    let conn = store.pool().get().map_err(|e| {
        crate::utils::error::AppError::database(format!("Failed to get connection: {}", e))
    })?;

    let category_filter = build_category_filter_sql(&request.categories);
    let scoped = resolve_scope_filter(&request.project_path);
    let sql = format!(
        "SELECT id, scope, project_path, session_id, category, content, keywords, importance, access_count,
                source_session_id, source_context, status, risk_tier, conflict_flag, created_at, updated_at, last_accessed_at,
                embedding
         FROM memory_entries_v2
         WHERE scope = ?1
           AND ((?1 = 'project' AND project_path = ?2) OR (?1 = 'session' AND session_id = ?3) OR (?1 = 'global'))
           AND status = 'active'
           AND importance >= ?4{}
         ORDER BY importance DESC
         LIMIT {}",
        category_filter,
        candidate_limit(request.top_k),
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        params![
            scoped.scope,
            scoped.project_path,
            scoped.session_id,
            request.min_importance
        ],
        |row| -> rusqlite::Result<MemoryCandidate> {
            let scope: String = row.get(1)?;
            let project_path: Option<String> = row.get(2)?;
            let session_id: Option<String> = row.get(3)?;
            let category_str: String = row.get(4)?;
            let keywords_json: String = row.get(6)?;
            let keywords: Vec<String> = serde_json::from_str(&keywords_json).unwrap_or_default();
            let embedding = row
                .get::<_, Option<Vec<u8>>>(17)?
                .map(|bytes| bytes_to_embedding(&bytes));
            let legacy_project_path = legacy_project_path_from_scope(
                &scope,
                project_path.as_deref(),
                session_id.as_deref(),
            );

            Ok(MemoryCandidate {
                entry: MemoryEntry {
                    id: row.get(0)?,
                    project_path: legacy_project_path,
                    scope: Some(scope),
                    session_id,
                    category: MemoryCategory::from_str(&category_str)
                        .unwrap_or(MemoryCategory::Fact),
                    content: row.get(5)?,
                    keywords,
                    importance: row.get(7)?,
                    access_count: row.get(8)?,
                    source_session_id: row.get(9)?,
                    source_context: row.get(10)?,
                    status: Some(row.get(11)?),
                    risk_tier: Some(row.get(12)?),
                    conflict_flag: Some(row.get::<_, i64>(13).unwrap_or(0) != 0),
                    trace_id: None,
                    created_at: row.get(14)?,
                    updated_at: row.get(15)?,
                    last_accessed_at: row.get(16)?,
                },
                embedding,
            })
        },
    )?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn compute_tfidf_semantic_scores(
    store: &ProjectMemoryStore,
    project_path: &str,
    query: &str,
    candidates: &[MemoryCandidate],
) -> Option<HashMap<String, f32>> {
    if candidates.is_empty() || store.ensure_vocabulary_for_project(project_path).is_err() {
        return None;
    }

    let query_embedding = store.embedding_service().embed_text(query);
    if query_embedding.is_empty() {
        return None;
    }

    let mut scores = HashMap::new();
    for candidate in candidates {
        let mem_emb = candidate
            .embedding
            .as_ref()
            .filter(|emb| !emb.is_empty() && emb.len() == query_embedding.len())
            .cloned()
            .unwrap_or_else(|| {
                store
                    .embedding_service()
                    .embed_text(&candidate.entry.content)
            });

        let similarity = if mem_emb.is_empty() {
            0.0
        } else {
            cosine_similarity(&query_embedding, &mem_emb)
        };
        scores.insert(candidate.entry.id.clone(), similarity);
    }

    Some(scores)
}

async fn compute_dense_semantic_scores(
    manager: &EmbeddingManager,
    query: &str,
    candidates: &[MemoryCandidate],
) -> Option<(HashMap<String, f32>, String)> {
    if candidates.is_empty() {
        return None;
    }

    let query_embedding = manager.embed_query(query).await.ok()?;
    if query_embedding.is_empty() {
        return None;
    }

    let docs: Vec<&str> = candidates
        .iter()
        .map(|candidate| candidate.entry.content.as_str())
        .collect();
    let doc_embeddings = manager.embed_documents(&docs).await.ok()?;
    if doc_embeddings.len() != candidates.len() {
        return None;
    }

    let mut scores = HashMap::new();
    for (candidate, mem_emb) in candidates.iter().zip(doc_embeddings.iter()) {
        let similarity = if mem_emb.is_empty() {
            0.0
        } else {
            cosine_similarity(&query_embedding, mem_emb)
        };
        scores.insert(candidate.entry.id.clone(), similarity);
    }

    if scores.is_empty() {
        return None;
    }

    Some((
        scores,
        provider_channel_name(manager.provider_type()).to_string(),
    ))
}

fn rank_candidates_v2(
    candidates: Vec<MemoryCandidate>,
    request: &MemorySearchRequestV2,
    semantic_scores: &HashMap<String, f32>,
    semantic_channel: &str,
    degraded: bool,
) -> Vec<MemorySearchResultV2> {
    let query_keywords = extract_query_keywords(&request.query);
    let weights = IntentWeights::for_intent(request.intent);
    let mut seen_normalized = HashSet::new();
    let mut final_results = Vec::new();

    for candidate in candidates {
        let normalized = normalize_for_dedupe(&candidate.entry.content);
        if !seen_normalized.insert(normalized) {
            continue;
        }

        let semantic_similarity = semantic_scores
            .get(&candidate.entry.id)
            .copied()
            .unwrap_or(0.0);
        let keyword_overlap = keyword_jaccard(&query_keywords, &candidate.entry.keywords);
        let lexical_overlap = if request.enable_lexical {
            lexical_overlap_score(&query_keywords, &candidate.entry.content)
        } else {
            0.0
        };
        let recency_score =
            1.0 / (1.0 + days_since(&candidate.entry.last_accessed_at) as f32 * 0.1);
        let source_reliability = if candidate.entry.source_session_id.is_some() {
            0.90
        } else {
            1.00
        };

        let final_score = (semantic_similarity * weights.semantic)
            + (lexical_overlap * weights.lexical)
            + (keyword_overlap * weights.keyword)
            + (candidate.entry.importance * weights.importance)
            + (recency_score * weights.recency)
            + (source_reliability * weights.reliability);

        let breakdown = MemoryScoreBreakdown {
            semantic_similarity,
            lexical_overlap,
            keyword_overlap,
            importance: candidate.entry.importance,
            recency_score,
            source_reliability,
            semantic_weight: weights.semantic,
            lexical_weight: weights.lexical,
            keyword_weight: weights.keyword,
            importance_weight: weights.importance,
            recency_weight: weights.recency,
            reliability_weight: weights.reliability,
            final_score,
        };

        final_results.push(MemorySearchResultV2 {
            entry: candidate.entry,
            relevance_score: final_score,
            score_breakdown: breakdown,
            semantic_channel: semantic_channel.to_string(),
            degraded,
        });
    }

    final_results.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    final_results.truncate(request.top_k.max(1));

    final_results
}

/// Search memories using the 4-signal ranking algorithm.
///
/// This function:
/// 1. Generates a TF-IDF embedding for the query
/// 2. Retrieves candidate memories filtered by project_path, categories, and min_importance
/// 3. Scores each candidate using the 4-signal formula
/// 4. Returns top_k results sorted by score descending
/// 5. Does not mutate access metadata by default
pub fn search_memories(
    store: &ProjectMemoryStore,
    request: &MemorySearchRequest,
) -> AppResult<Vec<MemorySearchResult>> {
    search_memories_with_options(store, request, MemorySearchOptions::default())
}

/// Search memories with explicit ranking and touch behavior.
pub fn search_memories_with_options(
    store: &ProjectMemoryStore,
    request: &MemorySearchRequest,
    options: MemorySearchOptions,
) -> AppResult<Vec<MemorySearchResult>> {
    // Ensure vocabulary and memory embeddings are ready for this project.
    if matches!(options.ranking_mode, MemoryRankingMode::Hybrid) {
        store.ensure_vocabulary_for_project(&request.project_path)?;
    }

    // Step 1: Generate query embedding
    let query_embedding = if matches!(options.ranking_mode, MemoryRankingMode::Hybrid) {
        store.embedding_service().embed_text(&request.query)
    } else {
        Vec::new()
    };
    let query_keywords = if matches!(options.ranking_mode, MemoryRankingMode::Hybrid) {
        extract_query_keywords(&request.query)
    } else {
        Vec::new()
    };

    // Step 2: Retrieve candidates from DB
    let conn = store.pool().get().map_err(|e| {
        crate::utils::error::AppError::database(format!("Failed to get connection: {}", e))
    })?;

    // Build category filter clause
    let category_filter = if let Some(ref cats) = request.categories {
        if cats.is_empty() {
            String::new()
        } else {
            let cat_strs: Vec<String> = cats.iter().map(|c| format!("'{}'", c.as_str())).collect();
            format!(" AND category IN ({})", cat_strs.join(","))
        }
    } else {
        String::new()
    };

    let scoped = resolve_scope_filter(&request.project_path);
    let sql = format!(
        "SELECT id, scope, project_path, session_id, category, content, keywords, importance, access_count,
                source_session_id, source_context, status, risk_tier, conflict_flag, created_at, updated_at, last_accessed_at,
                embedding
         FROM memory_entries_v2
         WHERE scope = ?1
           AND ((?1 = 'project' AND project_path = ?2) OR (?1 = 'session' AND session_id = ?3) OR (?1 = 'global'))
           AND status = 'active'
           AND importance >= ?4{}
         ORDER BY importance DESC",
        category_filter
    );

    let mut stmt = conn.prepare(&sql)?;

    struct Candidate {
        id: String,
        scope: String,
        project_path: Option<String>,
        session_id: Option<String>,
        category: String,
        content: String,
        keywords_json: String,
        importance: f32,
        access_count: i64,
        source_session_id: Option<String>,
        source_context: Option<String>,
        status: String,
        risk_tier: String,
        conflict_flag: bool,
        created_at: String,
        updated_at: String,
        last_accessed_at: String,
        embedding_bytes: Option<Vec<u8>>,
    }

    let candidates: Vec<Candidate> = stmt
        .query_map(
            params![
                scoped.scope,
                scoped.project_path,
                scoped.session_id,
                request.min_importance
            ],
            |row| {
                Ok(Candidate {
                    id: row.get(0)?,
                    scope: row.get(1)?,
                    project_path: row.get(2)?,
                    session_id: row.get(3)?,
                    category: row.get(4)?,
                    content: row.get(5)?,
                    keywords_json: row.get(6)?,
                    importance: row.get(7)?,
                    access_count: row.get(8)?,
                    source_session_id: row.get(9)?,
                    source_context: row.get(10)?,
                    status: row.get(11)?,
                    risk_tier: row.get(12)?,
                    conflict_flag: row.get::<_, i64>(13).unwrap_or(0) != 0,
                    created_at: row.get(14)?,
                    updated_at: row.get(15)?,
                    last_accessed_at: row.get(16)?,
                    embedding_bytes: row.get(17)?,
                })
            },
        )?
        .filter_map(|r| r.ok())
        .collect();

    // Release connection before scoring
    drop(stmt);
    drop(conn);

    // Step 3-5: Score each candidate
    let mut scored_results: Vec<MemorySearchResult> = candidates
        .into_iter()
        .map(|c| {
            // Compute embedding similarity
            let emb_sim = if let Some(ref bytes) = c.embedding_bytes {
                if !query_embedding.is_empty() {
                    let mem_emb = bytes_to_embedding(bytes);
                    cosine_similarity(&query_embedding, &mem_emb)
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // Compute keyword overlap
            let mem_keywords: Vec<String> =
                serde_json::from_str(&c.keywords_json).unwrap_or_default();
            let kw_overlap = keyword_jaccard(&query_keywords, &mem_keywords);

            // Compute recency
            let days = days_since(&c.last_accessed_at);

            // Combine scores
            let score = match options.ranking_mode {
                MemoryRankingMode::Hybrid => {
                    compute_relevance_score(emb_sim, kw_overlap, c.importance, days)
                }
                MemoryRankingMode::Browse => compute_browse_score(c.importance, days),
            };

            let category = MemoryCategory::from_str(&c.category).unwrap_or(MemoryCategory::Fact);
            let legacy_project_path = legacy_project_path_from_scope(
                &c.scope,
                c.project_path.as_deref(),
                c.session_id.as_deref(),
            );

            MemorySearchResult {
                entry: crate::services::memory::store::MemoryEntry {
                    id: c.id,
                    project_path: legacy_project_path,
                    scope: Some(c.scope),
                    session_id: c.session_id,
                    category,
                    content: c.content,
                    keywords: mem_keywords,
                    importance: c.importance,
                    access_count: c.access_count,
                    source_session_id: c.source_session_id,
                    source_context: c.source_context,
                    status: Some(c.status),
                    risk_tier: Some(c.risk_tier),
                    conflict_flag: Some(c.conflict_flag),
                    trace_id: None,
                    created_at: c.created_at,
                    updated_at: c.updated_at,
                    last_accessed_at: c.last_accessed_at,
                },
                relevance_score: score,
            }
        })
        .collect();

    // Step 6: Sort by score descending, take top_k
    scored_results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());
    scored_results.truncate(request.top_k);

    // Optional touch after read.
    if matches!(options.touch_policy, MemoryTouchPolicy::TouchReturned)
        && !scored_results.is_empty()
    {
        let ids: Vec<String> = scored_results.iter().map(|r| r.entry.id.clone()).collect();
        let _ = store.touch_memories(&ids)?;
    }

    Ok(scored_results)
}

/// V2 search with explainable scoring, lexical + semantic channels, and intent-aware weights.
pub fn search_memories_v2(
    store: &ProjectMemoryStore,
    request: &MemorySearchRequestV2,
) -> AppResult<Vec<MemorySearchResultV2>> {
    let candidates = load_candidates_v2(store, request)?;

    let mut semantic_channel = semantic_channel_without_semantic(request.enable_lexical);
    let mut degraded = false;
    let mut semantic_scores = HashMap::new();

    if request.enable_semantic {
        if let Some(scores) =
            compute_tfidf_semantic_scores(store, &request.project_path, &request.query, &candidates)
        {
            semantic_channel = "tfidf".to_string();
            semantic_scores = scores;
        } else {
            degraded = true;
        }
    }

    Ok(rank_candidates_v2(
        candidates,
        request,
        &semantic_scores,
        &semantic_channel,
        degraded,
    ))
}

/// Async V2 search with provider-aware semantic channel.
///
/// Uses dense embedding when an `EmbeddingManager` with non-TF-IDF provider is
/// configured. Falls back to TF-IDF semantic search, then lexical-only mode if
/// both semantic channels are unavailable.
pub async fn search_memories_v2_async(
    store: &ProjectMemoryStore,
    request: &MemorySearchRequestV2,
) -> AppResult<Vec<MemorySearchResultV2>> {
    let candidates = load_candidates_v2(store, request)?;

    let mut semantic_channel = semantic_channel_without_semantic(request.enable_lexical);
    let mut degraded = false;
    let mut semantic_scores = HashMap::new();

    if request.enable_semantic {
        let dense_manager = store
            .get_embedding_manager()
            .filter(|mgr| mgr.provider_type() != EmbeddingProviderType::TfIdf);
        let dense_configured = dense_manager.is_some();

        if let Some(manager) = dense_manager {
            if let Some((scores, channel)) =
                compute_dense_semantic_scores(manager.as_ref(), &request.query, &candidates).await
            {
                semantic_channel = channel;
                semantic_scores = scores;
            }
        }

        if semantic_scores.is_empty() {
            if let Some(scores) = compute_tfidf_semantic_scores(
                store,
                &request.project_path,
                &request.query,
                &candidates,
            ) {
                semantic_channel = if dense_configured {
                    "tfidf_fallback".to_string()
                } else {
                    "tfidf".to_string()
                };
                semantic_scores = scores;
            } else {
                degraded = true;
            }
        }
    }

    Ok(rank_candidates_v2(
        candidates,
        request,
        &semantic_scores,
        &semantic_channel,
        degraded,
    ))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::memory::store::{NewMemoryEntry, ProjectMemoryStore};
    use crate::services::orchestrator::embedding_service::EmbeddingService;
    use crate::storage::database::Database;
    use std::sync::Arc;

    fn create_test_store() -> ProjectMemoryStore {
        let db = Database::new_in_memory().unwrap();
        let embedding_service = Arc::new(EmbeddingService::new());

        // Build vocabulary with representative project terms
        embedding_service.build_vocabulary(&[
            "use pnpm not npm for package management",
            "API routes return CommandResponse type pattern",
            "tests in __tests__ directories convention",
            "Tauri React Rust application framework",
            "error handling uses AppResult AppError",
            "database sqlite connection pooling r2d2",
            "frontend zustand state management store",
        ]);

        ProjectMemoryStore::from_database(&db, embedding_service)
    }

    // -----------------------------------------------------------------------
    // compute_relevance_score tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_compute_relevance_score_all_max() {
        let score = compute_relevance_score(1.0, 1.0, 1.0, 0.0);
        // 0.40*1.0 + 0.25*1.0 + 0.20*1.0 + 0.15*1.0 = 1.0
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_relevance_score_all_zero() {
        let score = compute_relevance_score(0.0, 0.0, 0.0, 1000.0);
        // 0.40*0 + 0.25*0 + 0.20*0 + 0.15*(1/(1+100)) ≈ 0.0015
        assert!(score < 0.01);
        assert!(score >= 0.0);
    }

    #[test]
    fn test_compute_relevance_score_embedding_dominant() {
        let score = compute_relevance_score(1.0, 0.0, 0.0, 0.0);
        // 0.40*1.0 + 0 + 0 + 0.15*1.0 = 0.55
        assert!((score - 0.55).abs() < 0.001);
    }

    #[test]
    fn test_compute_relevance_score_recency_decay() {
        let recent = compute_relevance_score(0.5, 0.5, 0.5, 0.0);
        let old = compute_relevance_score(0.5, 0.5, 0.5, 30.0);
        assert!(recent > old, "Recent memories should score higher");
    }

    // -----------------------------------------------------------------------
    // keyword_jaccard tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_keyword_jaccard_identical() {
        let a = vec!["foo".into(), "bar".into()];
        let b = vec!["foo".into(), "bar".into()];
        assert!((keyword_jaccard(&a, &b) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_keyword_jaccard_disjoint() {
        let a = vec!["foo".into()];
        let b = vec!["bar".into()];
        assert!((keyword_jaccard(&a, &b) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_keyword_jaccard_partial() {
        let a = vec!["foo".into(), "bar".into(), "baz".into()];
        let b = vec!["bar".into(), "baz".into(), "qux".into()];
        // intersection = {bar, baz} = 2, union = {foo, bar, baz, qux} = 4
        assert!((keyword_jaccard(&a, &b) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_keyword_jaccard_both_empty() {
        let a: Vec<String> = vec![];
        let b: Vec<String> = vec![];
        assert!((keyword_jaccard(&a, &b) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_keyword_jaccard_one_empty() {
        let a = vec!["foo".into()];
        let b: Vec<String> = vec![];
        assert!((keyword_jaccard(&a, &b) - 0.0).abs() < f32::EPSILON);
    }

    // -----------------------------------------------------------------------
    // extract_query_keywords tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_query_keywords() {
        let keywords = extract_query_keywords("What is the pnpm package manager?");
        assert!(keywords.contains(&"pnpm".to_string()));
        assert!(keywords.contains(&"package".to_string()));
        assert!(keywords.contains(&"manager".to_string()));
        // Common stopwords should be filtered.
        assert!(!keywords.contains(&"what".to_string()));
        assert!(!keywords.contains(&"the".to_string()));
        // "is" has only 2 chars and should also be filtered by length.
        assert!(!keywords.contains(&"is".to_string()));
    }

    #[test]
    fn test_extract_query_keywords_filters_stopwords() {
        let keywords = extract_query_keywords("Can you use the API for this task and explain it?");
        assert!(!keywords.contains(&"can".to_string()));
        assert!(!keywords.contains(&"use".to_string()));
        assert!(!keywords.contains(&"the".to_string()));
        assert!(!keywords.contains(&"for".to_string()));
        assert!(!keywords.contains(&"and".to_string()));
        assert!(keywords.contains(&"api".to_string()));
        assert!(keywords.contains(&"task".to_string()));
        assert!(keywords.contains(&"explain".to_string()));
    }

    // -----------------------------------------------------------------------
    // search_memories integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_search_returns_results() {
        let store = create_test_store();

        // Add some memories
        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Preference,
                content: "Always use pnpm not npm for package management".into(),
                keywords: vec!["pnpm".into(), "npm".into(), "package".into()],
                importance: 0.9,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Convention,
                content: "API routes return CommandResponse type".into(),
                keywords: vec!["api".into(), "routes".into(), "commandresponse".into()],
                importance: 0.7,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: "This is a Tauri React Rust application".into(),
                keywords: vec!["tauri".into(), "react".into(), "rust".into()],
                importance: 0.5,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        let results = search_memories(
            &store,
            &MemorySearchRequest {
                project_path: "/test/project".into(),
                query: "pnpm package management".into(),
                categories: None,
                top_k: 10,
                min_importance: 0.1,
            },
        )
        .unwrap();

        assert!(!results.is_empty());

        // The pnpm memory should rank highest due to keyword + embedding match
        assert_eq!(
            results[0].entry.content,
            "Always use pnpm not npm for package management"
        );
        assert!(results[0].relevance_score > 0.0);
    }

    #[test]
    fn test_search_with_category_filter() {
        let store = create_test_store();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Preference,
                content: "Use pnpm".into(),
                keywords: vec!["pnpm".into()],
                importance: 0.9,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: "Uses pnpm for builds".into(),
                keywords: vec!["pnpm".into()],
                importance: 0.5,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        // Search only preferences
        let results = search_memories(
            &store,
            &MemorySearchRequest {
                project_path: "/test/project".into(),
                query: "pnpm".into(),
                categories: Some(vec![MemoryCategory::Preference]),
                top_k: 10,
                min_importance: 0.1,
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.category, MemoryCategory::Preference);
    }

    #[test]
    fn test_search_respects_min_importance() {
        let store = create_test_store();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: "High importance fact".into(),
                keywords: vec!["fact".into()],
                importance: 0.8,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: "Low importance fact".into(),
                keywords: vec!["fact".into()],
                importance: 0.1,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        let results = search_memories(
            &store,
            &MemorySearchRequest {
                project_path: "/test/project".into(),
                query: "fact".into(),
                categories: None,
                top_k: 10,
                min_importance: 0.5,
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].entry.importance >= 0.5);
    }

    #[test]
    fn test_search_respects_top_k() {
        let store = create_test_store();

        for i in 0..5 {
            store
                .add_memory(NewMemoryEntry {
                    project_path: "/test/project".into(),
                    category: MemoryCategory::Fact,
                    content: format!("Fact number {}", i),
                    keywords: vec!["fact".into()],
                    importance: 0.5,
                    source_session_id: None,
                    source_context: None,
                })
                .unwrap();
        }

        let results = search_memories(
            &store,
            &MemorySearchRequest {
                project_path: "/test/project".into(),
                query: "fact".into(),
                categories: None,
                top_k: 3,
                min_importance: 0.1,
            },
        )
        .unwrap();

        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_is_read_only_by_default() {
        let store = create_test_store();

        let mem = store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: "Tauri React Rust application framework".into(),
                keywords: vec!["tauri".into()],
                importance: 0.5,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        assert_eq!(mem.access_count, 0);

        // Search that returns this memory
        let _results = search_memories(
            &store,
            &MemorySearchRequest {
                project_path: "/test/project".into(),
                query: "tauri application".into(),
                categories: None,
                top_k: 10,
                min_importance: 0.1,
            },
        )
        .unwrap();

        // Default search should be read-only
        let updated = store.get_memory(&mem.id).unwrap().unwrap();
        assert_eq!(updated.access_count, 0);
    }

    #[test]
    fn test_search_touch_policy_updates_access_count() {
        let store = create_test_store();

        let mem = store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: "Tauri React Rust application framework".into(),
                keywords: vec!["tauri".into()],
                importance: 0.5,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        let _results = search_memories_with_options(
            &store,
            &MemorySearchRequest {
                project_path: "/test/project".into(),
                query: "tauri application".into(),
                categories: None,
                top_k: 10,
                min_importance: 0.1,
            },
            MemorySearchOptions {
                ranking_mode: MemoryRankingMode::Hybrid,
                touch_policy: MemoryTouchPolicy::TouchReturned,
            },
        )
        .unwrap();

        let updated = store.get_memory(&mem.id).unwrap().unwrap();
        assert_eq!(updated.access_count, 1);
    }

    #[test]
    fn test_search_empty_project() {
        let store = create_test_store();

        let results = search_memories(
            &store,
            &MemorySearchRequest {
                project_path: "/nonexistent/project".into(),
                query: "anything".into(),
                categories: None,
                top_k: 10,
                min_importance: 0.1,
            },
        )
        .unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn test_search_results_sorted_by_score() {
        let store = create_test_store();

        // Add memories with varying relevance
        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: "database sqlite connection pooling r2d2".into(),
                keywords: vec!["database".into(), "sqlite".into()],
                importance: 0.3,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Preference,
                content: "use pnpm not npm for package management".into(),
                keywords: vec!["pnpm".into(), "npm".into(), "package".into()],
                importance: 0.9,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        let results = search_memories(
            &store,
            &MemorySearchRequest {
                project_path: "/test/project".into(),
                query: "pnpm package".into(),
                categories: None,
                top_k: 10,
                min_importance: 0.1,
            },
        )
        .unwrap();

        // Results should be sorted by relevance_score descending
        for i in 1..results.len() {
            assert!(results[i - 1].relevance_score >= results[i].relevance_score);
        }
    }

    #[test]
    fn test_search_memories_v2_returns_breakdown() {
        let store = create_test_store();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Preference,
                content: "Always use pnpm not npm for package management".into(),
                keywords: vec!["pnpm".into(), "package".into()],
                importance: 0.9,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        let results = search_memories_v2(
            &store,
            &MemorySearchRequestV2 {
                project_path: "/test/project".into(),
                query: "pnpm package manager".into(),
                categories: None,
                top_k: 5,
                min_importance: 0.1,
                intent: MemorySearchIntent::Bugfix,
                enable_semantic: true,
                enable_lexical: true,
            },
        )
        .unwrap();

        assert!(!results.is_empty());
        assert!(results[0].score_breakdown.final_score > 0.0);
        assert_eq!(results[0].semantic_channel, "tfidf");
    }

    #[test]
    fn test_search_memories_v2_lexical_only_mode() {
        let store = create_test_store();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: "Unit tests are located under __tests__ directory".into(),
                keywords: vec!["tests".into(), "__tests__".into()],
                importance: 0.6,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        let results = search_memories_v2(
            &store,
            &MemorySearchRequestV2 {
                project_path: "/test/project".into(),
                query: "__tests__ directory".into(),
                categories: None,
                top_k: 3,
                min_importance: 0.1,
                intent: MemorySearchIntent::Docs,
                enable_semantic: false,
                enable_lexical: true,
            },
        )
        .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].semantic_channel, "lexical_only");
    }

    #[tokio::test]
    async fn test_search_memories_v2_async_uses_tfidf_when_no_dense_provider() {
        let store = create_test_store();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Preference,
                content: "Prefer pnpm for workspace dependency management".into(),
                keywords: vec!["pnpm".into(), "workspace".into()],
                importance: 0.8,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        let results = search_memories_v2_async(
            &store,
            &MemorySearchRequestV2 {
                project_path: "/test/project".into(),
                query: "workspace package manager".into(),
                categories: None,
                top_k: 5,
                min_importance: 0.1,
                intent: MemorySearchIntent::Default,
                enable_semantic: true,
                enable_lexical: true,
            },
        )
        .await
        .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].semantic_channel, "tfidf");
        assert!(!results[0].degraded);
    }

    #[tokio::test]
    async fn test_search_memories_v2_async_marks_lexical_only_when_semantic_unavailable() {
        let store = create_test_store();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: String::new(),
                keywords: vec![],
                importance: 0.4,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        let results = search_memories_v2_async(
            &store,
            &MemorySearchRequestV2 {
                project_path: "/test/project".into(),
                query: "any semantic query".into(),
                categories: None,
                top_k: 3,
                min_importance: 0.1,
                intent: MemorySearchIntent::Default,
                enable_semantic: true,
                enable_lexical: true,
            },
        )
        .await
        .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].semantic_channel, "lexical_only");
        assert!(results[0].degraded);
    }
}
