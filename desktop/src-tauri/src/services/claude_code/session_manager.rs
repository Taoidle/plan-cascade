//! Claude Code Session Manager
//!
//! Manages active Claude Code sessions, including lifecycle and state tracking.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::models::claude_code::{ActiveSessionInfo, ClaudeCodeSession, SessionState};
use crate::utils::error::{AppError, AppResult};

use super::executor::{ClaudeCodeExecutor, ClaudeCodeProcess, SpawnConfig};

/// Active Claude execution bound to a session.
#[derive(Clone)]
pub struct ActiveExecution {
    /// Unique execution ID for this run
    pub execution_id: String,
    /// Process handle for termination and status checks
    pub process: Arc<Mutex<ClaudeCodeProcess>>,
    /// Cancellation token consumed by stream forwarding tasks
    pub cancel_token: CancellationToken,
}

/// Manages active Claude Code sessions
pub struct ActiveSessionManager {
    /// Map of session ID to session data
    sessions: Arc<RwLock<HashMap<String, ClaudeCodeSession>>>,
    /// Map of session ID to active execution details
    executions: Arc<RwLock<HashMap<String, ActiveExecution>>>,
    /// The executor for spawning processes
    executor: ClaudeCodeExecutor,
}

impl ActiveSessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            executions: Arc::new(RwLock::new(HashMap::new())),
            executor: ClaudeCodeExecutor::new(),
        }
    }

    /// Generate a unique session ID
    fn generate_session_id() -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
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

    /// Spawn a process for a session (legacy helper).
    pub async fn spawn_process(&self, session_id: &str) -> AppResult<String> {
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
        let execution_id = uuid::Uuid::new_v4().to_string();
        self.register_execution(session_id, &execution_id, process)
            .await?;

        // Log the process spawn (using eprintln for now, can be replaced with proper logging)
        eprintln!(
            "[INFO] Spawned Claude Code process for session {} (execution {})",
            session_id, execution_id
        );

        Ok(execution_id)
    }

    /// Register an active execution process for a session.
    pub async fn register_execution(
        &self,
        session_id: &str,
        execution_id: &str,
        process: ClaudeCodeProcess,
    ) -> AppResult<CancellationToken> {
        // Ensure session exists
        {
            let sessions = self.sessions.read().await;
            if !sessions.contains_key(session_id) {
                return Err(AppError::not_found(format!(
                    "Session not found: {}",
                    session_id
                )));
            }
        }

        let existing_execution = {
            let executions = self.executions.read().await;
            executions
                .get(session_id)
                .map(|active| active.execution_id.clone())
        };
        if let Some(existing_execution_id) = existing_execution {
            return Err(AppError::validation(format!(
                "Session {} already has active execution {}",
                session_id, existing_execution_id
            )));
        }

        let cancel_token = CancellationToken::new();
        let process_arc = Arc::new(Mutex::new(process));

        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.mark_running();
            }
        }

        {
            let mut executions = self.executions.write().await;
            executions.insert(
                session_id.to_string(),
                ActiveExecution {
                    execution_id: execution_id.to_string(),
                    process: process_arc,
                    cancel_token: cancel_token.clone(),
                },
            );
        }

        Ok(cancel_token)
    }

    /// Get active execution metadata for a session.
    pub async fn get_active_execution(&self, session_id: &str) -> Option<ActiveExecution> {
        let executions = self.executions.read().await;
        executions.get(session_id).cloned()
    }

    /// Remove an execution if it matches the current active execution ID.
    pub async fn complete_execution(&self, session_id: &str, execution_id: &str) {
        let mut executions = self.executions.write().await;
        if let Some(active) = executions.get(session_id) {
            if active.execution_id == execution_id {
                executions.remove(session_id);
            }
        }
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

        let active = {
            let executions = self.executions.read().await;
            executions.get(session_id).cloned()
        };
        let (pid, is_alive) = if let Some(active) = active {
            let process = active.process.lock().await;
            (Some(process.pid()), process.is_running())
        } else {
            (None, false)
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
        let executions = self.executions.read().await.clone();
        let mut infos = Vec::with_capacity(sessions.len());

        for (id, session) in sessions.iter() {
            let (pid, is_alive) = if let Some(active) = executions.get(id).cloned() {
                let process = active.process.lock().await;
                (Some(process.pid()), process.is_running())
            } else {
                (None, false)
            };

            infos.push(ActiveSessionInfo {
                session: session.clone(),
                pid,
                is_process_alive: is_alive,
            });
        }

        infos
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

    /// Cancel an active execution for a session.
    ///
    /// Returns the cancelled execution ID on success.
    pub async fn cancel_session(&self, session_id: &str) -> AppResult<String> {
        let active = {
            let mut executions = self.executions.write().await;
            executions.remove(session_id)
        };

        let active = active.ok_or_else(|| {
            AppError::not_found(format!("No active execution for session {}", session_id))
        })?;

        active.cancel_token.cancel();
        {
            let mut process = active.process.lock().await;
            process.kill().await?;
        }

        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.mark_cancelled();
            }
        }

        Ok(active.execution_id)
    }

    /// Remove a session completely
    pub async fn remove_session(&self, session_id: &str) -> AppResult<()> {
        // First cancel any running process
        match self.cancel_session(session_id).await {
            Ok(_) => {}
            Err(AppError::NotFound(_)) => {}
            Err(e) => return Err(e),
        }

        // Remove the session
        {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id);
        }

        Ok(())
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

        let result = manager.cancel_session(&session.id).await;
        match result {
            Err(AppError::NotFound(message)) => {
                assert!(message.contains("No active execution"));
                assert!(message.contains(&session.id));
            }
            other => panic!("Expected NotFound error, got {:?}", other),
        }

        let updated = manager.get_session(&session.id).await.unwrap();
        assert_eq!(updated.state, SessionState::Idle);
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
