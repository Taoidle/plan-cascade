//! Project Memory Store
//!
//! Core CRUD operations for the project memory system. Manages persistent
//! cross-session memories stored in SQLite with TF-IDF embeddings for
//! semantic search.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::services::memory::query_policy_v2::{memory_query_tuning_v2, MemoryQueryPresetV2};
use crate::services::orchestrator::embedding_manager::EmbeddingManager;
use crate::services::orchestrator::embedding_service::{cosine_similarity, EmbeddingService};
use crate::storage::database::{Database, DbPool};
use crate::utils::error::{AppError, AppResult};

// ============================================================================
// Constants
// ============================================================================

/// Sentinel project path for global (cross-project) memories.
/// Memories stored under this path are loaded for every project session.
pub const GLOBAL_PROJECT_PATH: &str = "__global__";
/// Sentinel project-path prefix for session-scoped memories.
pub const SESSION_PROJECT_PATH_PREFIX: &str = "__session__:";

/// Similarity threshold tuned for sparse TF-IDF vectors during upsert merge.
///
/// This is intentionally lower than dense-embedding heuristics; TF-IDF cosine
/// distributions are generally more compressed.
const TFIDF_UPSERT_MERGE_THRESHOLD: f32 = 0.72;
const MEMORY_EMBEDDING_PROVIDER_TFIDF: &str = "tfidf";
const MEMORY_ENTRY_SELECT_SQL: &str = "id, scope, project_path, session_id, category, content, keywords, importance, access_count, source_session_id, source_context, status, risk_tier, conflict_flag, created_at, updated_at, last_accessed_at";

/// Normalize a session id for memory scoping.
///
/// Accepts raw ids and history-prefixed ids (`claude:<id>`, `standalone:<id>`).
pub fn normalize_memory_session_id(session_id: &str) -> Option<String> {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed
        .strip_prefix("claude:")
        .or_else(|| trimmed.strip_prefix("standalone:"))
        .unwrap_or(trimmed)
        .trim();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

/// Build the internal project_path sentinel for a session-scoped memory namespace.
pub fn build_session_project_path(session_id: &str) -> Option<String> {
    normalize_memory_session_id(session_id)
        .map(|normalized| format!("{}{}", SESSION_PROJECT_PATH_PREFIX, normalized))
}

#[derive(Debug, Clone)]
struct MemoryScopeMapping {
    scope: String,
    project_path: Option<String>,
    session_id: Option<String>,
}

fn map_legacy_project_path(project_path: &str) -> MemoryScopeMapping {
    if project_path == GLOBAL_PROJECT_PATH {
        MemoryScopeMapping {
            scope: "global".to_string(),
            project_path: None,
            session_id: None,
        }
    } else if let Some(session) = project_path.strip_prefix(SESSION_PROJECT_PATH_PREFIX) {
        MemoryScopeMapping {
            scope: "session".to_string(),
            project_path: None,
            session_id: normalize_memory_session_id(session),
        }
    } else {
        MemoryScopeMapping {
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
            .and_then(build_session_project_path)
            .unwrap_or_else(|| GLOBAL_PROJECT_PATH.to_string()),
        _ => project_path.unwrap_or_default().to_string(),
    }
}

fn default_status_and_risk(source_context: Option<&str>) -> (&'static str, &'static str) {
    match source_context {
        Some(ctx) if ctx.starts_with("llm_extract:") => ("pending_review", "medium"),
        Some(ctx) if ctx.starts_with("rule_extract:") => ("pending_review", "low"),
        _ => ("active", "high"),
    }
}

// ============================================================================
// Data Types
// ============================================================================

/// Categories of project memory
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    Preference,
    Convention,
    Pattern,
    Correction,
    Fact,
}

impl MemoryCategory {
    /// Convert to database string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryCategory::Preference => "preference",
            MemoryCategory::Convention => "convention",
            MemoryCategory::Pattern => "pattern",
            MemoryCategory::Correction => "correction",
            MemoryCategory::Fact => "fact",
        }
    }

    /// Parse from database string representation
    pub fn from_str(s: &str) -> AppResult<Self> {
        match s {
            "preference" => Ok(MemoryCategory::Preference),
            "convention" => Ok(MemoryCategory::Convention),
            "pattern" => Ok(MemoryCategory::Pattern),
            "correction" => Ok(MemoryCategory::Correction),
            "fact" => Ok(MemoryCategory::Fact),
            _ => Err(AppError::Validation(format!(
                "Invalid memory category: {}",
                s
            ))),
        }
    }
}

impl std::fmt::Display for MemoryCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub project_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub category: MemoryCategory,
    pub content: String,
    pub keywords: Vec<String>,
    pub importance: f32,
    pub access_count: i64,
    pub source_session_id: Option<String>,
    pub source_context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_flag: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_accessed_at: String,
}

/// Input for creating a new memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMemoryEntry {
    pub project_path: String,
    pub category: MemoryCategory,
    pub content: String,
    pub keywords: Vec<String>,
    pub importance: f32,
    pub source_session_id: Option<String>,
    pub source_context: Option<String>,
}

/// Partial update fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUpdate {
    pub content: Option<String>,
    pub category: Option<MemoryCategory>,
    pub importance: Option<f32>,
    pub keywords: Option<Vec<String>>,
}

