//! Worktree Commands
//!
//! Tauri commands for Git worktree management.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::commands::workflow::{emit_kernel_update_for_session, emit_session_catalog_update};
use crate::models::response::CommandResponse;
use crate::models::worktree::{
    CompleteWorktreeResult, CreateManagedWorktreeRequest, CreatePullRequestRequest,
    CreatePullRequestResult, CreateWorktreeRequest, ForgeProvider, PreparePullRequestResult,
    SessionWorktreeRuntimeBinding, Worktree, WorktreeCleanupPolicy, WorktreeRuntimeKind,
    WorktreeStatus,
};
use crate::services::workflow_kernel::{
    HandoffContextBundle, SessionRuntimeInfo, WorkflowKernelState, WorkflowMode,
    WorkflowRuntimeKind,
};
use crate::services::worktree::WorktreeManager;
use crate::state::AppState;

/// State for the worktree service
pub struct WorktreeState {
    pub(crate) manager: Arc<RwLock<WorktreeManager>>,
}

fn forge_token_key(provider: &ForgeProvider) -> &'static str {
    match provider {
        ForgeProvider::Github => "forge_github",
        ForgeProvider::Gitlab => "forge_gitlab",
        ForgeProvider::Gitea => "forge_gitea",
    }
}

impl WorktreeState {
    /// Create a new worktree state
    pub fn new() -> Self {
        Self {
            manager: Arc::new(RwLock::new(WorktreeManager::new())),
        }
    }
}

impl Default for WorktreeState {
    fn default() -> Self {
        Self::new()
    }
}

fn binding_to_runtime(binding: &SessionWorktreeRuntimeBinding) -> SessionRuntimeInfo {
    SessionRuntimeInfo {
        root_path: Some(binding.root_path.clone()),
        runtime_path: Some(binding.runtime_path.clone()),
        runtime_kind: match binding.runtime_kind {
            WorktreeRuntimeKind::Managed => WorkflowRuntimeKind::ManagedWorktree,
            WorktreeRuntimeKind::Legacy => WorkflowRuntimeKind::LegacyWorktree,
        },
        display_label: binding.display_label.clone(),
        branch: Some(binding.branch.clone()),
        target_branch: Some(binding.target_branch.clone()),
        managed_worktree_id: Some(binding.worktree_id.clone()),
        legacy: matches!(binding.runtime_kind, WorktreeRuntimeKind::Legacy),
        runtime_status: Some(WorktreeStatus::Active),
        pr_status: binding.pr_info.clone(),
    }
}

fn active_phase_for_session(
    session: &crate::services::workflow_kernel::WorkflowSession,
) -> Option<&str> {
    match session.active_mode {
        WorkflowMode::Chat => session
            .mode_snapshots
            .chat
            .as_ref()
            .map(|state| state.phase.as_str()),
        WorkflowMode::Plan => session
            .mode_snapshots
            .plan
            .as_ref()
            .map(|state| state.phase.as_str()),
        WorkflowMode::Task => session
            .mode_snapshots
            .task
            .as_ref()
            .map(|state| state.phase.as_str()),
        WorkflowMode::Debug => session
            .mode_snapshots
            .debug
            .as_ref()
            .map(|state| state.phase.as_str()),
    }
}

fn is_runtime_mutation_blocked(phase: Option<&str>, mode: WorkflowMode) -> bool {
    match mode {
        WorkflowMode::Chat => matches!(phase, Some("submitting" | "streaming" | "paused")),
        WorkflowMode::Plan => matches!(phase, Some("analyzing" | "planning" | "executing")),
        WorkflowMode::Task => matches!(
            phase,
            Some(
                "analyzing"
                    | "exploring"
                    | "requirement_analysis"
                    | "generating_prd"
                    | "generating_design_doc"
                    | "executing"
                    | "paused"
            )
        ),
        WorkflowMode::Debug => matches!(
            phase,
            Some(
                "clarifying"
                    | "gathering_signal"
                    | "reproducing"
                    | "testing_hypothesis"
                    | "identifying_root_cause"
                    | "patching"
                    | "verifying"
            )
        ),
    }
}

