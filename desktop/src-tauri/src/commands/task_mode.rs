//! Task Mode Tauri Commands
//!
//! Provides the complete Task Mode lifecycle as Tauri commands:
//! - enter/exit task mode
//! - generate/approve task PRD
//! - execution status/cancel/report

use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::models::CommandResponse;
use crate::services::design::GenerateResult;
use crate::services::knowledge::pipeline::ScopedDocumentRef;
use crate::services::strategy::analyzer::{analyze_task_for_mode, StrategyAnalysis};
use crate::services::task_mode::agent_resolver::AgentResolver;
use crate::services::task_mode::batch_executor::{
    BatchExecutionProgress, BatchExecutionResult, BatchExecutor, ExecutableStory, ExecutionBatch,
    ExecutionConfig, StoryContext, StoryExecutionContext, StoryExecutionOutcome,
    StoryExecutionState, TaskModeProgressEvent, TASK_MODE_EVENT_CHANNEL,
};
use crate::services::task_mode::exploration::{
    self, ExplorationResult, SummaryQuality, SummarySource,
};
use crate::services::task_mode::prd_generator;
use crate::services::workflow_kernel::{
    HandoffContextBundle, WorkflowKernelState, WorkflowKernelUpdatedEvent, WorkflowMode,
    WorkflowStatus,
    WORKFLOW_KERNEL_UPDATED_CHANNEL,
};

use crate::state::AppState;
use crate::storage::{ConfigService, KeyringService};
use crate::utils::paths::ensure_plan_cascade_dir;
use tauri::Emitter;

// ============================================================================
// Types
// ============================================================================

/// Bundled knowledge tool parameters for passing through closures/spawns.
///
/// When LLM mode story execution has knowledge enabled, these params are
/// pre-computed before tokio::spawn and passed to `execute_story_via_llm`
/// to wire SearchKnowledge on-demand tool access.
#[derive(Clone)]
struct KnowledgeToolParams {
    pipeline: Arc<crate::services::knowledge::pipeline::RagPipeline>,
    project_id: String,
    collection_filter: Option<Vec<String>>,
    document_filter: Option<Vec<ScopedDocumentRef>>,
    awareness_section: String,
}

/// Execution mode for story execution in Task Mode.
///
/// Determines whether stories are executed via external CLI tools
/// or directly via the built-in LLM API through OrchestratorService.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoryExecutionMode {
    /// Use external CLI tools (claude, codex, aider)
    Cli,
    /// Use direct LLM API via OrchestratorService
    Llm,
}

/// Task mode session status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskModeStatus {
    /// Session initialized, ready for PRD generation
    Initialized,
    /// Project exploration in progress
    Exploring,
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
    /// Project exploration result
    pub exploration_result: Option<ExplorationResult>,
    /// Execution progress
    pub progress: Option<BatchExecutionProgress>,
    /// Persisted execution launch metadata used for background resume.
    #[serde(default)]
    pub execution_resume_payload: Option<Value>,
    /// When the session was created
    pub created_at: String,
}

/// A quality dimension score for a single story (e.g., correctness, readability).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityDimensionScore {
    /// Story ID this score belongs to
    pub story_id: String,
    /// Quality dimension name (e.g., "correctness", "readability")
    pub dimension: String,
    /// Achieved score
    pub score: f64,
    /// Maximum possible score
    pub max_score: f64,
}

/// A timeline entry representing one story's execution span.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    /// Story ID
    pub story_id: String,
    /// Story title
    pub story_title: String,
    /// Batch index (0-based) this story belonged to
    pub batch_index: usize,
    /// Agent that executed the story
    pub agent: String,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Start offset from overall execution start in milliseconds
    pub start_offset_ms: u64,
    /// Final status ("completed", "failed", "cancelled")
    pub status: String,
    /// Summary of quality gate result (if gates were run)
    pub gate_result: Option<String>,
}

/// Aggregated performance metrics for a single agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPerformanceEntry {
    /// Agent name
    pub agent_name: String,
    /// Number of stories assigned to this agent
    pub stories_assigned: usize,
    /// Number of stories completed successfully by this agent
    pub stories_completed: usize,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Average duration in milliseconds for completed stories
    pub average_duration_ms: u64,
    /// Average quality score across all quality dimensions (if available)
    pub average_quality_score: Option<f64>,
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
    /// Per-story quality dimension scores (empty when no gate results available)
    pub quality_scores: Vec<QualityDimensionScore>,
    /// Timeline entries for waterfall visualization
    pub timeline: Vec<TimelineEntry>,
    /// Aggregated per-agent performance metrics
    pub agent_performance: Vec<AgentPerformanceEntry>,
}

/// A conversation turn from the Chat mode, passed via IPC for context sharing.
///
/// Used to provide the PRD generation LLM with the full conversation history
/// from the Chat session, enabling cross-mode context awareness.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurnInput {
    /// User message content
    pub user: String,
    /// Assistant response content
    pub assistant: String,
}

/// Phase agent configuration from the frontend Settings panel.
///
/// Maps to `PhaseConfig` in the agent resolver. Values like `"llm:anthropic:claude-sonnet-4-20250514"`
/// are stored as-is and passed through to the agent resolver for LLM-mode execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseConfigInput {
    /// Default agent for this phase (e.g., "claude-code" or "llm:anthropic:claude-sonnet-4-20250514")
    pub default_agent: String,
    /// Fallback chain of agent names
    #[serde(default)]
    pub fallback_chain: Vec<String>,
}

/// Workflow configuration from the frontend orchestrator.
///
/// Passed through from the Simple Mode workflow to control execution behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskWorkflowConfig {
    /// Flow level: quick/standard/full
    pub flow_level: Option<String>,
    /// TDD mode: off/flexible/strict
    pub tdd_mode: Option<String>,
    /// Whether to enable spec interview
    pub enable_interview: bool,
    /// Maximum parallel stories
    pub max_parallel: Option<usize>,
    /// Skip verification gates (--no-verify)
    pub skip_verification: bool,
    /// Skip code review gate (--no-review)
    pub skip_review: bool,
    /// Override all agents with this agent name
    pub global_agent_override: Option<String>,
    /// Override implementation agents only
    pub impl_agent_override: Option<String>,
}

/// Request payload for `generate_task_prd`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateTaskPrdRequest {
    pub session_id: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub compiled_spec: Option<serde_json::Value>,
    pub conversation_history: Option<Vec<ConversationTurnInput>>,
    pub max_context_tokens: Option<usize>,
    pub context_sources: Option<crate::services::task_mode::context_provider::ContextSourceConfig>,
    pub project_path: Option<String>,
}

/// Request payload for `explore_project`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExploreProjectRequest {
    pub session_id: String,
    pub flow_level: String,
    pub task_description: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub locale: Option<String>,
    pub context_sources: Option<crate::services::task_mode::context_provider::ContextSourceConfig>,
}

/// Request payload for `approve_task_prd`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApproveTaskPrdRequest {
    pub session_id: String,
    pub prd: TaskPrd,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub execution_mode: Option<StoryExecutionMode>,
    pub workflow_config: Option<TaskWorkflowConfig>,
    pub global_default_agent: Option<String>,
    pub phase_configs: Option<HashMap<String, PhaseConfigInput>>,
    pub context_sources: Option<crate::services::task_mode::context_provider::ContextSourceConfig>,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecutionResumePayload {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub execution_mode: Option<StoryExecutionMode>,
    pub workflow_config: Option<TaskWorkflowConfig>,
    pub global_default_agent: Option<String>,
    pub phase_configs: Option<HashMap<String, PhaseConfigInput>>,
    pub context_sources: Option<crate::services::task_mode::context_provider::ContextSourceConfig>,
    pub project_path: Option<String>,
}

/// Request payload for `run_requirement_analysis`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunRequirementAnalysisRequest {
    pub session_id: String,
    pub task_description: String,
    pub interview_result: Option<String>,
    pub exploration_context: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub locale: Option<String>,
    pub context_sources: Option<crate::services::task_mode::context_provider::ContextSourceConfig>,
    pub project_path: Option<String>,
}

/// Request payload for `run_architecture_review`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunArchitectureReviewRequest {
    pub session_id: String,
    pub prd_json: String,
    pub exploration_context: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub locale: Option<String>,
    pub context_sources: Option<crate::services::task_mode::context_provider::ContextSourceConfig>,
    pub project_path: Option<String>,
}

/// Request payload for `apply_task_prd_feedback`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyTaskPrdFeedbackRequest {
    pub session_id: String,
    pub feedback: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub conversation_history: Option<Vec<ConversationTurnInput>>,
    pub max_context_tokens: Option<usize>,
    pub locale: Option<String>,
    pub context_sources: Option<crate::services::task_mode::context_provider::ContextSourceConfig>,
    pub project_path: Option<String>,
}

/// Structured summary for PRD feedback application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrdFeedbackApplySummary {
    pub added_story_ids: Vec<String>,
    pub removed_story_ids: Vec<String>,
    pub updated_story_ids: Vec<String>,
    pub batch_changes: Vec<String>,
    pub warnings: Vec<String>,
}

/// Response payload for `apply_task_prd_feedback`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrdFeedbackApplyResult {
    pub prd: TaskPrd,
    pub summary: PrdFeedbackApplySummary,
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
// Persona & New Phase Types
// ============================================================================

/// Result of the requirement analysis phase (ProductManager persona).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementAnalysisResult {
    /// Natural language PM analysis (shown in card as markdown)
    pub analysis: String,
    /// Key requirements identified by the PM
    pub key_requirements: Vec<String>,
    /// Gaps identified in the requirements
    pub identified_gaps: Vec<String>,
    /// Suggested scope summary
    pub suggested_scope: String,
    /// Persona role that produced this analysis
    pub persona_role: String,
}

/// A concern identified during architecture review.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewConcern {
    /// Severity: "high", "medium", or "low"
    pub severity: String,
    /// Description of the concern
    pub description: String,
}

/// A suggested PRD modification from the architect.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrdModificationPayloadStory {
    /// Optional story ID (generated if absent)
    #[serde(default)]
    pub id: Option<String>,
    /// Story title
    pub title: String,
    /// Story description
    pub description: String,
    /// Story priority
    pub priority: String,
    /// Story dependencies
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Story acceptance criteria
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
}

/// Structured payload describing how to patch the PRD.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrdModificationPayload {
    /// Updated title (for update/merge)
    #[serde(default)]
    pub title: Option<String>,
    /// Updated description (for update/merge)
    #[serde(default)]
    pub description: Option<String>,
    /// Updated priority (for update/merge)
    #[serde(default)]
    pub priority: Option<String>,
    /// Updated dependencies (for update/merge)
    #[serde(default)]
    pub dependencies: Option<Vec<String>>,
    /// Updated acceptance criteria (for update/merge)
    #[serde(default)]
    pub acceptance_criteria: Option<Vec<String>>,
    /// Single story payload (for add/update)
    #[serde(default)]
    pub story: Option<PrdModificationPayloadStory>,
    /// Multi-story payload (for split)
    #[serde(default)]
    pub stories: Vec<PrdModificationPayloadStory>,
    /// Dependency remap map used by split operations
    #[serde(default)]
    pub dependency_remap: HashMap<String, Vec<String>>,
}

