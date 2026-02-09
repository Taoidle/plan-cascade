//! Iteration Models
//!
//! Data structures for the auto-iteration system with quality gates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Iteration mode determining when to stop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IterationMode {
    /// Run until all stories are complete
    #[default]
    UntilComplete,
    /// Run for a maximum number of iterations
    MaxIterations(u32),
    /// Run until current batch is complete
    BatchComplete,
    /// Run a single iteration
    SingleIteration,
}

/// Configuration for the iteration loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationConfig {
    /// Iteration mode
    #[serde(default)]
    pub mode: IterationMode,
    /// Maximum retries per story
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Poll interval between iterations in seconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
    /// Whether to run quality gates after each story
    #[serde(default = "default_run_quality_gates")]
    pub run_quality_gates: bool,
    /// Whether to stop on first failure
    #[serde(default)]
    pub stop_on_failure: bool,
    /// Maximum concurrent story executions
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    /// Timeout per story in seconds
    #[serde(default = "default_story_timeout")]
    pub story_timeout_seconds: u64,
}

fn default_max_retries() -> u32 {
    3
}

fn default_poll_interval() -> u64 {
    5
}

fn default_run_quality_gates() -> bool {
    true
}

fn default_max_concurrent() -> usize {
    3
}

fn default_story_timeout() -> u64 {
    300 // 5 minutes
}

impl Default for IterationConfig {
    fn default() -> Self {
        Self {
            mode: IterationMode::default(),
            max_retries: default_max_retries(),
            poll_interval_seconds: default_poll_interval(),
            run_quality_gates: default_run_quality_gates(),
            stop_on_failure: false,
            max_concurrent: default_max_concurrent(),
            story_timeout_seconds: default_story_timeout(),
        }
    }
}

/// Status of the iteration loop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IterationStatus {
    #[default]
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for IterationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IterationStatus::Pending => write!(f, "pending"),
            IterationStatus::Running => write!(f, "running"),
            IterationStatus::Paused => write!(f, "paused"),
            IterationStatus::Completed => write!(f, "completed"),
            IterationStatus::Failed => write!(f, "failed"),
            IterationStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Entry in the retry queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryEntry {
    /// Story ID to retry
    pub story_id: String,
    /// Context from the previous failure
    pub failure_context: Option<String>,
    /// Current retry number
    pub retry_number: u32,
    /// Timestamp when queued
    pub queued_at: String,
}

