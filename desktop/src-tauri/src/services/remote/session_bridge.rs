//! Session Bridge
//!
//! Bridges remote commands to local session operations.
//! Full implementation in story-004.

use super::types::{RemoteError, RemoteResponse, RemoteSessionMapping};
use crate::storage::Database;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Bridges remote commands to local session operations.
///
/// Maintains mapping between remote chat IDs and local session IDs,
/// routes messages to appropriate session types (ClaudeCode or Standalone),
/// and collects streaming responses into final text results.
pub struct SessionBridge {
    /// Mapping: chat_id -> local session
    pub(crate) sessions: RwLock<HashMap<i64, RemoteSessionMapping>>,
    /// Database for persistence
    pub(crate) db: Arc<Database>,
}

impl SessionBridge {
    /// Create a new SessionBridge.
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            db,
        }
    }

    /// Get the number of active remote sessions.
    pub async fn active_session_count(&self) -> u32 {
        self.sessions.read().await.len() as u32
    }

    /// List all session mappings.
    pub async fn list_all_sessions(&self) -> Vec<RemoteSessionMapping> {
        self.sessions.read().await.values().cloned().collect()
    }

    /// Get formatted sessions text for a chat.
    pub async fn list_sessions_text(&self, _chat_id: i64) -> String {
        let sessions = self.sessions.read().await;
        if sessions.is_empty() {
            return "No active remote sessions.".to_string();
        }
        let mut text = "Active Remote Sessions:\n".to_string();
        for (cid, mapping) in sessions.iter() {
            text.push_str(&format!(
                "  Chat {} -> {} ({})\n",
                cid,
                mapping.local_session_id.as_deref().unwrap_or("no session"),
                mapping.session_type
            ));
        }
        text
    }

    /// Get status text for a chat.
    pub async fn get_status_text(&self, chat_id: i64) -> String {
        let sessions = self.sessions.read().await;
        match sessions.get(&chat_id) {
            Some(mapping) => {
                format!(
                    "Session: {}\nType: {}\nCreated: {}",
                    mapping.local_session_id.as_deref().unwrap_or("none"),
                    mapping.session_type,
                    mapping.created_at
                )
            }
            None => "No active session for this chat.".to_string(),
        }
    }

    /// Cancel execution for a chat's active session.
    pub async fn cancel_execution(&self, chat_id: i64) -> Result<(), RemoteError> {
        let sessions = self.sessions.read().await;
        if !sessions.contains_key(&chat_id) {
            return Err(RemoteError::NoActiveSession);
        }
        // Stub: full cancellation logic in story-004
        Ok(())
    }

    /// Close and remove session mapping for a chat.
    pub async fn close_session(&self, chat_id: i64) -> Result<(), RemoteError> {
        let mut sessions = self.sessions.write().await;
        sessions
            .remove(&chat_id)
            .ok_or(RemoteError::NoActiveSession)?;
        Ok(())
    }

    /// Switch active session for a chat.
    pub async fn switch_session(
        &self,
        _chat_id: i64,
        _session_id: &str,
    ) -> Result<(), RemoteError> {
        // Stub: full implementation in story-004
        Ok(())
    }

    /// Send message to the active session and collect the response.
    pub async fn send_message(
        &self,
        chat_id: i64,
        _content: &str,
    ) -> Result<RemoteResponse, RemoteError> {
        let sessions = self.sessions.read().await;
        if !sessions.contains_key(&chat_id) {
            return Err(RemoteError::NoActiveSession);
        }
        // Stub: full implementation in story-004
        Ok(RemoteResponse {
            text: "Message received (bridge not yet implemented)".to_string(),
            thinking: None,
            tool_summary: None,
        })
    }

    /// Create a new session for a remote chat.
    ///
    /// The `adapter_type_name` and `username` are stored with the mapping
    /// so the frontend can display the remote source (e.g., "via Telegram @user").
    pub async fn create_session(
        &self,
        chat_id: i64,
        user_id: i64,
        _project_path: &str,
        _provider: Option<&str>,
        _model: Option<&str>,
    ) -> Result<String, RemoteError> {
        self.create_session_with_source(
            chat_id,
            user_id,
            _project_path,
            _provider,
            _model,
            None,
            None,
        )
        .await
    }

    /// Create a new session with remote source tracking.
    pub async fn create_session_with_source(
        &self,
        chat_id: i64,
        user_id: i64,
        _project_path: &str,
        _provider: Option<&str>,
        _model: Option<&str>,
        adapter_type_name: Option<&str>,
        username: Option<&str>,
    ) -> Result<String, RemoteError> {
        use super::types::{RemoteSessionMapping, SessionType};

        let session_id = uuid::Uuid::new_v4().to_string();
        let mapping = RemoteSessionMapping {
            chat_id,
            user_id,
            local_session_id: Some(session_id.clone()),
            session_type: SessionType::ClaudeCode,
            created_at: chrono::Utc::now().to_rfc3339(),
            adapter_type_name: adapter_type_name.map(|s| s.to_string()),
            username: username.map(|s| s.to_string()),
        };
        self.sessions.write().await.insert(chat_id, mapping);
        Ok(session_id)
    }

    /// Get the active local session ID for a given chat.
    pub async fn get_active_session_id(&self, chat_id: i64) -> Option<String> {
        let sessions = self.sessions.read().await;
        sessions
            .get(&chat_id)
            .and_then(|m| m.local_session_id.clone())
    }

    /// Load session mappings from database on startup.
    pub async fn load_mappings_from_db(&self) -> Result<(), RemoteError> {
        // Collect all DB results into a Vec before any .await point.
        // rusqlite types (Connection, Statement, MappedRows) are not Send/Sync,
        // so they must be dropped before crossing an await boundary.
        let collected: Vec<RemoteSessionMapping> = {
            let conn = self.db.get_connection().map_err(|e| {
                RemoteError::ConfigError(format!("Failed to get database connection: {}", e))
            })?;

            let mut stmt = conn
                .prepare(
                    "SELECT chat_id, user_id, adapter_type, local_session_id, session_type, created_at
                     FROM remote_session_mappings",
                )
                .map_err(|e| {
                    RemoteError::ConfigError(format!("Failed to prepare query: {}", e))
                })?;

            let mappings = stmt
                .query_map([], |row| {
                    let session_type_json: String = row.get(4)?;
                    let session_type: super::types::SessionType =
                        serde_json::from_str(&session_type_json)
                            .unwrap_or(super::types::SessionType::ClaudeCode);

                    let adapter_type_name: Option<String> = row.get(2).ok();

                    Ok(RemoteSessionMapping {
                        chat_id: row.get(0)?,
                        user_id: row.get(1)?,
                        local_session_id: row.get(3)?,
                        session_type,
                        created_at: row.get(5)?,
                        adapter_type_name,
                        username: None, // Not stored in current schema
                    })
                })
                .map_err(|e| {
                    RemoteError::ConfigError(format!("Failed to query mappings: {}", e))
                })?;

            mappings.flatten().collect()
        }; // conn, stmt, and MappedRows dropped here

        let mut sessions = self.sessions.write().await;
        for mapping in collected {
            sessions.insert(mapping.chat_id, mapping);
        }

        Ok(())
    }
}
