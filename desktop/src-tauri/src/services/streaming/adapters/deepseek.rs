//! DeepSeek API Adapter
//!
//! Handles DeepSeek SSE format with <think></think> tag parsing for R1 models.

use crate::services::streaming::adapter::StreamAdapter;
use crate::services::streaming::unified::{AdapterError, UnifiedStreamEvent};
use serde::Deserialize;

/// Internal event types from DeepSeek API (OpenAI-compatible format)
#[derive(Debug, Deserialize)]
struct DeepSeekEvent {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    #[serde(default)]
    delta: Option<Delta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ToolCallDelta {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<FunctionCallDelta>,
}

#[derive(Debug, Deserialize)]
struct FunctionCallDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

/// State machine for parsing <think> tags
#[derive(Debug, Clone, PartialEq)]
enum ThinkState {
    /// Not in a thinking block, looking for <think>
    Normal,
    /// Inside a thinking block, looking for </think>
    InThinking,
}

/// Adapter for DeepSeek API with R1 thinking support
pub struct DeepSeekAdapter {
    model: String,
    /// Current state of think tag parsing
    state: ThinkState,
    /// Buffer for accumulating content to check for tags
    buffer: String,
    /// Track tool calls being accumulated
    tool_id: Option<String>,
    tool_name: Option<String>,
    tool_args_buffer: String,
}

impl DeepSeekAdapter {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            state: ThinkState::Normal,
            buffer: String::new(),
            tool_id: None,
            tool_name: None,
            tool_args_buffer: String::new(),
        }
    }

    /// Check if model supports thinking (R1 models)
    fn model_supports_thinking(&self) -> bool {
        let model_lower = self.model.to_lowercase();
        model_lower.contains("r1") || model_lower.contains("deepseek-reasoner")
    }

    /// Flush any pending tool call, emitting a ToolComplete event
    fn flush_pending_tool(&mut self) -> Option<UnifiedStreamEvent> {
        if let (Some(id), Some(name)) = (self.tool_id.take(), self.tool_name.take()) {
            let args = std::mem::take(&mut self.tool_args_buffer);
            Some(UnifiedStreamEvent::ToolComplete {
                tool_id: id,
                tool_name: name,
                arguments: args,
            })
        } else {
            None
        }
    }

    /// Process buffered content and extract thinking/text events
    fn process_buffer(&mut self) -> Vec<UnifiedStreamEvent> {
        let mut events = vec![];

        while !self.buffer.is_empty() {
            match self.state {
                ThinkState::Normal => {
                    // Look for <think> tag
                    if let Some(start_pos) = self.buffer.find("<think>") {
                        // Emit any text before the tag
                        if start_pos > 0 {
                            let text = self.buffer[..start_pos].to_string();
                            if !text.is_empty() {
                                events.push(UnifiedStreamEvent::TextDelta { content: text });
                            }
                        }
                        // Remove processed content and the tag
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
                        // Might be start of <think> tag, wait for more content
                        break;
                    } else {
                        // No tag found, emit all as text
                        let text = std::mem::take(&mut self.buffer);
                        if !text.is_empty() {
                            events.push(UnifiedStreamEvent::TextDelta { content: text });
                        }
                        break;
                    }
                }
                ThinkState::InThinking => {
                    // Look for </think> tag
                    if let Some(end_pos) = self.buffer.find("</think>") {
                        // Emit thinking content before the tag
                        if end_pos > 0 {
                            let thinking = self.buffer[..end_pos].to_string();
                            if !thinking.is_empty() {
                                events.push(UnifiedStreamEvent::ThinkingDelta {
                                    content: thinking,
                                    thinking_id: None,
                                });
                            }
                        }
                        // Remove processed content and the tag
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
                        // Might be end of </think> tag, wait for more content
                        break;
                    } else {
                        // No tag found, emit all as thinking
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

impl StreamAdapter for DeepSeekAdapter {
    fn provider_name(&self) -> &'static str {
        "deepseek"
    }

    fn supports_thinking(&self) -> bool {
        self.model_supports_thinking()
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn adapt(&mut self, input: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        let trimmed = input.trim();

        // Handle SSE format
        let json_str = if trimmed.starts_with("data: ") {
            &trimmed[6..]
        } else if trimmed.is_empty() {
            return Ok(vec![]);
        } else {
            trimmed
        };

        if json_str.is_empty() || json_str == "[DONE]" {
            // Flush remaining buffer
            let mut events = self.process_buffer();
            // Flush any pending tool call
            if let Some(tool_event) = self.flush_pending_tool() {
                events.push(tool_event);
            }
            if self.state == ThinkState::InThinking {
                events.push(UnifiedStreamEvent::ThinkingEnd { thinking_id: None });
                self.state = ThinkState::Normal;
            }
            return Ok(events);
        }

        let event: DeepSeekEvent =
            serde_json::from_str(json_str).map_err(|e| AdapterError::ParseError(e.to_string()))?;

        let mut events = vec![];

        // Handle usage
        if let Some(usage) = event.usage {
            events.push(UnifiedStreamEvent::Usage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
                thinking_tokens: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
            });
        }

        for choice in event.choices {
            if let Some(finish_reason) = choice.finish_reason {
                // Flush buffer and end thinking if needed
                events.extend(self.process_buffer());
                // Flush any pending tool call
                if let Some(tool_event) = self.flush_pending_tool() {
                    events.push(tool_event);
                }
                if self.state == ThinkState::InThinking {
                    events.push(UnifiedStreamEvent::ThinkingEnd { thinking_id: None });
                    self.state = ThinkState::Normal;
                }
                events.push(UnifiedStreamEvent::Complete {
                    stop_reason: Some(finish_reason),
                });
                continue;
            }

            if let Some(delta) = choice.delta {
                if let Some(content) = delta.content {
                    // Add to buffer for tag processing
                    self.buffer.push_str(&content);
                    events.extend(self.process_buffer());
                }

                // Handle tool calls
                if let Some(tool_calls) = delta.tool_calls {
                    for tc in tool_calls {
                        if let Some(id) = tc.id {
                            // New tool call starting — flush any previous pending tool
                            if let Some(tool_event) = self.flush_pending_tool() {
                                events.push(tool_event);
                            }
                            self.tool_id = Some(id.clone());
                            if let Some(func) = &tc.function {
                                self.tool_name = func.name.clone();
                            }
                            self.tool_args_buffer.clear();

                            if let Some(name) = &self.tool_name {
                                events.push(UnifiedStreamEvent::ToolStart {
                                    tool_id: id,
                                    tool_name: name.clone(),
                                    arguments: None,
                                });
                            }
                        }

                        // Accumulate function arguments
                        if let Some(func) = tc.function {
                            if let Some(args) = func.arguments {
                                self.tool_args_buffer.push_str(&args);
                            }
                        }
                    }
                }
            }
        }

        Ok(events)
    }

    fn reset(&mut self) {
        self.state = ThinkState::Normal;
        self.buffer.clear();
        self.tool_id = None;
        self.tool_name = None;
        self.tool_args_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_think_tag_parsing() {
        let mut adapter = DeepSeekAdapter::new("deepseek-r1");
        assert!(adapter.supports_thinking());

        // Simulate stream with think tags
        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"content": "<think>Let me analyze"}}]}"#)
            .unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::ThinkingStart { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::ThinkingDelta { .. })));

        let events = adapter.adapt(r#"data: {"choices": [{"delta": {"content": " this problem.</think>The answer is"}}]}"#).unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::ThinkingEnd { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::TextDelta { .. })));
    }

    #[test]
    fn test_regular_model_no_thinking() {
        let adapter = DeepSeekAdapter::new("deepseek-chat");
        assert!(!adapter.supports_thinking());
    }

    #[test]
    fn test_r1_model_thinking() {
        let adapter = DeepSeekAdapter::new("deepseek-r1-lite");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_no_think_tags() {
        let mut adapter = DeepSeekAdapter::new("deepseek-r1");

        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"content": "Hello world"}}]}"#)
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
    fn test_finish_reason() {
        let mut adapter = DeepSeekAdapter::new("deepseek-r1");

        let events = adapter
            .adapt(r#"data: {"choices": [{"finish_reason": "stop"}]}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::Complete { stop_reason } => {
                assert_eq!(stop_reason, &Some("stop".to_string()));
            }
            _ => panic!("Expected Complete"),
        }
    }
}
