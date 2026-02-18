//! Pipeline and Graph Execution Commands
//!
//! Tauri commands for executing agent pipelines and graph workflows with
//! streaming events, status tracking, and cancellation support.
//!
//! ## Commands
//! - `execute_agent_pipeline` - Run an AgentComposer pipeline, stream events
//! - `execute_graph_workflow` - Run a GraphWorkflow, stream events
//! - `get_pipeline_execution_status` - Get execution progress
//! - `cancel_pipeline_execution` - Cancel a running pipeline

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::utils::error::{AppError, AppResult};

// ============================================================================
// Execution Status
// ============================================================================

/// Status of a pipeline or graph workflow execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Execution is pending (not yet started).
    Pending,
    /// Execution is actively running.
    Running,
    /// Execution completed successfully.
    Completed,
    /// Execution failed with an error.
    Failed,
    /// Execution was cancelled by the user.
    Cancelled,
}

/// Tracks the state of a pipeline or graph workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineExecutionState {
    /// Unique execution identifier.
    pub execution_id: String,
    /// Pipeline or workflow identifier being executed.
    pub pipeline_id: String,
    /// Current execution status.
    pub status: ExecutionStatus,
    /// Number of steps completed.
    pub steps_completed: usize,
    /// Total number of steps (if known).
    pub total_steps: Option<usize>,
    /// Name of the currently executing step/node.
    pub current_step: Option<String>,
    /// Error message (if status is Failed).
    pub error: Option<String>,
    /// ISO 8601 timestamp when execution started.
    pub started_at: String,
    /// ISO 8601 timestamp when execution completed (if done).
    pub completed_at: Option<String>,
}

impl PipelineExecutionState {
    /// Create a new execution state in Pending status.
    pub fn new(pipeline_id: impl Into<String>) -> Self {
        Self {
            execution_id: uuid::Uuid::new_v4().to_string(),
            pipeline_id: pipeline_id.into(),
            status: ExecutionStatus::Pending,
            steps_completed: 0,
            total_steps: None,
            current_step: None,
            error: None,
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
        }
    }

    /// Mark execution as running with the current step name.
    pub fn mark_running(&mut self, step_name: impl Into<String>) {
        self.status = ExecutionStatus::Running;
        self.current_step = Some(step_name.into());
    }

    /// Mark a step as completed.
    pub fn mark_step_completed(&mut self) {
        self.steps_completed += 1;
    }

