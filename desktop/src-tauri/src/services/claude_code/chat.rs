//! Chat Message Handling
//!
//! Handles sending messages to Claude Code and processing streaming responses.

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::models::claude_code::SessionState;
use crate::services::streaming::adapter::StreamAdapter;
use crate::services::streaming::adapters::claude_code::ClaudeCodeAdapter;
use crate::services::streaming::unified::UnifiedStreamEvent;
use crate::utils::error::{AppError, AppResult};

use super::executor::{ClaudeCodeExecutor, SpawnConfig};
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
    /// Spawns a new `claude --output-format stream-json --verbose` process
    /// for each message. The prompt is piped to stdin (not -p), which enables
    /// true streaming output with content_block_delta events.
    /// Uses `--resume <session_id>` for conversation continuity.
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
            return Err(AppError::validation(
                "Session is already processing a request",
            ));
        }

        // Update session state to running
        self.session_manager
            .update_session_state(session_id, SessionState::Running)
            .await?;

        // Build spawn config - prompt goes to stdin, NOT via -p flag
        let mut config = SpawnConfig::new(&session.project_path);

        if let Some(ref model) = session.model {
            config = config.with_model(model);
        }

        // Use --resume with the CLI session_id for conversation continuity
        if let Some(ref resume_token) = session.resume_token {
            config = config.with_resume(resume_token);
        }

        // Spawn a new process for this message
        let executor = ClaudeCodeExecutor::new();
        let mut process = executor.spawn(&config).await?;
        let pid = process.pid();
        eprintln!(
            "[INFO] Spawned Claude Code process {} for message in session {}",
            pid, session_id
        );

        // Write prompt to stdin, then close it to signal EOF
        // The CLI reads all of stdin and processes it as the user message
        if let Some(mut stdin) = process.take_stdin() {
            if let Err(e) = stdin.write_all(prompt.as_bytes()).await {
                eprintln!("[ERROR] Failed to write prompt to stdin: {}", e);
            }
            if let Err(e) = stdin.flush().await {
                eprintln!("[ERROR] Failed to flush stdin: {}", e);
            }
            // Drop stdin to close it - signals EOF to the CLI
            drop(stdin);
        }

        // Take stdout for reading events
        let stdout = process
            .take_stdout()
            .ok_or_else(|| AppError::internal("Failed to get stdout handle"))?;

        // Create event channel
        let (tx, rx) = mpsc::channel::<UnifiedStreamEvent>(100);

        // Clone session_id for the async task
        let session_id_clone = session_id.to_string();
        let session_manager = self.session_manager.clone();

        // Also capture stderr for debugging
        let stderr = process.take_stderr();

        // Spawn a task to log stderr
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("[claude stderr] {}", line);
                }
            });
        }

        // Spawn a task to read stdout and forward parsed events
        tokio::spawn(async move {
            eprintln!(
                "[DEBUG] stdout reader task started for session {}",
                session_id_clone
            );
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut adapter = ClaudeCodeAdapter::new();
            let mut line_count = 0u32;

            while let Ok(Some(line)) = lines.next_line().await {
                line_count += 1;
                // Skip empty lines
                if line.trim().is_empty() {
                    continue;
                }

                // Capture CLI session_id from raw JSON for conversation continuity
                // The system and result events include a session_id field
                if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(cli_sid) = raw.get("session_id").and_then(|v| v.as_str()) {
                        if !cli_sid.is_empty() {
                            eprintln!("[DEBUG] captured CLI session_id: {}", cli_sid);
                            let _ = session_manager
                                .set_resume_token(&session_id_clone, cli_sid)
                                .await;
                        }
                    }
                }

                // Try to parse the line via adapter
                match adapter.adapt(&line) {
                    Ok(events) => {
                        let mut should_break = false;

                        for event in events {
                            let is_complete = matches!(&event, UnifiedStreamEvent::Complete { .. });

                            // Check if this is a large text delta that needs chunking
                            // (e.g., from non-streaming assistant event with full response)
                            let is_large_text = matches!(
                                &event,
                                UnifiedStreamEvent::TextDelta { content } if content.chars().count() > 20
                            );
                            let is_text = matches!(&event, UnifiedStreamEvent::TextDelta { .. });

                            if is_large_text {
                                // Split large text into small chunks for typewriter effect
                                if let UnifiedStreamEvent::TextDelta { content } = event {
                                    let chars: Vec<char> = content.chars().collect();
                                    for chunk in chars.chunks(4) {
                                        let chunk_text: String = chunk.iter().collect();
                                        if tx
                                            .send(UnifiedStreamEvent::TextDelta {
                                                content: chunk_text,
                                            })
                                            .await
                                            .is_err()
                                        {
                                            should_break = true;
                                            break;
                                        }
                                        tokio::time::sleep(tokio::time::Duration::from_millis(12))
                                            .await;
                                    }
                                }
                            } else {
                                if tx.send(event).await.is_err() {
                                    should_break = true;
                                }
                                // Small delay after text deltas to prevent React from
                                // batching all rapid updates into a single render
                                if is_text && !should_break {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(8)).await;
                                }
                            }

                            if should_break {
                                eprintln!("[WARN] mpsc receiver dropped, stopping");
                                break;
                            }

                            if is_complete {
                                eprintln!(
                                    "[DEBUG] stream complete for session {}",
                                    session_id_clone
                                );
                                let _ = session_manager
                                    .update_session_state(&session_id_clone, SessionState::Idle)
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[DEBUG] adapter parse error on line {}: {}", line_count, e);
                    }
                }
            }

            eprintln!(
                "[DEBUG] stdout reader ended after {} lines for session {}",
                line_count, session_id_clone
            );

            // Stream ended (process exited) - update session state
            let _ = session_manager
                .update_session_state(&session_id_clone, SessionState::Idle)
                .await;
            let _ = session_manager
                .increment_message_count(&session_id_clone)
                .await;

            // Keep process alive until stdout is fully read, then let it drop
            drop(process);
        });

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
        assert!(matches!(
            events[0],
            UnifiedStreamEvent::ThinkingStart { .. }
        ));

        let events = handler
            .process_line(r#"{"type": "thinking_delta", "delta": "test", "thinking_id": "t1"}"#);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            UnifiedStreamEvent::ThinkingDelta { .. }
        ));
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