/// A suggested PRD modification from the architect.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrdModification {
    /// Stable operation ID for this suggestion
    pub operation_id: String,
    /// Operation type: update_story, add_story, remove_story, split_story, merge_story
    #[serde(rename = "type")]
    pub modification_type: String,
    /// Target story ID when applicable
    #[serde(default)]
    pub target_story_id: Option<String>,
    /// Structured payload that frontend can apply directly
    #[serde(default)]
    pub payload: PrdModificationPayload,
    /// One-line preview shown in the card
    pub preview: String,
    /// Reason for the modification
    pub reason: String,
    /// Confidence score in [0,1]
    pub confidence: f64,
}

/// Result of the architecture review phase (SoftwareArchitect persona).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureReviewResult {
    /// Natural language architecture analysis (shown in card as markdown)
    pub analysis: String,
    /// Architectural concerns with severity
    pub concerns: Vec<ReviewConcern>,
    /// Improvement suggestions
    pub suggestions: Vec<String>,
    /// Suggested PRD modifications (user can accept/reject)
    pub prd_modifications: Vec<PrdModification>,
    /// Whether the architect approves the PRD as-is
    pub approved: bool,
    /// Persona role that produced this analysis
    pub persona_role: String,
}

// ============================================================================
// State
// ============================================================================

/// Managed Tauri state for task mode.
#[derive(Clone)]
pub struct TaskModeState {
    sessions: Arc<RwLock<HashMap<String, TaskModeSession>>>,
    /// Cancellation tokens keyed by executing session id.
    cancellation_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
    /// Cancellation tokens keyed by session id for pre-execution operations.
    operation_cancellation_tokens: Arc<RwLock<HashMap<String, (String, CancellationToken)>>>,
    /// Final execution results keyed by session id.
    execution_results: Arc<RwLock<HashMap<String, BatchExecutionResult>>>,
    storage_root: Arc<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskModeSessionRecordV1 {
    version: u32,
    session: TaskModeSession,
}

const TASK_MODE_SESSION_RECORD_VERSION: u32 = 1;

impl TaskModeState {
    /// Create a new empty state.
    pub fn new() -> Self {
        Self::new_with_storage_dir(resolve_task_mode_storage_root())
    }

    pub fn new_with_storage_dir(storage_root: PathBuf) -> Self {
        let sessions_dir = storage_root.join("sessions");
        let _ = fs::create_dir_all(&sessions_dir);
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            operation_cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            execution_results: Arc::new(RwLock::new(HashMap::new())),
            storage_root: Arc::new(storage_root),
        }
    }

    pub async fn get_session_snapshot(&self, session_id: &str) -> Option<TaskModeSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    pub async fn get_or_load_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<TaskModeSession>, String> {
        if let Some(snapshot) = self.get_session_snapshot(session_id).await {
            return Ok(Some(snapshot));
        }

        match self.read_persisted_session(session_id).await {
            Ok(Some(snapshot)) => {
                {
                    let mut sessions = self.sessions.write().await;
                    sessions.insert(session_id.to_string(), snapshot.clone());
                }
                Ok(Some(snapshot))
            }
            Ok(None) => Ok(None),
            Err(error) => {
                let _ = self.delete_persisted_session(session_id).await;
                Err(error)
            }
        }
    }

    pub async fn persist_session_snapshot(&self, session: &TaskModeSession) -> Result<(), String> {
        let record = TaskModeSessionRecordV1 {
            version: TASK_MODE_SESSION_RECORD_VERSION,
            session: session.clone(),
        };
        let encoded = serde_json::to_vec_pretty(&record)
            .map_err(|e| format!("Failed to encode task mode session: {e}"))?;
        fs::write(self.session_file_path(&session.session_id), encoded).map_err(|e| {
            format!(
                "Failed to persist task mode session '{}': {e}",
                session.session_id
            )
        })
    }

    pub async fn store_session_snapshot(&self, session: TaskModeSession) -> Result<(), String> {
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session.session_id.clone(), session.clone());
        }
        self.persist_session_snapshot(&session).await
    }

    pub async fn delete_persisted_session(&self, session_id: &str) -> Result<(), String> {
        let path = self.session_file_path(session_id);
        if !path.exists() {
            return Ok(());
        }
        fs::remove_file(path).map_err(|e| {
            format!("Failed to delete persisted task mode session '{session_id}': {e}")
        })
    }

    async fn read_persisted_session(
        &self,
        session_id: &str,
    ) -> Result<Option<TaskModeSession>, String> {
        let path = self.session_file_path(session_id);
        if !path.exists() {
            return Ok(None);
        }

        let raw = fs::read(&path).map_err(|e| {
            format!("Failed to read persisted task mode session '{session_id}': {e}")
        })?;
        let record: TaskModeSessionRecordV1 = serde_json::from_slice(&raw)
            .map_err(|e| format!("Persisted task mode session '{session_id}' is corrupted: {e}"))?;
        if record.version != TASK_MODE_SESSION_RECORD_VERSION {
            return Err(format!(
                "Unsupported task mode session record version {} for '{}'",
                record.version, session_id
            ));
        }
        Ok(Some(record.session))
    }

    fn session_file_path(&self, session_id: &str) -> PathBuf {
        self.storage_root
            .join("sessions")
            .join(format!("{session_id}.json"))
    }
}

fn resolve_task_mode_storage_root() -> PathBuf {
    if let Ok(root) = ensure_plan_cascade_dir() {
        let path = root.join("task-mode");
        let _ = fs::create_dir_all(&path);
        return path;
    }

    let fallback = std::env::temp_dir().join("plan-cascade-task-mode");
    let _ = fs::create_dir_all(&fallback);
    fallback
}

const TASK_OPERATION_CANCELLED_ERROR: &str = "Operation cancelled";

async fn register_task_operation_token(
    state: &TaskModeState,
    session_id: &str,
) -> (String, CancellationToken) {
    let operation_id = uuid::Uuid::new_v4().to_string();
    let token = CancellationToken::new();

    let previous = {
        let mut tokens = state.operation_cancellation_tokens.write().await;
        tokens.insert(
            session_id.to_string(),
            (operation_id.clone(), token.clone()),
        )
    };
    if let Some((_, prev_token)) = previous {
        prev_token.cancel();
    }

    (operation_id, token)
}

async fn clear_task_operation_token(state: &TaskModeState, session_id: &str, operation_id: &str) {
    let mut tokens = state.operation_cancellation_tokens.write().await;
    let should_remove = tokens
        .get(session_id)
        .map(|(current_id, _)| current_id == operation_id)
        .unwrap_or(false);
    if should_remove {
        tokens.remove(session_id);
    }
}

pub(crate) async fn persist_task_session_best_effort(
    state: &TaskModeState,
    session: &TaskModeSession,
    source: &str,
) {
    if let Err(error) = state.persist_session_snapshot(session).await {
        eprintln!(
            "[task_mode] failed to persist session '{}' at {}: {}",
            session.session_id, source, error
        );
    }
}

fn task_status_to_kernel_phase(status: &TaskModeStatus) -> &'static str {
    match status {
        TaskModeStatus::Initialized => "configuring",
        TaskModeStatus::Exploring => "exploring",
        TaskModeStatus::GeneratingPrd => "generating_prd",
        TaskModeStatus::ReviewingPrd => "reviewing_prd",
        TaskModeStatus::Executing => "executing",
        TaskModeStatus::Completed => "completed",
        TaskModeStatus::Failed => "failed",
        TaskModeStatus::Cancelled => "cancelled",
    }
}

fn task_status_to_kernel_status(status: &TaskModeStatus) -> Option<WorkflowStatus> {
    match status {
        TaskModeStatus::Completed => Some(WorkflowStatus::Completed),
        TaskModeStatus::Failed => Some(WorkflowStatus::Failed),
        TaskModeStatus::Cancelled => Some(WorkflowStatus::Cancelled),
        _ => None,
    }
}

fn infer_current_story_from_progress(progress: &BatchExecutionProgress) -> Option<String> {
    progress
        .story_statuses
        .iter()
        .find_map(|(story_id, status)| {
            if status == "running" || status == "executing" {
                Some(story_id.clone())
            } else {
                None
            }
        })
}

async fn emit_kernel_updates(
    app: &tauri::AppHandle,
    kernel_state: &WorkflowKernelState,
    kernel_session_ids: &[String],
    source: &str,
) {
    for kernel_session_id in kernel_session_ids {
        if let Ok(session_state) = kernel_state.get_session_state(kernel_session_id).await {
            let revision = (session_state.events.len() + session_state.checkpoints.len()) as u64;
            let payload = WorkflowKernelUpdatedEvent {
                session_state,
                revision,
                source: source.to_string(),
            };
            let _ = app.emit(WORKFLOW_KERNEL_UPDATED_CHANNEL, payload);
        }
    }
}

async fn sync_kernel_task_snapshot_and_emit(
    app: &tauri::AppHandle,
    kernel_state: &WorkflowKernelState,
    session: &TaskModeSession,
    phase_override: Option<&str>,
    source: &str,
) {
    let phase = phase_override
        .map(|value| value.to_string())
        .or_else(|| Some(task_status_to_kernel_phase(&session.status).to_string()));
    let (current_story_id, completed_stories, failed_stories) = match session.progress.as_ref() {
        Some(progress) => (
            infer_current_story_from_progress(progress),
            Some(progress.stories_completed as u64),
            Some(progress.stories_failed as u64),
        ),
        None => (None, Some(0), Some(0)),
    };

    let kernel_session_ids = kernel_state
        .sync_task_snapshot_by_linked_session(
            &session.session_id,
            phase,
            current_story_id,
            completed_stories,
            failed_stories,
            task_status_to_kernel_status(&session.status),
        )
        .await
        .unwrap_or_default();

    emit_kernel_updates(app, kernel_state, &kernel_session_ids, source).await;
}

async fn sync_kernel_task_phase_by_linked_session_and_emit(
    app: &tauri::AppHandle,
    kernel_state: &WorkflowKernelState,
    task_session_id: &str,
    phase: &str,
    source: &str,
) {
    let kernel_session_ids = kernel_state
        .sync_task_snapshot_by_linked_session(
            task_session_id,
            Some(phase.to_string()),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap_or_default();
    emit_kernel_updates(app, kernel_state, &kernel_session_ids, source).await;
}

fn injection_phase_to_context_phase(
    phase: crate::services::skills::model::InjectionPhase,
) -> &'static str {
    match phase {
        crate::services::skills::model::InjectionPhase::Planning => "planning",
        crate::services::skills::model::InjectionPhase::Implementation => "implementation",
        crate::services::skills::model::InjectionPhase::Retry => "implementation",
        crate::services::skills::model::InjectionPhase::Always => "analysis",
    }
}