impl RetryEntry {
    /// Create a new retry entry
    pub fn new(
        story_id: impl Into<String>,
        failure_context: Option<String>,
        retry_number: u32,
    ) -> Self {
        Self {
            story_id: story_id.into(),
            failure_context,
            retry_number,
            queued_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// State of the iteration loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationState {
    /// Current status
    pub status: IterationStatus,
    /// Number of iterations completed
    pub iteration_count: u32,
    /// Current batch index
    pub current_batch: usize,
    /// Retry counts per story ID
    pub retry_counts: HashMap<String, u32>,
    /// Queue of stories to retry
    pub retry_queue: Vec<RetryEntry>,
    /// Completed story IDs
    pub completed_stories: Vec<String>,
    /// Failed story IDs
    pub failed_stories: Vec<String>,
    /// In-progress story IDs
    pub in_progress_stories: Vec<String>,
    /// Started timestamp
    pub started_at: Option<String>,
    /// Last update timestamp
    pub updated_at: Option<String>,
    /// Completed timestamp
    pub completed_at: Option<String>,
    /// Error if failed
    pub error: Option<String>,
}

impl Default for IterationState {
    fn default() -> Self {
        Self {
            status: IterationStatus::Pending,
            iteration_count: 0,
            current_batch: 0,
            retry_counts: HashMap::new(),
            retry_queue: Vec::new(),
            completed_stories: Vec::new(),
            failed_stories: Vec::new(),
            in_progress_stories: Vec::new(),
            started_at: None,
            updated_at: None,
            completed_at: None,
            error: None,
        }
    }
}

impl IterationState {
    /// Create a new iteration state
    pub fn new() -> Self {
        Self::default()
    }

    /// Start iteration
    pub fn start(&mut self) {
        self.status = IterationStatus::Running;
        self.started_at = Some(chrono::Utc::now().to_rfc3339());
        self.updated_at = self.started_at.clone();
    }

    /// Pause iteration
    pub fn pause(&mut self) {
        self.status = IterationStatus::Paused;
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Resume iteration
    pub fn resume(&mut self) {
        self.status = IterationStatus::Running;
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Complete iteration
    pub fn complete(&mut self) {
        self.status = IterationStatus::Completed;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.updated_at = self.completed_at.clone();
    }

    /// Fail iteration
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = IterationStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.updated_at = self.completed_at.clone();
    }

    /// Cancel iteration
    pub fn cancel(&mut self) {
        self.status = IterationStatus::Cancelled;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.updated_at = self.completed_at.clone();
    }

    /// Mark a story as in progress
    pub fn mark_in_progress(&mut self, story_id: &str) {
        if !self.in_progress_stories.contains(&story_id.to_string()) {
            self.in_progress_stories.push(story_id.to_string());
        }
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark a story as complete
    pub fn mark_complete(&mut self, story_id: &str) {
        self.in_progress_stories.retain(|id| id != story_id);
        if !self.completed_stories.contains(&story_id.to_string()) {
            self.completed_stories.push(story_id.to_string());
        }
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark a story as failed
    pub fn mark_failed(&mut self, story_id: &str, error: Option<String>) {
        self.in_progress_stories.retain(|id| id != story_id);
        if !self.failed_stories.contains(&story_id.to_string()) {
            self.failed_stories.push(story_id.to_string());
        }
        if let Some(err) = error {
            self.error = Some(err);
        }
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Queue a story for retry
    pub fn queue_retry(&mut self, story_id: &str, failure_context: Option<String>) {
        let retry_count = self.retry_counts.entry(story_id.to_string()).or_insert(0);
        *retry_count += 1;

        self.retry_queue
            .push(RetryEntry::new(story_id, failure_context, *retry_count));
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Get retry count for a story
    pub fn get_retry_count(&self, story_id: &str) -> u32 {
        *self.retry_counts.get(story_id).unwrap_or(&0)
    }

    /// Check if a story can be retried
    pub fn can_retry(&self, story_id: &str, max_retries: u32) -> bool {
        self.get_retry_count(story_id) < max_retries
    }

    /// Save to file
    pub fn to_file(&self, path: &std::path::Path) -> Result<(), IterationError> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| IterationError::SerializeError(e.to_string()))?;
        std::fs::write(path, content).map_err(|e| IterationError::IoError(e.to_string()))
    }

    /// Load from file
    pub fn from_file(path: &std::path::Path) -> Result<Self, IterationError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| IterationError::IoError(e.to_string()))?;
        serde_json::from_str(&content).map_err(|e| IterationError::ParseError(e.to_string()))
    }
}

/// Result of an iteration loop execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationResult {
    /// Whether all stories completed successfully
    pub success: bool,
    /// Total iterations performed
    pub iteration_count: u32,
    /// Number of completed stories
    pub completed_stories: usize,
    /// Number of failed stories
    pub failed_stories: usize,
    /// Total stories
    pub total_stories: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error message if failed
    pub error: Option<String>,
    /// Quality gate results summary
    pub quality_gates_passed: Option<bool>,
}

impl IterationResult {
    /// Create a successful result
    pub fn success(
        iteration_count: u32,
        completed_stories: usize,
        total_stories: usize,
        duration_ms: u64,
    ) -> Self {
        Self {
            success: true,
            iteration_count,
            completed_stories,
            failed_stories: 0,
            total_stories,
            duration_ms,
            error: None,
            quality_gates_passed: Some(true),
        }
    }

    /// Create a failed result
    pub fn failure(
        iteration_count: u32,
        completed_stories: usize,
        failed_stories: usize,
        total_stories: usize,
        duration_ms: u64,
        error: impl Into<String>,
    ) -> Self {
        Self {
            success: false,
            iteration_count,
            completed_stories,
            failed_stories,
            total_stories,
            duration_ms,
            error: Some(error.into()),
            quality_gates_passed: None,
        }
    }
}

/// Errors for iteration operations
#[derive(Debug, thiserror::Error)]
pub enum IterationError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Serialize error: {0}")]
    SerializeError(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("Cancelled")]
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iteration_config_default() {
        let config = IterationConfig::default();
        assert_eq!(config.max_retries, 3);
        assert!(config.run_quality_gates);
    }

    #[test]
    fn test_iteration_state_transitions() {
        let mut state = IterationState::new();
        assert_eq!(state.status, IterationStatus::Pending);

        state.start();
        assert_eq!(state.status, IterationStatus::Running);

        state.pause();
        assert_eq!(state.status, IterationStatus::Paused);

        state.resume();
        assert_eq!(state.status, IterationStatus::Running);

        state.complete();
        assert_eq!(state.status, IterationStatus::Completed);
    }

    #[test]
    fn test_story_tracking() {
        let mut state = IterationState::new();

        state.mark_in_progress("S001");
        assert!(state.in_progress_stories.contains(&"S001".to_string()));

        state.mark_complete("S001");
        assert!(!state.in_progress_stories.contains(&"S001".to_string()));
        assert!(state.completed_stories.contains(&"S001".to_string()));
    }

    #[test]
    fn test_retry_queue() {
        let mut state = IterationState::new();

        assert!(state.can_retry("S001", 3));

        state.queue_retry("S001", Some("Test failure".to_string()));
        assert_eq!(state.get_retry_count("S001"), 1);

        state.queue_retry("S001", None);
        assert_eq!(state.get_retry_count("S001"), 2);

        state.queue_retry("S001", None);
        assert!(!state.can_retry("S001", 3));
    }
}
