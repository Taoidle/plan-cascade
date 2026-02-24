//! Iteration Loop Runner
//!
//! Implements the auto-iteration system for executing PRD stories with
//! quality gates integration and retry logic.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::future::join_all;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

use crate::models::iteration::{IterationConfig, IterationMode, IterationResult, IterationState};
use crate::models::prd::{Prd, StoryStatus};
use crate::services::dependency::{Batch, DependencyAnalyzer, DependencyError};

/// Configuration for the iteration loop
#[derive(Debug, Clone)]
pub struct IterationLoopConfig {
    /// Iteration configuration
    pub iteration: IterationConfig,
    /// Project root directory
    pub project_root: PathBuf,
    /// Path to the PRD file
    pub prd_path: PathBuf,
    /// Whether to save state after each iteration
    pub persist_state: bool,
}

impl Default for IterationLoopConfig {
    fn default() -> Self {
        Self {
            iteration: IterationConfig::default(),
            project_root: PathBuf::from("."),
            prd_path: PathBuf::from("prd.json"),
            persist_state: true,
        }
    }
}

/// Errors from iteration loop operations
#[derive(Debug, thiserror::Error)]
pub enum IterationLoopError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("PRD error: {0}")]
    PrdError(String),

    #[error("Dependency error: {0}")]
    DependencyError(#[from] DependencyError),

    #[error("Story execution error: {0}")]
    StoryExecutionError(String),

    #[error("Quality gate error: {0}")]
    QualityGateError(String),

    #[error("Cancelled")]
    Cancelled,

    #[error("Max iterations reached: {0}")]
    MaxIterationsReached(u32),
}

/// Events emitted during iteration
#[derive(Debug, Clone)]
pub enum IterationEvent {
    /// Iteration loop started
    Started { total_stories: usize },
    /// New iteration started
    IterationStarted { iteration: u32 },
    /// Batch started
    BatchStarted {
        batch_index: usize,
        story_count: usize,
    },
    /// Story execution started
    StoryStarted { story_id: String, title: String },
    /// Story execution completed
    StoryCompleted { story_id: String, success: bool },
    /// Story failed
    StoryFailed { story_id: String, error: String },
    /// Story queued for retry
    StoryRetryQueued { story_id: String, retry_number: u32 },
    /// Quality gates started for story
    QualityGatesStarted { story_id: String },
    /// Quality gates completed
    QualityGatesCompleted { story_id: String, passed: bool },
    /// Batch completed
    BatchCompleted { batch_index: usize },
    /// Iteration completed
    IterationCompleted {
        iteration: u32,
        completed_stories: usize,
    },
    /// Loop completed
    Completed { result: IterationResult },
    /// Progress update
    Progress {
        completed: usize,
        total: usize,
        percentage: f32,
    },
    /// Error occurred
    Error { message: String },
}

/// Quality gate result with details from the pipeline execution.
#[derive(Debug, Clone)]
pub struct QualityGateResult {
    pub passed: bool,
    pub error: Option<String>,
    /// Detailed findings from individual gates (for retry context injection)
    pub details: Vec<String>,
}

impl QualityGateResult {
    /// Create a passing result.
    pub fn pass() -> Self {
        Self {
            passed: true,
            error: None,
            details: Vec::new(),
        }
    }

    /// Create a failing result with error message and findings.
    pub fn fail(error: String, details: Vec<String>) -> Self {
        Self {
            passed: false,
            error: Some(error),
            details,
        }
    }
}

/// Context passed to the quality gate runner callback.
#[derive(Debug, Clone)]
pub struct QualityGateContext {
    /// The story ID that just completed
    pub story_id: String,
    /// Project root path for running gates against
    pub project_root: PathBuf,
}

/// Type alias for the quality gate runner callback.
///
/// The callback receives a `QualityGateContext` and returns a future that
/// resolves to a `QualityGateResult`. This follows the same dependency
/// injection pattern as `StoryExecutorFn`.
pub type QualityGateRunnerFn = Arc<
    dyn Fn(QualityGateContext) -> Pin<Box<dyn Future<Output = QualityGateResult> + Send>>
        + Send
        + Sync,
>;

/// Execution result for a story
#[derive(Debug, Clone)]
pub struct StoryExecutionResult {
    pub success: bool,
    pub error: Option<String>,
}

/// Context passed to the story executor callback for each story execution.
///
/// Contains all the information needed to execute a single story.
#[derive(Debug, Clone)]
pub struct StoryExecutionContext {
    /// Story ID
    pub story_id: String,
    /// Story title
    pub story_title: String,
    /// Story description
    pub story_description: String,
    /// Acceptance criteria descriptions
    pub acceptance_criteria: Vec<String>,
    /// Project root path
    pub project_root: PathBuf,
}

