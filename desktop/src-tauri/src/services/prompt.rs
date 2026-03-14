//! Prompt Service
//!
//! Business logic for managing prompt templates.

use regex::Regex;
use std::cmp::Reverse;
use uuid::Uuid;

use crate::models::prompt::{PromptCreateRequest, PromptTemplate, PromptUpdateRequest};
use crate::storage::database::DbPool;
use crate::storage::Database;
use crate::utils::error::{AppError, AppResult};

use rusqlite::OptionalExtension;

#[derive(Clone, Copy)]
struct BuiltinPromptCatalogEntry {
    id: &'static str,
    title: &'static str,
    category: &'static str,
    content: &'static str,
    description: &'static str,
}

const BUILTIN_PROMPTS_EN: &[BuiltinPromptCatalogEntry] = &[
    BuiltinPromptCatalogEntry {
        id: "builtin-code-review",
        title: "Code Review",
        category: "coding",
        content: "Review this code for bugs, performance, and best practices:\n\n{{code}}",
        description: "Analyze code for issues and improvements",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-explain-code",
        title: "Explain Code",
        category: "coding",
        content: "Explain what this code does in simple terms:\n\n{{code}}",
        description: "Get a clear explanation of code",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-refactor",
        title: "Refactor",
        category: "coding",
        content: "Refactor this code to improve readability and maintainability:\n\n{{code}}",
        description: "Improve code structure",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-write-tests",
        title: "Write Tests",
        category: "coding",
        content: "Write comprehensive unit tests for:\n\n{{code}}",
        description: "Generate test cases",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-fix-bug",
        title: "Fix Bug",
        category: "coding",
        content: "Find and fix the bug:\n\n{{code}}\n\nError: {{error}}",
        description: "Debug and fix issues",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-explain-error",
        title: "Explain Error",
        category: "coding",
        content: "Explain this error and suggest a fix:\n\n{{error}}",
        description: "Understand error messages",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-summarize",
        title: "Summarize",
        category: "writing",
        content: "Summarize concisely:\n\n{{text}}",
        description: "Create concise summaries",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-improve-writing",
        title: "Improve Writing",
        category: "writing",
        content: "Improve clarity and flow:\n\n{{text}}",
        description: "Enhance writing quality",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-translate",
        title: "Translate",
        category: "writing",
        content: "Translate to {{language}}:\n\n{{text}}",
        description: "Translate text to another language",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-analyze",
        title: "Analyze",
        category: "analysis",
        content: "Analyze and provide key insights:\n\n{{content}}",
        description: "Extract key insights from content",
    },
];

const BUILTIN_PROMPTS_ZH: &[BuiltinPromptCatalogEntry] = &[
    BuiltinPromptCatalogEntry {
        id: "builtin-code-review",
        title: "代码审查",
        category: "coding",
        content: "请审查下面的代码，找出 bug、性能问题和最佳实践改进点：\n\n{{code}}",
        description: "分析代码中的问题与改进建议",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-explain-code",
        title: "解释代码",
        category: "coding",
        content: "请用易懂的方式解释这段代码的作用：\n\n{{code}}",
        description: "清晰解释代码的功能",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-refactor",
        title: "重构代码",
        category: "coding",
        content: "请重构这段代码，以提升可读性和可维护性：\n\n{{code}}",
        description: "改进代码结构",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-write-tests",
        title: "编写测试",
        category: "coding",
        content: "请为以下代码编写全面的单元测试：\n\n{{code}}",
        description: "生成测试用例",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-fix-bug",
        title: "修复 Bug",
        category: "coding",
        content: "请定位并修复这个 bug：\n\n{{code}}\n\n错误信息：{{error}}",
        description: "调试并修复问题",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-explain-error",
        title: "解释错误",
        category: "coding",
        content: "请解释这个错误并给出修复建议：\n\n{{error}}",
        description: "帮助理解错误信息",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-summarize",
        title: "总结内容",
        category: "writing",
        content: "请简洁总结以下内容：\n\n{{text}}",
        description: "生成简明摘要",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-improve-writing",
        title: "润色文本",
        category: "writing",
        content: "请提升这段文本的清晰度和表达流畅度：\n\n{{text}}",
        description: "优化写作质量",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-translate",
        title: "翻译文本",
        category: "writing",
        content: "请将以下内容翻译成 {{language}}：\n\n{{text}}",
        description: "翻译文本到目标语言",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-analyze",
        title: "内容分析",
        category: "analysis",
        content: "请分析以下内容并给出关键洞察：\n\n{{content}}",
        description: "提炼内容中的关键信息",
    },
];

