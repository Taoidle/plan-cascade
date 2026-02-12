//! Standalone Execution Integration Tests
//!
//! Tests for standalone LLM execution models and session management.

use plan_cascade_desktop::models::orchestrator::{
    ExecuteWithSessionRequest, ExecutionProgress, ExecutionSession, ExecutionSessionSummary,
    ExecutionStatus, ResumeExecutionRequest, StoryExecutionState,
};

// ============================================================================
// Execution Status Tests
// ============================================================================

#[test]
fn test_execution_status_display() {
    assert_eq!(ExecutionStatus::Pending.to_string(), "pending");
    assert_eq!(ExecutionStatus::Running.to_string(), "running");
    assert_eq!(ExecutionStatus::Paused.to_string(), "paused");
    assert_eq!(ExecutionStatus::Completed.to_string(), "completed");
    assert_eq!(ExecutionStatus::Failed.to_string(), "failed");
    assert_eq!(ExecutionStatus::Cancelled.to_string(), "cancelled");
}

#[test]
fn test_execution_status_parse() {
    assert_eq!(
        "pending".parse::<ExecutionStatus>().unwrap(),
        ExecutionStatus::Pending
    );
    assert_eq!(
        "running".parse::<ExecutionStatus>().unwrap(),
        ExecutionStatus::Running
    );
    assert_eq!(
        "completed".parse::<ExecutionStatus>().unwrap(),
        ExecutionStatus::Completed
    );
    assert_eq!(
        "failed".parse::<ExecutionStatus>().unwrap(),
        ExecutionStatus::Failed
    );
    assert_eq!(
        "paused".parse::<ExecutionStatus>().unwrap(),
        ExecutionStatus::Paused
    );
    assert_eq!(
        "cancelled".parse::<ExecutionStatus>().unwrap(),
        ExecutionStatus::Cancelled
    );
}

#[test]
fn test_execution_status_is_terminal() {
    assert!(!ExecutionStatus::Pending.is_terminal());
    assert!(!ExecutionStatus::Running.is_terminal());
    assert!(!ExecutionStatus::Paused.is_terminal());
    assert!(ExecutionStatus::Completed.is_terminal());
    assert!(!ExecutionStatus::Failed.is_terminal());
    assert!(ExecutionStatus::Cancelled.is_terminal());
}

#[test]
fn test_execution_status_can_resume() {
    assert!(!ExecutionStatus::Pending.can_resume());
    assert!(!ExecutionStatus::Running.can_resume());
    assert!(ExecutionStatus::Paused.can_resume());
    assert!(!ExecutionStatus::Completed.can_resume());
    assert!(ExecutionStatus::Failed.can_resume());
    assert!(!ExecutionStatus::Cancelled.can_resume());
}

#[test]
fn test_execution_status_is_active() {
    assert!(ExecutionStatus::Pending.is_active());
    assert!(ExecutionStatus::Running.is_active());
    assert!(!ExecutionStatus::Paused.is_active());
    assert!(!ExecutionStatus::Completed.is_active());
    assert!(!ExecutionStatus::Failed.is_active());
    assert!(!ExecutionStatus::Cancelled.is_active());
}

// ============================================================================
// Story Execution State Tests
// ============================================================================

#[test]
fn test_story_execution_state_creation() {
    let story = StoryExecutionState::new("story-001", "Test Story");

    assert_eq!(story.story_id, "story-001");
    assert_eq!(story.title, "Test Story");
    assert_eq!(story.status, ExecutionStatus::Pending);
    assert!(story.started_at.is_none());
    assert!(story.completed_at.is_none());
    assert!(story.error.is_none());
    assert_eq!(story.iterations, 0);
    assert_eq!(story.input_tokens, 0);
    assert_eq!(story.output_tokens, 0);
    assert!(story.quality_gates.is_empty());
}

#[test]
fn test_story_execution_state_start() {
    let mut story = StoryExecutionState::new("story-001", "Test Story");

    story.start();

    assert_eq!(story.status, ExecutionStatus::Running);
    assert!(story.started_at.is_some());
    assert!(story.completed_at.is_none());
}

