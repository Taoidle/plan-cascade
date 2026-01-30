//! Orchestrator Models
//!
//! Data structures for standalone LLM execution session management,
//! including session state persistence and progress tracking.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Execution session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Session is pending execution
    Pending,
    /// Session is currently running
    Running,
    /// Session completed successfully
    Completed,
    /// Session was cancelled by user
    Cancelled,
    /// Session failed with an error
    Failed,
    /// Session is paused and can be resumed
    Paused,
}

impl ExecutionStatus {
    /// Check if this status indicates the session can be resumed
    pub fn can_resume(&self) -> bool {
        matches!(self, ExecutionStatus::Paused | ExecutionStatus::Failed)
    }

    /// Check if this status indicates the session is terminal
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ExecutionStatus::Completed | ExecutionStatus::Cancelled
        )
    }

    /// Check if this status indicates the session is active
    pub fn is_active(&self) -> bool {
        matches!(self, ExecutionStatus::Running | ExecutionStatus::Pending)
    }
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Pending => write!(f, "pending"),
            ExecutionStatus::Running => write!(f, "running"),
            ExecutionStatus::Completed => write!(f, "completed"),
            ExecutionStatus::Cancelled => write!(f, "cancelled"),
            ExecutionStatus::Failed => write!(f, "failed"),
            ExecutionStatus::Paused => write!(f, "paused"),
        }
    }
}

impl std::str::FromStr for ExecutionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(ExecutionStatus::Pending),
            "running" => Ok(ExecutionStatus::Running),
            "completed" => Ok(ExecutionStatus::Completed),
            "cancelled" => Ok(ExecutionStatus::Cancelled),
            "failed" => Ok(ExecutionStatus::Failed),
            "paused" => Ok(ExecutionStatus::Paused),
            _ => Err(format!("Unknown execution status: {}", s)),
        }
    }
}

/// Story execution state for tracking individual story progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryExecutionState {
    /// Story ID from PRD
    pub story_id: String,
    /// Story title
    pub title: String,
    /// Execution status
    pub status: ExecutionStatus,
    /// Start time (Unix timestamp)
    pub started_at: Option<i64>,
    /// Completion time (Unix timestamp)
    pub completed_at: Option<i64>,
    /// Error message if failed
    pub error: Option<String>,
    /// Number of iterations taken
    pub iterations: u32,
    /// Tokens used for this story
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// Quality gate results (gate_id -> passed)
    pub quality_gates: HashMap<String, bool>,
}

impl StoryExecutionState {
    /// Create a new pending story state
    pub fn new(story_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            story_id: story_id.into(),
            title: title.into(),
            status: ExecutionStatus::Pending,
            started_at: None,
            completed_at: None,
            error: None,
            iterations: 0,
            input_tokens: 0,
            output_tokens: 0,
            quality_gates: HashMap::new(),
        }
    }

    /// Mark as running
    pub fn start(&mut self) {
        self.status = ExecutionStatus::Running;
        self.started_at = Some(chrono::Utc::now().timestamp());
    }

    /// Mark as completed
    pub fn complete(&mut self) {
        self.status = ExecutionStatus::Completed;
        self.completed_at = Some(chrono::Utc::now().timestamp());
    }

    /// Mark as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = ExecutionStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(chrono::Utc::now().timestamp());
    }

    /// Duration in milliseconds (if completed)
    pub fn duration_ms(&self) -> Option<u64> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some(((end - start) * 1000) as u64),
            _ => None,
        }
    }
}

/// Execution session for PRD-based standalone execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSession {
    /// Unique session ID
    pub id: String,
    /// Project path
    pub project_path: String,
    /// PRD file path (if executing from PRD)
    pub prd_path: Option<String>,
    /// Overall session status
    pub status: ExecutionStatus,
    /// LLM provider used
    pub provider: String,
    /// Model used
    pub model: String,
    /// System prompt
    pub system_prompt: Option<String>,
    /// Stories to execute
    pub stories: Vec<StoryExecutionState>,
    /// Current story index being executed
    pub current_story_index: usize,
    /// Total input tokens used
    pub total_input_tokens: u32,
    /// Total output tokens used
    pub total_output_tokens: u32,
    /// Session creation time
    pub created_at: i64,
    /// Last update time
    pub updated_at: i64,
    /// Session start time (when execution began)
    pub started_at: Option<i64>,
    /// Session completion time
    pub completed_at: Option<i64>,
    /// Error message if session failed
    pub error: Option<String>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

