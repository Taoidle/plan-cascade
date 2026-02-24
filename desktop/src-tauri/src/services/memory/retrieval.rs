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
//! 7. Bump access_count and last_accessed_at for returned entries

use std::collections::HashSet;

use chrono::{NaiveDateTime, Utc};
use rusqlite::params;

use crate::services::memory::store::{
    bytes_to_embedding, MemoryCategory, MemorySearchRequest, MemorySearchResult, ProjectMemoryStore,
};
use crate::services::orchestrator::embedding_service::cosine_similarity;
use crate::utils::error::AppResult;

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

/// Extract simple keywords from a query string.
///
/// Splits on whitespace and non-alphanumeric characters, lowercases,
/// filters out short tokens (< 3 chars).
pub fn extract_query_keywords(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| s.len() >= 3)
        .map(|s| s.to_string())
        .collect()
}

/// Search memories using the 4-signal ranking algorithm.
///
/// This function:
/// 1. Generates a TF-IDF embedding for the query
/// 2. Retrieves candidate memories filtered by project_path, categories, and min_importance
/// 3. Scores each candidate using the 4-signal formula
/// 4. Returns top_k results sorted by score descending
/// 5. Bumps access_count and last_accessed_at for returned entries
pub fn search_memories(
    store: &ProjectMemoryStore,
    request: &MemorySearchRequest,
) -> AppResult<Vec<MemorySearchResult>> {
    // Step 1: Generate query embedding
    let query_embedding = store.embedding_service().embed_text(&request.query);
    let query_keywords = extract_query_keywords(&request.query);

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

    let sql = format!(
        "SELECT id, project_path, category, content, keywords, importance, access_count,
                source_session_id, source_context, created_at, updated_at, last_accessed_at,
                embedding
         FROM project_memories
         WHERE project_path = ?1 AND importance >= ?2{}
         ORDER BY importance DESC",
        category_filter
    );

    let mut stmt = conn.prepare(&sql)?;

    struct Candidate {
        id: String,
        project_path: String,
        category: String,
        content: String,
        keywords_json: String,
        importance: f32,
        access_count: i64,
        source_session_id: Option<String>,
        source_context: Option<String>,
        created_at: String,
        updated_at: String,
        last_accessed_at: String,
        embedding_bytes: Option<Vec<u8>>,
    }

    let candidates: Vec<Candidate> = stmt
        .query_map(
            params![request.project_path, request.min_importance],
            |row| {
                Ok(Candidate {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    category: row.get(2)?,
                    content: row.get(3)?,
                    keywords_json: row.get(4)?,
                    importance: row.get(5)?,
                    access_count: row.get(6)?,
                    source_session_id: row.get(7)?,
                    source_context: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                    last_accessed_at: row.get(11)?,
                    embedding_bytes: row.get(12)?,
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
            let score = compute_relevance_score(emb_sim, kw_overlap, c.importance, days);

            let category = MemoryCategory::from_str(&c.category).unwrap_or(MemoryCategory::Fact);

            MemorySearchResult {
                entry: crate::services::memory::store::MemoryEntry {
                    id: c.id,
                    project_path: c.project_path,
                    category,
                    content: c.content,
                    keywords: mem_keywords,
                    importance: c.importance,
                    access_count: c.access_count,
                    source_session_id: c.source_session_id,
                    source_context: c.source_context,
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

    // Step 7: Bump access_count and last_accessed_at for returned entries
    if !scored_results.is_empty() {
        let conn = store.pool().get().map_err(|e| {
            crate::utils::error::AppError::database(format!("Failed to get connection: {}", e))
        })?;
        for result in &scored_results {
            let _ = conn.execute(
                "UPDATE project_memories SET access_count = access_count + 1, last_accessed_at = datetime('now') WHERE id = ?1",
                params![result.entry.id],
            );
        }
    }

    Ok(scored_results)
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
        assert!(keywords.contains(&"what".to_string()));
        assert!(keywords.contains(&"pnpm".to_string()));
        assert!(keywords.contains(&"package".to_string()));
        assert!(keywords.contains(&"manager".to_string()));
        // "the" is 3 chars so it passes, but "is" has only 2 chars and should be filtered
        assert!(!keywords.contains(&"is".to_string()));
        assert!(keywords.contains(&"the".to_string()));
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
    fn test_search_bumps_access_count() {
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

        // Verify access count was bumped
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
}
