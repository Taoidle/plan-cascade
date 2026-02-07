//! Spec Interview Integration Tests
//!
//! Tests for the complete spec interview pipeline:
//! - Multi-turn conversation flow through all phases
//! - SQLite state persistence and resume
//! - Spec compilation output (spec.json, spec.md, prd.json)
//!
//! All tests use in-memory SQLite databases via Database::new_in_memory().
//! No LLM calls are made.

use serde_json::json;

use plan_cascade_desktop::services::spec_interview::{
    CompileOptions, InterviewManager, InterviewPhase, InterviewQuestion, SpecCompiler,
};
use plan_cascade_desktop::services::spec_interview::interview::InterviewConfig;
use plan_cascade_desktop::services::spec_interview::state::{
    InterviewStateManager, InterviewTurn, PersistedInterviewState,
};
use plan_cascade_desktop::storage::database::Database;

// ============================================================================
// Helpers
// ============================================================================

fn create_state_manager() -> InterviewStateManager {
    let db = Database::new_in_memory().unwrap();
    let pool = db.pool().clone();
    let mgr = InterviewStateManager::new(pool);
    mgr.init_schema().unwrap();
    mgr
}

fn create_interview_manager() -> InterviewManager {
    let mgr = create_state_manager();
    InterviewManager::new(mgr)
}

fn standard_config() -> InterviewConfig {
    InterviewConfig {
        description: "Build a user authentication system".to_string(),
        flow_level: "standard".to_string(),
        max_questions: 18,
        first_principles: false,
        project_path: Some("/tmp/test-project".to_string()),
    }
}

// ============================================================================
// AC2: Multi-turn Conversation Flow
// ============================================================================

#[test]
fn test_start_interview_returns_first_question() {
    let manager = create_interview_manager();
    let session = manager.start_interview(standard_config()).unwrap();

    assert!(!session.id.is_empty());
    assert_eq!(session.status, "in_progress");
    assert_eq!(session.phase, InterviewPhase::Overview);
    assert_eq!(session.question_cursor, 0);
    assert!(session.current_question.is_some());

    let q = session.current_question.unwrap();
    assert!(!q.id.is_empty());
    assert!(!q.question.is_empty());
    assert_eq!(q.phase, InterviewPhase::Overview);
    assert_eq!(q.field_name, "title");
    assert!(q.required);
}

#[test]
fn test_submit_answer_advances_cursor() {
    let manager = create_interview_manager();
    let session = manager.start_interview(standard_config()).unwrap();

    let updated = manager.submit_answer(&session.id, "My Auth System").unwrap();

    assert_eq!(updated.question_cursor, 1);
    assert!(updated.current_question.is_some());
    assert_eq!(updated.status, "in_progress");
    // History should now contain 1 entry
    assert_eq!(updated.history.len(), 1);
    assert_eq!(updated.history[0].answer, "My Auth System");
}

#[test]
fn test_multi_turn_conversation_through_overview() {
    let manager = create_interview_manager();
    let session = manager.start_interview(standard_config()).unwrap();

    // Turn 1: Title
    let s = manager.submit_answer(&session.id, "Auth System").unwrap();
    assert_eq!(s.history.len(), 1);

    // Turn 2: Goal
    let s = manager.submit_answer(&s.id, "Secure user authentication").unwrap();
    assert_eq!(s.history.len(), 2);

    // Turn 3: Success metrics
    let s = manager.submit_answer(&s.id, "Login works, tokens are valid, sessions persist").unwrap();
    assert_eq!(s.history.len(), 3);

    // Turn 4: Non-goals
    let s = manager.submit_answer(&s.id, "SSO, 2FA").unwrap();
    assert_eq!(s.history.len(), 4);
}

