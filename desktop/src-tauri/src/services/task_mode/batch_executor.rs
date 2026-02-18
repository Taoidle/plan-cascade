//! Batch Parallel Execution Engine
//!
//! Calculates execution batches via topological sort (Kahn's algorithm),
//! launches parallel agents within each batch, runs quality gate pipeline
//! per story, and retries failed stories with different agents.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::services::quality_gates::format::FormatGate;
use crate::services::quality_gates::pipeline::{
    GatePhase, GatePipeline, PipelineConfig, PipelineGateResult, PipelineResult,
};
use crate::services::task_mode::agent_resolver::{
    AgentAssignment, AgentResolver, ExecutionPhase, StoryInfo,
};
use crate::utils::error::{AppError, AppResult};

// ============================================================================
// Types
// ============================================================================

/// A story to be executed by the batch executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableStory {
    /// Story ID
    pub id: String,
    /// Story title
    pub title: String,
    /// Story description
    pub description: String,
    /// Dependencies (story IDs that must complete first)
    pub dependencies: Vec<String>,
    /// Acceptance criteria
    pub acceptance_criteria: Vec<String>,
    /// Explicitly assigned agent (optional)
    pub agent: Option<String>,
}

/// A batch of stories to execute in parallel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionBatch {
    /// Batch index (0-based)
    pub index: usize,
    /// Stories in this batch
    pub story_ids: Vec<String>,
}

/// Configuration for batch execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionConfig {
    /// Maximum number of parallel stories per batch
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,
    /// Maximum retry attempts per story
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Whether retry is enabled
    #[serde(default = "default_retry_enabled")]
    pub retry_enabled: bool,
}

fn default_max_parallel() -> usize {
    4
}

fn default_max_retries() -> u32 {
    2
}

fn default_retry_enabled() -> bool {
    true
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_parallel: default_max_parallel(),
            max_retries: default_max_retries(),
            retry_enabled: default_retry_enabled(),
        }
    }
}

/// Context for retrying a failed story.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryContext {
    /// Story ID
    pub story_id: String,
    /// Previous failure reason
    pub failure_reason: String,
    /// Previous gate results (if any)
    pub gate_results: Vec<PipelineGateResult>,
    /// Current attempt number (1-based)
    pub attempt: u32,
    /// Previous agent that failed
    pub previous_agent: String,
}

/// Context passed to the story executor callback for each execution attempt.
#[derive(Debug, Clone)]
pub struct StoryExecutionContext {
    /// Story ID
    pub story_id: String,
    /// Story title
    pub story_title: String,
    /// Story description
    pub story_description: String,
    /// Acceptance criteria to satisfy
    pub acceptance_criteria: Vec<String>,
    /// Agent assigned to execute this story
    pub agent_name: String,
    /// Project root path
    pub project_path: std::path::PathBuf,
    /// Current attempt number (1-based)
    pub attempt: u32,
    /// Retry context from previous failed attempt (None on first attempt)
    pub retry_context: Option<RetryContext>,
}

/// Outcome returned by the story executor callback.
#[derive(Debug, Clone)]
pub struct StoryExecutionOutcome {
    /// Whether the story executed successfully
    pub success: bool,
    /// Error message if execution failed
    pub error: Option<String>,
}

/// Status of a single story execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoryExecutionState {
    /// Waiting to be executed
    Pending,
    /// Currently running
    Running {
        agent: String,
        attempt: u32,
    },
    /// Completed successfully
    Completed {
        agent: String,
        duration_ms: u64,
        gate_result: Option<PipelineResult>,
    },
    /// Failed after all retries
    Failed {
        reason: String,
        attempts: u32,
        last_agent: String,
    },
    /// Cancelled
    Cancelled,
}

impl StoryExecutionState {
    pub fn is_completed(&self) -> bool {
        matches!(self, StoryExecutionState::Completed { .. })
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, StoryExecutionState::Failed { .. })
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StoryExecutionState::Completed { .. }
                | StoryExecutionState::Failed { .. }
                | StoryExecutionState::Cancelled
        )
    }
}

/// Progress update emitted during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchExecutionProgress {
    /// Current batch index (0-based)
    pub current_batch: usize,
    /// Total number of batches
    pub total_batches: usize,
    /// Number of stories completed so far
    pub stories_completed: usize,
    /// Number of stories failed so far
    pub stories_failed: usize,
    /// Total stories
    pub total_stories: usize,
    /// Per-story status map
    pub story_statuses: HashMap<String, String>,
    /// Current phase: "dor", "executing", "gates", "dod"
    pub current_phase: String,
}

/// Event channel name for task mode progress events.
pub const TASK_MODE_EVENT_CHANNEL: &str = "task-mode-progress";

/// Progress event payload emitted to the frontend during batch execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskModeProgressEvent {
    /// Session ID
    pub session_id: String,
    /// Event type: "batch_started", "story_started", "story_completed",
    /// "story_failed", "gates_started", "gates_completed", "batch_completed",
    /// "execution_completed", "execution_cancelled", "error"
    pub event_type: String,
    /// Current batch index (0-based)
    pub current_batch: usize,
    /// Total number of batches
    pub total_batches: usize,
    /// Story ID (if event relates to a specific story)
    pub story_id: Option<String>,
    /// Story status: "running", "completed", "failed"
    pub story_status: Option<String>,
    /// Agent name assigned to the story
    pub agent_name: Option<String>,
    /// Quality gate results (if gates have been run)
    pub gate_results: Option<Vec<PipelineGateResult>>,
    /// Error message (if any)
    pub error: Option<String>,
    /// Overall progress percentage (0-100)
    pub progress_pct: f64,
}