const BUILTIN_PROMPTS_JA: &[BuiltinPromptCatalogEntry] = &[
    BuiltinPromptCatalogEntry {
        id: "builtin-code-review",
        title: "コードレビュー",
        category: "coding",
        content: "次のコードをレビューし、バグ、性能上の問題、ベストプラクティスの改善点を指摘してください:\n\n{{code}}",
        description: "コードの問題点と改善案を分析します",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-explain-code",
        title: "コードを説明",
        category: "coding",
        content: "このコードが何をしているのか、わかりやすく説明してください:\n\n{{code}}",
        description: "コードの動作を明確に説明します",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-refactor",
        title: "リファクタリング",
        category: "coding",
        content: "このコードを読みやすく保守しやすい形にリファクタリングしてください:\n\n{{code}}",
        description: "コード構造を改善します",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-write-tests",
        title: "テストを書く",
        category: "coding",
        content: "次のコードに対して包括的な単体テストを書いてください:\n\n{{code}}",
        description: "テストケースを生成します",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-fix-bug",
        title: "バグ修正",
        category: "coding",
        content: "このバグを見つけて修正してください:\n\n{{code}}\n\nエラー: {{error}}",
        description: "不具合を調査して修正します",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-explain-error",
        title: "エラーを説明",
        category: "coding",
        content: "このエラーを説明し、修正案を提案してください:\n\n{{error}}",
        description: "エラーメッセージを理解します",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-summarize",
        title: "要約する",
        category: "writing",
        content: "次の内容を簡潔に要約してください:\n\n{{text}}",
        description: "簡潔な要約を作成します",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-improve-writing",
        title: "文章を改善",
        category: "writing",
        content: "次の文章を、より明確で自然な表現に改善してください:\n\n{{text}}",
        description: "文章品質を向上させます",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-translate",
        title: "翻訳する",
        category: "writing",
        content: "次の内容を {{language}} に翻訳してください:\n\n{{text}}",
        description: "別の言語へ翻訳します",
    },
    BuiltinPromptCatalogEntry {
        id: "builtin-analyze",
        title: "分析する",
        category: "analysis",
        content: "次の内容を分析し、重要な洞察をまとめてください:\n\n{{content}}",
        description: "重要な洞察を抽出します",
    },
];

fn normalize_prompt_locale(locale: Option<&str>) -> &'static str {
    let normalized = locale.unwrap_or("en").trim().to_ascii_lowercase();
    if normalized.starts_with("zh") {
        "zh"
    } else if normalized.starts_with("ja") {
        "ja"
    } else {
        "en"
    }
}

fn builtin_prompt_catalog(locale: Option<&str>) -> &'static [BuiltinPromptCatalogEntry] {
    match normalize_prompt_locale(locale) {
        "zh" => BUILTIN_PROMPTS_ZH,
        "ja" => BUILTIN_PROMPTS_JA,
        _ => BUILTIN_PROMPTS_EN,
    }
}

fn builtin_prompt_entry(id: &str, locale: Option<&str>) -> Option<BuiltinPromptCatalogEntry> {
    builtin_prompt_catalog(locale)
        .iter()
        .find(|entry| entry.id == id)
        .copied()
        .or_else(|| {
            BUILTIN_PROMPTS_EN
                .iter()
                .find(|entry| entry.id == id)
                .copied()
        })
}

fn matches_search(prompt: &PromptTemplate, search: Option<&str>) -> bool {
    let Some(search) = search.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    let query = search.to_ascii_lowercase();
    prompt.title.to_ascii_lowercase().contains(&query)
        || prompt.category.to_ascii_lowercase().contains(&query)
        || prompt.content.to_ascii_lowercase().contains(&query)
        || prompt
            .description
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains(&query)
}

fn normalize_prompt_category(category: &str) -> String {
    let normalized = category.trim();
    if normalized.eq_ignore_ascii_case("custom") {
        String::new()
    } else {
        normalized.to_string()
    }
}

