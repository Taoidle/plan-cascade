//! Claude Code CLI Adapter
//!
//! Handles the stream-json format from Claude Code CLI with native thinking block support.

use serde::Deserialize;
use crate::services::streaming::adapter::StreamAdapter;
use crate::services::streaming::unified::{AdapterError, UnifiedStreamEvent};

/// Internal event types from Claude Code CLI stream-json format
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClaudeCodeEvent {
    Assistant {
        #[serde(default)]
        message: Option<AssistantMessage>,
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
    ContentBlockDelta {
        delta: ContentDelta,
    },
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
        stop_reason: Option<String>,
        #[serde(default)]
        usage: Option<Usage>,
    },
    Error {
        message: String,
        #[serde(default)]
        code: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct AssistantMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentDelta {
    TextDelta { text: String },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(default)]
    thinking_tokens: Option<u32>,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
}

/// Adapter for Claude Code CLI stream-json format
pub struct ClaudeCodeAdapter;

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        Self
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

        let event: ClaudeCodeEvent = serde_json::from_str(trimmed)
            .map_err(|e| AdapterError::ParseError(e.to_string()))?;

        let events = match event {
            ClaudeCodeEvent::Assistant { message } => {
                if let Some(msg) = message {
                    if let Some(content) = msg.content {
                        if !content.is_empty() {
                            vec![UnifiedStreamEvent::TextDelta { content }]
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            }
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
            ClaudeCodeEvent::ContentBlockDelta { delta } => match delta {
                ContentDelta::TextDelta { text } => {
                    vec![UnifiedStreamEvent::TextDelta { content: text }]
                }
                ContentDelta::Other => vec![],
            },
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
            ClaudeCodeEvent::Result { stop_reason, usage } => {
                let mut events = vec![];
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_events() {
        let mut adapter = ClaudeCodeAdapter::new();

        let events = adapter.adapt(r#"{"type": "thinking", "thinking_id": "t1"}"#).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ThinkingStart { thinking_id } => {
                assert_eq!(thinking_id, &Some("t1".to_string()));
            }
            _ => panic!("Expected ThinkingStart"),
        }

        let events = adapter.adapt(r#"{"type": "thinking_delta", "delta": "analyzing...", "thinking_id": "t1"}"#).unwrap();
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
            UnifiedStreamEvent::ToolStart { tool_id, tool_name, .. } => {
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
            UnifiedStreamEvent::Usage { input_tokens, output_tokens, .. } => {
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
}
