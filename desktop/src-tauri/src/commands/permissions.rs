//! Permission Commands
//!
//! Tauri commands for managing tool execution permissions.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::models::response::CommandResponse;
use crate::services::orchestrator::permission_gate::PermissionGate;
use crate::services::orchestrator::permissions::{PermissionLevel, PermissionResponse};

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
