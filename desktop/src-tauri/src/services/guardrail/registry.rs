//! Guardrail Registry
//!
//! Central runtime registry for built-in and custom guardrails. The registry is
//! shared by settings IPC, native execution flows, and debug artifact writes.

use std::sync::{Arc, OnceLock};

use rusqlite::params;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

use crate::services::orchestrator::hooks::{
    AfterLlmResult, AfterToolResult, AgenticHooks, BeforeToolResult, UserMessageHookResult,
};
use crate::storage::database::Database;

use super::{
    CodeSecurityGuardrail, CustomGuardrail, CustomRuleConfig, Direction, Guardrail,
    GuardrailAction, GuardrailEventEntry, GuardrailInfo, GuardrailResult, GuardrailRuntimeContext,
    SchemaValidationGuardrail, SensitiveDataGuardrail,
};

static GLOBAL_GUARDRAIL_REGISTRY: OnceLock<Arc<RwLock<GuardrailRegistry>>> = OnceLock::new();

fn table_has_column(conn: &rusqlite::Connection, table: &str, column: &str) -> bool {
    let sql = format!("PRAGMA table_info({})", table);
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) {
            return rows.flatten().any(|name| name == column);
        }
    }
    false
}

fn truncate_for_preview(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        trimmed.to_string()
    } else {
        format!("{}...", trimmed.chars().take(limit).collect::<String>())
    }
}

fn default_custom_scopes() -> Vec<Direction> {
    vec![Direction::Input, Direction::Output, Direction::Tool]
}

fn serialize_scopes(scope: &[Direction]) -> String {
    serde_json::to_string(scope).unwrap_or_else(|_| {
        serde_json::to_string(&default_custom_scopes()).unwrap_or_else(|_| "[]".to_string())
    })
}

fn deserialize_scopes(raw: Option<String>) -> Vec<Direction> {
    raw.and_then(|value| serde_json::from_str::<Vec<Direction>>(&value).ok())
        .filter(|scope| !scope.is_empty())
        .unwrap_or_else(default_custom_scopes)
}