    /// Mark execution as completed successfully.
    pub fn mark_completed(&mut self) {
        self.status = ExecutionStatus::Completed;
        self.current_step = None;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark execution as failed with an error message.
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = ExecutionStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark execution as cancelled.
    pub fn mark_cancelled(&mut self) {
        self.status = ExecutionStatus::Cancelled;
        self.current_step = None;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Check if execution is still active (pending or running).
    pub fn is_active(&self) -> bool {
        matches!(self.status, ExecutionStatus::Pending | ExecutionStatus::Running)
    }
}

// ============================================================================
// Execution Registry (in-memory tracking)
// ============================================================================

/// Thread-safe registry of active and completed executions.
///
/// Used by Tauri commands to track execution state across async boundaries.
pub struct ExecutionRegistry {
    executions: Arc<RwLock<HashMap<String, PipelineExecutionState>>>,
    /// Cancellation tokens: execution_id -> cancelled flag
    cancellation_tokens: Arc<RwLock<HashMap<String, Arc<tokio::sync::Notify>>>>,
}

impl ExecutionRegistry {
    /// Create a new empty execution registry.
    pub fn new() -> Self {
        Self {
            executions: Arc::new(RwLock::new(HashMap::new())),
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new execution and return its ID.
    pub async fn register(&self, pipeline_id: &str) -> String {
        let state = PipelineExecutionState::new(pipeline_id);
        let execution_id = state.execution_id.clone();

        let notify = Arc::new(tokio::sync::Notify::new());

        let mut executions = self.executions.write().await;
        executions.insert(execution_id.clone(), state);

        let mut tokens = self.cancellation_tokens.write().await;
        tokens.insert(execution_id.clone(), notify);

        execution_id
    }

    /// Get the current state of an execution.
    pub async fn get_status(&self, execution_id: &str) -> Option<PipelineExecutionState> {
        let executions = self.executions.read().await;
        executions.get(execution_id).cloned()
    }

    /// Update an execution's state.
    pub async fn update(&self, execution_id: &str, state: PipelineExecutionState) {
        let mut executions = self.executions.write().await;
        executions.insert(execution_id.to_string(), state);
    }

    /// Cancel an execution.
    pub async fn cancel(&self, execution_id: &str) -> bool {
        let mut executions = self.executions.write().await;
        if let Some(state) = executions.get_mut(execution_id) {
            if state.is_active() {
                state.mark_cancelled();

                // Notify cancellation
                let tokens = self.cancellation_tokens.read().await;
                if let Some(notify) = tokens.get(execution_id) {
                    notify.notify_one();
                }

                return true;
            }
        }
        false
    }

    /// Get the cancellation notify handle for an execution.
    pub async fn get_cancellation_token(
        &self,
        execution_id: &str,
    ) -> Option<Arc<tokio::sync::Notify>> {
        let tokens = self.cancellation_tokens.read().await;
        tokens.get(execution_id).cloned()
    }

    /// List all executions (active and completed).
    pub async fn list_all(&self) -> Vec<PipelineExecutionState> {
        let executions = self.executions.read().await;
        executions.values().cloned().collect()
    }

    /// List only active executions.
    pub async fn list_active(&self) -> Vec<PipelineExecutionState> {
        let executions = self.executions.read().await;
        executions
            .values()
            .filter(|s| s.is_active())
            .cloned()
            .collect()
    }

    /// Remove completed executions older than the given duration.
    pub async fn cleanup_completed(&self) {
        let mut executions = self.executions.write().await;
        let mut tokens = self.cancellation_tokens.write().await;

        let to_remove: Vec<String> = executions
            .iter()
            .filter(|(_, s)| !s.is_active())
            .map(|(id, _)| id.clone())
            .collect();

        for id in to_remove {
            executions.remove(&id);
            tokens.remove(&id);
        }
    }
}

impl Default for ExecutionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Execute an agent pipeline by ID, streaming events to the frontend.
///
/// Returns the execution ID for status tracking.
#[tauri::command]
pub async fn execute_agent_pipeline(
    state: State<'_, crate::state::AppState>,
    pipeline_id: String,
    input: Option<String>,
) -> Result<CommandResponse<String>, String> {
    // For now, return the execution ID after registering
    // Full implementation requires wiring up the AgentComposer
    let execution_id = uuid::Uuid::new_v4().to_string();

    Ok(CommandResponse::ok(execution_id))
}

/// Execute a graph workflow by ID, streaming events to the frontend.
///
/// Returns the execution ID for status tracking.
#[tauri::command]
pub async fn execute_graph_workflow_run(
    state: State<'_, crate::state::AppState>,
    workflow_id: String,
    input: Option<String>,
) -> Result<CommandResponse<String>, String> {
    let execution_id = uuid::Uuid::new_v4().to_string();

    Ok(CommandResponse::ok(execution_id))
}

/// Get the current execution status of a pipeline or workflow.
#[tauri::command]
pub async fn get_pipeline_execution_status(
    execution_id: String,
) -> Result<CommandResponse<Option<PipelineExecutionState>>, String> {
    // Without access to a shared registry in Tauri state, return None
    // This will be wired up when ExecutionRegistry is added to AppState
    Ok(CommandResponse::ok(None))
}

/// Cancel a running pipeline or workflow execution.
#[tauri::command]
pub async fn cancel_pipeline_execution(
    execution_id: String,
) -> Result<CommandResponse<bool>, String> {
    // Without access to a shared registry in Tauri state, return false
    Ok(CommandResponse::ok(false))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // ExecutionStatus Tests
    // ========================================================================

    #[test]
    fn test_execution_status_serialization() {
        let status = ExecutionStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");

        let parsed: ExecutionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ExecutionStatus::Running);
    }

    #[test]
    fn test_all_execution_statuses() {
        let statuses = vec![
            (ExecutionStatus::Pending, "\"pending\""),
            (ExecutionStatus::Running, "\"running\""),
            (ExecutionStatus::Completed, "\"completed\""),
            (ExecutionStatus::Failed, "\"failed\""),
            (ExecutionStatus::Cancelled, "\"cancelled\""),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, expected);
        }
    }

    // ========================================================================
    // PipelineExecutionState Tests
    // ========================================================================

    #[test]
    fn test_pipeline_execution_state_new() {
        let state = PipelineExecutionState::new("pipeline-1");
        assert!(!state.execution_id.is_empty());
        assert_eq!(state.pipeline_id, "pipeline-1");
        assert_eq!(state.status, ExecutionStatus::Pending);
        assert_eq!(state.steps_completed, 0);
        assert!(state.total_steps.is_none());
        assert!(state.current_step.is_none());
        assert!(state.error.is_none());
        assert!(!state.started_at.is_empty());
        assert!(state.completed_at.is_none());
        assert!(state.is_active());
    }

    #[test]
    fn test_pipeline_execution_state_mark_running() {
        let mut state = PipelineExecutionState::new("p1");
        state.mark_running("step-1");
        assert_eq!(state.status, ExecutionStatus::Running);
        assert_eq!(state.current_step.as_deref(), Some("step-1"));
        assert!(state.is_active());
    }

    #[test]
    fn test_pipeline_execution_state_mark_step_completed() {
        let mut state = PipelineExecutionState::new("p1");
        state.mark_running("step-1");
        assert_eq!(state.steps_completed, 0);

        state.mark_step_completed();
        assert_eq!(state.steps_completed, 1);

        state.mark_step_completed();
        assert_eq!(state.steps_completed, 2);
    }

    #[test]
    fn test_pipeline_execution_state_mark_completed() {
        let mut state = PipelineExecutionState::new("p1");
        state.mark_running("step-1");
        state.mark_completed();

        assert_eq!(state.status, ExecutionStatus::Completed);
        assert!(state.current_step.is_none());
        assert!(state.completed_at.is_some());
        assert!(!state.is_active());
    }

    #[test]
    fn test_pipeline_execution_state_mark_failed() {
        let mut state = PipelineExecutionState::new("p1");
        state.mark_running("step-1");
        state.mark_failed("Something went wrong");

        assert_eq!(state.status, ExecutionStatus::Failed);
        assert_eq!(state.error.as_deref(), Some("Something went wrong"));
        assert!(state.completed_at.is_some());
        assert!(!state.is_active());
    }

    #[test]
    fn test_pipeline_execution_state_mark_cancelled() {
        let mut state = PipelineExecutionState::new("p1");
        state.mark_running("step-1");
        state.mark_cancelled();

        assert_eq!(state.status, ExecutionStatus::Cancelled);
        assert!(state.current_step.is_none());
        assert!(state.completed_at.is_some());
        assert!(!state.is_active());
    }

    #[test]
    fn test_pipeline_execution_state_serialization() {
        let mut state = PipelineExecutionState::new("p1");
        state.total_steps = Some(5);
        state.mark_running("step-2");
        state.steps_completed = 1;

        let json = serde_json::to_string(&state).unwrap();
        let parsed: PipelineExecutionState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pipeline_id, "p1");
        assert_eq!(parsed.status, ExecutionStatus::Running);
        assert_eq!(parsed.total_steps, Some(5));
        assert_eq!(parsed.steps_completed, 1);
        assert_eq!(parsed.current_step.as_deref(), Some("step-2"));
    }

    // ========================================================================
    // ExecutionRegistry Tests
    // ========================================================================

    #[tokio::test]
    async fn test_execution_registry_new() {
        let registry = ExecutionRegistry::new();
        let all = registry.list_all().await;
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_execution_registry_register() {
        let registry = ExecutionRegistry::new();
        let id = registry.register("pipeline-1").await;
        assert!(!id.is_empty());

        let status = registry.get_status(&id).await;
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.pipeline_id, "pipeline-1");
        assert_eq!(status.status, ExecutionStatus::Pending);
    }

    #[tokio::test]
    async fn test_execution_registry_update() {
        let registry = ExecutionRegistry::new();
        let id = registry.register("pipeline-1").await;

        let mut state = registry.get_status(&id).await.unwrap();
        state.mark_running("step-1");
        registry.update(&id, state).await;

        let updated = registry.get_status(&id).await.unwrap();
        assert_eq!(updated.status, ExecutionStatus::Running);
    }

    #[tokio::test]
    async fn test_execution_registry_cancel() {
        let registry = ExecutionRegistry::new();
        let id = registry.register("pipeline-1").await;

        let cancelled = registry.cancel(&id).await;
        assert!(cancelled);

        let status = registry.get_status(&id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Cancelled);

        // Cancelling again should return false (already not active)
        let cancelled_again = registry.cancel(&id).await;
        assert!(!cancelled_again);
    }

    #[tokio::test]
    async fn test_execution_registry_cancel_nonexistent() {
        let registry = ExecutionRegistry::new();
        let cancelled = registry.cancel("nonexistent").await;
        assert!(!cancelled);
    }

    #[tokio::test]
    async fn test_execution_registry_list_active() {
        let registry = ExecutionRegistry::new();
        let id1 = registry.register("p1").await;
        let id2 = registry.register("p2").await;

        // Cancel one
        registry.cancel(&id1).await;

        let active = registry.list_active().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].execution_id, id2);
    }

    #[tokio::test]
    async fn test_execution_registry_list_all() {
        let registry = ExecutionRegistry::new();
        registry.register("p1").await;
        registry.register("p2").await;

        let all = registry.list_all().await;
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_execution_registry_cleanup() {
        let registry = ExecutionRegistry::new();
        let id1 = registry.register("p1").await;
        let _id2 = registry.register("p2").await;

        // Complete one
        let mut state = registry.get_status(&id1).await.unwrap();
        state.mark_completed();
        registry.update(&id1, state).await;

        // Cleanup
        registry.cleanup_completed().await;

        let all = registry.list_all().await;
        assert_eq!(all.len(), 1); // Only the active one remains
    }

    #[tokio::test]
    async fn test_execution_registry_get_cancellation_token() {
        let registry = ExecutionRegistry::new();
        let id = registry.register("p1").await;

        let token = registry.get_cancellation_token(&id).await;
        assert!(token.is_some());

        let missing = registry.get_cancellation_token("nonexistent").await;
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_execution_registry_default() {
        let registry = ExecutionRegistry::default();
        assert!(registry.list_all().await.is_empty());
    }

    #[test]
    fn test_get_status_nonexistent() {
        // Synchronous test to verify the function returns None
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let registry = ExecutionRegistry::new();
            let status = registry.get_status("nonexistent").await;
            assert!(status.is_none());
        });
    }
}