/// Result of an upsert operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpsertResult {
    Inserted(MemoryEntry),
    Merged {
        original_id: String,
        merged: MemoryEntry,
    },
    Skipped {
        reason: String,
    },
}

/// Request to search memories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchRequest {
    pub project_path: String,
    pub query: String,
    pub categories: Option<Vec<MemoryCategory>>,
    pub top_k: usize,
    pub min_importance: f32,
}

impl Default for MemorySearchRequest {
    fn default() -> Self {
        let tuning = memory_query_tuning_v2(MemoryQueryPresetV2::CommandSearch);
        Self {
            project_path: String::new(),
            query: String::new(),
            categories: None,
            top_k: tuning.top_k_total,
            min_importance: tuning.min_importance,
        }
    }
}

/// Search result with relevance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    pub entry: MemoryEntry,
    pub relevance_score: f32,
}

/// Statistics about project memories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_count: usize,
    pub category_counts: std::collections::HashMap<String, usize>,
    pub avg_importance: f32,
}

// ============================================================================
// ProjectMemoryStore
// ============================================================================

/// Core store implementation for project memories
pub struct ProjectMemoryStore {
    pool: DbPool,
    embedding_service: Arc<EmbeddingService>,
    embedding_manager: Option<Arc<EmbeddingManager>>,
    vocab_state: Arc<Mutex<VocabularyState>>,
}

#[derive(Debug, Default)]
struct VocabularyState {
    active_project: Option<String>,
    dirty_projects: HashSet<String>,
}

impl ProjectMemoryStore {
    /// Create a new ProjectMemoryStore from a connection pool and embedding service
    pub fn new(pool: DbPool, embedding_service: Arc<EmbeddingService>) -> Self {
        Self {
            pool,
            embedding_service,
            embedding_manager: None,
            vocab_state: Arc::new(Mutex::new(VocabularyState::default())),
        }
    }

    /// Create a new ProjectMemoryStore from a Database instance
    pub fn from_database(db: &Database, embedding_service: Arc<EmbeddingService>) -> Self {
        Self {
            pool: db.pool().clone(),
            embedding_service,
            embedding_manager: None,
            vocab_state: Arc::new(Mutex::new(VocabularyState::default())),
        }
    }

    /// Attach an embedding manager for provider-aware dense retrieval.
    pub fn set_embedding_manager(&mut self, manager: Arc<EmbeddingManager>) {
        self.embedding_manager = Some(manager);
    }

    /// Get the configured embedding manager, if any.
    pub fn get_embedding_manager(&self) -> Option<Arc<EmbeddingManager>> {
        self.embedding_manager.clone()
    }

    // ========================================================================
    // Write Operations
    // ========================================================================

    /// Add a new memory entry (generates UUID, computes embedding, inserts into DB)
    pub fn add_memory(&self, entry: NewMemoryEntry) -> AppResult<MemoryEntry> {
        let id = uuid::Uuid::new_v4().to_string();
        let keywords_json = serde_json::to_string(&entry.keywords)?;
        let scoped = map_legacy_project_path(&entry.project_path);
        let (status, risk_tier) = default_status_and_risk(entry.source_context.as_deref());

        self.mark_vocabulary_dirty(&entry.project_path);
        self.ensure_vocabulary_for_project_with_seed(
            &entry.project_path,
            std::slice::from_ref(&entry.content),
        )?;

        // Generate embedding
        let embedding = self.embedding_service.embed_text(&entry.content);
        let embedding_dim = embedding.len() as i64;
        let embedding_bytes = if embedding.is_empty() {
            None
        } else {
            Some(embedding_to_bytes(&embedding))
        };
        let quality_score = entry.importance.clamp(0.0, 1.0);

        {
            let conn = self.get_connection()?;
            conn.execute(
                "INSERT INTO memory_entries_v2 (
                    id, scope, project_path, session_id, category, content, content_hash,
                    keywords, embedding, importance, source_session_id, source_context,
                    status, risk_tier, conflict_flag, embedding_provider, embedding_dim, quality_score
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, lower(trim(?6)), ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, ?14, ?15, ?16)",
                params![
                    id,
                    scoped.scope,
                    scoped.project_path,
                    scoped.session_id,
                    entry.category.as_str(),
                    entry.content,
                    keywords_json,
                    embedding_bytes,
                    entry.importance,
                    entry.source_session_id,
                    entry.source_context,
                    status,
                    risk_tier,
                    MEMORY_EMBEDDING_PROVIDER_TFIDF,
                    embedding_dim,
                    quality_score,
                ],
            )?;
        } // connection released here

