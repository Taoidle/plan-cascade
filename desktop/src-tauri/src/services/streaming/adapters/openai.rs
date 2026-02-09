//! OpenAI API Adapter
//!
//! Handles OpenAI SSE format with reasoning_content support for o1/o3 models.

use crate::services::streaming::adapter::StreamAdapter;
use crate::services::streaming::unified::{AdapterError, UnifiedStreamEvent};
use serde::Deserialize;

/// Internal event types from OpenAI API SSE format
#[derive(Debug, Deserialize)]
struct OpenAIEvent {
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
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type")]
    #[serde(default)]
    tool_type: Option<String>,
    #[serde(default)]
    function: Option<FunctionCall>,
}

#[derive(Debug, Deserialize)]
struct FunctionCall {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    #[serde(default)]
    reasoning_tokens: Option<u32>,
}

/// Adapter for OpenAI API SSE format
pub struct OpenAIAdapter {
    model: String,
    /// Track if we're in a reasoning block
    in_reasoning: bool,
    /// Track tool calls being accumulated
    tool_id: Option<String>,
    tool_name: Option<String>,
    tool_args_buffer: String,
}

impl OpenAIAdapter {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            in_reasoning: false,
            tool_id: None,
            tool_name: None,
            tool_args_buffer: String::new(),
        }
    }

    /// Check if model supports reasoning (o1/o3 models)
    fn model_supports_reasoning(&self) -> bool {
        let model_lower = self.model.to_lowercase();
        model_lower.starts_with("o1") || model_lower.starts_with("o3")
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
}

impl StreamAdapter for OpenAIAdapter {
    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn supports_thinking(&self) -> bool {
        self.model_supports_reasoning()
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn adapt(&mut self, input: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        let trimmed = input.trim();

        // Handle SSE format: "data: {...}"
        let json_str = if trimmed.starts_with("data: ") {
            &trimmed[6..]
        } else if trimmed.is_empty() {
            return Ok(vec![]);
        } else {
            trimmed
        };

        if json_str.is_empty() || json_str == "[DONE]" {
            let mut events = vec![];
            // Flush any pending tool call
            if let Some(tool_event) = self.flush_pending_tool() {
                events.push(tool_event);
            }
            // End of stream - emit ThinkingEnd if we were in reasoning
            if self.in_reasoning {
                self.in_reasoning = false;
                events.push(UnifiedStreamEvent::ThinkingEnd { thinking_id: None });
            }
            return Ok(events);
        }

        let event: OpenAIEvent =
            serde_json::from_str(json_str).map_err(|e| AdapterError::ParseError(e.to_string()))?;

        let mut events = vec![];

        // Handle usage info
        if let Some(usage) = event.usage {
            events.push(UnifiedStreamEvent::Usage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
                thinking_tokens: usage.reasoning_tokens,
                cache_read_tokens: None,
                cache_creation_tokens: None,
            });
        }

        for choice in event.choices {
            if let Some(finish_reason) = choice.finish_reason {
                // Flush any pending tool call before completing
                if let Some(tool_event) = self.flush_pending_tool() {
                    events.push(tool_event);
                }
                // End any reasoning block
                if self.in_reasoning {
                    self.in_reasoning = false;
                    events.push(UnifiedStreamEvent::ThinkingEnd { thinking_id: None });
                }
                events.push(UnifiedStreamEvent::Complete {
                    stop_reason: Some(finish_reason),
                });
                continue;
            }

            if let Some(delta) = choice.delta {
                // Handle reasoning content (o1/o3 models)
                if let Some(reasoning) = delta.reasoning_content {
                    if !reasoning.is_empty() {
                        if !self.in_reasoning {
                            self.in_reasoning = true;
                            events.push(UnifiedStreamEvent::ThinkingStart { thinking_id: None });
                        }
                        events.push(UnifiedStreamEvent::ThinkingDelta {
                            content: reasoning,
                            thinking_id: None,
                        });
                    }
                }

                // Handle regular content
                if let Some(content) = delta.content {
                    if !content.is_empty() {
                        // If we were in reasoning, end it first
                        if self.in_reasoning {
                            self.in_reasoning = false;
                            events.push(UnifiedStreamEvent::ThinkingEnd { thinking_id: None });
                        }
                        events.push(UnifiedStreamEvent::TextDelta { content });
                    }
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
        self.in_reasoning = false;
        self.tool_id = None;
        self.tool_name = None;
        self.tool_args_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_delta() {
        let mut adapter = OpenAIAdapter::new("gpt-4");

        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"content": "Hello"}}]}"#)
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
    fn test_reasoning_content() {
        let mut adapter = OpenAIAdapter::new("o1-preview");
        assert!(adapter.supports_thinking());

        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"reasoning_content": "Let me think..."}}]}"#)
            .unwrap();
        assert_eq!(events.len(), 2);
        match &events[0] {
            UnifiedStreamEvent::ThinkingStart { .. } => {}
            _ => panic!("Expected ThinkingStart"),
        }
        match &events[1] {
            UnifiedStreamEvent::ThinkingDelta { content, .. } => {
                assert_eq!(content, "Let me think...");
            }
            _ => panic!("Expected ThinkingDelta"),
        }
    }

    #[test]
    fn test_finish_reason() {
        let mut adapter = OpenAIAdapter::new("gpt-4");

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

    #[test]
    fn test_gpt4_no_thinking() {
        let adapter = OpenAIAdapter::new("gpt-4");
        assert!(!adapter.supports_thinking());
    }

    #[test]
    fn test_o3_thinking() {
        let adapter = OpenAIAdapter::new("o3-mini");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_done_signal() {
        let mut adapter = OpenAIAdapter::new("gpt-4");
        let events = adapter.adapt("data: [DONE]").unwrap();
        assert!(events.is_empty());
    }
}