/// Type alias for the story executor callback.
///
/// The callback receives a `StoryExecutionContext` and returns a future that
/// resolves to a `StoryExecutionResult`. This follows the same dependency
/// injection pattern used by `task_mode/batch_executor.rs`.
pub type StoryExecutorFn = Arc<
    dyn Fn(StoryExecutionContext) -> Pin<Box<dyn Future<Output = StoryExecutionResult> + Send>>
        + Send
        + Sync,
>;

/// Iteration loop for auto-executing PRD stories
pub struct IterationLoop {
    /// Configuration
    config: IterationLoopConfig,
    /// The PRD being executed
    prd: Arc<RwLock<Prd>>,
    /// Current state
    state: Arc<RwLock<IterationState>>,
    /// Generated batches
    batches: Vec<Batch>,
    /// Cancellation token
    cancellation_token: CancellationToken,
    /// Story executor callback for running individual stories
    story_executor: Option<StoryExecutorFn>,
    /// Quality gate runner callback for validating story results
    quality_gate_runner: Option<QualityGateRunnerFn>,
}

impl IterationLoop {
    /// Create a new iteration loop
    pub fn new(config: IterationLoopConfig, prd: Prd) -> Result<Self, IterationLoopError> {
        // Generate batches from PRD
        let batches = DependencyAnalyzer::generate_batches(&prd)?;

        Ok(Self {
            config,
            prd: Arc::new(RwLock::new(prd)),
            state: Arc::new(RwLock::new(IterationState::new())),
            batches,
            cancellation_token: CancellationToken::new(),
            story_executor: None,
            quality_gate_runner: None,
        })
    }

    /// Set a story executor callback (builder pattern).
    ///
    /// When set, `execute_stories_parallel()` will call this callback for each
    /// story instead of returning a simulated success. The callback receives a
    /// `StoryExecutionContext` with the story's ID, title, description,
    /// acceptance criteria, and project root.
    pub fn with_story_executor(mut self, executor: StoryExecutorFn) -> Self {
        self.story_executor = Some(executor);
        self
    }

    /// Set a quality gate runner callback (builder pattern).
    ///
    /// When set, `run_quality_gates()` will call this callback after each
    /// story execution instead of returning a hardcoded pass. The callback
    /// receives a `QualityGateContext` with the story ID and project root.
    ///
    /// If no runner is set, quality gates default to passing (backward compat).
    pub fn with_quality_gate_runner(mut self, runner: QualityGateRunnerFn) -> Self {
        self.quality_gate_runner = Some(runner);
        self
    }

    /// Load from existing state file
    pub fn from_state(
        config: IterationLoopConfig,
        prd: Prd,
        state: IterationState,
    ) -> Result<Self, IterationLoopError> {
        let batches = DependencyAnalyzer::generate_batches(&prd)?;

        Ok(Self {
            config,
            prd: Arc::new(RwLock::new(prd)),
            state: Arc::new(RwLock::new(state)),
            batches,
            cancellation_token: CancellationToken::new(),
            story_executor: None,
            quality_gate_runner: None,
        })
    }

    /// Get the cancellation token
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    /// Cancel execution
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    /// Get current state
    pub async fn get_state(&self) -> IterationState {
        self.state.read().await.clone()
    }

    /// Main iteration loop
    pub async fn run(
        &self,
        event_tx: mpsc::Sender<IterationEvent>,
    ) -> Result<IterationResult, IterationLoopError> {
        let start_time = Instant::now();

        // Initialize state
        {
            let mut state = self.state.write().await;
            state.start();
        }

        let total_stories = {
            let prd = self.prd.read().await;
            prd.stories.len()
        };

        let _ = event_tx
            .send(IterationEvent::Started { total_stories })
            .await;

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                let mut state = self.state.write().await;
                state.cancel();
                return Err(IterationLoopError::Cancelled);
            }

            // Check termination conditions
            if self.should_terminate().await {
                break;
            }

            // Get pending stories for current batch
            let pending = self.get_pending_stories().await;

            if pending.is_empty() {
                // Try to advance to next batch
                if !self.advance_to_next_batch().await {
                    break; // No more batches
                }
                continue;
            }

            // Increment iteration count
            let iteration = {
                let mut state = self.state.write().await;
                state.iteration_count += 1;
                state.iteration_count
            };

            let _ = event_tx
                .send(IterationEvent::IterationStarted { iteration })
                .await;

            // Execute stories in parallel (up to max_concurrent)
            let results = self
                .execute_stories_parallel(&pending, event_tx.clone())
                .await?;

