//! Recovery Detector
//!
//! Scans the SQLite database for interrupted executions. Checks the executions
//! table for records with status != 'completed' and status != 'cancelled',
//! and the checkpoints table for incomplete checkpoint chains.
//!
//! Supports all execution modes: mega_plan, hybrid_auto, hybrid_worktree, direct.
//! Produces structured `IncompleteTask` summaries for the frontend.

use serde::{Deserialize, Serialize};

use crate::storage::database::Database;
use crate::utils::error::AppResult;

/// Execution mode for the interrupted task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Simple direct execution
    Direct,
    /// Multi-story with automatic PRD
    HybridAuto,
    /// Multi-story with Git worktree isolation
    HybridWorktree,
    /// Multi-feature project orchestration
    MegaPlan,
}

impl ExecutionMode {
    /// Parse from a database string value
    pub fn from_str(s: &str) -> Self {
        match s {
            "hybrid_auto" => ExecutionMode::HybridAuto,
            "hybrid_worktree" => ExecutionMode::HybridWorktree,
            "mega_plan" => ExecutionMode::MegaPlan,
            _ => ExecutionMode::Direct,
        }
    }

    /// Human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            ExecutionMode::Direct => "Direct",
            ExecutionMode::HybridAuto => "Hybrid Auto",
            ExecutionMode::HybridWorktree => "Hybrid Worktree",
            ExecutionMode::MegaPlan => "Mega Plan",
        }
    }
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Direct => write!(f, "direct"),
            ExecutionMode::HybridAuto => write!(f, "hybrid_auto"),
            ExecutionMode::HybridWorktree => write!(f, "hybrid_worktree"),
            ExecutionMode::MegaPlan => write!(f, "mega_plan"),
        }
    }
}

/// Summary of an interrupted execution found during detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncompleteTask {
    /// Unique execution identifier
    pub id: String,
    /// Associated session identifier (if any)
    pub session_id: Option<String>,
    /// Human-readable task name
    pub name: String,
    /// Execution mode that was running
    pub execution_mode: ExecutionMode,
    /// Current status when interrupted (running, paused, failed)
    pub status: String,
    /// Project path for the execution
    pub project_path: String,
    /// Total number of stories in the execution
    pub total_stories: i32,
    /// Number of stories completed before interruption
    pub completed_stories: i32,
    /// ID of the story that was active when interrupted
    pub current_story_id: Option<String>,
    /// Progress percentage (0.0 - 100.0)
    pub progress: f64,
    /// Timestamp of the last checkpoint or update (ISO 8601)
    pub last_checkpoint_timestamp: Option<String>,
    /// Whether the execution can be resumed
    pub recoverable: bool,
    /// Reason if not recoverable
    pub recovery_note: Option<String>,
    /// Number of checkpoints available
    pub checkpoint_count: i32,
    /// Error message if the execution had failed
    pub error_message: Option<String>,
}

/// Service for detecting interrupted executions in the database
pub struct RecoveryDetector;

impl RecoveryDetector {
    /// Scan the database for all incomplete executions.
    ///
    /// Queries the `executions` table for rows with status not in
    /// ('completed', 'cancelled'), then enriches each result with
    /// checkpoint information and recoverability assessment.
    pub fn detect(db: &Database) -> AppResult<Vec<IncompleteTask>> {
        let rows = db.get_incomplete_executions()?;

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let mut tasks = Vec::with_capacity(rows.len());

        for row in rows {
            // Get checkpoints for this session if available
            let (checkpoint_count, last_checkpoint_ts) = if let Some(ref sid) = row.session_id {
                match db.get_checkpoints_for_session(sid) {
                    Ok(checkpoints) => {
                        let count = checkpoints.len() as i32;
                        let last_ts = checkpoints.first().and_then(|c| c.created_at.clone());
                        (count, last_ts)
                    }
                    Err(_) => (0, None),
                }
            } else {
                (0, None)
            };

            // Use execution updated_at as fallback for last activity timestamp
            let last_timestamp = last_checkpoint_ts.or_else(|| row.updated_at.clone());

            // Assess recoverability
            let (recoverable, recovery_note) = Self::assess_recoverability(
                &row.execution_mode,
                &row.status,
                &row.context_snapshot,
                row.total_stories,
                row.completed_stories,
            );

            let mode = ExecutionMode::from_str(&row.execution_mode);

            tasks.push(IncompleteTask {
                id: row.id,
                session_id: row.session_id,
                name: row.name,
                execution_mode: mode,
                status: row.status,
                project_path: row.project_path,
                total_stories: row.total_stories,
                completed_stories: row.completed_stories,
                current_story_id: row.current_story_id,
                progress: row.progress,
                last_checkpoint_timestamp: last_timestamp,
                recoverable,
                recovery_note,
                checkpoint_count,
                error_message: row.error_message,
            });
        }

        Ok(tasks)
    }

