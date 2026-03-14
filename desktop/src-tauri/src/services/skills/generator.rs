//! Skill Generator
//!
//! Auto-generates skills from successful sessions and manages skill_library
//! database operations (CRUD).

use rusqlite::params;
use std::sync::Arc;

use crate::services::skills::model::{GeneratedSkill, GeneratedSkillRecord, SkillReviewStatus};
use crate::storage::database::{Database, DbPool};
use crate::utils::error::{AppError, AppResult};

/// Service for managing auto-generated skills in the skill_library table.
pub struct SkillGeneratorStore {
    pool: DbPool,
}

impl SkillGeneratorStore {
    /// Create a new SkillGeneratorStore.
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            pool: db.pool().clone(),
        }
    }

    /// Create a SkillGeneratorStore from an existing database pool.
    pub fn from_pool(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Save a generated skill to the database.
    pub fn save_generated_skill(
        &self,
        project_path: &str,
        skill: &GeneratedSkill,
    ) -> AppResult<GeneratedSkillRecord> {
        if let Some(existing) = self.find_duplicate_generated_skill(project_path, skill)? {
            return Ok(existing);
        }

        let id = uuid::Uuid::new_v4().to_string();
        let tags_json = serde_json::to_string(&skill.tags)?;
        let session_ids_json = serde_json::to_string(&skill.source_session_ids)?;
        let keywords_json = "[]"; // Keywords can be extracted later

        {
            let conn = self.get_connection()?;
            conn.execute(
                "INSERT INTO skill_library (id, project_path, name, description, tags, body, source_type, source_session_ids, keywords)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    id,
                    project_path,
                    skill.name,
                    skill.description,
                    tags_json,
                    skill.body,
                    "generated",
                    session_ids_json,
                    keywords_json,
                ],
            )?;
        } // conn dropped here

        self.get_generated_skill(&id)?
            .ok_or_else(|| AppError::internal("Failed to retrieve saved skill"))
    }

    fn find_duplicate_generated_skill(
        &self,
        project_path: &str,
        skill: &GeneratedSkill,
    ) -> AppResult<Option<GeneratedSkillRecord>> {
        let normalized_name = normalize_generated_skill_value(&skill.name);
        let normalized_body = normalize_generated_skill_value(&skill.body);
        let existing = self.list_generated_skills(project_path, true)?;
        Ok(existing.into_iter().find(|record| {
            normalize_generated_skill_value(&record.name) == normalized_name
                || normalize_generated_skill_value(&record.body) == normalized_body
        }))
    }

    /// List generated skills for a project.
    pub fn list_generated_skills(
        &self,
        project_path: &str,
        include_disabled: bool,
    ) -> AppResult<Vec<GeneratedSkillRecord>> {
        let conn = self.get_connection()?;

        let sql = if include_disabled {
            "SELECT id, project_path, name, description, tags, body, source_type, \
             source_session_ids, usage_count, success_rate, keywords, enabled, \
             review_status, review_notes, reviewed_at, \
             created_at, updated_at \
             FROM skill_library WHERE project_path = ?1 ORDER BY created_at DESC"
        } else {
            "SELECT id, project_path, name, description, tags, body, source_type, \
             source_session_ids, usage_count, success_rate, keywords, enabled, \
             review_status, review_notes, reviewed_at, \
             created_at, updated_at \
             FROM skill_library WHERE project_path = ?1 AND enabled = 1 ORDER BY created_at DESC"
        };

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![project_path], |row| {
            Ok(GeneratedSkillRow {
                id: row.get(0)?,
                project_path: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                tags_json: row.get(4)?,
                body: row.get(5)?,
                source_type: row.get(6)?,
                session_ids_json: row.get(7)?,
                usage_count: row.get(8)?,
                success_rate: row.get(9)?,
                keywords_json: row.get(10)?,
                enabled: row.get(11)?,
                review_status: row.get(12)?,
                review_notes: row.get(13)?,
                reviewed_at: row.get(14)?,
                created_at: row.get(15)?,
                updated_at: row.get(16)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            let row = row?;
            results.push(row.into_record()?);
        }
        Ok(results)
    }

    /// Get a single generated skill by ID.
    pub fn get_generated_skill(&self, id: &str) -> AppResult<Option<GeneratedSkillRecord>> {
        let conn = self.get_connection()?;

        let result = conn.query_row(
            "SELECT id, project_path, name, description, tags, body, source_type, \
             source_session_ids, usage_count, success_rate, keywords, enabled, \
             review_status, review_notes, reviewed_at, \
             created_at, updated_at \
             FROM skill_library WHERE id = ?1",
            params![id],
            |row| {
                Ok(GeneratedSkillRow {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    tags_json: row.get(4)?,
                    body: row.get(5)?,
                    source_type: row.get(6)?,
                    session_ids_json: row.get(7)?,
                    usage_count: row.get(8)?,
                    success_rate: row.get(9)?,
                    keywords_json: row.get(10)?,
                    enabled: row.get(11)?,
                    review_status: row.get(12)?,
                    review_notes: row.get(13)?,
                    reviewed_at: row.get(14)?,
                    created_at: row.get(15)?,
                    updated_at: row.get(16)?,
                })
            },
        );

        match result {
            Ok(row) => Ok(Some(row.into_record()?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::database(e.to_string())),
        }
    }

    /// Toggle a generated skill's enabled state.
    pub fn toggle_generated_skill(&self, id: &str, enabled: bool) -> AppResult<()> {
        let conn = self.get_connection()?;
        let enabled_int: i32 = if enabled { 1 } else { 0 };

        let rows = conn.execute(
            "UPDATE skill_library SET enabled = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![enabled_int, id],
        )?;

        if rows == 0 {
            return Err(AppError::not_found(format!(
                "Generated skill not found: {}",
                id
            )));
        }

        Ok(())
    }

    pub fn review_generated_skill(
        &self,
        id: &str,
        decision: SkillReviewStatus,
        review_notes: Option<&str>,
    ) -> AppResult<GeneratedSkillRecord> {
        let conn = self.get_connection()?;
        let enabled = matches!(decision, SkillReviewStatus::Approved) as i32;
        let rows = conn.execute(
            "UPDATE skill_library
             SET review_status = ?1,
                 review_notes = ?2,
                 reviewed_at = datetime('now'),
                 enabled = ?3,
                 updated_at = datetime('now')
             WHERE id = ?4",
            params![
                review_status_to_sql(&decision),
                review_notes
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty()),
                enabled,
                id,
            ],
        )?;

        if rows == 0 {
            return Err(AppError::not_found(format!(
                "Generated skill not found: {}",
                id
            )));
        }

        self.get_generated_skill(id)?
            .ok_or_else(|| AppError::internal("Failed to reload reviewed generated skill"))
    }

    pub fn update_generated_skill(
        &self,
        id: &str,
        name: &str,
        description: &str,
        tags: &[String],
        body: &str,
    ) -> AppResult<GeneratedSkillRecord> {
        let conn = self.get_connection()?;
        let rows = conn.execute(
            "UPDATE skill_library
             SET name = ?1,
                 description = ?2,
                 tags = ?3,
                 body = ?4,
                 updated_at = datetime('now')
             WHERE id = ?5",
            params![
                name.trim(),
                description.trim(),
                serde_json::to_string(tags)?,
                body,
                id
            ],
        )?;

        if rows == 0 {
            return Err(AppError::not_found(format!(
                "Generated skill not found: {}",
                id
            )));
        }

        self.get_generated_skill(id)?
            .ok_or_else(|| AppError::internal("Failed to reload updated generated skill"))
    }

    /// Delete a generated skill by ID.
    pub fn delete_generated_skill(&self, id: &str) -> AppResult<()> {
        let conn = self.get_connection()?;

        let rows = conn.execute("DELETE FROM skill_library WHERE id = ?1", params![id])?;

        if rows == 0 {
            return Err(AppError::not_found(format!(
                "Generated skill not found: {}",
                id
            )));
        }

        Ok(())
    }

    /// Increment usage count for a generated skill.
    pub fn increment_usage(&self, id: &str) -> AppResult<()> {
        let conn = self.get_connection()?;

        conn.execute(
            "UPDATE skill_library SET usage_count = usage_count + 1, updated_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;

        Ok(())
    }

    /// Count generated skills for a project.
    pub fn count_generated_skills(&self, project_path: &str) -> AppResult<usize> {
        let conn = self.get_connection()?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM skill_library WHERE project_path = ?1",
            params![project_path],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    fn get_connection(
        &self,
    ) -> AppResult<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))
    }
}

