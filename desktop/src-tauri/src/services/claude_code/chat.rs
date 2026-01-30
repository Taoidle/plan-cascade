//! Chat Message Handling
//!
//! Handles sending messages to Claude Code and processing streaming responses.

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::models::claude_code::SessionState;
use crate::services::streaming::adapters::claude_code::ClaudeCodeAdapter;
use crate::services::streaming::adapter::StreamAdapter;
use crate::services::streaming::unified::UnifiedStreamEvent;
use crate::utils::error::{AppError, AppResult};

use super::session_manager::ActiveSessionManager;

/// Result of a message send operation
#[derive(Debug)]
pub struct SendMessageResult {
    /// Whether the message was sent successfully
    pub sent: bool,
    /// Error message if sending failed
    pub error: Option<String>,
}

impl SendMessageResult {
    pub fn success() -> Self {
        Self {
            sent: true,
            error: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            sent: false,
            error: Some(error.into()),
        }
    }
}

/// Handles chat operations for a Claude Code session
pub struct ChatHandler {
    /// Session manager reference
    session_manager: Arc<ActiveSessionManager>,
    /// Stream adapter for parsing output
    adapter: ClaudeCodeAdapter,
}

impl ChatHandler {
    /// Create a new chat handler
    pub fn new(session_manager: Arc<ActiveSessionManager>) -> Self {
        Self {
            session_manager,
            adapter: ClaudeCodeAdapter::new(),
        }
    }

    /// Send a message to a session and start streaming responses
    ///
    /// Returns a channel receiver that yields UnifiedStreamEvents.
    pub async fn send_message(
        &mut self,
        session_id: &str,
        prompt: &str,
    ) -> AppResult<mpsc::Receiver<UnifiedStreamEvent>> {
        // Get the session
        let session = self
            .session_manager
            .get_session(session_id)
            .await
            .ok_or_else(|| AppError::not_found(format!("Session not found: {}", session_id)))?;

        // Check session state
        if session.state == SessionState::Running {
            return Err(AppError::validation("Session is already processing a request"));
        }

        // Make sure a process is spawned
        if !self.session_manager.has_running_process(session_id).await {
            self.session_manager.spawn_process(session_id).await?;
        }

        // Take the process to work with it
        let mut process = self
            .session_manager
            .take_process(session_id)
            .await
            .ok_or_else(|| AppError::internal("Failed to get process handle"))?;

        // Get stdin and stdout
        let mut stdin = process
            .take_stdin()
            .ok_or_else(|| AppError::internal("Failed to get stdin handle"))?;
        let stdout = process
            .take_stdout()
            .ok_or_else(|| AppError::internal("Failed to get stdout handle"))?;

        // Update session state to running
        self.session_manager
            .update_session_state(session_id, SessionState::Running)
            .await?;

        // Create event channel
        let (tx, rx) = mpsc::channel::<UnifiedStreamEvent>(100);

        // Clone session_id for the async task
        let session_id_clone = session_id.to_string();
        let session_manager = self.session_manager.clone();

        // Write the prompt to stdin
        let prompt_with_newline = format!("{}\n", prompt);
        stdin
            .write_all(prompt_with_newline.as_bytes())
            .await
            .map_err(|e| AppError::command(format!("Failed to write to stdin: {}", e)))?;

        // Flush stdin
        stdin.flush().await.ok();

        // Spawn a task to read and process stdout
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut adapter = ClaudeCodeAdapter::new();
            let mut line_buffer = String::new();

            while let Ok(Some(line)) = lines.next_line().await {
                // Skip empty lines
                if line.trim().is_empty() {
                    continue;
                }

                // Try to parse the line
                match adapter.adapt(&line) {
                    Ok(events) => {
                        for event in events {
                            // Check for completion
                            let is_complete = matches!(event, UnifiedStreamEvent::Complete { .. });

                            // Send the event
                            if tx.send(event).await.is_err() {
                                // Receiver dropped, stop processing
                                break;
                            }

                            if is_complete {
                                // Mark session as idle
                                let _ = session_manager
                                    .update_session_state(&session_id_clone, SessionState::Idle)
                                    .await;
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        // Buffer incomplete JSON and try combining
                        line_buffer.push_str(&line);

                        // Try to parse the combined buffer
                        if let Ok(events) = adapter.adapt(&line_buffer) {
                            for event in events {
                                if tx.send(event).await.is_err() {
                                    break;
                                }
                            }
                            line_buffer.clear();
                        }
                        // If still fails, try next line (might be continuation)
                    }
                }
            }

            // Stream ended - update session state
            let _ = session_manager
                .update_session_state(&session_id_clone, SessionState::Idle)
                .await;
            let _ = session_manager.increment_message_count(&session_id_clone).await;
        });

        // Return the process to the manager (without stdin/stdout)
        self.session_manager.return_process(session_id, process).await;

        Ok(rx)
    }

