//! Worktree Models
//!
//! Data structures for Git worktree management.

use serde::{Deserialize, Serialize};

/// Status of a worktree
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    /// Worktree is being created
    #[default]
    Creating,
    /// Worktree is active and ready for use
    Active,
    /// Work is in progress
    InProgress,
    /// Work is complete, ready for merge
    Ready,
    /// Merge in progress
    Merging,
    /// Merge completed successfully
    Completed,
    /// Error state
    Error,
}

impl WorktreeStatus {
    /// Check if the worktree can be completed (merged and cleaned up)
    pub fn can_complete(&self) -> bool {
        matches!(self, Self::Active | Self::InProgress | Self::Ready)
    }

    /// Check if the worktree is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Error)
    }
}

impl std::fmt::Display for WorktreeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Creating => "creating",
            Self::Active => "active",
            Self::InProgress => "in_progress",
            Self::Ready => "ready",
            Self::Merging => "merging",
            Self::Completed => "completed",
            Self::Error => "error",
        };
        write!(f, "{}", s)
    }
}

/// A Git worktree managed by Plan Cascade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    /// Unique identifier for this worktree (usually task name)
    pub id: String,
    /// Display name for the task
    pub name: String,
    /// Path to the worktree directory
    pub path: String,
    /// Name of the branch in this worktree
    pub branch: String,
    /// Target branch to merge into when complete
    pub target_branch: String,
    /// Current status
    pub status: WorktreeStatus,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    /// Last modified timestamp (ISO 8601)
    pub updated_at: String,
    /// Error message if status is Error
    pub error: Option<String>,
    /// Associated planning config
    pub planning_config: Option<PlanningConfig>,
}

impl Worktree {
    /// Create a new worktree instance
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        path: impl Into<String>,
        branch: impl Into<String>,
        target_branch: impl Into<String>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: id.into(),
            name: name.into(),
            path: path.into(),
            branch: branch.into(),
            target_branch: target_branch.into(),
            status: WorktreeStatus::Creating,
            created_at: now.clone(),
            updated_at: now,
            error: None,
            planning_config: None,
        }
    }

    /// Update the status and timestamp
    pub fn set_status(&mut self, status: WorktreeStatus) {
        self.status = status;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Set an error status with message
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.status = WorktreeStatus::Error;
        self.error = Some(message.into());
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}

/// Planning configuration stored in .planning-config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningConfig {
    /// Task name/identifier
    pub task_name: String,
    /// Target branch for merge
    pub target_branch: String,
    /// PRD file path (if using hybrid mode)
    pub prd_path: Option<String>,
    /// Execution mode (auto, manual)
    pub execution_mode: String,
    /// Current phase of execution
    pub phase: PlanningPhase,
    /// Story IDs that have been completed
    pub completed_stories: Vec<String>,
    /// Creation timestamp
    pub created_at: String,
    /// Last update timestamp
    pub updated_at: String,
}

impl PlanningConfig {
    /// Create a new planning config
    pub fn new(task_name: impl Into<String>, target_branch: impl Into<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            task_name: task_name.into(),
            target_branch: target_branch.into(),
            prd_path: None,
            execution_mode: "auto".to_string(),
            phase: PlanningPhase::Init,
            completed_stories: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Mark a story as complete
    pub fn complete_story(&mut self, story_id: impl Into<String>) {
        self.completed_stories.push(story_id.into());
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Set the current phase
    pub fn set_phase(&mut self, phase: PlanningPhase) {
        self.phase = phase;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}

/// Planning execution phases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanningPhase {
    /// Initial setup phase
    #[default]
    Init,
    /// PRD generation/loading
    PrdGeneration,
    /// PRD review
    PrdReview,
    /// Story execution
    Executing,
    /// All stories complete
    Complete,
}

impl std::fmt::Display for PlanningPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Init => "init",
            Self::PrdGeneration => "prd_generation",
            Self::PrdReview => "prd_review",
            Self::Executing => "executing",
            Self::Complete => "complete",
        };
        write!(f, "{}", s)
    }
}

