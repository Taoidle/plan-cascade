//! Guardrail Registry
//!
//! Manages a collection of guardrails (built-in + custom) with enable/disable
//! support and hooks integration.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::services::orchestrator::hooks::AgenticHooks;
use crate::storage::database::Database;

use super::{
    CodeSecurityGuardrail, CustomGuardrail, Direction, Guardrail, GuardrailAction, GuardrailInfo,
    GuardrailResult, SchemaValidationGuardrail, SensitiveDataGuardrail, TriggerLogEntry,
};

/// Managed guardrail entry with enabled state.
struct GuardrailEntry {
    guardrail: Box<dyn Guardrail>,
    enabled: bool,
    guardrail_type: String,
}

/// Registry managing all guardrails with enable/disable and validation.
pub struct GuardrailRegistry {
    entries: Vec<GuardrailEntry>,
    /// Optional database for persisting custom rules and trigger logs
    database: Option<Arc<Database>>,
}

impl GuardrailRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            database: None,
        }
    }

    /// Create a registry with default built-in guardrails enabled.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.add_builtin(Box::new(SensitiveDataGuardrail::new()), true);
        registry.add_builtin(Box::new(CodeSecurityGuardrail::new()), true);
        registry.add_builtin(Box::new(SchemaValidationGuardrail::new()), true);
        registry
    }

    /// Set the database for persistence.
    pub fn set_database(&mut self, db: Arc<Database>) {
        self.database = Some(db);
    }

    /// Add a built-in guardrail.
    pub fn add_builtin(&mut self, guardrail: Box<dyn Guardrail>, enabled: bool) {
        self.entries.push(GuardrailEntry {
            guardrail,
            enabled,
            guardrail_type: "builtin".to_string(),
        });
    }

    /// Add a custom guardrail.
    pub fn add_custom(&mut self, guardrail: CustomGuardrail, enabled: bool) {
        self.entries.push(GuardrailEntry {
            guardrail: Box::new(guardrail),
            enabled,
            guardrail_type: "custom".to_string(),
        });
    }

    /// Remove a custom guardrail by name.
    pub fn remove_custom(&mut self, name: &str) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|e| !(e.guardrail_type == "custom" && e.guardrail.name() == name));
        self.entries.len() < before
    }

    /// Enable or disable a guardrail by name.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> bool {
        for entry in &mut self.entries {
            if entry.guardrail.name() == name {
                entry.enabled = enabled;
                return true;
            }
        }
        false
    }

    /// Check if a guardrail is enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.entries
            .iter()
            .find(|e| e.guardrail.name() == name)
            .map(|e| e.enabled)
            .unwrap_or(false)
    }

    /// List all guardrails with their info.
    pub fn list_guardrails(&self) -> Vec<GuardrailInfo> {
        self.entries
            .iter()
            .map(|e| GuardrailInfo {
                name: e.guardrail.name().to_string(),
                guardrail_type: e.guardrail_type.clone(),
                enabled: e.enabled,
                description: e.guardrail.description().to_string(),
            })
            .collect()
    }

    /// Validate content against all enabled guardrails for the given direction.
    ///
    /// Returns the most severe result:
    /// - Block takes precedence over Redact/Warn/Pass
    /// - Redact takes precedence over Warn/Pass
    /// - Warn takes precedence over Pass
    pub async fn validate_all(&self, content: &str, direction: Direction) -> GuardrailResult {
        let mut most_severe = GuardrailResult::Pass;
        let mut combined_warnings = Vec::new();
        let mut all_redacted_items = Vec::new();

        for entry in &self.entries {
            if !entry.enabled {
                continue;
            }

            let result = entry.guardrail.validate(content, direction).await;

            // Log trigger if not pass
            if !result.is_pass() {
                self.log_trigger(entry.guardrail.name(), direction, &result, content);
            }

            match &result {
                GuardrailResult::Block { .. } => {
                    // Block is highest severity - return immediately
                    return result;
                }
                GuardrailResult::Redact {
                    redacted_content,
                    redacted_items,
                } => {
                    all_redacted_items.extend(redacted_items.clone());
                    most_severe = GuardrailResult::Redact {
                        redacted_content: redacted_content.clone(),
                        redacted_items: all_redacted_items.clone(),
                    };
                }
                GuardrailResult::Warn { message } => {
                    combined_warnings.push(message.clone());
                    if !most_severe.is_redact() {
                        most_severe = GuardrailResult::Warn {
                            message: combined_warnings.join("; "),
                        };
                    }
                }
                GuardrailResult::Pass => {}
            }
        }

        most_severe
    }

    /// Log a guardrail trigger event to the database (if available).
    fn log_trigger(
        &self,
        guardrail_name: &str,
        direction: Direction,
        result: &GuardrailResult,
        content: &str,
    ) {
        let db = match &self.database {
            Some(db) => db,
            None => return,
        };

        let snippet = if content.len() > 100 {
            format!("{}...", &content[..100])
        } else {
            content.to_string()
        };

        let conn = match db.get_connection() {
            Ok(c) => c,
            Err(_) => return,
        };

        let _ = conn.execute(
            "INSERT INTO guardrail_trigger_log (guardrail_name, direction, result_type, content_snippet, timestamp)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            rusqlite::params![
                guardrail_name,
                direction.to_string(),
                result.result_type(),
                snippet,
            ],
        );
    }

    /// Get trigger log entries with pagination.
    pub fn get_trigger_log(&self, limit: usize, offset: usize) -> Vec<TriggerLogEntry> {
        let db = match &self.database {
            Some(db) => db,
            None => return Vec::new(),
        };

        let conn = match db.get_connection() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut stmt = match conn.prepare(
            "SELECT id, guardrail_name, direction, result_type, content_snippet, timestamp
             FROM guardrail_trigger_log
             ORDER BY timestamp DESC
             LIMIT ?1 OFFSET ?2",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        stmt.query_map(rusqlite::params![limit as i64, offset as i64], |row| {
            Ok(TriggerLogEntry {
                id: row.get(0)?,
                guardrail_name: row.get(1)?,
                direction: row.get(2)?,
                result_type: row.get(3)?,
                content_snippet: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Clear all trigger log entries.
    pub fn clear_trigger_log(&self) -> bool {
        let db = match &self.database {
            Some(db) => db,
            None => return false,
        };

        let conn = match db.get_connection() {
            Ok(c) => c,
            Err(_) => return false,
        };

        conn.execute("DELETE FROM guardrail_trigger_log", [])
            .is_ok()
    }

    /// Save a custom rule to the database.
    pub fn save_custom_rule_to_db(
        &self,
        id: &str,
        name: &str,
        pattern: &str,
        action: &str,
        enabled: bool,
    ) -> bool {
        let db = match &self.database {
            Some(db) => db,
            None => return false,
        };

        let conn = match db.get_connection() {
            Ok(c) => c,
            Err(_) => return false,
        };

        conn.execute(
            "INSERT OR REPLACE INTO guardrail_rules (id, name, pattern, action, enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            rusqlite::params![id, name, pattern, action, enabled as i32],
        )
        .is_ok()
    }

    /// Delete a custom rule from the database.
    pub fn delete_custom_rule_from_db(&self, id: &str) -> bool {
        let db = match &self.database {
            Some(db) => db,
            None => return false,
        };

        let conn = match db.get_connection() {
            Ok(c) => c,
            Err(_) => return false,
        };

        conn.execute(
            "DELETE FROM guardrail_rules WHERE id = ?1",
            rusqlite::params![id],
        )
        .is_ok()
    }

    /// Load custom rules from the database and add them to the registry.
    pub fn load_custom_rules_from_db(&mut self) {
        let db = match &self.database {
            Some(db) => db.clone(),
            None => return,
        };

        let conn = match db.get_connection() {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut stmt =
            match conn.prepare("SELECT id, name, pattern, action, enabled FROM guardrail_rules") {
                Ok(s) => s,
                Err(_) => return,
            };

        let rules: Vec<(String, String, String, String, bool)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i32>(4)? != 0,
                ))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        for (id, name, pattern, action_str, enabled) in rules {
            let action = GuardrailAction::parse(&action_str).unwrap_or(GuardrailAction::Warn);
            if let Some(guardrail) = CustomGuardrail::new(id, name, &pattern, action) {
                self.add_custom(guardrail, enabled);
            }
        }
    }
}

impl Default for GuardrailRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Register guardrail hooks into the agentic lifecycle.
///
/// This wires the `GuardrailRegistry` into:
/// - `on_user_message`: validates user input, may redact or block
/// - `on_after_tool`: validates tool output, may warn
pub fn register_guardrail_hooks(
    hooks: &mut AgenticHooks,
    registry: Arc<RwLock<GuardrailRegistry>>,
) {
    // on_user_message: validate user input (Input direction)
    let registry_input = registry.clone();
    hooks.register_on_user_message(Box::new(move |_ctx, msg| {
        let reg = registry_input.clone();
        Box::pin(async move {
            let guard = reg.read().await;
            let result = guard.validate_all(&msg, Direction::Input).await;

            match result {
                GuardrailResult::Block { reason } => {
                    eprintln!("[guardrail] Input blocked: {}", reason);
                    // Return a modified message indicating the block
                    Ok(Some(format!(
                        "[GUARDRAIL BLOCKED] Your message was blocked: {}",
                        reason
                    )))
                }
                GuardrailResult::Redact {
                    redacted_content, ..
                } => {
                    eprintln!("[guardrail] Input redacted");
                    Ok(Some(redacted_content))
                }
                GuardrailResult::Warn { message } => {
                    eprintln!("[guardrail] Input warning: {}", message);
                    Ok(None) // Don't modify, just log
                }
                GuardrailResult::Pass => Ok(None),
            }
        })
    }));

    // on_after_tool: validate tool output (Tool direction)
    let registry_tool = registry;
    hooks.register_on_after_tool(Box::new(
        move |_ctx, tool_name, _success, output_snippet| {
            let reg = registry_tool.clone();
            Box::pin(async move {
                if let Some(ref output) = output_snippet {
                    let guard = reg.read().await;
                    let result = guard.validate_all(output, Direction::Tool).await;

                    match result {
                        GuardrailResult::Warn { message } => {
                            eprintln!(
                                "[guardrail] Tool '{}' output warning: {}",
                                tool_name, message
                            );
                        }
                        GuardrailResult::Redact { .. } => {
                            eprintln!(
                                "[guardrail] Tool '{}' output contained sensitive data",
                                tool_name
                            );
                        }
                        _ => {}
                    }
                }
                Ok(crate::services::orchestrator::hooks::AfterToolResult::default())
            })
        },
    ));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_with_defaults() {
        let registry = GuardrailRegistry::with_defaults();
        let list = registry.list_guardrails();
        assert_eq!(list.len(), 3);
        assert!(list.iter().any(|g| g.name == "SensitiveData"));
        assert!(list.iter().any(|g| g.name == "CodeSecurity"));
        assert!(list.iter().any(|g| g.name == "SchemaValidation"));
    }

    #[test]
    fn test_enable_disable() {
        let mut registry = GuardrailRegistry::with_defaults();
        assert!(registry.is_enabled("SensitiveData"));

        registry.set_enabled("SensitiveData", false);
        assert!(!registry.is_enabled("SensitiveData"));

        registry.set_enabled("SensitiveData", true);
        assert!(registry.is_enabled("SensitiveData"));
    }

    #[test]
    fn test_add_and_remove_custom() {
        let mut registry = GuardrailRegistry::new();
        let custom = CustomGuardrail::new(
            "r1".to_string(),
            "TestRule".to_string(),
            r"test",
            GuardrailAction::Warn,
        )
        .unwrap();
        registry.add_custom(custom, true);

        let list = registry.list_guardrails();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "TestRule");
        assert_eq!(list[0].guardrail_type, "custom");

        assert!(registry.remove_custom("TestRule"));
        assert!(registry.list_guardrails().is_empty());
    }

    #[tokio::test]
    async fn test_validate_all_pass() {
        let registry = GuardrailRegistry::with_defaults();
        let result = registry
            .validate_all("normal safe content", Direction::Input)
            .await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn test_validate_all_redact_input() {
        let registry = GuardrailRegistry::with_defaults();
        let result = registry
            .validate_all(
                "My key: sk-abcdefghijklmnopqrstuvwxyz1234567890123456",
                Direction::Input,
            )
            .await;
        assert!(result.is_redact());
    }

    #[tokio::test]
    async fn test_validate_all_warn_output() {
        let registry = GuardrailRegistry::with_defaults();
        let result = registry
            .validate_all(r#"eval("something")"#, Direction::Output)
            .await;
        assert!(result.is_warn());
    }

    #[tokio::test]
    async fn test_validate_all_block() {
        let mut registry = GuardrailRegistry::new();
        let custom = CustomGuardrail::new(
            "block-test".to_string(),
            "BlockAll".to_string(),
            r"blocked_word",
            GuardrailAction::Block,
        )
        .unwrap();
        registry.add_custom(custom, true);

        let result = registry
            .validate_all("this has blocked_word in it", Direction::Input)
            .await;
        assert!(result.is_block());
    }

    #[tokio::test]
    async fn test_disabled_guardrail_skipped() {
        let mut registry = GuardrailRegistry::with_defaults();
        registry.set_enabled("SensitiveData", false);

        let result = registry
            .validate_all(
                "sk-abcdefghijklmnopqrstuvwxyz1234567890123456",
                Direction::Input,
            )
            .await;
        // SensitiveData is disabled, so it should pass
        assert!(result.is_pass());
    }

    #[test]
    fn test_register_guardrail_hooks_adds_hooks() {
        let mut hooks = AgenticHooks::new();
        let registry = Arc::new(RwLock::new(GuardrailRegistry::with_defaults()));
        register_guardrail_hooks(&mut hooks, registry);

        // Should add 2 hooks: on_user_message + on_after_tool
        assert_eq!(hooks.total_hooks(), 2);
    }

    #[tokio::test]
    async fn test_hooks_integration_with_redact() {
        let mut hooks = AgenticHooks::new();
        let registry = Arc::new(RwLock::new(GuardrailRegistry::with_defaults()));
        register_guardrail_hooks(&mut hooks, registry);

        let ctx = crate::services::orchestrator::hooks::HookContext {
            session_id: "test".to_string(),
            project_path: std::path::PathBuf::from("/tmp"),
            provider_name: "test".to_string(),
            model_name: "test".to_string(),
        };

        // Input with API key should be redacted
        let result = hooks
            .fire_on_user_message(
                &ctx,
                "Use key sk-abcdefghijklmnopqrstuvwxyz1234567890123456".to_string(),
            )
            .await;
        assert!(result.contains("[REDACTED:OpenAI API Key]"));
    }

    #[tokio::test]
    async fn test_hooks_integration_pass_through() {
        let mut hooks = AgenticHooks::new();
        let registry = Arc::new(RwLock::new(GuardrailRegistry::with_defaults()));
        register_guardrail_hooks(&mut hooks, registry);

        let ctx = crate::services::orchestrator::hooks::HookContext {
            session_id: "test".to_string(),
            project_path: std::path::PathBuf::from("/tmp"),
            provider_name: "test".to_string(),
            model_name: "test".to_string(),
        };

        // Safe input should pass through unchanged
        let result = hooks
            .fire_on_user_message(&ctx, "normal message".to_string())
            .await;
        assert_eq!(result, "normal message");
    }

    #[test]
    fn test_set_enabled_nonexistent() {
        let mut registry = GuardrailRegistry::new();
        assert!(!registry.set_enabled("NonExistent", true));
    }

    #[test]
    fn test_is_enabled_nonexistent() {
        let registry = GuardrailRegistry::new();
        assert!(!registry.is_enabled("NonExistent"));
    }
}
