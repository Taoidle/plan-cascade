//! Claude Code CLI Adapter
//!
//! Handles the stream-json format from Claude Code CLI with native thinking block support.
//! Uses `--include-partial-messages` for true streaming via `stream_event` wrapper events.

use crate::services::streaming::adapter::StreamAdapter;
use crate::services::streaming::unified::{AdapterError, UnifiedStreamEvent};
use serde::Deserialize;

/// Internal event types from Claude Code CLI stream-json format
///
/// With `--include-partial-messages`, the CLI outputs:
///   system → stream_event* → assistant → result
///
/// The `stream_event` events wrap inner API events (content_block_delta, etc.)
/// and provide true real-time streaming. The `assistant` event contains the
/// final complete message and should NOT be used for text output (it would
/// duplicate what was already streamed).
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClaudeCodeEvent {
    /// System initialization event (emitted at start)
    System {
        #[serde(default)]
        session_id: Option<String>,
    },
    /// Real-time streaming wrapper (from --include-partial-messages)
    /// Contains inner API events like content_block_delta
    StreamEvent {
        #[serde(default)]
        event: Option<StreamInnerEvent>,
    },
    /// Assistant response — final complete message.
    /// Text content is NOT emitted here (already streamed via stream_event).
    /// Only tool calls and usage info are extracted.
    Assistant {
        #[serde(default)]
        message: Option<AssistantMessage>,
        #[serde(default)]
        session_id: Option<String>,
    },
    Thinking {
        #[serde(default)]
        thinking_id: Option<String>,
    },
    ThinkingDelta {
        delta: String,
        #[serde(default)]
        thinking_id: Option<String>,
    },
    ThinkingEnd {
        #[serde(default)]
        thinking_id: Option<String>,
    },
    /// Legacy top-level content_block_delta (without --include-partial-messages)
    ContentBlockDelta { delta: ContentDelta },
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: Option<serde_json::Value>,
    },
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        is_error: Option<bool>,
    },
    Result {
        #[serde(default)]
        subtype: Option<String>,
        #[serde(default)]
        result: Option<String>,
        #[serde(default)]
        is_error: Option<bool>,
        #[serde(default)]
        stop_reason: Option<String>,
        #[serde(default)]
        usage: Option<Usage>,
        #[serde(default)]
        session_id: Option<String>,
    },
    Error {
        message: String,
        #[serde(default)]
        code: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

/// Inner event inside a stream_event wrapper
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamInnerEvent {
    ContentBlockDelta {
        delta: StreamDelta,
    },
    #[serde(other)]
    Other,
}

/// Delta inside a stream_event → content_block_delta
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamDelta {
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        #[serde(default)]
        thinking: String,
    },
    #[serde(other)]
    Other,
}

/// Assistant message from Claude Code CLI
/// The `content` field is an array of content blocks (text, tool_use, etc.)
#[derive(Debug, Deserialize)]
struct AssistantMessage {
    #[serde(default)]
    content: ContentField,
    #[serde(default)]
    usage: Option<Usage>,
}

/// Content can be either a plain string or an array of content blocks
#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
enum ContentField {
    /// Array of content blocks (standard format from Claude API)
    Blocks(Vec<ContentBlock>),
    /// Plain string (simplified format)
    Text(String),
    #[default]
    Empty,
}

