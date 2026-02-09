//! Resume Engine
//!
//! Restores execution context from SQLite checkpoints and resumes
//! interrupted executions from the exact point of interruption.
//! Skips already-completed stories and emits Tauri events for
//! progress updates.

use serde::{Deserialize, Serialize};

use crate::storage::database::Database;
use crate::utils::error::{AppError, AppResult};

use super::detector::ExecutionMode;

/// Context restored from a checkpoint for resumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoredContext {
    /// The execution ID being resumed
    pub execution_id: String,
    /// Execution mode
    pub execution_mode: ExecutionMode,
    /// Project path
    pub project_path: String,
    /// Task name
    pub name: String,
    /// IDs of already-completed stories to skip
    pub completed_story_ids: Vec<String>,
    /// IDs of remaining stories to execute
    pub remaining_story_ids: Vec<String>,
    /// Full context snapshot (JSON) from the database
    pub context_snapshot: serde_json::Value,
    /// Total stories
    pub total_stories: i32,
    /// Completed stories count
    pub completed_stories: i32,
    /// Current progress percentage
    pub progress: f64,
}

/// Events emitted during resume for frontend progress updates
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ResumeEvent {
    /// Resume process has started
    Started {
        execution_id: String,
        execution_mode: ExecutionMode,
        total_stories: i32,
        completed_stories: i32,
    },
    /// Context has been successfully restored
    ContextRestored {
        execution_id: String,
        remaining_stories: i32,
    },
    /// A specific story is being skipped (already completed)
    StorySkipped {
        execution_id: String,
        story_id: String,
    },
    /// Resuming execution of remaining stories
    Resuming {
        execution_id: String,
        from_story_id: Option<String>,
    },
    /// Resume completed
    Completed { execution_id: String, success: bool },
    /// Resume encountered an error
    Error {
        execution_id: String,
        message: String,
    },
}

/// Result of a resume operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeResult {
    /// Whether the resume was successful
    pub success: bool,
    /// Execution ID that was resumed
    pub execution_id: String,
    /// Restored context for continuing execution
    pub context: Option<RestoredContext>,
    /// Error message if resume failed
    pub error: Option<String>,
    /// Events generated during resume
    pub events: Vec<ResumeEvent>,
}

impl ResumeResult {
    /// Create a successful resume result
    pub fn success(execution_id: impl Into<String>, context: RestoredContext) -> Self {
        Self {
            success: true,
            execution_id: execution_id.into(),
            context: Some(context),
            error: None,
            events: Vec::new(),
        }
    }

    /// Create a failed resume result
    pub fn failure(execution_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            execution_id: execution_id.into(),
            context: None,
            error: Some(error.into()),
            events: Vec::new(),
        }
    }

    /// Add an event to the result
    pub fn with_event(mut self, event: ResumeEvent) -> Self {
        self.events.push(event);
        self
    }
}

/// Engine for resuming interrupted executions
pub struct ResumeEngine;

impl ResumeEngine {
    /// Resume an interrupted execution by ID.
    ///
    /// 1. Loads the execution record from SQLite
    /// 2. Validates that the execution exists and is resumable
    /// 3. Parses the context snapshot
    /// 4. Determines completed vs remaining stories
    /// 5. Updates execution status to 'running'
    /// 6. Returns the restored context for the caller to continue execution
    pub fn resume(db: &Database, execution_id: &str) -> AppResult<ResumeResult> {
        // Step 1: Load the execution record
        let execution = db.get_execution(execution_id)?.ok_or_else(|| {
            AppError::not_found(format!("Execution '{}' not found", execution_id))
        })?;

        let mode = ExecutionMode::from_str(&execution.execution_mode);

        let mut result = ResumeResult {
            success: false,
            execution_id: execution_id.to_string(),
            context: None,
            error: None,
            events: vec![ResumeEvent::Started {
                execution_id: execution_id.to_string(),
                execution_mode: mode.clone(),
                total_stories: execution.total_stories,
                completed_stories: execution.completed_stories,
            }],
        };

        // Step 2: Validate resumability
        if execution.status == "completed" || execution.status == "cancelled" {
            result.error = Some(format!(
                "Execution '{}' has status '{}' and cannot be resumed",
                execution_id, execution.status
            ));
            result.events.push(ResumeEvent::Error {
                execution_id: execution_id.to_string(),
                message: result.error.clone().unwrap(),
            });
            return Ok(result);
        }

        // Step 3: Parse the context snapshot
        let context_value: serde_json::Value = serde_json::from_str(&execution.context_snapshot)
            .map_err(|e| {
                AppError::parse(format!(
                    "Failed to parse context snapshot for execution '{}': {}",
                    execution_id, e
                ))
            })?;

        // Step 4: Extract completed and remaining story IDs from the context
        let (completed_ids, remaining_ids) = Self::extract_story_progress(&context_value, &mode);

        // Record skip events
        for story_id in &completed_ids {
            result.events.push(ResumeEvent::StorySkipped {
                execution_id: execution_id.to_string(),
                story_id: story_id.clone(),
            });
        }

        // Step 5: Build the restored context
        let restored = RestoredContext {
            execution_id: execution_id.to_string(),
            execution_mode: mode.clone(),
            project_path: execution.project_path.clone(),
            name: execution.name.clone(),
            completed_story_ids: completed_ids,
            remaining_story_ids: remaining_ids.clone(),
            context_snapshot: context_value,
            total_stories: execution.total_stories,
            completed_stories: execution.completed_stories,
            progress: execution.progress,
        };

        result.events.push(ResumeEvent::ContextRestored {
            execution_id: execution_id.to_string(),
            remaining_stories: remaining_ids.len() as i32,
        });

        // Step 6: Update execution status to 'running'
        db.update_execution_status(execution_id, "running", None)?;

        // Determine the next story to execute
        let next_story_id = remaining_ids.first().cloned();
        result.events.push(ResumeEvent::Resuming {
            execution_id: execution_id.to_string(),
            from_story_id: next_story_id,
        });

        result.success = true;
        result.context = Some(restored);

        Ok(result)
    }