            // Process results and run quality gates
            for (story_id, result) in results {
                if result.success {
                    // Run quality gates if enabled
                    if self.config.iteration.run_quality_gates {
                        let _ = event_tx
                            .send(IterationEvent::QualityGatesStarted {
                                story_id: story_id.clone(),
                            })
                            .await;

                        let gate_result = self.run_quality_gates(&story_id).await;

                        let _ = event_tx
                            .send(IterationEvent::QualityGatesCompleted {
                                story_id: story_id.clone(),
                                passed: gate_result.passed,
                            })
                            .await;

                        if gate_result.passed {
                            self.mark_story_complete(&story_id).await;
                        } else if self.can_retry(&story_id).await {
                            self.queue_retry(&story_id, &gate_result).await;

                            let retry_number = {
                                let state = self.state.read().await;
                                state.get_retry_count(&story_id)
                            };

                            let _ = event_tx
                                .send(IterationEvent::StoryRetryQueued {
                                    story_id: story_id.clone(),
                                    retry_number,
                                })
                                .await;
                        } else {
                            self.mark_story_failed(&story_id, gate_result.error).await;

                            // Check if we should stop on failure
                            if self.config.iteration.stop_on_failure {
                                let mut state = self.state.write().await;
                                state.fail("Quality gate failure with stop_on_failure enabled");
                                return Err(IterationLoopError::QualityGateError(
                                    "Quality gates failed".to_string(),
                                ));
                            }
                        }
                    } else {
                        self.mark_story_complete(&story_id).await;
                    }
                } else {
                    let error = result.error.unwrap_or_else(|| "Unknown error".to_string());

                    if self.can_retry(&story_id).await {
                        self.queue_retry(
                            &story_id,
                            &QualityGateResult::fail(error.clone(), Vec::new()),
                        )
                        .await;
                    } else {
                        self.mark_story_failed(&story_id, Some(error.clone())).await;

                        if self.config.iteration.stop_on_failure {
                            let mut state = self.state.write().await;
                            state.fail(&error);
                            return Err(IterationLoopError::StoryExecutionError(error));
                        }
                    }
                }
            }

            // Save state if configured
            if self.config.persist_state {
                self.save_state().await?;
            }

            // Emit progress
            let (completed, total, percentage) = {
                let state = self.state.read().await;
                let completed = state.completed_stories.len();
                let percentage = if total_stories > 0 {
                    (completed as f32 / total_stories as f32) * 100.0
                } else {
                    100.0
                };
                (completed, total_stories, percentage)
            };

            let _ = event_tx
                .send(IterationEvent::Progress {
                    completed,
                    total,
                    percentage,
                })
                .await;

            let _ = event_tx
                .send(IterationEvent::IterationCompleted {
                    iteration,
                    completed_stories: completed,
                })
                .await;