impl ExecutionSession {
    /// Create a new execution session
    pub fn new(
        id: impl Into<String>,
        project_path: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: id.into(),
            project_path: project_path.into(),
            prd_path: None,
            status: ExecutionStatus::Pending,
            provider: provider.into(),
            model: model.into(),
            system_prompt: None,
            stories: Vec::new(),
            current_story_index: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            error: None,
            metadata: HashMap::new(),
        }
    }

    /// Set PRD path
    pub fn with_prd(mut self, prd_path: impl Into<String>) -> Self {
        self.prd_path = Some(prd_path.into());
        self
    }

    /// Set system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Add a story to execute
    pub fn add_story(&mut self, story_id: impl Into<String>, title: impl Into<String>) {
        self.stories.push(StoryExecutionState::new(story_id, title));
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Get the current story being executed
    pub fn current_story(&self) -> Option<&StoryExecutionState> {
        self.stories.get(self.current_story_index)
    }

    /// Get mutable reference to current story
    pub fn current_story_mut(&mut self) -> Option<&mut StoryExecutionState> {
        self.stories.get_mut(self.current_story_index)
    }

    /// Move to the next story
    pub fn advance_to_next_story(&mut self) -> bool {
        if self.current_story_index + 1 < self.stories.len() {
            self.current_story_index += 1;
            self.updated_at = chrono::Utc::now().timestamp();
            true
        } else {
            false
        }
    }

    /// Start the session execution
    pub fn start(&mut self) {
        self.status = ExecutionStatus::Running;
        self.started_at = Some(chrono::Utc::now().timestamp());
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Complete the session
    pub fn complete(&mut self) {
        self.status = ExecutionStatus::Completed;
        self.completed_at = Some(chrono::Utc::now().timestamp());
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Cancel the session
    pub fn cancel(&mut self) {
        self.status = ExecutionStatus::Cancelled;
        self.completed_at = Some(chrono::Utc::now().timestamp());
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Pause the session (for later resume)
    pub fn pause(&mut self) {
        self.status = ExecutionStatus::Paused;
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Fail the session with an error
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = ExecutionStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(chrono::Utc::now().timestamp());
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Update token counts
    pub fn add_tokens(&mut self, input: u32, output: u32) {
        self.total_input_tokens += input;
        self.total_output_tokens += output;
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Calculate progress percentage
    pub fn progress_percentage(&self) -> f32 {
        if self.stories.is_empty() {
            return 0.0;
        }

        let completed = self
            .stories
            .iter()
            .filter(|s| s.status == ExecutionStatus::Completed)
            .count();

        (completed as f32 / self.stories.len() as f32) * 100.0
    }

    /// Get completed story count
    pub fn completed_stories(&self) -> usize {
        self.stories
            .iter()
            .filter(|s| s.status == ExecutionStatus::Completed)
            .count()
    }

    /// Get failed story count
    pub fn failed_stories(&self) -> usize {
        self.stories
            .iter()
            .filter(|s| s.status == ExecutionStatus::Failed)
            .count()
    }

    /// Total duration in milliseconds
    pub fn duration_ms(&self) -> Option<u64> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some(((end - start) * 1000) as u64),
            (Some(start), None) => {
                let now = chrono::Utc::now().timestamp();
                Some(((now - start) * 1000) as u64)
            }
            _ => None,
        }
    }
}

/// Progress update event for real-time tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionProgress {
    /// Session ID
    pub session_id: String,
    /// Current story index
    pub current_story: usize,
    /// Total stories
    pub total_stories: usize,
    /// Current story ID
    pub story_id: Option<String>,
    /// Current story title
    pub story_title: Option<String>,
    /// Overall progress percentage (0-100)
    pub percentage: f32,
    /// Session status
    pub status: ExecutionStatus,
    /// Total input tokens used
    pub total_input_tokens: u32,
    /// Total output tokens used
    pub total_output_tokens: u32,
    /// Current iteration for the current story
    pub current_iteration: u32,
    /// Estimated time remaining in seconds
    pub estimated_remaining_secs: Option<u64>,
}

impl ExecutionProgress {
    /// Create from an execution session
    pub fn from_session(session: &ExecutionSession) -> Self {
        let current_story = session.current_story();
        Self {
            session_id: session.id.clone(),
            current_story: session.current_story_index,
            total_stories: session.stories.len(),
            story_id: current_story.map(|s| s.story_id.clone()),
            story_title: current_story.map(|s| s.title.clone()),
            percentage: session.progress_percentage(),
            status: session.status,
            total_input_tokens: session.total_input_tokens,
            total_output_tokens: session.total_output_tokens,
            current_iteration: current_story.map(|s| s.iterations).unwrap_or(0),
            estimated_remaining_secs: None, // Calculated by the service
        }
    }
}

/// Status response for standalone execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandaloneStatus {
    /// Active sessions
    pub active_sessions: Vec<ExecutionSessionSummary>,
    /// Recent completed sessions
    pub recent_sessions: Vec<ExecutionSessionSummary>,
    /// Total sessions count
    pub total_sessions: usize,
}

/// Summary of an execution session (lightweight version)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSessionSummary {
    /// Session ID
    pub id: String,
    /// Project path
    pub project_path: String,
    /// Session status
    pub status: ExecutionStatus,
    /// Progress percentage
    pub progress_percentage: f32,
    /// Completed stories
    pub completed_stories: usize,
    /// Total stories
    pub total_stories: usize,
    /// Provider name
    pub provider: String,
    /// Model name
    pub model: String,
    /// Created timestamp
    pub created_at: i64,
    /// Last updated timestamp
    pub updated_at: i64,
}

