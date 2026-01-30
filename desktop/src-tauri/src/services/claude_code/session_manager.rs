//! Claude Code Session Manager
//!
//! Manages active Claude Code sessions, including lifecycle and state tracking.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::claude_code::{ActiveSessionInfo, ClaudeCodeSession, SessionState};
use crate::utils::error::{AppError, AppResult};

use super::executor::{ClaudeCodeExecutor, ClaudeCodeProcess, SpawnConfig};

/// Manages active Claude Code sessions
pub struct ActiveSessionManager {
    /// Map of session ID to session data
    sessions: Arc<RwLock<HashMap<String, ClaudeCodeSession>>>,
    /// Map of session ID to running process
    processes: Arc<RwLock<HashMap<String, ClaudeCodeProcess>>>,
    /// The executor for spawning processes
    executor: ClaudeCodeExecutor,
}

impl ActiveSessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            processes: Arc::new(RwLock::new(HashMap::new())),
            executor: ClaudeCodeExecutor::new(),
        }
    }

    /// Generate a unique session ID
    fn generate_session_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        format!("claude-{:x}-{:x}-{:x}", timestamp as u32, pid, counter)
    }

    /// Start a new session
    pub async fn start_session(
        &self,
        project_path: &str,
        model: Option<&str>,
    ) -> AppResult<ClaudeCodeSession> {
        let session_id = Self::generate_session_id();
        let mut session = ClaudeCodeSession::new(&session_id, project_path);

        if let Some(m) = model {
            session = session.with_model(m);
        }

        // Store the session
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), session.clone());
        }

        Ok(session)
    }

    /// Resume an existing session
    pub async fn resume_session(
        &self,
        session_id: &str,
        project_path: &str,
    ) -> AppResult<ClaudeCodeSession> {
        // Check if session already exists in memory
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(session_id) {
                if session.can_resume() {
                    return Ok(session.clone());
                } else if session.state == SessionState::Running {
                    return Err(AppError::validation("Session is already running"));
                }
            }
        }

        // Create a new session with the provided ID for resumption
        let mut session = ClaudeCodeSession::new(session_id, project_path);
        session.set_resume_token(session_id);

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.to_string(), session.clone());
        }

        Ok(session)
    }

    /// Spawn a process for a session
    pub async fn spawn_process(&self, session_id: &str) -> AppResult<()> {
        let session = {
            let sessions = self.sessions.read().await;
            sessions.get(session_id).cloned()
        }
        .ok_or_else(|| AppError::not_found(format!("Session not found: {}", session_id)))?;

        let mut config = SpawnConfig::new(&session.project_path);

        if let Some(ref model) = session.model {
            config = config.with_model(model);
        }

        if let Some(ref resume_token) = session.resume_token {
            config = config.with_resume(resume_token);
        }

        let process = self.executor.spawn(&config).await?;
        let pid = process.pid();

        // Store the process
        {
            let mut processes = self.processes.write().await;
            processes.insert(session_id.to_string(), process);
        }

        // Update session state
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.mark_running();
            }
        }

        // Log the process spawn (using eprintln for now, can be replaced with proper logging)
        eprintln!("[INFO] Spawned Claude Code process {} for session {}", pid, session_id);

        Ok(())
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<ClaudeCodeSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Get session info including process status
    pub async fn get_session_info(&self, session_id: &str) -> Option<ActiveSessionInfo> {
        let session = {
            let sessions = self.sessions.read().await;
            sessions.get(session_id).cloned()
        }?;

        let (pid, is_alive) = {
            let processes = self.processes.read().await;
            if let Some(process) = processes.get(session_id) {
                (Some(process.pid()), process.is_running())
            } else {
                (None, false)
            }
        };

        Some(ActiveSessionInfo {
            session,
            pid,
            is_process_alive: is_alive,
        })
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Vec<ClaudeCodeSession> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    /// List all active session infos
    pub async fn list_session_infos(&self) -> Vec<ActiveSessionInfo> {
        let sessions = self.sessions.read().await;
        let processes = self.processes.read().await;

        sessions
            .iter()
            .map(|(id, session)| {
                let (pid, is_alive) = if let Some(process) = processes.get(id) {
                    (Some(process.pid()), process.is_running())
                } else {
                    (None, false)
                };

                ActiveSessionInfo {
                    session: session.clone(),
                    pid,
                    is_process_alive: is_alive,
                }
            })
            .collect()
    }

    /// Update session state
    pub async fn update_session_state(
        &self,
        session_id: &str,
        state: SessionState,
    ) -> AppResult<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::not_found(format!("Session not found: {}", session_id)))?;

        session.set_state(state);
        Ok(())
    }

    /// Mark session as having an error
    pub async fn mark_session_error(&self, session_id: &str, message: &str) -> AppResult<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::not_found(format!("Session not found: {}", session_id)))?;

        session.mark_error(message);
        Ok(())
    }

    /// Increment message count for a session
    pub async fn increment_message_count(&self, session_id: &str) -> AppResult<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::not_found(format!("Session not found: {}", session_id)))?;

        session.increment_messages();
        Ok(())
    }

    /// Set resume token for a session
    pub async fn set_resume_token(&self, session_id: &str, token: &str) -> AppResult<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::not_found(format!("Session not found: {}", session_id)))?;

        session.set_resume_token(token);
        Ok(())
    }

    /// Cancel a session's execution
    pub async fn cancel_session(&self, session_id: &str) -> AppResult<()> {
        // Kill the process if it exists
        {
            let mut processes = self.processes.write().await;
            if let Some(mut process) = processes.remove(session_id) {
                process.kill().await?;
            }
        }

        // Update session state
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.mark_cancelled();
            }
        }

        Ok(())
    }

    /// Remove a session completely
    pub async fn remove_session(&self, session_id: &str) -> AppResult<()> {
        // First cancel any running process
        self.cancel_session(session_id).await?;

        // Remove the session
        {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id);
        }

        Ok(())
    }

    /// Take the process for a session (for direct manipulation)
    pub async fn take_process(&self, session_id: &str) -> Option<ClaudeCodeProcess> {
        let mut processes = self.processes.write().await;
        processes.remove(session_id)
    }

    /// Return a process to the manager
    pub async fn return_process(&self, session_id: &str, process: ClaudeCodeProcess) {
        let mut processes = self.processes.write().await;
        processes.insert(session_id.to_string(), process);
    }

    /// Check if a session has a running process
    pub async fn has_running_process(&self, session_id: &str) -> bool {
        let processes = self.processes.read().await;
        processes.get(session_id).map(|p| p.is_running()).unwrap_or(false)
    }

    /// Clean up dead processes
    pub async fn cleanup_dead_processes(&self) {
        let dead_sessions: Vec<String> = {
            let processes = self.processes.read().await;
            processes
                .iter()
                .filter(|(_, p)| !p.is_running())
                .map(|(id, _)| id.clone())
                .collect()
        };

        for session_id in dead_sessions {
            // Remove the dead process
            {
                let mut processes = self.processes.write().await;
                processes.remove(&session_id);
            }

            // Update session state to idle
            {
                let mut sessions = self.sessions.write().await;
                if let Some(session) = sessions.get_mut(&session_id) {
                    if session.state == SessionState::Running {
                        session.mark_idle();
                    }
                }
            }
        }
    }
}