            // Poll interval
            tokio::time::sleep(tokio::time::Duration::from_secs(
                self.config.iteration.poll_interval_seconds,
            ))
            .await;
        }

        // Complete
        {
            let mut state = self.state.write().await;
            state.complete();
        }

        let result = self
            .generate_result(start_time.elapsed().as_millis() as u64)
            .await;

        let _ = event_tx
            .send(IterationEvent::Completed {
                result: result.clone(),
            })
            .await;

        Ok(result)
    }

    /// Check if loop should terminate
    async fn should_terminate(&self) -> bool {
        let state = self.state.read().await;

        match self.config.iteration.mode {
            IterationMode::UntilComplete => self.all_stories_complete().await,
            IterationMode::MaxIterations(max) => state.iteration_count >= max,
            IterationMode::BatchComplete => self.current_batch_complete().await,
            IterationMode::SingleIteration => state.iteration_count >= 1,
        }
    }

    /// Check if all stories are complete
    async fn all_stories_complete(&self) -> bool {
        let prd = self.prd.read().await;
        prd.stories
            .iter()
            .all(|s| s.status == StoryStatus::Completed)
    }

    /// Check if current batch is complete
    async fn current_batch_complete(&self) -> bool {
        let state = self.state.read().await;
        let current_batch_idx = state.current_batch;

        if current_batch_idx >= self.batches.len() {
            return true;
        }

        let batch = &self.batches[current_batch_idx];
        let prd = self.prd.read().await;

        batch.story_ids.iter().all(|id| {
            prd.get_story(id)
                .map(|s| s.status == StoryStatus::Completed)
                .unwrap_or(true)
        })
    }

    /// Get pending stories in current batch
    async fn get_pending_stories(&self) -> Vec<String> {
        let state = self.state.read().await;
        let current_batch_idx = state.current_batch;

        if current_batch_idx >= self.batches.len() {
            return Vec::new();
        }

        let batch = &self.batches[current_batch_idx];
        let prd = self.prd.read().await;

        batch
            .story_ids
            .iter()
            .filter(|id| {
                prd.get_story(id)
                    .map(|s| s.status == StoryStatus::Pending)
                    .unwrap_or(false)
                    && !state.in_progress_stories.contains(*id)
            })
            .take(self.config.iteration.max_concurrent)
            .cloned()
            .collect()
    }

    /// Advance to next batch
    async fn advance_to_next_batch(&self) -> bool {
        let mut state = self.state.write().await;

        if state.current_batch + 1 >= self.batches.len() {
            return false;
        }

        state.current_batch += 1;
        true
    }

    /// Execute stories in parallel
    async fn execute_stories_parallel(
        &self,
        story_ids: &[String],
        event_tx: mpsc::Sender<IterationEvent>,
    ) -> Result<Vec<(String, StoryExecutionResult)>, IterationLoopError> {
        let tasks: Vec<_> = story_ids
            .iter()
            .map(|id| {
                let story_id = id.clone();
                let event_tx = event_tx.clone();
                let state = Arc::clone(&self.state);
                let prd = Arc::clone(&self.prd);
                let cancellation_token = self.cancellation_token.clone();
                let executor = self.story_executor.clone();
                let project_root = self.config.project_root.clone();
                let timeout_seconds = self.config.iteration.story_timeout_seconds;

                async move {
                    if cancellation_token.is_cancelled() {
                        return (
                            story_id,
                            StoryExecutionResult {
                                success: false,
                                error: Some("Cancelled".to_string()),
                            },
                        );
                    }

                    // Get story details for context
                    let (title, description, acceptance_criteria) = {
                        let prd = prd.read().await;
                        match prd.get_story(&story_id) {
                            Some(s) => (
                                s.title.clone(),
                                s.description.clone(),
                                s.acceptance_criteria
                                    .iter()
                                    .map(|ac| ac.description.clone())
                                    .collect::<Vec<_>>(),
                            ),
                            None => (String::new(), String::new(), Vec::new()),
                        }
                    };

                    // Mark in progress
                    {
                        let mut state = state.write().await;
                        state.mark_in_progress(&story_id);
                    }

                    let _ = event_tx
                        .send(IterationEvent::StoryStarted {
                            story_id: story_id.clone(),
                            title: title.clone(),
                        })
                        .await;

                    // Execute story via the executor callback, or fall back to
                    // simulated success when no executor is configured
                    let result = if let Some(exec_fn) = executor {
                        let context = StoryExecutionContext {
                            story_id: story_id.clone(),
                            story_title: title,
                            story_description: description,
                            acceptance_criteria,
                            project_root,
                        };

                        // Run with timeout and cancellation
                        let execution_future = exec_fn(context);
                        let timeout_duration = Duration::from_secs(timeout_seconds);

                        tokio::select! {
                            _ = cancellation_token.cancelled() => {
                                StoryExecutionResult {
                                    success: false,
                                    error: Some("Cancelled".to_string()),
                                }
                            }
                            result = tokio::time::timeout(timeout_duration, execution_future) => {
                                match result {
                                    Ok(execution_result) => execution_result,
                                    Err(_) => StoryExecutionResult {
                                        success: false,
                                        error: Some(format!(
                                            "Story execution timed out after {} seconds",
                                            timeout_seconds
                                        )),
                                    },
                                }
                            }
                        }
                    } else {
                        // No executor configured -- simulate success (legacy behaviour)
                        StoryExecutionResult {
                            success: true,
                            error: None,
                        }
                    };

                    let _ = event_tx
                        .send(IterationEvent::StoryCompleted {
                            story_id: story_id.clone(),
                            success: result.success,
                        })
                        .await;

                    (story_id, result)
                }
            })
            .collect();

        Ok(join_all(tasks).await)
    }

    /// Run quality gates for a story.
    ///
    /// If a `QualityGateRunnerFn` callback has been set via
    /// `with_quality_gate_runner()`, it is invoked with a
    /// `QualityGateContext` containing the story ID and project root.
    /// Otherwise, falls back to a hardcoded pass for backward compatibility.
    async fn run_quality_gates(&self, story_id: &str) -> QualityGateResult {
        if let Some(ref runner) = self.quality_gate_runner {
            let context = QualityGateContext {
                story_id: story_id.to_string(),
                project_root: self.config.project_root.clone(),
            };
            runner(context).await
        } else {
            // No runner configured -- default to pass (backward compat)
            QualityGateResult::pass()
        }
    }

    /// Check if story can be retried
    async fn can_retry(&self, story_id: &str) -> bool {
        let state = self.state.read().await;
        state.can_retry(story_id, self.config.iteration.max_retries)
    }

    /// Queue a story for retry
    async fn queue_retry(&self, story_id: &str, gate_result: &QualityGateResult) {
        {
            let mut state = self.state.write().await;
            state.queue_retry(story_id, gate_result.error.clone());
            // Remove from in-progress so get_pending_stories() picks it up again
            state.in_progress_stories.retain(|id| id != story_id);
        }

        // Also update PRD to mark story as pending again
        let mut prd = self.prd.write().await;
        if let Some(story) = prd.get_story_mut(story_id) {
            story.status = StoryStatus::Pending;
        }
    }

    /// Mark a story as complete
    async fn mark_story_complete(&self, story_id: &str) {
        {
            let mut state = self.state.write().await;
            state.mark_complete(story_id);
        }

        {
            let mut prd = self.prd.write().await;
            if let Some(story) = prd.get_story_mut(story_id) {
                story.status = StoryStatus::Completed;
            }
        }
    }

    /// Mark a story as failed
    async fn mark_story_failed(&self, story_id: &str, error: Option<String>) {
        {
            let mut state = self.state.write().await;
            state.mark_failed(story_id, error);
        }

        {
            let mut prd = self.prd.write().await;
            if let Some(story) = prd.get_story_mut(story_id) {
                story.status = StoryStatus::Failed;
            }
        }
    }

    /// Save current state to file
    async fn save_state(&self) -> Result<(), IterationLoopError> {
        let state = self.state.read().await;
        let state_path = self.config.project_root.join(".iteration-state.json");

        state
            .to_file(&state_path)
            .map_err(|e| IterationLoopError::IoError(std::io::Error::other(e.to_string())))?;

        // Also save PRD
        let prd = self.prd.read().await;
        prd.to_file(&self.config.prd_path)
            .map_err(|e| IterationLoopError::PrdError(e.to_string()))?;

        Ok(())
    }

    /// Generate final result
    async fn generate_result(&self, duration_ms: u64) -> IterationResult {
        let state = self.state.read().await;
        let prd = self.prd.read().await;

        let completed_stories = state.completed_stories.len();
        let failed_stories = state.failed_stories.len();
        let total_stories = prd.stories.len();

        let success = failed_stories == 0 && completed_stories == total_stories;

        IterationResult {
            success,
            iteration_count: state.iteration_count,
            completed_stories,
            failed_stories,
            total_stories,
            duration_ms,
            error: state.error.clone(),
            quality_gates_passed: if self.config.iteration.run_quality_gates {
                Some(failed_stories == 0)
            } else {
                None
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::iteration::{IterationMode, IterationStatus};
    use crate::models::prd::{AcceptanceCriteria, Story};

    fn create_test_prd() -> Prd {
        let mut prd = Prd::new("Test PRD");

        prd.add_story(Story::new("S001", "Setup"));

        let mut s2 = Story::new("S002", "Auth");
        s2.dependencies = vec!["S001".to_string()];
        prd.add_story(s2);

        let mut s3 = Story::new("S003", "Database");
        s3.dependencies = vec!["S001".to_string()];
        prd.add_story(s3);

        prd
    }

    #[tokio::test]
    async fn test_iteration_loop_creation() {
        let prd = create_test_prd();
        let config = IterationLoopConfig::default();
        let loop_runner = IterationLoop::new(config, prd).unwrap();

        let state = loop_runner.get_state().await;
        assert_eq!(state.status, IterationStatus::Pending);
    }

    #[tokio::test]
    async fn test_get_pending_stories() {
        let prd = create_test_prd();
        let config = IterationLoopConfig::default();
        let loop_runner = IterationLoop::new(config, prd).unwrap();

        let pending = loop_runner.get_pending_stories().await;

        // Only S001 should be pending in batch 1
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0], "S001");
    }

    #[tokio::test]
    async fn test_cancellation() {
        let prd = create_test_prd();
        let config = IterationLoopConfig::default();
        let loop_runner = IterationLoop::new(config, prd).unwrap();

        loop_runner.cancel();
        assert!(loop_runner.cancellation_token.is_cancelled());
    }

    // ========================================================================
    // Story Executor Callback Tests
    // ========================================================================

    #[tokio::test]
    async fn test_executor_callback_is_called_with_correct_context() {
        // Track which story IDs were passed to the executor
        let executed_contexts: Arc<tokio::sync::Mutex<Vec<StoryExecutionContext>>> =
            Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let executed_clone = executed_contexts.clone();

        let executor: StoryExecutorFn = Arc::new(move |ctx: StoryExecutionContext| {
            let captured = executed_clone.clone();
            Box::pin(async move {
                captured.lock().await.push(ctx);
                StoryExecutionResult {
                    success: true,
                    error: None,
                }
            })
        });

        // Create a PRD with a single story (no dependencies) with details
        let mut prd = Prd::new("Test PRD");
        let mut story = Story::new("S001", "Setup Project");
        story.description = "Initialize the project structure".to_string();
        story.acceptance_criteria = vec![
            AcceptanceCriteria {
                id: "AC1".to_string(),
                description: "Project compiles".to_string(),
                met: false,
            },
            AcceptanceCriteria {
                id: "AC2".to_string(),
                description: "Tests pass".to_string(),
                met: false,
            },
        ];
        prd.add_story(story);

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::SingleIteration,
                poll_interval_seconds: 0,
                run_quality_gates: false,
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_story_executor(executor);

        let (tx, mut rx) = mpsc::channel(100);

        // Run the loop (SingleIteration mode completes after one iteration)
        let result = loop_runner.run(tx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.completed_stories, 1);

        // Verify the executor was called with the correct context
        let contexts = executed_contexts.lock().await;
        assert_eq!(contexts.len(), 1);

        let ctx = &contexts[0];
        assert_eq!(ctx.story_id, "S001");
        assert_eq!(ctx.story_title, "Setup Project");
        assert_eq!(ctx.story_description, "Initialize the project structure");
        assert_eq!(ctx.acceptance_criteria.len(), 2);
        assert_eq!(ctx.acceptance_criteria[0], "Project compiles");
        assert_eq!(ctx.acceptance_criteria[1], "Tests pass");
        assert_eq!(ctx.project_root, PathBuf::from("/test/project"));

        // Drain events
        rx.close();
    }

    #[tokio::test]
    async fn test_executor_callback_failure_propagates() {
        let executor: StoryExecutorFn = Arc::new(|_ctx: StoryExecutionContext| {
            Box::pin(async {
                StoryExecutionResult {
                    success: false,
                    error: Some("Agent crashed".to_string()),
                }
            })
        });

        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "Failing Story"));

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::SingleIteration,
                poll_interval_seconds: 0,
                run_quality_gates: false,
                stop_on_failure: true,
                max_retries: 0,
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_story_executor(executor);

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await;

        // Should return an error because stop_on_failure is true
        assert!(result.is_err());
        match result {
            Err(IterationLoopError::StoryExecutionError(msg)) => {
                assert_eq!(msg, "Agent crashed");
            }
            other => panic!("Expected StoryExecutionError, got {:?}", other),
        }

        rx.close();
    }

    #[tokio::test]
    async fn test_executor_timeout_returns_failure() {
        let executor: StoryExecutorFn = Arc::new(|_ctx: StoryExecutionContext| {
            Box::pin(async {
                // Sleep longer than the timeout
                tokio::time::sleep(Duration::from_secs(10)).await;
                StoryExecutionResult {
                    success: true,
                    error: None,
                }
            })
        });

        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "Slow Story"));

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::SingleIteration,
                poll_interval_seconds: 0,
                run_quality_gates: false,
                stop_on_failure: false,
                max_retries: 0,
                // Very short timeout to trigger timeout quickly
                story_timeout_seconds: 1,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_story_executor(executor);

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await.unwrap();

        // Story should have failed due to timeout
        assert_eq!(result.failed_stories, 1);
        assert_eq!(result.completed_stories, 0);

        rx.close();
    }

    #[tokio::test]
    async fn test_executor_cancellation_stops_in_flight() {
        let executor: StoryExecutorFn = Arc::new(|_ctx: StoryExecutionContext| {
            Box::pin(async {
                // Simulate a long-running execution
                tokio::time::sleep(Duration::from_secs(30)).await;
                StoryExecutionResult {
                    success: true,
                    error: None,
                }
            })
        });

        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "Long Running Story"));

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::UntilComplete,
                poll_interval_seconds: 0,
                run_quality_gates: false,
                story_timeout_seconds: 300,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_story_executor(executor);

        let cancel_token = loop_runner.cancellation_token();
        let (tx, mut rx) = mpsc::channel(100);

        // Cancel after a short delay
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel_token.cancel();
        });

        let result = loop_runner.run(tx).await;

        // Should return Cancelled error
        assert!(matches!(result, Err(IterationLoopError::Cancelled)));

        rx.close();
    }

    #[tokio::test]
    async fn test_without_executor_uses_simulated_success() {
        // No executor set -- should use the legacy simulated success path
        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "Simple Story"));

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::SingleIteration,
                poll_interval_seconds: 0,
                run_quality_gates: false,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd).unwrap();

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.completed_stories, 1);

        rx.close();
    }

    #[tokio::test]
    async fn test_executor_called_for_each_story_in_batch() {
        // Track all story IDs the executor receives
        let executed_ids: Arc<tokio::sync::Mutex<Vec<String>>> =
            Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let ids_clone = executed_ids.clone();

        let executor: StoryExecutorFn = Arc::new(move |ctx: StoryExecutionContext| {
            let ids = ids_clone.clone();
            Box::pin(async move {
                ids.lock().await.push(ctx.story_id);
                StoryExecutionResult {
                    success: true,
                    error: None,
                }
            })
        });

        // Create PRD with 3 stories: S001 (no deps), S002 & S003 depend on S001
        let prd = create_test_prd();

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::UntilComplete,
                poll_interval_seconds: 0,
                run_quality_gates: false,
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_story_executor(executor);

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.completed_stories, 3);

        // All 3 stories should have been executed
        let ids = executed_ids.lock().await;
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"S001".to_string()));
        assert!(ids.contains(&"S002".to_string()));
        assert!(ids.contains(&"S003".to_string()));

        rx.close();
    }

    #[tokio::test]
    async fn test_from_state_with_executor() {
        let executed: Arc<tokio::sync::Mutex<bool>> = Arc::new(tokio::sync::Mutex::new(false));
        let executed_clone = executed.clone();

        let executor: StoryExecutorFn = Arc::new(move |_ctx: StoryExecutionContext| {
            let flag = executed_clone.clone();
            Box::pin(async move {
                *flag.lock().await = true;
                StoryExecutionResult {
                    success: true,
                    error: None,
                }
            })
        });

        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "Story"));

        let state = IterationState::new();

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::SingleIteration,
                poll_interval_seconds: 0,
                run_quality_gates: false,
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::from_state(config, prd, state)
            .unwrap()
            .with_story_executor(executor);

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await.unwrap();

        assert!(result.success);
        assert!(*executed.lock().await);

        rx.close();
    }

    // ========================================================================
    // Quality Gate Runner Callback Tests
    // ========================================================================

    #[tokio::test]
    async fn test_quality_gate_runner_is_called_after_story_execution() {
        // Track which story IDs were passed to the gate runner
        let gate_contexts: Arc<tokio::sync::Mutex<Vec<QualityGateContext>>> =
            Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let gate_clone = gate_contexts.clone();

        let gate_runner: QualityGateRunnerFn = Arc::new(move |ctx: QualityGateContext| {
            let captured = gate_clone.clone();
            Box::pin(async move {
                captured.lock().await.push(ctx);
                QualityGateResult::pass()
            })
        });

        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "Story With Gates"));

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::SingleIteration,
                poll_interval_seconds: 0,
                run_quality_gates: true,
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_quality_gate_runner(gate_runner);

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.completed_stories, 1);

        // Verify the gate runner was called with correct context
        let contexts = gate_contexts.lock().await;
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].story_id, "S001");
        assert_eq!(contexts[0].project_root, PathBuf::from("/test/project"));

        rx.close();
    }

    #[tokio::test]
    async fn test_quality_gate_runner_receives_correct_parameters() {
        // Verify that both story executor and gate runner receive correct params
        let executed_story_ids: Arc<tokio::sync::Mutex<Vec<String>>> =
            Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let ids_clone = executed_story_ids.clone();

        let executor: StoryExecutorFn = Arc::new(move |ctx: StoryExecutionContext| {
            let ids = ids_clone.clone();
            Box::pin(async move {
                ids.lock().await.push(ctx.story_id);
                StoryExecutionResult {
                    success: true,
                    error: None,
                }
            })
        });

        let gate_contexts: Arc<tokio::sync::Mutex<Vec<QualityGateContext>>> =
            Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let gate_clone = gate_contexts.clone();

        let gate_runner: QualityGateRunnerFn = Arc::new(move |ctx: QualityGateContext| {
            let captured = gate_clone.clone();
            Box::pin(async move {
                captured.lock().await.push(ctx);
                QualityGateResult::pass()
            })
        });

        let prd = create_test_prd(); // S001, S002 (dep S001), S003 (dep S001)

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::UntilComplete,
                poll_interval_seconds: 0,
                run_quality_gates: true,
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/my/project"),
            prd_path: PathBuf::from("/my/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_story_executor(executor)
            .with_quality_gate_runner(gate_runner);

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.completed_stories, 3);

        // Gate runner should have been called for each story
        let gate_ctxs = gate_contexts.lock().await;
        assert_eq!(gate_ctxs.len(), 3);

        // All should have the correct project root
        for ctx in gate_ctxs.iter() {
            assert_eq!(ctx.project_root, PathBuf::from("/my/project"));
        }

        // All story IDs should be present
        let gate_story_ids: Vec<&str> = gate_ctxs.iter().map(|c| c.story_id.as_str()).collect();
        assert!(gate_story_ids.contains(&"S001"));
        assert!(gate_story_ids.contains(&"S002"));
        assert!(gate_story_ids.contains(&"S003"));

        rx.close();
    }

    #[tokio::test]
    async fn test_quality_gate_failure_produces_error_message() {
        let gate_runner: QualityGateRunnerFn = Arc::new(|_ctx: QualityGateContext| {
            Box::pin(async {
                QualityGateResult::fail(
                    "TypeCheck failed: 3 errors found".to_string(),
                    vec![
                        "src/main.rs:10 - type mismatch".to_string(),
                        "src/lib.rs:20 - missing field".to_string(),
                    ],
                )
            })
        });

        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "Failing Gate Story"));

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::SingleIteration,
                poll_interval_seconds: 0,
                run_quality_gates: true,
                stop_on_failure: true,
                max_retries: 0,
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_quality_gate_runner(gate_runner);

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await;

        // Should fail because quality gates failed with stop_on_failure
        assert!(result.is_err());
        match result {
            Err(IterationLoopError::QualityGateError(msg)) => {
                assert_eq!(msg, "Quality gates failed");
            }
            other => panic!("Expected QualityGateError, got {:?}", other),
        }

        rx.close();
    }

    #[tokio::test]
    async fn test_quality_gate_failure_triggers_retry() {
        let call_count: Arc<tokio::sync::Mutex<u32>> = Arc::new(tokio::sync::Mutex::new(0));
        let count_clone = call_count.clone();

        let gate_runner: QualityGateRunnerFn = Arc::new(move |_ctx: QualityGateContext| {
            let count = count_clone.clone();
            Box::pin(async move {
                let mut c = count.lock().await;
                *c += 1;
                if *c <= 1 {
                    // First call: fail to trigger retry
                    QualityGateResult::fail(
                        "Lint errors found".to_string(),
                        vec!["warning: unused variable".to_string()],
                    )
                } else {
                    // Second call: pass
                    QualityGateResult::pass()
                }
            })
        });

        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "Retry Story"));

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::UntilComplete,
                poll_interval_seconds: 0,
                run_quality_gates: true,
                stop_on_failure: false,
                max_retries: 3,
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_quality_gate_runner(gate_runner);

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.completed_stories, 1);
        // Gate runner called at least twice (once failing, once passing)
        assert!(*call_count.lock().await >= 2);

        rx.close();
    }

    #[tokio::test]
    async fn test_no_quality_gate_runner_defaults_to_pass() {
        // When run_quality_gates is true but no runner is set,
        // gates should default to pass (backward compatibility)
        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "No Runner Story"));

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::SingleIteration,
                poll_interval_seconds: 0,
                run_quality_gates: true,
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd).unwrap();

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await.unwrap();

        assert!(result.success);
        assert_eq!(result.completed_stories, 1);
        assert_eq!(result.quality_gates_passed, Some(true));

        rx.close();
    }

    #[tokio::test]
    async fn test_quality_gate_not_called_when_gates_disabled() {
        // When run_quality_gates is false, gate runner should NOT be called
        let call_count: Arc<tokio::sync::Mutex<u32>> = Arc::new(tokio::sync::Mutex::new(0));
        let count_clone = call_count.clone();

        let gate_runner: QualityGateRunnerFn = Arc::new(move |_ctx: QualityGateContext| {
            let count = count_clone.clone();
            Box::pin(async move {
                *count.lock().await += 1;
                QualityGateResult::pass()
            })
        });

        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "Gates Disabled"));

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::SingleIteration,
                poll_interval_seconds: 0,
                run_quality_gates: false, // Gates disabled
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_quality_gate_runner(gate_runner);

        let (tx, mut rx) = mpsc::channel(100);
        let result = loop_runner.run(tx).await.unwrap();

        assert!(result.success);
        // Gate runner should not have been called at all
        assert_eq!(*call_count.lock().await, 0);
        // quality_gates_passed should be None when gates are disabled
        assert_eq!(result.quality_gates_passed, None);

        rx.close();
    }

    #[tokio::test]
    async fn test_quality_gate_events_emitted() {
        let gate_runner: QualityGateRunnerFn =
            Arc::new(|_ctx: QualityGateContext| Box::pin(async { QualityGateResult::pass() }));

        let mut prd = Prd::new("Test PRD");
        prd.add_story(Story::new("S001", "Gate Events Story"));

        let config = IterationLoopConfig {
            iteration: IterationConfig {
                mode: IterationMode::SingleIteration,
                poll_interval_seconds: 0,
                run_quality_gates: true,
                story_timeout_seconds: 60,
                ..Default::default()
            },
            project_root: PathBuf::from("/test/project"),
            prd_path: PathBuf::from("/test/prd.json"),
            persist_state: false,
        };

        let loop_runner = IterationLoop::new(config, prd)
            .unwrap()
            .with_quality_gate_runner(gate_runner);

        let (tx, mut rx) = mpsc::channel(100);
        let _result = loop_runner.run(tx).await.unwrap();

        // Collect all events
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        // Should have QualityGatesStarted and QualityGatesCompleted events
        let gate_started = events.iter().any(
            |e| matches!(e, IterationEvent::QualityGatesStarted { story_id } if story_id == "S001"),
        );
        let gate_completed = events.iter().any(|e| matches!(e, IterationEvent::QualityGatesCompleted { story_id, passed } if story_id == "S001" && *passed));

        assert!(gate_started, "Expected QualityGatesStarted event for S001");
        assert!(
            gate_completed,
            "Expected QualityGatesCompleted(passed=true) event for S001"
        );

        rx.close();
    }

    #[tokio::test]
    async fn test_quality_gate_result_constructors() {
        let pass = QualityGateResult::pass();
        assert!(pass.passed);
        assert!(pass.error.is_none());
        assert!(pass.details.is_empty());

        let fail = QualityGateResult::fail(
            "compilation error".to_string(),
            vec!["line 10: syntax error".to_string()],
        );
        assert!(!fail.passed);
        assert_eq!(fail.error, Some("compilation error".to_string()));
        assert_eq!(fail.details.len(), 1);
        assert_eq!(fail.details[0], "line 10: syntax error");
    }
}
