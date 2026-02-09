//! Worktree Manager
//!
//! Manages the lifecycle of git worktrees for isolated story execution.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::worktree::{
    CompleteWorktreeResult, CreateWorktreeRequest, MergeConflict, Worktree, WorktreeStatus,
};
use crate::utils::error::{AppError, AppResult};

use super::config::PlanningConfigService;
use super::git_ops::{GitOps, MergeResult};

/// Default worktree directory name
pub const DEFAULT_WORKTREE_DIR: &str = ".worktree";

/// Manager for git worktree lifecycle operations
pub struct WorktreeManager {
    /// Git operations wrapper
    git: GitOps,
    /// Planning config service
    config_service: PlanningConfigService,
    /// Cache of known worktrees
    worktrees: Arc<RwLock<HashMap<String, Worktree>>>,
}

impl WorktreeManager {
    /// Create a new worktree manager
    pub fn new() -> Self {
        Self {
            git: GitOps::new(),
            config_service: PlanningConfigService::new(),
            worktrees: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the default worktree base directory for a repository
    pub fn get_worktree_base(&self, repo_path: &Path) -> PathBuf {
        repo_path.join(DEFAULT_WORKTREE_DIR)
    }

    /// Create a new worktree with a task branch
    pub async fn create_worktree(
        &self,
        repo_path: &Path,
        request: CreateWorktreeRequest,
    ) -> AppResult<Worktree> {
        // Sanitize task name for use as branch/directory name
        let sanitized_name = self.sanitize_task_name(&request.task_name);

        // Determine the worktree path
        let base_path = request
            .base_path
            .map(PathBuf::from)
            .unwrap_or_else(|| self.get_worktree_base(repo_path));

        let worktree_path = base_path.join(&sanitized_name);

        // Check if worktree already exists
        if worktree_path.exists() {
            return Err(AppError::validation(format!(
                "Worktree directory already exists: {}",
                worktree_path.display()
            )));
        }

        // Create branch name
        let branch_name = format!("worktree/{}", sanitized_name);

        // Check if branch already exists
        if self.git.branch_exists(repo_path, &branch_name)? {
            return Err(AppError::validation(format!(
                "Branch already exists: {}",
                branch_name
            )));
        }

        // Ensure target branch exists
        if !self.git.branch_exists(repo_path, &request.target_branch)? {
            return Err(AppError::validation(format!(
                "Target branch does not exist: {}",
                request.target_branch
            )));
        }

        // Create the worktree base directory if it doesn't exist
        if !base_path.exists() {
            fs::create_dir_all(&base_path)?;
        }

        // Create worktree with new branch
        self.git.add_worktree_with_new_branch(
            repo_path,
            &worktree_path,
            &branch_name,
            &request.target_branch,
        )?;

        // Create the worktree record
        let mut worktree = Worktree::new(
            sanitized_name.clone(),
            request.task_name.clone(),
            worktree_path.to_string_lossy().to_string(),
            branch_name,
            request.target_branch.clone(),
        );

        // Initialize planning config
        let config = self.config_service.create(
            &worktree_path,
            &request.task_name,
            &request.target_branch,
            request.prd_path.as_deref(),
            &request.execution_mode,
        )?;

        worktree.planning_config = Some(config);
        worktree.set_status(WorktreeStatus::Active);

        // Cache the worktree
        {
            let mut cache = self.worktrees.write().await;
            cache.insert(sanitized_name.clone(), worktree.clone());
        }

        Ok(worktree)
    }

    /// List all active worktrees
    pub async fn list_worktrees(&self, repo_path: &Path) -> AppResult<Vec<Worktree>> {
        // Get worktrees from git
        let git_worktrees = self.git.list_worktrees(repo_path)?;

        // Get the main repo root to filter it out
        let repo_root = self.git.get_repo_root(repo_path)?;

        let mut worktrees = Vec::new();

        for git_wt in git_worktrees {
            // Skip the main worktree
            if git_wt.path == repo_root {
                continue;
            }

            // Skip bare or prunable worktrees
            if git_wt.is_bare || git_wt.is_prunable {
                continue;
            }

            let wt_path = PathBuf::from(&git_wt.path);

            // Try to read planning config
            let planning_config = self.config_service.read(&wt_path).ok();

            // Determine status based on planning config
            let status = if let Some(ref config) = planning_config {
                match config.phase {
                    crate::models::worktree::PlanningPhase::Complete => WorktreeStatus::Ready,
                    crate::models::worktree::PlanningPhase::Executing => WorktreeStatus::InProgress,
                    _ => WorktreeStatus::Active,
                }
            } else {
                WorktreeStatus::Active
            };

            // Extract task name from branch
            let task_name = git_wt
                .branch
                .strip_prefix("worktree/")
                .unwrap_or(&git_wt.branch)
                .to_string();

            // Get timestamps from directory
            let (created_at, updated_at) = self.get_worktree_timestamps(&wt_path);

            let worktree = Worktree {
                id: task_name.clone(),
                name: planning_config
                    .as_ref()
                    .map(|c| c.task_name.clone())
                    .unwrap_or_else(|| task_name.clone()),
                path: git_wt.path,
                branch: git_wt.branch,
                target_branch: planning_config
                    .as_ref()
                    .map(|c| c.target_branch.clone())
                    .unwrap_or_else(|| "main".to_string()),
                status,
                created_at,
                updated_at,
                error: None,
                planning_config,
            };

            worktrees.push(worktree);
        }

        Ok(worktrees)
    }

    /// Get a specific worktree by ID
    pub async fn get_worktree(&self, repo_path: &Path, worktree_id: &str) -> AppResult<Worktree> {
        let worktrees = self.list_worktrees(repo_path).await?;

        worktrees
            .into_iter()
            .find(|wt| wt.id == worktree_id)
            .ok_or_else(|| AppError::not_found(format!("Worktree not found: {}", worktree_id)))
    }

    /// Get the status of a worktree
    pub async fn get_worktree_status(
        &self,
        repo_path: &Path,
        worktree_id: &str,
    ) -> AppResult<WorktreeStatus> {
        let worktree = self.get_worktree(repo_path, worktree_id).await?;
        Ok(worktree.status)
    }

    /// Remove a worktree
    pub async fn remove_worktree(
        &self,
        repo_path: &Path,
        worktree_id: &str,
        force: bool,
    ) -> AppResult<()> {
        let worktree = self.get_worktree(repo_path, worktree_id).await?;
        let wt_path = PathBuf::from(&worktree.path);

        // Remove the worktree
        self.git.remove_worktree(repo_path, &wt_path, force)?;

        // Delete the branch if it still exists
        if self.git.branch_exists(repo_path, &worktree.branch)? {
            self.git.delete_branch(repo_path, &worktree.branch, force)?;
        }

        // Remove from cache
        {
            let mut cache = self.worktrees.write().await;
            cache.remove(worktree_id);
        }

        // Prune any stale worktrees
        self.git.prune_worktrees(repo_path)?;

        Ok(())
    }

    /// Complete a worktree: commit code, merge to target, cleanup
    pub async fn complete_worktree(
        &self,
        repo_path: &Path,
        worktree_id: &str,
        commit_message: Option<&str>,
    ) -> AppResult<CompleteWorktreeResult> {
        let worktree = self.get_worktree(repo_path, worktree_id).await?;
        let wt_path = PathBuf::from(&worktree.path);

        // Verify worktree can be completed
        if !worktree.status.can_complete() {
            return Ok(CompleteWorktreeResult::error(format!(
                "Worktree cannot be completed in status: {}",
                worktree.status
            )));
        }

        // Get git status
        let status = self.git.status(&wt_path)?;

        // Get files to commit (excluding planning files)
        let all_files: Vec<String> = status
            .staged
            .iter()
            .chain(status.modified.iter())
            .chain(status.untracked.iter())
            .cloned()
            .collect();

        let committable_files = self
            .config_service
            .get_committable_files(&wt_path, &all_files);

        let mut result = CompleteWorktreeResult::success(None, false, false);

        // Stage and commit if there are changes
        if !committable_files.is_empty() {
            // Reset staging area first
            self.git.reset_staging(&wt_path).ok();

            // Stage only committable files
            let file_refs: Vec<&str> = committable_files.iter().map(|s| s.as_str()).collect();
            self.git.add(&wt_path, &file_refs)?;

            // Commit
            let message = commit_message.unwrap_or_else(|| {
                Box::leak(Box::new(format!(
                    "Complete worktree task: {}",
                    worktree.name
                )))
            });

            match self.git.commit(&wt_path, message) {
                Ok(sha) => {
                    result.commit_sha = Some(sha);
                }
                Err(e) => {
                    return Ok(CompleteWorktreeResult::error(format!(
                        "Failed to commit: {}",
                        e
                    )));
                }
            }
        } else {
            result = result.with_warning("No code changes to commit".to_string());
        }

        // Checkout target branch in main repo
        self.git.checkout(repo_path, &worktree.target_branch)?;

        // Merge the worktree branch
        let merge_message = format!("Merge {} into {}", worktree.branch, worktree.target_branch);

        match self
            .git
            .merge(repo_path, &worktree.branch, Some(&merge_message))?
        {
            MergeResult::Success => {
                result.merged = true;
            }
            MergeResult::Conflict(conflicts) => {
                // Abort the merge
                self.git.merge_abort(repo_path).ok();

                let conflict_info = MergeConflict::new(
                    conflicts,
                    worktree.branch.clone(),
                    worktree.target_branch.clone(),
                );

                return Ok(CompleteWorktreeResult::error(format!(
                    "Merge conflict detected in {} files. Please resolve manually.",
                    conflict_info.conflicting_files.len()
                ))
                .with_warning(format!(
                    "Conflicting files: {:?}",
                    conflict_info.conflicting_files
                )));
            }
            MergeResult::Error(msg) => {
                return Ok(CompleteWorktreeResult::error(format!(
                    "Merge failed: {}",
                    msg
                )));
            }
        }

        // Cleanup: remove worktree and branch
        if result.merged {
            match self.remove_worktree(repo_path, worktree_id, true).await {
                Ok(_) => {
                    result.cleaned_up = true;
                }
                Err(e) => {
                    result = result.with_warning(format!("Failed to cleanup worktree: {}", e));
                }
            }
        }

        Ok(result)
    }

    /// Sanitize a task name for use as branch/directory name
    fn sanitize_task_name(&self, name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else if c.is_whitespace() {
                    '-'
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .to_lowercase()
    }

    /// Get timestamps for a worktree directory
    fn get_worktree_timestamps(&self, path: &Path) -> (String, String) {
        let default_time = chrono::Utc::now().to_rfc3339();

        let created_at = fs::metadata(path)
            .ok()
            .and_then(|m| m.created().ok())
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.to_rfc3339()
            })
            .unwrap_or_else(|| default_time.clone());

        let updated_at = fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.to_rfc3339()
            })
            .unwrap_or(default_time);

        (created_at, updated_at)
    }
}

impl Default for WorktreeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for WorktreeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorktreeManager").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_task_name() {
        let manager = WorktreeManager::new();

        assert_eq!(
            manager.sanitize_task_name("My Feature Task"),
            "my-feature-task"
        );
        assert_eq!(
            manager.sanitize_task_name("feature/add-button"),
            "feature_add-button"
        );
        assert_eq!(
            manager.sanitize_task_name("Task with @special#chars!"),
            "task-with-_special_chars_"
        );
        assert_eq!(manager.sanitize_task_name("simple"), "simple");
    }

    #[test]
    fn test_get_worktree_base() {
        let manager = WorktreeManager::new();
        let repo_path = Path::new("/path/to/repo");
        let base = manager.get_worktree_base(repo_path);
        assert!(base.to_string_lossy().contains(".worktree"));
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let manager = WorktreeManager::new();
        // Verify the manager can be created
        assert!(manager.worktrees.read().await.is_empty());
    }
}
