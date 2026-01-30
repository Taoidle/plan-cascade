//! Session Service
//!
//! Parses and manages Claude Code session files (JSONL format)

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use serde::Deserialize;

use crate::models::session::{MessageSummary, ResumeResult, Session, SessionDetails};
use crate::utils::error::{AppError, AppResult};

/// Maximum length for message previews
const PREVIEW_MAX_LEN: usize = 100;

/// Session file entry (simplified for parsing)
#[derive(Debug, Deserialize)]
struct SessionEntry {
    #[serde(rename = "type")]
    entry_type: Option<String>,
    role: Option<String>,
    content: Option<serde_json::Value>,
    timestamp: Option<String>,
    #[serde(default)]
    is_checkpoint: bool,
}

/// Service for managing Claude Code sessions
#[derive(Debug, Default)]
pub struct SessionService;

impl SessionService {
    /// Create a new session service
    pub fn new() -> Self {
        Self
    }

    /// List all sessions for a project
    pub fn list_sessions(&self, project_path: &str) -> AppResult<Vec<Session>> {
        let sessions_dir = PathBuf::from(project_path).join("sessions");

        if !sessions_dir.exists() {
            return Ok(vec![]);
        }

        let mut sessions = Vec::new();
        let project_id = PathBuf::from(project_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let entries = fs::read_dir(&sessions_dir)
            .map_err(|e| AppError::Io(e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Some(session) = self.parse_session_file(&path, &project_id) {
                    sessions.push(session);
                }
            }
        }

        // Sort by last activity (most recent first)
        sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        Ok(sessions)
    }

    /// Parse a session JSONL file and extract metadata
    fn parse_session_file(&self, path: &PathBuf, project_id: &str) -> Option<Session> {
        let file = File::open(path).ok()?;
        let reader = BufReader::new(file);

        let session_id = path
            .file_stem()?
            .to_string_lossy()
            .to_string();

        let mut session = Session::new(
            session_id,
            project_id.to_string(),
            path.to_string_lossy().to_string(),
        );

        let mut first_user_message: Option<String> = None;
        let mut last_timestamp: Option<String> = None;
        let mut first_timestamp: Option<String> = None;

        for line in reader.lines().take(1000) { // Limit for performance
            if let Ok(line_str) = line {
                if let Ok(entry) = serde_json::from_str::<SessionEntry>(&line_str) {
                    session.message_count += 1;

                    // Track timestamps
                    if let Some(ts) = &entry.timestamp {
                        if first_timestamp.is_none() {
                            first_timestamp = Some(ts.clone());
                        }
                        last_timestamp = Some(ts.clone());
                    }

                    // Get first user message
                    if first_user_message.is_none() {
                        if entry.role.as_deref() == Some("user") || entry.entry_type.as_deref() == Some("user") {
                            first_user_message = self.extract_content_preview(&entry.content);
                        }
                    }

                    // Count checkpoints
                    if entry.is_checkpoint || entry.entry_type.as_deref() == Some("checkpoint") {
                        session.checkpoint_count += 1;
                    }
                }
            }
        }

        session.created_at = first_timestamp;
        session.last_activity = last_timestamp.or_else(|| {
            // Fall back to file modification time
            fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let datetime: chrono::DateTime<chrono::Utc> = t.into();
                    datetime.to_rfc3339()
                })
        });
        session.first_message_preview = first_user_message;

        Some(session)
    }

    /// Extract a preview from content value
    fn extract_content_preview(&self, content: &Option<serde_json::Value>) -> Option<String> {
        let content = content.as_ref()?;

        let text = if let Some(s) = content.as_str() {
            s.to_string()
        } else if let Some(arr) = content.as_array() {
            // Handle array of content blocks
            arr.iter()
                .filter_map(|v| {
                    if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
                        Some(text.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            return None;
        };

        if text.is_empty() {
            return None;
        }

        // Truncate to preview length
        let preview = if text.len() > PREVIEW_MAX_LEN {
            format!("{}...", &text[..PREVIEW_MAX_LEN])
        } else {
            text
        };

        Some(preview.replace('\n', " ").trim().to_string())
    }

    /// Get detailed session information
    pub fn get_session(&self, session_path: &str) -> AppResult<SessionDetails> {
        let path = PathBuf::from(session_path);

        if !path.exists() {
            return Err(AppError::not_found(format!("Session file not found: {}", session_path)));
        }

        let project_id = path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let session = self.parse_session_file(&path, &project_id)
            .ok_or_else(|| AppError::internal("Failed to parse session file"))?;

        // Parse messages for details
        let messages = self.parse_session_messages(&path)?;

        Ok(SessionDetails { session, messages })
    }

    /// Parse session messages for detail view
    fn parse_session_messages(&self, path: &PathBuf) -> AppResult<Vec<MessageSummary>> {
        let file = File::open(path).map_err(|e| AppError::Io(e))?;
        let reader = BufReader::new(file);

        let mut messages = Vec::new();

        for line in reader.lines().take(500) { // Limit for performance
            if let Ok(line_str) = line {
                if let Ok(entry) = serde_json::from_str::<SessionEntry>(&line_str) {
                    let message_type = entry.role
                        .or(entry.entry_type)
                        .unwrap_or_else(|| "unknown".to_string());

                    messages.push(MessageSummary {
                        message_type,
                        preview: self.extract_content_preview(&entry.content),
                        timestamp: entry.timestamp,
                        is_checkpoint: entry.is_checkpoint,
                    });
                }
            }
        }

        Ok(messages)
    }

    /// Prepare to resume a session
    pub fn resume_session(&self, session_path: &str) -> AppResult<ResumeResult> {
        let path = PathBuf::from(session_path);

        if !path.exists() {
            return Ok(ResumeResult::failure(
                session_path.to_string(),
                "Session file not found",
            ));
        }

        let session_id = path
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let project_path = path
            .parent() // sessions/
            .and_then(|p| p.parent()) // project/
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        Ok(ResumeResult::success(
            session_id,
            session_path.to_string(),
            project_path,
        ))
    }

    /// Search sessions across a project
    pub fn search_sessions(&self, project_path: &str, query: &str) -> AppResult<Vec<Session>> {
        let sessions = self.list_sessions(project_path)?;
        let query_lower = query.to_lowercase();

        Ok(sessions
            .into_iter()
            .filter(|s| {
                s.first_message_preview
                    .as_ref()
                    .map(|p| p.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
                    || s.id.to_lowercase().contains(&query_lower)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = SessionService::new();
        let _ = service;
    }

    #[test]
    fn test_extract_content_preview_string() {
        let service = SessionService::new();

        let content = Some(serde_json::json!("Hello world"));
        let preview = service.extract_content_preview(&content);
        assert_eq!(preview, Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_content_preview_long() {
        let service = SessionService::new();

        let long_text = "a".repeat(200);
        let content = Some(serde_json::json!(long_text));
        let preview = service.extract_content_preview(&content);
        assert!(preview.unwrap().ends_with("..."));
    }

    #[test]
    fn test_extract_content_preview_array() {
        let service = SessionService::new();

        let content = Some(serde_json::json!([
            {"type": "text", "text": "Hello"},
            {"type": "text", "text": "World"}
        ]));
        let preview = service.extract_content_preview(&content);
        assert_eq!(preview, Some("Hello World".to_string()));
    }

    #[test]
    fn test_resume_result() {
        let service = SessionService::new();

        // Non-existent file should return failure
        let result = service.resume_session("/nonexistent/path.jsonl").unwrap();
        assert!(!result.success);
    }
}
