//! Iteration Loop Runner
//!
//! Implements the auto-iteration system for executing PRD stories with
//! quality gates integration and retry logic.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use futures_util::future::join_all;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

use crate::models::iteration::{
    IterationConfig, IterationMode, IterationResult, IterationState,
};
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
    BatchStarted { batch_index: usize, story_count: usize },
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
    IterationCompleted { iteration: u32, completed_stories: usize },
    /// Loop completed
    Completed { result: IterationResult },
    /// Progress update
    Progress { completed: usize, total: usize, percentage: f32 },
    /// Error occurred
    Error { message: String },
}

/// Quality gate result (placeholder - integrate with actual QualityGateRunner)
#[derive(Debug, Clone)]
pub struct QualityGateResult {
    pub passed: bool,
    pub error: Option<String>,
}

/// Execution result for a story
#[derive(Debug, Clone)]
pub struct StoryExecutionResult {
    pub success: bool,
    pub error: Option<String>,
}

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
        })
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

        let _ = event_tx.send(IterationEvent::Started { total_stories }).await;

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
            let results = self.execute_stories_parallel(&pending, event_tx.clone()).await?;

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
                            &QualityGateResult {
                                passed: false,
                                error: Some(error.clone()),
                            },
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

        let result = self.generate_result(start_time.elapsed().as_millis() as u64).await;

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

                    // Get story title
                    let title = {
                        let prd = prd.read().await;
                        prd.get_story(&story_id)
                            .map(|s| s.title.clone())
                            .unwrap_or_default()
                    };

                    // Mark in progress
                    {
                        let mut state = state.write().await;
                        state.mark_in_progress(&story_id);
                    }

                    let _ = event_tx
                        .send(IterationEvent::StoryStarted {
                            story_id: story_id.clone(),
                            title,
                        })
                        .await;

                    // Execute story
                    // In a real implementation, this would call the agent executor
                    // For now, simulate success
                    let result = StoryExecutionResult {
                        success: true,
                        error: None,
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

    /// Run quality gates for a story
    async fn run_quality_gates(&self, _story_id: &str) -> QualityGateResult {
        // In a real implementation, this would call QualityGateRunner
        // For now, simulate success
        QualityGateResult {
            passed: true,
            error: None,
        }
    }

    /// Check if story can be retried
    async fn can_retry(&self, story_id: &str) -> bool {
        let state = self.state.read().await;
        state.can_retry(story_id, self.config.iteration.max_retries)
    }

    /// Queue a story for retry
    async fn queue_retry(&self, story_id: &str, gate_result: &QualityGateResult) {
        let mut state = self.state.write().await;
        state.queue_retry(story_id, gate_result.error.clone());

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
    use crate::models::prd::Story;
    use crate::models::iteration::IterationStatus;

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
}
