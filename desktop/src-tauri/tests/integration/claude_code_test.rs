//! Claude Code Integration Tests
//!
//! Tests for Claude Code models including sessions, messages, and requests.

use plan_cascade_desktop::models::claude_code::{
    ActiveSessionInfo, ChatMessage, ClaudeCodeSession, SendMessageRequest, SessionState,
    StartChatRequest, StartChatResponse, ToolCallSummary,
};

// ============================================================================
// SessionState Tests
// ============================================================================

#[test]
fn test_session_state_default() {
    let state = SessionState::default();
    assert_eq!(state, SessionState::Idle);
}

#[test]
fn test_session_state_serialization() {
    // Test roundtrip serialization
    let states = vec![
        SessionState::Idle,
        SessionState::Running,
        SessionState::Waiting,
        SessionState::Cancelled,
        SessionState::Error,
    ];

    for state in states {
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SessionState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }
}

#[test]
fn test_session_state_json_format() {
    assert_eq!(
        serde_json::to_string(&SessionState::Idle).unwrap(),
        "\"idle\""
    );
    assert_eq!(
        serde_json::to_string(&SessionState::Running).unwrap(),
        "\"running\""
    );
    assert_eq!(
        serde_json::to_string(&SessionState::Waiting).unwrap(),
        "\"waiting\""
    );
    assert_eq!(
        serde_json::to_string(&SessionState::Cancelled).unwrap(),
        "\"cancelled\""
    );
    assert_eq!(
        serde_json::to_string(&SessionState::Error).unwrap(),
        "\"error\""
    );
}

// ============================================================================
// ClaudeCodeSession Tests
// ============================================================================

#[test]
fn test_session_creation() {
    let session = ClaudeCodeSession::new("sess-001", "/test/project");

    assert_eq!(session.id, "sess-001");
    assert_eq!(session.project_path, "/test/project");
    assert_eq!(session.state, SessionState::Idle);
    assert_eq!(session.message_count, 0);
    assert!(session.resume_token.is_none());
    assert!(session.model.is_none());
    assert!(session.error_message.is_none());
}

#[test]
fn test_session_with_model() {
    let session = ClaudeCodeSession::new("sess-001", "/test/project")
        .with_model("claude-3-5-sonnet-20241022");

    assert_eq!(
        session.model,
        Some("claude-3-5-sonnet-20241022".to_string())
    );
}

#[test]
fn test_session_mark_running() {
    let mut session = ClaudeCodeSession::new("sess-001", "/test/project");
    assert_eq!(session.state, SessionState::Idle);
    assert!(session.last_message_at.is_none());

    session.mark_running();

    assert_eq!(session.state, SessionState::Running);
    assert!(session.last_message_at.is_some());
}

#[test]
fn test_session_mark_idle() {
    let mut session = ClaudeCodeSession::new("sess-001", "/test/project");
    session.mark_running();
    assert_eq!(session.state, SessionState::Running);

    session.mark_idle();

    assert_eq!(session.state, SessionState::Idle);
}

#[test]
fn test_session_mark_error() {
    let mut session = ClaudeCodeSession::new("sess-001", "/test/project");

    session.mark_error("Connection failed");

    assert_eq!(session.state, SessionState::Error);
    assert_eq!(session.error_message, Some("Connection failed".to_string()));
}

#[test]
fn test_session_mark_cancelled() {
    let mut session = ClaudeCodeSession::new("sess-001", "/test/project");

    session.mark_cancelled();

    assert_eq!(session.state, SessionState::Cancelled);
}

#[test]
fn test_session_increment_messages() {
    let mut session = ClaudeCodeSession::new("sess-001", "/test/project");
    assert_eq!(session.message_count, 0);

    session.increment_messages();
    assert_eq!(session.message_count, 1);

    session.increment_messages();
    assert_eq!(session.message_count, 2);

    // Should also update last_message_at
    assert!(session.last_message_at.is_some());
}