fn normalize_generated_skill_value(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Internal row type for database queries.
struct GeneratedSkillRow {
    id: String,
    project_path: String,
    name: String,
    description: String,
    tags_json: String,
    body: String,
    source_type: String,
    session_ids_json: String,
    usage_count: i64,
    success_rate: f64,
    keywords_json: String,
    enabled: i32,
    review_status: String,
    review_notes: Option<String>,
    reviewed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl GeneratedSkillRow {
    fn into_record(self) -> AppResult<GeneratedSkillRecord> {
        let tags: Vec<String> = serde_json::from_str(&self.tags_json).unwrap_or_default();
        let source_session_ids: Vec<String> =
            serde_json::from_str(&self.session_ids_json).unwrap_or_default();
        let keywords: Vec<String> = serde_json::from_str(&self.keywords_json).unwrap_or_default();

        Ok(GeneratedSkillRecord {
            id: self.id,
            project_path: self.project_path,
            name: self.name,
            description: self.description,
            tags,
            body: self.body,
            source_type: self.source_type,
            source_session_ids,
            usage_count: self.usage_count,
            success_rate: self.success_rate,
            keywords,
            enabled: self.enabled != 0,
            review_status: review_status_from_sql(&self.review_status),
            review_notes: self.review_notes,
            reviewed_at: self.reviewed_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

fn review_status_from_sql(value: &str) -> SkillReviewStatus {
    match value {
        "pending_review" => SkillReviewStatus::PendingReview,
        "rejected" => SkillReviewStatus::Rejected,
        "archived" => SkillReviewStatus::Archived,
        _ => SkillReviewStatus::Approved,
    }
}

fn review_status_to_sql(value: &SkillReviewStatus) -> &'static str {
    match value {
        SkillReviewStatus::PendingReview => "pending_review",
        SkillReviewStatus::Approved => "approved",
        SkillReviewStatus::Rejected => "rejected",
        SkillReviewStatus::Archived => "archived",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_store() -> SkillGeneratorStore {
        let db = Database::new_in_memory().unwrap();
        SkillGeneratorStore::new(Arc::new(db))
    }

    fn make_generated_skill(name: &str) -> GeneratedSkill {
        GeneratedSkill {
            name: name.to_string(),
            description: format!("Auto-generated skill: {}", name),
            tags: vec!["test".to_string(), "generated".to_string()],
            body: format!("# {}\n\n## Steps\n1. Do thing\n2. Do other thing", name),
            source_session_ids: vec!["session-1".to_string()],
        }
    }

    #[test]
    fn test_save_and_get_generated_skill() {
        let store = setup_store();
        let skill = make_generated_skill("add-tauri-command");

        let saved = store.save_generated_skill("/test/project", &skill).unwrap();
        assert_eq!(saved.name, "add-tauri-command");
        assert_eq!(saved.description, "Auto-generated skill: add-tauri-command");
        assert_eq!(saved.tags, vec!["test", "generated"]);
        assert!(saved.body.contains("# add-tauri-command"));
        assert_eq!(saved.source_type, "generated");
        assert_eq!(saved.source_session_ids, vec!["session-1"]);
        assert_eq!(saved.usage_count, 0);
        assert!((saved.success_rate - 1.0).abs() < f64::EPSILON);
        assert!(saved.enabled);
        assert!(!saved.id.is_empty());

        // Get by ID
        let fetched = store.get_generated_skill(&saved.id).unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.name, "add-tauri-command");
    }

    #[test]
    fn test_list_generated_skills() {
        let store = setup_store();

        store
            .save_generated_skill("/test/project", &make_generated_skill("skill-a"))
            .unwrap();
        store
            .save_generated_skill("/test/project", &make_generated_skill("skill-b"))
            .unwrap();
        store
            .save_generated_skill("/other/project", &make_generated_skill("skill-c"))
            .unwrap();

        let all = store.list_generated_skills("/test/project", true).unwrap();
        assert_eq!(all.len(), 2);

        let other = store.list_generated_skills("/other/project", true).unwrap();
        assert_eq!(other.len(), 1);

        let empty = store.list_generated_skills("/empty/project", true).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_list_generated_skills_filter_disabled() {
        let store = setup_store();

        let saved = store
            .save_generated_skill("/test/project", &make_generated_skill("skill-a"))
            .unwrap();
        store
            .save_generated_skill("/test/project", &make_generated_skill("skill-b"))
            .unwrap();

        // Disable one
        store.toggle_generated_skill(&saved.id, false).unwrap();

        let all = store.list_generated_skills("/test/project", true).unwrap();
        assert_eq!(all.len(), 2);

        let enabled_only = store.list_generated_skills("/test/project", false).unwrap();
        assert_eq!(enabled_only.len(), 1);
        assert_eq!(enabled_only[0].name, "skill-b");
    }

    #[test]
    fn test_toggle_generated_skill() {
        let store = setup_store();
        let saved = store
            .save_generated_skill("/test/project", &make_generated_skill("test"))
            .unwrap();

        // Initially enabled
        let fetched = store.get_generated_skill(&saved.id).unwrap().unwrap();
        assert!(fetched.enabled);

        // Disable
        store.toggle_generated_skill(&saved.id, false).unwrap();
        let fetched = store.get_generated_skill(&saved.id).unwrap().unwrap();
        assert!(!fetched.enabled);

        // Re-enable
        store.toggle_generated_skill(&saved.id, true).unwrap();
        let fetched = store.get_generated_skill(&saved.id).unwrap().unwrap();
        assert!(fetched.enabled);
    }

    #[test]
    fn test_toggle_nonexistent_skill() {
        let store = setup_store();
        let result = store.toggle_generated_skill("nonexistent-id", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_generated_skill() {
        let store = setup_store();
        let saved = store
            .save_generated_skill("/test/project", &make_generated_skill("to-delete"))
            .unwrap();

        store.delete_generated_skill(&saved.id).unwrap();
        let fetched = store.get_generated_skill(&saved.id).unwrap();
        assert!(fetched.is_none());
    }

    #[test]
    fn test_delete_nonexistent_skill() {
        let store = setup_store();
        let result = store.delete_generated_skill("nonexistent-id");
        assert!(result.is_err());
    }

    #[test]
    fn test_increment_usage() {
        let store = setup_store();
        let saved = store
            .save_generated_skill("/test/project", &make_generated_skill("test"))
            .unwrap();
        assert_eq!(saved.usage_count, 0);

        store.increment_usage(&saved.id).unwrap();
        store.increment_usage(&saved.id).unwrap();

        let fetched = store.get_generated_skill(&saved.id).unwrap().unwrap();
        assert_eq!(fetched.usage_count, 2);
    }

    #[test]
    fn test_count_generated_skills() {
        let store = setup_store();

        assert_eq!(store.count_generated_skills("/test/project").unwrap(), 0);

        store
            .save_generated_skill("/test/project", &make_generated_skill("a"))
            .unwrap();
        store
            .save_generated_skill("/test/project", &make_generated_skill("b"))
            .unwrap();

        assert_eq!(store.count_generated_skills("/test/project").unwrap(), 2);
    }

    #[test]
    fn test_save_generated_skill_dedupes_same_name() {
        let store = setup_store();
        let saved = store
            .save_generated_skill("/test/project", &make_generated_skill("duplicate"))
            .unwrap();
        let duplicate = store
            .save_generated_skill("/test/project", &make_generated_skill("duplicate"))
            .unwrap();

        assert_eq!(saved.id, duplicate.id);
        assert_eq!(
            store
                .list_generated_skills("/test/project", true)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn test_get_nonexistent_skill() {
        let store = setup_store();
        let fetched = store.get_generated_skill("nonexistent").unwrap();
        assert!(fetched.is_none());
    }
}
