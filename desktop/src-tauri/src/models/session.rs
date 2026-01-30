//! Session Models
//!
//! Data structures for Claude Code sessions.

use serde::{Deserialize, Serialize};

/// A Claude Code session within a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    pub id: String,
    /// Parent project identifier
    pub project_id: String,
    /// Session creation timestamp (ISO 8601)
    pub created_at: Option<String>,
    /// Last activity timestamp (ISO 8601)
    pub last_activity: Option<String>,
    /// Total message count in this session
    pub message_count: u32,
    /// Number of checkpoints in this session
    pub checkpoint_count: u32,
    /// Preview of the first user message (truncated)
    pub first_message_preview: Option<String>,
    /// Full path to the session JSONL file
    pub file_path: String,
}

impl Session {
    /// Create a new session with minimal info
    pub fn new(id: impl Into<String>, project_id: impl Into<String>, file_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            project_id: project_id.into(),
            created_at: None,
            last_activity: None,
            message_count: 0,
            checkpoint_count: 0,
            first_message_preview: None,
            file_path: file_path.into(),
        }
    }
}

/// Detailed session information including messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetails {
    /// Basic session info
    #[serde(flatten)]
    pub session: Session,
    /// Message summaries (not full content for performance)
    pub messages: Vec<MessageSummary>,
}

/// Summary of a message in a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSummary {
    /// Message type: user, assistant, tool_call, tool_result
    pub message_type: String,
    /// Truncated content preview
    pub preview: Option<String>,
    /// Timestamp if available
    pub timestamp: Option<String>,
    /// Whether this is a checkpoint marker
    pub is_checkpoint: bool,
}

/// Result of attempting to resume a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeResult {
    /// Session ID being resumed
    pub session_id: String,
    /// Full path to session file
    pub session_path: String,
    /// Project path for context
    pub project_path: String,
    /// Whether resume preparation was successful
    pub success: bool,
    /// Error message if not successful
    pub error: Option<String>,
}

impl ResumeResult {
    /// Create a successful resume result
    pub fn success(session_id: String, session_path: String, project_path: String) -> Self {
        Self {
            session_id,
            session_path,
            project_path,
            success: true,
            error: None,
        }
    }

    /// Create a failed resume result
    pub fn failure(session_id: String, error: impl Into<String>) -> Self {
        Self {
            session_id,
            session_path: String::new(),
            project_path: String::new(),
            success: false,
            error: Some(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::new("sess1", "proj1", "/path/to/session.jsonl");
        assert_eq!(session.id, "sess1");
        assert_eq!(session.project_id, "proj1");
        assert_eq!(session.message_count, 0);
    }

    #[test]
    fn test_resume_result() {
        let success = ResumeResult::success("s1".into(), "/path".into(), "/proj".into());
        assert!(success.success);
        assert!(success.error.is_none());

        let failure = ResumeResult::failure("s1".into(), "File not found");
        assert!(!failure.success);
        assert_eq!(failure.error, Some("File not found".to_string()));
    }

    #[test]
    fn test_session_serialization() {
        let session = Session::new("sess1", "proj1", "/test.jsonl");
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"id\":\"sess1\""));
    }
}
