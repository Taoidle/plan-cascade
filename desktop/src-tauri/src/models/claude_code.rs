//! Claude Code Models
//!
//! Data structures for Claude Code GUI integration.

use serde::{Deserialize, Serialize};

/// State of a Claude Code session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Session is idle, waiting for input
    Idle,
    /// Session is actively running (processing a request)
    Running,
    /// Session is waiting for user confirmation (tool approval, etc.)
    Waiting,
    /// Session was cancelled by user
    Cancelled,
    /// Session encountered an error
    Error,
}

impl Default for SessionState {
    fn default() -> Self {
        Self::Idle
    }
}

/// A Claude Code session for GUI mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCodeSession {
    /// Unique session identifier
    pub id: String,
    /// Project path this session is associated with
    pub project_path: String,
    /// Session creation timestamp (ISO 8601)
    pub created_at: String,
    /// Last message timestamp (ISO 8601)
    pub last_message_at: Option<String>,
    /// Claude Code session ID for resume support
    pub resume_token: Option<String>,
    /// Current state of the session
    pub state: SessionState,
    /// Model being used (if specified)
    pub model: Option<String>,
    /// Error message if state is Error
    pub error_message: Option<String>,
    /// Number of messages exchanged
    pub message_count: u32,
}

impl ClaudeCodeSession {
    /// Create a new Claude Code session
    pub fn new(id: impl Into<String>, project_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            project_path: project_path.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_message_at: None,
            resume_token: None,
            state: SessionState::Idle,
            model: None,
            error_message: None,
            message_count: 0,
        }
    }

    /// Set the model for this session
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Update the session state
    pub fn set_state(&mut self, state: SessionState) {
        self.state = state;
        if state == SessionState::Error {
            // Clear resume token on error
        }
    }

    /// Mark session as running
    pub fn mark_running(&mut self) {
        self.state = SessionState::Running;
        self.last_message_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark session as idle
    pub fn mark_idle(&mut self) {
        self.state = SessionState::Idle;
    }

    /// Mark session as having an error
    pub fn mark_error(&mut self, message: impl Into<String>) {
        self.state = SessionState::Error;
        self.error_message = Some(message.into());
    }

    /// Mark session as cancelled
    pub fn mark_cancelled(&mut self) {
        self.state = SessionState::Cancelled;
    }

    /// Increment message count
    pub fn increment_messages(&mut self) {
        self.message_count += 1;
        self.last_message_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Set the resume token
    pub fn set_resume_token(&mut self, token: impl Into<String>) {
        self.resume_token = Some(token.into());
    }

    /// Check if session can be resumed
    pub fn can_resume(&self) -> bool {
        self.resume_token.is_some()
            && self.state != SessionState::Error
            && self.state != SessionState::Running
    }
}

/// Information about an active session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSessionInfo {
    /// Session data
    pub session: ClaudeCodeSession,
    /// Process ID if running
    pub pid: Option<u32>,
    /// Whether the process is currently running
    pub is_process_alive: bool,
}

/// Request to start a new chat session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartChatRequest {
    /// Project path to start the chat in
    pub project_path: String,
    /// Optional model to use
    pub model: Option<String>,
    /// Optional session ID to resume
    pub resume_session_id: Option<String>,
}

/// Response when starting a chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartChatResponse {
    /// The session ID
    pub session_id: String,
    /// Whether this is a resumed session
    pub is_resumed: bool,
}

/// Request to send a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    /// Session ID to send the message to
    pub session_id: String,
    /// The message content
    pub prompt: String,
}

/// Chat message in history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Message role (user, assistant, system)
    pub role: String,
    /// Message content
    pub content: String,
    /// Timestamp (ISO 8601)
    pub timestamp: String,
    /// Optional tool calls in this message
    pub tool_calls: Option<Vec<ToolCallSummary>>,
}

/// Summary of a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallSummary {
    /// Tool name
    pub tool_name: String,
    /// Whether it succeeded
    pub success: bool,
    /// Brief description of what was done
    pub summary: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = ClaudeCodeSession::new("sess-123", "/path/to/project");
        assert_eq!(session.id, "sess-123");
        assert_eq!(session.project_path, "/path/to/project");
        assert_eq!(session.state, SessionState::Idle);
        assert_eq!(session.message_count, 0);
    }

    #[test]
    fn test_session_with_model() {
        let session =
            ClaudeCodeSession::new("sess-123", "/project").with_model("claude-sonnet-4-20250514");
        assert_eq!(session.model, Some("claude-sonnet-4-20250514".to_string()));
    }

    #[test]
    fn test_session_state_transitions() {
        let mut session = ClaudeCodeSession::new("sess-123", "/project");

        session.mark_running();
        assert_eq!(session.state, SessionState::Running);
        assert!(session.last_message_at.is_some());

        session.mark_idle();
        assert_eq!(session.state, SessionState::Idle);

        session.mark_error("Something went wrong");
        assert_eq!(session.state, SessionState::Error);
        assert_eq!(
            session.error_message,
            Some("Something went wrong".to_string())
        );
    }

    #[test]
    fn test_can_resume() {
        let mut session = ClaudeCodeSession::new("sess-123", "/project");
        assert!(!session.can_resume()); // No resume token

        session.set_resume_token("resume-token-xyz");
        assert!(session.can_resume());

        session.mark_running();
        assert!(!session.can_resume()); // Running state

        session.mark_idle();
        assert!(session.can_resume());

        session.mark_error("error");
        assert!(!session.can_resume()); // Error state
    }

    #[test]
    fn test_increment_messages() {
        let mut session = ClaudeCodeSession::new("sess-123", "/project");
        assert_eq!(session.message_count, 0);

        session.increment_messages();
        assert_eq!(session.message_count, 1);
        assert!(session.last_message_at.is_some());

        session.increment_messages();
        assert_eq!(session.message_count, 2);
    }

    #[test]
    fn test_session_serialization() {
        let session = ClaudeCodeSession::new("sess-123", "/project");
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"id\":\"sess-123\""));
        assert!(json.contains("\"state\":\"idle\""));
    }

    #[test]
    fn test_session_state_serialization() {
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
    }
}
