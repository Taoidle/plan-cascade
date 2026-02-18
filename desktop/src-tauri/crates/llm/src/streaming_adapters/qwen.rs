//! Qwen (Alibaba Cloud DashScope) SSE Stream Adapter
//!
//! Handles DashScope SSE format with reasoning_content support for Qwen3/QwQ models.
//! DashScope uses the same OpenAI-compatible format with reasoning_content field.
//!
//! Note: Since the migration to the async-dashscope SDK (v0.12), the QwenProvider
//! in `services::llm::qwen` processes structured `GenerationOutput` stream chunks
//! directly from the SDK, bypassing this SSE adapter. This adapter remains available
//! for the `AdapterFactory` and any external SSE-based streaming scenarios.

use plan_cascade_core::streaming::{AdapterError, StreamAdapter, UnifiedStreamEvent};
use serde::Deserialize;

/// Internal event types from DashScope API SSE format
#[derive(Debug, Deserialize)]
struct QwenEvent {
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

/// Adapter for Qwen (DashScope) API SSE format
pub struct QwenAdapter {
    model: String,
    /// Track if we're in a reasoning block
    in_reasoning: bool,
    /// Track tool calls being accumulated
    tool_id: Option<String>,
    tool_name: Option<String>,
    tool_args_buffer: String,
}

impl QwenAdapter {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            in_reasoning: false,
            tool_id: None,
            tool_name: None,
            tool_args_buffer: String::new(),
        }
    }

    /// Check if model supports reasoning (Qwen3 series, QwQ models)
    fn model_supports_reasoning(&self) -> bool {
        let model_lower = self.model.to_lowercase();
        model_lower.contains("qwen3")
            || model_lower.contains("qwq")
            || model_lower.contains("thinking")
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

impl StreamAdapter for QwenAdapter {
    fn provider_name(&self) -> &'static str {
        "qwen"
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

        let event: QwenEvent =
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
                // Handle reasoning content (Qwen3 with enable_thinking, QwQ models)
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
                        // Only treat as a NEW tool call when id is present,
                        // non-empty, AND different from the pending tool_id.
                        // DashScope sends continuation chunks with id: ""
                        // (standard Qwen), or with the SAME non-empty id
                        // (Qwen3-Omni-Flash). Both should NOT flush the
                        // pending tool or start a new one.
                        if let Some(id) = tc.id.as_deref() {
                            if !id.is_empty() && self.tool_id.as_deref() != Some(id) {
                                // New tool call starting (different id) — flush any previous pending tool
                                if let Some(tool_event) = self.flush_pending_tool() {
                                    events.push(tool_event);
                                }
                                self.tool_id = Some(id.to_string());
                                if let Some(func) = &tc.function {
                                    self.tool_name = func.name.clone().filter(|n| !n.is_empty());
                                }
                                self.tool_args_buffer.clear();

                                if let Some(name) = &self.tool_name {
                                    events.push(UnifiedStreamEvent::ToolStart {
                                        tool_id: id.to_string(),
                                        tool_name: name.clone(),
                                        arguments: None,
                                    });
                                }
                            }
                        }

                        // Accumulate function arguments (and pick up tool name
                        // from continuation chunks if we don't have one yet)
                        if let Some(func) = tc.function {
                            if self.tool_name.is_none() {
                                if let Some(name) = func.name.as_ref().filter(|n| !n.is_empty()) {
                                    self.tool_name = Some(name.clone());
                                }
                            }
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
        let mut adapter = QwenAdapter::new("qwen-plus");

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
        let mut adapter = QwenAdapter::new("qwen3-plus");
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
        let mut adapter = QwenAdapter::new("qwen-plus");

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
    fn test_qwen_plus_no_thinking() {
        let adapter = QwenAdapter::new("qwen-plus");
        assert!(!adapter.supports_thinking());
    }

    #[test]
    fn test_qwen3_thinking() {
        let adapter = QwenAdapter::new("qwen3-max");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_qwq_thinking() {
        let adapter = QwenAdapter::new("qwq-plus");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_done_signal() {
        let mut adapter = QwenAdapter::new("qwen-plus");
        let events = adapter.adapt("data: [DONE]").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_tool_call_empty_id_continuation() {
        let mut adapter = QwenAdapter::new("qwen3-max");

        // First chunk: new tool call with a real ID
        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"tool_calls": [{"id": "call_abc123", "type": "function", "function": {"name": "Read", "arguments": "{\"file"}}]}}]}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ToolStart { tool_id, tool_name, .. } => {
                assert_eq!(tool_id, "call_abc123");
                assert_eq!(tool_name, "Read");
            }
            _ => panic!("Expected ToolStart"),
        }

        // Continuation chunk: empty id should NOT flush or start a new tool
        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"tool_calls": [{"id": "", "function": {"arguments": "_path\": \"src/main.rs\"}"}}]}}]}"#)
            .unwrap();
        // Should produce no events (just accumulates arguments)
        assert!(events.is_empty());

        // Done signal should flush the completed tool
        let events = adapter.adapt("data: [DONE]").unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ToolComplete { tool_id, tool_name, arguments } => {
                assert_eq!(tool_id, "call_abc123");
                assert_eq!(tool_name, "Read");
                assert!(arguments.contains("file_path"));
                assert!(arguments.contains("src/main.rs"));
            }
            _ => panic!("Expected ToolComplete"),
        }
    }

    #[test]
    fn test_empty_tool_name_filtered() {
        let mut adapter = QwenAdapter::new("qwen3-max");

        // First chunk: tool call with empty string name (Qwen3-MAX bug)
        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"tool_calls": [{"id": "call_xyz", "type": "function", "function": {"name": "", "arguments": "{\"path\":"}}]}}]}"#)
            .unwrap();
        // No ToolStart because tool_name is empty and gets filtered to None
        assert!(events.is_empty(), "Expected no events when tool name is empty, got: {:?}", events);

        // Continuation chunk with arguments
        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"tool_calls": [{"id": "", "function": {"arguments": " \"foo\"}"}}]}}]}"#)
            .unwrap();
        assert!(events.is_empty());

        // Done: flush_pending_tool requires both tool_id and tool_name to be Some.
        // Since tool_name was filtered to None, no ToolComplete should be emitted.
        let events = adapter.adapt("data: [DONE]").unwrap();
        assert!(events.is_empty(), "Expected no ToolComplete with empty tool name, got: {:?}", events);
    }

    #[test]
    fn test_same_id_continuation_no_premature_flush() {
        let mut adapter = QwenAdapter::new("qwen3-omni-flash");

        // First chunk: new tool call with id and name
        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"tool_calls": [{"id": "call_omni_1", "type": "function", "function": {"name": "Bash", "arguments": "{\"cmd\":"}}]}}]}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ToolStart { tool_id, tool_name, .. } => {
                assert_eq!(tool_id, "call_omni_1");
                assert_eq!(tool_name, "Bash");
            }
            _ => panic!("Expected ToolStart, got {:?}", events[0]),
        }

        // Continuation chunk: SAME non-empty id (Qwen3-Omni-Flash behavior)
        // Should NOT flush and NOT start a new tool — just accumulate arguments
        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"tool_calls": [{"id": "call_omni_1", "function": {"arguments": " \"ls -la\"}"}}]}}]}"#)
            .unwrap();
        assert!(events.is_empty(), "Same-id continuation should not produce events, got: {:?}", events);

        // Flush via finish_reason
        let events = adapter
            .adapt(r#"data: {"choices": [{"finish_reason": "tool_calls"}]}"#)
            .unwrap();
        // Should get ToolComplete then Complete
        assert_eq!(events.len(), 2, "Expected ToolComplete + Complete, got: {:?}", events);
        match &events[0] {
            UnifiedStreamEvent::ToolComplete { tool_id, tool_name, arguments } => {
                assert_eq!(tool_id, "call_omni_1");
                assert_eq!(tool_name, "Bash");
                assert_eq!(arguments, r#"{"cmd": "ls -la"}"#);
            }
            _ => panic!("Expected ToolComplete, got {:?}", events[0]),
        }
        match &events[1] {
            UnifiedStreamEvent::Complete { stop_reason } => {
                assert_eq!(stop_reason, &Some("tool_calls".to_string()));
            }
            _ => panic!("Expected Complete, got {:?}", events[1]),
        }
    }

    #[test]
    fn test_different_id_flushes_previous_tool() {
        let mut adapter = QwenAdapter::new("qwen3-max");

        // First tool call
        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"tool_calls": [{"id": "call_first", "type": "function", "function": {"name": "Read", "arguments": "{\"path\": \"a.rs\"}"}}]}}]}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ToolStart { tool_id, tool_name, .. } => {
                assert_eq!(tool_id, "call_first");
                assert_eq!(tool_name, "Read");
            }
            _ => panic!("Expected ToolStart for first tool"),
        }

        // Second tool call with a DIFFERENT id — should flush the first
        let events = adapter
            .adapt(r#"data: {"choices": [{"delta": {"tool_calls": [{"id": "call_second", "type": "function", "function": {"name": "Bash", "arguments": "{\"cmd\": \"echo hi\"}"}}]}}]}"#)
            .unwrap();
        // Should get ToolComplete (flushed first tool) + ToolStart (new tool)
        assert_eq!(events.len(), 2, "Expected ToolComplete + ToolStart, got: {:?}", events);
        match &events[0] {
            UnifiedStreamEvent::ToolComplete { tool_id, tool_name, arguments } => {
                assert_eq!(tool_id, "call_first");
                assert_eq!(tool_name, "Read");
                assert!(arguments.contains("a.rs"));
            }
            _ => panic!("Expected ToolComplete for first tool, got {:?}", events[0]),
        }
        match &events[1] {
            UnifiedStreamEvent::ToolStart { tool_id, tool_name, .. } => {
                assert_eq!(tool_id, "call_second");
                assert_eq!(tool_name, "Bash");
            }
            _ => panic!("Expected ToolStart for second tool, got {:?}", events[1]),
        }

        // Flush second tool via [DONE]
        let events = adapter.adapt("data: [DONE]").unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ToolComplete { tool_id, tool_name, arguments } => {
                assert_eq!(tool_id, "call_second");
                assert_eq!(tool_name, "Bash");
                assert!(arguments.contains("echo hi"));
            }
            _ => panic!("Expected ToolComplete for second tool"),
        }
    }
}