#[test]
fn test_story_execution_state_complete() {
    let mut story = StoryExecutionState::new("story-001", "Test Story");

    story.start();
    story.complete();

    assert_eq!(story.status, ExecutionStatus::Completed);
    assert!(story.started_at.is_some());
    assert!(story.completed_at.is_some());
    assert!(story.error.is_none());
}

#[test]
fn test_story_execution_state_fail() {
    let mut story = StoryExecutionState::new("story-001", "Test Story");

    story.start();
    story.fail("Something went wrong");

    assert_eq!(story.status, ExecutionStatus::Failed);
    assert!(story.completed_at.is_some());
    assert_eq!(story.error, Some("Something went wrong".to_string()));
}

#[test]
fn test_story_execution_state_duration() {
    let mut story = StoryExecutionState::new("story-001", "Test Story");

    // No duration before start
    assert!(story.duration_ms().is_none());

    story.start();
    // No duration before complete
    assert!(story.duration_ms().is_none());

    story.complete();
    // Duration should be available after complete
    assert!(story.duration_ms().is_some());
}

// ============================================================================
// Execution Session Tests
// ============================================================================

#[test]
fn test_execution_session_creation() {
    let session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    );

    assert_eq!(session.id, "session-001");
    assert_eq!(session.project_path, "/test/project");
    assert_eq!(session.provider, "anthropic");
    assert_eq!(session.model, "claude-3-5-sonnet");
    assert_eq!(session.status, ExecutionStatus::Pending);
    assert!(session.prd_path.is_none());
    assert!(session.system_prompt.is_none());
    assert!(session.stories.is_empty());
    assert_eq!(session.current_story_index, 0);
    assert_eq!(session.total_input_tokens, 0);
    assert_eq!(session.total_output_tokens, 0);
}

#[test]
fn test_execution_session_with_prd() {
    let session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    )
    .with_prd("/test/project/prd.json");

    assert_eq!(session.prd_path, Some("/test/project/prd.json".to_string()));
}

#[test]
fn test_execution_session_with_system_prompt() {
    let session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    )
    .with_system_prompt("You are a helpful coding assistant.");

    assert_eq!(
        session.system_prompt,
        Some("You are a helpful coding assistant.".to_string())
    );
}

#[test]
fn test_execution_session_add_story() {
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
    assert_eq!(session.stories[0].story_id, "story-001");
    assert_eq!(session.stories[1].story_id, "story-002");
    assert_eq!(session.stories[2].story_id, "story-003");
}

#[test]
fn test_execution_session_current_story() {
    let mut session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    );

    session.add_story("story-001", "First Story");
    session.add_story("story-002", "Second Story");

    let current = session.current_story();
    assert!(current.is_some());
    assert_eq!(current.unwrap().story_id, "story-001");
}

#[test]
fn test_execution_session_advance_to_next_story() {
    let mut session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    );

    session.add_story("story-001", "First Story");
    session.add_story("story-002", "Second Story");

    assert_eq!(session.current_story_index, 0);

    let advanced = session.advance_to_next_story();
    assert!(advanced);
    assert_eq!(session.current_story_index, 1);

    // Cannot advance past the last story
    let advanced = session.advance_to_next_story();
    assert!(!advanced);
    assert_eq!(session.current_story_index, 1);
}

#[test]
fn test_execution_session_lifecycle() {
    let mut session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    );

    assert_eq!(session.status, ExecutionStatus::Pending);

    session.start();
    assert_eq!(session.status, ExecutionStatus::Running);
    assert!(session.started_at.is_some());

    session.pause();
    assert_eq!(session.status, ExecutionStatus::Paused);

    session.complete();
    assert_eq!(session.status, ExecutionStatus::Completed);
    assert!(session.completed_at.is_some());
}

#[test]
fn test_execution_session_cancel() {
    let mut session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    );

    session.start();
    session.cancel();

    assert_eq!(session.status, ExecutionStatus::Cancelled);
    assert!(session.completed_at.is_some());
}

