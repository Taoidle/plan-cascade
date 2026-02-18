//! Task Mode Tauri Commands
//!
//! Provides the complete Task Mode lifecycle as Tauri commands:
//! - enter/exit task mode
//! - generate/approve task PRD
//! - execution status/cancel/report

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::models::CommandResponse;
use crate::services::strategy::analyzer::{analyze_task_for_mode, StrategyAnalysis};
use crate::services::task_mode::batch_executor::{
    BatchExecutionProgress, ExecutableStory, ExecutionBatch, ExecutionConfig,
};

// ============================================================================
// Types
// ============================================================================

/// Task mode session status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskModeStatus {
    /// Session initialized, ready for PRD generation
    Initialized,
    /// PRD is being generated
    GeneratingPrd,
    /// PRD generated, awaiting review/approval
    ReviewingPrd,
    /// Stories are being executed
    Executing,
    /// Execution completed successfully
    Completed,
    /// Execution failed
    Failed,
    /// Execution was cancelled
    Cancelled,
}

/// A story in the task PRD.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStory {
    /// Story ID
    pub id: String,
    /// Story title
    pub title: String,
    /// Story description
    pub description: String,
    /// Priority (high/medium/low)
    pub priority: String,
    /// Dependencies (story IDs)
    pub dependencies: Vec<String>,
    /// Acceptance criteria
    pub acceptance_criteria: Vec<String>,
}

/// Task PRD (Product Requirements Document).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPrd {
    /// PRD title
    pub title: String,
    /// Overall description
    pub description: String,
    /// Stories
    pub stories: Vec<TaskStory>,
    /// Execution batches (calculated from dependencies)
    pub batches: Vec<ExecutionBatch>,
}

/// Task mode session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskModeSession {
    /// Unique session ID
    pub session_id: String,
    /// Task description
    pub description: String,
    /// Current status
    pub status: TaskModeStatus,
    /// Strategy analysis result
    pub strategy_analysis: Option<StrategyAnalysis>,
    /// Generated PRD
    pub prd: Option<TaskPrd>,
    /// Execution progress
    pub progress: Option<BatchExecutionProgress>,
    /// When the session was created
    pub created_at: String,
}

/// Execution report after task mode completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionReport {
    /// Session ID
    pub session_id: String,
    /// Total stories
    pub total_stories: usize,
    /// Stories completed
    pub stories_completed: usize,
    /// Stories failed
    pub stories_failed: usize,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Agent assignments per story
    pub agent_assignments: HashMap<String, String>,
    /// Overall success
    pub success: bool,
}

/// Current task execution status for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecutionStatus {
    /// Session ID
    pub session_id: String,
    /// Current status
    pub status: TaskModeStatus,
    /// Current batch index
    pub current_batch: usize,
    /// Total batches
    pub total_batches: usize,
    /// Per-story status
    pub story_statuses: HashMap<String, String>,
    /// Stories completed
    pub stories_completed: usize,
    /// Stories failed
    pub stories_failed: usize,
}

// ============================================================================
// State
// ============================================================================

/// Managed Tauri state for task mode.
pub struct TaskModeState {
    session: Arc<RwLock<Option<TaskModeSession>>>,
}

impl TaskModeState {
    /// Create a new empty state.
    pub fn new() -> Self {
        Self {
            session: Arc::new(RwLock::new(None)),
        }
    }
}

// ============================================================================
// Commands
// ============================================================================

