//! Permission Commands
//!
//! Tauri commands for managing tool execution permissions.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::models::response::CommandResponse;
use crate::services::orchestrator::permission_gate::PermissionGate;
use crate::services::orchestrator::permissions::{
    builtin_network_domain_allowlist, builtin_network_domain_allowlist_available_versions,
    builtin_network_domain_allowlist_version, PermissionLevel, PermissionPolicyConfig,
    PermissionResponse,
};

/// Tauri managed state for the global permission gate singleton.
pub struct PermissionState {
    pub gate: Arc<PermissionGate>,
}

impl PermissionState {
    pub fn new() -> Self {
        Self {
            gate: Arc::new(PermissionGate::new()),
        }
    }
}

impl Default for PermissionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Request payload for setting a session's permission level.
#[derive(Debug, Deserialize)]
pub struct SetPermissionLevelRequest {
    pub session_id: String,
    pub level: PermissionLevel,
}

/// Request payload for responding to a tool permission request.
#[derive(Debug, Deserialize)]
pub struct RespondPermissionRequest {
    pub request_id: String,
    pub allowed: bool,
    pub always_allow: bool,
}

/// Request payload for updating Policy v2 config.
#[derive(Debug, Deserialize)]
pub struct SetPolicyConfigRequest {
    /// Auto-allow Bash network calls only for these allowlisted domains.
    pub network_domain_allowlist: Vec<String>,
}

/// Permission policy config payload returned to frontend.
#[derive(Debug, Serialize)]
pub struct PermissionPolicyConfigResponse {
    /// Custom allowlist configured by user.
    pub network_domain_allowlist: Vec<String>,
    /// Built-in allowlist shipped with app.
    pub builtin_network_domain_allowlist: Vec<String>,
    /// Current built-in allowlist version id.
    pub builtin_network_domain_allowlist_version: String,
    /// All built-in allowlist versions known by this app build.
    pub builtin_network_domain_allowlist_available_versions: Vec<String>,
}

/// Set the permission level for a session.
#[tauri::command]
pub async fn set_session_permission_level(
    state: tauri::State<'_, PermissionState>,
    request: SetPermissionLevelRequest,
) -> Result<CommandResponse<()>, String> {
    state
        .gate
        .set_session_level(&request.session_id, request.level)
        .await;
    Ok(CommandResponse::ok(()))
}

/// Get the permission level for a session.
#[tauri::command]
pub async fn get_session_permission_level(
    state: tauri::State<'_, PermissionState>,
    session_id: String,
) -> Result<CommandResponse<PermissionLevel>, String> {
    let level = state.gate.get_session_level(&session_id).await;
    Ok(CommandResponse::ok(level))
}

/// Respond to a tool permission request (Allow / Deny / Always Allow).
#[tauri::command]
pub async fn respond_tool_permission(
    state: tauri::State<'_, PermissionState>,
    request: RespondPermissionRequest,
) -> Result<CommandResponse<()>, String> {
    state
        .gate
        .resolve(
            &request.request_id,
            PermissionResponse {
                request_id: request.request_id.clone(),
                allowed: request.allowed,
                always_allow: request.always_allow,
            },
        )
        .await;
    Ok(CommandResponse::ok(()))
}

/// Get current Policy v2 config.
#[tauri::command]
pub async fn get_permission_policy_config(
    state: tauri::State<'_, PermissionState>,
) -> Result<CommandResponse<PermissionPolicyConfigResponse>, String> {
    let config = state.gate.get_policy_config().await;
    Ok(CommandResponse::ok(PermissionPolicyConfigResponse {
        network_domain_allowlist: config.network_domain_allowlist,
        builtin_network_domain_allowlist: builtin_network_domain_allowlist()
            .iter()
            .map(|s| s.to_string())
            .collect(),
        builtin_network_domain_allowlist_version: builtin_network_domain_allowlist_version()
            .to_string(),
        builtin_network_domain_allowlist_available_versions:
            builtin_network_domain_allowlist_available_versions()
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
    }))
}

/// Replace Policy v2 config.
#[tauri::command]
pub async fn set_permission_policy_config(
    state: tauri::State<'_, PermissionState>,
    request: SetPolicyConfigRequest,
) -> Result<CommandResponse<()>, String> {
    state
        .gate
        .set_policy_config(PermissionPolicyConfig {
            network_domain_allowlist: request.network_domain_allowlist,
        })
        .await;
    Ok(CommandResponse::ok(()))
}
