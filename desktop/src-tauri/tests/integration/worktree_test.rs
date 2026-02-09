//! Worktree Integration Tests
//!
//! Tests for git worktree models and utility functions.
//! Note: Full worktree lifecycle tests require a git repository.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

use plan_cascade_desktop::models::worktree::{
    CompleteWorktreeResult, CreateWorktreeRequest, MergeConflict, PlanningConfig, PlanningPhase,
    Worktree, WorktreeStatus,
};
use plan_cascade_desktop::services::worktree::{GitOps, WorktreeManager};

// ============================================================================
// Helper Functions
// ============================================================================

/// Initialize a git repository in the given directory
fn init_git_repo(path: &std::path::Path) -> bool {
    let result = Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output();

    if result.is_err() {
        return false;
    }

    let _ = Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(path)
        .output();

    let _ = Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(path)
        .output();

    true
}

/// Create an initial commit
fn create_initial_commit(path: &std::path::Path) -> bool {
    fs::write(path.join("README.md"), "# Test Project\n").unwrap();

    let add = Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output();

    if add.is_err() {
        return false;
    }

    let commit = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(path)
        .output();

    commit.map(|o| o.status.success()).unwrap_or(false)
}

/// Create a test file
fn create_test_file(path: &std::path::Path, name: &str, content: &str) {
    let file_path = path.join(name);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&file_path, content).unwrap();
}

// ============================================================================
// Worktree Model Tests
// ============================================================================

#[test]
fn test_worktree_creation() {
    let wt = Worktree::new("task-1", "My Task", "/path/to/wt", "feature/task-1", "main");

    assert_eq!(wt.id, "task-1");
    assert_eq!(wt.name, "My Task");
    assert_eq!(wt.branch, "feature/task-1");
    assert_eq!(wt.target_branch, "main");
    assert!(matches!(wt.status, WorktreeStatus::Creating));
}

#[test]
fn test_worktree_status_transitions() {
    let mut wt = Worktree::new("task-1", "Task", "/path", "branch", "main");

    wt.set_status(WorktreeStatus::Active);
    assert!(matches!(wt.status, WorktreeStatus::Active));
    assert!(wt.status.can_complete());

    wt.set_status(WorktreeStatus::InProgress);
    assert!(matches!(wt.status, WorktreeStatus::InProgress));
    assert!(wt.status.can_complete());

    wt.set_status(WorktreeStatus::Ready);
    assert!(matches!(wt.status, WorktreeStatus::Ready));
    assert!(wt.status.can_complete());

    wt.set_status(WorktreeStatus::Completed);
    assert!(wt.status.is_terminal());
    assert!(!wt.status.can_complete());
}

#[test]
fn test_worktree_error_handling() {
    let mut wt = Worktree::new("task-1", "Task", "/path", "branch", "main");

    wt.set_error("Something went wrong");

    assert!(matches!(wt.status, WorktreeStatus::Error));
    assert_eq!(wt.error, Some("Something went wrong".to_string()));
    assert!(wt.status.is_terminal());
}

// ============================================================================
// Planning Config Tests
// ============================================================================

#[test]
fn test_planning_config_creation() {
    let config = PlanningConfig::new("test-task", "main");

    assert_eq!(config.task_name, "test-task");
    assert_eq!(config.target_branch, "main");
    assert!(matches!(config.phase, PlanningPhase::Init));
    assert!(config.completed_stories.is_empty());
    assert_eq!(config.execution_mode, "auto");
}

#[test]
fn test_planning_config_story_completion() {
    let mut config = PlanningConfig::new("test-task", "main");

    config.complete_story("story-001");
    config.complete_story("story-002");

    assert_eq!(config.completed_stories.len(), 2);
    assert!(config.completed_stories.contains(&"story-001".to_string()));
    assert!(config.completed_stories.contains(&"story-002".to_string()));
}

#[test]
fn test_planning_config_phase_transitions() {
    let mut config = PlanningConfig::new("test-task", "main");

    config.set_phase(PlanningPhase::PrdGeneration);
    assert!(matches!(config.phase, PlanningPhase::PrdGeneration));

    config.set_phase(PlanningPhase::PrdReview);
    assert!(matches!(config.phase, PlanningPhase::PrdReview));

    config.set_phase(PlanningPhase::Executing);
    assert!(matches!(config.phase, PlanningPhase::Executing));

    config.set_phase(PlanningPhase::Complete);
    assert!(matches!(config.phase, PlanningPhase::Complete));
}