impl From<&ExecutionSession> for ExecutionSessionSummary {
    fn from(session: &ExecutionSession) -> Self {
        Self {
            id: session.id.clone(),
            project_path: session.project_path.clone(),
            status: session.status,
            progress_percentage: session.progress_percentage(),
            completed_stories: session.completed_stories(),
            total_stories: session.stories.len(),
            provider: session.provider.clone(),
            model: session.model.clone(),
            created_at: session.created_at,
            updated_at: session.updated_at,
        }
    }
}

/// Request to start a session-based execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteWithSessionRequest {
    /// Project path
    pub project_path: String,
    /// PRD file path (optional, will use default if not provided)
    pub prd_path: Option<String>,
    /// LLM provider
    pub provider: String,
    /// Model name
    pub model: String,
    /// System prompt override
    pub system_prompt: Option<String>,
    /// Whether to run quality gates after each story
    pub run_quality_gates: bool,
    /// Specific story IDs to execute (all if empty)
    pub story_ids: Option<Vec<String>>,
}

/// Request to resume a paused/failed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeExecutionRequest {
    /// Session ID to resume
    pub session_id: String,
    /// Whether to retry failed stories
    pub retry_failed: bool,
    /// Whether to skip the current story
    pub skip_current: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_status() {
        assert!(ExecutionStatus::Paused.can_resume());
        assert!(ExecutionStatus::Failed.can_resume());
        assert!(!ExecutionStatus::Running.can_resume());
        assert!(!ExecutionStatus::Completed.can_resume());

        assert!(ExecutionStatus::Completed.is_terminal());
        assert!(ExecutionStatus::Cancelled.is_terminal());
        assert!(!ExecutionStatus::Running.is_terminal());

        assert!(ExecutionStatus::Running.is_active());
        assert!(ExecutionStatus::Pending.is_active());
        assert!(!ExecutionStatus::Completed.is_active());
    }

    #[test]
    fn test_story_execution_state() {
        let mut story = StoryExecutionState::new("story-001", "Test Story");
        assert_eq!(story.status, ExecutionStatus::Pending);
        assert!(story.started_at.is_none());

        story.start();
        assert_eq!(story.status, ExecutionStatus::Running);
        assert!(story.started_at.is_some());

        story.complete();
        assert_eq!(story.status, ExecutionStatus::Completed);
        assert!(story.completed_at.is_some());
        assert!(story.duration_ms().is_some());
    }

    #[test]
    fn test_execution_session() {
        let mut session = ExecutionSession::new(
            "session-001",
            "/test/project",
            "anthropic",
            "claude-3-5-sonnet",
        );

        session.add_story("story-001", "First Story");
        session.add_story("story-002", "Second Story");
        session.add_story("story-003", "Third Story");

        assert_eq!(session.stories.len(), 3);
        assert_eq!(session.current_story_index, 0);
        assert_eq!(session.progress_percentage(), 0.0);

        // Complete first story
        session.current_story_mut().unwrap().complete();
        assert_eq!(session.completed_stories(), 1);

        // Progress should update when we have explicit completed count
        // Note: progress is based on completed status, not index
        let progress = session.progress_percentage();
        assert!((progress - 33.333).abs() < 1.0);

        // Advance to next story
        assert!(session.advance_to_next_story());
        assert_eq!(session.current_story_index, 1);
    }

    #[test]
    fn test_execution_progress() {
        let mut session = ExecutionSession::new(
            "session-001",
            "/test/project",
            "anthropic",
            "claude-3-5-sonnet",
        );

        session.add_story("story-001", "First Story");
        session.add_story("story-002", "Second Story");
        session.start();

        let progress = ExecutionProgress::from_session(&session);
        assert_eq!(progress.session_id, "session-001");
        assert_eq!(progress.total_stories, 2);
        assert_eq!(progress.status, ExecutionStatus::Running);
    }
}