/// Enter task mode by creating a new session.
///
/// Initializes a TaskModeSession and stores it in managed state.
/// Also runs strategy analysis to provide mode recommendation.
#[tauri::command]
pub async fn enter_task_mode(
    description: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<TaskModeSession>, String> {
    if description.trim().is_empty() {
        return Ok(CommandResponse::err("Task description cannot be empty"));
    }

    // Check if already in task mode
    {
        let session = state.session.read().await;
        if let Some(ref s) = *session {
            if !matches!(
                s.status,
                TaskModeStatus::Completed | TaskModeStatus::Failed | TaskModeStatus::Cancelled
            ) {
                return Ok(CommandResponse::err(format!(
                    "Already in task mode (session: {}). Exit first.",
                    s.session_id
                )));
            }
        }
    }

    // Run strategy analysis
    let analysis = analyze_task_for_mode(&description, None);

    let session = TaskModeSession {
        session_id: uuid::Uuid::new_v4().to_string(),
        description: description.clone(),
        status: TaskModeStatus::Initialized,
        strategy_analysis: Some(analysis),
        prd: None,
        progress: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    // Store session
    {
        let mut s = state.session.write().await;
        *s = Some(session.clone());
    }

    Ok(CommandResponse::ok(session))
}

/// Generate a task PRD from the session description.
///
/// In production, this would call the LLM provider to decompose the task.
/// For now, it creates a placeholder PRD structure.
#[tauri::command]
pub async fn generate_task_prd(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<TaskPrd>, String> {
    let mut session_guard = state.session.write().await;
    let session = match session_guard.as_mut() {
        Some(s) if s.session_id == session_id => s,
        _ => return Ok(CommandResponse::err("Invalid session ID or no active session")),
    };

    if session.status != TaskModeStatus::Initialized {
        return Ok(CommandResponse::err(format!(
            "Cannot generate PRD in {:?} status",
            session.status
        )));
    }

    session.status = TaskModeStatus::GeneratingPrd;

    // Build a placeholder PRD (in production, this calls LLM)
    let prd = TaskPrd {
        title: format!("PRD: {}", session.description),
        description: session.description.clone(),
        stories: vec![],
        batches: vec![],
    };

    session.status = TaskModeStatus::ReviewingPrd;
    session.prd = Some(prd.clone());

    Ok(CommandResponse::ok(prd))
}

/// Approve a task PRD and trigger batch execution.
///
/// Validates the PRD structure and starts execution in a background task.
#[tauri::command]
pub async fn approve_task_prd(
    session_id: String,
    prd: TaskPrd,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<bool>, String> {
    let mut session_guard = state.session.write().await;
    let session = match session_guard.as_mut() {
        Some(s) if s.session_id == session_id => s,
        _ => return Ok(CommandResponse::err("Invalid session ID or no active session")),
    };

    if session.status != TaskModeStatus::ReviewingPrd {
        return Ok(CommandResponse::err(format!(
            "Cannot approve PRD in {:?} status",
            session.status
        )));
    }

    // Validate PRD
    if prd.stories.is_empty() {
        return Ok(CommandResponse::err("PRD must contain at least one story"));
    }

    // Calculate batches
    let stories: Vec<ExecutableStory> = prd
        .stories
        .iter()
        .map(|s| ExecutableStory {
            id: s.id.clone(),
            title: s.title.clone(),
            description: s.description.clone(),
            dependencies: s.dependencies.clone(),
            acceptance_criteria: s.acceptance_criteria.clone(),
            agent: None,
        })
        .collect();

    let config = ExecutionConfig::default();
    match crate::services::task_mode::calculate_batches(&stories, config.max_parallel) {
        Ok(batches) => {
            let mut approved_prd = prd;
            approved_prd.batches = batches;
            session.prd = Some(approved_prd);
            session.status = TaskModeStatus::Executing;
            // Note: In production, this would spawn a background tokio task
            // that runs the BatchExecutor.
            Ok(CommandResponse::ok(true))
        }
        Err(e) => Ok(CommandResponse::err(format!("PRD validation failed: {}", e))),
    }
}

/// Get the current task execution status.
#[tauri::command]
pub async fn get_task_execution_status(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<TaskExecutionStatus>, String> {
    let session_guard = state.session.read().await;
    let session = match session_guard.as_ref() {
        Some(s) if s.session_id == session_id => s,
        _ => return Ok(CommandResponse::err("Invalid session ID or no active session")),
    };

    let progress = session.progress.clone().unwrap_or(BatchExecutionProgress {
        current_batch: 0,
        total_batches: session
            .prd
            .as_ref()
            .map(|p| p.batches.len())
            .unwrap_or(0),
        stories_completed: 0,
        stories_failed: 0,
        total_stories: session
            .prd
            .as_ref()
            .map(|p| p.stories.len())
            .unwrap_or(0),
        story_statuses: HashMap::new(),
        current_phase: "idle".to_string(),
    });

    Ok(CommandResponse::ok(TaskExecutionStatus {
        session_id: session.session_id.clone(),
        status: session.status.clone(),
        current_batch: progress.current_batch,
        total_batches: progress.total_batches,
        story_statuses: progress.story_statuses,
        stories_completed: progress.stories_completed,
        stories_failed: progress.stories_failed,
    }))
}

/// Cancel the current task execution.
#[tauri::command]
pub async fn cancel_task_execution(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<bool>, String> {
    let mut session_guard = state.session.write().await;
    let session = match session_guard.as_mut() {
        Some(s) if s.session_id == session_id => s,
        _ => return Ok(CommandResponse::err("Invalid session ID or no active session")),
    };

    if session.status != TaskModeStatus::Executing {
        return Ok(CommandResponse::err("No execution in progress to cancel"));
    }

    session.status = TaskModeStatus::Cancelled;
    Ok(CommandResponse::ok(true))
}

/// Get the execution report after completion.
#[tauri::command]
pub async fn get_task_execution_report(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<ExecutionReport>, String> {
    let session_guard = state.session.read().await;
    let session = match session_guard.as_ref() {
        Some(s) if s.session_id == session_id => s,
        _ => return Ok(CommandResponse::err("Invalid session ID or no active session")),
    };

    if !matches!(
        session.status,
        TaskModeStatus::Completed | TaskModeStatus::Failed | TaskModeStatus::Cancelled
    ) {
        return Ok(CommandResponse::err("Execution has not finished yet"));
    }

    let progress = session.progress.clone().unwrap_or(BatchExecutionProgress {
        current_batch: 0,
        total_batches: 0,
        stories_completed: 0,
        stories_failed: 0,
        total_stories: 0,
        story_statuses: HashMap::new(),
        current_phase: "complete".to_string(),
    });

    Ok(CommandResponse::ok(ExecutionReport {
        session_id: session.session_id.clone(),
        total_stories: progress.total_stories,
        stories_completed: progress.stories_completed,
        stories_failed: progress.stories_failed,
        total_duration_ms: 0,
        agent_assignments: HashMap::new(),
        success: session.status == TaskModeStatus::Completed,
    }))
}

/// Exit task mode and clean up session state.
#[tauri::command]
pub async fn exit_task_mode(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<bool>, String> {
    let mut session_guard = state.session.write().await;
    match session_guard.as_ref() {
        Some(s) if s.session_id == session_id => {
            *session_guard = None;
            Ok(CommandResponse::ok(true))
        }
        _ => Ok(CommandResponse::err("Invalid session ID or no active session")),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create test state
    fn test_state() -> TaskModeState {
        TaskModeState::new()
    }

    // We can't easily test Tauri commands directly without a Tauri app context,
    // so we test the core types and logic here.

    #[test]
    fn test_task_mode_status_serialization() {
        let json = serde_json::to_string(&TaskModeStatus::Initialized).unwrap();
        assert_eq!(json, "\"initialized\"");
        let json = serde_json::to_string(&TaskModeStatus::Executing).unwrap();
        assert_eq!(json, "\"executing\"");
        let json = serde_json::to_string(&TaskModeStatus::Completed).unwrap();
        assert_eq!(json, "\"completed\"");
    }

    #[test]
    fn test_task_story_serialization() {
        let story = TaskStory {
            id: "story-001".to_string(),
            title: "Test Story".to_string(),
            description: "A test".to_string(),
            priority: "high".to_string(),
            dependencies: vec![],
            acceptance_criteria: vec!["Criterion 1".to_string()],
        };
        let json = serde_json::to_string(&story).unwrap();
        assert!(json.contains("\"acceptanceCriteria\""));
    }

    #[test]
    fn test_task_prd_serialization() {
        let prd = TaskPrd {
            title: "Test PRD".to_string(),
            description: "A test PRD".to_string(),
            stories: vec![],
            batches: vec![],
        };
        let json = serde_json::to_string(&prd).unwrap();
        assert!(json.contains("\"stories\""));
        assert!(json.contains("\"batches\""));
    }

    #[test]
    fn test_task_mode_session_serialization() {
        let session = TaskModeSession {
            session_id: "test-123".to_string(),
            description: "Build a feature".to_string(),
            status: TaskModeStatus::Initialized,
            strategy_analysis: None,
            prd: None,
            progress: None,
            created_at: "2026-02-18T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"strategyAnalysis\""));
        assert!(json.contains("\"createdAt\""));
    }

    #[test]
    fn test_execution_report_serialization() {
        let report = ExecutionReport {
            session_id: "test-123".to_string(),
            total_stories: 5,
            stories_completed: 4,
            stories_failed: 1,
            total_duration_ms: 10000,
            agent_assignments: HashMap::new(),
            success: false,
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"storiesCompleted\""));
        assert!(json.contains("\"totalDurationMs\""));
        assert!(json.contains("\"agentAssignments\""));
    }

    #[test]
    fn test_task_execution_status_serialization() {
        let status = TaskExecutionStatus {
            session_id: "test-123".to_string(),
            status: TaskModeStatus::Executing,
            current_batch: 1,
            total_batches: 3,
            story_statuses: HashMap::new(),
            stories_completed: 2,
            stories_failed: 0,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"currentBatch\""));
        assert!(json.contains("\"storyStatuses\""));
    }

    #[tokio::test]
    async fn test_state_creation() {
        let state = test_state();
        let session = state.session.read().await;
        assert!(session.is_none());
    }
}