    /// Process a stream of lines from Claude Code and convert to unified events
    pub fn process_line(&mut self, line: &str) -> Vec<UnifiedStreamEvent> {
        match self.adapter.adapt(line) {
            Ok(events) => events,
            Err(_) => vec![],
        }
    }

    /// Reset the adapter state
    pub fn reset(&mut self) {
        self.adapter.reset();
    }
}

/// A buffer for handling incomplete JSON lines
#[derive(Debug, Default)]
pub struct JsonLineBuffer {
    buffer: String,
    brace_depth: i32,
}

impl JsonLineBuffer {
    /// Create a new buffer
    pub fn new() -> Self {
        Self::default()
    }

    /// Add content to the buffer
    pub fn push(&mut self, content: &str) {
        self.buffer.push_str(content);

        // Update brace depth
        for c in content.chars() {
            match c {
                '{' => self.brace_depth += 1,
                '}' => self.brace_depth -= 1,
                _ => {}
            }
        }
    }

    /// Check if the buffer contains a complete JSON object
    pub fn is_complete(&self) -> bool {
        self.brace_depth == 0 && !self.buffer.is_empty()
    }

    /// Take the buffer contents and reset
    pub fn take(&mut self) -> String {
        self.brace_depth = 0;
        std::mem::take(&mut self.buffer)
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.brace_depth = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_message_result() {
        let success = SendMessageResult::success();
        assert!(success.sent);
        assert!(success.error.is_none());

        let failure = SendMessageResult::failure("test error");
        assert!(!failure.sent);
        assert_eq!(failure.error, Some("test error".to_string()));
    }

    #[test]
    fn test_json_line_buffer() {
        let mut buffer = JsonLineBuffer::new();
        assert!(buffer.is_empty());
        assert!(!buffer.is_complete());

        buffer.push(r#"{"type": "#);
        assert!(!buffer.is_complete());

        buffer.push(r#""text"}"#);
        assert!(buffer.is_complete());

        let content = buffer.take();
        assert_eq!(content, r#"{"type": "text"}"#);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_json_line_buffer_nested() {
        let mut buffer = JsonLineBuffer::new();

        buffer.push(r#"{"data": {"nested": "#);
        assert!(!buffer.is_complete());

        buffer.push(r#"true}}"#);
        assert!(buffer.is_complete());
    }

    #[test]
    fn test_chat_handler_process_line() {
        let session_manager = Arc::new(ActiveSessionManager::new());
        let mut handler = ChatHandler::new(session_manager);

        let events = handler.process_line(r#"{"type": "thinking", "thinking_id": "t1"}"#);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], UnifiedStreamEvent::ThinkingStart { .. }));

        let events = handler.process_line(r#"{"type": "thinking_delta", "delta": "test", "thinking_id": "t1"}"#);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], UnifiedStreamEvent::ThinkingDelta { .. }));
    }

    #[test]
    fn test_chat_handler_reset() {
        let session_manager = Arc::new(ActiveSessionManager::new());
        let mut handler = ChatHandler::new(session_manager);

        // Process some content
        let _ = handler.process_line(r#"{"type": "thinking", "thinking_id": "t1"}"#);

        // Reset should not panic
        handler.reset();
    }
}