impl TaskModeProgressEvent {
    /// Create a batch_started event.
    pub fn batch_started(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "batch_started".to_string(),
            current_batch,
            total_batches,
            story_id: None,
            story_status: None,
            agent_name: None,
            gate_results: None,
            error: None,
            progress_pct,
        }
    }

    /// Create a story_started event.
    pub fn story_started(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        story_id: &str,
        agent_name: &str,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "story_started".to_string(),
            current_batch,
            total_batches,
            story_id: Some(story_id.to_string()),
            story_status: Some("running".to_string()),
            agent_name: Some(agent_name.to_string()),
            gate_results: None,
            error: None,
            progress_pct,
        }
    }

    /// Create a story_completed event.
    pub fn story_completed(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        story_id: &str,
        agent_name: &str,
        gate_results: Vec<PipelineGateResult>,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "story_completed".to_string(),
            current_batch,
            total_batches,
            story_id: Some(story_id.to_string()),
            story_status: Some("completed".to_string()),
            agent_name: Some(agent_name.to_string()),
            gate_results: Some(gate_results),
            error: None,
            progress_pct,
        }
    }

    /// Create a story_failed event.
    pub fn story_failed(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        story_id: &str,
        agent_name: &str,
        error: &str,
        gate_results: Option<Vec<PipelineGateResult>>,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "story_failed".to_string(),
            current_batch,
            total_batches,
            story_id: Some(story_id.to_string()),
            story_status: Some("failed".to_string()),
            agent_name: Some(agent_name.to_string()),
            gate_results,
            error: Some(error.to_string()),
            progress_pct,
        }
    }

    /// Create an execution_completed event.
    pub fn execution_completed(
        session_id: &str,
        total_batches: usize,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "execution_completed".to_string(),
            current_batch: total_batches.saturating_sub(1),
            total_batches,
            story_id: None,
            story_status: None,
            agent_name: None,
            gate_results: None,
            error: None,
            progress_pct,
        }
    }

    /// Create an execution_cancelled event.
    pub fn execution_cancelled(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "execution_cancelled".to_string(),
            current_batch,
            total_batches,
            story_id: None,
            story_status: None,
            agent_name: None,
            gate_results: None,
            error: None,
            progress_pct,
        }
    }

    /// Create an error event.
    pub fn error(session_id: &str, error: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "error".to_string(),
            current_batch: 0,
            total_batches: 0,
            story_id: None,
            story_status: None,
            agent_name: None,
            gate_results: None,
            error: Some(error.to_string()),
            progress_pct: 0.0,
        }
    }
}

/// Overall execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchExecutionResult {
    /// Whether all stories completed successfully
    pub success: bool,
    /// Total stories
    pub total_stories: usize,
    /// Stories completed
    pub completed: usize,
    /// Stories failed
    pub failed: usize,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Batches executed
    pub batches_executed: usize,
    /// Per-story results
    pub story_results: HashMap<String, StoryExecutionState>,
    /// Per-story agent assignments
    pub agent_assignments: HashMap<String, AgentAssignment>,
    /// Whether execution was cancelled
    pub cancelled: bool,
}

// ============================================================================
// Topological Sort
// ============================================================================

/// Calculate execution batches using Kahn's algorithm for topological sort.
///
/// Stories with no dependencies go into batch 0. Stories depending on batch-0
/// stories go into batch 1, etc. Stories within a batch have no mutual
/// dependencies and can be executed in parallel.
pub fn calculate_batches(
    stories: &[ExecutableStory],
    max_parallel: usize,
) -> AppResult<Vec<ExecutionBatch>> {
    let story_ids: HashSet<&str> = stories.iter().map(|s| s.id.as_str()).collect();

    // Build adjacency list and in-degree count
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

    for story in stories {
        in_degree.entry(story.id.as_str()).or_insert(0);
        for dep in &story.dependencies {
            if !story_ids.contains(dep.as_str()) {
                // Dependency not in our story set - treat as already resolved
                continue;
            }
            *in_degree.entry(story.id.as_str()).or_insert(0) += 1;
            dependents
                .entry(dep.as_str())
                .or_default()
                .push(story.id.as_str());
        }
    }

    // Detect circular dependencies
    let mut remaining: HashSet<&str> = story_ids.clone();
    let mut sorted_count = 0;
    let mut batches = Vec::new();

    // Kahn's algorithm: process nodes with in-degree 0 layer by layer
    loop {
        let ready: Vec<&str> = remaining
            .iter()
            .filter(|id| in_degree.get(*id).copied().unwrap_or(0) == 0)
            .copied()
            .collect();

        if ready.is_empty() {
            break;
        }

        // Split into sub-batches if exceeding max_parallel
        let chunks: Vec<Vec<&str>> = ready
            .chunks(max_parallel)
            .map(|c| c.to_vec())
            .collect();

        for chunk in chunks {
            let batch_index = batches.len();
            let batch = ExecutionBatch {
                index: batch_index,
                story_ids: chunk.iter().map(|id| id.to_string()).collect(),
            };
            batches.push(batch);
        }

        // Remove processed nodes and update in-degrees
        for id in &ready {
            remaining.remove(id);
            sorted_count += 1;
            if let Some(deps) = dependents.get(id) {
                for dep in deps {
                    if let Some(degree) = in_degree.get_mut(dep) {
                        *degree = degree.saturating_sub(1);
                    }
                }
            }
        }
    }

    // If we didn't process all stories, there's a cycle
    if sorted_count < stories.len() {
        let cycle_nodes: Vec<String> = remaining.iter().map(|s| s.to_string()).collect();
        return Err(AppError::validation(format!(
            "Circular dependency detected among stories: {}",
            cycle_nodes.join(", ")
        )));
    }

    Ok(batches)
}

// ============================================================================
// Batch Executor
// ============================================================================

/// Batch executor that manages parallel story execution.
pub struct BatchExecutor {
    /// Stories to execute
    stories: Vec<ExecutableStory>,
    /// Execution configuration
    config: ExecutionConfig,
    /// Cancellation token for graceful shutdown
    cancellation_token: CancellationToken,
    /// Current execution state
    state: Arc<RwLock<BatchExecutionState>>,
}

