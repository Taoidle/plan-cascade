//! Guardrail Commands
//!
//! Tauri commands for managing guardrail security rules and viewing trigger logs.

use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::services::guardrail::{
    CustomGuardrail, GuardrailAction, GuardrailInfo, GuardrailRegistry, TriggerLogEntry,
};

/// Tauri-managed state for the guardrail registry.
pub struct GuardrailState {
    pub registry: Arc<RwLock<Option<GuardrailRegistry>>>,
}

impl GuardrailState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(None)),
        }
    }
}

impl Default for GuardrailState {
    fn default() -> Self {
        Self::new()
    }
}

/// List all guardrails with their name, type, and enabled status.
#[tauri::command]
pub async fn list_guardrails(
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<Vec<GuardrailInfo>>, String> {
    let guard = state.registry.read().await;
    match &*guard {
        Some(registry) => Ok(CommandResponse::ok(registry.list_guardrails())),
        None => Ok(CommandResponse::err("Guardrail registry not initialized")),
    }
}

/// Toggle a guardrail on or off by name.
#[tauri::command]
pub async fn toggle_guardrail(
    name: String,
    enabled: bool,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<bool>, String> {
    let mut guard = state.registry.write().await;
    match &mut *guard {
        Some(registry) => {
            let success = registry.set_enabled(&name, enabled);
            if success {
                Ok(CommandResponse::ok(true))
            } else {
                Ok(CommandResponse::err(format!(
                    "Guardrail '{}' not found",
                    name
                )))
            }
        }
        None => Ok(CommandResponse::err("Guardrail registry not initialized")),
    }
}

/// Add a new custom guardrail rule.
#[tauri::command]
pub async fn add_custom_rule(
    name: String,
    pattern: String,
    action: String,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<GuardrailInfo>, String> {
    let parsed_action = match GuardrailAction::parse(&action) {
        Some(a) => a,
        None => {
            return Ok(CommandResponse::err(format!(
                "Invalid action '{}'. Must be 'warn', 'block', or 'redact'",
                action
            )));
        }
    };

    let id = uuid::Uuid::new_v4().to_string();

    let guardrail = match CustomGuardrail::new(id.clone(), name.clone(), &pattern, parsed_action) {
        Some(g) => g,
        None => {
            return Ok(CommandResponse::err(format!(
                "Invalid regex pattern: '{}'",
                pattern
            )));
        }
    };

    let mut guard = state.registry.write().await;
    match &mut *guard {
        Some(registry) => {
            // Persist to database
            registry.save_custom_rule_to_db(&id, &name, &pattern, &action, true);

            let info = GuardrailInfo {
                name: name.clone(),
                guardrail_type: "custom".to_string(),
                enabled: true,
                description: "User-defined guardrail rule".to_string(),
            };

            registry.add_custom(guardrail, true);
            Ok(CommandResponse::ok(info))
        }
        None => Ok(CommandResponse::err("Guardrail registry not initialized")),
    }
}

/// Remove a custom guardrail rule by name.
#[tauri::command]
pub async fn remove_custom_rule(
    name: String,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<bool>, String> {
    let mut guard = state.registry.write().await;
    match &mut *guard {
        Some(registry) => {
            let removed = registry.remove_custom(&name);
            if removed {
                // Also remove from database (using name as we don't have id here)
                // The registry handles DB deletion internally if needed
                Ok(CommandResponse::ok(true))
            } else {
                Ok(CommandResponse::err(format!(
                    "Custom rule '{}' not found",
                    name
                )))
            }
        }
        None => Ok(CommandResponse::err("Guardrail registry not initialized")),
    }
}

/// Get paginated trigger log entries.
#[tauri::command]
pub async fn get_trigger_log(
    limit: Option<usize>,
    offset: Option<usize>,
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<Vec<TriggerLogEntry>>, String> {
    let guard = state.registry.read().await;
    match &*guard {
        Some(registry) => {
            let limit = limit.unwrap_or(50);
            let offset = offset.unwrap_or(0);
            let entries = registry.get_trigger_log(limit, offset);
            Ok(CommandResponse::ok(entries))
        }
        None => Ok(CommandResponse::err("Guardrail registry not initialized")),
    }
}

/// Clear all trigger log entries.
#[tauri::command]
pub async fn clear_trigger_log(
    state: State<'_, GuardrailState>,
) -> Result<CommandResponse<bool>, String> {
    let guard = state.registry.read().await;
    match &*guard {
        Some(registry) => {
            let success = registry.clear_trigger_log();
            Ok(CommandResponse::ok(success))
        }
        None => Ok(CommandResponse::err("Guardrail registry not initialized")),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guardrail_state_new() {
        let state = GuardrailState::new();
        // Registry starts as None
        let guard = state.registry.try_read().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn test_guardrail_state_default() {
        let state = GuardrailState::default();
        let guard = state.registry.try_read().unwrap();
        assert!(guard.is_none());
    }
}
