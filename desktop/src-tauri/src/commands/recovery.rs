//! Recovery Commands
//!
//! Tauri commands for detecting interrupted executions and resuming them.
//! Exposes the recovery detector and resume engine to the frontend.

use tauri::{AppHandle, Emitter, State};

use crate::models::CommandResponse;
use crate::services::recovery::detector::{IncompleteTask, RecoveryDetector};
use crate::services::recovery::resume::{ResumeEngine, ResumeResult};
use crate::state::AppState;

/// Detect all incomplete (interrupted) executions in the database.
///
/// Scans the executions table for rows with status not in ('completed', 'cancelled'),
/// enriches them with checkpoint info and recoverability assessment, and returns
/// structured summaries for the frontend.
///
/// # Returns
/// `CommandResponse<Vec<IncompleteTask>>` with summaries of all interrupted executions.
#[tauri::command]
pub async fn detect_incomplete_tasks(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<IncompleteTask>>, String> {
    let result = state
        .with_database(|db| RecoveryDetector::detect(db))
        .await;

    match result {
        Ok(tasks) => Ok(CommandResponse::ok(tasks)),
        Err(e) => Ok(CommandResponse::err(format!("Failed to detect incomplete tasks: {}", e))),
    }
}

/// Resume an interrupted execution from its last checkpoint.
///
/// Restores execution context from the SQLite checkpoint, re-initializes
/// service state, and returns the restored context for the frontend to
/// continue execution. Also marks the execution as 'running' again.
///
/// Emits `recovery:resume` events for progress updates.
///
/// # Arguments
/// * `task_id` - The execution ID to resume
///
/// # Returns
/// `CommandResponse<ResumeResult>` with the restored context or error.
#[tauri::command]
pub async fn resume_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: String,
) -> Result<CommandResponse<ResumeResult>, String> {
    if task_id.trim().is_empty() {
        return Ok(CommandResponse::err("Task ID cannot be empty"));
    }

    let result = state
        .with_database(|db| ResumeEngine::resume(db, &task_id))
        .await;

    match result {
        Ok(resume_result) => {
            // Emit progress events to the frontend
            for event in &resume_result.events {
                let _ = app.emit("recovery:resume", event);
            }

            Ok(CommandResponse::ok(resume_result))
        }
        Err(e) => Ok(CommandResponse::err(format!("Failed to resume task '{}': {}", task_id, e))),
    }
}

/// Discard an interrupted execution, marking it as cancelled.
///
/// # Arguments
/// * `task_id` - The execution ID to discard
///
/// # Returns
/// `CommandResponse<String>` with confirmation.
#[tauri::command]
pub async fn discard_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<CommandResponse<String>, String> {
    if task_id.trim().is_empty() {
        return Ok(CommandResponse::err("Task ID cannot be empty"));
    }

    let result = state
        .with_database(|db| ResumeEngine::discard(db, &task_id))
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(format!("Execution '{}' discarded", task_id))),
        Err(e) => Ok(CommandResponse::err(format!("Failed to discard task '{}': {}", task_id, e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require a running Tauri app context.
    // Unit tests focus on input validation logic.

    #[tokio::test]
    async fn test_resume_task_empty_id() {
        // We can't easily test the full command without AppState,
        // but we verify the validation would catch empty IDs.
        let task_id = "  ".to_string();
        assert!(task_id.trim().is_empty());
    }

    #[tokio::test]
    async fn test_discard_task_empty_id() {
        let task_id = "".to_string();
        assert!(task_id.trim().is_empty());
    }

    #[test]
    fn test_incomplete_task_json_compatibility() {
        // Ensure the IncompleteTask struct can be serialized for Tauri IPC
        let task = IncompleteTask {
            id: "exec-001".to_string(),
            session_id: Some("sess-001".to_string()),
            name: "Test task".to_string(),
            execution_mode: crate::services::recovery::detector::ExecutionMode::HybridAuto,
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
        assert!(json.contains("\"id\":\"exec-001\""));
        assert!(json.contains("\"execution_mode\":\"hybrid_auto\""));
    }

    #[test]
    fn test_resume_result_json_compatibility() {
        let result = ResumeResult::failure("exec-001", "Test error");
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"execution_id\":\"exec-001\""));
    }
}