/// Internal execution state.
#[derive(Debug)]
struct BatchExecutionState {
    /// Per-story execution state
    story_states: HashMap<String, StoryExecutionState>,
    /// Current batch index
    current_batch: usize,
    /// Total batches
    total_batches: usize,
    /// Agent assignments
    agent_assignments: HashMap<String, AgentAssignment>,
}

impl BatchExecutor {
    /// Create a new batch executor.
    pub fn new(
        stories: Vec<ExecutableStory>,
        config: ExecutionConfig,
        cancellation_token: CancellationToken,
    ) -> Self {
        let story_states: HashMap<String, StoryExecutionState> = stories
            .iter()
            .map(|s| (s.id.clone(), StoryExecutionState::Pending))
            .collect();

        Self {
            stories,
            config,
            cancellation_token,
            state: Arc::new(RwLock::new(BatchExecutionState {
                story_states,
                current_batch: 0,
                total_batches: 0,
                agent_assignments: HashMap::new(),
            })),
        }
    }

    /// Calculate execution batches.
    pub fn calculate_batches(&self) -> AppResult<Vec<ExecutionBatch>> {
        calculate_batches(&self.stories, self.config.max_parallel)
    }

    /// Get the current progress snapshot.
    pub async fn get_progress(&self) -> BatchExecutionProgress {
        let state = self.state.read().await;
        let stories_completed = state
            .story_states
            .values()
            .filter(|s| s.is_completed())
            .count();
        let stories_failed = state
            .story_states
            .values()
            .filter(|s| s.is_failed())
            .count();

        let story_statuses: HashMap<String, String> = state
            .story_states
            .iter()
            .map(|(id, s)| {
                let status = match s {
                    StoryExecutionState::Pending => "pending".to_string(),
                    StoryExecutionState::Running { .. } => "running".to_string(),
                    StoryExecutionState::Completed { .. } => "completed".to_string(),
                    StoryExecutionState::Failed { .. } => "failed".to_string(),
                    StoryExecutionState::Cancelled => "cancelled".to_string(),
                };
                (id.clone(), status)
            })
            .collect();

        BatchExecutionProgress {
            current_batch: state.current_batch,
            total_batches: state.total_batches,
            stories_completed,
            stories_failed,
            total_stories: state.story_states.len(),
            story_statuses,
            current_phase: "executing".to_string(),
        }
    }