#[test]
fn test_session_set_resume_token() {
    let mut session = ClaudeCodeSession::new("sess-001", "/test/project");
    assert!(session.resume_token.is_none());

    session.set_resume_token("resume-token-xyz");

    assert_eq!(session.resume_token, Some("resume-token-xyz".to_string()));
}

#[test]
fn test_session_can_resume() {
    let mut session = ClaudeCodeSession::new("sess-001", "/test/project");

    // Cannot resume without token
    assert!(!session.can_resume());

    // Set resume token
    session.set_resume_token("token");
    assert!(session.can_resume());

    // Cannot resume while running
    session.mark_running();
    assert!(!session.can_resume());

    // Can resume when idle
    session.mark_idle();
    assert!(session.can_resume());

    // Cannot resume with error
    session.mark_error("error");
    assert!(!session.can_resume());
}

#[test]
fn test_session_set_state() {
    let mut session = ClaudeCodeSession::new("sess-001", "/test/project");

    session.set_state(SessionState::Running);
    assert_eq!(session.state, SessionState::Running);

    session.set_state(SessionState::Waiting);
    assert_eq!(session.state, SessionState::Waiting);

    session.set_state(SessionState::Idle);
    assert_eq!(session.state, SessionState::Idle);
}

#[test]
fn test_session_serialization_roundtrip() {
    let mut session = ClaudeCodeSession::new("sess-001", "/test/project");
    session.set_resume_token("token-123");
    session.increment_messages();

    let json = serde_json::to_string(&session).unwrap();
    let deserialized: ClaudeCodeSession = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "sess-001");
    assert_eq!(deserialized.project_path, "/test/project");
    assert_eq!(deserialized.resume_token, Some("token-123".to_string()));
    assert_eq!(deserialized.message_count, 1);
}

// ============================================================================
// ChatMessage Tests
// ============================================================================

#[test]
fn test_chat_message_serialization() {
    let message = ChatMessage {
        role: "user".to_string(),
        content: "Hello, Claude!".to_string(),
        timestamp: "2024-01-15T10:00:00Z".to_string(),
        tool_calls: None,
    };

    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains("\"role\":\"user\""));
    assert!(json.contains("Hello, Claude!"));

    let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.role, "user");
    assert_eq!(deserialized.content, "Hello, Claude!");
}

#[test]
fn test_chat_message_with_tool_calls() {
    let tool_calls = vec![
        ToolCallSummary {
            tool_name: "Read".to_string(),
            success: true,
            summary: Some("Read file.txt".to_string()),
        },
        ToolCallSummary {
            tool_name: "Write".to_string(),
            success: false,
            summary: Some("Failed to write".to_string()),
        },
    ];

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "I'll help with that.".to_string(),
        timestamp: "2024-01-15T10:00:00Z".to_string(),
        tool_calls: Some(tool_calls),
    };

    assert!(message.tool_calls.is_some());
    let tools = message.tool_calls.as_ref().unwrap();
    assert_eq!(tools.len(), 2);
    assert!(tools[0].success);
    assert!(!tools[1].success);
}

// ============================================================================
// ToolCallSummary Tests
// ============================================================================

#[test]
fn test_tool_call_summary_serialization() {
    let summary = ToolCallSummary {
        tool_name: "Glob".to_string(),
        success: true,
        summary: Some("Found 5 files".to_string()),
    };

    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("\"tool_name\":\"Glob\""));
    assert!(json.contains("\"success\":true"));

    let deserialized: ToolCallSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.tool_name, "Glob");
    assert!(deserialized.success);
    assert_eq!(deserialized.summary, Some("Found 5 files".to_string()));
}

#[test]
fn test_tool_call_summary_without_summary() {
    let summary = ToolCallSummary {
        tool_name: "Bash".to_string(),
        success: true,
        summary: None,
    };

    assert!(summary.summary.is_none());

    let json = serde_json::to_string(&summary).unwrap();
    let deserialized: ToolCallSummary = serde_json::from_str(&json).unwrap();
    assert!(deserialized.summary.is_none());
}

