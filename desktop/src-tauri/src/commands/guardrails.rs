//! Guardrail Commands
//!
//! Tauri IPC for listing, editing, and auditing runtime guardrails.

use std::sync::Arc;

use tauri::State;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::services::guardrail::{
    shared_guardrail_registry, CustomRuleConfig, Direction, GuardrailAction, GuardrailEventEntry,
    GuardrailInfo, GuardrailMode, GuardrailRegistry,
};
use crate::storage::Database;

/// Tauri-managed handle to the shared guardrail registry.
pub struct GuardrailState {
    pub registry: Arc<RwLock<GuardrailRegistry>>,
}

impl GuardrailState {
    pub fn new() -> Self {
        Self {
            registry: shared_guardrail_registry(),
        }
    }

    pub async fn initialize(&self, database: Arc<Database>) -> Result<(), String> {
        self.registry
            .write()
            .await
            .initialize_with_database(database)
    }
}

impl Default for GuardrailState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GuardrailRuntimeStatus {
    pub mode: GuardrailMode,
    pub strict_mode: bool,
    pub native_runtime_managed: bool,
    pub claude_code_managed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GuardrailOverview {
    pub guardrails: Vec<GuardrailInfo>,
    pub runtime: GuardrailRuntimeStatus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomGuardrailInput {
    pub id: Option<String>,
    pub name: String,
    pub pattern: String,
    pub action: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub scope: Vec<Direction>,
    #[serde(default)]
    pub description: String,
}

fn parse_custom_rule_input(input: CustomGuardrailInput) -> Result<CustomRuleConfig, String> {
    let action = GuardrailAction::parse(&input.action).ok_or_else(|| {
        format!(
            "Invalid action '{}'. Must be 'warn', 'block', or 'redact'",
            input.action
        )
    })?;

    Ok(CustomRuleConfig {
        id: input.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        name: input.name,
        pattern: input.pattern,
        action,
        enabled: input.enabled,
        scope: input.scope,
        description: input.description,
    })
}

#[tauri::command]
pub async fn list_guardrails(
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<GuardrailOverview>, String> {
    let registry = state.registry.read().await;
    Ok(CommandResponse::ok(GuardrailOverview {
        guardrails: registry.list_guardrails(),
        runtime: GuardrailRuntimeStatus {
            mode: registry.mode(),
            strict_mode: registry.strict_mode(),
            native_runtime_managed: registry.native_runtime_managed(),
            claude_code_managed: registry.claude_code_managed(),
            init_error: registry.init_error(),
        },
    }))
}

#[tauri::command]
pub async fn set_guardrail_mode(
    mode: String,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<GuardrailRuntimeStatus>, String> {
    let parsed_mode = GuardrailMode::parse(&mode)
        .ok_or_else(|| format!("Invalid guardrail mode '{}'", mode))?;
    let mut registry = state.registry.write().await;
    match registry.set_mode(parsed_mode) {
        Ok(applied_mode) => Ok(CommandResponse::ok(GuardrailRuntimeStatus {
            mode: applied_mode,
            strict_mode: applied_mode == GuardrailMode::Strict,
            native_runtime_managed: registry.native_runtime_managed(),
            claude_code_managed: registry.claude_code_managed(),
            init_error: registry.init_error(),
        })),
        Err(error) => Ok(CommandResponse::err(error)),
    }
}

#[tauri::command]
pub async fn toggle_guardrail(
    id: String,
    enabled: bool,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<GuardrailInfo>, String> {
    let mut registry = state.registry.write().await;
    match registry.set_enabled(&id, enabled) {
        Ok(info) => Ok(CommandResponse::ok(info)),
        Err(error) => Ok(CommandResponse::err(error)),
    }
}

#[tauri::command]
pub async fn create_custom_guardrail(
    rule: CustomGuardrailInput,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<GuardrailInfo>, String> {
    let mut registry = state.registry.write().await;
    match parse_custom_rule_input(rule).and_then(|config| registry.create_custom_rule(config)) {
        Ok(info) => Ok(CommandResponse::ok(info)),
        Err(error) => Ok(CommandResponse::err(error)),
    }
}

#[tauri::command]
pub async fn update_guardrail(
    rule: CustomGuardrailInput,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<GuardrailInfo>, String> {
    let mut registry = state.registry.write().await;
    match parse_custom_rule_input(rule).and_then(|config| registry.update_custom_rule(config)) {
        Ok(info) => Ok(CommandResponse::ok(info)),
        Err(error) => Ok(CommandResponse::err(error)),
    }
}

#[tauri::command]
pub async fn delete_guardrail(
    id: String,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<bool>, String> {
    let mut registry = state.registry.write().await;
    match registry.delete_guardrail(&id) {
        Ok(deleted) => Ok(CommandResponse::ok(deleted)),
        Err(error) => Ok(CommandResponse::err(error)),
    }
}

#[tauri::command]
pub async fn list_guardrail_events(
    limit: Option<usize>,
    offset: Option<usize>,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<Vec<GuardrailEventEntry>>, String> {
    let registry = state.registry.read().await;
    Ok(CommandResponse::ok(
        registry.get_events(limit.unwrap_or(50), offset.unwrap_or(0)),
    ))
}

#[tauri::command]
pub async fn clear_guardrail_events(
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<bool>, String> {
    let registry = state.registry.read().await;
    Ok(CommandResponse::ok(registry.clear_events()))
}

// Compatibility wrappers for the existing frontend while it migrates.
#[tauri::command]
pub async fn add_custom_rule(
    name: String,
    pattern: String,
    action: String,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<GuardrailInfo>, String> {
    create_custom_guardrail(
        CustomGuardrailInput {
            id: None,
            name,
            pattern,
            action,
            enabled: true,
            scope: Vec::new(),
            description: String::new(),
        },
        state,
    )
    .await
}

#[tauri::command]
pub async fn remove_custom_rule(
    id: String,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<bool>, String> {
    delete_guardrail(id, state).await
}

#[tauri::command]
pub async fn get_trigger_log(
    limit: Option<usize>,
    offset: Option<usize>,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<Vec<GuardrailEventEntry>>, String> {
    list_guardrail_events(limit, offset, state).await
}

#[tauri::command]
pub async fn clear_trigger_log(
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<bool>, String> {
    clear_guardrail_events(state).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guardrail_state_uses_shared_registry() {
        let state = GuardrailState::new();
        let other = GuardrailState::new();
        assert!(Arc::ptr_eq(&state.registry, &other.registry));
    }

    #[test]
    fn parse_custom_input_defaults_id_and_action() {
        let parsed = parse_custom_rule_input(
            CustomGuardrailInput {
                id: None,
                name: "Block TODO".to_string(),
                pattern: "TODO".to_string(),
                action: "block".to_string(),
                enabled: true,
                scope: vec![Direction::Input],
                description: String::new(),
            },
        )
        .unwrap();
        assert_eq!(parsed.name, "Block TODO");
        assert_eq!(parsed.action, GuardrailAction::Block);
        assert!(!parsed.id.is_empty());
    }
}