fn ensure_runtime_can_mutate(
    session: &crate::services::workflow_kernel::WorkflowSession,
) -> Result<(), String> {
    let phase = active_phase_for_session(session);
    if is_runtime_mutation_blocked(phase, session.active_mode) {
        return Err(format!(
            "Cannot modify runtime while the current {:?} flow is active ({})",
            session.active_mode,
            phase.unwrap_or("unknown")
        ));
    }
    Ok(())
}

fn session_root_path(
    session: &crate::services::workflow_kernel::WorkflowSession,
) -> Option<String> {
    session
        .runtime
        .root_path
        .clone()
        .or_else(|| session.workspace_path.clone())
}

fn default_session_task_name(
    session: &crate::services::workflow_kernel::WorkflowSession,
) -> String {
    let trimmed = session.display_title.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    session_root_path(session)
        .and_then(|path| {
            PathBuf::from(path)
                .file_name()
                .and_then(|value| value.to_str())
                .map(ToOwned::to_owned)
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "session-runtime".to_string())
}

/// Create a new worktree for isolated task execution
#[tauri::command]
pub async fn create_worktree(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
    task_name: String,
    target_branch: String,
    base_path: Option<String>,
    prd_path: Option<String>,
    execution_mode: Option<String>,
) -> Result<CommandResponse<Worktree>, String> {
    let manager = state.manager.read().await;
    let repo = PathBuf::from(&repo_path);

    let request = CreateWorktreeRequest {
        task_name,
        target_branch,
        base_path,
        prd_path,
        execution_mode: execution_mode.unwrap_or_else(|| "auto".to_string()),
    };

    match manager.create_worktree(&repo, request).await {
        Ok(worktree) => Ok(CommandResponse::ok(worktree)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List all active worktrees in a repository
#[tauri::command]
pub async fn list_worktrees(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
) -> Result<CommandResponse<Vec<Worktree>>, String> {
    let manager = state.manager.read().await;
    let repo = PathBuf::from(&repo_path);

    match manager.list_worktrees(&repo).await {
        Ok(worktrees) => Ok(CommandResponse::ok(worktrees)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get a specific worktree by ID
#[tauri::command]
pub async fn get_worktree(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
    worktree_id: String,
) -> Result<CommandResponse<Worktree>, String> {
    let manager = state.manager.read().await;
    let repo = PathBuf::from(&repo_path);

    match manager.get_worktree(&repo, &worktree_id).await {
        Ok(worktree) => Ok(CommandResponse::ok(worktree)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get the status of a worktree
#[tauri::command]
pub async fn get_worktree_status(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
    worktree_id: String,
) -> Result<CommandResponse<WorktreeStatus>, String> {
    let manager = state.manager.read().await;
    let repo = PathBuf::from(&repo_path);

    match manager.get_worktree_status(&repo, &worktree_id).await {
        Ok(status) => Ok(CommandResponse::ok(status)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Remove a worktree
#[tauri::command]
pub async fn remove_worktree(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
    worktree_id: String,
    force: Option<bool>,
) -> Result<CommandResponse<()>, String> {
    let manager = state.manager.read().await;
    let repo = PathBuf::from(&repo_path);

    match manager
        .remove_worktree(&repo, &worktree_id, force.unwrap_or(false))
        .await
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Complete a worktree: commit code changes, merge to target branch, cleanup
#[tauri::command]
pub async fn complete_worktree(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
    worktree_id: String,
    commit_message: Option<String>,
) -> Result<CommandResponse<CompleteWorktreeResult>, String> {
    let manager = state.manager.read().await;
    let repo = PathBuf::from(&repo_path);

    match manager
        .complete_worktree(&repo, &worktree_id, commit_message.as_deref())
        .await
    {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn workflow_create_isolated_session(
    initial_mode: Option<WorkflowMode>,
    repo_path: String,
    task_name: String,
    target_branch: String,
    display_title: Option<String>,
    cleanup_policy: Option<WorktreeCleanupPolicy>,
    state: tauri::State<'_, WorkflowKernelState>,
    worktree_state: tauri::State<'_, WorktreeState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<crate::services::workflow_kernel::WorkflowSession>, String> {
    let root_path = PathBuf::from(&repo_path);
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "workspacePath".to_string(),
        serde_json::Value::String(repo_path.clone()),
    );
    metadata.insert(
        "workspaceRootPath".to_string(),
        serde_json::Value::String(repo_path.clone()),
    );
    if let Some(title) = display_title
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        metadata.insert(
            "displayTitle".to_string(),
            serde_json::Value::String(title.clone()),
        );
    }
    let initial_context = Some(HandoffContextBundle {
        conversation_context: Vec::new(),
        summary_items: Vec::new(),
        artifact_refs: Vec::new(),
        context_sources: vec!["simple_mode".to_string()],
        metadata,
    });

    let opened = match state.open_session(initial_mode, initial_context).await {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };

    let request = CreateManagedWorktreeRequest {
        session_id: opened.session_id.clone(),
        task_name,
        branch_name: None,
        target_branch,
        display_label: display_title.clone(),
        cleanup_policy: cleanup_policy.unwrap_or_default(),
        prd_path: None,
        execution_mode: "auto".to_string(),
    };

    let manager = worktree_state.manager.read().await;
    let runtime_binding = match manager
        .create_managed_worktree_for_session(&root_path, request)
        .await
    {
        Ok(binding) => binding,
        Err(error) => {
            let _ = state.delete_session(&opened.session_id).await;
            return Ok(CommandResponse::err(error.to_string()));
        }
    };
    drop(manager);

    let updated = match state
        .update_session_runtime(&opened.session_id, binding_to_runtime(&runtime_binding))
        .await
    {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    let _ = emit_kernel_update_for_session(
        &app,
        state.inner(),
        &updated.session_id,
        "workflow_create_isolated_session",
    )
    .await;
    let _ =
        emit_session_catalog_update(&app, state.inner(), "workflow_create_isolated_session").await;
    Ok(CommandResponse::ok(updated))
}

#[tauri::command]
pub async fn workflow_move_session_to_worktree(
    session_id: String,
    repo_path: String,
    branch_name: String,
    target_branch: String,
    cleanup_policy: Option<WorktreeCleanupPolicy>,
    state: tauri::State<'_, WorkflowKernelState>,
    worktree_state: tauri::State<'_, WorktreeState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<crate::services::workflow_kernel::WorkflowSession>, String> {
    let session = match state.get_session(&session_id).await {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    if let Err(error) = ensure_runtime_can_mutate(&session) {
        return Ok(CommandResponse::err(error));
    }
    if session.runtime.runtime_kind == WorkflowRuntimeKind::ManagedWorktree {
        return Ok(CommandResponse::err(
            "Session is already running inside a managed worktree",
        ));
    }
    let Some(root_path) = session_root_path(&session) else {
        return Ok(CommandResponse::err(
            "Session is missing root workspace path",
        ));
    };
    if PathBuf::from(&repo_path) != PathBuf::from(&root_path) {
        return Ok(CommandResponse::err(format!(
            "Session root path mismatch: expected '{}', got '{}'",
            root_path, repo_path
        )));
    }
    if let Some(worktree_id) = session.runtime.managed_worktree_id.as_deref() {
        let manager = worktree_state.manager.read().await;
        let _ = manager
            .detach_worktree_from_session(&PathBuf::from(&root_path), worktree_id)
            .await;
    }

    let request = CreateManagedWorktreeRequest {
        session_id: session.session_id.clone(),
        task_name: default_session_task_name(&session),
        branch_name: Some(branch_name),
        target_branch,
        display_label: Some(session.display_title.clone()),
        cleanup_policy: cleanup_policy.unwrap_or_default(),
        prd_path: None,
        execution_mode: "auto".to_string(),
    };

    let manager = worktree_state.manager.read().await;
    let runtime_binding = match manager
        .create_managed_worktree_for_session(&PathBuf::from(&root_path), request)
        .await
    {
        Ok(binding) => binding,
        Err(error) => return Ok(CommandResponse::err(error.to_string())),
    };
    drop(manager);

    let updated = match state
        .update_session_runtime(&session_id, binding_to_runtime(&runtime_binding))
        .await
    {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    let _ = emit_kernel_update_for_session(
        &app,
        state.inner(),
        &updated.session_id,
        "workflow_move_session_to_worktree",
    )
    .await;
    let _ =
        emit_session_catalog_update(&app, state.inner(), "workflow_move_session_to_worktree").await;
    Ok(CommandResponse::ok(updated))
}

#[tauri::command]
pub async fn workflow_attach_session_worktree(
    session_id: String,
    repo_path: String,
    worktree_path: String,
    display_label: Option<String>,
    cleanup_policy: Option<WorktreeCleanupPolicy>,
    state: tauri::State<'_, WorkflowKernelState>,
    worktree_state: tauri::State<'_, WorktreeState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<crate::services::workflow_kernel::WorkflowSession>, String> {
    let session = match state.get_session(&session_id).await {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    if let Err(error) = ensure_runtime_can_mutate(&session) {
        return Ok(CommandResponse::err(error));
    }
    if session
        .runtime
        .runtime_path
        .as_deref()
        .map(|current| current == worktree_path)
        .unwrap_or(false)
    {
        return Ok(CommandResponse::ok(session));
    }
    if let Some(current_managed_id) = session.runtime.managed_worktree_id.as_deref() {
        if let Some(root_path) = session_root_path(&session) {
            let manager = worktree_state.manager.read().await;
            let _ = manager
                .detach_worktree_from_session(&PathBuf::from(root_path), current_managed_id)
                .await;
        }
    }

    let manager = worktree_state.manager.read().await;
    let binding = match manager
        .attach_existing_worktree_to_session(
            &PathBuf::from(&repo_path),
            &PathBuf::from(&worktree_path),
            &session_id,
            display_label,
            cleanup_policy.unwrap_or_default(),
        )
        .await
    {
        Ok(binding) => binding,
        Err(error) => return Ok(CommandResponse::err(error.to_string())),
    };
    drop(manager);
    let updated = match state
        .update_session_runtime(&session_id, binding_to_runtime(&binding))
        .await
    {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    let _ = emit_kernel_update_for_session(
        &app,
        state.inner(),
        &updated.session_id,
        "workflow_attach_session_worktree",
    )
    .await;
    let _ =
        emit_session_catalog_update(&app, state.inner(), "workflow_attach_session_worktree").await;
    Ok(CommandResponse::ok(updated))
}

#[tauri::command]
pub async fn workflow_detach_session_worktree(
    session_id: String,
    state: tauri::State<'_, WorkflowKernelState>,
    worktree_state: tauri::State<'_, WorktreeState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<crate::services::workflow_kernel::WorkflowSession>, String> {
    let session = match state.get_session(&session_id).await {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    if let Err(error) = ensure_runtime_can_mutate(&session) {
        return Ok(CommandResponse::err(error));
    }
    if let Some(worktree_id) = session.runtime.managed_worktree_id.as_deref() {
        if let Some(root_path) = session.runtime.root_path.as_deref() {
            let manager = worktree_state.manager.read().await;
            let _ = manager
                .detach_worktree_from_session(&PathBuf::from(root_path), worktree_id)
                .await;
        }
    }
    let updated = match state
        .update_session_runtime(
            &session_id,
            SessionRuntimeInfo {
                root_path: session
                    .runtime
                    .root_path
                    .clone()
                    .or_else(|| session.workspace_path.clone()),
                runtime_path: session
                    .runtime
                    .root_path
                    .clone()
                    .or_else(|| session.workspace_path.clone()),
                runtime_kind: WorkflowRuntimeKind::Main,
                display_label: None,
                branch: None,
                target_branch: None,
                managed_worktree_id: None,
                legacy: false,
                runtime_status: None,
                pr_status: None,
            },
        )
        .await
    {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    let _ = emit_kernel_update_for_session(
        &app,
        state.inner(),
        &updated.session_id,
        "workflow_detach_session_worktree",
    )
    .await;
    let _ =
        emit_session_catalog_update(&app, state.inner(), "workflow_detach_session_worktree").await;
    Ok(CommandResponse::ok(updated))
}

#[tauri::command]
pub async fn workflow_cleanup_session_worktree(
    session_id: String,
    force: Option<bool>,
    state: tauri::State<'_, WorkflowKernelState>,
    worktree_state: tauri::State<'_, WorktreeState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<crate::services::workflow_kernel::WorkflowSession>, String> {
    let session = match state.get_session(&session_id).await {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    if let Err(error) = ensure_runtime_can_mutate(&session) {
        return Ok(CommandResponse::err(error));
    }
    let Some(worktree_id) = session.runtime.managed_worktree_id.as_deref() else {
        return Ok(CommandResponse::err(
            "Session is not bound to a managed worktree",
        ));
    };
    let Some(root_path) = session.runtime.root_path.as_deref() else {
        return Ok(CommandResponse::err(
            "Session is missing root workspace path",
        ));
    };
    let manager = worktree_state.manager.read().await;
    if let Err(error) = manager
        .cleanup_managed_worktree(
            &PathBuf::from(root_path),
            worktree_id,
            force.unwrap_or(true),
        )
        .await
    {
        return Ok(CommandResponse::err(error.to_string()));
    }
    drop(manager);

    let updated = match state
        .update_session_runtime(
            &session_id,
            SessionRuntimeInfo {
                root_path: session.runtime.root_path.clone(),
                runtime_path: session.runtime.root_path.clone(),
                runtime_kind: WorkflowRuntimeKind::Main,
                display_label: None,
                branch: None,
                target_branch: None,
                managed_worktree_id: None,
                legacy: false,
                runtime_status: None,
                pr_status: None,
            },
        )
        .await
    {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    let _ = emit_kernel_update_for_session(
        &app,
        state.inner(),
        &updated.session_id,
        "workflow_cleanup_session_worktree",
    )
    .await;
    let _ =
        emit_session_catalog_update(&app, state.inner(), "workflow_cleanup_session_worktree").await;
    Ok(CommandResponse::ok(updated))
}

#[tauri::command]
pub async fn workflow_list_repo_worktrees(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
) -> Result<CommandResponse<Vec<Worktree>>, String> {
    let manager = state.manager.read().await;
    match manager.list_worktrees(&PathBuf::from(repo_path)).await {
        Ok(worktrees) => Ok(CommandResponse::ok(worktrees)),
        Err(error) => Ok(CommandResponse::err(error.to_string())),
    }
}

#[tauri::command]
pub async fn workflow_prepare_session_pr(
    session_id: String,
    state: tauri::State<'_, WorkflowKernelState>,
    worktree_state: tauri::State<'_, WorktreeState>,
) -> Result<CommandResponse<PreparePullRequestResult>, String> {
    let session = match state.get_session(&session_id).await {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    let Some(worktree_id) = session.runtime.managed_worktree_id.as_deref() else {
        return Ok(CommandResponse::err(
            "Session is not bound to a managed worktree",
        ));
    };
    let Some(root_path) = session.runtime.root_path.as_deref() else {
        return Ok(CommandResponse::err(
            "Session is missing root workspace path",
        ));
    };
    let manager = worktree_state.manager.read().await;
    match manager
        .prepare_pull_request(&PathBuf::from(root_path), worktree_id)
        .await
    {
        Ok(payload) => Ok(CommandResponse::ok(payload)),
        Err(error) => Ok(CommandResponse::err(error.to_string())),
    }
}

#[tauri::command]
pub async fn workflow_create_session_pr(
    session_id: String,
    provider: ForgeProvider,
    remote_name: String,
    title: String,
    body: String,
    draft: Option<bool>,
    state: tauri::State<'_, WorkflowKernelState>,
    worktree_state: tauri::State<'_, WorktreeState>,
    app_state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<CreatePullRequestResult>, String> {
    let session = match state.get_session(&session_id).await {
        Ok(session) => session,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    let Some(worktree_id) = session.runtime.managed_worktree_id.as_deref() else {
        return Ok(CommandResponse::err(
            "Session is not bound to a managed worktree",
        ));
    };
    let Some(root_path) = session.runtime.root_path.as_deref() else {
        return Ok(CommandResponse::err(
            "Session is missing root workspace path",
        ));
    };
    let token_key = forge_token_key(&provider);
    let token = match app_state.get_api_key(token_key).await {
        Ok(Some(token)) if !token.trim().is_empty() => token,
        Ok(_) => {
            return Ok(CommandResponse::err(format!(
                "No token configured for {}",
                token_key
            )))
        }
        Err(error) => return Ok(CommandResponse::err(error.to_string())),
    };

    let manager = worktree_state.manager.read().await;
    let request = CreatePullRequestRequest {
        worktree_id: worktree_id.to_string(),
        provider,
        remote_name,
        title,
        body,
        draft: draft.unwrap_or(false),
        base_branch: session.runtime.target_branch.clone(),
    };
    match manager
        .create_pull_request(&PathBuf::from(root_path), &request, &token)
        .await
    {
        Ok(result) => {
            if let Ok(worktree) = manager
                .get_worktree(&PathBuf::from(root_path), worktree_id)
                .await
            {
                let mut runtime = session.runtime.clone();
                runtime.branch = Some(worktree.branch.clone());
                runtime.target_branch = Some(worktree.target_branch.clone());
                runtime.pr_status = worktree.pr_info.clone();
                runtime.runtime_status = Some(worktree.status);
                if let Ok(updated) = state.update_session_runtime(&session_id, runtime).await {
                    let _ = emit_kernel_update_for_session(
                        &app,
                        state.inner(),
                        &updated.session_id,
                        "workflow_create_session_pr",
                    )
                    .await;
                    let _ = emit_session_catalog_update(
                        &app,
                        state.inner(),
                        "workflow_create_session_pr",
                    )
                    .await;
                }
            }
            Ok(CommandResponse::ok(result))
        }
        Err(error) => Ok(CommandResponse::err(error.to_string())),
    }
}

#[tauri::command]
pub async fn set_forge_token(
    provider: ForgeProvider,
    token: String,
    app_state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    let key = forge_token_key(&provider);
    let value = token.trim().to_string();
    let result = if value.is_empty() {
        app_state.delete_api_key(key).await
    } else {
        app_state.set_api_key(key, &value).await
    };
    match result {
        Ok(()) => Ok(CommandResponse::ok(true)),
        Err(error) => Ok(CommandResponse::err(error.to_string())),
    }
}

#[tauri::command]
pub async fn has_forge_token(
    provider: ForgeProvider,
    app_state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    let key = forge_token_key(&provider);
    match app_state.get_api_key(key).await {
        Ok(Some(token)) => Ok(CommandResponse::ok(!token.trim().is_empty())),
        Ok(None) => Ok(CommandResponse::ok(false)),
        Err(error) => Ok(CommandResponse::err(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worktree_state_creation() {
        let state = WorktreeState::new();
        // Verify state can be created
        let _ = state;
    }

    #[tokio::test]
    async fn test_worktree_state_manager_access() {
        let state = WorktreeState::new();
        let manager = state.manager.read().await;
        // Verify we can access the manager
        let _ = manager;
    }
}