pub(crate) async fn handoff_context_for_task_session(
    kernel_state: &WorkflowKernelState,
    task_session_id: &str,
) -> Option<HandoffContextBundle> {
    kernel_state
        .handoff_context_for_mode_session(WorkflowMode::Task, task_session_id)
        .await
}

pub(crate) fn conversation_history_from_task_handoff(
    handoff: &HandoffContextBundle,
) -> Vec<ConversationTurnInput> {
    handoff
        .conversation_context
        .iter()
        .map(|turn| ConversationTurnInput {
            user: turn.user.clone(),
            assistant: turn.assistant.clone(),
        })
        .collect()
}

pub(crate) fn render_task_handoff_context(handoff: &HandoffContextBundle) -> Option<String> {
    let mut sections = Vec::new();

    if !handoff.artifact_refs.is_empty() {
        sections.push(format!(
            "[artifact-refs]\n{}",
            handoff.artifact_refs.join("\n")
        ));
    }

    if !handoff.context_sources.is_empty() {
        sections.push(format!(
            "[context-sources]\n{}",
            handoff.context_sources.join("\n")
        ));
    }

    if !handoff.metadata.is_empty() {
        if let Ok(metadata) = serde_json::to_string_pretty(&handoff.metadata) {
            sections.push(format!("[handoff-metadata]\n{}", metadata));
        }
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}

pub(crate) async fn conversation_history_for_task_session(
    kernel_state: &WorkflowKernelState,
    task_session_id: &str,
) -> Vec<ConversationTurnInput> {
    handoff_context_for_task_session(kernel_state, task_session_id)
        .await
        .map(|handoff| conversation_history_from_task_handoff(&handoff))
        .unwrap_or_default()
}

async fn assemble_enriched_context_v2(
    app_state: &AppState,
    knowledge_state: &crate::commands::knowledge::KnowledgeState,
    project_path: &str,
    query: &str,
    phase: crate::services::skills::model::InjectionPhase,
    context_sources: Option<&crate::services::task_mode::context_provider::ContextSourceConfig>,
    mode: &str,
    session_id: Option<&str>,
    include_knowledge: bool,
) -> crate::services::task_mode::context_provider::EnrichedContext {
    let Some(mut config) = context_sources.cloned() else {
        return crate::services::task_mode::context_provider::EnrichedContext::default();
    };

    if !include_knowledge {
        if let Some(knowledge) = config.knowledge.as_mut() {
            knowledge.enabled = false;
        }
    }

    let request = crate::commands::context_v2::PrepareTurnContextV2Request {
        project_path: project_path.to_string(),
        query: query.to_string(),
        project_id: if config.project_id.trim().is_empty() {
            None
        } else {
            Some(config.project_id.clone())
        },
        session_id: session_id.map(|id| id.to_string()),
        mode: Some(mode.to_string()),
        turn_id: None,
        intent: None,
        phase: Some(injection_phase_to_context_phase(phase).to_string()),
        conversation_history: Vec::new(),
        context_sources: Some(config),
        rules: Vec::new(),
        manual_blocks: Vec::new(),
        input_token_budget: None,
        reserved_output_tokens: None,
        hard_limit: None,
        compaction_policy: None,
        fault_injection: None,
        enforce_user_skill_selection: true,
    };

    let assembly = match crate::commands::context_v2::assemble_turn_context_internal(
        request,
        app_state,
        knowledge_state,
    )
    .await
    {
        Ok(resp) => resp,
        Err(err) => {
            tracing::warn!(
                "[task_mode] Context V2 assembly failed, using empty context: {}",
                err
            );
            return crate::services::task_mode::context_provider::EnrichedContext::default();
        }
    };

    let slices = crate::commands::context_v2::split_assembly_into_slices(&assembly);
    let selected_skills =
        crate::services::task_mode::context_provider::hydrate_skill_matches_by_ids(
            app_state,
            project_path,
            &assembly.diagnostics.effective_skill_ids,
        )
        .await;

    let skill_expertise = if !selected_skills.is_empty() {
        selected_skills
            .iter()
            .map(|m| format!("{} best practices", m.skill.name))
            .collect::<Vec<_>>()
    } else {
        assembly
            .diagnostics
            .selected_skills
            .iter()
            .map(|name| format!("{} best practices", name))
            .collect::<Vec<_>>()
    };

    crate::services::task_mode::context_provider::EnrichedContext {
        knowledge_block: slices.knowledge_block,
        memory_block: slices.memory_block,
        skills_block: slices.skills_block,
        skill_expertise,
        selected_skills,
        blocked_tools: assembly.diagnostics.blocked_tools,
        skill_selection_reason: assembly.diagnostics.selection_reason,
    }
}

// ============================================================================
// Commands
// ============================================================================

pub mod analysis_commands;
pub mod execution_commands;
pub mod generation_commands;
pub mod session_lifecycle_commands;

pub use analysis_commands::{run_architecture_review, run_requirement_analysis};
pub use execution_commands::{
    approve_task_prd, cancel_task_execution, cancel_task_operation, get_task_execution_report,
    get_task_execution_status,
};
pub use generation_commands::{apply_task_prd_feedback, explore_project, generate_task_prd};
pub use session_lifecycle_commands::{enter_task_mode, exit_task_mode};

/// Resolve an LLM provider from frontend parameters and OS keyring.
///
/// Looks up the API key from the keyring if not provided explicitly.
/// Returns an Arc<dyn LlmProvider> ready for use.
pub(crate) async fn resolve_llm_provider(
    provider_name: &str,
    model: &str,
    explicit_api_key: Option<String>,
    explicit_base_url: Option<String>,
    app_state: &tauri::State<'_, AppState>,
) -> Result<Arc<dyn crate::services::llm::provider::LlmProvider>, String> {
    use crate::commands::standalone::normalize_provider_name;
    use crate::services::llm::types::{ProviderConfig, ProviderType};

    let canonical = normalize_provider_name(provider_name)
        .ok_or_else(|| format!("Unknown provider: {}", provider_name))?;

    let provider_type = match canonical {
        "anthropic" => ProviderType::Anthropic,
        "openai" => ProviderType::OpenAI,
        "deepseek" => ProviderType::DeepSeek,
        "glm" => ProviderType::Glm,
        "qwen" => ProviderType::Qwen,
        "minimax" => ProviderType::Minimax,
        "ollama" => ProviderType::Ollama,
        _ => return Err(format!("Unsupported provider: {}", canonical)),
    };

    // Resolve API key: explicit parameter > OS keyring
    let api_key = explicit_api_key
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .or_else(|| {
            let keyring = KeyringService::new();
            keyring.get_api_key(canonical).ok().flatten()
        });

    // Validate API key for non-Ollama providers
    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return Err(format!(
            "API key not configured for provider '{}'. \
             Please configure it in Settings or pass it explicitly.",
            canonical
        ));
    }

    // Resolve base URL: explicit > DB settings > default
    let mut resolved_base_url = explicit_base_url
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());

    if resolved_base_url.is_none() {
        let key = format!("provider_{}_base_url", canonical);
        if let Ok(Some(db_url)) = app_state.with_database(|db| db.get_setting(&key)).await {
            if !db_url.is_empty() {
                resolved_base_url = Some(db_url);
            }
        }
    }

    // Resolve proxy
    let proxy = {
        let keyring = KeyringService::new();
        app_state
            .with_database(|db| {
                Ok(crate::commands::proxy::resolve_provider_proxy(
                    &keyring, db, canonical,
                ))
            })
            .await
            .unwrap_or(None)
    };

    let config = ProviderConfig {
        provider: provider_type,
        api_key,
        base_url: resolved_base_url,
        model: model.to_string(),
        proxy,
        ..Default::default()
    };

    Ok(prd_generator::create_provider(config))
}

/// Resolve a ProviderConfig from frontend parameters and OS keyring.
///
/// Same as `resolve_llm_provider` but returns the raw config instead of
/// an instantiated provider. Used by `execute_story_via_llm()` to create
/// OrchestratorService instances.
pub(crate) async fn resolve_provider_config(
    provider_name: &str,
    model: &str,
    explicit_api_key: Option<String>,
    explicit_base_url: Option<String>,
    app_state: &tauri::State<'_, AppState>,
) -> Result<crate::services::llm::types::ProviderConfig, String> {
    use crate::commands::standalone::normalize_provider_name;
    use crate::services::llm::types::{ProviderConfig, ProviderType};

    let canonical = normalize_provider_name(provider_name)
        .ok_or_else(|| format!("Unknown provider: {}", provider_name))?;

    let provider_type = match canonical {
        "anthropic" => ProviderType::Anthropic,
        "openai" => ProviderType::OpenAI,
        "deepseek" => ProviderType::DeepSeek,
        "glm" => ProviderType::Glm,
        "qwen" => ProviderType::Qwen,
        "minimax" => ProviderType::Minimax,
        "ollama" => ProviderType::Ollama,
        _ => return Err(format!("Unsupported provider: {}", canonical)),
    };

    let api_key = explicit_api_key
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            let keyring = KeyringService::new();
            keyring.get_api_key(canonical).ok().flatten()
        });

    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return Err(format!(
            "API key not configured for provider '{}'.",
            canonical
        ));
    }

    // Resolve base URL: explicit param > DB settings > default
    let mut resolved_base_url = explicit_base_url
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());
    if resolved_base_url.is_none() {
        let key = format!("provider_{}_base_url", canonical);
        if let Ok(Some(db_url)) = app_state.with_database(|db| db.get_setting(&key)).await {
            if !db_url.is_empty() {
                resolved_base_url = Some(db_url);
            }
        }
    }

    let proxy = {
        let keyring = KeyringService::new();
        app_state
            .with_database(|db| {
                Ok(crate::commands::proxy::resolve_provider_proxy(
                    &keyring, db, canonical,
                ))
            })
            .await
            .unwrap_or(None)
    };

    Ok(ProviderConfig {
        provider: provider_type,
        api_key,
        base_url: resolved_base_url,
        model: model.to_string(),
        proxy,
        ..Default::default()
    })
}

pub(crate) fn resolve_search_provider_for_tools() -> (String, Option<String>) {
    use crate::commands::standalone::get_search_api_key_with_aliases;

    let provider = ConfigService::new()
        .map(|svc| svc.get_config_clone().search_provider)
        .map(|p| p.trim().to_ascii_lowercase())
        .unwrap_or_else(|_| "duckduckgo".to_string());
    let provider = if provider.is_empty() {
        "duckduckgo".to_string()
    } else {
        provider
    };

    let keyring = KeyringService::new();
    let api_key = get_search_api_key_with_aliases(&keyring, &provider).unwrap_or(None);
    (provider, api_key)
}

// ============================================================================
// Requirement Analysis & Architecture Review
// ============================================================================