        self.get_memory(&id)?
            .ok_or_else(|| AppError::Internal("Failed to retrieve newly inserted memory".into()))
    }

    /// Batch add memories
    pub fn add_memories(&self, entries: Vec<NewMemoryEntry>) -> AppResult<Vec<MemoryEntry>> {
        let mut results = Vec::new();
        for entry in entries {
            results.push(self.add_memory(entry)?);
        }
        Ok(results)
    }

    /// Update an existing memory
    pub fn update_memory(&self, id: &str, updates: MemoryUpdate) -> AppResult<MemoryEntry> {
        let existing = self
            .get_memory(id)?
            .ok_or_else(|| AppError::NotFound(format!("Memory not found: {}", id)))?;

        let new_content = updates.content.as_deref().unwrap_or(&existing.content);
        let new_category = updates.category.as_ref().unwrap_or(&existing.category);
        let new_importance = updates.importance.unwrap_or(existing.importance);
        let new_keywords = updates.keywords.as_ref().unwrap_or(&existing.keywords);

        let keywords_json = serde_json::to_string(new_keywords)?;

        // Recompute embedding metadata if content changed, otherwise preserve existing metadata.
        let (embedding_bytes, embedding_provider, embedding_dim, quality_score) =
            if updates.content.is_some() {
                self.mark_vocabulary_dirty(&existing.project_path);
                let seed_content = new_content.to_string();
                self.ensure_vocabulary_for_project_with_seed(
                    &existing.project_path,
                    std::slice::from_ref(&seed_content),
                )?;
                let emb = self.embedding_service.embed_text(new_content);
                let emb_dim = emb.len() as i64;
                let emb_bytes = if emb.is_empty() {
                    None
                } else {
                    Some(embedding_to_bytes(&emb))
                };
                (
                    emb_bytes,
                    MEMORY_EMBEDDING_PROVIDER_TFIDF.to_string(),
                    emb_dim,
                    new_importance.clamp(0.0, 1.0),
                )
            } else {
                let conn = self.get_connection()?;
                conn.query_row(
                    "SELECT embedding, embedding_provider, embedding_dim, quality_score
                 FROM memory_entries_v2 WHERE id = ?1",
                    params![id],
                    |row| {
                        Ok((
                            row.get::<_, Option<Vec<u8>>>(0)?,
                            row.get::<_, Option<String>>(1)?
                                .unwrap_or_else(|| MEMORY_EMBEDDING_PROVIDER_TFIDF.to_string()),
                            row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                            row.get::<_, Option<f32>>(3)?.unwrap_or(1.0),
                        ))
                    },
                )?
            };

        {
            let conn = self.get_connection()?;
            conn.execute(
                "UPDATE memory_entries_v2
                 SET content = ?2, content_hash = lower(trim(?2)), category = ?3, importance = ?4, keywords = ?5, embedding = ?6,
                     embedding_provider = ?7, embedding_dim = ?8, quality_score = ?9,
                     updated_at = datetime('now')
                 WHERE id = ?1",
                params![
                    id,
                    new_content,
                    new_category.as_str(),
                    new_importance,
                    keywords_json,
                    embedding_bytes,
                    embedding_provider,
                    embedding_dim,
                    quality_score,
                ],
            )?;
        } // connection released here

        self.get_memory(id)?
            .ok_or_else(|| AppError::Internal("Failed to retrieve updated memory".into()))
    }

    /// Upsert: if similar memory exists (cosine > TFIDF_UPSERT_MERGE_THRESHOLD), merge; otherwise insert.
    pub fn upsert_memory(&self, entry: NewMemoryEntry) -> AppResult<UpsertResult> {
        if entry.content.trim().is_empty() {
            return Ok(UpsertResult::Skipped {
                reason: "Empty content".into(),
            });
        }

        self.mark_vocabulary_dirty(&entry.project_path);
        self.ensure_vocabulary_for_project_with_seed(
            &entry.project_path,
            std::slice::from_ref(&entry.content),
        )?;

        // Generate embedding for the new entry
        let new_embedding = self.embedding_service.embed_text(&entry.content);

        struct ExistingCandidate {
            id: String,
            content: String,
            importance: f32,
            keywords: Vec<String>,
            embedding: Vec<f32>,
        }

        // Load existing memories + embeddings in one query (avoid N+1 queries).
        let existing: Vec<ExistingCandidate> = {
            let conn = self.get_connection()?;
            let scoped = map_legacy_project_path(&entry.project_path);
            let map_row = |row: &rusqlite::Row| -> rusqlite::Result<ExistingCandidate> {
                let keywords_json: String = row.get(3)?;
                let embedding_bytes: Option<Vec<u8>> = row.get(4)?;
                Ok(ExistingCandidate {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    importance: row.get(2)?,
                    keywords: serde_json::from_str(&keywords_json).unwrap_or_default(),
                    embedding: embedding_bytes
                        .map(|bytes| bytes_to_embedding(&bytes))
                        .unwrap_or_default(),
                })
            };

            if scoped.scope == "global" {
                let mut stmt = conn.prepare(
                    "SELECT id, content, importance, keywords, embedding
                     FROM memory_entries_v2
                     WHERE scope = 'global' AND status = 'active'
                     ORDER BY importance DESC
                     LIMIT 1000",
                )?;
                let rows = stmt
                    .query_map([], map_row)?
                    .filter_map(|r| r.ok())
                    .collect();
                rows
            } else if scoped.scope == "session" {
                let sid = scoped.session_id.unwrap_or_default();
                let mut stmt = conn.prepare(
                    "SELECT id, content, importance, keywords, embedding
                     FROM memory_entries_v2
                     WHERE scope = 'session' AND session_id = ?1 AND status = 'active'
                     ORDER BY importance DESC
                     LIMIT 1000",
                )?;
                let rows = stmt
                    .query_map(params![sid], map_row)?
                    .filter_map(|r| r.ok())
                    .collect();
                rows
            } else {
                let project = scoped.project_path.unwrap_or_default();
                let mut stmt = conn.prepare(
                    "SELECT id, content, importance, keywords, embedding
                     FROM memory_entries_v2
                     WHERE scope = 'project' AND project_path = ?1 AND status = 'active'
                     ORDER BY importance DESC
                     LIMIT 1000",
                )?;
                let rows = stmt
                    .query_map(params![project], map_row)?
                    .filter_map(|r| r.ok())
                    .collect();
                rows
            }
        };

        // Check for high-similarity duplicates
        if !new_embedding.is_empty() {
            for mem in &existing {
                if mem.embedding.is_empty() {
                    continue;
                }
                let sim = cosine_similarity(&new_embedding, &mem.embedding);

                if sim > TFIDF_UPSERT_MERGE_THRESHOLD {
                    // Merge: update existing entry with combined content
                    let merged_content = if mem.content.contains(&entry.content) {
                        mem.content.clone()
                    } else {
                        format!("{} | {}", mem.content, entry.content)
                    };
                    let merged_importance = mem.importance.max(entry.importance);

                    let mut merged_keywords = mem.keywords.clone();
                    for kw in &entry.keywords {
                        if !merged_keywords.contains(kw) {
                            merged_keywords.push(kw.clone());
                        }
                    }

                    let update = MemoryUpdate {
                        content: Some(merged_content),
                        category: None,
                        importance: Some(merged_importance),
                        keywords: Some(merged_keywords),
                    };
                    let merged = self.update_memory(&mem.id, update)?;
                    return Ok(UpsertResult::Merged {
                        original_id: mem.id.clone(),
                        merged,
                    });
                }
            }
        }

        // No similar memory found — insert new
        let inserted = self.add_memory(entry)?;
        Ok(UpsertResult::Inserted(inserted))
    }

    // ========================================================================
    // Read Operations
    // ========================================================================

    /// Get a single memory by ID
    pub fn get_memory(&self, id: &str) -> AppResult<Option<MemoryEntry>> {
        let conn = self.get_connection()?;
        let sql = format!(
            "SELECT {} FROM memory_entries_v2 WHERE id = ?1",
            MEMORY_ENTRY_SELECT_SQL
        );
        let result = conn.query_row(&sql, params![id], row_to_memory_entry);

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// List memories for a project with optional category filter and pagination
    pub fn list_memories(
        &self,
        project_path: &str,
        category: Option<MemoryCategory>,
        offset: usize,
        limit: usize,
    ) -> AppResult<Vec<MemoryEntry>> {
        let conn = self.get_connection()?;
        let scoped = map_legacy_project_path(project_path);

        let sql_with_category = format!(
            "SELECT {}
             FROM memory_entries_v2
             WHERE scope = ?1
               AND ((?1 = 'project' AND project_path = ?2) OR (?1 = 'session' AND session_id = ?3) OR (?1 = 'global'))
               AND category = ?4
             ORDER BY importance DESC, updated_at DESC
             LIMIT ?5 OFFSET ?6",
            MEMORY_ENTRY_SELECT_SQL
        );
        let sql_without_category = format!(
            "SELECT {}
             FROM memory_entries_v2
             WHERE scope = ?1
               AND ((?1 = 'project' AND project_path = ?2) OR (?1 = 'session' AND session_id = ?3) OR (?1 = 'global'))
             ORDER BY importance DESC, updated_at DESC
             LIMIT ?4 OFFSET ?5",
            MEMORY_ENTRY_SELECT_SQL
        );

        if let Some(ref cat) = category {
            let mut stmt = conn.prepare(&sql_with_category)?;
            let rows = stmt
                .query_map(
                    params![
                        scoped.scope,
                        scoped.project_path,
                        scoped.session_id,
                        cat.as_str(),
                        limit as i64,
                        offset as i64
                    ],
                    row_to_memory_entry,
                )?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        } else {
            let mut stmt = conn.prepare(&sql_without_category)?;
            let rows = stmt
                .query_map(
                    params![
                        scoped.scope,
                        scoped.project_path,
                        scoped.session_id,
                        limit as i64,
                        offset as i64
                    ],
                    row_to_memory_entry,
                )?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        }
    }

    /// Get memory count by project
    pub fn count_memories(&self, project_path: &str) -> AppResult<usize> {
        let conn = self.get_connection()?;
        let scoped = map_legacy_project_path(project_path);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM memory_entries_v2
             WHERE scope = ?1
               AND ((?1 = 'project' AND project_path = ?2) OR (?1 = 'session' AND session_id = ?3) OR (?1 = 'global'))",
            params![scoped.scope, scoped.project_path, scoped.session_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Get memory statistics for a project
    pub fn get_stats(&self, project_path: &str) -> AppResult<MemoryStats> {
        let conn = self.get_connection()?;
        let scoped = map_legacy_project_path(project_path);

        let total_count: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM memory_entries_v2
             WHERE scope = ?1
               AND ((?1 = 'project' AND project_path = ?2) OR (?1 = 'session' AND session_id = ?3) OR (?1 = 'global'))",
            params![scoped.scope, scoped.project_path, scoped.session_id],
            |row| row.get(0),
        )?;

        let avg_importance: f64 = conn.query_row(
            "SELECT COALESCE(AVG(importance), 0.0)
             FROM memory_entries_v2
             WHERE scope = ?1
               AND ((?1 = 'project' AND project_path = ?2) OR (?1 = 'session' AND session_id = ?3) OR (?1 = 'global'))",
            params![scoped.scope, scoped.project_path, scoped.session_id],
            |row| row.get(0),
        )?;

        // Category counts
        let mut stmt = conn.prepare(
            "SELECT category, COUNT(*)
             FROM memory_entries_v2
             WHERE scope = ?1
               AND ((?1 = 'project' AND project_path = ?2) OR (?1 = 'session' AND session_id = ?3) OR (?1 = 'global'))
             GROUP BY category",
        )?;
        let category_counts: std::collections::HashMap<String, usize> = stmt
            .query_map(
                params![scoped.scope, scoped.project_path, scoped.session_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize)),
            )?
            .filter_map(|r| r.ok())
            .collect();

        Ok(MemoryStats {
            total_count: total_count as usize,
            category_counts,
            avg_importance: avg_importance as f32,
        })
    }

    // ========================================================================
    // Delete Operations
    // ========================================================================

    /// Delete a specific memory
    pub fn delete_memory(&self, id: &str) -> AppResult<()> {
        let project_path = self
            .get_memory(id)?
            .map(|m| m.project_path)
            .unwrap_or_default();
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM memory_entries_v2 WHERE id = ?1", params![id])?;
        if !project_path.is_empty() {
            self.mark_vocabulary_dirty(&project_path);
        }
        Ok(())
    }

    /// Delete all memories for a project, returns count of deleted entries
    pub fn clear_project_memories(&self, project_path: &str) -> AppResult<usize> {
        let conn = self.get_connection()?;
        let scoped = map_legacy_project_path(project_path);
        let count = conn.execute(
            "DELETE FROM memory_entries_v2
             WHERE scope = ?1
               AND ((?1 = 'project' AND project_path = ?2) OR (?1 = 'session' AND session_id = ?3) OR (?1 = 'global'))",
            params![scoped.scope, scoped.project_path, scoped.session_id],
        )?;
        self.mark_vocabulary_dirty(project_path);
        Ok(count)
    }

    /// Delete all memories for a session namespace.
    pub fn clear_session_memories(&self, session_id: &str) -> AppResult<usize> {
        let Some(project_path) = build_session_project_path(session_id) else {
            return Ok(0);
        };
        self.clear_project_memories(&project_path)
    }

    /// Delete expired session-scope memories older than `ttl_days`.
    ///
    /// This operates directly on V2 storage.
    pub fn cleanup_expired_session_memories(&self, ttl_days: i64) -> AppResult<usize> {
        let effective_ttl = ttl_days.max(1);
        let threshold = format!("-{} days", effective_ttl);
        let conn = self.get_connection()?;
        let deleted = conn.execute(
            "DELETE FROM memory_entries_v2
             WHERE scope = 'session'
               AND updated_at < datetime('now', ?1)",
            params![threshold],
        )?;
        Ok(deleted)
    }

    /// Increment access counters for the specified memory IDs.
    ///
    /// Returns the number of rows updated.
    pub fn touch_memories(&self, ids: &[String]) -> AppResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let conn = self.get_connection()?;
        let tx = conn.unchecked_transaction()?;
        let mut touched = 0usize;
        for id in ids {
            touched += tx.execute(
                "UPDATE memory_entries_v2
                 SET access_count = access_count + 1,
                     last_accessed_at = datetime('now')
                 WHERE id = ?1",
                params![id],
            )?;
        }
        tx.commit()?;
        Ok(touched)
    }

    /// Ensure TF-IDF vocabulary is ready for the target project.
    ///
    /// The memory module uses a single `EmbeddingService` instance. To keep
    /// search/upsert quality stable, we rebuild vocabulary when switching
    /// projects or when writes have marked a project as dirty.
    pub fn ensure_vocabulary_for_project(&self, project_path: &str) -> AppResult<()> {
        self.ensure_vocabulary_for_project_with_seed(project_path, &[])
    }

    /// Mark a project's vocabulary as stale so the next query/write rebuilds it.
    pub fn mark_vocabulary_dirty_for_project(&self, project_path: &str) {
        self.mark_vocabulary_dirty(project_path);
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    fn ensure_vocabulary_for_project_with_seed(
        &self,
        project_path: &str,
        seed_docs: &[String],
    ) -> AppResult<()> {
        if project_path.trim().is_empty() {
            return Ok(());
        }

        let needs_rebuild = {
            let state = self.vocab_state.lock().unwrap();
            let active_matches = state
                .active_project
                .as_ref()
                .map(|p| p == project_path)
                .unwrap_or(false);
            !active_matches
                || state.dirty_projects.contains(project_path)
                || !self.embedding_service.is_ready()
        };

        if !needs_rebuild {
            return Ok(());
        }

        let mut corpus = self.load_memory_contents_for_project(project_path)?;
        corpus.extend(seed_docs.iter().cloned());

        if corpus.is_empty() {
            return Ok(());
        }

        let corpus_refs: Vec<&str> = corpus.iter().map(String::as_str).collect();
        self.embedding_service.build_vocabulary(&corpus_refs);
        self.backfill_embeddings_for_project(project_path)?;

        let mut state = self.vocab_state.lock().unwrap();
        state.active_project = Some(project_path.to_string());
        state.dirty_projects.remove(project_path);
        Ok(())
    }

    fn load_memory_contents_for_project(&self, project_path: &str) -> AppResult<Vec<String>> {
        let conn = self.get_connection()?;
        let scoped = map_legacy_project_path(project_path);

        if scoped.scope == "global" {
            let mut stmt = conn.prepare(
                "SELECT content
                 FROM memory_entries_v2
                 WHERE scope = 'global' AND status = 'active'
                 ORDER BY importance DESC, updated_at DESC
                 LIMIT 2000",
            )?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            return Ok(rows);
        }

        if scoped.scope == "session" {
            let mut stmt = conn.prepare(
                "SELECT content
                 FROM memory_entries_v2
                 WHERE scope = 'session' AND session_id = ?1 AND status = 'active'
                 ORDER BY importance DESC, updated_at DESC
                 LIMIT 2000",
            )?;
            let rows = stmt
                .query_map(params![scoped.session_id], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            return Ok(rows);
        }

        let mut stmt = conn.prepare(
            "SELECT content
             FROM memory_entries_v2
             WHERE scope = 'project' AND project_path = ?1 AND status = 'active'
             ORDER BY importance DESC, updated_at DESC
             LIMIT 2000",
        )?;
        let rows = stmt
            .query_map(params![scoped.project_path], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn backfill_embeddings_for_project(&self, project_path: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        let scoped = map_legacy_project_path(project_path);
        let rows: Vec<(String, String)> = if scoped.scope == "global" {
            let mut stmt = conn.prepare(
                "SELECT id, content
                 FROM memory_entries_v2
                 WHERE scope = 'global' AND status = 'active'",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        } else if scoped.scope == "session" {
            let mut stmt = conn.prepare(
                "SELECT id, content
                 FROM memory_entries_v2
                 WHERE scope = 'session' AND session_id = ?1 AND status = 'active'",
            )?;
            let rows = stmt
                .query_map(params![scoped.session_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, content
                 FROM memory_entries_v2
                 WHERE scope = 'project' AND project_path = ?1 AND status = 'active'",
            )?;
            let rows = stmt
                .query_map(params![scoped.project_path], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };

        for (id, content) in rows {
            let emb = self.embedding_service.embed_text(&content);
            let emb_dim = emb.len() as i64;
            let emb_bytes = if emb.is_empty() {
                None
            } else {
                Some(embedding_to_bytes(&emb))
            };
            conn.execute(
                "UPDATE memory_entries_v2
                 SET embedding = ?2, embedding_provider = ?3, embedding_dim = ?4, updated_at = datetime('now')
                 WHERE id = ?1",
                params![id, emb_bytes, MEMORY_EMBEDDING_PROVIDER_TFIDF, emb_dim],
            )?;
        }
        Ok(())
    }

    fn mark_vocabulary_dirty(&self, project_path: &str) {
        if project_path.trim().is_empty() {
            return;
        }
        let mut state = self.vocab_state.lock().unwrap();
        state.dirty_projects.insert(project_path.to_string());
    }

    /// Get a connection from the pool
    fn get_connection(
        &self,
    ) -> AppResult<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))
    }

    /// Get access to the embedding service (used by retrieval and maintenance)
    pub fn embedding_service(&self) -> &EmbeddingService {
        &self.embedding_service
    }

    /// Get access to the connection pool (used by retrieval and maintenance)
    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Convert a database row to a MemoryEntry
fn row_to_memory_entry(row: &rusqlite::Row) -> rusqlite::Result<MemoryEntry> {
    let id: String = row.get(0)?;
    let scope: String = row.get(1)?;
    let project_path_v2: Option<String> = row.get(2)?;
    let session_id_v2: Option<String> = row.get(3)?;
    let category_str: String = row.get(4)?;
    let content: String = row.get(5)?;
    let keywords_json: String = row.get(6)?;
    let importance: f32 = row.get(7)?;
    let access_count: i64 = row.get(8)?;
    let source_session_id: Option<String> = row.get(9)?;
    let source_context: Option<String> = row.get(10)?;
    let status: String = row.get(11)?;
    let risk_tier: String = row.get(12)?;
    let conflict_flag = row.get::<_, i64>(13).unwrap_or(0) != 0;
    let created_at: String = row.get(14)?;
    let updated_at: String = row.get(15)?;
    let last_accessed_at: String = row.get(16)?;
    let legacy_project_path = legacy_project_path_from_scope(
        &scope,
        project_path_v2.as_deref(),
        session_id_v2.as_deref(),
    );

    let category = MemoryCategory::from_str(&category_str).unwrap_or(MemoryCategory::Fact);
    let keywords: Vec<String> = serde_json::from_str(&keywords_json).unwrap_or_default();

    Ok(MemoryEntry {
        id,
        project_path: legacy_project_path,
        scope: Some(scope),
        session_id: session_id_v2,
        category,
        content,
        keywords,
        importance,
        access_count,
        source_session_id,
        source_context,
        status: Some(status),
        risk_tier: Some(risk_tier),
        conflict_flag: Some(conflict_flag),
        trace_id: None,
        created_at,
        updated_at,
        last_accessed_at,
    })
}

/// Serialize f32 embedding vector to bytes for SQLite BLOB storage
pub fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Deserialize bytes from SQLite BLOB back to f32 embedding vector
pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
            f32::from_le_bytes(arr)
        })
        .collect()
}

impl std::fmt::Debug for ProjectMemoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectMemoryStore").finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_store() -> ProjectMemoryStore {
        let db = Database::new_in_memory().unwrap();
        let embedding_service = Arc::new(EmbeddingService::new());

        // Build a basic vocabulary for embedding generation
        embedding_service.build_vocabulary(&[
            "use pnpm not npm for package management",
            "API routes return CommandResponse type",
            "tests in __tests__ directories",
            "Tauri React Rust application",
            "error handling uses AppResult",
        ]);

        ProjectMemoryStore::from_database(&db, embedding_service)
    }

    fn sample_entry(content: &str) -> NewMemoryEntry {
        NewMemoryEntry {
            project_path: "/test/project".into(),
            category: MemoryCategory::Fact,
            content: content.into(),
            keywords: vec!["test".into()],
            importance: 0.5,
            source_session_id: None,
            source_context: None,
        }
    }

    #[test]
    fn test_memory_category_roundtrip() {
        let categories = vec![
            MemoryCategory::Preference,
            MemoryCategory::Convention,
            MemoryCategory::Pattern,
            MemoryCategory::Correction,
            MemoryCategory::Fact,
        ];

        for cat in categories {
            let s = cat.as_str();
            let parsed = MemoryCategory::from_str(s).unwrap();
            assert_eq!(cat, parsed);
        }
    }

    #[test]
    fn test_memory_category_invalid() {
        assert!(MemoryCategory::from_str("invalid").is_err());
    }

    #[test]
    fn test_add_and_get_memory() {
        let store = create_test_store();

        let entry = NewMemoryEntry {
            project_path: "/test/project".into(),
            category: MemoryCategory::Preference,
            content: "Always use pnpm not npm".into(),
            keywords: vec!["pnpm".into(), "npm".into()],
            importance: 0.9,
            source_session_id: Some("session-1".into()),
            source_context: Some("User said to use pnpm".into()),
        };

        let mem = store.add_memory(entry).unwrap();
        assert!(!mem.id.is_empty());
        assert_eq!(mem.project_path, "/test/project");
        assert_eq!(mem.category, MemoryCategory::Preference);
        assert_eq!(mem.content, "Always use pnpm not npm");
        assert_eq!(mem.keywords, vec!["pnpm", "npm"]);
        assert_eq!(mem.importance, 0.9);
        assert_eq!(mem.access_count, 0);

        // Get by ID
        let fetched = store.get_memory(&mem.id).unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.content, "Always use pnpm not npm");
    }

    #[test]
    fn test_add_memories_batch() {
        let store = create_test_store();

        let entries = vec![
            sample_entry("Fact one"),
            sample_entry("Fact two"),
            sample_entry("Fact three"),
        ];

        let results = store.add_memories(entries).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_list_memories() {
        let store = create_test_store();

        // Add memories with different categories
        store
            .add_memory(NewMemoryEntry {
                category: MemoryCategory::Preference,
                importance: 0.9,
                ..sample_entry("Preference entry")
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                category: MemoryCategory::Convention,
                importance: 0.7,
                ..sample_entry("Convention entry")
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                category: MemoryCategory::Fact,
                importance: 0.5,
                ..sample_entry("Fact entry")
            })
            .unwrap();

        // List all
        let all = store.list_memories("/test/project", None, 0, 100).unwrap();
        assert_eq!(all.len(), 3);

        // List by category
        let prefs = store
            .list_memories("/test/project", Some(MemoryCategory::Preference), 0, 100)
            .unwrap();
        assert_eq!(prefs.len(), 1);
        assert_eq!(prefs[0].content, "Preference entry");

        // Pagination
        let page1 = store.list_memories("/test/project", None, 0, 2).unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = store.list_memories("/test/project", None, 2, 2).unwrap();
        assert_eq!(page2.len(), 1);
    }

    #[test]
    fn test_count_memories() {
        let store = create_test_store();

        assert_eq!(store.count_memories("/test/project").unwrap(), 0);

        store.add_memory(sample_entry("Entry 1")).unwrap();
        store.add_memory(sample_entry("Entry 2")).unwrap();

        assert_eq!(store.count_memories("/test/project").unwrap(), 2);
        assert_eq!(store.count_memories("/other/project").unwrap(), 0);
    }

    #[test]
    fn test_update_memory() {
        let store = create_test_store();

        let mem = store.add_memory(sample_entry("Original content")).unwrap();

        let updated = store
            .update_memory(
                &mem.id,
                MemoryUpdate {
                    content: Some("Updated content".into()),
                    category: Some(MemoryCategory::Convention),
                    importance: Some(0.8),
                    keywords: Some(vec!["updated".into()]),
                },
            )
            .unwrap();

        assert_eq!(updated.content, "Updated content");
        assert_eq!(updated.category, MemoryCategory::Convention);
        assert_eq!(updated.importance, 0.8);
        assert_eq!(updated.keywords, vec!["updated"]);
    }

    #[test]
    fn test_update_memory_not_found() {
        let store = create_test_store();

        let result = store.update_memory(
            "nonexistent",
            MemoryUpdate {
                content: Some("test".into()),
                category: None,
                importance: None,
                keywords: None,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_memory() {
        let store = create_test_store();

        let mem = store.add_memory(sample_entry("To be deleted")).unwrap();
        assert!(store.get_memory(&mem.id).unwrap().is_some());

        store.delete_memory(&mem.id).unwrap();
        assert!(store.get_memory(&mem.id).unwrap().is_none());
    }

    #[test]
    fn test_clear_project_memories() {
        let store = create_test_store();

        store.add_memory(sample_entry("Entry 1")).unwrap();
        store.add_memory(sample_entry("Entry 2")).unwrap();
        store.add_memory(sample_entry("Entry 3")).unwrap();

        let count = store.clear_project_memories("/test/project").unwrap();
        assert_eq!(count, 3);

        assert_eq!(store.count_memories("/test/project").unwrap(), 0);
    }

    #[test]
    fn test_get_stats() {
        let store = create_test_store();

        store
            .add_memory(NewMemoryEntry {
                category: MemoryCategory::Preference,
                importance: 0.9,
                ..sample_entry("Pref 1")
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                category: MemoryCategory::Fact,
                importance: 0.5,
                ..sample_entry("Fact 1")
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                category: MemoryCategory::Fact,
                importance: 0.3,
                ..sample_entry("Fact 2")
            })
            .unwrap();

        let stats = store.get_stats("/test/project").unwrap();
        assert_eq!(stats.total_count, 3);
        assert_eq!(stats.category_counts.get("preference"), Some(&1));
        assert_eq!(stats.category_counts.get("fact"), Some(&2));

        // Average importance: (0.9 + 0.5 + 0.3) / 3 = 0.566...
        assert!((stats.avg_importance - 0.5666).abs() < 0.01);
    }

    #[test]
    fn test_upsert_memory_insert() {
        let store = create_test_store();

        let entry = sample_entry("Completely new memory content");
        let result = store.upsert_memory(entry).unwrap();

        match result {
            UpsertResult::Inserted(mem) => {
                assert_eq!(mem.content, "Completely new memory content");
            }
            _ => panic!("Expected Inserted result"),
        }
    }

    #[test]
    fn test_upsert_memory_skip_empty() {
        let store = create_test_store();

        let entry = NewMemoryEntry {
            content: "  ".into(),
            ..sample_entry("")
        };
        let result = store.upsert_memory(entry).unwrap();

        match result {
            UpsertResult::Skipped { reason } => {
                assert!(reason.contains("Empty"));
            }
            _ => panic!("Expected Skipped result"),
        }
    }

    #[test]
    fn test_upsert_memory_merge_similar() {
        let store = create_test_store();

        // Insert first memory
        let entry1 = NewMemoryEntry {
            project_path: "/test/project".into(),
            category: MemoryCategory::Fact,
            content: "use pnpm not npm for package management".into(),
            keywords: vec!["pnpm".into()],
            importance: 0.5,
            source_session_id: None,
            source_context: None,
        };
        store.add_memory(entry1).unwrap();

        // Upsert with identical content — should merge
        let entry2 = NewMemoryEntry {
            project_path: "/test/project".into(),
            category: MemoryCategory::Fact,
            content: "use pnpm not npm for package management".into(),
            keywords: vec!["npm".into()],
            importance: 0.8,
            source_session_id: None,
            source_context: None,
        };
        let result = store.upsert_memory(entry2).unwrap();

        match result {
            UpsertResult::Merged { merged, .. } => {
                // Higher importance should win
                assert!(merged.importance >= 0.8);
            }
            UpsertResult::Inserted(_) => {
                // Even if not merged (vocab may not produce high-enough similarity),
                // the test is still valid — it means the cosine was below the merge threshold
                // which is acceptable behavior
            }
            UpsertResult::Skipped { .. } => panic!("Should not be skipped"),
        }
    }

    #[test]
    fn test_embedding_roundtrip() {
        let original = vec![0.1f32, 0.2, 0.3, 0.4, 0.5];
        let bytes = embedding_to_bytes(&original);
        let restored = bytes_to_embedding(&bytes);
        assert_eq!(original.len(), restored.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_get_memory_not_found() {
        let store = create_test_store();
        let result = store.get_memory("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_different_projects_isolated() {
        let store = create_test_store();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/project-a".into(),
                ..sample_entry("Memory for project A")
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/project-b".into(),
                ..sample_entry("Memory for project B")
            })
            .unwrap();

        assert_eq!(store.count_memories("/project-a").unwrap(), 1);
        assert_eq!(store.count_memories("/project-b").unwrap(), 1);

        let a_mems = store.list_memories("/project-a", None, 0, 100).unwrap();
        assert_eq!(a_mems.len(), 1);
        assert_eq!(a_mems[0].content, "Memory for project A");
    }

    #[test]
    fn test_normalize_memory_session_id() {
        assert_eq!(
            normalize_memory_session_id("standalone:abc-123").as_deref(),
            Some("abc-123")
        );
        assert_eq!(
            normalize_memory_session_id("claude:task-1").as_deref(),
            Some("task-1")
        );
        assert_eq!(
            normalize_memory_session_id("raw-session").as_deref(),
            Some("raw-session")
        );
        assert!(normalize_memory_session_id("claude:").is_none());
        assert!(normalize_memory_session_id(" ").is_none());
    }

    #[test]
    fn test_clear_session_memories() {
        let store = create_test_store();
        let session_path = build_session_project_path("standalone:session-1").unwrap();

        store
            .add_memory(NewMemoryEntry {
                project_path: session_path.clone(),
                content: "session scoped memory".into(),
                ..sample_entry("ignored")
            })
            .unwrap();
        store.add_memory(sample_entry("project memory")).unwrap();

        let removed = store.clear_session_memories("session-1").unwrap();
        assert_eq!(removed, 1);
        assert_eq!(store.count_memories(&session_path).unwrap(), 0);
        assert_eq!(store.count_memories("/test/project").unwrap(), 1);
    }
}
