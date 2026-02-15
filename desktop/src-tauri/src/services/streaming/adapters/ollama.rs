//! Ollama API Adapter
//!
//! Handles Ollama JSON stream format with model-dependent thinking detection
//! and native tool call support. Supports both the generate API (`response` field)
//! and the chat API (`message` field) formats.

use crate::services::streaming::adapter::StreamAdapter;
use crate::services::streaming::unified::{AdapterError, UnifiedStreamEvent};
use serde::Deserialize;

/// Known models that support thinking via <think> tags
const THINKING_MODELS: &[&str] = &["deepseek-r1", "deepseek-reasoner", "qwq", "qwen-qwq"];

/// Ollama generate API response format
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    /// Content from generate API
    #[serde(default)]
    response: Option<String>,
    /// Message from chat API
    #[serde(default)]
    message: Option<OllamaMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<u32>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
}

/// Message structure from Ollama chat API
#[derive(Debug, Deserialize)]
struct OllamaMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
}

/// Tool call from Ollama chat API
#[derive(Debug, Deserialize)]
struct OllamaToolCall {
    function: OllamaToolCallFunction,
}

/// Tool call function from Ollama chat API
#[derive(Debug, Deserialize)]
struct OllamaToolCallFunction {
    name: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

/// State for think tag parsing
#[derive(Debug, Clone, PartialEq)]
enum ThinkState {
    Normal,
    InThinking,
}

/// Adapter for Ollama JSON stream format.
///
/// Handles both the generate API (response field) and chat API (message field)
/// formats, with support for thinking tags, native tool calls, and thinking
/// content extracted by the SDK.
pub struct OllamaAdapter {
    model: String,
    /// Whether this model supports thinking
    thinking_enabled: bool,
    /// State for think tag parsing
    state: ThinkState,
    /// Buffer for tag detection
    buffer: String,
    /// Counter for generating tool call IDs
    tool_call_counter: usize,
}

impl OllamaAdapter {
    pub fn new(model: impl Into<String>) -> Self {
        let model = model.into();
        let thinking_enabled = Self::model_supports_thinking_static(&model);
        Self {
            model,
            thinking_enabled,
            state: ThinkState::Normal,
            buffer: String::new(),
            tool_call_counter: 0,
        }
    }

    /// Check if a model name indicates thinking support
    fn model_supports_thinking_static(model: &str) -> bool {
        let model_lower = model.to_lowercase();

        // Check against known thinking models
        for known in THINKING_MODELS {
            if model_lower.contains(known) {
                return true;
            }
        }

        // Check for r1/qwq patterns
        model_lower.contains("r1") || model_lower.contains("qwq")
    }