fn sha256_hex(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

struct GuardrailEntry {
    guardrail: Box<dyn Guardrail>,
    enabled: bool,
    scope: Vec<Direction>,
    action: String,
    guardrail_type: String,
    editable: bool,
    builtin_key: Option<String>,
    pattern: Option<String>,
}

impl GuardrailEntry {
    fn info(&self) -> GuardrailInfo {
        GuardrailInfo {
            id: self.guardrail.id().to_string(),
            name: self.guardrail.name().to_string(),
            guardrail_type: self.guardrail_type.clone(),
            builtin_key: self.builtin_key.clone(),
            pattern: self.pattern.clone(),
            enabled: self.enabled,
            scope: self.scope.clone(),
            action: self.action.clone(),
            editable: self.editable,
            description: self.guardrail.description().to_string(),
        }
    }

    fn applies_to(&self, surface: Direction) -> bool {
        self.scope.contains(&surface)
    }
}

/// Registry managing all runtime guardrails and their sanitized audit trail.
pub struct GuardrailRegistry {
    entries: Vec<GuardrailEntry>,
    database: Option<Arc<Database>>,
    strict_mode: bool,
    native_runtime_managed: bool,
    claude_code_managed: bool,
    init_error: Option<String>,
}

impl GuardrailRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            database: None,
            strict_mode: true,
            native_runtime_managed: false,
            claude_code_managed: false,
            init_error: None,
        }
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.add_builtin(Box::new(SensitiveDataGuardrail::new()), true);
        registry.add_builtin(Box::new(CodeSecurityGuardrail::new()), true);
        registry.add_builtin(Box::new(SchemaValidationGuardrail::new()), true);
        registry
    }

    pub fn strict_mode(&self) -> bool {
        self.strict_mode
    }

    pub fn native_runtime_managed(&self) -> bool {
        self.native_runtime_managed
    }

    pub fn claude_code_managed(&self) -> bool {
        self.claude_code_managed
    }

    pub fn init_error(&self) -> Option<String> {
        self.init_error.clone()
    }

    pub fn initialize_with_database(&mut self, database: Arc<Database>) -> Result<(), String> {
        self.database = Some(database);
        if let Err(error) = self.ensure_database_layout() {
            self.init_error = Some(error.clone());
            return Err(error);
        }
        if let Err(error) = self.sync_from_database() {
            self.init_error = Some(error.clone());
            return Err(error);
        }
        self.native_runtime_managed = true;
        self.init_error = None;
        Ok(())
    }

    pub fn add_builtin(&mut self, guardrail: Box<dyn Guardrail>, enabled: bool) {
        self.entries.push(GuardrailEntry {
            enabled,
            scope: guardrail.default_scopes(),
            action: guardrail.default_action_label().to_string(),
            guardrail_type: "builtin".to_string(),
            editable: guardrail.editable(),
            builtin_key: guardrail.builtin_key().map(ToOwned::to_owned),
            pattern: None,
            guardrail,
        });
    }

    fn add_custom(
        &mut self,
        guardrail: CustomGuardrail,
        enabled: bool,
        scope: Vec<Direction>,
        description: String,
    ) {
        let pattern = Some(guardrail.pattern().to_string());
        let action = guardrail.action().to_string();
        let mut entry = GuardrailEntry {
            enabled,
            scope,
            action,
            guardrail_type: "custom".to_string(),
            editable: true,
            builtin_key: None,
            pattern,
            guardrail: Box::new(guardrail),
        };
        if !description.is_empty() {
            entry.guardrail_type = "custom".to_string();
        }
        self.entries.push(entry);
    }

    fn upsert_builtin_row(&self, entry: &GuardrailEntry) -> Result<(), String> {
        let Some(db) = &self.database else {
            return Ok(());
        };
        let conn = db.get_connection().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO guardrail_rules
             (id, name, guardrail_type, builtin_key, pattern, action, scope, enabled, editable, description, created_at, updated_at)
             VALUES (?1, ?2, 'builtin', ?3, NULL, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now'))
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               guardrail_type = excluded.guardrail_type,
               builtin_key = excluded.builtin_key,
               action = excluded.action,
               scope = excluded.scope,
               editable = excluded.editable,
               description = excluded.description",
            params![
                entry.guardrail.id(),
                entry.guardrail.name(),
                entry.builtin_key.as_deref(),
                entry.action,
                serialize_scopes(&entry.scope),
                entry.enabled as i32,
                entry.editable as i32,
                entry.guardrail.description(),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn ensure_database_layout(&self) -> Result<(), String> {
        let Some(db) = &self.database else {
            return Ok(());
        };
        let conn = db.get_connection().map_err(|e| e.to_string())?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS guardrail_rules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                guardrail_type TEXT NOT NULL DEFAULT 'custom',
                builtin_key TEXT,
                pattern TEXT,
                action TEXT NOT NULL DEFAULT 'warn',
                scope TEXT NOT NULL DEFAULT '[\"input\",\"assistant_output\",\"tool_result\"]',
                enabled INTEGER NOT NULL DEFAULT 1,
                editable INTEGER NOT NULL DEFAULT 1,
                description TEXT NOT NULL DEFAULT '',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .map_err(|e| e.to_string())?;

        if !table_has_column(&conn, "guardrail_rules", "guardrail_type") {
            conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN guardrail_type TEXT NOT NULL DEFAULT 'custom'",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        if !table_has_column(&conn, "guardrail_rules", "builtin_key") {
            conn.execute("ALTER TABLE guardrail_rules ADD COLUMN builtin_key TEXT", [])
                .map_err(|e| e.to_string())?;
        }
        if !table_has_column(&conn, "guardrail_rules", "scope") {
            conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN scope TEXT NOT NULL DEFAULT '[\"input\",\"assistant_output\",\"tool_result\"]'",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        if !table_has_column(&conn, "guardrail_rules", "editable") {
            conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN editable INTEGER NOT NULL DEFAULT 1",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        if !table_has_column(&conn, "guardrail_rules", "description") {
            conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN description TEXT NOT NULL DEFAULT ''",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        if !table_has_column(&conn, "guardrail_rules", "updated_at") {
            conn.execute(
                "ALTER TABLE guardrail_rules ADD COLUMN updated_at TEXT DEFAULT CURRENT_TIMESTAMP",
                [],
            )
            .map_err(|e| e.to_string())?;
        }

        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_guardrail_rules_builtin_key
             ON guardrail_rules(builtin_key)
             WHERE builtin_key IS NOT NULL",
            [],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS guardrail_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rule_id TEXT NOT NULL,
                rule_name TEXT NOT NULL,
                surface TEXT NOT NULL,
                tool_name TEXT,
                session_id TEXT,
                execution_id TEXT,
                decision TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                safe_preview TEXT NOT NULL DEFAULT '',
                timestamp TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_guardrail_events_timestamp
             ON guardrail_events(timestamp DESC)",
            [],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_guardrail_events_rule_id
             ON guardrail_events(rule_id)",
            [],
        )
        .map_err(|e| e.to_string())?;

        let _ = conn.execute("DELETE FROM guardrail_trigger_log", []);
        Ok(())
    }

    fn sync_from_database(&mut self) -> Result<(), String> {
        for index in 0..self.entries.len() {
            let entry = &self.entries[index];
            if entry.guardrail_type == "builtin" {
                self.upsert_builtin_row(entry)?;
            }
        }

        self.entries.retain(|entry| entry.guardrail_type == "builtin");

        let Some(db) = &self.database else {
            return Ok(());
        };
        let conn = db.get_connection().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, guardrail_type, builtin_key, pattern, action, scope, enabled, editable, description
                 FROM guardrail_rules
                 ORDER BY CASE guardrail_type WHEN 'builtin' THEN 0 ELSE 1 END, name ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows: Vec<(String, String, String, Option<String>, Option<String>, String, Option<String>, bool, bool, String)> =
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, i32>(7)? != 0,
                    row.get::<_, i32>(8)? != 0,
                    row.get::<_, String>(9)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();

        for (id, name, kind, builtin_key, pattern, action, scope_json, enabled, editable, description) in rows {
            if kind == "builtin" {
                if let Some(entry) = self.entries.iter_mut().find(|entry| {
                    entry.guardrail_type == "builtin"
                        && (entry.builtin_key.as_deref() == builtin_key.as_deref()
                            || entry.guardrail.id() == id)
                }) {
                    entry.enabled = enabled;
                    entry.scope = deserialize_scopes(scope_json);
                    entry.action = action;
                    entry.editable = editable;
                }
                continue;
            }

            let Some(pattern) = pattern else {
                continue;
            };
            let parsed_action = GuardrailAction::parse(&action).unwrap_or(GuardrailAction::Warn);
            let scope = deserialize_scopes(scope_json);
            if let Some(guardrail) = CustomGuardrail::new_with_description(
                id,
                name,
                &pattern,
                parsed_action,
                description.clone(),
            ) {
                self.add_custom(guardrail, enabled, scope, description);
            }
        }

        Ok(())
    }

    pub fn list_guardrails(&self) -> Vec<GuardrailInfo> {
        self.entries.iter().map(GuardrailEntry::info).collect()
    }

    pub fn create_custom_rule(&mut self, config: CustomRuleConfig) -> Result<GuardrailInfo, String> {
        let guardrail = CustomGuardrail::new_with_description(
            config.id.clone(),
            config.name.clone(),
            &config.pattern,
            config.action,
            if config.description.is_empty() {
                "User-defined guardrail rule".to_string()
            } else {
                config.description.clone()
            },
        )
        .ok_or_else(|| format!("Invalid regex pattern: '{}'", config.pattern))?;

        let Some(db) = &self.database else {
            return Err("Guardrail database is not available".to_string());
        };
        let conn = db.get_connection().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO guardrail_rules
             (id, name, guardrail_type, builtin_key, pattern, action, scope, enabled, editable, description, created_at, updated_at)
             VALUES (?1, ?2, 'custom', NULL, ?3, ?4, ?5, ?6, 1, ?7, datetime('now'), datetime('now'))",
            params![
                config.id,
                config.name,
                config.pattern,
                config.action.to_string(),
                serialize_scopes(&config.scope),
                config.enabled as i32,
                if config.description.is_empty() {
                    "User-defined guardrail rule".to_string()
                } else {
                    config.description.clone()
                },
            ],
        )
        .map_err(|e| e.to_string())?;

        self.add_custom(
            guardrail,
            config.enabled,
            if config.scope.is_empty() {
                default_custom_scopes()
            } else {
                config.scope
            },
            config.description,
        );
        self.list_guardrails()
            .into_iter()
            .find(|entry| entry.id == config.id)
            .ok_or_else(|| "Created custom rule but failed to read it back".to_string())
    }

    pub fn update_custom_rule(&mut self, config: CustomRuleConfig) -> Result<GuardrailInfo, String> {
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.guardrail.id() == config.id && entry.guardrail_type == "custom")
        else {
            return Err(format!("Guardrail '{}' not found", config.id));
        };

        let guardrail = CustomGuardrail::new_with_description(
            config.id.clone(),
            config.name.clone(),
            &config.pattern,
            config.action,
            if config.description.is_empty() {
                "User-defined guardrail rule".to_string()
            } else {
                config.description.clone()
            },
        )
        .ok_or_else(|| format!("Invalid regex pattern: '{}'", config.pattern))?;

        let Some(db) = &self.database else {
            return Err("Guardrail database is not available".to_string());
        };
        let conn = db.get_connection().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE guardrail_rules
             SET name = ?2,
                 pattern = ?3,
                 action = ?4,
                 scope = ?5,
                 enabled = ?6,
                 editable = 1,
                 description = ?7,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
                config.id,
                config.name,
                config.pattern,
                config.action.to_string(),
                serialize_scopes(&config.scope),
                config.enabled as i32,
                if config.description.is_empty() {
                    "User-defined guardrail rule".to_string()
                } else {
                    config.description.clone()
                },
            ],
        )
        .map_err(|e| e.to_string())?;

        self.entries[index] = GuardrailEntry {
            enabled: config.enabled,
            scope: if config.scope.is_empty() {
                default_custom_scopes()
            } else {
                config.scope
            },
            action: config.action.to_string(),
            guardrail_type: "custom".to_string(),
            editable: true,
            builtin_key: None,
            pattern: Some(config.pattern),
            guardrail: Box::new(guardrail),
        };
        Ok(self.entries[index].info())
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<GuardrailInfo, String> {
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.guardrail.id() == id) else {
            return Err(format!("Guardrail '{}' not found", id));
        };
        entry.enabled = enabled;
        if let Some(db) = &self.database {
            let conn = db.get_connection().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE guardrail_rules SET enabled = ?2, updated_at = datetime('now') WHERE id = ?1",
                params![id, enabled as i32],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(entry.info())
    }

    pub fn delete_guardrail(&mut self, id: &str) -> Result<bool, String> {
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.guardrail.id() == id && entry.guardrail_type == "custom")
        else {
            return Err(format!("Custom rule '{}' not found", id));
        };
        self.entries.remove(index);
        if let Some(db) = &self.database {
            let conn = db.get_connection().map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM guardrail_rules WHERE id = ?1", params![id])
                .map_err(|e| e.to_string())?;
        }
        Ok(true)
    }

    pub fn get_events(&self, limit: usize, offset: usize) -> Vec<GuardrailEventEntry> {
        let Some(db) = &self.database else {
            return Vec::new();
        };
        let Ok(conn) = db.get_connection() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, rule_id, rule_name, surface, tool_name, session_id, execution_id, decision, content_hash, safe_preview, timestamp
             FROM guardrail_events
             ORDER BY timestamp DESC
             LIMIT ?1 OFFSET ?2",
        ) else {
            return Vec::new();
        };

        stmt.query_map(params![limit as i64, offset as i64], |row| {
            Ok(GuardrailEventEntry {
                id: row.get(0)?,
                rule_id: row.get(1)?,
                rule_name: row.get(2)?,
                surface: row.get(3)?,
                tool_name: row.get(4)?,
                session_id: row.get(5)?,
                execution_id: row.get(6)?,
                decision: row.get(7)?,
                content_hash: row.get(8)?,
                safe_preview: row.get(9)?,
                timestamp: row.get(10)?,
            })
        })
        .ok()
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default()
    }

    pub fn clear_events(&self) -> bool {
        let Some(db) = &self.database else {
            return false;
        };
        let Ok(conn) = db.get_connection() else {
            return false;
        };
        conn.execute("DELETE FROM guardrail_events", []).is_ok()
    }

    fn log_event(
        &self,
        entry: &GuardrailEntry,
        surface: Direction,
        content: &str,
        result: &GuardrailResult,
        runtime: &GuardrailRuntimeContext,
    ) {
        let Some(db) = &self.database else {
            return;
        };
        let Ok(conn) = db.get_connection() else {
            return;
        };

        let preview = match result {
            GuardrailResult::Redact {
                redacted_content, ..
            } => redacted_content.clone(),
            _ => entry
                .guardrail
                .redact_preview(content)
                .unwrap_or_else(|| content.to_string()),
        };
        let safe_preview = truncate_for_preview(&preview, 160);

        let _ = conn.execute(
            "INSERT INTO guardrail_events
             (rule_id, rule_name, surface, tool_name, session_id, execution_id, decision, content_hash, safe_preview, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))",
            params![
                entry.guardrail.id(),
                entry.guardrail.name(),
                surface.to_string(),
                runtime.tool_name.as_deref(),
                runtime.session_id.as_deref(),
                runtime.execution_id.as_deref(),
                result.result_type(),
                sha256_hex(content),
                safe_preview,
            ],
        );
    }

    pub async fn validate_all(
        &self,
        content: &str,
        surface: Direction,
        runtime: &GuardrailRuntimeContext,
    ) -> GuardrailResult {
        let mut current_content = content.to_string();
        let mut warnings = Vec::new();
        let mut redacted_items = Vec::new();

        for entry in &self.entries {
            if !entry.enabled || !entry.applies_to(surface) {
                continue;
            }

            let result = entry
                .guardrail
                .validate(&current_content, surface, runtime)
                .await;

            if !result.is_pass() {
                self.log_event(entry, surface, &current_content, &result, runtime);
            }

            match result {
                GuardrailResult::Pass => {}
                GuardrailResult::Warn { message } => {
                    warnings.push(format!("{}: {}", entry.guardrail.name(), message));
                }
                GuardrailResult::Redact {
                    redacted_content,
                    redacted_items: items,
                } => {
                    current_content = redacted_content;
                    redacted_items.extend(items);
                }
                GuardrailResult::Block { reason } => {
                    return GuardrailResult::Block { reason };
                }
            }
        }

        if !redacted_items.is_empty() {
            GuardrailResult::Redact {
                redacted_content: current_content,
                redacted_items,
            }
        } else if !warnings.is_empty() {
            GuardrailResult::Warn {
                message: warnings.join("; "),
            }
        } else {
            GuardrailResult::Pass
        }
    }
}

