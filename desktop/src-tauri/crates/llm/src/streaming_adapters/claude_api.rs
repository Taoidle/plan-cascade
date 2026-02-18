//! Claude API Adapter
//!
//! Handles the SSE format from Claude API with content_block_delta parsing.

use plan_cascade_core::streaming::{AdapterError, StreamAdapter, UnifiedStreamEvent};
use serde::Deserialize;

/// Internal event types from Claude API SSE format
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClaudeApiEvent {
    MessageStart {
        message: MessageInfo,
    },
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: Delta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDelta,
        #[serde(default)]
        usage: Option<DeltaUsage>,
    },
    MessageStop,
    Ping,
    Error {
        error: ApiError,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct MessageInfo {
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {
        #[serde(default)]
        text: Option<String>,
    },
    Thinking {
        #[serde(default)]
        thinking_id: Option<String>,
    },
    ToolUse {
        id: String,
        name: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Delta {
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        thinking: String,
    },
    InputJsonDelta {
        partial_json: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct MessageDelta {
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct DeltaUsage {
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

/// Adapter for Claude API SSE format
pub struct ClaudeApiAdapter {
    /// Track current content block for thinking correlation
    current_thinking_id: Option<String>,
    /// Track current tool ID for input accumulation
    current_tool_id: Option<String>,
    current_tool_name: Option<String>,
    tool_input_buffer: String,
}

impl ClaudeApiAdapter {
    pub fn new() -> Self {
        Self {
            current_thinking_id: None,
            current_tool_id: None,
            current_tool_name: None,
            tool_input_buffer: String::new(),
        }
    }
}

impl Default for ClaudeApiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamAdapter for ClaudeApiAdapter {
    fn provider_name(&self) -> &'static str {
        "claude-api"
    }

    fn supports_thinking(&self) -> bool {
        true
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn adapt(&mut self, input: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        let trimmed = input.trim();

        // Handle SSE format: "data: {...}"
        // SSE streams may include event:, id:, retry:, and comment lines.
        let json_str = if let Some(rest) = trimmed.strip_prefix("data: ") {
            rest
        } else if trimmed.starts_with('{') {
            // Raw JSON without SSE prefix
            trimmed
        } else {
            // Skip non-data SSE lines (event:, id:, retry:, comments, empty)
            return Ok(vec![]);
        };

        if json_str.is_empty() || json_str == "[DONE]" {
            return Ok(vec![]);
        }

        let event: ClaudeApiEvent =
            serde_json::from_str(json_str).map_err(|e| AdapterError::ParseError(e.to_string()))?;

        let events = match event {
            ClaudeApiEvent::MessageStart { message } => {
                if let Some(usage) = message.usage {
                    vec![UnifiedStreamEvent::Usage {
                        input_tokens: usage.input_tokens,
                        output_tokens: usage.output_tokens,
                        thinking_tokens: None,
                        cache_read_tokens: usage.cache_read_input_tokens,
                        cache_creation_tokens: usage.cache_creation_input_tokens,
                    }]
                } else {
                    vec![]
                }
            }
            ClaudeApiEvent::ContentBlockStart { content_block, .. } => match content_block {
                ContentBlock::Thinking { thinking_id } => {
                    self.current_thinking_id = thinking_id.clone();
                    vec![UnifiedStreamEvent::ThinkingStart { thinking_id }]
                }
                ContentBlock::ToolUse { id, name } => {
                    self.current_tool_id = Some(id.clone());
                    self.current_tool_name = Some(name.clone());
                    self.tool_input_buffer.clear();
                    vec![UnifiedStreamEvent::ToolStart {
                        tool_id: id,
                        tool_name: name,
                        arguments: None,
                    }]
                }
                _ => vec![],
            },
            ClaudeApiEvent::ContentBlockDelta { delta, .. } => match delta {
                Delta::TextDelta { text } => {
                    vec![UnifiedStreamEvent::TextDelta { content: text }]
                }
                Delta::ThinkingDelta { thinking } => {
                    vec![UnifiedStreamEvent::ThinkingDelta {
                        content: thinking,
                        thinking_id: self.current_thinking_id.clone(),
                    }]
                }
                Delta::InputJsonDelta { partial_json } => {
                    self.tool_input_buffer.push_str(&partial_json);
                    vec![]
                }
                Delta::Other => vec![],
            },
            ClaudeApiEvent::ContentBlockStop { .. } => {
                let mut events = vec![];

                // If we were in a thinking block, emit ThinkingEnd
                if self.current_thinking_id.is_some() {
                    events.push(UnifiedStreamEvent::ThinkingEnd {
                        thinking_id: self.current_thinking_id.take(),
                    });
                }

                // If we were accumulating a tool call, emit ToolComplete
                if let (Some(id), Some(name)) =
                    (self.current_tool_id.take(), self.current_tool_name.take())
                {
                    let args = std::mem::take(&mut self.tool_input_buffer);
                    events.push(UnifiedStreamEvent::ToolComplete {
                        tool_id: id,
                        tool_name: name,
                        arguments: args,
                    });
                }

                events
            }
            ClaudeApiEvent::MessageDelta { delta, usage } => {
                let mut events = vec![];
                if let Some(u) = usage {
                    events.push(UnifiedStreamEvent::Usage {
                        input_tokens: 0,
                        output_tokens: u.output_tokens,
                        thinking_tokens: None,
                        cache_read_tokens: None,
                        cache_creation_tokens: None,
                    });
                }
                if delta.stop_reason.is_some() {
                    events.push(UnifiedStreamEvent::Complete {
                        stop_reason: delta.stop_reason,
                    });
                }
                events
            }
            ClaudeApiEvent::MessageStop => {
                vec![UnifiedStreamEvent::Complete { stop_reason: None }]
            }
            ClaudeApiEvent::Error { error } => {
                vec![UnifiedStreamEvent::Error {
                    message: error.message,
                    code: error.error_type,
                }]
            }
            ClaudeApiEvent::Ping | ClaudeApiEvent::Unknown => vec![],
        };

        Ok(events)
    }

    fn reset(&mut self) {
        self.current_thinking_id = None;
        self.current_tool_id = None;
        self.current_tool_name = None;
        self.tool_input_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_format_parsing() {
        let mut adapter = ClaudeApiAdapter::new();

        let events = adapter.adapt(r#"data: {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}"#).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::TextDelta { content } => {
                assert_eq!(content, "Hello");
            }
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_thinking_block() {
        let mut adapter = ClaudeApiAdapter::new();

        let events = adapter.adapt(r#"data: {"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking_id": "t1"}}"#).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ThinkingStart { thinking_id } => {
                assert_eq!(thinking_id, &Some("t1".to_string()));
            }
            _ => panic!("Expected ThinkingStart"),
        }

        let events = adapter.adapt(r#"data: {"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "reasoning..."}}"#).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ThinkingDelta {
                content,
                thinking_id,
            } => {
                assert_eq!(content, "reasoning...");
                assert_eq!(thinking_id, &Some("t1".to_string()));
            }
            _ => panic!("Expected ThinkingDelta"),
        }
    }

    #[test]
    fn test_message_stop() {
        let mut adapter = ClaudeApiAdapter::new();

        let events = adapter.adapt(r#"data: {"type": "message_stop"}"#).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::Complete { .. } => {}
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_empty_and_done() {
        let mut adapter = ClaudeApiAdapter::new();

        assert!(adapter.adapt("").unwrap().is_empty());
        assert!(adapter.adapt("data: [DONE]").unwrap().is_empty());
    }
}