fn normalize_locale(locale: Option<&str>) -> &'static str {
    let normalized = locale.unwrap_or("en").to_lowercase();
    if normalized.starts_with("zh") {
        "zh"
    } else if normalized.starts_with("ja") {
        "ja"
    } else {
        "en"
    }
}

fn locale_instruction(locale_tag: &str) -> &'static str {
    match locale_tag {
        "zh" => {
            "CRITICAL: Your final answer MUST be in Simplified Chinese. Keep code symbols, identifiers, and file paths unchanged."
        }
        "ja" => {
            "CRITICAL: Your final answer MUST be in Japanese. Keep code symbols, identifiers, and file paths unchanged."
        }
        _ => "CRITICAL: Your final answer MUST be in English. Keep code symbols, identifiers, and file paths unchanged.",
    }
}

fn value_to_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_payload_story(value: &serde_json::Value) -> Option<PrdModificationPayloadStory> {
    let title = value
        .get("title")
        .and_then(|v| v.as_str())?
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }

    Some(PrdModificationPayloadStory {
        id: value
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        title,
        description: value
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        priority: value
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("medium")
            .to_string(),
        dependencies: value_to_string_array(value.get("dependencies")),
        acceptance_criteria: value_to_string_array(
            value
                .get("acceptance_criteria")
                .or_else(|| value.get("acceptanceCriteria")),
        ),
    })
}

fn parse_prd_modification_payload(value: Option<&serde_json::Value>) -> PrdModificationPayload {
    let mut payload = PrdModificationPayload::default();
    let Some(obj) = value.and_then(|v| v.as_object()) else {
        return payload;
    };

    payload.title = obj
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    payload.description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    payload.priority = obj
        .get("priority")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if let Some(v) = obj.get("dependencies") {
        payload.dependencies = Some(value_to_string_array(Some(v)));
    }
    if let Some(v) = obj
        .get("acceptance_criteria")
        .or_else(|| obj.get("acceptanceCriteria"))
    {
        payload.acceptance_criteria = Some(value_to_string_array(Some(v)));
    }

    payload.story = obj.get("story").and_then(parse_payload_story);
    payload.stories = obj
        .get("stories")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_payload_story).collect())
        .unwrap_or_default();

    payload.dependency_remap = obj
        .get("dependency_remap")
        .or_else(|| obj.get("dependencyRemap"))
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .map(|(key, value)| (key.clone(), value_to_string_array(Some(value))))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    payload
}

fn normalize_modification_type(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "modify" | "update" | "update_story" => "update_story".to_string(),
        "add" | "add_story" => "add_story".to_string(),
        "remove" | "delete" | "remove_story" => "remove_story".to_string(),
        "split" | "split_story" => "split_story".to_string(),
        "merge" | "merge_story" => "merge_story".to_string(),
        _ => "update_story".to_string(),
    }
}

fn truncate_for_preview(input: &str, max_len: usize) -> String {
    let trimmed = input.trim();
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }
    let mut out = trimmed.chars().take(max_len).collect::<String>();
    out.push_str("...");
    out
}

fn build_modification_preview(
    modification_type: &str,
    target_story_id: Option<&str>,
    payload: &PrdModificationPayload,
    reason: &str,
) -> String {
    let target = target_story_id.unwrap_or("N/A");
    match modification_type {
        "add_story" => {
            let title = payload
                .story
                .as_ref()
                .map(|s| s.title.as_str())
                .or(payload.title.as_deref())
                .unwrap_or("New story");
            format!("Add story: {}", truncate_for_preview(title, 80))
        }
        "remove_story" => format!("Remove story {}", target),
        "split_story" => {
            if payload.stories.is_empty() {
                format!("Split story {}", target)
            } else {
                let names = payload
                    .stories
                    .iter()
                    .map(|s| s.title.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "Split {} into: {}",
                    target,
                    truncate_for_preview(&names, 100)
                )
            }
        }
        "merge_story" => {
            let title = payload.title.as_deref().unwrap_or("Merged story");
            format!("Merge into {}: {}", target, truncate_for_preview(title, 80))
        }
        _ => {
            let title = payload
                .story
                .as_ref()
                .map(|s| s.title.as_str())
                .or(payload.title.as_deref())
                .unwrap_or(reason);
            if target_story_id.is_some() {
                format!("Update {}: {}", target, truncate_for_preview(title, 100))
            } else {
                format!("Update story: {}", truncate_for_preview(title, 100))
            }
        }
    }
}

fn parse_single_prd_modification(
    value: &serde_json::Value,
    index: usize,
) -> Option<PrdModification> {
    let modification_type = normalize_modification_type(
        value
            .get("type")
            .or_else(|| value.get("modification_type"))
            .or_else(|| value.get("action"))
            .and_then(|v| v.as_str())
            .unwrap_or("update_story"),
    );
    let target_story_id = value
        .get("target_story_id")
        .or_else(|| value.get("targetStoryId"))
        .or_else(|| value.get("story_id"))
        .or_else(|| value.get("storyId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let reason = value
        .get("reason")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "No reason provided".to_string());

    let mut payload = parse_prd_modification_payload(value.get("payload"));

    // Backward compatibility for old formatter output where fields are top-level.
    if payload.story.is_none() {
        payload.story = value.get("story").and_then(parse_payload_story);
    }
    if payload.stories.is_empty() {
        payload.stories = value
            .get("stories")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(parse_payload_story).collect())
            .unwrap_or_default();
    }
    if payload.title.is_none() {
        payload.title = value
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }
    if payload.description.is_none() {
        payload.description = value
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }
    if payload.priority.is_none() {
        payload.priority = value
            .get("priority")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }
    if payload.dependencies.is_none() && value.get("dependencies").is_some() {
        payload.dependencies = Some(value_to_string_array(value.get("dependencies")));
    }
    if payload.acceptance_criteria.is_none()
        && (value.get("acceptance_criteria").is_some() || value.get("acceptanceCriteria").is_some())
    {
        payload.acceptance_criteria = Some(value_to_string_array(
            value
                .get("acceptance_criteria")
                .or_else(|| value.get("acceptanceCriteria")),
        ));
    }

    if modification_type == "add_story" && payload.story.is_none() {
        payload.story = Some(PrdModificationPayloadStory {
            id: None,
            title: payload
                .title
                .clone()
                .unwrap_or_else(|| "New Story".to_string()),
            description: payload
                .description
                .clone()
                .unwrap_or_else(|| reason.clone()),
            priority: payload
                .priority
                .clone()
                .unwrap_or_else(|| "medium".to_string()),
            dependencies: payload.dependencies.clone().unwrap_or_default(),
            acceptance_criteria: payload.acceptance_criteria.clone().unwrap_or_default(),
        });
    }

    let operation_id = value
        .get("operation_id")
        .or_else(|| value.get("operationId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("arch_mod_{:03}", index + 1));
    let preview = value
        .get("preview")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            build_modification_preview(
                &modification_type,
                target_story_id.as_deref(),
                &payload,
                &reason,
            )
        });
    let confidence = value
        .get("confidence")
        .and_then(|v| v.as_f64())
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(0.7);

    Some(PrdModification {
        operation_id,
        modification_type,
        target_story_id,
        payload,
        preview,
        reason,
        confidence,
    })
}