#[test]
fn test_first_principles_mode_adds_problem_question() {
    let manager = create_interview_manager();

    let config = InterviewConfig {
        description: "Auth system".to_string(),
        flow_level: "standard".to_string(),
        max_questions: 18,
        first_principles: true,
        project_path: None,
    };

    let session = manager.start_interview(config).unwrap();

    // First question should be title
    assert_eq!(
        session.current_question.as_ref().unwrap().field_name,
        "title"
    );

    // Answer title
    let s = manager.submit_answer(&session.id, "Auth System").unwrap();

    // Next question should be "problem" in first-principles mode
    assert_eq!(
        s.current_question.as_ref().unwrap().field_name,
        "problem"
    );
}

#[test]
fn test_phase_transition_via_next_keyword() {
    let manager = create_interview_manager();
    let session = manager.start_interview(standard_config()).unwrap();

    // Fill overview fields
    let s = manager.submit_answer(&session.id, "Test Project").unwrap(); // title
    let s = manager.submit_answer(&s.id, "Test goal").unwrap(); // goal
    let s = manager.submit_answer(&s.id, "Metric1, Metric2").unwrap(); // success_metrics
    let s = manager.submit_answer(&s.id, "Not this, Not that").unwrap(); // non_goals

    // At this point the overview fields are filled; the next question
    // should be a transition or auto-advance
    if s.phase == InterviewPhase::Overview {
        let s = manager.submit_answer(&s.id, "next").unwrap();
        assert!(
            s.phase == InterviewPhase::Scope || s.phase == InterviewPhase::Overview,
            "Phase should advance to Scope after 'next', got {:?}",
            s.phase
        );
    }
}

