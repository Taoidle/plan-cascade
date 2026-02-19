//! Artifact Commands
//!
//! Tauri commands for versioned artifact storage operations.
//!
//! Uses lazy initialization: the `DefaultArtifactService` is constructed on
//! first use from the application's Database instance.  The artifact storage
//! root is placed under the platform-appropriate data directory
//! (`~/.plan-cascade/artifacts/`).

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::services::artifacts::{
    ArtifactMeta, ArtifactScope, ArtifactService, ArtifactVersion, DefaultArtifactService,
};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// ArtifactState
// ---------------------------------------------------------------------------

/// Tauri-managed state for the artifact service.
///
/// Follows the same `Arc<RwLock<Option<Arc<T>>>>` lazy-init pattern used by
/// `KnowledgeState` and other domain states.
pub struct ArtifactState {
    service: Arc<RwLock<Option<Arc<DefaultArtifactService>>>>,
}

impl ArtifactState {
    /// Create a new uninitialized ArtifactState.
    pub fn new() -> Self {
        Self {
            service: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the artifact service from a Database instance.
    ///
    /// The storage root is `~/.plan-cascade/artifacts/`.  Subsequent calls are
    /// no-ops if already initialized.
    pub async fn initialize(&self, service: DefaultArtifactService) {
        let mut guard = self.service.write().await;
        *guard = Some(Arc::new(service));
    }

    /// Get the initialized service, or an error if not yet initialized.
    pub async fn get_service(&self) -> Result<Arc<DefaultArtifactService>, String> {
        let guard = self.service.read().await;
        guard.clone().ok_or_else(|| {
            "Artifact service not initialized. Please initialize the app first.".to_string()
        })
    }

    /// Check whether the service has been initialized.
    pub async fn is_initialized(&self) -> bool {
        let guard = self.service.read().await;
        guard.is_some()
    }
}

impl Default for ArtifactState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Response payload for `artifact_load` containing both metadata and binary data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactLoadResponse {
    /// Artifact metadata.
    pub meta: ArtifactMeta,
    /// Raw artifact bytes.
    pub data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Lazy initialization helper
// ---------------------------------------------------------------------------

/// Ensure the artifact service is initialized, using AppState's database.
///
/// On first call, clones the Database from AppState and constructs a
/// `DefaultArtifactService` with the storage root at
/// `~/.plan-cascade/artifacts/`.
async fn ensure_initialized(
    artifact_state: &ArtifactState,
    app_state: &AppState,
) -> Result<(), String> {
    if artifact_state.is_initialized().await {
        return Ok(());
    }

    let db = app_state
        .with_database(|db| Ok(Arc::new(db.clone())))
        .await
        .map_err(|e| format!("Failed to access database: {}", e))?;

    let storage_root = crate::utils::paths::plan_cascade_dir()
        .map(|p| p.join("artifacts"))
        .map_err(|e| format!("Failed to determine artifact storage path: {}", e))?;

    // Ensure the storage root directory exists
    std::fs::create_dir_all(&storage_root)
        .map_err(|e| format!("Failed to create artifact storage directory: {}", e))?;

    let service = DefaultArtifactService::new(db, &storage_root)
        .map_err(|e| format!("Failed to initialize artifact service: {}", e))?;

    artifact_state.initialize(service).await;
    Ok(())
}

/// Build an `ArtifactScope` from the individual parameters passed by the frontend.
fn build_scope(
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
) -> ArtifactScope {
    ArtifactScope {
        project_id,
        session_id,
        user_id,
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Save an artifact.
#[tauri::command]
pub async fn artifact_save(
    artifact_state: State<'_, ArtifactState>,
    app_state: State<'_, AppState>,
    name: String,
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
    content_type: String,
    data: Vec<u8>,
) -> Result<CommandResponse<ArtifactMeta>, String> {
    ensure_initialized(&artifact_state, &app_state).await?;

    let service = match artifact_state.get_service().await {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let scope = build_scope(project_id, session_id, user_id);

    match service.save(&name, &scope, &content_type, &data).await {
        Ok(meta) => Ok(CommandResponse::ok(meta)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Load an artifact (latest or specific version).
#[tauri::command]
pub async fn artifact_load(
    artifact_state: State<'_, ArtifactState>,
    app_state: State<'_, AppState>,
    name: String,
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
    version: Option<u32>,
) -> Result<CommandResponse<ArtifactLoadResponse>, String> {
    ensure_initialized(&artifact_state, &app_state).await?;

    let service = match artifact_state.get_service().await {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let scope = build_scope(project_id, session_id, user_id);

    match service.load(&name, &scope, version).await {
        Ok((meta, data)) => Ok(CommandResponse::ok(ArtifactLoadResponse { meta, data })),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List artifacts in a scope.
#[tauri::command]
pub async fn artifact_list(
    artifact_state: State<'_, ArtifactState>,
    app_state: State<'_, AppState>,
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
) -> Result<CommandResponse<Vec<ArtifactMeta>>, String> {
    ensure_initialized(&artifact_state, &app_state).await?;

    let service = match artifact_state.get_service().await {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let scope = build_scope(project_id, session_id, user_id);

    match service.list(&scope).await {
        Ok(list) => Ok(CommandResponse::ok(list)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List all versions of a named artifact.
#[tauri::command]
pub async fn artifact_versions(
    artifact_state: State<'_, ArtifactState>,
    app_state: State<'_, AppState>,
    name: String,
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
) -> Result<CommandResponse<Vec<ArtifactVersion>>, String> {
    ensure_initialized(&artifact_state, &app_state).await?;

    let service = match artifact_state.get_service().await {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let scope = build_scope(project_id, session_id, user_id);

    match service.versions(&name, &scope).await {
        Ok(versions) => Ok(CommandResponse::ok(versions)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete an artifact and all its versions.
#[tauri::command]
pub async fn artifact_delete(
    artifact_state: State<'_, ArtifactState>,
    app_state: State<'_, AppState>,
    name: String,
    project_id: String,
    session_id: Option<String>,
    user_id: Option<String>,
) -> Result<CommandResponse<bool>, String> {
    ensure_initialized(&artifact_state, &app_state).await?;

    let service = match artifact_state.get_service().await {
        Ok(s) => s,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let scope = build_scope(project_id, session_id, user_id);

    match service.delete(&name, &scope).await {
        Ok(()) => Ok(CommandResponse::ok(true)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}
