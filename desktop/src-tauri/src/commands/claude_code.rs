//! Claude Code Commands
//!
//! Tauri commands for Claude Code CLI integration.

use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::claude_code::{
    ActiveSessionInfo, ClaudeCodeSession, SendMessageRequest, StartChatRequest, StartChatResponse,
};
use crate::models::response::CommandResponse;
use crate::services::claude_code::{ActiveSessionManager, ChatHandler};

/// State for Claude Code services
pub struct ClaudeCodeState {
    pub session_manager: Arc<ActiveSessionManager>,
    pub chat_handler: Arc<RwLock<ChatHandler>>,
}

impl ClaudeCodeState {
    pub fn new() -> Self {
        let session_manager = Arc::new(ActiveSessionManager::new());
        let chat_handler = Arc::new(RwLock::new(ChatHandler::new(session_manager.clone())));
        Self {
            session_manager,
            chat_handler,
        }
    }
}

impl Default for ClaudeCodeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Start a new Claude Code chat session
///
/// Creates a new session associated with the given project path.
/// Optionally specify a model or resume an existing session.
#[tauri::command]
pub async fn start_chat(
    request: StartChatRequest,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<StartChatResponse>, String> {
    let is_resume = request.resume_session_id.is_some();

    let session = if let Some(ref resume_id) = request.resume_session_id {
        // Resume existing session
        match state
            .session_manager
            .resume_session(resume_id, &request.project_path)
            .await
        {
            Ok(s) => s,
            Err(e) => return Ok(CommandResponse::err(e.to_string())),
        }
    } else {
        // Start new session
        match state
            .session_manager
            .start_session(&request.project_path, request.model.as_deref())
            .await
        {
            Ok(s) => s,
            Err(e) => return Ok(CommandResponse::err(e.to_string())),
        }
    };

    Ok(CommandResponse::ok(StartChatResponse {
        session_id: session.id,
        is_resumed: is_resume,
    }))
}

/// Send a message to a Claude Code session
///
/// This triggers the streaming response. The actual stream events
/// are emitted through Tauri events, not through this command's return value.
#[tauri::command]
pub async fn send_message(
    request: SendMessageRequest,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<bool>, String> {
    let mut chat_handler = state.chat_handler.write().await;

    match chat_handler.send_message(&request.session_id, &request.prompt).await {
        Ok(_rx) => {
            // The receiver is returned for internal use
            // Events will be emitted through the event system (story-007)
            Ok(CommandResponse::ok(true))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Cancel the current execution in a session
///
/// Kills the running process and marks the session as cancelled.
#[tauri::command]
pub async fn cancel_execution(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<bool>, String> {
    match state.session_manager.cancel_session(&session_id).await {
        Ok(_) => Ok(CommandResponse::ok(true)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get the history/details for a session
///
/// Returns session metadata including message count and state.
#[tauri::command]
pub async fn get_session_history(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<ClaudeCodeSession>, String> {
    match state.session_manager.get_session(&session_id).await {
        Some(session) => Ok(CommandResponse::ok(session)),
        None => Ok(CommandResponse::err(format!(
            "Session not found: {}",
            session_id
        ))),
    }
}

/// List all active Claude Code sessions
///
/// Returns information about all sessions including their process status.
#[tauri::command]
pub async fn list_active_sessions(
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<Vec<ActiveSessionInfo>>, String> {
    let sessions = state.session_manager.list_session_infos().await;
    Ok(CommandResponse::ok(sessions))
}

/// Remove a session completely
///
/// Cancels any running process and removes the session from memory.
#[tauri::command]
pub async fn remove_session(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<bool>, String> {
    match state.session_manager.remove_session(&session_id).await {
        Ok(_) => Ok(CommandResponse::ok(true)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get information about a specific session including process status
#[tauri::command]
pub async fn get_session_info(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<ActiveSessionInfo>, String> {
    match state.session_manager.get_session_info(&session_id).await {
        Some(info) => Ok(CommandResponse::ok(info)),
        None => Ok(CommandResponse::err(format!(
            "Session not found: {}",
            session_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_code_state_creation() {
        let state = ClaudeCodeState::new();
        // Verify state is created without panic
        let _ = state;
    }

    #[tokio::test]
    async fn test_start_chat_request() {
        let request = StartChatRequest {
            project_path: "/test/project".to_string(),
            model: Some("claude-sonnet-4-20250514".to_string()),
            resume_session_id: None,
        };

        assert_eq!(request.project_path, "/test/project");
        assert_eq!(request.model, Some("claude-sonnet-4-20250514".to_string()));
        assert!(request.resume_session_id.is_none());
    }

    #[tokio::test]
    async fn test_send_message_request() {
        let request = SendMessageRequest {
            session_id: "test-session".to_string(),
            prompt: "Hello, Claude!".to_string(),
        };

        assert_eq!(request.session_id, "test-session");
        assert_eq!(request.prompt, "Hello, Claude!");
    }
}
