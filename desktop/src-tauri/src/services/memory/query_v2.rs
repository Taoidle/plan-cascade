//! Memory Query Orchestrator V2
//!
//! Unified multi-scope query/read/review helpers over `memory_entries_v2`.
//! This module is intentionally backend-centric so all callers (commands/hooks/
//! context providers) can share one retrieval path.

use std::collections::{HashMap, HashSet};

use chrono::{NaiveDateTime, Utc};
use rusqlite::{params, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};

use crate::services::memory::query_policy_v2::{
    DEFAULT_MIN_IMPORTANCE_V2, DEFAULT_PER_SCOPE_BUDGET_V2, DEFAULT_TOP_K_TOTAL_V2,
};
use crate::services::memory::retrieval::{
    extract_query_keywords, keyword_jaccard, MemoryScoreBreakdown, MemorySearchIntent,
    MemorySearchResultV2,
};
use crate::services::memory::store::{
    build_session_project_path, bytes_to_embedding, MemoryCategory, MemoryEntry, MemoryStats,
    ProjectMemoryStore, GLOBAL_PROJECT_PATH,
};
use crate::services::orchestrator::embedding_service::cosine_similarity;
use crate::utils::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScopeV2 {
    Project,
    Global,
    Session,
}

impl MemoryScopeV2 {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryScopeV2::Project => "project",
            MemoryScopeV2::Global => "global",
            MemoryScopeV2::Session => "session",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "project" => Some(Self::Project),
            "global" => Some(Self::Global),
            "session" => Some(Self::Session),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatusV2 {
    Active,
    PendingReview,
    Rejected,
    Archived,
}

impl MemoryStatusV2 {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryStatusV2::Active => "active",
            MemoryStatusV2::PendingReview => "pending_review",
            MemoryStatusV2::Rejected => "rejected",
            MemoryStatusV2::Archived => "archived",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "active" => Some(Self::Active),
            "pending_review" => Some(Self::PendingReview),
            "rejected" => Some(Self::Rejected),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTierV2 {
    Low,
    Medium,
    High,
}

impl RiskTierV2 {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedMemoryQueryRequestV2 {
    pub project_path: String,
    pub query: String,
    #[serde(default)]
    pub scopes: Vec<MemoryScopeV2>,
    #[serde(default)]
    pub categories: Vec<MemoryCategory>,
    #[serde(default)]
    pub include_ids: Vec<String>,
    #[serde(default)]
    pub exclude_ids: Vec<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default = "default_top_k_total")]
    pub top_k_total: usize,
    #[serde(default = "default_min_importance")]
    pub min_importance: f32,
    #[serde(default = "default_per_scope_budget")]
    pub per_scope_budget: usize,
    #[serde(default)]
    pub intent: MemorySearchIntent,
    #[serde(default = "default_true")]
    pub enable_semantic: bool,
    #[serde(default = "default_true")]
    pub enable_lexical: bool,
    #[serde(default)]
    pub statuses: Vec<MemoryStatusV2>,
}

fn default_top_k_total() -> usize {
    DEFAULT_TOP_K_TOTAL_V2
}

fn default_min_importance() -> f32 {
    DEFAULT_MIN_IMPORTANCE_V2
}

fn default_per_scope_budget() -> usize {
    DEFAULT_PER_SCOPE_BUDGET_V2
}

fn default_true() -> bool {
    true
}

impl Default for UnifiedMemoryQueryRequestV2 {
    fn default() -> Self {
        Self {
            project_path: String::new(),
            query: String::new(),
            scopes: vec![MemoryScopeV2::Project, MemoryScopeV2::Global],
            categories: vec![],
            include_ids: vec![],
            exclude_ids: vec![],
            session_id: None,
            top_k_total: default_top_k_total(),
            min_importance: default_min_importance(),
            per_scope_budget: default_per_scope_budget(),
            intent: MemorySearchIntent::Default,
            enable_semantic: true,
            enable_lexical: true,
            statuses: vec![MemoryStatusV2::Active],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedMemoryQueryResultV2 {
    pub trace_id: String,
    pub degraded: bool,
    pub candidate_count: usize,
    pub results: Vec<MemorySearchResultV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReviewCandidateV2 {
    pub id: String,
    pub scope: MemoryScopeV2,
    pub project_path: Option<String>,
    pub session_id: Option<String>,
    pub category: MemoryCategory,
    pub content: String,
    pub keywords: Vec<String>,
    pub importance: f32,
    pub source_session_id: Option<String>,
    pub source_context: Option<String>,
    pub status: MemoryStatusV2,
    pub risk_tier: RiskTierV2,
    pub conflict_flag: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryReviewDecisionV2 {
    Approve,
    Reject,
    Archive,
}

impl MemoryReviewDecisionV2 {
    fn target_status(&self) -> &'static str {
        match self {
            MemoryReviewDecisionV2::Approve => "active",
            MemoryReviewDecisionV2::Reject => "rejected",
            MemoryReviewDecisionV2::Archive => "archived",
        }
    }

    fn audit_label(&self) -> &'static str {
        match self {
            MemoryReviewDecisionV2::Approve => "approve",
            MemoryReviewDecisionV2::Reject => "reject",
            MemoryReviewDecisionV2::Archive => "archive",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReviewSummaryV2 {
    pub updated: usize,
}

#[derive(Debug, Clone)]
struct CandidateRow {
    entry: MemoryEntry,
    keywords: Vec<String>,
    embedding: Option<Vec<f32>>,
    last_accessed_at: String,
}

fn normalize_scopes(scopes: &[MemoryScopeV2], session_id: Option<&str>) -> Vec<MemoryScopeV2> {
    let mut dedup = HashSet::new();
    let mut out = Vec::new();
    for scope in scopes {
        if *scope == MemoryScopeV2::Session && session_id.unwrap_or("").trim().is_empty() {
            continue;
        }
        if dedup.insert(*scope) {
            out.push(*scope);
        }
    }
    if out.is_empty() {
        out.push(MemoryScopeV2::Project);
        out.push(MemoryScopeV2::Global);
        if !session_id.unwrap_or("").trim().is_empty() {
            out.push(MemoryScopeV2::Session);
        }
    }
    out
}

fn normalize_statuses(statuses: &[MemoryStatusV2]) -> Vec<MemoryStatusV2> {
    if statuses.is_empty() {
        return vec![MemoryStatusV2::Active];
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for status in statuses {
        if seen.insert(*status) {
            out.push(*status);
        }
    }
    out
}

fn scope_legacy_project_path(
    scope: MemoryScopeV2,
    project_path: Option<&str>,
    session_id: Option<&str>,
) -> String {
    match scope {
        MemoryScopeV2::Project => project_path.unwrap_or_default().to_string(),
        MemoryScopeV2::Global => GLOBAL_PROJECT_PATH.to_string(),
        MemoryScopeV2::Session => session_id
            .and_then(build_session_project_path)
            .unwrap_or_default(),
    }
}

fn scope_session_for_entry(
    scope: MemoryScopeV2,
    session_id: Option<&str>,
) -> (Option<String>, Option<String>) {
    match scope {
        MemoryScopeV2::Project => (Some("project".to_string()), None),
        MemoryScopeV2::Global => (Some("global".to_string()), None),
        MemoryScopeV2::Session => (
            Some("session".to_string()),
            session_id.map(|s| s.to_string()),
        ),
    }
}

fn build_category_filter(categories: &[MemoryCategory], params: &mut Vec<Value>) -> String {
    if categories.is_empty() {
        return String::new();
    }
    let placeholders = (0..categories.len())
        .map(|_| "?".to_string())
        .collect::<Vec<_>>()
        .join(", ");
    for category in categories {
        params.push(Value::Text(category.as_str().to_string()));
    }
    format!(" AND category IN ({})", placeholders)
}

fn build_status_filter(statuses: &[MemoryStatusV2], params: &mut Vec<Value>) -> String {
    if statuses.is_empty() {
        return String::new();
    }
    let placeholders = (0..statuses.len())
        .map(|_| "?".to_string())
        .collect::<Vec<_>>()
        .join(", ");
    for status in statuses {
        params.push(Value::Text(status.as_str().to_string()));
    }
    format!(" AND status IN ({})", placeholders)
}

fn build_scope_filter(
    scopes: &[MemoryScopeV2],
    project_path: &str,
    session_id: Option<&str>,
    params: &mut Vec<Value>,
) -> String {
    if scopes.is_empty() {
        return String::new();
    }
    let mut clauses = Vec::new();
    for scope in scopes {
        match scope {
            MemoryScopeV2::Project => {
                clauses.push("(scope = 'project' AND project_path = ?)".to_string());
                params.push(Value::Text(project_path.to_string()));
            }
            MemoryScopeV2::Global => {
                clauses.push("(scope = 'global')".to_string());
            }
            MemoryScopeV2::Session => {
                let sid = session_id.unwrap_or("").trim();
                if sid.is_empty() {
                    continue;
                }
                clauses.push("(scope = 'session' AND session_id = ?)".to_string());
                params.push(Value::Text(sid.to_string()));
            }
        }
    }
    if clauses.is_empty() {
        String::new()
    } else {
        format!(" AND ({})", clauses.join(" OR "))
    }
}

fn candidate_limit(top_k_total: usize) -> usize {
    top_k_total.max(1).saturating_mul(8).clamp(40, 400)
}

fn apply_per_scope_budget(
    candidates: Vec<CandidateRow>,
    per_scope_budget: usize,
) -> Vec<CandidateRow> {
    if per_scope_budget == 0 {
        return candidates;
    }

    let mut per_scope_counts: HashMap<String, usize> = HashMap::new();
    let mut filtered = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let scope_key = candidate
            .entry
            .scope
            .clone()
            .unwrap_or_else(|| "project".to_string());
        let count = per_scope_counts.entry(scope_key).or_insert(0);
        if *count >= per_scope_budget {
            continue;
        }
        *count += 1;
        filtered.push(candidate);
    }
    filtered
}

fn days_since(datetime_str: &str) -> f64 {
    if let Ok(dt) = NaiveDateTime::parse_from_str(datetime_str, "%Y-%m-%d %H:%M:%S") {
        let now = Utc::now().naive_utc();
        let duration = now - dt;
        duration.num_seconds().max(0) as f64 / 86_400.0
    } else {
        30.0
    }
}

fn recency_score(last_accessed_at: &str) -> f32 {
    1.0 / (1.0 + days_since(last_accessed_at) as f32 * 0.1)
}

fn load_candidates(
    store: &ProjectMemoryStore,
    request: &UnifiedMemoryQueryRequestV2,
) -> AppResult<Vec<CandidateRow>> {
    let conn = store.pool().get().map_err(|e| {
        AppError::database(format!(
            "Failed to get connection for unified memory query: {}",
            e
        ))
    })?;

    let scopes = normalize_scopes(&request.scopes, request.session_id.as_deref());
    let statuses = normalize_statuses(&request.statuses);
    let mut params: Vec<Value> = vec![Value::Real(request.min_importance as f64)];

    let category_filter = build_category_filter(&request.categories, &mut params);
    let status_filter = build_status_filter(&statuses, &mut params);
    let scope_filter = build_scope_filter(
        &scopes,
        request.project_path.trim(),
        request.session_id.as_deref(),
        &mut params,
    );

    let sql = format!(
        "SELECT id, scope, project_path, session_id, category, content, keywords, importance,
                access_count, source_session_id, source_context, created_at, updated_at,
                last_accessed_at, status, risk_tier, conflict_flag, embedding
         FROM memory_entries_v2
         WHERE importance >= ?{}{}{}
         ORDER BY importance DESC, updated_at DESC
         LIMIT {}",
        category_filter,
        status_filter,
        scope_filter,
        candidate_limit(request.top_k_total)
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
        let scope_str: String = row.get(1)?;
        let project_path: Option<String> = row.get(2)?;
        let session_id: Option<String> = row.get(3)?;
        let category_str: String = row.get(4)?;
        let keywords_json: String = row.get(6)?;
        let status: String = row.get(14)?;
        let risk_tier: String = row.get(15)?;
        let conflict_flag = row.get::<_, i64>(16).unwrap_or(0) != 0;
        let keywords: Vec<String> = serde_json::from_str(&keywords_json).unwrap_or_default();
        let parsed_scope = MemoryScopeV2::from_str(&scope_str).unwrap_or(MemoryScopeV2::Project);
        let legacy_project_path =
            scope_legacy_project_path(parsed_scope, project_path.as_deref(), session_id.as_deref());
        let (scope, v2_session_id) = scope_session_for_entry(parsed_scope, session_id.as_deref());
        let embedding = row
            .get::<_, Option<Vec<u8>>>(17)?
            .map(|bytes| bytes_to_embedding(&bytes));
        Ok(CandidateRow {
            entry: MemoryEntry {
                id: row.get(0)?,
                project_path: legacy_project_path,
                scope,
                session_id: v2_session_id,
                category: MemoryCategory::from_str(&category_str).unwrap_or(MemoryCategory::Fact),
                content: row.get(5)?,
                keywords: keywords.clone(),
                importance: row.get(7)?,
                access_count: row.get(8)?,
                source_session_id: row.get(9)?,
                source_context: row.get(10)?,
                status: Some(status),
                risk_tier: Some(risk_tier),
                conflict_flag: Some(conflict_flag),
                trace_id: None,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
                last_accessed_at: row.get(13)?,
            },
            keywords,
            embedding,
            last_accessed_at: row.get(13)?,
        })
    })?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn include_ids_as_results(
    store: &ProjectMemoryStore,
    include_ids: &[String],
    exclude_ids: &HashSet<&str>,
    statuses: &[MemoryStatusV2],
) -> AppResult<Vec<MemorySearchResultV2>> {
    if include_ids.is_empty() {
        return Ok(vec![]);
    }
    let conn = store.pool().get().map_err(|e| {
        AppError::database(format!(
            "Failed to get connection for include-id memory query: {}",
            e
        ))
    })?;
    let status_set: HashSet<&str> = statuses.iter().map(|s| s.as_str()).collect();
    let mut out = Vec::new();
    for id in include_ids {
        if exclude_ids.contains(id.as_str()) {
            continue;
        }
        let row = conn.query_row(
            "SELECT id, scope, project_path, session_id, category, content, keywords, importance,
                    access_count, source_session_id, source_context, created_at, updated_at,
                    last_accessed_at, status, risk_tier, conflict_flag
             FROM memory_entries_v2 WHERE id = ?1",
            params![id],
            |row| {
                let scope_str: String = row.get(1)?;
                let project_path: Option<String> = row.get(2)?;
                let session_id: Option<String> = row.get(3)?;
                let category_str: String = row.get(4)?;
                let keywords_json: String = row.get(6)?;
                let keywords: Vec<String> =
                    serde_json::from_str(&keywords_json).unwrap_or_default();
                let parsed_scope =
                    MemoryScopeV2::from_str(&scope_str).unwrap_or(MemoryScopeV2::Project);
                let legacy_project_path = scope_legacy_project_path(
                    parsed_scope,
                    project_path.as_deref(),
                    session_id.as_deref(),
                );
                let (scope, v2_session_id) =
                    scope_session_for_entry(parsed_scope, session_id.as_deref());
                Ok((
                    MemoryEntry {
                        id: row.get(0)?,
                        project_path: legacy_project_path,
                        scope,
                        session_id: v2_session_id,
                        category: MemoryCategory::from_str(&category_str)
                            .unwrap_or(MemoryCategory::Fact),
                        content: row.get(5)?,
                        keywords,
                        importance: row.get(7)?,
                        access_count: row.get(8)?,
                        source_session_id: row.get(9)?,
                        source_context: row.get(10)?,
                        status: Some(row.get(14)?),
                        risk_tier: Some(row.get(15)?),
                        conflict_flag: Some(row.get::<_, i64>(16).unwrap_or(0) != 0),
                        trace_id: None,
                        created_at: row.get(11)?,
                        updated_at: row.get(12)?,
                        last_accessed_at: row.get(13)?,
                    },
                    row.get::<_, String>(14)?,
                ))
            },
        );
        if let Ok((entry, status)) = row {
            if !status_set.is_empty() && !status_set.contains(status.as_str()) {
                continue;
            }
            out.push(MemorySearchResultV2 {
                entry,
                relevance_score: 1.1,
                score_breakdown: MemoryScoreBreakdown {
                    final_score: 1.1,
                    ..Default::default()
                },
                semantic_channel: "manual_select".to_string(),
                degraded: false,
            });
        }
    }
    Ok(out)
}

fn intent_weights(intent: MemorySearchIntent) -> (f32, f32, f32, f32) {
    match intent {
        MemorySearchIntent::Bugfix => (0.35, 0.30, 0.25, 0.10),
        MemorySearchIntent::Refactor => (0.32, 0.28, 0.28, 0.12),
        MemorySearchIntent::Qa => (0.28, 0.34, 0.24, 0.14),
        MemorySearchIntent::Docs => (0.24, 0.36, 0.24, 0.16),
        MemorySearchIntent::Default => (0.30, 0.30, 0.25, 0.15),
    }
}

pub async fn query_memory_entries_v2(
    store: &ProjectMemoryStore,
    request: &UnifiedMemoryQueryRequestV2,
) -> AppResult<UnifiedMemoryQueryResultV2> {
    let trace_id = uuid::Uuid::new_v4().to_string();
    let exclude_ids: HashSet<&str> = request.exclude_ids.iter().map(|id| id.as_str()).collect();
    let statuses = normalize_statuses(&request.statuses);
    let mut merged: HashMap<String, MemorySearchResultV2> = HashMap::new();

    for row in include_ids_as_results(store, &request.include_ids, &exclude_ids, &statuses)? {
        merged.insert(row.entry.id.clone(), row);
    }

    let loaded_candidates = load_candidates(store, request)?;
    let candidates = apply_per_scope_budget(loaded_candidates, request.per_scope_budget);
    let candidate_count = candidates.len() + merged.len();
    let query = request.query.trim();
    let query_keywords = extract_query_keywords(query);
    let mut degraded = false;
    let mut semantic_scores: HashMap<String, f32> = HashMap::new();
    let mut semantic_channel = if request.enable_lexical {
        "lexical_only".to_string()
    } else {
        "disabled".to_string()
    };

    if !query.is_empty() && request.enable_semantic {
        let query_embedding = store.embedding_service().embed_text(query);
        if query_embedding.is_empty() {
            degraded = true;
        } else {
            let mut semantic_hits = 0usize;
            for candidate in &candidates {
                if let Some(embedding) = &candidate.embedding {
                    if embedding.is_empty() {
                        continue;
                    }
                    let sim = cosine_similarity(&query_embedding, embedding);
                    semantic_scores.insert(candidate.entry.id.clone(), sim);
                    semantic_hits += 1;
                }
            }
            if semantic_hits == 0 {
                degraded = true;
            } else {
                semantic_channel = "tfidf".to_string();
            }
        }
    }

    let (semantic_w, lexical_w, importance_w, recency_w) = intent_weights(request.intent);

    for candidate in candidates {
        if exclude_ids.contains(candidate.entry.id.as_str()) {
            continue;
        }
        let recency = recency_score(&candidate.last_accessed_at);
        let relevance_score = if query.is_empty() {
            0.80 * candidate.entry.importance + 0.20 * recency
        } else {
            let semantic = if request.enable_semantic {
                *semantic_scores.get(&candidate.entry.id).unwrap_or(&0.0)
            } else {
                0.0
            };
            let lexical = if request.enable_lexical {
                let kw_overlap = keyword_jaccard(&query_keywords, &candidate.keywords);
                if query_keywords.is_empty() {
                    kw_overlap
                } else {
                    let content_lower = candidate.entry.content.to_ascii_lowercase();
                    let hit_ratio = query_keywords
                        .iter()
                        .filter(|kw| content_lower.contains(kw.as_str()))
                        .count() as f32
                        / query_keywords.len() as f32;
                    kw_overlap.max(hit_ratio)
                }
            } else {
                0.0
            };
            semantic_w * semantic
                + lexical_w * lexical
                + importance_w * candidate.entry.importance
                + recency_w * recency
        };
        let entry_id = candidate.entry.id.clone();
        let entry_importance = candidate.entry.importance;
        let lexical_overlap = if request.enable_lexical {
            keyword_jaccard(&query_keywords, &candidate.keywords)
        } else {
            0.0
        };
        let semantic_similarity = *semantic_scores.get(&entry_id).unwrap_or(&0.0);
        let result = MemorySearchResultV2 {
            entry: candidate.entry,
            relevance_score,
            score_breakdown: MemoryScoreBreakdown {
                semantic_similarity,
                lexical_overlap,
                keyword_overlap: keyword_jaccard(&query_keywords, &candidate.keywords),
                importance: entry_importance,
                recency_score: recency,
                semantic_weight: semantic_w,
                lexical_weight: lexical_w,
                importance_weight: importance_w,
                recency_weight: recency_w,
                final_score: relevance_score,
                ..Default::default()
            },
            semantic_channel: semantic_channel.clone(),
            degraded,
        };
        match merged.get(&result.entry.id) {
            Some(existing) if existing.relevance_score >= result.relevance_score => {}
            _ => {
                merged.insert(result.entry.id.clone(), result);
            }
        }
    }

    let mut results: Vec<MemorySearchResultV2> = merged.into_values().collect();
    for result in &mut results {
        result.entry.trace_id = Some(trace_id.clone());
    }
    results.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if results.len() > request.top_k_total {
        results.truncate(request.top_k_total);
    }

    Ok(UnifiedMemoryQueryResultV2 {
        trace_id,
        degraded,
        candidate_count,
        results,
    })
}

pub async fn list_memory_entries_v2(
    store: &ProjectMemoryStore,
    mut request: UnifiedMemoryQueryRequestV2,
) -> AppResult<Vec<MemoryEntry>> {
    request.query = String::new();
    request.enable_semantic = false;
    request.enable_lexical = false;
    let queried = query_memory_entries_v2(store, &request).await?;
    Ok(queried.results.into_iter().map(|r| r.entry).collect())
}

pub fn memory_stats_v2(
    store: &ProjectMemoryStore,
    request: &UnifiedMemoryQueryRequestV2,
) -> AppResult<MemoryStats> {
    let conn = store.pool().get().map_err(|e| {
        AppError::database(format!(
            "Failed to get connection for memory_stats_v2: {}",
            e
        ))
    })?;
    let scopes = normalize_scopes(&request.scopes, request.session_id.as_deref());
    let statuses = normalize_statuses(&request.statuses);
    let mut params: Vec<Value> = vec![];
    let status_filter = build_status_filter(&statuses, &mut params);
    let scope_filter = build_scope_filter(
        &scopes,
        request.project_path.trim(),
        request.session_id.as_deref(),
        &mut params,
    );
    let category_filter = build_category_filter(&request.categories, &mut params);
    let where_sql = format!(
        " WHERE 1=1{}{}{}",
        status_filter, scope_filter, category_filter
    );

    let total_sql = format!("SELECT COUNT(*) FROM memory_entries_v2{}", where_sql);
    let total_count: i64 = conn.query_row(&total_sql, params_from_iter(params.iter()), |row| {
        row.get(0)
    })?;

    let avg_params = params.clone();
    let avg_sql = format!(
        "SELECT COALESCE(AVG(importance), 0.0) FROM memory_entries_v2{}",
        where_sql
    );
    let avg_importance: f64 =
        conn.query_row(&avg_sql, params_from_iter(avg_params.iter()), |row| {
            row.get(0)
        })?;

    let cat_params = params.clone();
    let cat_sql = format!(
        "SELECT category, COUNT(*) FROM memory_entries_v2{} GROUP BY category",
        where_sql
    );
    let mut stmt = conn.prepare(&cat_sql)?;
    let category_counts: HashMap<String, usize> = stmt
        .query_map(params_from_iter(cat_params.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(MemoryStats {
        total_count: total_count as usize,
        category_counts,
        avg_importance: avg_importance as f32,
    })
}

pub fn list_pending_memory_candidates_v2(
    store: &ProjectMemoryStore,
    project_path: &str,
    session_id: Option<&str>,
    scopes: &[MemoryScopeV2],
    limit: usize,
) -> AppResult<Vec<MemoryReviewCandidateV2>> {
    let conn = store.pool().get().map_err(|e| {
        AppError::database(format!(
            "Failed to get connection for list_pending_memory_candidates_v2: {}",
            e
        ))
    })?;
    let normalized_scopes = normalize_scopes(scopes, session_id);
    let mut params: Vec<Value> = vec![Value::Text("pending_review".to_string())];
    let scope_filter = build_scope_filter(
        &normalized_scopes,
        project_path.trim(),
        session_id,
        &mut params,
    );
    let sql = format!(
        "SELECT id, scope, project_path, session_id, category, content, keywords, importance,
                source_session_id, source_context, status, risk_tier, conflict_flag, created_at, updated_at
         FROM memory_entries_v2
         WHERE status = ?{}
         ORDER BY updated_at DESC
         LIMIT {}",
        scope_filter,
        limit.max(1)
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
        let category_str: String = row.get(4)?;
        let keywords_json: String = row.get(6)?;
        let scope_str: String = row.get(1)?;
        let status_str: String = row.get(10)?;
        let risk_tier_str: String = row.get(11)?;
        Ok(MemoryReviewCandidateV2 {
            id: row.get(0)?,
            scope: MemoryScopeV2::from_str(&scope_str).unwrap_or(MemoryScopeV2::Project),
            project_path: row.get(2)?,
            session_id: row.get(3)?,
            category: MemoryCategory::from_str(&category_str).unwrap_or(MemoryCategory::Fact),
            content: row.get(5)?,
            keywords: serde_json::from_str(&keywords_json).unwrap_or_default(),
            importance: row.get(7)?,
            source_session_id: row.get(8)?,
            source_context: row.get(9)?,
            status: MemoryStatusV2::from_str(&status_str).unwrap_or(MemoryStatusV2::PendingReview),
            risk_tier: RiskTierV2::from_str(&risk_tier_str).unwrap_or(RiskTierV2::Medium),
            conflict_flag: row.get::<_, i64>(12).unwrap_or(0) != 0,
            created_at: row.get(13)?,
            updated_at: row.get(14)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn review_memory_candidates_v2(
    store: &ProjectMemoryStore,
    ids: &[String],
    decision: MemoryReviewDecisionV2,
) -> AppResult<MemoryReviewSummaryV2> {
    if ids.is_empty() {
        return Ok(MemoryReviewSummaryV2 { updated: 0 });
    }
    let conn = store.pool().get().map_err(|e| {
        AppError::database(format!(
            "Failed to get connection for review_memory_candidates_v2: {}",
            e
        ))
    })?;
    let tx = conn.unchecked_transaction()?;
    let mut updated = 0usize;
    for id in ids {
        updated += tx.execute(
            "UPDATE memory_entries_v2
             SET status = ?2,
                 conflict_flag = CASE WHEN ?2 = 'pending_review' THEN conflict_flag ELSE 0 END,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id, decision.target_status()],
        )?;
        let _ = tx.execute(
            "INSERT INTO memory_review_audit_v2 (memory_id, decision, operator, created_at)
             VALUES (?1, ?2, 'user', datetime('now'))",
            params![id, decision.audit_label()],
        );
    }
    tx.commit()?;
    Ok(MemoryReviewSummaryV2 { updated })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    use crate::services::orchestrator::embedding_service::EmbeddingService;
    use crate::storage::database::Database;

    fn create_store() -> ProjectMemoryStore {
        let db = Database::new_in_memory().unwrap();
        let embedding_service = Arc::new(EmbeddingService::new());
        embedding_service.build_vocabulary(&["memory", "project", "session", "global"]);
        ProjectMemoryStore::from_database(&db, embedding_service)
    }

    #[tokio::test]
    async fn test_query_memory_entries_v2_respects_per_scope_budget() {
        let store = create_store();
        let conn = store.pool().get().unwrap();
        for idx in 0..5 {
            conn.execute(
                "INSERT INTO memory_entries_v2 (id, scope, project_path, category, content, content_hash, status, importance)
                 VALUES (?1, 'project', '/scope-test', 'fact', ?2, lower(trim(?2)), 'active', 0.8)",
                params![format!("project-{}", idx), format!("project memory {}", idx)],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO memory_entries_v2 (id, scope, category, content, content_hash, status, importance)
                 VALUES (?1, 'global', 'fact', ?2, lower(trim(?2)), 'active', 0.8)",
                params![format!("global-{}", idx), format!("global memory {}", idx)],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO memory_entries_v2 (id, scope, session_id, category, content, content_hash, status, importance)
                 VALUES (?1, 'session', 'sess-a', 'fact', ?2, lower(trim(?2)), 'active', 0.8)",
                params![format!("session-{}", idx), format!("session memory {}", idx)],
            )
            .unwrap();
        }
        drop(conn);

        let request = UnifiedMemoryQueryRequestV2 {
            project_path: "/scope-test".to_string(),
            query: String::new(),
            scopes: vec![
                MemoryScopeV2::Project,
                MemoryScopeV2::Global,
                MemoryScopeV2::Session,
            ],
            categories: vec![],
            include_ids: vec![],
            exclude_ids: vec![],
            session_id: Some("sess-a".to_string()),
            top_k_total: 20,
            min_importance: 0.0,
            per_scope_budget: 1,
            intent: MemorySearchIntent::Default,
            enable_semantic: false,
            enable_lexical: false,
            statuses: vec![MemoryStatusV2::Active],
        };

        let result = query_memory_entries_v2(&store, &request).await.unwrap();
        assert_eq!(result.results.len(), 3);
    }

    #[tokio::test]
    #[ignore = "performance release gate"]
    async fn test_memory_v2_query_p95_gate() {
        let store = create_store();
        let conn = store.pool().get().unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        for idx in 0..10_000 {
            let content = format!("memory entry {} for performance gate", idx);
            tx.execute(
                "INSERT INTO memory_entries_v2 (id, scope, project_path, category, content, content_hash, status, importance)
                 VALUES (?1, 'project', '/perf', 'fact', ?2, lower(trim(?2)), 'active', 0.5)",
                params![format!("perf-{}", idx), content],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        drop(conn);

        let stats_request = UnifiedMemoryQueryRequestV2 {
            project_path: "/perf".to_string(),
            query: String::new(),
            scopes: vec![MemoryScopeV2::Project],
            categories: vec![],
            include_ids: vec![],
            exclude_ids: vec![],
            session_id: None,
            top_k_total: 20,
            min_importance: 0.0,
            per_scope_budget: 200,
            intent: MemorySearchIntent::Default,
            enable_semantic: false,
            enable_lexical: false,
            statuses: vec![MemoryStatusV2::Active],
        };

        let stats_start = Instant::now();
        let stats = memory_stats_v2(&store, &stats_request).unwrap();
        let stats_ms = stats_start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(stats.total_count, 10_000);

        let query_request = UnifiedMemoryQueryRequestV2 {
            project_path: "/perf".to_string(),
            query: "performance gate memory".to_string(),
            scopes: vec![MemoryScopeV2::Project],
            categories: vec![],
            include_ids: vec![],
            exclude_ids: vec![],
            session_id: None,
            top_k_total: 20,
            min_importance: 0.0,
            per_scope_budget: 200,
            intent: MemorySearchIntent::Default,
            enable_semantic: false,
            enable_lexical: true,
            statuses: vec![MemoryStatusV2::Active],
        };

        let mut latencies = Vec::new();
        for _ in 0..30 {
            let started = Instant::now();
            let result = query_memory_entries_v2(&store, &query_request)
                .await
                .unwrap();
            assert!(!result.results.is_empty());
            latencies.push(started.elapsed().as_secs_f64() * 1000.0);
        }

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p95_idx = ((latencies.len() as f64 * 0.95).ceil() as usize)
            .saturating_sub(1)
            .min(latencies.len().saturating_sub(1));
        let query_p95_ms = latencies[p95_idx];

        let query_threshold_ms = std::env::var("MEMORY_V2_QUERY_P95_MS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(300.0);
        let stats_threshold_ms = std::env::var("MEMORY_V2_STATS_P95_MS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(300.0);

        assert!(
            query_p95_ms <= query_threshold_ms,
            "query p95 {}ms exceeded threshold {}ms",
            query_p95_ms,
            query_threshold_ms
        );
        assert!(
            stats_ms <= stats_threshold_ms,
            "memory_stats_v2 {}ms exceeded threshold {}ms",
            stats_ms,
            stats_threshold_ms
        );
    }
}
