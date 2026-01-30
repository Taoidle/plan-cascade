//! Worktree Commands
//!
//! Tauri commands for Git worktree management.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::models::worktree::{
    CompleteWorktreeResult, CreateWorktreeRequest, Worktree, WorktreeStatus,
};
use crate::services::worktree::WorktreeManager;

/// State for the worktree service
pub struct WorktreeState {
    manager: Arc<RwLock<WorktreeManager>>,
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