    /// Discard an interrupted execution, marking it as cancelled.
    pub fn discard(db: &Database, execution_id: &str) -> AppResult<()> {
        let execution = db.get_execution(execution_id)?.ok_or_else(|| {
            AppError::not_found(format!("Execution '{}' not found", execution_id))
        })?;

        if execution.status == "completed" {
            return Err(AppError::validation(
                "Cannot discard a completed execution".to_string(),
            ));
        }

        db.update_execution_status(execution_id, "cancelled", Some("Discarded by user"))?;
        Ok(())
    }

    /// Extract completed and remaining story IDs from the context snapshot.
    ///
    /// The context snapshot format varies by execution mode, but generally
    /// includes a list of completed story IDs and the full PRD story list.
    fn extract_story_progress(
        context: &serde_json::Value,
        _mode: &ExecutionMode,
    ) -> (Vec<String>, Vec<String>) {
        let mut completed = Vec::new();
        let mut remaining = Vec::new();

        // Try to extract completed story IDs from known context fields
        if let Some(completed_arr) = context
            .get("completed_story_ids")
            .and_then(|v| v.as_array())
        {
            for id in completed_arr {
                if let Some(s) = id.as_str() {
                    completed.push(s.to_string());
                }
            }
        }

        // Try to extract all story IDs from PRD
        if let Some(stories_arr) = context
            .get("prd")
            .and_then(|p| p.get("stories"))
            .and_then(|s| s.as_array())
        {
            for story in stories_arr {
                if let Some(id) = story.get("id").and_then(|v| v.as_str()) {
                    if !completed.contains(&id.to_string()) {
                        remaining.push(id.to_string());
                    }
                }
            }
        }

        // Alternative: stories might be at top level
        if remaining.is_empty() {
            if let Some(stories_arr) = context.get("stories").and_then(|s| s.as_array()) {
                for story in stories_arr {
                    if let Some(id) = story.get("id").and_then(|v| v.as_str()) {
                        // Check story status
                        let status = story
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("pending");
                        if status == "completed" {
                            if !completed.contains(&id.to_string()) {
                                completed.push(id.to_string());
                            }
                        } else {
                            remaining.push(id.to_string());
                        }
                    }
                }
            }
        }

        // Alternative: remaining_story_ids field
        if remaining.is_empty() {
            if let Some(remaining_arr) = context
                .get("remaining_story_ids")
                .and_then(|v| v.as_array())
            {
                for id in remaining_arr {
                    if let Some(s) = id.as_str() {
                        remaining.push(s.to_string());
                    }
                }
            }
        }

        (completed, remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resume_result_success() {
        let context = RestoredContext {
            execution_id: "exec-001".to_string(),
            execution_mode: ExecutionMode::HybridAuto,
            project_path: "/project".to_string(),
            name: "Test task".to_string(),
            completed_story_ids: vec!["s1".to_string()],
            remaining_story_ids: vec!["s2".to_string(), "s3".to_string()],
            context_snapshot: serde_json::json!({}),
            total_stories: 3,
            completed_stories: 1,
            progress: 33.3,
        };

        let result = ResumeResult::success("exec-001", context);
        assert!(result.success);
        assert!(result.context.is_some());
        assert!(result.error.is_none());
        assert_eq!(result.execution_id, "exec-001");

        let ctx = result.context.unwrap();
        assert_eq!(ctx.completed_story_ids.len(), 1);
        assert_eq!(ctx.remaining_story_ids.len(), 2);
    }

    #[test]
    fn test_resume_result_failure() {
        let result = ResumeResult::failure("exec-001", "Snapshot corrupted");
        assert!(!result.success);
        assert!(result.context.is_none());
        assert_eq!(result.error, Some("Snapshot corrupted".to_string()));
    }

    #[test]
    fn test_resume_result_with_event() {
        let result = ResumeResult::failure("exec-001", "test").with_event(ResumeEvent::Error {
            execution_id: "exec-001".to_string(),
            message: "test error".to_string(),
        });
        assert_eq!(result.events.len(), 1);
    }

    #[test]
    fn test_extract_story_progress_from_completed_ids() {
        let context = serde_json::json!({
            "completed_story_ids": ["s1", "s2"],
            "prd": {
                "stories": [
                    {"id": "s1", "title": "Story 1"},
                    {"id": "s2", "title": "Story 2"},
                    {"id": "s3", "title": "Story 3"},
                    {"id": "s4", "title": "Story 4"},
                ]
            }
        });

        let (completed, remaining) =
            ResumeEngine::extract_story_progress(&context, &ExecutionMode::HybridAuto);

        assert_eq!(completed, vec!["s1", "s2"]);
        assert_eq!(remaining, vec!["s3", "s4"]);
    }

    #[test]
    fn test_extract_story_progress_from_story_status() {
        let context = serde_json::json!({
            "stories": [
                {"id": "s1", "title": "Story 1", "status": "completed"},
                {"id": "s2", "title": "Story 2", "status": "completed"},
                {"id": "s3", "title": "Story 3", "status": "in_progress"},
                {"id": "s4", "title": "Story 4", "status": "pending"},
            ]
        });

        let (completed, remaining) =
            ResumeEngine::extract_story_progress(&context, &ExecutionMode::MegaPlan);

        assert_eq!(completed, vec!["s1", "s2"]);
        assert_eq!(remaining, vec!["s3", "s4"]);
    }

    #[test]
    fn test_extract_story_progress_from_remaining_ids() {
        let context = serde_json::json!({
            "completed_story_ids": ["s1"],
            "remaining_story_ids": ["s2", "s3"],
        });

        let (completed, remaining) =
            ResumeEngine::extract_story_progress(&context, &ExecutionMode::Direct);

        assert_eq!(completed, vec!["s1"]);
        assert_eq!(remaining, vec!["s2", "s3"]);
    }

    #[test]
    fn test_extract_story_progress_empty_context() {
        let context = serde_json::json!({});

        let (completed, remaining) =
            ResumeEngine::extract_story_progress(&context, &ExecutionMode::Direct);

        assert!(completed.is_empty());
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_resume_event_serialization() {
        let event = ResumeEvent::Started {
            execution_id: "exec-001".to_string(),
            execution_mode: ExecutionMode::HybridWorktree,
            total_stories: 5,
            completed_stories: 2,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"Started\""));
        assert!(json.contains("\"hybrid_worktree\""));
    }

    #[test]
    fn test_restored_context_serialization() {
        let context = RestoredContext {
            execution_id: "exec-001".to_string(),
            execution_mode: ExecutionMode::MegaPlan,
            project_path: "/my/project".to_string(),
            name: "Build app".to_string(),
            completed_story_ids: vec!["s1".to_string()],
            remaining_story_ids: vec!["s2".to_string()],
            context_snapshot: serde_json::json!({"key": "value"}),
            total_stories: 2,
            completed_stories: 1,
            progress: 50.0,
        };

        let json = serde_json::to_string(&context).unwrap();
        let parsed: RestoredContext = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.execution_id, "exec-001");
        assert_eq!(parsed.execution_mode, ExecutionMode::MegaPlan);
        assert_eq!(parsed.remaining_story_ids, vec!["s2"]);
    }

    #[test]
    fn test_resume_result_serialization() {
        let result = ResumeResult::failure("exec-001", "test error");
        let json = serde_json::to_string(&result).unwrap();
        let parsed: ResumeResult = serde_json::from_str(&json).unwrap();
        assert!(!parsed.success);
        assert_eq!(parsed.execution_id, "exec-001");
        assert_eq!(parsed.error, Some("test error".to_string()));
    }
}
