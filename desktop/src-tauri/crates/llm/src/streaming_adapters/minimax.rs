//! MiniMax API Adapter
//!
//! Handles the Anthropic-protocol SSE format from MiniMax's Anthropic-compatible
//! endpoint (api.minimax.io/anthropic/v1). Delegates to ClaudeApiAdapter since
//! MiniMax uses the same SSE event structure as Anthropic.
//!
//! ADR-003: After SDK migration, MiniMax speaks Anthropic protocol.
//! ADR-004: Thinking/reasoning blocks may not appear in SDK v0.6 streaming.

use super::claude_api::ClaudeApiAdapter;
use plan_cascade_core::streaming::{AdapterError, StreamAdapter, UnifiedStreamEvent};

/// Adapter for MiniMax API SSE format (Anthropic-compatible protocol).
///
/// Since MiniMax now uses the Anthropic protocol endpoint, this adapter
/// delegates all parsing to ClaudeApiAdapter. The wrapper preserves the
/// MiniMax-specific provider_name for logging and model detection logic.
pub struct MinimaxAdapter {
    model: String,
    inner: ClaudeApiAdapter,
}

impl MinimaxAdapter {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            inner: ClaudeApiAdapter::new(),
        }
    }

    /// Check if model supports reasoning (M2 series)
    fn model_supports_reasoning(&self) -> bool {
        let model_lower = self.model.to_lowercase();
        model_lower.contains("minimax-m2")
    }
}

impl StreamAdapter for MinimaxAdapter {
    fn provider_name(&self) -> &'static str {
        "minimax"
    }

    fn supports_thinking(&self) -> bool {
        self.model_supports_reasoning()
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn adapt(&mut self, input: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        self.inner.adapt(input)
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_delta() {
        let mut adapter = MinimaxAdapter::new("MiniMax-M2.5");

        // Anthropic SSE format: content_block_delta with text_delta
        let events = adapter
            .adapt(r#"data: {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}"#)
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
    fn test_thinking_block() {
        let mut adapter = MinimaxAdapter::new("MiniMax-M2.5");
        assert!(adapter.supports_thinking());

        // Anthropic format: content_block_start with thinking type
        let events = adapter
            .adapt(r#"data: {"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking_id": "t1"}}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ThinkingStart { thinking_id } => {
                assert_eq!(thinking_id, &Some("t1".to_string()));
            }
            _ => panic!("Expected ThinkingStart"),
        }

        let events = adapter
            .adapt(r#"data: {"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "reasoning..."}}"#)
            .unwrap();
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
        let mut adapter = MinimaxAdapter::new("MiniMax-M2");

        let events = adapter.adapt(r#"data: {"type": "message_stop"}"#).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::Complete { .. } => {}
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_message_delta_stop_reason() {
        let mut adapter = MinimaxAdapter::new("MiniMax-M2.5");

        let events = adapter
            .adapt(r#"data: {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 42}}"#)
            .unwrap();
        // Should produce Usage + Complete
        assert!(events.len() >= 1);
        let has_complete = events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::Complete { .. }));
        assert!(has_complete, "Expected Complete event");
    }

    #[test]
    fn test_m2_5_thinking() {
        let adapter = MinimaxAdapter::new("MiniMax-M2.5");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_m2_1_thinking() {
        let adapter = MinimaxAdapter::new("MiniMax-M2.1-highspeed");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_text_01_no_thinking() {
        let adapter = MinimaxAdapter::new("MiniMax-Text-01");
        assert!(!adapter.supports_thinking());
    }

    #[test]
    fn test_done_signal() {
        let mut adapter = MinimaxAdapter::new("MiniMax-M2");
        let events = adapter.adapt("data: [DONE]").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_empty_line() {
        let mut adapter = MinimaxAdapter::new("MiniMax-M2");
        let events = adapter.adapt("").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_tool_use_via_anthropic_protocol() {
        let mut adapter = MinimaxAdapter::new("MiniMax-M2.5");

        // Tool use start
        let events = adapter
            .adapt(r#"data: {"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "toolu_abc", "name": "Read"}}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ToolStart {
                tool_id, tool_name, ..
            } => {
                assert_eq!(tool_id, "toolu_abc");
                assert_eq!(tool_name, "Read");
            }
            _ => panic!("Expected ToolStart"),
        }

        // Input json delta
        let events = adapter
            .adapt(r#"data: {"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"file_path\": \"main.rs\"}"}}"#)
            .unwrap();
        assert!(events.is_empty(), "InputJsonDelta should be buffered");

        // Content block stop -> flush tool
        let events = adapter
            .adapt(r#"data: {"type": "content_block_stop", "index": 1}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::ToolComplete {
                tool_id,
                tool_name,
                arguments,
            } => {
                assert_eq!(tool_id, "toolu_abc");
                assert_eq!(tool_name, "Read");
                assert!(arguments.contains("file_path"));
            }
            _ => panic!("Expected ToolComplete"),
        }
    }

    #[test]
    fn test_usage_from_message_start() {
        let mut adapter = MinimaxAdapter::new("MiniMax-M2.5");

        let events = adapter
            .adapt(r#"data: {"type": "message_start", "message": {"id": "msg_1", "model": "MiniMax-M2.5", "role": "assistant", "content": [], "usage": {"input_tokens": 100, "output_tokens": 0}}}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::Usage {
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(*input_tokens, 100);
                assert_eq!(*output_tokens, 0);
            }
            _ => panic!("Expected Usage"),
        }
    }

    #[test]
    fn test_provider_name() {
        let adapter = MinimaxAdapter::new("MiniMax-M2.5");
        assert_eq!(adapter.provider_name(), "minimax");
    }

    #[test]
    fn test_reset() {
        let mut adapter = MinimaxAdapter::new("MiniMax-M2.5");
        // Start a tool block
        let _ = adapter
            .adapt(r#"data: {"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "t1", "name": "Read"}}"#);
        // Reset clears state
        adapter.reset();
        // After reset, content_block_stop should not emit ToolComplete
        // (tool state was cleared)
        let events = adapter
            .adapt(r#"data: {"type": "content_block_stop", "index": 0}"#)
            .unwrap();
        assert!(
            events.is_empty(),
            "After reset, no pending tool should be flushed"
        );
    }
}
