//! Batch Parallel Execution Engine
//!
//! Calculates execution batches via topological sort (Kahn's algorithm),
//! launches parallel agents within each batch, runs quality gate pipeline
//! per story, and retries failed stories with different agents.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::services::quality_gates::pipeline::{PipelineGateResult, PipelineResult};
use crate::services::task_mode::agent_resolver::{AgentAssignment, ExecutionPhase};
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
}