    /// Assess whether an interrupted execution can be resumed.
    ///
    /// Checks for valid context snapshot, progress state, and execution mode
    /// to determine if resumption is safe.
    fn assess_recoverability(
        execution_mode: &str,
        status: &str,
        context_snapshot: &str,
        total_stories: i32,
        completed_stories: i32,
    ) -> (bool, Option<String>) {
        // Check 1: Context snapshot must exist and be valid JSON
        if context_snapshot.is_empty() || context_snapshot == "{}" {
            // Direct mode may not need a context snapshot
            if execution_mode == "direct" && completed_stories == 0 {
                return (true, Some("Direct execution will restart from beginning".to_string()));
            }
            return (
                false,
                Some("No execution context snapshot available for recovery".to_string()),
            );
        }

        // Check 2: Parse the context snapshot to validate it
        if serde_json::from_str::<serde_json::Value>(context_snapshot).is_err() {
            return (
                false,
                Some("Execution context snapshot is corrupted".to_string()),
            );
        }

        // Check 3: If all stories completed but status isn't completed,
        // it likely failed during finalization
        if total_stories > 0 && completed_stories >= total_stories {
            return (
                true,
                Some("All stories completed; finalization can be retried".to_string()),
            );
        }

        // Check 4: Failed executions can be retried from last good checkpoint
        if status == "failed" {
            return (
                true,
                Some("Execution failed; will resume from last completed story".to_string()),
            );
        }

        // Check 5: Running or paused executions are generally recoverable
        if status == "running" || status == "paused" || status == "pending" {
            return (true, None);
        }

        (true, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_mode_from_str() {
        assert_eq!(ExecutionMode::from_str("direct"), ExecutionMode::Direct);
        assert_eq!(ExecutionMode::from_str("hybrid_auto"), ExecutionMode::HybridAuto);
        assert_eq!(ExecutionMode::from_str("hybrid_worktree"), ExecutionMode::HybridWorktree);
        assert_eq!(ExecutionMode::from_str("mega_plan"), ExecutionMode::MegaPlan);
        assert_eq!(ExecutionMode::from_str("unknown"), ExecutionMode::Direct);
    }

    #[test]
    fn test_execution_mode_label() {
        assert_eq!(ExecutionMode::Direct.label(), "Direct");
        assert_eq!(ExecutionMode::HybridAuto.label(), "Hybrid Auto");
        assert_eq!(ExecutionMode::HybridWorktree.label(), "Hybrid Worktree");
        assert_eq!(ExecutionMode::MegaPlan.label(), "Mega Plan");
    }

    #[test]
    fn test_execution_mode_display() {
        assert_eq!(format!("{}", ExecutionMode::Direct), "direct");
        assert_eq!(format!("{}", ExecutionMode::HybridAuto), "hybrid_auto");
        assert_eq!(format!("{}", ExecutionMode::HybridWorktree), "hybrid_worktree");
        assert_eq!(format!("{}", ExecutionMode::MegaPlan), "mega_plan");
    }

    #[test]
    fn test_execution_mode_serialization() {
        let mode = ExecutionMode::HybridAuto;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"hybrid_auto\"");

        let parsed: ExecutionMode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ExecutionMode::HybridAuto);
    }

    #[test]
    fn test_assess_recoverability_empty_context_direct() {
        let (recoverable, note) = RecoveryDetector::assess_recoverability(
            "direct", "running", "{}", 0, 0,
        );
        assert!(recoverable);
        assert!(note.is_some());
        assert!(note.unwrap().contains("restart"));
    }

    #[test]
    fn test_assess_recoverability_empty_context_hybrid() {
        let (recoverable, note) = RecoveryDetector::assess_recoverability(
            "hybrid_auto", "running", "{}", 5, 2,
        );
        assert!(!recoverable);
        assert!(note.is_some());
        assert!(note.unwrap().contains("No execution context"));
    }

    #[test]
    fn test_assess_recoverability_corrupted_json() {
        let (recoverable, note) = RecoveryDetector::assess_recoverability(
            "hybrid_auto", "running", "{invalid json", 5, 2,
        );
        assert!(!recoverable);
        assert!(note.unwrap().contains("corrupted"));
    }

    #[test]
    fn test_assess_recoverability_all_stories_done() {
        let (recoverable, note) = RecoveryDetector::assess_recoverability(
            "hybrid_auto", "running", r#"{"stories":[]}"#, 5, 5,
        );
        assert!(recoverable);
        assert!(note.unwrap().contains("finalization"));
    }

    #[test]
    fn test_assess_recoverability_failed() {
        let (recoverable, note) = RecoveryDetector::assess_recoverability(
            "mega_plan", "failed", r#"{"stories":[]}"#, 10, 3,
        );
        assert!(recoverable);
        assert!(note.unwrap().contains("failed"));
    }

    #[test]
    fn test_assess_recoverability_running_with_context() {
        let (recoverable, note) = RecoveryDetector::assess_recoverability(
            "hybrid_worktree",
            "running",
            r#"{"prd":{"stories":[]},"completed":["s1","s2"]}"#,
            5,
            2,
        );
        assert!(recoverable);
        assert!(note.is_none());
    }

    #[test]
    fn test_incomplete_task_serialization() {
        let task = IncompleteTask {
            id: "exec-001".to_string(),
            session_id: Some("sess-001".to_string()),
            name: "Build auth system".to_string(),
            execution_mode: ExecutionMode::HybridAuto,
            status: "running".to_string(),
            project_path: "/project".to_string(),
            total_stories: 5,
            completed_stories: 2,
            current_story_id: Some("story-3".to_string()),
            progress: 40.0,
            last_checkpoint_timestamp: Some("2025-01-15T10:30:00Z".to_string()),
            recoverable: true,
            recovery_note: None,
            checkpoint_count: 3,
            error_message: None,
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"execution_mode\":\"hybrid_auto\""));
        assert!(json.contains("\"total_stories\":5"));
        assert!(json.contains("\"completed_stories\":2"));

        let parsed: IncompleteTask = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "exec-001");
        assert_eq!(parsed.execution_mode, ExecutionMode::HybridAuto);
        assert_eq!(parsed.progress, 40.0);
    }
}