#[test]
fn test_execution_session_fail() {
    let mut session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    );

    session.start();
    session.fail("Something went wrong");

    assert_eq!(session.status, ExecutionStatus::Failed);
    assert_eq!(session.error, Some("Something went wrong".to_string()));
}

#[test]
fn test_execution_session_add_tokens() {
    let mut session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    );

    session.add_tokens(100, 50);
    assert_eq!(session.total_input_tokens, 100);
    assert_eq!(session.total_output_tokens, 50);

    session.add_tokens(200, 100);
    assert_eq!(session.total_input_tokens, 300);
    assert_eq!(session.total_output_tokens, 150);
}

#[test]
fn test_execution_session_progress_percentage() {
    let mut session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    );

    // Empty session has 0% progress
    assert_eq!(session.progress_percentage(), 0.0);

    // Add stories
    session.add_story("story-001", "First Story");
    session.add_story("story-002", "Second Story");
    session.add_story("story-003", "Third Story");

    // No stories complete
    assert_eq!(session.progress_percentage(), 0.0);

    // Complete first story
    session.stories[0].complete();
    let progress = session.progress_percentage();
    assert!((progress - 33.333).abs() < 1.0);

    // Complete all stories
    session.stories[1].complete();
    session.stories[2].complete();
    assert_eq!(session.progress_percentage(), 100.0);
}

#[test]
fn test_execution_session_story_counts() {
    let mut session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    );

    session.add_story("story-001", "First Story");
    session.add_story("story-002", "Second Story");
    session.add_story("story-003", "Third Story");

    session.stories[0].complete();
    session.stories[1].fail("Error".to_string());

    assert_eq!(session.completed_stories(), 1);
    assert_eq!(session.failed_stories(), 1);
}

// ============================================================================
// Execution Progress Tests
// ============================================================================

#[test]
fn test_execution_progress_from_session() {
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
    assert_eq!(progress.current_story, 0);
    assert_eq!(progress.status, ExecutionStatus::Running);
    assert_eq!(progress.story_id, Some("story-001".to_string()));
    assert_eq!(progress.story_title, Some("First Story".to_string()));
}

// ============================================================================
// Execution Session Summary Tests
// ============================================================================

#[test]
fn test_execution_session_summary_from_session() {
    let mut session = ExecutionSession::new(
        "session-001",
        "/test/project",
        "anthropic",
        "claude-3-5-sonnet",
    );

    session.add_story("story-001", "First Story");
    session.add_story("story-002", "Second Story");
    session.stories[0].complete();

    let summary = ExecutionSessionSummary::from(&session);

    assert_eq!(summary.id, "session-001");
    assert_eq!(summary.project_path, "/test/project");
    assert_eq!(summary.total_stories, 2);
    assert_eq!(summary.completed_stories, 1);
    assert_eq!(summary.provider, "anthropic");
    assert_eq!(summary.model, "claude-3-5-sonnet");
}

// ============================================================================
// Request Types Tests
// ============================================================================

#[test]
fn test_execute_with_session_request() {
    let request = ExecuteWithSessionRequest {
        project_path: "/test/project".to_string(),
        prd_path: Some("/test/project/prd.json".to_string()),
        provider: "anthropic".to_string(),
        model: "claude-3-5-sonnet".to_string(),
        system_prompt: Some("Custom prompt".to_string()),
        run_quality_gates: true,
        story_ids: Some(vec!["story-001".to_string(), "story-002".to_string()]),
        enable_thinking: None,
        max_total_tokens: None,
        max_iterations: None,
    };

    assert_eq!(request.project_path, "/test/project");
    assert!(request.prd_path.is_some());
    assert!(request.run_quality_gates);
    assert_eq!(request.story_ids.as_ref().unwrap().len(), 2);
}

#[test]
fn test_resume_execution_request() {
    let request = ResumeExecutionRequest {
        session_id: "session-001".to_string(),
        retry_failed: true,
        skip_current: false,
    };

    assert_eq!(request.session_id, "session-001");
    assert!(request.retry_failed);
    assert!(!request.skip_current);
}
