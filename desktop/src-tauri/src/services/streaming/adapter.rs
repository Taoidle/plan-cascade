//! Stream Adapter Trait
//!
//! Defines the common interface that all provider adapters must implement.

use super::unified::{AdapterError, UnifiedStreamEvent};

/// Trait for adapting provider-specific stream formats to unified events.
///
/// All provider adapters (Claude, OpenAI, DeepSeek, Ollama) implement this trait
/// to provide a consistent interface for stream processing.
pub trait StreamAdapter: Send + Sync {
    /// Returns the provider name for logging and identification.
    fn provider_name(&self) -> &'static str;

    /// Returns whether this adapter/provider supports thinking blocks.
    ///
    /// This depends on both the provider and the specific model:
    /// - Claude: Always true (native extended thinking)
    /// - OpenAI: True for o1/o3 models (reasoning_content)
    /// - DeepSeek: True for R1 models (<think> tags)
    /// - Ollama: Model-dependent (deepseek-r1, qwq, etc.)
    fn supports_thinking(&self) -> bool;

    /// Returns whether this adapter/provider supports tool calls.
    fn supports_tools(&self) -> bool;

    /// Adapt a raw stream line/chunk to unified events.
    ///
    /// A single input line may produce zero, one, or multiple events.
    /// For example:
    /// - Empty/keepalive lines produce zero events
    /// - Text deltas produce one TextDelta event
    /// - Tool calls may produce ToolStart followed by ToolResult
    ///
    /// # Arguments
    /// * `input` - Raw stream line/chunk from the provider
    ///
    /// # Returns
    /// * `Ok(Vec<UnifiedStreamEvent>)` - Zero or more unified events
    /// * `Err(AdapterError)` - If the input couldn't be parsed
    fn adapt(&mut self, input: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError>;

    /// Reset adapter state for a new stream.
    ///
    /// Called when starting a new streaming session to clear any accumulated state
    /// (e.g., partial thinking blocks, tool call buffers).
    fn reset(&mut self) {
        // Default implementation does nothing
        // Stateful adapters should override this
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock adapter for testing the trait
    struct MockAdapter {
        name: &'static str,
        thinking: bool,
        tools: bool,
    }

    impl StreamAdapter for MockAdapter {
        fn provider_name(&self) -> &'static str {
            self.name
        }

        fn supports_thinking(&self) -> bool {
            self.thinking
        }

        fn supports_tools(&self) -> bool {
            self.tools
        }

        fn adapt(&mut self, input: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
            if input.is_empty() {
                return Ok(vec![]);
            }
            Ok(vec![UnifiedStreamEvent::TextDelta {
                content: input.to_string(),
            }])
        }
    }

    #[test]
    fn test_mock_adapter() {
        let mut adapter = MockAdapter {
            name: "test",
            thinking: true,
            tools: false,
        };

        assert_eq!(adapter.provider_name(), "test");
        assert!(adapter.supports_thinking());
        assert!(!adapter.supports_tools());

        let events = adapter.adapt("hello").unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            UnifiedStreamEvent::TextDelta { content } => {
                assert_eq!(content, "hello");
            }
            _ => panic!("Expected TextDelta"),
        }

        let events = adapter.adapt("").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_adapter_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockAdapter>();
    }
}
