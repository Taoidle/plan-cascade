//! Recovery System Integration Tests
//!
//! Tests for the recovery detector and resume engine using a real in-memory
//! SQLite database:
//! - Detection of interrupted executions across all modes (AC4)
//! - Accurate state summaries (AC4)
//! - Successful resume from checkpoints (AC4)
//! - Discard functionality
//!
//! No LLM calls are made. Tests use the actual Database service with in-memory SQLite.

use serde_json::json;

use plan_cascade_desktop::services::recovery::{
    IncompleteTask, RecoveryDetector, ResumeEngine, ResumeEvent, ResumeResult,
};
use plan_cascade_desktop::services::recovery::detector::ExecutionMode;
use plan_cascade_desktop::services::recovery::resume::RestoredContext;
use plan_cascade_desktop::storage::database::Database;
use plan_cascade_desktop::utils::error::AppResult;

// ============================================================================
// Helpers
// ============================================================================

fn create_test_db() -> Database {
    Database::new_in_memory().expect("Failed to create in-memory test database")
}

fn insert_execution(
    db: &Database,
    id: &str,
    mode: &str,
    status: &str,
    total: i32,
    completed: i32,
    context: &str,
) {
    db.insert_execution(id, None, &format!("Test {}", id), mode, "/test/project", total, context)
        .unwrap();

    if status != "running" {
        db.update_execution_status(id, status, None).unwrap();
    }

    if completed > 0 {
        db.update_execution_progress(id, completed, None, (completed as f64 / total as f64) * 100.0, context)
            .unwrap();
    }
}

fn insert_execution_with_session(
    db: &Database,
    id: &str,
    session_id: &str,
    mode: &str,
    status: &str,
    total: i32,
    completed: i32,
    context: &str,
) {
    db.insert_execution(
        id,
        Some(session_id),
        &format!("Test {}", id),
        mode,
        "/test/project",
        total,
        context,
    )
    .unwrap();

    if status != "running" {
        db.update_execution_status(id, status, None).unwrap();
    }

    if completed > 0 {
        db.update_execution_progress(
            id,
            completed,
            Some(&format!("story-{}", completed)),
            (completed as f64 / total as f64) * 100.0,
            context,
        )
        .unwrap();
    }
}

// ============================================================================
// AC4: Detection of Interrupted Executions in All Modes
// ============================================================================

#[test]
fn test_detect_no_incomplete_tasks() {
    let db = create_test_db();
    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert!(tasks.is_empty());
}

#[test]
fn test_detect_direct_mode_interruption() {
    let db = create_test_db();

    let ctx = json!({"task": "fix bug"}).to_string();
    insert_execution(&db, "exec-direct-001", "direct", "running", 1, 0, &ctx);

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].execution_mode, ExecutionMode::Direct);
    assert_eq!(tasks[0].status, "running");
    assert_eq!(tasks[0].total_stories, 1);
    assert_eq!(tasks[0].completed_stories, 0);
}

#[test]
fn test_detect_hybrid_auto_interruption() {
    let db = create_test_db();

    let ctx = json!({
        "completed_story_ids": ["s1", "s2"],
        "prd": {"stories": [
            {"id": "s1"}, {"id": "s2"}, {"id": "s3"}, {"id": "s4"}, {"id": "s5"}
        ]}
    })
    .to_string();
    insert_execution(&db, "exec-hybrid-001", "hybrid_auto", "running", 5, 2, &ctx);

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].execution_mode, ExecutionMode::HybridAuto);
    assert_eq!(tasks[0].total_stories, 5);
    assert_eq!(tasks[0].completed_stories, 2);
    assert!(tasks[0].progress > 0.0);
    assert!(tasks[0].recoverable);
}

#[test]
fn test_detect_hybrid_worktree_interruption() {
    let db = create_test_db();

    let ctx = json!({"worktree": "/tmp/wt", "stories": [
        {"id": "s1", "status": "completed"},
        {"id": "s2", "status": "in_progress"},
        {"id": "s3", "status": "pending"},
    ]})
    .to_string();
    insert_execution(&db, "exec-wt-001", "hybrid_worktree", "running", 3, 1, &ctx);
    db.update_execution_status("exec-wt-001", "paused", None).unwrap();

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].execution_mode, ExecutionMode::HybridWorktree);
    assert_eq!(tasks[0].status, "paused");
    assert!(tasks[0].recoverable);
}