fn localize_builtin_prompt(mut prompt: PromptTemplate, locale: Option<&str>) -> PromptTemplate {
    if !prompt.is_builtin {
        return prompt;
    }
    if let Some(entry) = builtin_prompt_entry(&prompt.id, locale) {
        prompt.title = entry.title.to_string();
        prompt.category = entry.category.to_string();
        prompt.content = entry.content.to_string();
        prompt.description = Some(entry.description.to_string());
        prompt.variables = extract_variables(entry.content);
    }
    prompt
}

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

        for builtin in BUILTIN_PROMPTS_EN {
            let variables = extract_variables(builtin.content);
            let variables_json =
                serde_json::to_string(&variables).unwrap_or_else(|_| "[]".to_string());

            conn.execute(
                "INSERT OR IGNORE INTO prompts (id, title, content, description, category, tags, variables, is_builtin, is_pinned, use_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, '[]', ?6, 1, 0, 0)",
                rusqlite::params![
                    builtin.id,
                    builtin.title,
                    builtin.content,
                    builtin.description,
                    builtin.category,
                    variables_json
                ],
            )?;
        }

        Ok(())
    }

    /// List prompts with optional category filter and search
    pub fn list_prompts(
        &self,
        category: Option<&str>,
        search: Option<&str>,
        locale: Option<&str>,
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
            let normalized_category = normalize_prompt_category(cat);
            if normalized_category.is_empty() {
                sql.push_str(" AND (trim(category) = '' OR lower(trim(category)) = 'custom')");
            } else {
                sql.push_str(" AND category = ?");
                params_vec.push(Box::new(normalized_category));
            }
        }

        sql.push_str(" ORDER BY is_pinned DESC, use_count DESC, title ASC");

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| Ok(row_to_prompt(row)))?;

        let mut prompts = Vec::new();
        for row in rows {
            prompts.push(localize_builtin_prompt(row?, locale));
        }

        prompts.retain(|prompt| matches_search(prompt, search));
        prompts.sort_by_key(|prompt| {
            (
                !prompt.is_pinned,
                Reverse(prompt.use_count),
                prompt.title.to_ascii_lowercase(),
            )
        });
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

    pub fn get_prompt_localized(
        &self,
        id: &str,
        locale: Option<&str>,
    ) -> AppResult<Option<PromptTemplate>> {
        Ok(self
            .get_prompt(id)?
            .map(|prompt| localize_builtin_prompt(prompt, locale)))
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
                normalize_prompt_category(&req.category),
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
        let category = normalize_prompt_category(&req.category.unwrap_or(existing.category));
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
    pub fn toggle_pin(&self, id: &str, locale: Option<&str>) -> AppResult<PromptTemplate> {
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

        self.get_prompt_localized(id, locale)?
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
        category: normalize_prompt_category(&row.get::<_, String>(4).unwrap_or_default()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_service() -> PromptService {
        let db = Database::new_in_memory().unwrap();
        PromptService::from_database(&db)
    }

    #[test]
    fn list_prompts_localizes_builtins_by_locale() {
        let service = create_service();
        service.seed_builtins().unwrap();

        let prompts = service.list_prompts(None, None, Some("zh-CN")).unwrap();
        let review = prompts
            .into_iter()
            .find(|prompt| prompt.id == "builtin-code-review")
            .unwrap();

        assert_eq!(review.title, "代码审查");
        assert!(review.content.contains("请审查下面的代码"));
    }

    #[test]
    fn list_prompts_keeps_custom_prompt_content_stable() {
        let service = create_service();
        let created = service
            .create_prompt(PromptCreateRequest {
                title: "Custom".to_string(),
                content: "Keep this custom text".to_string(),
                description: Some("desc".to_string()),
                category: "".to_string(),
                tags: vec![],
                is_pinned: false,
            })
            .unwrap();

        let prompts = service.list_prompts(None, None, Some("ja")).unwrap();
        let custom = prompts
            .into_iter()
            .find(|prompt| prompt.id == created.id)
            .unwrap();

        assert_eq!(custom.title, "Custom");
        assert!(custom.category.is_empty());
        assert_eq!(custom.content, "Keep this custom text");
    }
}