fn parse_prd_modifications(structured: &serde_json::Value) -> Vec<PrdModification> {
    structured
        .get("prd_modifications")
        .or_else(|| structured.get("modifications"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .enumerate()
                .filter_map(|(index, value)| parse_single_prd_modification(value, index))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

// ============================================================================
// Design Doc Preparation
// ============================================================================

/// Prepare a design document for a Task Mode PRD.
///
/// Serializes the in-memory TaskPrd to `prd.json` in the project directory,
/// then runs the deterministic DesignDocGenerator to produce `design_doc.json`.
/// This enables `batch_executor` to inject `StoryContext` during execution.
///
/// # Arguments
/// * `prd` - The task PRD (in-memory, from the workflow)
/// * `project_path` - Optional project root. Falls back to cwd.
#[tauri::command]
pub async fn prepare_design_doc_for_task(
    session_id: Option<String>,
    prd: TaskPrd,
    project_path: Option<String>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<GenerateResult>, String> {
    use crate::services::design::DesignDocGenerator;

    if let Some(task_session_id) = session_id.as_ref().map(String::as_str) {
        sync_kernel_task_phase_by_linked_session_and_emit(
            &app_handle,
            kernel_state.inner(),
            task_session_id,
            "generating_design_doc",
            "task_mode.prepare_design_doc.started",
        )
        .await;
    }

    // 1. Resolve project path
    let base = match project_path {
        Some(ref p) if !p.trim().is_empty() => std::path::PathBuf::from(p),
        _ => std::path::PathBuf::from("."),
    };

    // 2. Serialize PRD to JSON and save as {base}/prd.json
    let prd_path = base.join("prd.json");
    let json = match serde_json::to_string_pretty(&prd) {
        Ok(j) => j,
        Err(e) => {
            if let Some(task_session_id) = session_id.as_ref().map(String::as_str) {
                sync_kernel_task_phase_by_linked_session_and_emit(
                    &app_handle,
                    kernel_state.inner(),
                    task_session_id,
                    "failed",
                    "task_mode.prepare_design_doc.failed_serialize",
                )
                .await;
            }
            return Ok(CommandResponse::err(format!(
                "Failed to serialize PRD: {}",
                e
            )));
        }
    };
    if let Err(e) = std::fs::write(&prd_path, &json) {
        if let Some(task_session_id) = session_id.as_ref().map(String::as_str) {
            sync_kernel_task_phase_by_linked_session_and_emit(
                &app_handle,
                kernel_state.inner(),
                task_session_id,
                "failed",
                "task_mode.prepare_design_doc.failed_write",
            )
            .await;
        }
        return Ok(CommandResponse::err(format!("Failed to write PRD: {}", e)));
    }

    // 3. Call existing DesignDocGenerator
    match DesignDocGenerator::generate_from_file(&prd_path, None, true) {
        Ok(result) => {
            if let Some(task_session_id) = session_id.as_ref().map(String::as_str) {
                sync_kernel_task_phase_by_linked_session_and_emit(
                    &app_handle,
                    kernel_state.inner(),
                    task_session_id,
                    "reviewing_prd",
                    "task_mode.prepare_design_doc.completed",
                )
                .await;
            }
            Ok(CommandResponse::ok(result))
        }
        Err(e) => {
            if let Some(task_session_id) = session_id.as_ref().map(String::as_str) {
                sync_kernel_task_phase_by_linked_session_and_emit(
                    &app_handle,
                    kernel_state.inner(),
                    task_session_id,
                    "failed",
                    "task_mode.prepare_design_doc.failed_generate",
                )
                .await;
            }
            Ok(CommandResponse::err(e.to_string()))
        }
    }
}

// ============================================================================
// Story Executor
// ============================================================================

/// Load story-specific context from the project's design_doc.json.
///
/// Reads `design_doc.json` from the project root and extracts the story
/// mapping for the given `story_id` from the `story_mappings` section.
/// Returns `None` gracefully when the file does not exist, cannot be parsed,
/// or the story ID is not present in the mappings.
fn load_story_context(project_path: &std::path::Path, story_id: &str) -> Option<StoryContext> {
    let design_doc_path = project_path.join("design_doc.json");
    let content = std::fs::read_to_string(&design_doc_path).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&content).ok()?;
    let mappings = doc.get("story_mappings")?.as_object()?;
    let mapping = mappings.get(story_id)?;

    Some(StoryContext {
        relevant_files: mapping
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        components: mapping
            .get("components")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        design_decisions: mapping
            .get("decisions")
            .or_else(|| mapping.get("design_decisions"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        additional_context: mapping
            .get("additional_context")
            .and_then(|v| v.as_str())
            .map(String::from),
    })
}

/// Build a story executor callback that runs each story through CLI or LLM.
///
/// In CLI mode, spawns external CLI tools (claude, codex, aider).
/// In LLM mode, uses OrchestratorService for direct LLM API execution.
fn build_story_executor(
    app_handle: tauri::AppHandle,
    mode: StoryExecutionMode,
    provider_config: Option<crate::services::llm::types::ProviderConfig>,
    db_pool: Option<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>,
    knowledge_block: String,
    memory_block: String,
    skills_block: String,
    selected_skill_matches: Vec<crate::services::skills::model::SkillMatch>,
    knowledge_tool_params: Option<KnowledgeToolParams>,
) -> impl Fn(StoryExecutionContext) -> Pin<Box<dyn Future<Output = StoryExecutionOutcome> + Send>>
       + Send
       + Sync
       + Clone
       + 'static {
    move |ctx: StoryExecutionContext| -> Pin<Box<dyn Future<Output = StoryExecutionOutcome> + Send>> {
        let app = app_handle.clone();
        let mode = mode.clone();
        let provider_config = provider_config.clone();
        let db_pool = db_pool.clone();
        let knowledge_block = knowledge_block.clone();
        let memory_block = memory_block.clone();
        let skills_block = skills_block.clone();
        let selected_skill_matches = selected_skill_matches.clone();
        let knowledge_tool_params = knowledge_tool_params.clone();
        Box::pin(async move {
            eprintln!(
                "[INFO] Executing story '{}' (attempt {}) with agent '{}' in {} [mode: {:?}]",
                ctx.story_id,
                ctx.attempt,
                ctx.agent_name,
                ctx.project_path.display(),
                mode,
            );

            // Emit story execution event for frontend tracking
            {
                use tauri::Emitter;
                let _ = app.emit(
                    TASK_MODE_EVENT_CHANNEL,
                    &TaskModeProgressEvent {
                        session_id: String::new(),
                        event_type: "story_executing".to_string(),
                        current_batch: 0,
                        total_batches: 0,
                        story_id: Some(ctx.story_id.clone()),
                        story_status: Some("executing".to_string()),
                        agent_name: Some(ctx.agent_name.clone()),
                        gate_results: None,
                        error: None,
                        progress_pct: 0.0,
                    },
                );
            }

            // Load story-specific context from design_doc.json if available
            let mut ctx = ctx;
            if ctx.story_context.is_none() {
                ctx.story_context = load_story_context(&ctx.project_path, &ctx.story_id);
            }

            // Build execution prompt from story context
            let prompt = build_story_prompt(&ctx, &knowledge_block, &memory_block, &skills_block);

            match mode {
                StoryExecutionMode::Cli => {
                    // Spawn agent CLI process to execute the story
                    execute_story_via_agent(
                        &ctx.agent_name,
                        &prompt,
                        &ctx.project_path,
                        ctx.cancel_token.clone(),
                    )
                    .await
                }
                StoryExecutionMode::Llm => {
                    // Execute via OrchestratorService using direct LLM API
                    execute_story_via_llm(
                        provider_config.as_ref(),
                        &prompt,
                        &ctx.project_path,
                        db_pool.as_ref(),
                        &knowledge_block,
                        &memory_block,
                        &skills_block,
                        selected_skill_matches.as_slice(),
                        knowledge_tool_params.as_ref(),
                        ctx.cancel_token.clone(),
                    )
                    .await
                }
            }
        })
    }
}

/// Execute a story by spawning an agent CLI process in the project directory.
///
/// Supports multiple agent backends: `claude`, `codex`, `aider`, etc.
/// The prompt is passed via the `-p` flag. The process exit code determines
/// success or failure.
async fn execute_story_via_agent(
    agent_name: &str,
    prompt: &str,
    project_path: &std::path::Path,
    cancel_token: tokio_util::sync::CancellationToken,
) -> StoryExecutionOutcome {
    use tokio::process::Command;

    // Resolve agent CLI command and arguments
    let (command, args) = match agent_name {
        name if name.starts_with("claude") => (
            "claude".to_string(),
            vec![
                "-p".to_string(),
                prompt.to_string(),
                "--output-format".to_string(),
                "text".to_string(),
            ],
        ),
        "codex" => (
            "codex".to_string(),
            vec!["--prompt".to_string(), prompt.to_string()],
        ),
        "aider" => (
            "aider".to_string(),
            vec![
                "--message".to_string(),
                prompt.to_string(),
                "--yes".to_string(),
            ],
        ),
        other => (
            other.to_string(),
            vec!["-p".to_string(), prompt.to_string()],
        ),
    };

    eprintln!(
        "[INFO] Spawning agent '{}' in {}",
        command,
        project_path.display()
    );

    let mut process_builder = Command::new(&command);
    process_builder
        .args(&args)
        .current_dir(project_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let child = match process_builder.spawn() {
        Ok(child) => child,
        Err(e) => {
            let error_msg = format!("Failed to spawn agent '{}': {}", command, e);
            eprintln!("[WARN] {}", error_msg);
            return StoryExecutionOutcome {
                success: false,
                error: Some(error_msg),
            };
        }
    };

    let mut wait_handle = tokio::spawn(async move { child.wait_with_output().await });

    tokio::select! {
        _ = cancel_token.cancelled() => {
            wait_handle.abort();
            eprintln!("[INFO] Agent '{}' execution cancelled", command);
            StoryExecutionOutcome {
                success: false,
                error: Some("Story execution cancelled".to_string()),
            }
        }
        join_result = &mut wait_handle => {
            match join_result {
                Ok(Ok(output)) if output.status.success() => {
                    eprintln!("[INFO] Agent '{}' completed successfully", command);
                    StoryExecutionOutcome {
                        success: true,
                        error: None,
                    }
                }
                Ok(Ok(output)) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let error_msg = format!(
                        "Agent '{}' exited with code {}: {}",
                        command,
                        output.status.code().unwrap_or(-1),
                        stderr.chars().take(500).collect::<String>()
                    );
                    eprintln!("[WARN] {}", error_msg);
                    StoryExecutionOutcome {
                        success: false,
                        error: Some(error_msg),
                    }
                }
                Ok(Err(e)) => {
                    let error_msg = format!("Agent '{}' failed while waiting for output: {}", command, e);
                    eprintln!("[WARN] {}", error_msg);
                    StoryExecutionOutcome {
                        success: false,
                        error: Some(error_msg),
                    }
                }
                Err(e) => {
                    let error_msg = format!("Agent '{}' task join failed: {}", command, e);
                    eprintln!("[WARN] {}", error_msg);
                    StoryExecutionOutcome {
                        success: false,
                        error: Some(error_msg),
                    }
                }
            }
        }
    }
}

/// Execute a story via the OrchestratorService using direct LLM API.
///
/// Creates a fresh OrchestratorService instance for each story execution,
/// runs the full agentic loop (tool use, code generation, etc.), and
/// maps the result to StoryExecutionOutcome.
async fn execute_story_via_llm(
    provider_config: Option<&crate::services::llm::types::ProviderConfig>,
    prompt: &str,
    project_path: &std::path::Path,
    db_pool: Option<&r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>,
    knowledge_block: &str,
    memory_block: &str,
    skills_block: &str,
    selected_skill_matches: &[crate::services::skills::model::SkillMatch],
    knowledge_tool_params: Option<&KnowledgeToolParams>,
    cancel_token: tokio_util::sync::CancellationToken,
) -> StoryExecutionOutcome {
    use crate::services::orchestrator::{OrchestratorConfig, OrchestratorService};
    use crate::services::streaming::UnifiedStreamEvent;

    let provider_config = match provider_config {
        Some(cfg) => cfg.clone(),
        None => {
            return StoryExecutionOutcome {
                success: false,
                error: Some("No LLM provider config available for story execution".to_string()),
            };
        }
    };

    let analysis_artifacts_root = dirs::home_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join(".plan-cascade")
        .join("analysis-runs");

    let mut system_prompt = String::from(
        "You are an expert software engineer executing a story task. \
         Use the provided tools to implement the required changes. \
         Read relevant files, make code changes, and run tests to verify.",
    );
    if !skills_block.is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(skills_block);
    }
    // Only pre-inject knowledge_block when SearchKnowledge tool is NOT wired
    // (i.e. CLI mode fallback path). When tool is wired, AI searches on demand.
    if knowledge_tool_params.is_none() && !knowledge_block.is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(knowledge_block);
    }
    if !memory_block.is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(memory_block);
    }

    let config = OrchestratorConfig {
        provider: provider_config,
        system_prompt: Some(system_prompt),
        max_iterations: 50,
        max_total_tokens: 1_000_000,
        project_root: project_path.to_path_buf(),
        analysis_artifacts_root,
        streaming: true,
        enable_compaction: true,
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: None,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let (search_provider, search_api_key) = resolve_search_provider_for_tools();
    let mut orchestrator =
        OrchestratorService::new(config).with_search_provider(&search_provider, search_api_key);

    if !selected_skill_matches.is_empty() {
        let selected_skills =
            std::sync::Arc::new(tokio::sync::RwLock::new(selected_skill_matches.to_vec()));
        orchestrator = orchestrator.with_selected_skills(selected_skills);
    }

    // Wire database pool for CodebaseSearch if available
    if let Some(pool) = db_pool {
        orchestrator = orchestrator.with_database(pool.clone());
    }

    // Wire SearchKnowledge tool for on-demand knowledge base access
    if let Some(params) = knowledge_tool_params {
        orchestrator = orchestrator.with_knowledge_tool(
            Arc::clone(&params.pipeline),
            params.project_id.clone(),
            params.collection_filter.clone(),
            params.document_filter.clone(),
            params.awareness_section.clone(),
        );
    }

    // Create channel for event collection (events are discarded for story execution)
    let (tx, mut rx) = tokio::sync::mpsc::channel::<UnifiedStreamEvent>(256);

    // Drain events in background to prevent channel backpressure
    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            // Events are discarded — story execution doesn't stream to frontend
        }
    });

    eprintln!(
        "[INFO] Executing story via LLM in {}",
        project_path.display()
    );

    // Run the full agentic loop with cancellation support.
    let execute_future = orchestrator.execute(prompt.to_string(), tx);
    tokio::pin!(execute_future);

    let result = tokio::select! {
        _ = cancel_token.cancelled() => {
            orchestrator.cancel();
            return StoryExecutionOutcome {
                success: false,
                error: Some("Story execution cancelled".to_string()),
            };
        }
        result = &mut execute_future => result,
    };

    if cancel_token.is_cancelled() {
        orchestrator.cancel();
        return StoryExecutionOutcome {
            success: false,
            error: Some("Story execution cancelled".to_string()),
        };
    }

    if result.success {
        eprintln!("[INFO] LLM story execution completed successfully");
        StoryExecutionOutcome {
            success: true,
            error: None,
        }
    } else {
        let error_msg = result
            .error
            .unwrap_or_else(|| "LLM execution failed with no error details".to_string());
        eprintln!("[WARN] LLM story execution failed: {}", error_msg);
        StoryExecutionOutcome {
            success: false,
            error: Some(error_msg),
        }
    }
}

