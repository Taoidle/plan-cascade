//! Session Commands
//!
//! Tauri commands for session management.

use crate::models::response::CommandResponse;
use crate::models::session::{ResumeResult, Session, SessionDetails};
use crate::services::session::SessionService;

/// List all sessions for a project
#[tauri::command]
pub fn list_sessions(project_path: String) -> Result<CommandResponse<Vec<Session>>, String> {
    let service = SessionService::new();

    match service.list_sessions(&project_path) {
        Ok(sessions) => Ok(CommandResponse::ok(sessions)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get detailed session information
#[tauri::command]
pub fn get_session(session_path: String) -> Result<CommandResponse<SessionDetails>, String> {
    let service = SessionService::new();

    match service.get_session(&session_path) {
        Ok(details) => Ok(CommandResponse::ok(details)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Prepare to resume a session
#[tauri::command]
pub fn resume_session(session_path: String) -> Result<CommandResponse<ResumeResult>, String> {
    let service = SessionService::new();

    match service.resume_session(&session_path) {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Search sessions within a project
#[tauri::command]
pub fn search_sessions(
    project_path: String,
    query: String,
) -> Result<CommandResponse<Vec<Session>>, String> {
    let service = SessionService::new();

    match service.search_sessions(&project_path, &query) {
        Ok(sessions) => Ok(CommandResponse::ok(sessions)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_sessions_nonexistent() {
        let result = list_sessions("/nonexistent/path".to_string()).unwrap();
        assert!(result.success);
        assert!(result.data.map(|d| d.is_empty()).unwrap_or(true));
    }

    #[test]
    fn test_resume_session_nonexistent() {
        let result = resume_session("/nonexistent/session.jsonl".to_string()).unwrap();
        assert!(result.success);
        // The result itself shows failure for nonexistent file
        if let Some(resume_result) = result.data {
            assert!(!resume_result.success);
        }
    }
}