    /// Get the completed story IDs.
    pub async fn completed_story_ids(&self) -> Vec<String> {
        let state = self.state.read().await;
        state
            .story_states
            .iter()
            .filter(|(_, s)| s.is_completed())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Cancel the execution.
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    /// Update a story's state.
    pub async fn update_story_state(&self, story_id: &str, state: StoryExecutionState) {
        let mut s = self.state.write().await;
        s.story_states.insert(story_id.to_string(), state);
    }

    /// Record an agent assignment.
    pub async fn record_agent_assignment(&self, story_id: &str, assignment: AgentAssignment) {
        let mut s = self.state.write().await;
        s.agent_assignments
            .insert(story_id.to_string(), assignment);
    }

    /// Update the current batch index.
    pub async fn set_current_batch(&self, batch: usize, total: usize) {
        let mut s = self.state.write().await;
        s.current_batch = batch;
        s.total_batches = total;
    }

    /// Build the final execution result.
    pub async fn build_result(&self, duration_ms: u64) -> BatchExecutionResult {
        let state = self.state.read().await;
        let completed = state
            .story_states
            .values()
            .filter(|s| s.is_completed())
            .count();
        let failed = state
            .story_states
            .values()
            .filter(|s| s.is_failed())
            .count();

        BatchExecutionResult {
            success: failed == 0 && !self.cancellation_token.is_cancelled(),
            total_stories: state.story_states.len(),
            completed,
            failed,
            total_duration_ms: duration_ms,
            batches_executed: state.current_batch + 1,
            story_results: state.story_states.clone(),
            agent_assignments: state.agent_assignments.clone(),
            cancelled: self.cancellation_token.is_cancelled(),
        }
    }

    /// Get a reference to the stories.
    pub fn stories(&self) -> &[ExecutableStory] {
        &self.stories
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &ExecutionConfig {
        &self.config
    }

    /// Execute all batches sequentially, running stories within each batch in parallel.
    ///
    /// Uses the `AgentResolver` for agent selection and the `GatePipeline` for
    /// quality gates after each story. Emits progress events via the provided
    /// callback.
    ///
    /// `emit_event` is a callback that sends `TaskModeProgressEvent` to the frontend.
    /// This indirection allows testing without a real `AppHandle`.
    pub async fn execute<F, E>(
        &self,
        session_id: &str,
        agent_resolver: &AgentResolver,
        project_path: std::path::PathBuf,
        emit_event: F,
        story_executor: E,
    ) -> AppResult<BatchExecutionResult>
    where
        F: Fn(TaskModeProgressEvent) + Send + Sync + Clone + 'static,
        E: Fn(StoryExecutionContext) -> Pin<Box<dyn Future<Output = StoryExecutionOutcome> + Send>>
            + Send
            + Sync
            + Clone
            + 'static,
    {
        let start = Instant::now();

        // Calculate batches
        let batches = self.calculate_batches()?;
        let total_batches = batches.len();
        let total_stories = self.stories.len();

        self.set_current_batch(0, total_batches).await;

        // Build story lookup map
        let story_map: HashMap<String, &ExecutableStory> =
            self.stories.iter().map(|s| (s.id.clone(), s)).collect();

        let mut stories_processed: usize = 0;

        for batch in &batches {
            // Check cancellation before each batch
            if self.cancellation_token.is_cancelled() {
                // Mark remaining stories as cancelled
                let state = self.state.read().await;
                let pending_ids: Vec<String> = state
                    .story_states
                    .iter()
                    .filter(|(_, s)| matches!(s, StoryExecutionState::Pending))
                    .map(|(id, _)| id.clone())
                    .collect();
                drop(state);

                for id in pending_ids {
                    self.update_story_state(&id, StoryExecutionState::Cancelled)
                        .await;
                }

                let progress_pct = if total_stories > 0 {
                    (stories_processed as f64 / total_stories as f64) * 100.0
                } else {
                    100.0
                };

                emit_event(TaskModeProgressEvent::execution_cancelled(
                    session_id,
                    batch.index,
                    total_batches,
                    progress_pct,
                ));

                let duration_ms = start.elapsed().as_millis() as u64;
                return Ok(self.build_result(duration_ms).await);
            }

            self.set_current_batch(batch.index, total_batches).await;

            let progress_pct = if total_stories > 0 {
                (stories_processed as f64 / total_stories as f64) * 100.0
            } else {
                0.0
            };

            emit_event(TaskModeProgressEvent::batch_started(
                session_id,
                batch.index,
                total_batches,
                progress_pct,
            ));

            // Execute stories in this batch in parallel
            let mut handles = Vec::new();

            for story_id in &batch.story_ids {
                let story = match story_map.get(story_id) {
                    Some(s) => (*s).clone(),
                    None => continue,
                };

                // Resolve agent for this story
                let story_info = StoryInfo {
                    title: story.title.clone(),
                    description: story.description.clone(),
                    agent: story.agent.clone(),
                };
                let assignment =
                    agent_resolver.resolve(&story_info, ExecutionPhase::Implementation);

                self.record_agent_assignment(story_id, assignment.clone())
                    .await;

                // Mark story as running
                self.update_story_state(
                    story_id,
                    StoryExecutionState::Running {
                        agent: assignment.agent_name.clone(),
                        attempt: 1,
                    },
                )
                .await;

                let sid = session_id.to_string();
                let batch_index = batch.index;
                let tb = total_batches;
                let emit = emit_event.clone();
                let agent_name = assignment.agent_name.clone();
                let s_id = story_id.clone();
                let pp = project_path.clone();
                let max_retries = self.config.max_retries;
                let retry_enabled = self.config.retry_enabled;
                let cancel_token = self.cancellation_token.clone();
                let state_ref = self.state.clone();
                let resolver_config = agent_resolver.config().clone();
                let se = story_executor.clone();

                emit(TaskModeProgressEvent::story_started(
                    &sid,
                    batch_index,
                    tb,
                    &s_id,
                    &agent_name,
                    progress_pct,
                ));

                // Spawn parallel execution for this story
                let handle = tokio::spawn(async move {
                    Self::execute_story_with_retry(
                        &sid,
                        batch_index,
                        tb,
                        &story,
                        &agent_name,
                        &pp,
                        max_retries,
                        retry_enabled,
                        cancel_token,
                        state_ref,
                        resolver_config,
                        emit,
                        se,
                    )
                    .await
                });

                handles.push(handle);
            }

            // Await all parallel stories in this batch
            for handle in handles {
                let _ = handle.await;
            }

            stories_processed += batch.story_ids.len();
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let result = self.build_result(duration_ms).await;

        let final_pct = if result.success { 100.0 } else {
            if total_stories > 0 {
                (result.completed as f64 / total_stories as f64) * 100.0
            } else {
                100.0
            }
        };

        emit_event(TaskModeProgressEvent::execution_completed(
            session_id,
            total_batches,
            final_pct,
        ));

        Ok(result)
    }

    /// Execute a single story with retry logic and quality gates.
    ///
    /// On failure, retries up to `max_retries` times with a different agent
    /// (resolved via the Retry phase). The `story_executor` callback handles
    /// actual story execution (e.g., invoking the orchestrator/LLM agent).
    async fn execute_story_with_retry<E>(
        session_id: &str,
        batch_index: usize,
        total_batches: usize,
        story: &ExecutableStory,
        initial_agent: &str,
        project_path: &std::path::Path,
        max_retries: u32,
        retry_enabled: bool,
        cancel_token: CancellationToken,
        state: Arc<RwLock<BatchExecutionState>>,
        agents_config: crate::services::task_mode::agent_resolver::AgentsConfig,
        emit: impl Fn(TaskModeProgressEvent) + Send + Sync,
        story_executor: E,
    ) where
        E: Fn(StoryExecutionContext) -> Pin<Box<dyn Future<Output = StoryExecutionOutcome> + Send>>
            + Send
            + Sync,
    {
        let story_id = &story.id;
        let mut current_agent = initial_agent.to_string();
        let max_attempts = if retry_enabled { max_retries + 1 } else { 1 };
        let mut last_gate_results: Option<Vec<PipelineGateResult>> = None;
        let mut last_error = String::new();
        let mut previous_agent = String::new();

        for attempt in 1..=max_attempts {
            // Check cancellation
            if cancel_token.is_cancelled() {
                let mut s = state.write().await;
                s.story_states
                    .insert(story_id.to_string(), StoryExecutionState::Cancelled);
                return;
            }

            // Update state to Running with current attempt
            {
                let mut s = state.write().await;
                s.story_states.insert(
                    story_id.to_string(),
                    StoryExecutionState::Running {
                        agent: current_agent.clone(),
                        attempt,
                    },
                );
            }

            let story_start = Instant::now();

            // Build execution context for the story executor callback
            let retry_context = if attempt > 1 {
                Some(RetryContext {
                    story_id: story_id.to_string(),
                    failure_reason: last_error.clone(),
                    gate_results: last_gate_results.clone().unwrap_or_default(),
                    attempt,
                    previous_agent: previous_agent.clone(),
                })
            } else {
                None
            };

            let ctx = StoryExecutionContext {
                story_id: story_id.to_string(),
                story_title: story.title.clone(),
                story_description: story.description.clone(),
                acceptance_criteria: story.acceptance_criteria.clone(),
                agent_name: current_agent.clone(),
                project_path: project_path.to_path_buf(),
                attempt,
                retry_context,
            };

            // Execute story via the provided executor callback
            let outcome = story_executor(ctx).await;

            if !outcome.success {
                last_error =
                    outcome.error.unwrap_or_else(|| "Story execution failed".to_string());
                previous_agent = current_agent.clone();
                // On retry, switch to a different agent
                if attempt < max_attempts {
                    let resolver = AgentResolver::new(agents_config.clone());
                    let story_info = StoryInfo {
                        title: story_id.to_string(),
                        description: String::new(),
                        agent: None,
                    };
                    let retry_assignment =
                        resolver.resolve(&story_info, ExecutionPhase::Retry);
                    current_agent = retry_assignment.agent_name.clone();

                    // Record the retry agent assignment
                    let mut s = state.write().await;
                    s.agent_assignments
                        .insert(story_id.to_string(), retry_assignment);
                }
                continue;
            }

            // Run quality gate pipeline with registered gates
            let pipeline_config = PipelineConfig::new(project_path.to_path_buf());
            let mut pipeline = GatePipeline::new(pipeline_config);

            // Register FormatGate for automated code formatting validation
            let format_path = project_path.to_path_buf();
            pipeline.register_gate(
                "format",
                Box::new(move || {
                    let gate = FormatGate::new(format_path.clone());
                    Box::pin(async move { gate.run().await })
                }),
            );

            let gate_result = pipeline.execute().await;

            match gate_result {
                Ok(pipeline_result) => {
                    let gate_results: Vec<PipelineGateResult> = pipeline_result
                        .phase_results
                        .iter()
                        .flat_map(|pr| pr.gate_results.clone())
                        .collect();

                    if pipeline_result.passed {
                        // Story completed successfully
                        let duration_ms = story_start.elapsed().as_millis() as u64;
                        {
                            let mut s = state.write().await;
                            s.story_states.insert(
                                story_id.to_string(),
                                StoryExecutionState::Completed {
                                    agent: current_agent.clone(),
                                    duration_ms,
                                    gate_result: Some(pipeline_result),
                                },
                            );
                        }

                        emit(TaskModeProgressEvent::story_completed(
                            session_id,
                            batch_index,
                            total_batches,
                            story_id,
                            &current_agent,
                            gate_results,
                            0.0, // Progress will be recalculated by caller
                        ));
                        return;
                    } else {
                        // Gates failed
                        last_error = format!(
                            "Quality gates failed: {}",
                            pipeline_result
                                .short_circuit_phase
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| "validation".to_string())
                        );
                        last_gate_results = Some(gate_results);

                        // On retry, switch to a different agent
                        if attempt < max_attempts {
                            let resolver = AgentResolver::new(agents_config.clone());
                            let story_info = StoryInfo {
                                title: story_id.to_string(),
                                description: String::new(),
                                agent: None,
                            };
                            let retry_assignment =
                                resolver.resolve(&story_info, ExecutionPhase::Retry);
                            current_agent = retry_assignment.agent_name.clone();

                            let mut s = state.write().await;
                            s.agent_assignments
                                .insert(story_id.to_string(), retry_assignment);
                        }
                    }
                }
                Err(e) => {
                    last_error = format!("Quality gate pipeline error: {}", e);
                    last_gate_results = None;
                }
            }
        }

        // All retries exhausted -- mark as failed
        {
            let mut s = state.write().await;
            s.story_states.insert(
                story_id.to_string(),
                StoryExecutionState::Failed {
                    reason: last_error.clone(),
                    attempts: max_attempts,
                    last_agent: current_agent.clone(),
                },
            );
        }

        emit(TaskModeProgressEvent::story_failed(
            session_id,
            batch_index,
            total_batches,
            story_id,
            &current_agent,
            &last_error,
            last_gate_results,
            0.0,
        ));
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn story(id: &str, deps: Vec<&str>) -> ExecutableStory {
        ExecutableStory {
            id: id.to_string(),
            title: format!("Story {}", id),
            description: format!("Description for {}", id),
            dependencies: deps.iter().map(|s| s.to_string()).collect(),
            acceptance_criteria: vec!["Criterion 1".to_string(), "Criterion 2".to_string()],
            agent: None,
        }
    }

    /// Mock story executor that always succeeds. Used in tests where
    /// the focus is on batch orchestration, not story execution.
    fn mock_story_executor(
    ) -> impl Fn(StoryExecutionContext) -> Pin<Box<dyn Future<Output = StoryExecutionOutcome> + Send>>
           + Send
           + Sync
           + Clone {
        |_ctx| Box::pin(async { StoryExecutionOutcome { success: true, error: None } })
    }

    // ========================================================================
    // Topological Sort Tests
    // ========================================================================

    #[test]
    fn test_no_dependencies() {
        let stories = vec![story("s1", vec![]), story("s2", vec![]), story("s3", vec![])];
        let batches = calculate_batches(&stories, 10).unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].story_ids.len(), 3);
    }

    #[test]
    fn test_linear_dependencies() {
        let stories = vec![
            story("s1", vec![]),
            story("s2", vec!["s1"]),
            story("s3", vec!["s2"]),
        ];
        let batches = calculate_batches(&stories, 10).unwrap();
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].story_ids, vec!["s1"]);
        assert_eq!(batches[1].story_ids, vec!["s2"]);
        assert_eq!(batches[2].story_ids, vec!["s3"]);
    }

    #[test]
    fn test_diamond_dependencies() {
        // s1 -> s2, s3 -> s4
        let stories = vec![
            story("s1", vec![]),
            story("s2", vec!["s1"]),
            story("s3", vec!["s1"]),
            story("s4", vec!["s2", "s3"]),
        ];
        let batches = calculate_batches(&stories, 10).unwrap();
        // s1 in batch 0, s2+s3 in batch 1, s4 in batch 2
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].story_ids, vec!["s1"]);
        assert!(batches[1].story_ids.contains(&"s2".to_string()));
        assert!(batches[1].story_ids.contains(&"s3".to_string()));
        assert_eq!(batches[2].story_ids, vec!["s4"]);
    }

    #[test]
    fn test_circular_dependency_detected() {
        let stories = vec![
            story("s1", vec!["s3"]),
            story("s2", vec!["s1"]),
            story("s3", vec!["s2"]),
        ];
        let result = calculate_batches(&stories, 10);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Circular dependency"));
    }

    #[test]
    fn test_max_parallel_splits_batches() {
        let stories = vec![
            story("s1", vec![]),
            story("s2", vec![]),
            story("s3", vec![]),
            story("s4", vec![]),
            story("s5", vec![]),
        ];
        let batches = calculate_batches(&stories, 2).unwrap();
        // 5 stories with no deps, max 2 per batch -> 3 batches
        assert!(batches.len() >= 3);
        for batch in &batches {
            assert!(batch.story_ids.len() <= 2);
        }
    }

    #[test]
    fn test_external_dependency_treated_as_resolved() {
        // s1 depends on "external" which is not in the story set
        let stories = vec![
            story("s1", vec!["external"]),
            story("s2", vec!["s1"]),
        ];
        let batches = calculate_batches(&stories, 10).unwrap();
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn test_empty_stories() {
        let stories: Vec<ExecutableStory> = vec![];
        let batches = calculate_batches(&stories, 10).unwrap();
        assert_eq!(batches.len(), 0);
    }

    #[test]
    fn test_single_story() {
        let stories = vec![story("s1", vec![])];
        let batches = calculate_batches(&stories, 10).unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].story_ids, vec!["s1"]);
    }

    // ========================================================================
    // BatchExecutor Tests
    // ========================================================================

    #[tokio::test]
    async fn test_executor_initial_state() {
        let stories = vec![story("s1", vec![]), story("s2", vec!["s1"])];
        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);

        let progress = executor.get_progress().await;
        assert_eq!(progress.total_stories, 2);
        assert_eq!(progress.stories_completed, 0);
        assert_eq!(progress.stories_failed, 0);
    }

    #[tokio::test]
    async fn test_executor_update_state() {
        let stories = vec![story("s1", vec![])];
        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);

        executor
            .update_story_state(
                "s1",
                StoryExecutionState::Completed {
                    agent: "test-agent".to_string(),
                    duration_ms: 1000,
                    gate_result: None,
                },
            )
            .await;

        let progress = executor.get_progress().await;
        assert_eq!(progress.stories_completed, 1);
    }

    #[tokio::test]
    async fn test_executor_cancellation() {
        let stories = vec![story("s1", vec![])];
        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);

        assert!(!executor.is_cancelled());
        executor.cancel();
        assert!(executor.is_cancelled());
    }

    #[tokio::test]
    async fn test_executor_build_result() {
        let stories = vec![story("s1", vec![]), story("s2", vec![])];
        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);

        executor
            .update_story_state(
                "s1",
                StoryExecutionState::Completed {
                    agent: "agent1".to_string(),
                    duration_ms: 500,
                    gate_result: None,
                },
            )
            .await;
        executor
            .update_story_state(
                "s2",
                StoryExecutionState::Failed {
                    reason: "test failure".to_string(),
                    attempts: 2,
                    last_agent: "agent2".to_string(),
                },
            )
            .await;

        let result = executor.build_result(1000).await;
        assert!(!result.success);
        assert_eq!(result.completed, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.total_stories, 2);
    }

    #[tokio::test]
    async fn test_executor_completed_story_ids() {
        let stories = vec![story("s1", vec![]), story("s2", vec![])];
        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);

        executor
            .update_story_state(
                "s1",
                StoryExecutionState::Completed {
                    agent: "agent1".to_string(),
                    duration_ms: 500,
                    gate_result: None,
                },
            )
            .await;

        let completed = executor.completed_story_ids().await;
        assert_eq!(completed.len(), 1);
        assert!(completed.contains(&"s1".to_string()));
    }

    // ========================================================================
    // Serialization Tests
    // ========================================================================

    #[test]
    fn test_execution_config_serialization() {
        let config = ExecutionConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"maxParallel\""));
        assert!(json.contains("\"maxRetries\""));
        assert!(json.contains("\"retryEnabled\""));
    }

    #[test]
    fn test_batch_execution_progress_serialization() {
        let progress = BatchExecutionProgress {
            current_batch: 0,
            total_batches: 3,
            stories_completed: 2,
            stories_failed: 0,
            total_stories: 5,
            story_statuses: HashMap::new(),
            current_phase: "executing".to_string(),
        };
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"currentBatch\""));
        assert!(json.contains("\"totalBatches\""));
        assert!(json.contains("\"storiesCompleted\""));
    }

    #[test]
    fn test_retry_context_serialization() {
        let ctx = RetryContext {
            story_id: "s1".to_string(),
            failure_reason: "tests failed".to_string(),
            gate_results: vec![],
            attempt: 2,
            previous_agent: "claude-sonnet".to_string(),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("\"storyId\""));
        assert!(json.contains("\"failureReason\""));
        assert!(json.contains("\"previousAgent\""));
    }

    #[test]
    fn test_story_execution_state_helpers() {
        assert!(StoryExecutionState::Completed {
            agent: "a".to_string(),
            duration_ms: 0,
            gate_result: None,
        }
        .is_completed());

        assert!(StoryExecutionState::Failed {
            reason: "err".to_string(),
            attempts: 1,
            last_agent: "a".to_string(),
        }
        .is_failed());

        assert!(!StoryExecutionState::Pending.is_terminal());
        assert!(StoryExecutionState::Cancelled.is_terminal());
    }

    // ========================================================================
    // TaskModeProgressEvent Tests
    // ========================================================================

    #[test]
    fn test_progress_event_batch_started() {
        let event = TaskModeProgressEvent::batch_started("sess-1", 0, 3, 0.0);
        assert_eq!(event.session_id, "sess-1");
        assert_eq!(event.event_type, "batch_started");
        assert_eq!(event.current_batch, 0);
        assert_eq!(event.total_batches, 3);
        assert!(event.story_id.is_none());
        assert!(event.error.is_none());
    }

    #[test]
    fn test_progress_event_story_started() {
        let event = TaskModeProgressEvent::story_started(
            "sess-1", 0, 3, "s1", "claude-sonnet", 33.3,
        );
        assert_eq!(event.event_type, "story_started");
        assert_eq!(event.story_id, Some("s1".to_string()));
        assert_eq!(event.story_status, Some("running".to_string()));
        assert_eq!(event.agent_name, Some("claude-sonnet".to_string()));
    }

    #[test]
    fn test_progress_event_story_completed() {
        let gate_results = vec![
            PipelineGateResult::passed("format", "Format", GatePhase::PreValidation, 10),
        ];
        let event = TaskModeProgressEvent::story_completed(
            "sess-1", 0, 3, "s1", "claude-sonnet", gate_results.clone(), 50.0,
        );
        assert_eq!(event.event_type, "story_completed");
        assert_eq!(event.story_status, Some("completed".to_string()));
        assert!(event.gate_results.is_some());
        assert_eq!(event.gate_results.unwrap().len(), 1);
    }

    #[test]
    fn test_progress_event_story_failed() {
        let event = TaskModeProgressEvent::story_failed(
            "sess-1", 0, 3, "s1", "claude-sonnet", "tests failed", None, 25.0,
        );
        assert_eq!(event.event_type, "story_failed");
        assert_eq!(event.story_status, Some("failed".to_string()));
        assert_eq!(event.error, Some("tests failed".to_string()));
    }

    #[test]
    fn test_progress_event_execution_completed() {
        let event = TaskModeProgressEvent::execution_completed("sess-1", 3, 100.0);
        assert_eq!(event.event_type, "execution_completed");
        assert_eq!(event.progress_pct, 100.0);
    }

    #[test]
    fn test_progress_event_execution_cancelled() {
        let event = TaskModeProgressEvent::execution_cancelled("sess-1", 1, 3, 33.3);
        assert_eq!(event.event_type, "execution_cancelled");
        assert_eq!(event.current_batch, 1);
    }

    #[test]
    fn test_progress_event_error() {
        let event = TaskModeProgressEvent::error("sess-1", "something went wrong");
        assert_eq!(event.event_type, "error");
        assert_eq!(event.error, Some("something went wrong".to_string()));
    }

    #[test]
    fn test_progress_event_serialization() {
        let event = TaskModeProgressEvent::story_started(
            "sess-1", 0, 3, "s1", "claude-sonnet", 25.0,
        );
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"eventType\""));
        assert!(json.contains("\"currentBatch\""));
        assert!(json.contains("\"totalBatches\""));
        assert!(json.contains("\"storyId\""));
        assert!(json.contains("\"storyStatus\""));
        assert!(json.contains("\"agentName\""));
        assert!(json.contains("\"progressPct\""));
    }

    #[test]
    fn test_task_mode_event_channel_constant() {
        assert_eq!(TASK_MODE_EVENT_CHANNEL, "task-mode-progress");
    }

    // ========================================================================
    // BatchExecutor.execute() Tests
    // ========================================================================

    #[tokio::test]
    async fn test_execute_processes_batches_sequentially() {
        // Create stories with dependencies: s1 (no deps), s2 (depends on s1),
        // s3 (depends on s1), s4 (depends on s2 and s3).
        // Expected batches: [s1], [s2, s3], [s4]
        let stories = vec![
            story("s1", vec![]),
            story("s2", vec!["s1"]),
            story("s3", vec!["s1"]),
            story("s4", vec!["s2", "s3"]),
        ];

        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);
        let resolver = AgentResolver::with_defaults();

        // Collect emitted events
        let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let emit = move |event: TaskModeProgressEvent| {
            let events = events_clone.clone();
            // Use blocking lock since this is called from sync context
            if let Ok(mut evts) = events.try_lock() {
                evts.push(event);
            };
        };

        let result = executor
            .execute("test-session", &resolver, std::path::PathBuf::from("/tmp"), emit, mock_story_executor())
            .await
            .unwrap();

        // All stories should complete
        assert!(result.success);
        assert_eq!(result.total_stories, 4);
        assert_eq!(result.completed, 4);
        assert_eq!(result.failed, 0);
        assert!(!result.cancelled);

        // Check events were emitted
        let emitted = events.lock().await;
        // Should have: batch_started for each batch, story_started and story_completed
        // for each story, and execution_completed
        let batch_started_count = emitted
            .iter()
            .filter(|e| e.event_type == "batch_started")
            .count();
        assert_eq!(batch_started_count, 3, "Should have 3 batch_started events");

        let story_completed_count = emitted
            .iter()
            .filter(|e| e.event_type == "story_completed")
            .count();
        assert_eq!(story_completed_count, 4, "Should have 4 story_completed events");

        let execution_completed = emitted
            .iter()
            .any(|e| e.event_type == "execution_completed");
        assert!(execution_completed, "Should have execution_completed event");
    }

    #[tokio::test]
    async fn test_execute_batch_order_respects_dependencies() {
        // Linear chain: s1 -> s2 -> s3
        let stories = vec![
            story("s1", vec![]),
            story("s2", vec!["s1"]),
            story("s3", vec!["s2"]),
        ];

        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);
        let resolver = AgentResolver::with_defaults();

        let batch_order = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let batch_order_clone = batch_order.clone();
        let emit = move |event: TaskModeProgressEvent| {
            let order = batch_order_clone.clone();
            if event.event_type == "story_completed" {
                if let Ok(mut ord) = order.try_lock() {
                    if let Some(ref sid) = event.story_id {
                        ord.push((sid.clone(), event.current_batch));
                    }
                }
            }
        };

        let result = executor
            .execute("test-session", &resolver, std::path::PathBuf::from("/tmp"), emit, mock_story_executor())
            .await
            .unwrap();

        assert!(result.success);

        let order = batch_order.lock().await;
        assert_eq!(order.len(), 3);

        // s1 must complete in batch 0 before s2 in batch 1 before s3 in batch 2
        let s1_batch = order.iter().find(|(id, _)| id == "s1").map(|(_, b)| *b);
        let s2_batch = order.iter().find(|(id, _)| id == "s2").map(|(_, b)| *b);
        let s3_batch = order.iter().find(|(id, _)| id == "s3").map(|(_, b)| *b);

        assert!(
            s1_batch.unwrap() < s2_batch.unwrap(),
            "s1 (batch {:?}) must execute before s2 (batch {:?})",
            s1_batch, s2_batch,
        );
        assert!(
            s2_batch.unwrap() < s3_batch.unwrap(),
            "s2 (batch {:?}) must execute before s3 (batch {:?})",
            s2_batch, s3_batch,
        );
    }

    #[tokio::test]
    async fn test_execute_cancellation_stops_execution() {
        // Two batches: [s1, s2], [s3]
        let stories = vec![
            story("s1", vec![]),
            story("s2", vec![]),
            story("s3", vec!["s1"]),
        ];

        let token = CancellationToken::new();
        let token_clone = token.clone();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);
        let resolver = AgentResolver::with_defaults();

        let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();

        // Cancel after batch 0 starts by cancelling on the first batch_started event
        // for batch index > 0 won't happen because we cancel synchronously on first completion
        let emit = move |event: TaskModeProgressEvent| {
            let events = events_clone.clone();
            let token = token_clone.clone();
            if let Ok(mut evts) = events.try_lock() {
                evts.push(event.clone());
                // Cancel after first batch completes (looking for story_completed)
                let completed_count = evts
                    .iter()
                    .filter(|e| e.event_type == "story_completed")
                    .count();
                if completed_count >= 2 {
                    // Both s1 and s2 completed, cancel before batch 2
                    token.cancel();
                }
            };
        };

        let result = executor
            .execute("test-session", &resolver, std::path::PathBuf::from("/tmp"), emit, mock_story_executor())
            .await
            .unwrap();

        assert!(result.cancelled);

        // s3 should be cancelled (not completed)
        let s3_state = result.story_results.get("s3");
        assert!(
            s3_state.is_some(),
            "s3 should be in results"
        );
        assert!(
            matches!(s3_state.unwrap(), StoryExecutionState::Cancelled),
            "s3 should be cancelled, got: {:?}",
            s3_state,
        );
    }

    #[tokio::test]
    async fn test_execute_agent_resolution_uses_resolver() {
        let stories = vec![story("s1", vec![])];

        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);
        let resolver = AgentResolver::with_defaults();

        let emit = |_: TaskModeProgressEvent| {};

        let result = executor
            .execute("test-session", &resolver, std::path::PathBuf::from("/tmp"), emit, mock_story_executor())
            .await
            .unwrap();

        assert!(result.success);

        // Check that an agent was assigned
        assert!(
            result.agent_assignments.contains_key("s1"),
            "s1 should have an agent assignment"
        );
        let assignment = &result.agent_assignments["s1"];
        assert!(
            !assignment.agent_name.is_empty(),
            "Agent name should not be empty"
        );
    }

    #[tokio::test]
    async fn test_execute_empty_stories() {
        let stories: Vec<ExecutableStory> = vec![];

        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);
        let resolver = AgentResolver::with_defaults();

        let emit = |_: TaskModeProgressEvent| {};

        let result = executor
            .execute("test-session", &resolver, std::path::PathBuf::from("/tmp"), emit, mock_story_executor())
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.total_stories, 0);
        assert_eq!(result.completed, 0);
    }

    #[tokio::test]
    async fn test_execute_parallel_stories_in_same_batch() {
        // All stories have no dependencies - should execute in one batch in parallel
        let stories = vec![
            story("s1", vec![]),
            story("s2", vec![]),
            story("s3", vec![]),
        ];

        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);
        let resolver = AgentResolver::with_defaults();

        let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let emit = move |event: TaskModeProgressEvent| {
            if let Ok(mut evts) = events_clone.try_lock() {
                evts.push(event);
            }
        };

        let result = executor
            .execute("test-session", &resolver, std::path::PathBuf::from("/tmp"), emit, mock_story_executor())
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.completed, 3);

        let emitted = events.lock().await;
        // Only 1 batch_started event (all stories in one batch)
        let batch_started_count = emitted
            .iter()
            .filter(|e| e.event_type == "batch_started")
            .count();
        assert_eq!(batch_started_count, 1, "All stories should be in one batch");

        // All 3 stories should have been started
        let story_started_count = emitted
            .iter()
            .filter(|e| e.event_type == "story_started")
            .count();
        assert_eq!(story_started_count, 3);
    }

    #[tokio::test]
    async fn test_execute_progress_events_include_required_fields() {
        let stories = vec![story("s1", vec![])];

        let token = CancellationToken::new();
        let executor = BatchExecutor::new(stories, ExecutionConfig::default(), token);
        let resolver = AgentResolver::with_defaults();

        let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let emit = move |event: TaskModeProgressEvent| {
            if let Ok(mut evts) = events_clone.try_lock() {
                evts.push(event);
            }
        };

        let _ = executor
            .execute("sess-123", &resolver, std::path::PathBuf::from("/tmp"), emit, mock_story_executor())
            .await
            .unwrap();

        let emitted = events.lock().await;

        // Verify each story_completed event has the required payload fields
        for event in emitted.iter().filter(|e| e.event_type == "story_completed") {
            assert_eq!(event.session_id, "sess-123");
            assert!(event.story_id.is_some(), "story_id required");
            assert_eq!(event.story_status, Some("completed".to_string()));
            assert!(event.agent_name.is_some(), "agent_name required");
            assert!(event.gate_results.is_some(), "gate_results required");
        }
    }
}