/// Build an execution prompt from story context for the LLM agent.
fn build_story_prompt(
    ctx: &StoryExecutionContext,
    knowledge_block: &str,
    memory_block: &str,
    skills_block: &str,
) -> String {
    let criteria = ctx
        .acceptance_criteria
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}. {}", i + 1, c))
        .collect::<Vec<_>>()
        .join("\n");

    let mut prompt = format!(
        "Execute the following task:\n\n\
         ## {} ({})\n\n\
         {}\n\n\
         ## Acceptance Criteria\n\
         {}\n\n\
         ## Instructions\n\
         - Implement all acceptance criteria\n\
         - Run relevant tests to verify correctness\n\
         - Ensure code compiles without errors",
        ctx.story_title, ctx.story_id, ctx.story_description, criteria,
    );

    if let Some(ref story_ctx) = ctx.story_context {
        prompt.push_str("\n\n## Relevant Context\n");
        if !story_ctx.relevant_files.is_empty() {
            prompt.push_str("\n### Relevant Files\n");
            for f in &story_ctx.relevant_files {
                prompt.push_str(&format!("- {}\n", f));
            }
        }
        if !story_ctx.components.is_empty() {
            prompt.push_str("\n### Components\n");
            for c in &story_ctx.components {
                prompt.push_str(&format!("- {}\n", c));
            }
        }
        if !story_ctx.design_decisions.is_empty() {
            prompt.push_str("\n### Design Decisions\n");
            for d in &story_ctx.design_decisions {
                prompt.push_str(&format!("- {}\n", d));
            }
        }
        if let Some(ref extra) = story_ctx.additional_context {
            prompt.push_str(&format!("\n### Additional Context\n{}\n", extra));
        }
    }

    if !knowledge_block.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(knowledge_block);
    }
    if !memory_block.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(memory_block);
    }
    if !skills_block.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(skills_block);
    }

    if let Some(ref retry) = ctx.retry_context {
        prompt.push_str(&format!(
            "\n\n## Retry Context (Attempt {})\n\
             Previous attempt failed: {}\n\
             Previous agent: {}\n\
             Please analyze the failure and try a different approach.",
            retry.attempt, retry.failure_reason, retry.previous_agent,
        ));
    }

    prompt
}

// ============================================================================
// Phase Config Conversion
// ============================================================================

/// Build an `AgentsConfig` from frontend `PhaseConfigInput` settings.
///
/// Converts the flat frontend phase configs into the `AgentsConfig` structure
/// expected by `AgentResolver`. CLI agent names (e.g., "claude-code") are
/// registered as agents. LLM provider refs (e.g., "llm:anthropic:model") are
/// also registered so the resolver considers them "available".
fn build_agents_config_from_frontend(
    configs: &HashMap<String, PhaseConfigInput>,
    global_default_agent: Option<&str>,
) -> crate::services::task_mode::agent_resolver::AgentsConfig {
    use crate::services::task_mode::agent_resolver::{
        AgentDefinition, AgentOverrides, AgentsConfig, PhaseConfig,
    };

    let mut agents: HashMap<String, AgentDefinition> = HashMap::new();
    let mut phase_defaults: HashMap<String, PhaseConfig> = HashMap::new();
    let normalized_global_default = global_default_agent
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string);

    // Helper: register an agent name (CLI or LLM) so it's marked available
    let mut register_agent = |name: &str| {
        if name.trim().is_empty() {
            return;
        }
        if !agents.contains_key(name) {
            agents.insert(
                name.to_string(),
                AgentDefinition {
                    name: name.to_string(),
                    description: String::new(),
                    available: true,
                    suitable_phases: vec![],
                },
            );
        }
    };

    if let Some(ref global_agent) = normalized_global_default {
        register_agent(global_agent);
    }

    for (phase_id, input) in configs {
        let normalized_default = input.default_agent.trim();
        if !normalized_default.is_empty() {
            register_agent(normalized_default);
        }

        let mut fallback_chain: Vec<String> = Vec::new();
        for fallback in &input.fallback_chain {
            let normalized = fallback.trim();
            if normalized.is_empty() {
                continue;
            }
            register_agent(normalized);
            fallback_chain.push(normalized.to_string());
        }

        let phase_default = if normalized_default.is_empty() {
            normalized_global_default.clone()
        } else {
            Some(normalized_default.to_string())
        };

        phase_defaults.insert(
            phase_id.clone(),
            PhaseConfig {
                default_agent: phase_default,
                fallback_chain,
                story_type_overrides: HashMap::new(),
            },
        );
    }

    let default_agent = normalized_global_default.unwrap_or_else(|| "claude-code".to_string());
    register_agent(&default_agent);

    AgentsConfig {
        default_agent,
        agents,
        phase_defaults,
        overrides: AgentOverrides::default(),
    }
}

// ============================================================================
// Quality Score Helpers
// ============================================================================

