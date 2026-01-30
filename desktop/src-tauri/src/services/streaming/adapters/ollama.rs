//! Ollama API Adapter
//!
//! Handles Ollama JSON stream format with model-dependent thinking detection.

use serde::Deserialize;
use crate::services::streaming::adapter::StreamAdapter;
use crate::services::streaming::unified::{AdapterError, UnifiedStreamEvent};

/// Known models that support thinking via <think> tags
const THINKING_MODELS: &[&str] = &[
    "deepseek-r1",
    "deepseek-reasoner",
    "qwq",
    "qwen-qwq",
];

/// Ollama response format
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    #[serde(default)]
    response: Option<String>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<u32>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
}

/// State for think tag parsing
#[derive(Debug, Clone, PartialEq)]
enum ThinkState {
    Normal,
    InThinking,
}

/// Adapter for Ollama JSON stream format
pub struct OllamaAdapter {
    model: String,
    /// Whether this model supports thinking
    thinking_enabled: bool,
    /// State for think tag parsing
    state: ThinkState,
    /// Buffer for tag detection
    buffer: String,
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
                    } else if self.buffer.ends_with('<') ||
                              self.buffer.ends_with("<t") ||
                              self.buffer.ends_with("<th") ||
                              self.buffer.ends_with("<thi") ||
                              self.buffer.ends_with("<thin") ||
                              self.buffer.ends_with("<think") {
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
                    } else if self.buffer.ends_with('<') ||
                              self.buffer.ends_with("</") ||
                              self.buffer.ends_with("</t") ||
                              self.buffer.ends_with("</th") ||
                              self.buffer.ends_with("</thi") ||
                              self.buffer.ends_with("</thin") ||
                              self.buffer.ends_with("</think") {
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
        // Ollama tool support depends on model, assume true for now
        true
    }

    fn adapt(&mut self, input: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }

        let response: OllamaResponse = serde_json::from_str(trimmed)
            .map_err(|e| AdapterError::ParseError(e.to_string()))?;

        let mut events = vec![];

        // Handle response content
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regular_model() {
        let mut adapter = OllamaAdapter::new("llama3.2");
        assert!(!adapter.supports_thinking());

        let events = adapter.adapt(r#"{"response": "Hello", "done": false}"#).unwrap();
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

        let events = adapter.adapt(r#"{"done": true, "prompt_eval_count": 50, "eval_count": 100}"#).unwrap();

        assert!(events.iter().any(|e| matches!(e, UnifiedStreamEvent::Usage { .. })));
        assert!(events.iter().any(|e| matches!(e, UnifiedStreamEvent::Complete { .. })));
    }

    #[test]
    fn test_thinking_with_tags() {
        let mut adapter = OllamaAdapter::new("deepseek-r1");

        let events = adapter.adapt(r#"{"response": "<think>analyzing", "done": false}"#).unwrap();
        assert!(events.iter().any(|e| matches!(e, UnifiedStreamEvent::ThinkingStart { .. })));

        let events = adapter.adapt(r#"{"response": "</think>result", "done": false}"#).unwrap();
        assert!(events.iter().any(|e| matches!(e, UnifiedStreamEvent::ThinkingEnd { .. })));
        assert!(events.iter().any(|e| matches!(e, UnifiedStreamEvent::TextDelta { content } if content == "result")));
    }
}
