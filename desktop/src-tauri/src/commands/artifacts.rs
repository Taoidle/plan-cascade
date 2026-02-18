//! Artifact Commands
//!
//! Tauri commands for versioned artifact storage operations.

use serde::Deserialize;
use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::artifacts::{ArtifactMeta, ArtifactScope, ArtifactVersion};

/// Tauri-managed state for the artifact service.
pub struct ArtifactState {
    pub _initialized: bool,
}

impl ArtifactState {
    pub fn new() -> Self {
        Self {
            _initialized: false,
        }
    }
}

impl Default for ArtifactState {
    fn default() -> Self {
        Self::new()
    }
}

/// Save an artifact.
#[tauri::command]
pub async fn artifact_save(
    name: String,
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
    content_type: String,
    data: Vec<u8>,
) -> Result<CommandResponse<ArtifactMeta>, String> {
    Ok(CommandResponse::err(format!(
        "Artifact service not yet initialized. Save requested for '{}' in project '{}'",
        name, project_id
    )))
}

/// Load an artifact (latest or specific version).
#[tauri::command]
pub async fn artifact_load(
    name: String,
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
    version: Option<u32>,
) -> Result<CommandResponse<Vec<u8>>, String> {
    Ok(CommandResponse::err(format!(
        "Artifact service not yet initialized. Load requested for '{}' in project '{}'",
        name, project_id
    )))
}

/// List artifacts in a scope.
#[tauri::command]
pub async fn artifact_list(
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
) -> Result<CommandResponse<Vec<ArtifactMeta>>, String> {
    Ok(CommandResponse::err(format!(
        "Artifact service not yet initialized for project '{}'",
        project_id
    )))
}

/// List all versions of a named artifact.
#[tauri::command]
pub async fn artifact_versions(
    name: String,
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
) -> Result<CommandResponse<Vec<ArtifactVersion>>, String> {
    Ok(CommandResponse::err(format!(
        "Artifact service not yet initialized. Versions requested for '{}' in project '{}'",
        name, project_id
    )))
}

/// Delete an artifact and all its versions.
#[tauri::command]
pub async fn artifact_delete(
    name: String,
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
) -> Result<CommandResponse<bool>, String> {
    Ok(CommandResponse::err(format!(
        "Artifact service not yet initialized. Delete requested for '{}' in project '{}'",
        name, project_id
    )))
}
