//! Prompt Service
//!
//! Business logic for managing prompt templates.

use regex::Regex;
use uuid::Uuid;

use crate::models::prompt::{PromptCreateRequest, PromptTemplate, PromptUpdateRequest};
use crate::storage::database::DbPool;
use crate::storage::Database;
use crate::utils::error::{AppError, AppResult};

/// Service for managing prompt templates
pub struct PromptService {
    pool: DbPool,
}

impl PromptService {
    /// Create a new PromptService with a database pool
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Create from a Database reference
    pub fn from_database(db: &Database) -> Self {
        Self {
            pool: db.pool().clone(),
        }
    }

    /// Seed built-in prompts if none exist
    pub fn seed_builtins(&self) -> AppResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prompts WHERE is_builtin = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if count > 0 {
            return Ok(());
        }

        let builtins = vec![
            (
                "builtin-code-review",
                "Code Review",
                "coding",
                "Review this code for bugs, performance, and best practices:\n\n{{code}}",
                "Analyze code for issues and improvements",
            ),
            (
                "builtin-explain-code",
                "Explain Code",
                "coding",
                "Explain what this code does in simple terms:\n\n{{code}}",
                "Get a clear explanation of code",
            ),
            (
                "builtin-refactor",
                "Refactor",
                "coding",
                "Refactor this code to improve readability and maintainability:\n\n{{code}}",
                "Improve code structure",
            ),
            (
                "builtin-write-tests",
                "Write Tests",
                "coding",
                "Write comprehensive unit tests for:\n\n{{code}}",
                "Generate test cases",
            ),
            (
                "builtin-fix-bug",
                "Fix Bug",
                "coding",
                "Find and fix the bug:\n\n{{code}}\n\nError: {{error}}",
                "Debug and fix issues",
            ),
            (
                "builtin-explain-error",
                "Explain Error",
                "coding",
                "Explain this error and suggest a fix:\n\n{{error}}",
                "Understand error messages",
            ),
            (
                "builtin-summarize",
                "Summarize",
                "writing",
                "Summarize concisely:\n\n{{text}}",
                "Create concise summaries",
            ),
            (
                "builtin-improve-writing",
                "Improve Writing",
                "writing",
                "Improve clarity and flow:\n\n{{text}}",
                "Enhance writing quality",
            ),
            (
                "builtin-translate",
                "Translate",
                "writing",
                "Translate to {{language}}:\n\n{{text}}",
                "Translate text to another language",
            ),
            (
                "builtin-analyze",
                "Analyze",
                "analysis",
                "Analyze and provide key insights:\n\n{{content}}",
                "Extract key insights from content",
            ),
        ];

        for (id, title, category, content, description) in builtins {
            let variables = extract_variables(content);
            let variables_json =
                serde_json::to_string(&variables).unwrap_or_else(|_| "[]".to_string());

            conn.execute(
                "INSERT OR IGNORE INTO prompts (id, title, content, description, category, tags, variables, is_builtin, is_pinned, use_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, '[]', ?6, 1, 0, 0)",
                rusqlite::params![id, title, content, description, category, variables_json],
            )?;
        }