// ============================================================================
// StartChatRequest Tests
// ============================================================================

#[test]
fn test_start_chat_request_minimal() {
    let request = StartChatRequest {
        project_path: "/test/project".to_string(),
        model: None,
        resume_session_id: None,
    };

    assert_eq!(request.project_path, "/test/project");
    assert!(request.model.is_none());
    assert!(request.resume_session_id.is_none());
}

#[test]
fn test_start_chat_request_with_model() {
    let request = StartChatRequest {
        project_path: "/test/project".to_string(),
        model: Some("claude-3-5-sonnet".to_string()),
        resume_session_id: None,
    };

    assert_eq!(request.model, Some("claude-3-5-sonnet".to_string()));
}

#[test]
fn test_start_chat_request_with_resume() {
    let request = StartChatRequest {
        project_path: "/test/project".to_string(),
        model: None,
        resume_session_id: Some("sess-old-001".to_string()),
    };

    assert_eq!(request.resume_session_id, Some("sess-old-001".to_string()));
}

#[test]
fn test_start_chat_request_serialization() {
    let request = StartChatRequest {
        project_path: "/test/project".to_string(),
        model: Some("claude-3-5-sonnet".to_string()),
        resume_session_id: None,
    };

    let json = serde_json::to_string(&request).unwrap();
    let deserialized: StartChatRequest = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.project_path, "/test/project");
    assert_eq!(deserialized.model, Some("claude-3-5-sonnet".to_string()));
}

// ============================================================================
// StartChatResponse Tests
// ============================================================================

#[test]
fn test_start_chat_response() {
    let response = StartChatResponse {
        session_id: "sess-new-001".to_string(),
        is_resumed: false,
    };

    assert_eq!(response.session_id, "sess-new-001");
    assert!(!response.is_resumed);
}

#[test]
fn test_start_chat_response_resumed() {
    let response = StartChatResponse {
        session_id: "sess-old-001".to_string(),
        is_resumed: true,
    };

    assert!(response.is_resumed);
}

// ============================================================================
// SendMessageRequest Tests
// ============================================================================

#[test]
fn test_send_message_request() {
    let request = SendMessageRequest {
        session_id: "sess-001".to_string(),
        prompt: "Please help me with this code".to_string(),
    };

    assert_eq!(request.session_id, "sess-001");
    assert_eq!(request.prompt, "Please help me with this code");
}

#[test]
fn test_send_message_request_serialization() {
    let request = SendMessageRequest {
        session_id: "sess-001".to_string(),
        prompt: "Test prompt".to_string(),
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"session_id\":\"sess-001\""));
    assert!(json.contains("\"prompt\":\"Test prompt\""));

    let deserialized: SendMessageRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.session_id, "sess-001");
    assert_eq!(deserialized.prompt, "Test prompt");
}

// ============================================================================
// ActiveSessionInfo Tests
// ============================================================================

#[test]
fn test_active_session_info() {
    let session = ClaudeCodeSession::new("sess-001", "/test/project");
    let info = ActiveSessionInfo {
        session,
        pid: Some(12345),
        is_process_alive: true,
    };

    assert_eq!(info.session.id, "sess-001");
    assert_eq!(info.pid, Some(12345));
    assert!(info.is_process_alive);
}

#[test]
fn test_active_session_info_without_pid() {
    let session = ClaudeCodeSession::new("sess-001", "/test/project");
    let info = ActiveSessionInfo {
        session,
        pid: None,
        is_process_alive: false,
    };

    assert!(info.pid.is_none());
    assert!(!info.is_process_alive);
}

#[test]
fn test_active_session_info_serialization() {
    let session = ClaudeCodeSession::new("sess-001", "/test/project");
    let info = ActiveSessionInfo {
        session,
        pid: Some(12345),
        is_process_alive: true,
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"pid\":12345"));
    assert!(json.contains("\"is_process_alive\":true"));
}