impl Default for GuardrailRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn shared_guardrail_registry() -> Arc<RwLock<GuardrailRegistry>> {
    GLOBAL_GUARDRAIL_REGISTRY
        .get_or_init(|| Arc::new(RwLock::new(GuardrailRegistry::with_defaults())))
        .clone()
}

pub fn register_guardrail_hooks(
    hooks: &mut AgenticHooks,
    registry: Arc<RwLock<GuardrailRegistry>>,
) {
    let registry_input = registry.clone();
    hooks.register_on_user_message(Box::new(move |ctx, message| {
        let registry = registry_input.clone();
        Box::pin(async move {
            let runtime = GuardrailRuntimeContext {
                session_id: Some(ctx.session_id.clone()),
                execution_id: ctx.execution_id.clone(),
                tool_name: None,
                content_kind: ctx.task_type.clone(),
            };
            let result = registry
                .read()
                .await
                .validate_all(&message, Direction::Input, &runtime)
                .await;

            let output = match result {
                GuardrailResult::Pass | GuardrailResult::Warn { .. } => UserMessageHookResult {
                    modified_message: Some(message),
                    stop_reason: None,
                },
                GuardrailResult::Redact {
                    redacted_content, ..
                } => UserMessageHookResult {
                    modified_message: Some(redacted_content),
                    stop_reason: None,
                },
                GuardrailResult::Block { reason } => UserMessageHookResult {
                    modified_message: Some(message),
                    stop_reason: Some(reason),
                },
            };
            Ok(output)
        })
    }));

    let registry_before_tool = registry.clone();
    hooks.register_on_before_tool(Box::new(move |ctx, tool_name, arguments| {
        let registry = registry_before_tool.clone();
        Box::pin(async move {
            let runtime = GuardrailRuntimeContext {
                session_id: Some(ctx.session_id.clone()),
                execution_id: ctx.execution_id.clone(),
                tool_name: Some(tool_name.clone()),
                content_kind: None,
            };
            let result = registry
                .read()
                .await
                .validate_all(&arguments, Direction::ToolCall, &runtime)
                .await;

            let output = match result {
                GuardrailResult::Pass | GuardrailResult::Warn { .. } => BeforeToolResult::default(),
                GuardrailResult::Redact {
                    redacted_content, ..
                } => BeforeToolResult {
                    skip: false,
                    skip_reason: None,
                    modified_arguments: Some(redacted_content),
                },
                GuardrailResult::Block { reason } => BeforeToolResult {
                    skip: true,
                    skip_reason: Some(reason),
                    modified_arguments: None,
                },
            };
            Ok(output)
        })
    }));

    let registry_after_tool = registry.clone();
    hooks.register_on_after_tool(Box::new(move |ctx, tool_name, _success, output_snippet| {
        let registry = registry_after_tool.clone();
        Box::pin(async move {
            let Some(output) = output_snippet else {
                return Ok(AfterToolResult::default());
            };
            let runtime = GuardrailRuntimeContext {
                session_id: Some(ctx.session_id.clone()),
                execution_id: ctx.execution_id.clone(),
                tool_name: Some(tool_name),
                content_kind: ctx.task_type.clone(),
            };
            let result = registry
                .read()
                .await
                .validate_all(&output, Direction::Tool, &runtime)
                .await;

            let output = match result {
                GuardrailResult::Pass | GuardrailResult::Warn { .. } => AfterToolResult::default(),
                GuardrailResult::Redact {
                    redacted_content, ..
                } => AfterToolResult {
                    injected_context: None,
                    replacement_output: Some(redacted_content),
                    block_reason: None,
                },
                GuardrailResult::Block { reason } => AfterToolResult {
                    injected_context: None,
                    replacement_output: None,
                    block_reason: Some(reason),
                },
            };
            Ok(output)
        })
    }));

    hooks.register_on_after_llm(Box::new(move |ctx, response_text| {
        let registry = registry.clone();
        Box::pin(async move {
            let Some(response_text) = response_text else {
                return Ok(AfterLlmResult::default());
            };
            let runtime = GuardrailRuntimeContext {
                session_id: Some(ctx.session_id.clone()),
                execution_id: ctx.execution_id.clone(),
                tool_name: None,
                content_kind: ctx.task_type.clone(),
            };
            let result = registry
                .read()
                .await
                .validate_all(&response_text, Direction::Output, &runtime)
                .await;

            let output = match result {
                GuardrailResult::Pass | GuardrailResult::Warn { .. } => AfterLlmResult::default(),
                GuardrailResult::Redact {
                    redacted_content, ..
                } => AfterLlmResult {
                    replacement_text: Some(redacted_content),
                    block_reason: None,
                },
                GuardrailResult::Block { reason } => AfterLlmResult {
                    replacement_text: None,
                    block_reason: Some(reason),
                },
            };
            Ok(output)
        })
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn validate_all_redacts_sensitive_input() {
        let registry = GuardrailRegistry::with_defaults();
        let result = registry
            .validate_all(
                "secret sk-abcdefghijklmnopqrstuvwxyz123456789012345678",
                Direction::Input,
                &GuardrailRuntimeContext::default(),
            )
            .await;
        assert!(matches!(result, GuardrailResult::Redact { .. }));
    }

    #[tokio::test]
    async fn validate_all_blocks_tool_call_secrets() {
        let registry = GuardrailRegistry::with_defaults();
        let result = registry
            .validate_all(
                r#"{"token":"sk-abcdefghijklmnopqrstuvwxyz123456789012345678"}"#,
                Direction::ToolCall,
                &GuardrailRuntimeContext::default(),
            )
            .await;
        assert!(matches!(result, GuardrailResult::Block { .. }));
    }

    #[test]
    fn shared_registry_is_singleton() {
        let a = shared_guardrail_registry();
        let b = shared_guardrail_registry();
        assert!(Arc::ptr_eq(&a, &b));
    }
}