        Ok(())
    }

    /// List prompts with optional category filter and search
    pub fn list_prompts(
        &self,
        category: Option<&str>,
        search: Option<&str>,
    ) -> AppResult<Vec<PromptTemplate>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut sql = String::from(
            "SELECT id, title, content, description, category, tags, variables,
                    is_builtin, is_pinned, use_count, last_used_at, created_at, updated_at
             FROM prompts WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(cat) = category {
            sql.push_str(" AND category = ?");
            params_vec.push(Box::new(cat.to_string()));
        }

        if let Some(q) = search {
            sql.push_str(" AND (title LIKE ? OR description LIKE ? OR content LIKE ?)");
            let pattern = format!("%{}%", q);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY is_pinned DESC, use_count DESC, title ASC");

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| Ok(row_to_prompt(row)))?;

        let mut prompts = Vec::new();
        for row in rows {
            prompts.push(row?);
        }

        Ok(prompts)
    }

    /// Get a single prompt by ID
    pub fn get_prompt(&self, id: &str) -> AppResult<Option<PromptTemplate>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn.prepare(
            "SELECT id, title, content, description, category, tags, variables,
                    is_builtin, is_pinned, use_count, last_used_at, created_at, updated_at
             FROM prompts WHERE id = ?1",
        )?;

        let result = stmt
            .query_row(rusqlite::params![id], |row| Ok(row_to_prompt(row)))
            .optional()?;

        Ok(result)
    }

    /// Create a new prompt template
    pub fn create_prompt(&self, req: PromptCreateRequest) -> AppResult<PromptTemplate> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let id = Uuid::new_v4().to_string();
        let variables = extract_variables(&req.content);
        let tags_json = serde_json::to_string(&req.tags).unwrap_or_else(|_| "[]".to_string());
        let variables_json = serde_json::to_string(&variables).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            "INSERT INTO prompts (id, title, content, description, category, tags, variables, is_builtin, is_pinned, use_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, 0)",
            rusqlite::params![
                id,
                req.title,
                req.content,
                req.description,
                req.category,
                tags_json,
                variables_json,
                req.is_pinned as i32,
            ],
        )?;

        self.get_prompt(&id)?
            .ok_or_else(|| AppError::database("Failed to retrieve created prompt"))
    }

    /// Update an existing prompt template
    pub fn update_prompt(&self, id: &str, req: PromptUpdateRequest) -> AppResult<PromptTemplate> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let existing = self
            .get_prompt(id)?
            .ok_or_else(|| AppError::database(format!("Prompt not found: {}", id)))?;

        let title = req.title.unwrap_or(existing.title);
        let content = req.content.unwrap_or(existing.content.clone());
        let description = req.description.or(existing.description);
        let category = req.category.unwrap_or(existing.category);
        let tags = req.tags.unwrap_or(existing.tags);
        let is_pinned = req.is_pinned.unwrap_or(existing.is_pinned);

        let variables = extract_variables(&content);
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
        let variables_json = serde_json::to_string(&variables).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            "UPDATE prompts SET title = ?1, content = ?2, description = ?3, category = ?4,
             tags = ?5, variables = ?6, is_pinned = ?7, updated_at = datetime('now')
             WHERE id = ?8",
            rusqlite::params![
                title,
                content,
                description,
                category,
                tags_json,
                variables_json,
                is_pinned as i32,
                id,
            ],
        )?;

        self.get_prompt(id)?
            .ok_or_else(|| AppError::database("Failed to retrieve updated prompt"))
    }

    /// Delete a prompt (refuses to delete built-in prompts)
    pub fn delete_prompt(&self, id: &str) -> AppResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Check if it's a built-in prompt
        let is_builtin: bool = conn
            .query_row(
                "SELECT is_builtin FROM prompts WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get::<_, i32>(0).map(|v| v != 0),
            )
            .map_err(|_| AppError::database(format!("Prompt not found: {}", id)))?;

        if is_builtin {
            return Err(AppError::database(
                "Cannot delete built-in prompts. Use 'Duplicate as Custom' instead.",
            ));
        }

        conn.execute("DELETE FROM prompts WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    /// Record usage of a prompt (increment use_count, update last_used_at)
    pub fn record_use(&self, id: &str) -> AppResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "UPDATE prompts SET use_count = use_count + 1, last_used_at = datetime('now')
             WHERE id = ?1",
            rusqlite::params![id],
        )?;

        Ok(())
    }

    /// Toggle pin status of a prompt
    pub fn toggle_pin(&self, id: &str) -> AppResult<PromptTemplate> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "UPDATE prompts SET is_pinned = CASE WHEN is_pinned = 1 THEN 0 ELSE 1 END,
             updated_at = datetime('now')
             WHERE id = ?1",
            rusqlite::params![id],
        )?;

        self.get_prompt(id)?
            .ok_or_else(|| AppError::database("Failed to retrieve updated prompt"))
    }
}

/// Extract {{variable}} names from template content
fn extract_variables(content: &str) -> Vec<String> {
    let re = Regex::new(r"\{\{(\w+)\}\}").unwrap();
    let mut vars: Vec<String> = re
        .captures_iter(content)
        .map(|c| c[1].to_string())
        .collect();
    vars.dedup();
    vars
}

/// Convert a database row to a PromptTemplate
fn row_to_prompt(row: &rusqlite::Row) -> PromptTemplate {
    let tags_str: String = row.get::<_, String>(5).unwrap_or_else(|_| "[]".to_string());
    let variables_str: String = row.get::<_, String>(6).unwrap_or_else(|_| "[]".to_string());

    PromptTemplate {
        id: row.get(0).unwrap_or_default(),
        title: row.get(1).unwrap_or_default(),
        content: row.get(2).unwrap_or_default(),
        description: row.get(3).unwrap_or(None),
        category: row.get(4).unwrap_or_else(|_| "custom".to_string()),
        tags: serde_json::from_str(&tags_str).unwrap_or_default(),
        variables: serde_json::from_str(&variables_str).unwrap_or_default(),
        is_builtin: row.get::<_, i32>(7).unwrap_or(0) != 0,
        is_pinned: row.get::<_, i32>(8).unwrap_or(0) != 0,
        use_count: row.get::<_, u32>(9).unwrap_or(0),
        last_used_at: row.get(10).unwrap_or(None),
        created_at: row.get(11).unwrap_or(None),
        updated_at: row.get(12).unwrap_or(None),
    }
}

use rusqlite::OptionalExtension;