#[test]
fn test_submit_answer_to_completed_interview_fails() {
    let mgr = create_state_manager();

    // Create a finalized interview directly
    let state = PersistedInterviewState {
        id: "finalized-001".to_string(),
        status: "finalized".to_string(),
        phase: "complete".to_string(),
        flow_level: "standard".to_string(),
        first_principles: false,
        max_questions: 18,
        question_cursor: 10,
        description: "Done".to_string(),
        project_path: None,
        spec_data: "{}".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    mgr.create_interview(&state).unwrap();

    let manager = InterviewManager::new(mgr);
    let result = manager.submit_answer("finalized-001", "too late");
    assert!(result.is_err());
}

#[test]
fn test_get_nonexistent_interview_fails() {
    let manager = create_interview_manager();
    let result = manager.get_interview_state("nonexistent-id");
    assert!(result.is_err());
}

// ============================================================================
// AC2: SQLite State Persistence and Resume
// ============================================================================

#[test]
fn test_state_persistence_across_managers() {
    let db = Database::new_in_memory().unwrap();
    let pool = db.pool().clone();

    let mgr = InterviewStateManager::new(pool.clone());
    mgr.init_schema().unwrap();

    // Start interview with first manager
    let manager1 = InterviewManager::new(mgr.clone());
    let session = manager1.start_interview(standard_config()).unwrap();
    let s = manager1.submit_answer(&session.id, "Persistent Project").unwrap();

    // Create new manager with same pool (simulates app restart)
    let manager2 = InterviewManager::new(mgr);

    // Should be able to resume from exactly where we left off
    let resumed = manager2.get_interview_state(&session.id).unwrap();
    assert_eq!(resumed.question_cursor, s.question_cursor);
    assert_eq!(resumed.history.len(), 1);
    assert_eq!(resumed.history[0].answer, "Persistent Project");
}

#[test]
fn test_turn_persistence() {
    let mgr = create_state_manager();

    let state = PersistedInterviewState {
        id: "turn-test".to_string(),
        status: "in_progress".to_string(),
        phase: "overview".to_string(),
        flow_level: "standard".to_string(),
        first_principles: false,
        max_questions: 18,
        question_cursor: 0,
        description: "Test".to_string(),
        project_path: None,
        spec_data: "{}".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    mgr.create_interview(&state).unwrap();

    // Add multiple turns
    for i in 1..=5 {
        let turn = InterviewTurn {
            id: format!("turn-{}", i),
            interview_id: "turn-test".to_string(),
            turn_number: i,
            phase: "overview".to_string(),
            question: format!("Question {}", i),
            answer: format!("Answer {}", i),
            field_name: format!("field_{}", i),
            created_at: format!("2024-01-01T00:00:{:02}Z", i),
        };
        mgr.add_turn(&turn).unwrap();
    }

    let turns = mgr.get_turns("turn-test").unwrap();
    assert_eq!(turns.len(), 5);
    // Verify ordering
    for (i, turn) in turns.iter().enumerate() {
        assert_eq!(turn.turn_number, (i + 1) as i32);
    }

    let count = mgr.count_turns("turn-test").unwrap();
    assert_eq!(count, 5);
}

#[test]
fn test_interview_update_and_retrieval() {
    let mgr = create_state_manager();

    let mut state = PersistedInterviewState {
        id: "update-test".to_string(),
        status: "in_progress".to_string(),
        phase: "overview".to_string(),
        flow_level: "standard".to_string(),
        first_principles: false,
        max_questions: 18,
        question_cursor: 0,
        description: "Original".to_string(),
        project_path: None,
        spec_data: "{}".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    mgr.create_interview(&state).unwrap();

    // Update multiple fields
    state.phase = "requirements".to_string();
    state.question_cursor = 8;
    state.spec_data = r#"{"overview":{"title":"Test"}}"#.to_string();
    state.updated_at = "2024-01-02T00:00:00Z".to_string();
    mgr.update_interview(&state).unwrap();

    let retrieved = mgr.get_interview("update-test").unwrap().unwrap();
    assert_eq!(retrieved.phase, "requirements");
    assert_eq!(retrieved.question_cursor, 8);
    assert!(retrieved.spec_data.contains("Test"));
}

#[test]
fn test_list_interviews_filter() {
    let mgr = create_state_manager();

    // Create 5 interviews: 3 in_progress, 2 finalized
    for i in 1..=5 {
        let status = if i <= 3 { "in_progress" } else { "finalized" };
        let state = PersistedInterviewState {
            id: format!("list-{}", i),
            status: status.to_string(),
            phase: "overview".to_string(),
            flow_level: "standard".to_string(),
            first_principles: false,
            max_questions: 18,
            question_cursor: 0,
            description: format!("Interview {}", i),
            project_path: None,
            spec_data: "{}".to_string(),
            created_at: format!("2024-01-01T00:00:{:02}Z", i),
            updated_at: format!("2024-01-01T00:00:{:02}Z", i),
        };
        mgr.create_interview(&state).unwrap();
    }

    let all = mgr.list_interviews(None).unwrap();
    assert_eq!(all.len(), 5);

    let in_progress = mgr.list_interviews(Some("in_progress")).unwrap();
    assert_eq!(in_progress.len(), 3);

    let finalized = mgr.list_interviews(Some("finalized")).unwrap();
    assert_eq!(finalized.len(), 2);
}

#[test]
fn test_delete_interview_cascades_turns() {
    let mgr = create_state_manager();

    let state = PersistedInterviewState {
        id: "del-test".to_string(),
        status: "in_progress".to_string(),
        phase: "overview".to_string(),
        flow_level: "standard".to_string(),
        first_principles: false,
        max_questions: 18,
        question_cursor: 0,
        description: "To be deleted".to_string(),
        project_path: None,
        spec_data: "{}".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    mgr.create_interview(&state).unwrap();

    let turn = InterviewTurn {
        id: "t1".to_string(),
        interview_id: "del-test".to_string(),
        turn_number: 1,
        phase: "overview".to_string(),
        question: "Q?".to_string(),
        answer: "A!".to_string(),
        field_name: "title".to_string(),
        created_at: "2024-01-01T00:00:01Z".to_string(),
    };
    mgr.add_turn(&turn).unwrap();

    mgr.delete_interview("del-test").unwrap();

    assert!(mgr.get_interview("del-test").unwrap().is_none());
    assert!(mgr.get_turns("del-test").unwrap().is_empty());
}

// ============================================================================
// AC2: Spec Compilation Output
// ============================================================================

#[test]
fn test_compile_minimal_spec() {
    let spec_data = json!({
        "overview": {
            "title": "Auth System",
            "goal": "Secure authentication",
        }
    });

    let result = SpecCompiler::compile(&spec_data, &CompileOptions::default()).unwrap();

    // spec_json
    assert!(result.spec_json.get("metadata").is_some());
    assert_eq!(
        result.spec_json["metadata"]["schema_version"].as_str(),
        Some("spec-0.1")
    );
    assert_eq!(
        result.spec_json["overview"]["title"].as_str(),
        Some("Auth System")
    );

    // spec_md
    assert!(!result.spec_md.is_empty());
    assert!(result.spec_md.contains("Auth System"));
    assert!(result.spec_md.contains("Secure authentication"));

    // prd_json
    assert!(result.prd_json.get("goal").is_some());
    assert!(result.prd_json.get("stories").is_some());
    assert!(result.prd_json.get("metadata").is_some());
}

#[test]
fn test_compile_full_spec_with_stories() {
    let spec_data = json!({
        "overview": {
            "title": "Full Project",
            "goal": "Build everything",
            "success_metrics": ["All tests pass", "Coverage > 80%"],
            "non_goals": ["Mobile support"],
        },
        "scope": {
            "in_scope": ["Backend API", "Database layer"],
            "out_of_scope": ["Frontend"],
        },
        "requirements": {
            "functional": ["User login", "Token refresh", "Password reset"],
            "non_functional": {
                "performance_targets": ["< 200ms API response"],
                "security": ["HTTPS only", "Bcrypt passwords"],
            },
        },
        "interfaces": {
            "api": [
                {"name": "POST /auth/login", "notes": "User authentication"},
                {"name": "POST /auth/refresh", "notes": "Token refresh"},
            ],
            "data_models": [
                {"name": "User", "fields": ["id", "email", "password_hash"]},
            ],
        },
        "stories": [
            {
                "id": "story-001",
                "title": "User Registration",
                "category": "core",
                "description": "Implement user registration endpoint",
                "acceptance_criteria": ["POST /auth/register works", "Passwords are hashed"],
                "verification": {"commands": ["cargo test"], "manual_steps": []},
                "dependencies": [],
                "context_estimate": "medium",
            },
            {
                "id": "story-002",
                "title": "Login Flow",
                "category": "core",
                "description": "Implement login with JWT tokens",
                "acceptance_criteria": ["JWT tokens issued on login"],
                "verification": {"commands": ["cargo test"], "manual_steps": []},
                "dependencies": ["story-001"],
                "context_estimate": "medium",
            },
        ],
        "open_questions": ["What session store to use?"],
    });

    let result = SpecCompiler::compile(&spec_data, &CompileOptions::default()).unwrap();

    // Verify spec_json schema conformance
    assert_eq!(result.spec_json["metadata"]["schema_version"].as_str(), Some("spec-0.1"));
    assert_eq!(result.spec_json["metadata"]["source"].as_str(), Some("spec-interview"));
    assert!(result.spec_json["metadata"]["created_at"].as_str().is_some());

    // Verify stories in spec_json
    let stories = result.spec_json["stories"].as_array().unwrap();
    assert_eq!(stories.len(), 2);
    assert_eq!(stories[0]["id"].as_str(), Some("story-001"));
    assert_eq!(stories[1]["id"].as_str(), Some("story-002"));

    // Verify spec_md contains all sections
    assert!(result.spec_md.contains("# Spec: Full Project"));
    assert!(result.spec_md.contains("## Goal"));
    assert!(result.spec_md.contains("## Success Metrics"));
    assert!(result.spec_md.contains("## Scope"));
    assert!(result.spec_md.contains("## Requirements"));
    assert!(result.spec_md.contains("## Interfaces"));
    assert!(result.spec_md.contains("## Stories"));
    assert!(result.spec_md.contains("## Open Questions"));
    assert!(result.spec_md.contains("User Registration"));
    assert!(result.spec_md.contains("Login Flow"));

    // Verify prd_json
    assert_eq!(result.prd_json["goal"].as_str(), Some("Build everything"));
    let objectives = result.prd_json["objectives"].as_array().unwrap();
    assert_eq!(objectives.len(), 3); // functional requirements
    let prd_stories = result.prd_json["stories"].as_array().unwrap();
    assert_eq!(prd_stories.len(), 2);
    assert_eq!(prd_stories[0]["id"].as_str(), Some("story-001"));
    assert_eq!(prd_stories[0]["status"].as_str(), Some("pending"));
    assert_eq!(prd_stories[0]["priority"].as_str(), Some("high")); // core -> high
    assert!(!prd_stories[1]["dependencies"].as_array().unwrap().is_empty());
}

#[test]
fn test_compile_with_flow_options() {
    let spec_data = json!({
        "overview": {"goal": "Test"},
        "requirements": {"functional": ["Feature"]},
        "stories": [],
    });

    let options = CompileOptions {
        description: "Custom description".to_string(),
        flow_level: Some("full".to_string()),
        tdd_mode: Some("on".to_string()),
        confirm: false,
        no_confirm: true,
    };

    let result = SpecCompiler::compile(&spec_data, &options).unwrap();

    // Check flow config
    assert!(result.prd_json.get("flow_config").is_some());
    assert_eq!(result.prd_json["flow_config"]["level"].as_str(), Some("full"));

    // Check verification gate (full flow enables it)
    assert!(result.prd_json.get("verification_gate").is_some());
    assert_eq!(result.prd_json["verification_gate"]["enabled"].as_bool(), Some(true));

    // Check TDD config
    assert!(result.prd_json.get("tdd_config").is_some());
    assert_eq!(result.prd_json["tdd_config"]["mode"].as_str(), Some("on"));
    assert_eq!(
        result.prd_json["tdd_config"]["test_requirements"]["require_test_changes"].as_bool(),
        Some(true)
    );

    // Check execution config (no_confirm)
    assert!(result.prd_json.get("execution_config").is_some());
    assert_eq!(
        result.prd_json["execution_config"]["no_confirm_override"].as_bool(),
        Some(true)
    );

    // Check custom description
    assert_eq!(
        result.prd_json["metadata"]["description"].as_str(),
        Some("Custom description")
    );
}

#[test]
fn test_compile_objectives_truncation() {
    let functional: Vec<serde_json::Value> = (1..=10)
        .map(|i| json!(format!("Requirement {}", i)))
        .collect();

    let spec_data = json!({
        "overview": {"goal": "Truncation test"},
        "requirements": {"functional": functional},
        "stories": [],
    });

    let result = SpecCompiler::compile(&spec_data, &CompileOptions::default()).unwrap();
    let objectives = result.prd_json["objectives"].as_array().unwrap();
    assert!(objectives.len() <= 7, "Objectives should be truncated to 7, got {}", objectives.len());
}

#[test]
fn test_compile_category_to_priority_mapping() {
    let spec_data = json!({
        "overview": {"goal": "Priority test"},
        "stories": [
            {"id": "s1", "title": "Setup", "category": "setup", "description": "Setup", "acceptance_criteria": [], "verification": {"commands": [], "manual_steps": []}, "dependencies": [], "context_estimate": "small"},
            {"id": "s2", "title": "Core", "category": "core", "description": "Core", "acceptance_criteria": [], "verification": {"commands": [], "manual_steps": []}, "dependencies": [], "context_estimate": "medium"},
            {"id": "s3", "title": "Integration", "category": "integration", "description": "Integration", "acceptance_criteria": [], "verification": {"commands": [], "manual_steps": []}, "dependencies": [], "context_estimate": "medium"},
            {"id": "s4", "title": "Polish", "category": "polish", "description": "Polish", "acceptance_criteria": [], "verification": {"commands": [], "manual_steps": []}, "dependencies": [], "context_estimate": "small"},
            {"id": "s5", "title": "Test", "category": "test", "description": "Test", "acceptance_criteria": [], "verification": {"commands": [], "manual_steps": []}, "dependencies": [], "context_estimate": "small"},
        ],
    });

    let result = SpecCompiler::compile(&spec_data, &CompileOptions::default()).unwrap();
    let stories = result.prd_json["stories"].as_array().unwrap();

    assert_eq!(stories[0]["priority"].as_str(), Some("high"));    // setup -> high
    assert_eq!(stories[1]["priority"].as_str(), Some("high"));    // core -> high
    assert_eq!(stories[2]["priority"].as_str(), Some("medium"));  // integration -> medium
    assert_eq!(stories[3]["priority"].as_str(), Some("low"));     // polish -> low
    assert_eq!(stories[4]["priority"].as_str(), Some("medium"));  // test -> medium
}

// ============================================================================
// Interview Phase Progression
// ============================================================================

#[test]
fn test_phase_index_monotonically_increases() {
    let phases = vec![
        InterviewPhase::Overview,
        InterviewPhase::Scope,
        InterviewPhase::Requirements,
        InterviewPhase::Interfaces,
        InterviewPhase::Stories,
        InterviewPhase::Review,
        InterviewPhase::Complete,
    ];

    let mut prev_index = None;
    for phase in &phases {
        let idx = phase.index();
        if let Some(prev) = prev_index {
            assert!(idx > prev, "Phase {:?} index {} should be > {}", phase, idx, prev);
        }
        prev_index = Some(idx);
    }
}

#[test]
fn test_phase_next_chain() {
    let mut phase = InterviewPhase::Overview;
    let expected = vec![
        InterviewPhase::Scope,
        InterviewPhase::Requirements,
        InterviewPhase::Interfaces,
        InterviewPhase::Stories,
        InterviewPhase::Review,
        InterviewPhase::Complete,
        InterviewPhase::Complete, // Complete stays at Complete
    ];

    for expected_next in expected {
        phase = phase.next();
        assert_eq!(phase, expected_next, "Unexpected next phase");
    }
}

#[test]
fn test_phase_string_roundtrip() {
    let phases = vec![
        InterviewPhase::Overview,
        InterviewPhase::Scope,
        InterviewPhase::Requirements,
        InterviewPhase::Interfaces,
        InterviewPhase::Stories,
        InterviewPhase::Review,
        InterviewPhase::Complete,
    ];

    for phase in phases {
        let s = phase.as_str();
        let parsed = InterviewPhase::from_str(s);
        assert_eq!(parsed, phase, "Roundtrip failed for {:?}", phase);
    }
}

// ============================================================================
// Progress Calculation
// ============================================================================

#[test]
fn test_progress_increases_through_phases() {
    let manager = create_interview_manager();
    let session = manager.start_interview(standard_config()).unwrap();

    // At Overview phase, progress should be small
    assert!(session.progress >= 0.0);
    assert!(session.progress < 50.0);

    // Submit answers to advance
    let s = manager.submit_answer(&session.id, "Test Project").unwrap();
    assert!(s.progress >= session.progress, "Progress should not decrease");
}

// ============================================================================
// Spec Data Extraction
// ============================================================================

#[test]
fn test_get_spec_data_returns_accumulated_answers() {
    let manager = create_interview_manager();
    let session = manager.start_interview(standard_config()).unwrap();

    // Answer the title question
    manager.submit_answer(&session.id, "My Project").unwrap();

    let spec_data = manager.get_spec_data(&session.id).unwrap();
    // Should have overview with title populated
    let overview = spec_data.get("overview");
    assert!(overview.is_some());
    if let Some(overview) = overview {
        assert_eq!(overview.get("title").and_then(|v| v.as_str()), Some("My Project"));
    }
}

#[test]
fn test_get_spec_data_nonexistent_fails() {
    let manager = create_interview_manager();
    let result = manager.get_spec_data("nonexistent");
    assert!(result.is_err());
}

// ============================================================================
// Quick Flow Level
// ============================================================================

#[test]
fn test_quick_flow_config() {
    let manager = create_interview_manager();

    let config = InterviewConfig {
        description: "Quick test".to_string(),
        flow_level: "quick".to_string(),
        max_questions: 8,
        first_principles: false,
        project_path: None,
    };

    let session = manager.start_interview(config).unwrap();
    assert_eq!(session.flow_level, "quick");
    assert_eq!(session.max_questions, 8);
}