    /// Process buffer for think tags (similar to DeepSeek)
    fn process_buffer(&mut self) -> Vec<UnifiedStreamEvent> {
        if !self.thinking_enabled {
            // No thinking support, emit all as text
            let text = std::mem::take(&mut self.buffer);
            if text.is_empty() {
                return vec![];
            }
            return vec![UnifiedStreamEvent::TextDelta { content: text }];
        }

        let mut events = vec![];

        while !self.buffer.is_empty() {
            match self.state {
                ThinkState::Normal => {
                    if let Some(start_pos) = self.buffer.find("<think>") {
                        if start_pos > 0 {
                            let text = self.buffer[..start_pos].to_string();
                            if !text.is_empty() {
                                events.push(UnifiedStreamEvent::TextDelta { content: text });
                            }
                        }
                        self.buffer = self.buffer[start_pos + 7..].to_string();
                        self.state = ThinkState::InThinking;
                        events.push(UnifiedStreamEvent::ThinkingStart { thinking_id: None });
                    } else if self.buffer.ends_with('<')
                        || self.buffer.ends_with("<t")
                        || self.buffer.ends_with("<th")
                        || self.buffer.ends_with("<thi")
                        || self.buffer.ends_with("<thin")
                        || self.buffer.ends_with("<think")
                    {
                        break;
                    } else {
                        let text = std::mem::take(&mut self.buffer);
                        if !text.is_empty() {
                            events.push(UnifiedStreamEvent::TextDelta { content: text });
                        }
                        break;
                    }
                }
                ThinkState::InThinking => {
                    if let Some(end_pos) = self.buffer.find("</think>") {
                        if end_pos > 0 {
                            let thinking = self.buffer[..end_pos].to_string();
                            if !thinking.is_empty() {
                                events.push(UnifiedStreamEvent::ThinkingDelta {
                                    content: thinking,
                                    thinking_id: None,
                                });
                            }
                        }
                        self.buffer = self.buffer[end_pos + 8..].to_string();
                        self.state = ThinkState::Normal;
                        events.push(UnifiedStreamEvent::ThinkingEnd { thinking_id: None });
                    } else if self.buffer.ends_with('<')
                        || self.buffer.ends_with("</")
                        || self.buffer.ends_with("</t")
                        || self.buffer.ends_with("</th")
                        || self.buffer.ends_with("</thi")
                        || self.buffer.ends_with("</thin")
                        || self.buffer.ends_with("</think")
                    {
                        break;
                    } else {
                        let thinking = std::mem::take(&mut self.buffer);
                        if !thinking.is_empty() {
                            events.push(UnifiedStreamEvent::ThinkingDelta {
                                content: thinking,
                                thinking_id: None,
                            });
                        }
                        break;
                    }
                }
            }
        }

        events
    }
}

impl StreamAdapter for OllamaAdapter {
    fn provider_name(&self) -> &'static str {
        "ollama"
    }

    fn supports_thinking(&self) -> bool {
        self.thinking_enabled
    }

    fn supports_tools(&self) -> bool {
        // Native tool calling is now supported through ollama-rs SDK
        true
    }

    fn adapt(&mut self, input: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }

        let response: OllamaResponse =
            serde_json::from_str(trimmed).map_err(|e| AdapterError::ParseError(e.to_string()))?;

        let mut events = vec![];

        // Handle chat API message format
        if let Some(ref message) = response.message {
            // Handle thinking content from SDK (separate thinking field)
            if let Some(ref thinking) = message.thinking {
                if !thinking.is_empty() {
                    events.push(UnifiedStreamEvent::ThinkingStart { thinking_id: None });
                    events.push(UnifiedStreamEvent::ThinkingDelta {
                        content: thinking.clone(),
                        thinking_id: None,
                    });
                    events.push(UnifiedStreamEvent::ThinkingEnd { thinking_id: None });
                }
            }

            // Handle text content (may contain <think> tags for inline thinking)
            if let Some(ref content) = message.content {
                if !content.is_empty() {
                    self.buffer.push_str(content);
                    events.extend(self.process_buffer());
                }
            }

            // Handle tool calls
            for tc in &message.tool_calls {
                let tool_id = format!("call_{}", self.tool_call_counter);
                self.tool_call_counter += 1;
                let arguments = serde_json::to_string(&tc.function.arguments)
                    .unwrap_or_else(|_| "{}".to_string());
                events.push(UnifiedStreamEvent::ToolComplete {
                    tool_id,
                    tool_name: tc.function.name.clone(),
                    arguments,
                });
            }
        }

        // Handle generate API response format (backward compatibility)
        if let Some(content) = response.response {
            if !content.is_empty() {
                self.buffer.push_str(&content);
                events.extend(self.process_buffer());
            }
        }

        // Handle completion
        if response.done {
            // Flush buffer
            events.extend(self.process_buffer());

            // End thinking if still in progress
            if self.state == ThinkState::InThinking {
                events.push(UnifiedStreamEvent::ThinkingEnd { thinking_id: None });
                self.state = ThinkState::Normal;
            }

            // Emit usage if available
            if response.prompt_eval_count.is_some() || response.eval_count.is_some() {
                events.push(UnifiedStreamEvent::Usage {
                    input_tokens: response.prompt_eval_count.unwrap_or(0),
                    output_tokens: response.eval_count.unwrap_or(0),
                    thinking_tokens: None,
                    cache_read_tokens: None,
                    cache_creation_tokens: None,
                });
            }

            events.push(UnifiedStreamEvent::Complete {
                stop_reason: Some("stop".to_string()),
            });
        }