/// Request to create a new worktree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorktreeRequest {
    /// Task name (used for worktree ID and branch name)
    pub task_name: String,
    /// Target branch to create from and merge back to
    pub target_branch: String,
    /// Base path for worktree directory (defaults to .worktree in repo root)
    pub base_path: Option<String>,
    /// Optional PRD path for hybrid mode
    pub prd_path: Option<String>,
    /// Execution mode (auto, manual)
    #[serde(default = "default_execution_mode")]
    pub execution_mode: String,
}

fn default_execution_mode() -> String {
    "auto".to_string()
}

/// Result of completing a worktree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteWorktreeResult {
    /// Whether the completion was successful
    pub success: bool,
    /// Commit SHA if code was committed
    pub commit_sha: Option<String>,
    /// Whether merge was successful
    pub merged: bool,
    /// Whether worktree was cleaned up
    pub cleaned_up: bool,
    /// Any warning messages
    pub warnings: Vec<String>,
    /// Error message if not successful
    pub error: Option<String>,
}

impl CompleteWorktreeResult {
    /// Create a successful result
    pub fn success(commit_sha: Option<String>, merged: bool, cleaned_up: bool) -> Self {
        Self {
            success: true,
            commit_sha,
            merged,
            cleaned_up,
            warnings: Vec::new(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            commit_sha: None,
            merged: false,
            cleaned_up: false,
            warnings: Vec::new(),
            error: Some(message.into()),
        }
    }

    /// Add a warning
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

/// Merge conflict information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflict {
    /// Files with conflicts
    pub conflicting_files: Vec<String>,
    /// Branch being merged from
    pub source_branch: String,
    /// Branch being merged into
    pub target_branch: String,
    /// Suggested resolution steps
    pub resolution_steps: Vec<String>,
}

impl MergeConflict {
    /// Create a new merge conflict
    pub fn new(
        conflicting_files: Vec<String>,
        source_branch: impl Into<String>,
        target_branch: impl Into<String>,
    ) -> Self {
        let source = source_branch.into();
        let target = target_branch.into();
        let resolution_steps = vec![
            format!("1. Open each conflicting file and resolve the conflicts"),
            format!("2. Stage the resolved files: git add <file>"),
            format!("3. Complete the merge: git commit"),
            format!("4. Or abort the merge: git merge --abort"),
        ];
        Self {
            conflicting_files,
            source_branch: source,
            target_branch: target,
            resolution_steps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        wt.set_status(WorktreeStatus::Completed);
        assert!(wt.status.is_terminal());
        assert!(!wt.status.can_complete());
    }

    #[test]
    fn test_planning_config() {
        let mut config = PlanningConfig::new("my-task", "main");
        assert_eq!(config.task_name, "my-task");
        assert_eq!(config.target_branch, "main");
        assert!(matches!(config.phase, PlanningPhase::Init));

        config.complete_story("story-001");
        assert!(config.completed_stories.contains(&"story-001".to_string()));

        config.set_phase(PlanningPhase::Executing);
        assert!(matches!(config.phase, PlanningPhase::Executing));
    }

    #[test]
    fn test_complete_worktree_result() {
        let result = CompleteWorktreeResult::success(Some("abc123".to_string()), true, true);
        assert!(result.success);
        assert_eq!(result.commit_sha, Some("abc123".to_string()));
        assert!(result.merged);

        let error_result = CompleteWorktreeResult::error("Something went wrong");
        assert!(!error_result.success);
        assert!(error_result.error.is_some());
    }

    #[test]
    fn test_merge_conflict() {
        let conflict = MergeConflict::new(
            vec!["file1.rs".to_string(), "file2.rs".to_string()],
            "feature/task",
            "main",
        );
        assert_eq!(conflict.conflicting_files.len(), 2);
        assert!(!conflict.resolution_steps.is_empty());
    }
}