#[test]
fn test_planning_phase_display() {
    assert_eq!(PlanningPhase::Init.to_string(), "init");
    assert_eq!(PlanningPhase::PrdGeneration.to_string(), "prd_generation");
    assert_eq!(PlanningPhase::PrdReview.to_string(), "prd_review");
    assert_eq!(PlanningPhase::Executing.to_string(), "executing");
    assert_eq!(PlanningPhase::Complete.to_string(), "complete");
}

// ============================================================================
// Complete Worktree Result Tests
// ============================================================================

#[test]
fn test_complete_worktree_success() {
    let result = CompleteWorktreeResult::success(Some("abc123def456".to_string()), true, true);

    assert!(result.success);
    assert_eq!(result.commit_sha, Some("abc123def456".to_string()));
    assert!(result.merged);
    assert!(result.cleaned_up);
    assert!(result.warnings.is_empty());
    assert!(result.error.is_none());
}

#[test]
fn test_complete_worktree_error() {
    let result = CompleteWorktreeResult::error("Merge conflict detected");

    assert!(!result.success);
    assert!(result.commit_sha.is_none());
    assert!(!result.merged);
    assert!(!result.cleaned_up);
    assert_eq!(result.error, Some("Merge conflict detected".to_string()));
}

#[test]
fn test_complete_worktree_with_warnings() {
    let result = CompleteWorktreeResult::success(None, true, true)
        .with_warning("No code changes to commit")
        .with_warning("Planning files were excluded");

    assert!(result.success);
    assert_eq!(result.warnings.len(), 2);
    assert!(result
        .warnings
        .contains(&"No code changes to commit".to_string()));
}

// ============================================================================
// Merge Conflict Tests
// ============================================================================

#[test]
fn test_merge_conflict_creation() {
    let conflict = MergeConflict::new(
        vec!["file1.rs".to_string(), "file2.rs".to_string()],
        "feature/task",
        "main",
    );

    assert_eq!(conflict.conflicting_files.len(), 2);
    assert_eq!(conflict.source_branch, "feature/task");
    assert_eq!(conflict.target_branch, "main");
    assert!(!conflict.resolution_steps.is_empty());
}

// ============================================================================
// Create Worktree Request Tests
// ============================================================================

#[test]
fn test_create_worktree_request() {
    let request = CreateWorktreeRequest {
        task_name: "my-feature".to_string(),
        target_branch: "main".to_string(),
        base_path: Some("/custom/path".to_string()),
        prd_path: Some("/path/to/prd.json".to_string()),
        execution_mode: "manual".to_string(),
    };

    assert_eq!(request.task_name, "my-feature");
    assert_eq!(request.target_branch, "main");
    assert!(request.base_path.is_some());
    assert!(request.prd_path.is_some());
    assert_eq!(request.execution_mode, "manual");
}

// ============================================================================
// WorktreeStatus Tests
// ============================================================================

#[test]
fn test_worktree_status_display() {
    assert_eq!(WorktreeStatus::Creating.to_string(), "creating");
    assert_eq!(WorktreeStatus::Active.to_string(), "active");
    assert_eq!(WorktreeStatus::InProgress.to_string(), "in_progress");
    assert_eq!(WorktreeStatus::Ready.to_string(), "ready");
    assert_eq!(WorktreeStatus::Merging.to_string(), "merging");
    assert_eq!(WorktreeStatus::Completed.to_string(), "completed");
    assert_eq!(WorktreeStatus::Error.to_string(), "error");
}

#[test]
fn test_worktree_status_can_complete() {
    assert!(!WorktreeStatus::Creating.can_complete());
    assert!(WorktreeStatus::Active.can_complete());
    assert!(WorktreeStatus::InProgress.can_complete());
    assert!(WorktreeStatus::Ready.can_complete());
    assert!(!WorktreeStatus::Merging.can_complete());
    assert!(!WorktreeStatus::Completed.can_complete());
    assert!(!WorktreeStatus::Error.can_complete());
}

#[test]
fn test_worktree_status_is_terminal() {
    assert!(!WorktreeStatus::Creating.is_terminal());
    assert!(!WorktreeStatus::Active.is_terminal());
    assert!(!WorktreeStatus::InProgress.is_terminal());
    assert!(!WorktreeStatus::Ready.is_terminal());
    assert!(!WorktreeStatus::Merging.is_terminal());
    assert!(WorktreeStatus::Completed.is_terminal());
    assert!(WorktreeStatus::Error.is_terminal());
}