        Ok(events)
    }

    fn reset(&mut self) {
        self.state = ThinkState::Normal;
        self.buffer.clear();
        self.tool_call_counter = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regular_model() {
        let mut adapter = OllamaAdapter::new("llama3.2");
        assert!(!adapter.supports_thinking());
        assert!(adapter.supports_tools()); // Now true with SDK support

        let events = adapter
            .adapt(r#"{"response": "Hello", "done": false}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::TextDelta { content } => {
                assert_eq!(content, "Hello");
            }
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_thinking_model() {
        let adapter = OllamaAdapter::new("deepseek-r1:14b");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_qwq_model() {
        let adapter = OllamaAdapter::new("qwq:32b");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_done_response() {
        let mut adapter = OllamaAdapter::new("llama3.2");

        let events = adapter
            .adapt(r#"{"done": true, "prompt_eval_count": 50, "eval_count": 100}"#)
            .unwrap();

        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::Usage { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::Complete { .. })));
    }

    #[test]
    fn test_thinking_with_tags() {
        let mut adapter = OllamaAdapter::new("deepseek-r1");

        let events = adapter
            .adapt(r#"{"response": "<think>analyzing", "done": false}"#)
            .unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::ThinkingStart { .. })));

        let events = adapter
            .adapt(r#"{"response": "</think>result", "done": false}"#)
            .unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::ThinkingEnd { .. })));
        assert!(events.iter().any(
            |e| matches!(e, UnifiedStreamEvent::TextDelta { content } if content == "result")
        ));
    }

    #[test]
    fn test_chat_api_message_format() {
        let mut adapter = OllamaAdapter::new("llama3.2");

        let events = adapter
            .adapt(r#"{"message": {"content": "Hello world"}, "done": false}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::TextDelta { content } => {
                assert_eq!(content, "Hello world");
            }
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_chat_api_tool_calls() {
        let mut adapter = OllamaAdapter::new("llama3.2");

        let events = adapter
            .adapt(r#"{"message": {"content": "", "tool_calls": [{"function": {"name": "read_file", "arguments": {"path": "/test"}}}]}, "done": false}"#)
            .unwrap();

        assert!(events.iter().any(|e| matches!(
            e,
            UnifiedStreamEvent::ToolComplete {
                tool_name,
                ..
            } if tool_name == "read_file"
        )));
    }

    #[test]
    fn test_chat_api_thinking_field() {
        let mut adapter = OllamaAdapter::new("deepseek-r1");

        let events = adapter
            .adapt(r#"{"message": {"content": "answer", "thinking": "reasoning here"}, "done": false}"#)
            .unwrap();

        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::ThinkingStart { .. })));
        assert!(events.iter().any(
            |e| matches!(e, UnifiedStreamEvent::ThinkingDelta { content, .. } if content == "reasoning here")
        ));
        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::ThinkingEnd { .. })));
        assert!(events.iter().any(
            |e| matches!(e, UnifiedStreamEvent::TextDelta { content } if content == "answer")
        ));
    }

    #[test]
    fn test_reset() {
        let mut adapter = OllamaAdapter::new("deepseek-r1");

        // Start some thinking
        let _ = adapter.adapt(r#"{"response": "<think>analyzing", "done": false}"#);
        assert_eq!(adapter.state, ThinkState::InThinking);

        // Reset
        adapter.reset();
        assert_eq!(adapter.state, ThinkState::Normal);
        assert!(adapter.buffer.is_empty());
        assert_eq!(adapter.tool_call_counter, 0);
    }
}
