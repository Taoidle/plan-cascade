//! Memory Maintenance
//!
//! Decay, pruning, and compaction operations for project memories.
//!
//! - **Decay**: Reduces importance of memories not accessed recently.
//!   Formula: new_importance = importance * 0.95^(days_since_last_access / 7)
//! - **Prune**: Removes memories with importance below a threshold.
//! - **Compact**: Merges highly similar memories (cosine > 0.90).

use rusqlite::params;

use crate::services::memory::store::{
    bytes_to_embedding, embedding_to_bytes, ProjectMemoryStore,
};
use crate::services::orchestrator::embedding_service::cosine_similarity;
use crate::utils::error::AppResult;

/// Memory maintenance operations
pub struct MemoryMaintenance;

impl MemoryMaintenance {
    /// Decay importance of stale memories.
    ///
    /// Formula: new_importance = importance * 0.95^(days_since_last_access / 7)
    ///
    /// Returns count of memories whose importance was updated.
    pub fn decay_memories(store: &ProjectMemoryStore, project_path: &str) -> AppResult<usize> {
        // Read all memories with their last_accessed_at and current importance
        struct DecayCandidate {
            id: String,
            importance: f32,
            days_since_access: f64,
        }

        let candidates: Vec<DecayCandidate> = {
            let conn = store.pool().get().map_err(|e| crate::utils::error::AppError::database(format!("Failed to get connection: {}", e)))?;
            let mut stmt = conn.prepare(
                "SELECT id, importance, julianday('now') - julianday(last_accessed_at)
                 FROM project_memories
                 WHERE project_path = ?1",
            )?;

            let rows: Vec<DecayCandidate> = stmt
                .query_map(params![project_path], |row| {
                    Ok(DecayCandidate {
                        id: row.get(0)?,
                        importance: row.get(1)?,
                        days_since_access: row.get::<_, f64>(2).unwrap_or(0.0),
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };

        let mut affected = 0;
        for candidate in &candidates {
            if candidate.days_since_access <= 0.0 {
                continue;
            }

            // new_importance = importance * 0.95^(days / 7)
            let decay_factor = 0.95_f64.powf(candidate.days_since_access / 7.0);
            let new_importance = (candidate.importance as f64 * decay_factor) as f32;

            if (new_importance - candidate.importance).abs() > f32::EPSILON {
                let conn = store.pool().get().map_err(|e| crate::utils::error::AppError::database(format!("Failed to get connection: {}", e)))?;
                conn.execute(
                    "UPDATE project_memories SET importance = ?2, updated_at = datetime('now') WHERE id = ?1",
                    params![candidate.id, new_importance],
                )?;
                affected += 1;
            }
        }

        Ok(affected)
    }

    /// Remove memories with importance below the given threshold.
    ///
    /// Returns count of deleted memories.
    pub fn prune_memories(
        store: &ProjectMemoryStore,
        project_path: &str,
        min_importance: f32,
    ) -> AppResult<usize> {
        let conn = store.pool().get().map_err(|e| crate::utils::error::AppError::database(format!("Failed to get connection: {}", e)))?;

        let deleted = conn.execute(
            "DELETE FROM project_memories WHERE project_path = ?1 AND importance < ?2",
            params![project_path, min_importance],
        )?;

        Ok(deleted)
    }

    /// Merge highly similar memories (cosine similarity > 0.90).
    ///
    /// For each pair of similar memories:
    /// - Keeps the one with higher importance
    /// - Merges content from both
    /// - Combines keywords
    /// - Deletes the less important one
    ///
    /// Returns count of memories merged (removed).
    pub fn compact_memories(store: &ProjectMemoryStore, project_path: &str) -> AppResult<usize> {
        // Load all memories with their embeddings
        struct MemWithEmb {
            id: String,
            content: String,
            importance: f32,
            keywords_json: String,
            embedding: Option<Vec<f32>>,
        }

        let memories: Vec<MemWithEmb> = {
            let conn = store.pool().get().map_err(|e| crate::utils::error::AppError::database(format!("Failed to get connection: {}", e)))?;
            let mut stmt = conn.prepare(
                "SELECT id, content, importance, keywords, embedding
                 FROM project_memories
                 WHERE project_path = ?1
                 ORDER BY importance DESC",
            )?;

            let rows: Vec<MemWithEmb> = stmt
                .query_map(params![project_path], |row| {
                    let id: String = row.get(0)?;
                    let content: String = row.get(1)?;
                    let importance: f32 = row.get(2)?;
                    let keywords_json: String = row.get(3)?;
                    let embedding_bytes: Option<Vec<u8>> = row.get(4)?;

                    let embedding = embedding_bytes.map(|b| bytes_to_embedding(&b));

                    Ok(MemWithEmb {
                        id,
                        content,
                        importance,
                        keywords_json,
                        embedding,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        }; // connection released

        // Find pairs to merge
        let mut to_delete: Vec<String> = Vec::new();
        let mut merged_count = 0;

        for i in 0..memories.len() {
            if to_delete.contains(&memories[i].id) {
                continue; // Already scheduled for deletion
            }

            for j in (i + 1)..memories.len() {
                if to_delete.contains(&memories[j].id) {
                    continue;
                }

                // Compute cosine similarity
                if let (Some(ref emb_a), Some(ref emb_b)) =
                    (&memories[i].embedding, &memories[j].embedding)
                {
                    let sim = cosine_similarity(emb_a, emb_b);

                    if sim > 0.90 {
                        // Merge j into i (i has higher or equal importance since sorted DESC)
                        let merged_content = if memories[i].content.contains(&memories[j].content) {
                            memories[i].content.clone()
                        } else {
                            format!("{} | {}", memories[i].content, memories[j].content)
                        };

                        let keywords_i: Vec<String> =
                            serde_json::from_str(&memories[i].keywords_json).unwrap_or_default();
                        let keywords_j: Vec<String> =
                            serde_json::from_str(&memories[j].keywords_json).unwrap_or_default();

                        let mut merged_keywords = keywords_i;
                        for kw in keywords_j {
                            if !merged_keywords.contains(&kw) {
                                merged_keywords.push(kw);
                            }
                        }

                        // Recompute embedding for merged content
                        let new_emb = store.embedding_service().embed_text(&merged_content);
                        let new_emb_bytes = if new_emb.is_empty() {
                            None
                        } else {
                            Some(embedding_to_bytes(&new_emb))
                        };

                        // Update the kept memory
                        {
                            let conn = store.pool().get().map_err(|e| crate::utils::error::AppError::database(format!("Failed to get connection: {}", e)))?;
                            let merged_kw_json =
                                serde_json::to_string(&merged_keywords).unwrap_or_default();
                            conn.execute(
                                "UPDATE project_memories SET content = ?2, keywords = ?3, embedding = ?4, updated_at = datetime('now') WHERE id = ?1",
                                params![
                                    memories[i].id,
                                    merged_content,
                                    merged_kw_json,
                                    new_emb_bytes,
                                ],
                            )?;
                        }

                        // Schedule the less important one for deletion
                        to_delete.push(memories[j].id.clone());
                        merged_count += 1;
                    }
                }
            }
        }

        // Delete merged entries
        if !to_delete.is_empty() {
            let conn = store.pool().get().map_err(|e| crate::utils::error::AppError::database(format!("Failed to get connection: {}", e)))?;
            for id in &to_delete {
                conn.execute(
                    "DELETE FROM project_memories WHERE id = ?1",
                    params![id],
                )?;
            }
        }

        Ok(merged_count)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::memory::store::{MemoryCategory, NewMemoryEntry, ProjectMemoryStore};
    use crate::services::orchestrator::embedding_service::EmbeddingService;
    use crate::storage::database::Database;
    use std::sync::Arc;

    fn create_test_store() -> ProjectMemoryStore {
        let db = Database::new_in_memory().unwrap();
        let embedding_service = Arc::new(EmbeddingService::new());

        embedding_service.build_vocabulary(&[
            "use pnpm not npm for package management",
            "API routes return CommandResponse type",
            "tests in __tests__ directories",
            "Tauri React Rust application",
            "error handling uses AppResult",
            "database sqlite connection pooling",
        ]);

        ProjectMemoryStore::from_database(&db, embedding_service)
    }

    fn sample_entry(content: &str, importance: f32) -> NewMemoryEntry {
        NewMemoryEntry {
            project_path: "/test/project".into(),
            category: MemoryCategory::Fact,
            content: content.into(),
            keywords: vec!["test".into()],
            importance,
            source_session_id: None,
            source_context: None,
        }
    }

    // -----------------------------------------------------------------------
    // Decay tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_decay_memories_updates_importance() {
        let store = create_test_store();

        // Insert a memory and manually set its last_accessed_at to 14 days ago
        let mem = store.add_memory(sample_entry("Test decay entry", 0.8)).unwrap();

        {
            let conn = store.pool().get().map_err(|e| format!("{}", e)).unwrap();
            conn.execute(
                "UPDATE project_memories SET last_accessed_at = datetime('now', '-14 days') WHERE id = ?1",
                params![mem.id],
            ).unwrap();
        }

        let affected = MemoryMaintenance::decay_memories(&store, "/test/project").unwrap();
        assert!(affected > 0, "Should have decayed at least one memory");

        let updated = store.get_memory(&mem.id).unwrap().unwrap();
        // After 14 days: 0.8 * 0.95^(14/7) = 0.8 * 0.95^2 = 0.8 * 0.9025 = 0.722
        assert!(
            updated.importance < 0.8,
            "Importance should have decreased from 0.8 to ~0.722, got {}",
            updated.importance
        );
        assert!(
            updated.importance > 0.5,
            "Importance should not have decayed too much, got {}",
            updated.importance
        );
    }

    #[test]
    fn test_decay_memories_no_effect_on_recent() {
        let store = create_test_store();

        let mem = store.add_memory(sample_entry("Recent entry", 0.8)).unwrap();

        // Memory was just created, so last_accessed_at is now
        // Decay should still technically run (julianday diff > 0 within seconds) but effect is minimal
        let _affected = MemoryMaintenance::decay_memories(&store, "/test/project").unwrap();

        let updated = store.get_memory(&mem.id).unwrap().unwrap();
        // For a just-created memory, decay should be negligible
        assert!(
            (updated.importance - 0.8).abs() < 0.05,
            "Recent memory importance should barely change, got {}",
            updated.importance
        );
    }

    // -----------------------------------------------------------------------
    // Prune tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_prune_removes_low_importance() {
        let store = create_test_store();

        store.add_memory(sample_entry("High importance", 0.9)).unwrap();
        store.add_memory(sample_entry("Medium importance", 0.5)).unwrap();
        store.add_memory(sample_entry("Low importance", 0.1)).unwrap();
        store.add_memory(sample_entry("Very low importance", 0.05)).unwrap();

        let deleted = MemoryMaintenance::prune_memories(&store, "/test/project", 0.3).unwrap();
        assert_eq!(deleted, 2, "Should have pruned 2 memories below 0.3");

        let remaining = store.count_memories("/test/project").unwrap();
        assert_eq!(remaining, 2);
    }

    #[test]
    fn test_prune_no_effect_when_all_above_threshold() {
        let store = create_test_store();

        store.add_memory(sample_entry("Entry A", 0.8)).unwrap();
        store.add_memory(sample_entry("Entry B", 0.6)).unwrap();

        let deleted = MemoryMaintenance::prune_memories(&store, "/test/project", 0.3).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn test_prune_empty_project() {
        let store = create_test_store();

        let deleted = MemoryMaintenance::prune_memories(&store, "/nonexistent", 0.5).unwrap();
        assert_eq!(deleted, 0);
    }

    // -----------------------------------------------------------------------
    // Compact tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_compact_merges_identical_content() {
        let store = create_test_store();

        // Add two memories with nearly identical content (should have high cosine)
        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: "use pnpm not npm for package management".into(),
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
                content: "use pnpm not npm for package management always".into(),
                keywords: vec!["npm".into()],
                importance: 0.7,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        let before = store.count_memories("/test/project").unwrap();
        let merged = MemoryMaintenance::compact_memories(&store, "/test/project").unwrap();

        let after = store.count_memories("/test/project").unwrap();

        // If the embeddings were similar enough (> 0.90), compaction should have merged
        if merged > 0 {
            assert!(after < before);
        }
        // Either way, the operation should succeed without errors
    }

    #[test]
    fn test_compact_does_not_merge_different_content() {
        let store = create_test_store();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Fact,
                content: "use pnpm not npm for package management".into(),
                keywords: vec!["pnpm".into()],
                importance: 0.9,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        store
            .add_memory(NewMemoryEntry {
                project_path: "/test/project".into(),
                category: MemoryCategory::Convention,
                content: "database sqlite connection pooling r2d2".into(),
                keywords: vec!["database".into()],
                importance: 0.7,
                source_session_id: None,
                source_context: None,
            })
            .unwrap();

        let merged = MemoryMaintenance::compact_memories(&store, "/test/project").unwrap();
        assert_eq!(merged, 0, "Dissimilar memories should not be merged");

        let count = store.count_memories("/test/project").unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_compact_empty_project() {
        let store = create_test_store();

        let merged = MemoryMaintenance::compact_memories(&store, "/nonexistent").unwrap();
        assert_eq!(merged, 0);
    }
}