// ============================================================================
// WorktreeManager Tests
// ============================================================================

#[test]
fn test_worktree_manager_sanitize_name() {
    let manager = WorktreeManager::new();

    // Test via worktree_base - indirectly tests internal logic
    let repo_path = std::path::Path::new("/test/repo");
    let base = manager.get_worktree_base(repo_path);
    assert!(base.to_string_lossy().contains(".worktree"));
}

// ============================================================================
// GitOps Unit Tests
// ============================================================================

#[test]
fn test_git_status_is_clean() {
    use plan_cascade_desktop::services::worktree::GitStatus;

    let status = GitStatus::default();
    assert!(status.is_clean());

    let mut status = GitStatus::default();
    status.modified.push("file.txt".to_string());
    assert!(!status.is_clean());
}

#[test]
fn test_git_status_change_count() {
    use plan_cascade_desktop::services::worktree::GitStatus;

    let mut status = GitStatus::default();
    status.modified.push("a.txt".to_string());
    status.staged.push("b.txt".to_string());
    status.untracked.push("c.txt".to_string());
    assert_eq!(status.change_count(), 3);
}

#[test]
fn test_git_result_into_result() {
    use plan_cascade_desktop::services::worktree::GitResult;

    let success = GitResult {
        success: true,
        stdout: "output".to_string(),
        stderr: "".to_string(),
        exit_code: 0,
    };
    assert_eq!(success.into_result().unwrap(), "output");

    let failure = GitResult {
        success: false,
        stdout: "".to_string(),
        stderr: "error message".to_string(),
        exit_code: 1,
    };
    assert!(failure.into_result().is_err());
}

// ============================================================================
// Integration Tests (require git)
// ============================================================================

#[test]
fn test_git_ops_with_real_repo() {
    let temp = tempfile::tempdir().unwrap();

    // Try to init git repo - skip if git not available
    if !init_git_repo(temp.path()) {
        eprintln!("Skipping git integration test - git not available");
        return;
    }

    if !create_initial_commit(temp.path()) {
        eprintln!("Skipping git integration test - could not create initial commit");
        return;
    }

    let git = GitOps::new();

    // Test repo root
    let root = git.get_repo_root(temp.path());
    assert!(root.is_ok());

    // Test current branch
    let branch = git.get_current_branch(temp.path());
    assert!(branch.is_ok());

    // Test status
    let status = git.status(temp.path());
    assert!(status.is_ok());
    assert!(status.unwrap().is_clean());

    // Create a new file and test status again
    create_test_file(temp.path(), "newfile.txt", "content");
    let status = git.status(temp.path()).unwrap();
    assert!(!status.is_clean());
    assert!(status.untracked.contains(&"newfile.txt".to_string()));

    // Test add and commit
    git.add(temp.path(), &["newfile.txt"]).unwrap();
    let commit_result = git.commit(temp.path(), "Add newfile");
    assert!(commit_result.is_ok());

    // Should be clean after commit
    assert!(git.is_clean(temp.path()).unwrap());
}

#[test]
fn test_git_branch_operations() {
    let temp = tempfile::tempdir().unwrap();

    if !init_git_repo(temp.path()) {
        return;
    }

    if !create_initial_commit(temp.path()) {
        return;
    }

    let git = GitOps::new();

    // Get current branch
    let main_branch = git.get_current_branch(temp.path()).unwrap();
    assert!(main_branch == "master" || main_branch == "main");

    // Check branch exists
    assert!(git.branch_exists(temp.path(), &main_branch).unwrap());
    assert!(!git
        .branch_exists(temp.path(), "nonexistent-branch-xyz")
        .unwrap());

    // Create a new branch
    git.create_branch(temp.path(), "test-branch", &main_branch)
        .unwrap();
    assert!(git.branch_exists(temp.path(), "test-branch").unwrap());

    // Delete the branch
    git.delete_branch(temp.path(), "test-branch", false)
        .unwrap();
    assert!(!git.branch_exists(temp.path(), "test-branch").unwrap());
}

#[test]
fn test_git_worktree_operations() {
    let temp = tempfile::tempdir().unwrap();

    if !init_git_repo(temp.path()) {
        return;
    }

    if !create_initial_commit(temp.path()) {
        return;
    }

    let git = GitOps::new();

    // List worktrees - should have at least main worktree
    let worktrees = git.list_worktrees(temp.path());
    assert!(worktrees.is_ok());
    assert!(!worktrees.unwrap().is_empty());
}
