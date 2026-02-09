//! Tauri Event Emission System
//!
//! Provides WebSocket-like real-time event emission from Rust to frontend
//! using Tauri's event system. Events are namespaced for frontend filtering.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use crate::models::claude_code::{ClaudeCodeSession, SessionState};
use crate::services::streaming::unified::UnifiedStreamEvent;

use super::thinking::ThinkingBlock;
use super::tools::ToolExecution;

/// Event channel names for Claude Code events
pub mod channels {
    /// Stream events (text, tool calls, etc.)
    pub const STREAM: &str = "claude_code:stream";
    /// Thinking block updates
    pub const THINKING: &str = "claude_code:thinking";
    /// Tool execution updates
    pub const TOOL: &str = "claude_code:tool";
    /// Session state changes
    pub const SESSION: &str = "claude_code:session";
}

/// Thinking update event payload
#[derive(Debug, Clone, Serialize)]
pub struct ThinkingUpdateEvent {
    /// The thinking block data
    pub block: ThinkingBlock,
    /// Type of update: "started", "updated", "completed"
    pub update_type: String,
    /// Associated session ID
    pub session_id: String,
}

/// Tool update event payload
#[derive(Debug, Clone, Serialize)]
pub struct ToolUpdateEvent {
    /// The tool execution data
    pub execution: ToolExecution,
    /// Type of update: "started", "completed"
    pub update_type: String,
    /// Associated session ID
    pub session_id: String,
}

/// Session update event payload
#[derive(Debug, Clone, Serialize)]
pub struct SessionUpdateEvent {
    /// The session data
    pub session: ClaudeCodeSession,
    /// Type of update: "created", "state_changed", "message_sent", "removed"
    pub update_type: String,
    /// Previous state (for state_changed)
    pub previous_state: Option<SessionState>,
}

/// Stream event payload with session context
#[derive(Debug, Clone, Serialize)]
pub struct StreamEventPayload {
    /// The stream event
    pub event: UnifiedStreamEvent,
    /// Associated session ID
    pub session_id: String,
}

/// Event emitter for Claude Code events
///
/// Wraps Tauri's AppHandle to provide typed event emission
/// with proper error handling (log failures, don't crash).
pub struct ClaudeCodeEventEmitter<R: Runtime> {
    app_handle: AppHandle<R>,
}

impl<R: Runtime> ClaudeCodeEventEmitter<R> {
    /// Create a new event emitter
    pub fn new(app_handle: AppHandle<R>) -> Self {
        Self { app_handle }
    }

    /// Emit a stream event
    pub fn emit_stream_event(&self, session_id: &str, event: UnifiedStreamEvent) {
        let payload = StreamEventPayload {
            event,
            session_id: session_id.to_string(),
        };

        if let Err(e) = self.app_handle.emit(channels::STREAM, &payload) {
            eprintln!(
                "[WARN] Failed to emit stream event for session {}: {}",
                session_id, e
            );
        }
    }

    /// Emit a thinking update event
    pub fn emit_thinking_update(&self, session_id: &str, block: ThinkingBlock, update_type: &str) {
        let payload = ThinkingUpdateEvent {
            block,
            update_type: update_type.to_string(),
            session_id: session_id.to_string(),
        };

        if let Err(e) = self.app_handle.emit(channels::THINKING, &payload) {
            eprintln!(
                "[WARN] Failed to emit thinking update for session {}: {}",
                session_id, e
            );
        }
    }

    /// Emit a tool update event
    pub fn emit_tool_update(&self, session_id: &str, execution: ToolExecution, update_type: &str) {
        let payload = ToolUpdateEvent {
            execution,
            update_type: update_type.to_string(),
            session_id: session_id.to_string(),
        };

        if let Err(e) = self.app_handle.emit(channels::TOOL, &payload) {
            eprintln!(
                "[WARN] Failed to emit tool update for session {}: {}",
                session_id, e
            );
        }
    }

    /// Emit a session update event
    pub fn emit_session_update(
        &self,
        session: ClaudeCodeSession,
        update_type: &str,
        previous_state: Option<SessionState>,
    ) {
        let payload = SessionUpdateEvent {
            session,
            update_type: update_type.to_string(),
            previous_state,
        };

        if let Err(e) = self.app_handle.emit(channels::SESSION, &payload) {
            eprintln!("[WARN] Failed to emit session update: {}", e);
        }
    }

    /// Emit a session created event
    pub fn emit_session_created(&self, session: ClaudeCodeSession) {
        self.emit_session_update(session, "created", None);
    }

    /// Emit a session state changed event
    pub fn emit_session_state_changed(
        &self,
        session: ClaudeCodeSession,
        previous_state: SessionState,
    ) {
        self.emit_session_update(session, "state_changed", Some(previous_state));
    }

    /// Emit a session removed event
    pub fn emit_session_removed(&self, session: ClaudeCodeSession) {
        self.emit_session_update(session, "removed", None);
    }

    /// Emit a thinking started event
    pub fn emit_thinking_started(&self, session_id: &str, block: ThinkingBlock) {
        self.emit_thinking_update(session_id, block, "started");
    }

    /// Emit a thinking updated event (content appended)
    pub fn emit_thinking_updated(&self, session_id: &str, block: ThinkingBlock) {
        self.emit_thinking_update(session_id, block, "updated");
    }

    /// Emit a thinking completed event
    pub fn emit_thinking_completed(&self, session_id: &str, block: ThinkingBlock) {
        self.emit_thinking_update(session_id, block, "completed");
    }

    /// Emit a tool started event
    pub fn emit_tool_started(&self, session_id: &str, execution: ToolExecution) {
        self.emit_tool_update(session_id, execution, "started");
    }

    /// Emit a tool completed event
    pub fn emit_tool_completed(&self, session_id: &str, execution: ToolExecution) {
        self.emit_tool_update(session_id, execution, "completed");
    }
}

impl<R: Runtime> Clone for ClaudeCodeEventEmitter<R> {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_names() {
        assert_eq!(channels::STREAM, "claude_code:stream");
        assert_eq!(channels::THINKING, "claude_code:thinking");
        assert_eq!(channels::TOOL, "claude_code:tool");
        assert_eq!(channels::SESSION, "claude_code:session");
    }

    #[test]
    fn test_thinking_update_event_serialization() {
        let block = ThinkingBlock::new("t1");
        let event = ThinkingUpdateEvent {
            block,
            update_type: "started".to_string(),
            session_id: "sess-1".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"update_type\":\"started\""));
        assert!(json.contains("\"session_id\":\"sess-1\""));
    }

    #[test]
    fn test_tool_update_event_serialization() {
        let execution = ToolExecution::new("tool-1", "Read");
        let event = ToolUpdateEvent {
            execution,
            update_type: "started".to_string(),
            session_id: "sess-1".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"update_type\":\"started\""));
        assert!(json.contains("\"session_id\":\"sess-1\""));
    }

    #[test]
    fn test_session_update_event_serialization() {
        let session = ClaudeCodeSession::new("sess-1", "/project");
        let event = SessionUpdateEvent {
            session,
            update_type: "created".to_string(),
            previous_state: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"update_type\":\"created\""));
        assert!(json.contains("\"id\":\"sess-1\""));
    }

    #[test]
    fn test_stream_event_payload_serialization() {
        let event = UnifiedStreamEvent::TextDelta {
            content: "Hello".to_string(),
        };
        let payload = StreamEventPayload {
            event,
            session_id: "sess-1".to_string(),
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"session_id\":\"sess-1\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }
}