/// Compute a quality dimension score from gate results.
///
/// Maps quality dimensions to relevant gates where possible:
/// - "correctness" -> overall pipeline pass/fail
/// - "readability" -> "code_review" gate result
/// - "maintainability" -> "code_review" gate result
/// - "test_coverage" -> "ai_verify" gate result
/// - "security" -> "ai_verify" gate result
///
/// When a specific gate is found, the score is 100 if passed, 0 if failed.
/// When no relevant gate is found, falls back to 80 if the overall pipeline
/// passed, or 20 if it failed (partial credit for completing execution).
fn compute_quality_dimension_score(
    dimension: &str,
    gate_results: &[&crate::services::quality_gates::pipeline::PipelineGateResult],
    overall_passed: bool,
) -> f64 {
    let relevant_gate_id = match dimension {
        "correctness" => None, // based on overall pass/fail
        "readability" | "maintainability" => Some("code_review"),
        "test_coverage" | "security" => Some("ai_verify"),
        _ => None,
    };

    if let Some(gate_id) = relevant_gate_id {
        if let Some(gate) = gate_results.iter().find(|g| g.gate_id == gate_id) {
            return if gate.passed { 100.0 } else { 0.0 };
        }
    }

    // Fallback: derive from overall pipeline result
    if overall_passed {
        80.0
    } else {
        20.0
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
            exploration_result: None,
            progress: None,
            execution_resume_payload: None,
            created_at: "2026-02-18T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"strategyAnalysis\""));
        assert!(json.contains("\"createdAt\""));
    }

    fn sample_task_mode_session(session_id: &str) -> TaskModeSession {
        TaskModeSession {
            session_id: session_id.to_string(),
            description: "sample".to_string(),
            status: TaskModeStatus::ReviewingPrd,
            strategy_analysis: None,
            prd: None,
            exploration_result: None,
            progress: None,
            execution_resume_payload: None,
            created_at: "2026-03-05T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn task_mode_session_persistence_roundtrip() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let state = TaskModeState::new_with_storage_dir(temp_dir.path().to_path_buf());
        let snapshot = sample_task_mode_session("task-session-roundtrip");

        state
            .store_session_snapshot(snapshot.clone())
            .await
            .expect("persist snapshot");

        let restored = state
            .get_or_load_session_snapshot("task-session-roundtrip")
            .await
            .expect("load snapshot")
            .expect("snapshot should exist");
        assert_eq!(restored.session_id, snapshot.session_id);
        assert_eq!(restored.status, snapshot.status);
    }

    #[tokio::test]
    async fn task_mode_session_corruption_is_reported_and_cleaned_up() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let state = TaskModeState::new_with_storage_dir(temp_dir.path().to_path_buf());
        let session_id = "task-session-corrupted";
        let path = state.session_file_path(session_id);
        std::fs::write(&path, b"{not json").expect("write corrupted file");

        let result = state.get_or_load_session_snapshot(session_id).await;
        assert!(result.is_err(), "corrupted record should return error");
        assert!(
            !path.exists(),
            "corrupted persisted record should be removed after failed decode"
        );
    }

    #[tokio::test]
    async fn task_mode_exit_cleanup_deletes_persisted_file() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let state = TaskModeState::new_with_storage_dir(temp_dir.path().to_path_buf());
        let session_id = "task-session-cleanup";
        let snapshot = sample_task_mode_session(session_id);
        state
            .store_session_snapshot(snapshot)
            .await
            .expect("persist snapshot");

        state
            .delete_persisted_session(session_id)
            .await
            .expect("delete snapshot");
        assert!(
            !state.session_file_path(session_id).exists(),
            "persisted task session should be removed"
        );
    }

    #[test]
    fn render_task_handoff_context_includes_non_conversation_sections() {
        let handoff = HandoffContextBundle {
            conversation_context: Vec::new(),
            artifact_refs: vec!["spec.md".to_string()],
            context_sources: vec!["chat_transcript_sync".to_string()],
            metadata: serde_json::Map::from_iter([(
                "workspacePath".to_string(),
                Value::String("/tmp/demo".to_string()),
            )]),
        };

        let rendered = render_task_handoff_context(&handoff).expect("rendered handoff context");
        assert!(rendered.contains("[artifact-refs]"));
        assert!(rendered.contains("spec.md"));
        assert!(rendered.contains("[context-sources]"));
        assert!(rendered.contains("chat_transcript_sync"));
        assert!(rendered.contains("[handoff-metadata]"));
        assert!(rendered.contains("workspacePath"));
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
            quality_scores: vec![QualityDimensionScore {
                story_id: "s1".to_string(),
                dimension: "correctness".to_string(),
                score: 80.0,
                max_score: 100.0,
            }],
            timeline: vec![TimelineEntry {
                story_id: "s1".to_string(),
                story_title: "Story 1".to_string(),
                batch_index: 0,
                agent: "claude-sonnet".to_string(),
                duration_ms: 5000,
                start_offset_ms: 0,
                status: "completed".to_string(),
                gate_result: Some("passed".to_string()),
            }],
            agent_performance: vec![AgentPerformanceEntry {
                agent_name: "claude-sonnet".to_string(),
                stories_assigned: 3,
                stories_completed: 2,
                success_rate: 0.666,
                average_duration_ms: 4500,
                average_quality_score: Some(85.0),
            }],
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"storiesCompleted\""));
        assert!(json.contains("\"totalDurationMs\""));
        assert!(json.contains("\"agentAssignments\""));
        assert!(json.contains("\"qualityScores\""));
        assert!(json.contains("\"timeline\""));
        assert!(json.contains("\"agentPerformance\""));
    }

    #[test]
    fn generate_task_prd_request_accepts_camel_case_only() {
        let ok = serde_json::from_value::<GenerateTaskPrdRequest>(serde_json::json!({
            "sessionId": "session-1",
            "provider": "anthropic",
            "model": "claude-sonnet-4-20250514",
            "apiKey": "k",
            "baseUrl": "https://example.com",
            "projectPath": "/tmp/project"
        }))
        .unwrap();
        assert_eq!(ok.session_id, "session-1");
        assert_eq!(ok.api_key.as_deref(), Some("k"));
        assert_eq!(ok.project_path.as_deref(), Some("/tmp/project"));

        let legacy_err = serde_json::from_value::<GenerateTaskPrdRequest>(serde_json::json!({
            "sessionId": "session-1",
            "api_key": "k",
            "project_path": "/tmp/project"
        }));
        assert!(legacy_err.is_err());
    }

    #[test]
    fn approve_task_prd_request_rejects_unknown_legacy_fields() {
        let ok = serde_json::from_value::<ApproveTaskPrdRequest>(serde_json::json!({
            "sessionId": "session-1",
            "prd": {
                "title": "Test",
                "description": "Desc",
                "stories": [
                    {
                        "id": "S-1",
                        "title": "Story",
                        "description": "Story desc",
                        "priority": "high",
                        "dependencies": [],
                        "acceptanceCriteria": []
                    }
                ],
                "batches": []
            },
            "projectPath": "/tmp/project"
        }))
        .unwrap();
        assert_eq!(ok.session_id, "session-1");
        assert_eq!(ok.project_path.as_deref(), Some("/tmp/project"));

        let legacy_err = serde_json::from_value::<ApproveTaskPrdRequest>(serde_json::json!({
            "sessionId": "session-1",
            "prd": {
                "title": "Test",
                "description": "Desc",
                "stories": [],
                "batches": []
            },
            "project_path": "/tmp/project"
        }));
        assert!(legacy_err.is_err());
    }

    #[test]
    fn run_requirement_analysis_request_accepts_camel_case_only() {
        let ok = serde_json::from_value::<RunRequirementAnalysisRequest>(serde_json::json!({
            "sessionId": "session-1",
            "taskDescription": "Build feature X",
            "apiKey": "k",
            "baseUrl": "https://example.com",
            "projectPath": "/tmp/project"
        }))
        .unwrap();
        assert_eq!(ok.session_id, "session-1");
        assert_eq!(ok.task_description, "Build feature X");
        assert_eq!(ok.api_key.as_deref(), Some("k"));

        let legacy_err =
            serde_json::from_value::<RunRequirementAnalysisRequest>(serde_json::json!({
                "sessionId": "session-1",
                "taskDescription": "Build feature X",
                "api_key": "k",
                "project_path": "/tmp/project"
            }));
        assert!(legacy_err.is_err());
    }

    #[test]
    fn run_architecture_review_request_accepts_camel_case_only() {
        let ok = serde_json::from_value::<RunArchitectureReviewRequest>(serde_json::json!({
            "sessionId": "session-1",
            "prdJson": "{}",
            "apiKey": "k",
            "baseUrl": "https://example.com",
            "projectPath": "/tmp/project"
        }))
        .unwrap();
        assert_eq!(ok.session_id, "session-1");
        assert_eq!(ok.prd_json, "{}");
        assert_eq!(ok.project_path.as_deref(), Some("/tmp/project"));

        let legacy_err =
            serde_json::from_value::<RunArchitectureReviewRequest>(serde_json::json!({
                "sessionId": "session-1",
                "prdJson": "{}",
                "api_key": "k",
                "project_path": "/tmp/project"
            }));
        assert!(legacy_err.is_err());
    }

    #[test]
    fn test_build_agents_config_from_frontend_uses_global_default_for_empty_phase_defaults() {
        let mut configs = HashMap::new();
        configs.insert(
            "planning".to_string(),
            PhaseConfigInput {
                default_agent: "".to_string(),
                fallback_chain: vec!["codex".to_string()],
            },
        );
        configs.insert(
            "implementation".to_string(),
            PhaseConfigInput {
                default_agent: "  ".to_string(),
                fallback_chain: vec!["aider".to_string()],
            },
        );

        let parsed = build_agents_config_from_frontend(&configs, Some("claude-code"));
        let planning = parsed
            .phase_defaults
            .get("planning")
            .expect("planning config");
        let implementation = parsed
            .phase_defaults
            .get("implementation")
            .expect("implementation config");

        assert_eq!(parsed.default_agent, "claude-code");
        assert_eq!(planning.default_agent.as_deref(), Some("claude-code"));
        assert_eq!(implementation.default_agent.as_deref(), Some("claude-code"));
        assert_eq!(planning.fallback_chain, vec!["codex"]);
        assert_eq!(implementation.fallback_chain, vec!["aider"]);
        assert!(parsed.agents.contains_key("claude-code"));
        assert!(!parsed.agents.contains_key(""));
    }

    #[test]
    fn test_build_agents_config_from_frontend_falls_back_when_global_default_missing() {
        let mut configs = HashMap::new();
        configs.insert(
            "planning".to_string(),
            PhaseConfigInput {
                default_agent: "".to_string(),
                fallback_chain: vec![],
            },
        );

        let parsed = build_agents_config_from_frontend(&configs, None);
        let planning = parsed
            .phase_defaults
            .get("planning")
            .expect("planning config");

        assert_eq!(parsed.default_agent, "claude-code");
        assert_eq!(planning.default_agent, None);
        assert!(parsed.agents.contains_key("claude-code"));
    }

    #[test]
    fn test_quality_dimension_score_serialization() {
        let score = QualityDimensionScore {
            story_id: "story-001".to_string(),
            dimension: "correctness".to_string(),
            score: 95.0,
            max_score: 100.0,
        };
        let json = serde_json::to_string(&score).unwrap();
        assert!(json.contains("\"storyId\""));
        assert!(json.contains("\"dimension\""));
        assert!(json.contains("\"score\""));
        assert!(json.contains("\"maxScore\""));
        // Verify exact camelCase field names
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["storyId"], "story-001");
        assert_eq!(parsed["maxScore"], 100.0);
    }

    #[test]
    fn test_timeline_entry_serialization() {
        let entry = TimelineEntry {
            story_id: "story-002".to_string(),
            story_title: "Implement Auth".to_string(),
            batch_index: 1,
            agent: "claude-sonnet".to_string(),
            duration_ms: 8500,
            start_offset_ms: 3000,
            status: "completed".to_string(),
            gate_result: Some("passed".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"storyId\""));
        assert!(json.contains("\"storyTitle\""));
        assert!(json.contains("\"batchIndex\""));
        assert!(json.contains("\"durationMs\""));
        assert!(json.contains("\"startOffsetMs\""));
        assert!(json.contains("\"gateResult\""));
        // Verify exact camelCase field names
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["storyId"], "story-002");
        assert_eq!(parsed["batchIndex"], 1);
        assert_eq!(parsed["durationMs"], 8500);
        assert_eq!(parsed["startOffsetMs"], 3000);
    }

    #[test]
    fn test_agent_performance_entry_serialization() {
        let entry = AgentPerformanceEntry {
            agent_name: "claude-sonnet".to_string(),
            stories_assigned: 5,
            stories_completed: 4,
            success_rate: 0.8,
            average_duration_ms: 6000,
            average_quality_score: Some(87.5),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"agentName\""));
        assert!(json.contains("\"storiesAssigned\""));
        assert!(json.contains("\"storiesCompleted\""));
        assert!(json.contains("\"successRate\""));
        assert!(json.contains("\"averageDurationMs\""));
        assert!(json.contains("\"averageQualityScore\""));
        // Verify exact camelCase field names
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["agentName"], "claude-sonnet");
        assert_eq!(parsed["storiesAssigned"], 5);
        assert_eq!(parsed["averageDurationMs"], 6000);
    }

    #[test]
    fn test_agent_performance_entry_null_quality_score() {
        let entry = AgentPerformanceEntry {
            agent_name: "codex".to_string(),
            stories_assigned: 2,
            stories_completed: 0,
            success_rate: 0.0,
            average_duration_ms: 0,
            average_quality_score: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["averageQualityScore"].is_null());
    }

    #[test]
    fn test_execution_report_empty_enriched_fields() {
        // Simulates the fallback report with empty new fields
        let report = ExecutionReport {
            session_id: "test-fallback".to_string(),
            total_stories: 3,
            stories_completed: 0,
            stories_failed: 0,
            total_duration_ms: 0,
            agent_assignments: HashMap::new(),
            success: false,
            quality_scores: Vec::new(),
            timeline: Vec::new(),
            agent_performance: Vec::new(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["qualityScores"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["timeline"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["agentPerformance"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_timeline_entry_with_null_gate_result() {
        let entry = TimelineEntry {
            story_id: "s1".to_string(),
            story_title: "Story".to_string(),
            batch_index: 0,
            agent: "agent".to_string(),
            duration_ms: 0,
            start_offset_ms: 0,
            status: "failed".to_string(),
            gate_result: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["gateResult"].is_null());
    }

    #[test]
    fn test_quality_dimension_score_roundtrip() {
        let score = QualityDimensionScore {
            story_id: "s1".to_string(),
            dimension: "security".to_string(),
            score: 100.0,
            max_score: 100.0,
        };
        let json = serde_json::to_string(&score).unwrap();
        let deserialized: QualityDimensionScore = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.story_id, "s1");
        assert_eq!(deserialized.dimension, "security");
        assert_eq!(deserialized.score, 100.0);
        assert_eq!(deserialized.max_score, 100.0);
    }

    #[test]
    fn test_compute_quality_dimension_score_helper() {
        use crate::services::quality_gates::pipeline::{GatePhase, PipelineGateResult};

        // Test with matching code_review gate that passed
        let gate = PipelineGateResult::passed(
            "code_review",
            "Code Review",
            GatePhase::PostValidation,
            100,
        );
        let results = vec![&gate];
        assert_eq!(
            compute_quality_dimension_score("readability", &results, true),
            100.0
        );
        assert_eq!(
            compute_quality_dimension_score("maintainability", &results, true),
            100.0
        );

        // Test with matching ai_verify gate that passed
        let gate2 =
            PipelineGateResult::passed("ai_verify", "AI Verify", GatePhase::PostValidation, 50);
        let results2 = vec![&gate2];
        assert_eq!(
            compute_quality_dimension_score("test_coverage", &results2, true),
            100.0
        );
        assert_eq!(
            compute_quality_dimension_score("security", &results2, true),
            100.0
        );

        // Test correctness (uses overall pass/fail, no gate mapping)
        assert_eq!(
            compute_quality_dimension_score("correctness", &results, true),
            80.0
        );
        assert_eq!(
            compute_quality_dimension_score("correctness", &results, false),
            20.0
        );

        // Test fallback when no relevant gate found
        let empty: Vec<&PipelineGateResult> = vec![];
        assert_eq!(
            compute_quality_dimension_score("readability", &empty, true),
            80.0
        );
        assert_eq!(
            compute_quality_dimension_score("readability", &empty, false),
            20.0
        );
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
        let sessions = state.sessions.read().await;
        assert!(sessions.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_execute_story_via_agent_cancels_inflight_process() {
        use std::os::unix::fs::PermissionsExt;
        use tokio::time::{Duration, Instant};
        use tokio_util::sync::CancellationToken;

        let temp_dir = tempfile::tempdir().unwrap();
        let agent_path = temp_dir.path().join("fake-agent.sh");

        std::fs::write(
            &agent_path,
            "#!/bin/sh\n# Simulate a long-running agent process\nsleep 30\n",
        )
        .unwrap();
        let mut perms = std::fs::metadata(&agent_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&agent_path, perms).unwrap();

        let token = CancellationToken::new();
        let cancel_token = token.clone();
        let started = Instant::now();

        let cancel_handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(120)).await;
            cancel_token.cancel();
        });

        let agent_cmd = agent_path.to_string_lossy().into_owned();
        let outcome =
            execute_story_via_agent(&agent_cmd, "test prompt", temp_dir.path(), token).await;

        let _ = cancel_handle.await;

        assert!(!outcome.success);
        assert!(outcome
            .error
            .unwrap_or_default()
            .to_lowercase()
            .contains("cancelled"));
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "cancellation should interrupt in-flight story quickly"
        );
    }

    #[tokio::test]
    async fn test_execute_story_via_llm_returns_cancelled_when_token_cancelled() {
        use crate::services::llm::types::{ProviderConfig, ProviderType};
        use tokio::time::Duration;
        use tokio_util::sync::CancellationToken;

        let temp_dir = tempfile::tempdir().unwrap();
        let token = CancellationToken::new();
        token.cancel();

        let provider = ProviderConfig {
            provider: ProviderType::Anthropic,
            api_key: Some("test-key".to_string()),
            model: "claude-sonnet-4-6-20260219".to_string(),
            ..ProviderConfig::default()
        };

        let outcome = tokio::time::timeout(
            Duration::from_secs(3),
            execute_story_via_llm(
                Some(&provider),
                "Implement a tiny change",
                temp_dir.path(),
                None,
                "",
                "",
                "",
                &[],
                None,
                token.clone(),
            ),
        )
        .await
        .expect("llm story cancellation path should return quickly");

        assert!(!outcome.success);
        assert!(outcome
            .error
            .unwrap_or_default()
            .to_lowercase()
            .contains("cancelled"));
    }

    // ========================================================================
    // StoryContext & load_story_context Tests
    // ========================================================================

    /// Helper to create a StoryExecutionContext for prompt tests.
    fn make_story_ctx(story_context: Option<StoryContext>) -> StoryExecutionContext {
        StoryExecutionContext {
            story_id: "story-001".to_string(),
            story_title: "Add Login Feature".to_string(),
            story_description: "Implement user login with OAuth".to_string(),
            acceptance_criteria: vec![
                "Users can log in".to_string(),
                "OAuth tokens are stored securely".to_string(),
            ],
            agent_name: "claude-sonnet".to_string(),
            project_path: std::path::PathBuf::from("/tmp/test-project"),
            attempt: 1,
            retry_context: None,
            story_context,
            cancel_token: CancellationToken::new(),
        }
    }

    #[test]
    fn test_load_story_context_with_valid_file() {
        let tmp = tempfile::tempdir().unwrap();
        let design_doc = serde_json::json!({
            "story_mappings": {
                "story-001": {
                    "files": ["src/auth.rs", "src/oauth.rs"],
                    "components": ["AuthService", "OAuthProvider"],
                    "decisions": ["ADR-001: Use OAuth2"],
                    "additional_context": "Requires HTTPS in production"
                }
            }
        });
        std::fs::write(
            tmp.path().join("design_doc.json"),
            serde_json::to_string_pretty(&design_doc).unwrap(),
        )
        .unwrap();

        let ctx = load_story_context(tmp.path(), "story-001");
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.relevant_files, vec!["src/auth.rs", "src/oauth.rs"]);
        assert_eq!(ctx.components, vec!["AuthService", "OAuthProvider"]);
        assert_eq!(ctx.design_decisions, vec!["ADR-001: Use OAuth2"]);
        assert_eq!(
            ctx.additional_context.as_deref(),
            Some("Requires HTTPS in production")
        );
    }

    #[test]
    fn test_load_story_context_with_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        // No design_doc.json created
        let ctx = load_story_context(tmp.path(), "story-001");
        assert!(ctx.is_none());
    }

    #[test]
    fn test_load_story_context_with_missing_story() {
        let tmp = tempfile::tempdir().unwrap();
        let design_doc = serde_json::json!({
            "story_mappings": {
                "story-999": {
                    "files": ["src/other.rs"],
                    "components": [],
                    "decisions": []
                }
            }
        });
        std::fs::write(
            tmp.path().join("design_doc.json"),
            serde_json::to_string_pretty(&design_doc).unwrap(),
        )
        .unwrap();

        let ctx = load_story_context(tmp.path(), "story-001");
        assert!(ctx.is_none());
    }

    #[test]
    fn test_load_story_context_with_design_decisions_field() {
        // Test fallback: "design_decisions" instead of "decisions"
        let tmp = tempfile::tempdir().unwrap();
        let design_doc = serde_json::json!({
            "story_mappings": {
                "story-002": {
                    "files": [],
                    "components": [],
                    "design_decisions": ["ADR-F002: Retry with repair"]
                }
            }
        });
        std::fs::write(
            tmp.path().join("design_doc.json"),
            serde_json::to_string_pretty(&design_doc).unwrap(),
        )
        .unwrap();

        let ctx = load_story_context(tmp.path(), "story-002");
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.design_decisions, vec!["ADR-F002: Retry with repair"]);
    }

    #[test]
    fn test_load_story_context_with_invalid_json() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("design_doc.json"), "not valid json {{{").unwrap();

        let ctx = load_story_context(tmp.path(), "story-001");
        assert!(ctx.is_none());
    }

    #[test]
    fn test_load_story_context_with_no_story_mappings_key() {
        let tmp = tempfile::tempdir().unwrap();
        let design_doc = serde_json::json!({
            "title": "Design Doc",
            "components": []
        });
        std::fs::write(
            tmp.path().join("design_doc.json"),
            serde_json::to_string_pretty(&design_doc).unwrap(),
        )
        .unwrap();

        let ctx = load_story_context(tmp.path(), "story-001");
        assert!(ctx.is_none());
    }

    #[test]
    fn test_build_story_prompt_without_context() {
        let ctx = make_story_ctx(None);
        let prompt = build_story_prompt(&ctx, "", "", "");

        assert!(prompt.contains("Add Login Feature"));
        assert!(prompt.contains("story-001"));
        assert!(prompt.contains("Implement user login with OAuth"));
        assert!(prompt.contains("1. Users can log in"));
        assert!(prompt.contains("2. OAuth tokens are stored securely"));
        // Should NOT contain context section
        assert!(!prompt.contains("## Relevant Context"));
    }

    #[test]
    fn test_build_story_prompt_with_context() {
        let story_ctx = StoryContext {
            relevant_files: vec!["src/auth.rs".to_string(), "src/oauth.rs".to_string()],
            components: vec!["AuthService".to_string()],
            design_decisions: vec!["ADR-001: Use OAuth2".to_string()],
            additional_context: Some("Must support Google and GitHub".to_string()),
        };
        let ctx = make_story_ctx(Some(story_ctx));
        let prompt = build_story_prompt(&ctx, "", "", "");

        // Standard prompt parts
        assert!(prompt.contains("Add Login Feature"));
        assert!(prompt.contains("story-001"));

        // Context section
        assert!(prompt.contains("## Relevant Context"));
        assert!(prompt.contains("### Relevant Files"));
        assert!(prompt.contains("- src/auth.rs"));
        assert!(prompt.contains("- src/oauth.rs"));
        assert!(prompt.contains("### Components"));
        assert!(prompt.contains("- AuthService"));
        assert!(prompt.contains("### Design Decisions"));
        assert!(prompt.contains("- ADR-001: Use OAuth2"));
        assert!(prompt.contains("### Additional Context"));
        assert!(prompt.contains("Must support Google and GitHub"));
    }

    #[test]
    fn test_build_story_prompt_with_partial_context() {
        // Only relevant_files, no components/decisions/additional
        let story_ctx = StoryContext {
            relevant_files: vec!["src/main.rs".to_string()],
            components: vec![],
            design_decisions: vec![],
            additional_context: None,
        };
        let ctx = make_story_ctx(Some(story_ctx));
        let prompt = build_story_prompt(&ctx, "", "", "");

        assert!(prompt.contains("## Relevant Context"));
        assert!(prompt.contains("### Relevant Files"));
        assert!(prompt.contains("- src/main.rs"));
        // Empty sections should be omitted
        assert!(!prompt.contains("### Components"));
        assert!(!prompt.contains("### Design Decisions"));
        assert!(!prompt.contains("### Additional Context"));
    }

    #[test]
    fn test_story_context_serialization_roundtrip() {
        let ctx = StoryContext {
            relevant_files: vec!["src/lib.rs".to_string(), "tests/test.rs".to_string()],
            components: vec!["Parser".to_string(), "Lexer".to_string()],
            design_decisions: vec!["ADR-003: Use tree-sitter".to_string()],
            additional_context: Some("Performance-critical path".to_string()),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: StoryContext = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.relevant_files, ctx.relevant_files);
        assert_eq!(deserialized.components, ctx.components);
        assert_eq!(deserialized.design_decisions, ctx.design_decisions);
        assert_eq!(deserialized.additional_context, ctx.additional_context);

        // Verify camelCase serialization
        assert!(json.contains("\"relevantFiles\""));
        assert!(json.contains("\"designDecisions\""));
        assert!(json.contains("\"additionalContext\""));
    }

    #[test]
    fn test_story_context_serialization_with_none_additional() {
        let ctx = StoryContext {
            relevant_files: vec![],
            components: vec![],
            design_decisions: vec![],
            additional_context: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: StoryContext = serde_json::from_str(&json).unwrap();

        assert!(deserialized.relevant_files.is_empty());
        assert!(deserialized.additional_context.is_none());
    }
}