impl Default for ActiveSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_manager_creation() {
        let manager = ActiveSessionManager::new();
        let sessions = manager.list_sessions().await;
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_start_session() {
        let manager = ActiveSessionManager::new();
        let session = manager.start_session("/project/path", None).await.unwrap();

        assert!(session.id.starts_with("claude-"));
        assert_eq!(session.project_path, "/project/path");
        assert_eq!(session.state, SessionState::Idle);

        // Session should be stored
        let stored = manager.get_session(&session.id).await;
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn test_start_session_with_model() {
        let manager = ActiveSessionManager::new();
        let session = manager
            .start_session("/project/path", Some("claude-sonnet-4-20250514"))
            .await
            .unwrap();

        assert_eq!(session.model, Some("claude-sonnet-4-20250514".to_string()));
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let manager = ActiveSessionManager::new();

        manager.start_session("/project1", None).await.unwrap();
        manager.start_session("/project2", None).await.unwrap();

        let sessions = manager.list_sessions().await;
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_update_session_state() {
        let manager = ActiveSessionManager::new();
        let session = manager.start_session("/project", None).await.unwrap();

        manager
            .update_session_state(&session.id, SessionState::Running)
            .await
            .unwrap();

        let updated = manager.get_session(&session.id).await.unwrap();
        assert_eq!(updated.state, SessionState::Running);
    }

    #[tokio::test]
    async fn test_mark_session_error() {
        let manager = ActiveSessionManager::new();
        let session = manager.start_session("/project", None).await.unwrap();

        manager
            .mark_session_error(&session.id, "Test error")
            .await
            .unwrap();

        let updated = manager.get_session(&session.id).await.unwrap();
        assert_eq!(updated.state, SessionState::Error);
        assert_eq!(updated.error_message, Some("Test error".to_string()));
    }

    #[tokio::test]
    async fn test_cancel_session() {
        let manager = ActiveSessionManager::new();
        let session = manager.start_session("/project", None).await.unwrap();

        manager.cancel_session(&session.id).await.unwrap();

        let updated = manager.get_session(&session.id).await.unwrap();
        assert_eq!(updated.state, SessionState::Cancelled);
    }

    #[tokio::test]
    async fn test_remove_session() {
        let manager = ActiveSessionManager::new();
        let session = manager.start_session("/project", None).await.unwrap();

        manager.remove_session(&session.id).await.unwrap();

        let removed = manager.get_session(&session.id).await;
        assert!(removed.is_none());
    }

    #[tokio::test]
    async fn test_increment_message_count() {
        let manager = ActiveSessionManager::new();
        let session = manager.start_session("/project", None).await.unwrap();

        manager.increment_message_count(&session.id).await.unwrap();
        manager.increment_message_count(&session.id).await.unwrap();

        let updated = manager.get_session(&session.id).await.unwrap();
        assert_eq!(updated.message_count, 2);
    }

    #[tokio::test]
    async fn test_set_resume_token() {
        let manager = ActiveSessionManager::new();
        let session = manager.start_session("/project", None).await.unwrap();

        manager
            .set_resume_token(&session.id, "resume-token-123")
            .await
            .unwrap();

        let updated = manager.get_session(&session.id).await.unwrap();
        assert_eq!(updated.resume_token, Some("resume-token-123".to_string()));
        assert!(updated.can_resume());
    }
}