/// A content block in the assistant message
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        #[serde(default)]
        thinking: Option<String>,
    },
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: Option<serde_json::Value>,
    },
    ToolResult {
        #[serde(default)]
        tool_use_id: Option<String>,
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        is_error: Option<bool>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentDelta {
    TextDelta {
        text: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
    #[serde(default)]
    thinking_tokens: Option<u32>,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
}

/// Adapter for Claude Code CLI stream-json format
///
/// With `--include-partial-messages`, text is streamed via `stream_event`
/// wrapper events. The `assistant` event is only used for tool calls and
/// usage — its text content is skipped to avoid duplication.
///
/// Without `--include-partial-messages` (legacy), text comes from top-level
/// `content_block_delta` events or the `assistant` event as fallback.
pub struct ClaudeCodeAdapter {
    /// Set to true when we receive streaming text (via stream_event or
    /// content_block_delta). When true, text from the Assistant event
    /// is skipped to avoid duplication.
    has_streamed_text: bool,
    /// Set to true when we receive any stream_event. When true, top-level
    /// content_block_delta events are skipped (they would be duplicates
    /// of what stream_event already provided).
    uses_stream_events: bool,
}

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        Self {
            has_streamed_text: false,
            uses_stream_events: false,
        }
    }
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamAdapter for ClaudeCodeAdapter {
    fn provider_name(&self) -> &'static str {
        "claude-code"
    }

    fn supports_thinking(&self) -> bool {
        true
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn adapt(&mut self, input: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }

        let event: ClaudeCodeEvent =
            serde_json::from_str(trimmed).map_err(|e| AdapterError::ParseError(e.to_string()))?;

        let events = match event {
            ClaudeCodeEvent::System { .. } => {
                vec![]
            }

            // ── stream_event: real-time streaming (from --include-partial-messages) ──
            ClaudeCodeEvent::StreamEvent { event: inner } => {
                self.uses_stream_events = true;
                match inner {
                    Some(StreamInnerEvent::ContentBlockDelta { delta }) => match delta {
                        StreamDelta::TextDelta { text } => {
                            self.has_streamed_text = true;
                            vec![UnifiedStreamEvent::TextDelta { content: text }]
                        }
                        StreamDelta::ThinkingDelta { thinking } => {
                            if !thinking.is_empty() {
                                vec![UnifiedStreamEvent::ThinkingDelta {
                                    content: thinking,
                                    thinking_id: None,
                                }]
                            } else {
                                vec![]
                            }
                        }
                        StreamDelta::Other => vec![],
                    },
                    Some(StreamInnerEvent::Other) | None => vec![],
                }
            }

            // ── assistant: final complete message ──
            // Text is NOT emitted (already streamed via stream_event).
            // Only extract tool calls and usage info.
            ClaudeCodeEvent::Assistant { message, .. } => {
                if let Some(msg) = message {
                    let mut events = vec![];

                    match msg.content {
                        ContentField::Blocks(blocks) => {
                            for block in blocks {
                                match block {
                                    ContentBlock::Text { text } => {
                                        // Only emit if we never received streaming text
                                        // (fallback for CLIs without --include-partial-messages)
                                        if !self.has_streamed_text && !text.is_empty() {
                                            events.push(UnifiedStreamEvent::TextDelta {
                                                content: text,
                                            });
                                        }
                                    }
                                    ContentBlock::Thinking { thinking } => {
                                        if !self.has_streamed_text {
                                            if let Some(content) = thinking {
                                                events.push(UnifiedStreamEvent::ThinkingDelta {
                                                    content,
                                                    thinking_id: None,
                                                });
                                            }
                                        }
                                    }
                                    ContentBlock::ToolUse { id, name, input } => {
                                        events.push(UnifiedStreamEvent::ToolStart {
                                            tool_id: id,
                                            tool_name: name,
                                            arguments: input.map(|v| v.to_string()),
                                        });
                                    }
                                    ContentBlock::ToolResult {
                                        tool_use_id,
                                        content,
                                        is_error,
                                    } => {
                                        let tool_id =
                                            tool_use_id.unwrap_or_else(|| "unknown".to_string());
                                        if is_error.unwrap_or(false) {
                                            events.push(UnifiedStreamEvent::ToolResult {
                                                tool_id,
                                                result: None,
                                                error: content,
                                            });
                                        } else {
                                            events.push(UnifiedStreamEvent::ToolResult {
                                                tool_id,
                                                result: content,
                                                error: None,
                                            });
                                        }
                                    }
                                    ContentBlock::Other => {}
                                }
                            }
                        }
                        ContentField::Text(text) => {
                            if !self.has_streamed_text && !text.is_empty() {
                                events.push(UnifiedStreamEvent::TextDelta { content: text });
                            }
                        }
                        ContentField::Empty => {}
                    }

                    if let Some(u) = msg.usage {
                        events.push(UnifiedStreamEvent::Usage {
                            input_tokens: u.input_tokens,
                            output_tokens: u.output_tokens,
                            thinking_tokens: u.thinking_tokens,
                            cache_read_tokens: u.cache_read_input_tokens,
                            cache_creation_tokens: u.cache_creation_input_tokens,
                        });
                    }
                    events
                } else {
                    vec![]
                }
            }

            // ── thinking events (top-level) ──
            ClaudeCodeEvent::Thinking { thinking_id } => {
                vec![UnifiedStreamEvent::ThinkingStart { thinking_id }]
            }
            ClaudeCodeEvent::ThinkingDelta { delta, thinking_id } => {
                vec![UnifiedStreamEvent::ThinkingDelta {
                    content: delta,
                    thinking_id,
                }]
            }
            ClaudeCodeEvent::ThinkingEnd { thinking_id } => {
                vec![UnifiedStreamEvent::ThinkingEnd { thinking_id }]
            }

            // ── content_block_delta (legacy top-level) ──
            // With --include-partial-messages, the CLI outputs BOTH stream_event
            // wrappers AND top-level content_block_delta for each delta. We must
            // skip the top-level one when stream_events are active to avoid duplication.
            ClaudeCodeEvent::ContentBlockDelta { delta } => match delta {
                ContentDelta::TextDelta { text } => {
                    if self.uses_stream_events {
                        // Already received via stream_event — skip to avoid duplication
                        vec![]
                    } else {
                        // Legacy mode (no --include-partial-messages)
                        self.has_streamed_text = true;
                        vec![UnifiedStreamEvent::TextDelta { content: text }]
                    }
                }
                ContentDelta::Other => vec![],
            },

            // ── tool events ──
            ClaudeCodeEvent::ToolUse { id, name, input } => {
                vec![UnifiedStreamEvent::ToolStart {
                    tool_id: id,
                    tool_name: name,
                    arguments: input.map(|v| v.to_string()),
                }]
            }
            ClaudeCodeEvent::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                if is_error.unwrap_or(false) {
                    vec![UnifiedStreamEvent::ToolResult {
                        tool_id: tool_use_id,
                        result: None,
                        error: content,
                    }]
                } else {
                    vec![UnifiedStreamEvent::ToolResult {
                        tool_id: tool_use_id,
                        result: content,
                        error: None,
                    }]
                }
            }

            // ── result (end of turn) ──
            ClaudeCodeEvent::Result {
                stop_reason,
                usage,
                is_error,
                ..
            } => {
                // Reset for next turn
                self.has_streamed_text = false;
                self.uses_stream_events = false;
                let mut events = vec![];
                if is_error.unwrap_or(false) {
                    events.push(UnifiedStreamEvent::Error {
                        message: "Execution failed".to_string(),
                        code: None,
                    });
                }
                if let Some(u) = usage {
                    events.push(UnifiedStreamEvent::Usage {
                        input_tokens: u.input_tokens,
                        output_tokens: u.output_tokens,
                        thinking_tokens: u.thinking_tokens,
                        cache_read_tokens: u.cache_read_input_tokens,
                        cache_creation_tokens: u.cache_creation_input_tokens,
                    });
                }
                events.push(UnifiedStreamEvent::Complete { stop_reason });
                events
            }

            ClaudeCodeEvent::Error { message, code } => {
                vec![UnifiedStreamEvent::Error { message, code }]
            }
            ClaudeCodeEvent::Unknown => vec![],
        };

        Ok(events)
    }

    fn reset(&mut self) {
        self.has_streamed_text = false;
        self.uses_stream_events = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_events() {
        let mut adapter = ClaudeCodeAdapter::new();

        let events = adapter
            .adapt(r#"{"type": "thinking", "thinking_id": "t1"}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ThinkingStart { thinking_id } => {
                assert_eq!(thinking_id, &Some("t1".to_string()));
            }
            _ => panic!("Expected ThinkingStart"),
        }

        let events = adapter
            .adapt(r#"{"type": "thinking_delta", "delta": "analyzing...", "thinking_id": "t1"}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ThinkingDelta { content, .. } => {
                assert_eq!(content, "analyzing...");
            }
            _ => panic!("Expected ThinkingDelta"),
        }
    }

    #[test]
    fn test_tool_events() {
        let mut adapter = ClaudeCodeAdapter::new();

        let events = adapter.adapt(r#"{"type": "tool_use", "id": "tool_1", "name": "read_file", "input": {"path": "/foo"}}"#).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ToolStart {
                tool_id, tool_name, ..
            } => {
                assert_eq!(tool_id, "tool_1");
                assert_eq!(tool_name, "read_file");
            }
            _ => panic!("Expected ToolStart"),
        }
    }

    #[test]
    fn test_result_with_usage() {
        let mut adapter = ClaudeCodeAdapter::new();

        let events = adapter.adapt(r#"{"type": "result", "stop_reason": "end_turn", "usage": {"input_tokens": 100, "output_tokens": 50}}"#).unwrap();
        assert_eq!(events.len(), 2);
        match &events[0] {
            UnifiedStreamEvent::Usage {
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(*input_tokens, 100);
                assert_eq!(*output_tokens, 50);
            }
            _ => panic!("Expected Usage"),
        }
        match &events[1] {
            UnifiedStreamEvent::Complete { stop_reason } => {
                assert_eq!(stop_reason, &Some("end_turn".to_string()));
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_empty_input() {
        let mut adapter = ClaudeCodeAdapter::new();
        let events = adapter.adapt("").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_system_event() {
        let mut adapter = ClaudeCodeAdapter::new();
        let events = adapter
            .adapt(r#"{"type":"system","subtype":"init","session_id":"abc-123","tools":[]}"#)
            .unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_assistant_text_skipped_after_streaming() {
        let mut adapter = ClaudeCodeAdapter::new();

        // No streaming yet — assistant text should be emitted
        let input = r#"{"type":"assistant","message":{"model":"claude-opus-4-6","content":[{"type":"text","text":"Hello!"}],"usage":{"input_tokens":3,"output_tokens":12}},"session_id":"abc-123"}"#;
        let events = adapter.adapt(input).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, UnifiedStreamEvent::TextDelta { .. })),
            "Without streaming, assistant text should be emitted"
        );
    }

    #[test]
    fn test_result_event_with_session_id() {
        let mut adapter = ClaudeCodeAdapter::new();
        let input = r#"{"type":"result","subtype":"success","is_error":false,"result":"Hello!","session_id":"abc-123","usage":{"input_tokens":3,"output_tokens":12}}"#;
        let events = adapter.adapt(input).unwrap();
        assert!(events.len() >= 1);
        let has_complete = events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::Complete { .. }));
        assert!(has_complete, "Expected Complete event in {:?}", events);
    }

    #[test]
    fn test_stream_event_text_delta() {
        let mut adapter = ClaudeCodeAdapter::new();

        // stream_event wrapping content_block_delta with text_delta
        let input = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}}"#;
        let events = adapter.adapt(input).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::TextDelta { content } => assert_eq!(content, "Hello"),
            _ => panic!("Expected TextDelta, got {:?}", events[0]),
        }
    }

    #[test]
    fn test_stream_event_thinking_delta() {
        let mut adapter = ClaudeCodeAdapter::new();

        let input = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"analyzing..."}}}"#;
        let events = adapter.adapt(input).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ThinkingDelta { content, .. } => {
                assert_eq!(content, "analyzing...")
            }
            _ => panic!("Expected ThinkingDelta, got {:?}", events[0]),
        }
    }

    #[test]
    fn test_streaming_deduplication() {
        let mut adapter = ClaudeCodeAdapter::new();

        // 1. stream_event deltas arrive (real streaming)
        let events = adapter
            .adapt(r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], UnifiedStreamEvent::TextDelta { content } if content == "Hello")
        );

        let events = adapter
            .adapt(r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world!"}}}"#)
            .unwrap();
        assert_eq!(events.len(), 1);

        // 2. Top-level content_block_delta with same text — should be SKIPPED
        //    (CLI outputs both stream_event and content_block_delta for each delta)
        let events = adapter
            .adapt(r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}"#)
            .unwrap();
        assert_eq!(
            events.len(),
            0,
            "Top-level content_block_delta should be skipped when stream_events active, got {:?}",
            events
        );

        // 3. Assistant event with full text — text should be SKIPPED
        let events = adapter
            .adapt(r#"{"type":"assistant","message":{"model":"claude-opus-4-6","content":[{"type":"text","text":"Hello world!"}],"usage":{"input_tokens":3,"output_tokens":5}}}"#)
            .unwrap();
        // Should only contain Usage, NOT TextDelta
        assert_eq!(events.len(), 1, "Expected only Usage, got {:?}", events);
        assert!(
            matches!(&events[0], UnifiedStreamEvent::Usage { .. }),
            "Expected Usage, got {:?}",
            events[0]
        );

        // 4. Result resets the flags
        let _ = adapter
            .adapt(r#"{"type":"result","stop_reason":"end_turn","usage":{"input_tokens":3,"output_tokens":5}}"#)
            .unwrap();

        // 5. New turn without streaming — assistant text should be emitted
        let events = adapter
            .adapt(r#"{"type":"assistant","message":{"model":"claude-opus-4-6","content":[{"type":"text","text":"New response"}],"usage":{"input_tokens":1,"output_tokens":2}}}"#)
            .unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, UnifiedStreamEvent::TextDelta { .. })),
            "Expected TextDelta for non-streamed turn, got {:?}",
            events
        );
    }

    #[test]
    fn test_legacy_content_block_delta_without_stream_events() {
        let mut adapter = ClaudeCodeAdapter::new();

        // Without stream_events, top-level content_block_delta should work
        let events = adapter
            .adapt(r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Legacy text"}}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], UnifiedStreamEvent::TextDelta { content } if content == "Legacy text")
        );

        // After receiving content_block_delta, assistant text should be skipped
        let events = adapter
            .adapt(r#"{"type":"assistant","message":{"model":"claude-opus-4-6","content":[{"type":"text","text":"Legacy text"}],"usage":{"input_tokens":1,"output_tokens":2}}}"#)
            .unwrap();
        assert_eq!(
            events.len(),
            1,
            "Expected only Usage after legacy streaming, got {:?}",
            events
        );
        assert!(matches!(&events[0], UnifiedStreamEvent::Usage { .. }));
    }
}