#[test]
fn test_detect_mega_plan_interruption() {
    let db = create_test_db();

    let ctx = json!({
        "mega_plan": true,
        "completed_story_ids": ["f1-s1", "f1-s2", "f2-s1"],
        "prd": {"stories": [
            {"id": "f1-s1"}, {"id": "f1-s2"},
            {"id": "f2-s1"}, {"id": "f2-s2"}, {"id": "f2-s3"},
            {"id": "f3-s1"}, {"id": "f3-s2"}, {"id": "f3-s3"}, {"id": "f3-s4"}, {"id": "f3-s5"},
        ]}
    })
    .to_string();
    insert_execution(&db, "exec-mega-001", "mega_plan", "running", 10, 3, &ctx);

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].execution_mode, ExecutionMode::MegaPlan);
    assert_eq!(tasks[0].total_stories, 10);
    assert_eq!(tasks[0].completed_stories, 3);
    assert!(tasks[0].recoverable);
}

#[test]
fn test_detect_multiple_interrupted_executions() {
    let db = create_test_db();

    insert_execution(&db, "exec-1", "direct", "running", 1, 0, r#"{"a":1}"#);
    insert_execution(&db, "exec-2", "hybrid_auto", "running", 5, 2, r#"{"b":2}"#);
    insert_execution(&db, "exec-3", "mega_plan", "running", 10, 3, r#"{"c":3}"#);
    db.update_execution_status("exec-3", "failed", None).unwrap();
    insert_execution(&db, "exec-4", "hybrid_worktree", "running", 3, 1, r#"{"d":4}"#);
    db.update_execution_status("exec-4", "paused", None).unwrap();

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert_eq!(tasks.len(), 4);

    let modes: Vec<ExecutionMode> = tasks.iter().map(|t| t.execution_mode.clone()).collect();
    assert!(modes.contains(&ExecutionMode::Direct));
    assert!(modes.contains(&ExecutionMode::HybridAuto));
    assert!(modes.contains(&ExecutionMode::MegaPlan));
    assert!(modes.contains(&ExecutionMode::HybridWorktree));
}

#[test]
fn test_detect_ignores_completed_and_cancelled() {
    let db = create_test_db();

    insert_execution(&db, "exec-done", "hybrid_auto", "running", 5, 5, r#"{"ok":true}"#);
    db.update_execution_status("exec-done", "completed", None).unwrap();

    insert_execution(&db, "exec-cancel", "direct", "running", 1, 0, r#"{"ok":true}"#);
    db.update_execution_status("exec-cancel", "cancelled", None).unwrap();

    insert_execution(&db, "exec-active", "hybrid_auto", "running", 3, 1, r#"{"ok":true}"#);

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "exec-active");
}

// ============================================================================
// AC4: Accurate State Summaries
// ============================================================================

#[test]
fn test_state_summary_has_correct_fields() {
    let db = create_test_db();

    let ctx = json!({
        "completed_story_ids": ["s1"],
        "prd": {"stories": [{"id": "s1"}, {"id": "s2"}, {"id": "s3"}]}
    })
    .to_string();
    insert_execution(&db, "exec-sum", "hybrid_auto", "running", 3, 1, &ctx);

    let tasks = RecoveryDetector::detect(&db).unwrap();
    let task = &tasks[0];

    assert_eq!(task.id, "exec-sum");
    assert_eq!(task.name, "Test exec-sum");
    assert_eq!(task.execution_mode, ExecutionMode::HybridAuto);
    assert_eq!(task.status, "running");
    assert_eq!(task.project_path, "/test/project");
    assert_eq!(task.total_stories, 3);
    assert_eq!(task.completed_stories, 1);
    assert!(task.progress > 0.0);
    assert!(task.recoverable);
    assert!(task.recovery_note.is_none()); // Running with valid context
}

#[test]
fn test_state_summary_failed_execution() {
    let db = create_test_db();

    let ctx = json!({"error": "OOM"}).to_string();
    insert_execution(&db, "exec-fail", "hybrid_auto", "running", 5, 2, &ctx);
    db.update_execution_status("exec-fail", "failed", Some("Out of memory")).unwrap();

    let tasks = RecoveryDetector::detect(&db).unwrap();
    let task = &tasks[0];

    assert_eq!(task.status, "failed");
    assert!(task.recoverable);
    assert!(task.recovery_note.is_some());
    assert!(task.recovery_note.as_deref().unwrap().contains("failed"));
    assert_eq!(task.error_message.as_deref(), Some("Out of memory"));
}

#[test]
fn test_state_summary_no_context_hybrid_not_recoverable() {
    let db = create_test_db();

    insert_execution(&db, "exec-noctx", "hybrid_auto", "running", 5, 2, "{}");

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert_eq!(tasks.len(), 1);
    assert!(!tasks[0].recoverable);
    assert!(tasks[0].recovery_note.as_deref().unwrap().contains("No execution context"));
}

#[test]
fn test_state_summary_corrupted_context_not_recoverable() {
    let db = create_test_db();

    insert_execution(&db, "exec-corrupt", "hybrid_auto", "running", 5, 2, "not json");

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert!(!tasks[0].recoverable);
    assert!(tasks[0].recovery_note.as_deref().unwrap().contains("corrupted"));
}

#[test]
fn test_state_summary_all_stories_done_recoverable() {
    let db = create_test_db();

    let ctx = json!({"final": true}).to_string();
    insert_execution(&db, "exec-alldone", "hybrid_auto", "running", 5, 5, &ctx);

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert!(tasks[0].recoverable);
    assert!(tasks[0].recovery_note.as_deref().unwrap().contains("finalization"));
}

#[test]
fn test_state_summary_direct_empty_context_recoverable() {
    let db = create_test_db();

    insert_execution(&db, "exec-direct-empty", "direct", "running", 0, 0, "{}");

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert!(tasks[0].recoverable);
    assert!(tasks[0].recovery_note.as_deref().unwrap().contains("restart"));
}

// ============================================================================
// AC4: Checkpoint Integration
// ============================================================================

#[test]
fn test_detect_with_checkpoints() {
    let db = create_test_db();

    let ctx = json!({"ok": true}).to_string();

    // Create a session first
    let conn = db.get_connection().unwrap();
    conn.execute(
        "INSERT INTO sessions (id, project_path, name) VALUES ('sess-cp', '/test', 'Test Session')",
        [],
    )
    .unwrap();
    drop(conn);

    // Insert execution linked to session
    insert_execution_with_session(
        &db, "exec-cp", "sess-cp", "hybrid_auto", "running", 5, 2, &ctx,
    );

    // Insert checkpoints for the session
    let conn = db.get_connection().unwrap();
    conn.execute(
        "INSERT INTO checkpoints (id, session_id, name, snapshot, created_at)
         VALUES ('cp-1', 'sess-cp', 'Checkpoint 1', '{}', '2024-01-01T10:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO checkpoints (id, session_id, name, snapshot, created_at)
         VALUES ('cp-2', 'sess-cp', 'Checkpoint 2', '{}', '2024-01-01T11:00:00Z')",
        [],
    )
    .unwrap();
    drop(conn);

    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].checkpoint_count, 2);
    assert!(tasks[0].last_checkpoint_timestamp.is_some());
}

// ============================================================================
// AC4: Resume from Checkpoints
// ============================================================================

#[test]
fn test_resume_hybrid_auto_execution() {
    let db = create_test_db();

    let ctx = json!({
        "completed_story_ids": ["s1", "s2"],
        "prd": {
            "stories": [
                {"id": "s1", "title": "Story 1"},
                {"id": "s2", "title": "Story 2"},
                {"id": "s3", "title": "Story 3"},
                {"id": "s4", "title": "Story 4"},
            ]
        }
    })
    .to_string();
    insert_execution(&db, "exec-resume-1", "hybrid_auto", "running", 4, 2, &ctx);

    let result = ResumeEngine::resume(&db, "exec-resume-1").unwrap();

    assert!(result.success);
    assert_eq!(result.execution_id, "exec-resume-1");

    let context = result.context.unwrap();
    assert_eq!(context.execution_mode, ExecutionMode::HybridAuto);
    assert_eq!(context.completed_story_ids, vec!["s1", "s2"]);
    assert_eq!(context.remaining_story_ids, vec!["s3", "s4"]);
    assert_eq!(context.total_stories, 4);
    assert_eq!(context.completed_stories, 2);

    // Verify events
    let event_types: Vec<String> = result
        .events
        .iter()
        .map(|e| match e {
            ResumeEvent::Started { .. } => "Started".to_string(),
            ResumeEvent::ContextRestored { .. } => "ContextRestored".to_string(),
            ResumeEvent::StorySkipped { .. } => "StorySkipped".to_string(),
            ResumeEvent::Resuming { .. } => "Resuming".to_string(),
            ResumeEvent::Completed { .. } => "Completed".to_string(),
            ResumeEvent::Error { .. } => "Error".to_string(),
        })
        .collect();

    assert!(event_types.contains(&"Started".to_string()));
    assert!(event_types.contains(&"ContextRestored".to_string()));
    assert_eq!(
        event_types.iter().filter(|e| *e == "StorySkipped").count(),
        2,
        "Should have 2 StorySkipped events"
    );
    assert!(event_types.contains(&"Resuming".to_string()));

    // Verify execution status was updated to running
    let exec = db.get_execution("exec-resume-1").unwrap().unwrap();
    assert_eq!(exec.status, "running");
}

#[test]
fn test_resume_mega_plan_with_top_level_stories() {
    let db = create_test_db();

    let ctx = json!({
        "stories": [
            {"id": "s1", "status": "completed"},
            {"id": "s2", "status": "completed"},
            {"id": "s3", "status": "in_progress"},
            {"id": "s4", "status": "pending"},
        ]
    })
    .to_string();
    insert_execution(&db, "exec-mega-res", "mega_plan", "running", 4, 2, &ctx);

    let result = ResumeEngine::resume(&db, "exec-mega-res").unwrap();

    assert!(result.success);
    let context = result.context.unwrap();
    assert_eq!(context.execution_mode, ExecutionMode::MegaPlan);
    assert_eq!(context.completed_story_ids, vec!["s1", "s2"]);
    assert_eq!(context.remaining_story_ids, vec!["s3", "s4"]);
}

#[test]
fn test_resume_with_remaining_ids_fallback() {
    let db = create_test_db();

    let ctx = json!({
        "completed_story_ids": ["s1"],
        "remaining_story_ids": ["s2", "s3"],
    })
    .to_string();
    insert_execution(&db, "exec-remaining", "hybrid_worktree", "running", 3, 1, &ctx);

    let result = ResumeEngine::resume(&db, "exec-remaining").unwrap();

    assert!(result.success);
    let context = result.context.unwrap();
    assert_eq!(context.completed_story_ids, vec!["s1"]);
    assert_eq!(context.remaining_story_ids, vec!["s2", "s3"]);
}

#[test]
fn test_resume_completed_execution_fails() {
    let db = create_test_db();

    insert_execution(&db, "exec-done-2", "hybrid_auto", "running", 3, 3, r#"{"done":true}"#);
    db.update_execution_status("exec-done-2", "completed", None).unwrap();

    let result = ResumeEngine::resume(&db, "exec-done-2").unwrap();
    assert!(!result.success);
    assert!(result.error.is_some());
    assert!(result.error.unwrap().contains("completed"));
}

#[test]
fn test_resume_cancelled_execution_fails() {
    let db = create_test_db();

    insert_execution(&db, "exec-cancel-2", "direct", "running", 1, 0, r#"{"cancel":true}"#);
    db.update_execution_status("exec-cancel-2", "cancelled", None).unwrap();

    let result = ResumeEngine::resume(&db, "exec-cancel-2").unwrap();
    assert!(!result.success);
    assert!(result.error.unwrap().contains("cancelled"));
}

#[test]
fn test_resume_nonexistent_execution_fails() {
    let db = create_test_db();
    let result = ResumeEngine::resume(&db, "nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_resume_corrupted_context_fails() {
    let db = create_test_db();

    insert_execution(&db, "exec-bad-ctx", "hybrid_auto", "running", 5, 2, "not json");

    let result = ResumeEngine::resume(&db, "exec-bad-ctx");
    assert!(result.is_err());
}

#[test]
fn test_resume_failed_execution() {
    let db = create_test_db();

    let ctx = json!({
        "completed_story_ids": ["s1"],
        "prd": {"stories": [{"id": "s1"}, {"id": "s2"}, {"id": "s3"}]}
    })
    .to_string();
    insert_execution(&db, "exec-failed-res", "hybrid_auto", "running", 3, 1, &ctx);
    db.update_execution_status("exec-failed-res", "failed", Some("Timeout")).unwrap();

    let result = ResumeEngine::resume(&db, "exec-failed-res").unwrap();
    assert!(result.success);
    let context = result.context.unwrap();
    assert_eq!(context.remaining_story_ids, vec!["s2", "s3"]);

    // Status should be updated back to running
    let exec = db.get_execution("exec-failed-res").unwrap().unwrap();
    assert_eq!(exec.status, "running");
}

// ============================================================================
// AC4: Discard Functionality
// ============================================================================

#[test]
fn test_discard_running_execution() {
    let db = create_test_db();

    insert_execution(&db, "exec-discard-1", "hybrid_auto", "running", 5, 2, r#"{"ok":true}"#);

    ResumeEngine::discard(&db, "exec-discard-1").unwrap();

    let exec = db.get_execution("exec-discard-1").unwrap().unwrap();
    assert_eq!(exec.status, "cancelled");
}

#[test]
fn test_discard_paused_execution() {
    let db = create_test_db();

    insert_execution(&db, "exec-discard-2", "mega_plan", "running", 10, 3, r#"{"ok":true}"#);
    db.update_execution_status("exec-discard-2", "paused", None).unwrap();

    ResumeEngine::discard(&db, "exec-discard-2").unwrap();

    let exec = db.get_execution("exec-discard-2").unwrap().unwrap();
    assert_eq!(exec.status, "cancelled");
}

#[test]
fn test_discard_completed_execution_fails() {
    let db = create_test_db();

    insert_execution(&db, "exec-discard-3", "direct", "running", 1, 1, r#"{"ok":true}"#);
    db.update_execution_status("exec-discard-3", "completed", None).unwrap();

    let result = ResumeEngine::discard(&db, "exec-discard-3");
    assert!(result.is_err());
}

#[test]
fn test_discard_nonexistent_fails() {
    let db = create_test_db();
    let result = ResumeEngine::discard(&db, "nonexistent");
    assert!(result.is_err());
}

// ============================================================================
// Model Serialization Tests
// ============================================================================

#[test]
fn test_incomplete_task_serialization_all_modes() {
    for (mode_str, mode) in [
        ("direct", ExecutionMode::Direct),
        ("hybrid_auto", ExecutionMode::HybridAuto),
        ("hybrid_worktree", ExecutionMode::HybridWorktree),
        ("mega_plan", ExecutionMode::MegaPlan),
    ] {
        let task = IncompleteTask {
            id: format!("exec-{}", mode_str),
            session_id: None,
            name: format!("Test {}", mode_str),
            execution_mode: mode.clone(),
            status: "running".to_string(),
            project_path: "/test".to_string(),
            total_stories: 5,
            completed_stories: 2,
            current_story_id: Some("s3".to_string()),
            progress: 40.0,
            last_checkpoint_timestamp: None,
            recoverable: true,
            recovery_note: None,
            checkpoint_count: 0,
            error_message: None,
        };

        let json_str = serde_json::to_string(&task).unwrap();
        assert!(json_str.contains(mode_str));

        let parsed: IncompleteTask = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.execution_mode, mode);
    }
}

#[test]
fn test_resume_result_serialization() {
    let result = ResumeResult::success(
        "exec-001",
        RestoredContext {
            execution_id: "exec-001".to_string(),
            execution_mode: ExecutionMode::HybridAuto,
            project_path: "/project".to_string(),
            name: "Test".to_string(),
            completed_story_ids: vec!["s1".to_string()],
            remaining_story_ids: vec!["s2".to_string()],
            context_snapshot: json!({}),
            total_stories: 2,
            completed_stories: 1,
            progress: 50.0,
        },
    );

    let json_str = serde_json::to_string(&result).unwrap();
    let parsed: ResumeResult = serde_json::from_str(&json_str).unwrap();

    assert!(parsed.success);
    assert!(parsed.context.is_some());
    assert_eq!(parsed.context.unwrap().remaining_story_ids, vec!["s2"]);
}

// ============================================================================
// End-to-End: Detect then Resume
// ============================================================================

#[test]
fn test_detect_then_resume_workflow() {
    let db = create_test_db();

    // Simulate an interrupted execution
    let ctx = json!({
        "completed_story_ids": ["s1", "s2", "s3"],
        "prd": {
            "stories": [
                {"id": "s1"}, {"id": "s2"}, {"id": "s3"},
                {"id": "s4"}, {"id": "s5"},
            ]
        }
    })
    .to_string();
    insert_execution(&db, "exec-e2e", "hybrid_auto", "running", 5, 3, &ctx);

    // Step 1: Detect
    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert_eq!(tasks.len(), 1);
    assert!(tasks[0].recoverable);
    assert_eq!(tasks[0].id, "exec-e2e");

    // Step 2: Resume
    let result = ResumeEngine::resume(&db, "exec-e2e").unwrap();
    assert!(result.success);

    let context = result.context.unwrap();
    assert_eq!(context.completed_story_ids.len(), 3);
    assert_eq!(context.remaining_story_ids.len(), 2);
    assert_eq!(context.remaining_story_ids, vec!["s4", "s5"]);
}

#[test]
fn test_detect_then_discard_workflow() {
    let db = create_test_db();

    let ctx = json!({"ok": true}).to_string();
    insert_execution(&db, "exec-discard-e2e", "hybrid_worktree", "running", 3, 1, &ctx);

    // Detect
    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert_eq!(tasks.len(), 1);

    // Discard
    ResumeEngine::discard(&db, "exec-discard-e2e").unwrap();

    // Detect again - should be empty
    let tasks = RecoveryDetector::detect(&db).unwrap();
    assert!(tasks.is_empty());
}
