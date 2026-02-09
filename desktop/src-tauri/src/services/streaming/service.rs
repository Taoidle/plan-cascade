//! Unified Streaming Service
//!
//! Main service for processing LLM streams through adapters.

use tokio::sync::mpsc;

use super::adapter::StreamAdapter;
use super::factory::AdapterFactory;
use super::unified::{AdapterError, UnifiedStreamEvent};

/// Errors that can occur during stream processing
#[derive(Debug, Clone)]
pub enum StreamError {
    /// Error from the adapter during parsing
    AdapterError(AdapterError),
    /// Error sending events through the channel
    ChannelError(String),
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamError::AdapterError(e) => write!(f, "Adapter error: {}", e),
            StreamError::ChannelError(e) => write!(f, "Channel error: {}", e),
        }
    }
}

impl std::error::Error for StreamError {}

impl From<AdapterError> for StreamError {
    fn from(err: AdapterError) -> Self {
        StreamError::AdapterError(err)
    }
}

/// Unified streaming service that processes LLM streams through adapters.
pub struct UnifiedStreamingService {
    /// Provider name for logging
    provider: String,
    /// Model name for thinking detection
    model: String,
    /// The adapter for this provider/model
    adapter: Box<dyn StreamAdapter>,
    /// Optional event sender for async event emission
    event_tx: Option<mpsc::Sender<UnifiedStreamEvent>>,
}

impl UnifiedStreamingService {
    /// Create a new streaming service.
    ///
    /// # Arguments
    /// * `provider` - Provider name (claude-code, openai, etc.)
    /// * `model` - Model identifier
    /// * `event_tx` - Optional channel sender for event emission
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        event_tx: Option<mpsc::Sender<UnifiedStreamEvent>>,
    ) -> Self {
        let provider = provider.into();
        let model = model.into();
        let adapter = AdapterFactory::create(&provider, &model);

        Self {
            provider,
            model,
            adapter,
            event_tx,
        }
    }

    /// Process a raw stream line and return unified events.
    ///
    /// If an event channel is configured, events are also sent through it.
    pub async fn process_line(
        &mut self,
        line: &str,
    ) -> Result<Vec<UnifiedStreamEvent>, StreamError> {
        let events = self.adapter.adapt(line)?;

        // Send events through channel if configured
        if let Some(tx) = &self.event_tx {
            for event in &events {
                tx.send(event.clone())
                    .await
                    .map_err(|e| StreamError::ChannelError(e.to_string()))?;
            }
        }

        Ok(events)
    }

    /// Process a line synchronously (without sending through channel).
    pub fn process_line_sync(
        &mut self,
        line: &str,
    ) -> Result<Vec<UnifiedStreamEvent>, StreamError> {
        Ok(self.adapter.adapt(line)?)
    }

    /// Check if the current provider/model supports thinking blocks.
    pub fn supports_thinking(&self) -> bool {
        self.adapter.supports_thinking()
    }

    /// Check if the current provider/model supports tool calls.
    pub fn supports_tools(&self) -> bool {
        self.adapter.supports_tools()
    }

    /// Get a human-readable description of the thinking format.
    pub fn thinking_format(&self) -> &'static str {
        if !self.supports_thinking() {
            return "No thinking support";
        }

        match self.adapter.provider_name() {
            "claude-code" => "Claude Extended Thinking (native)",
            "claude-api" => "Claude Extended Thinking (API)",
            "openai" => {
                let model_lower = self.model.to_lowercase();
                if model_lower.starts_with("o1") {
                    "OpenAI o1 Reasoning"
                } else if model_lower.starts_with("o3") {
                    "OpenAI o3 Reasoning"
                } else {
                    "OpenAI Reasoning"
                }
            }
            "deepseek" => "DeepSeek R1 Thinking (<think> tags)",
            "ollama" => {
                let model_lower = self.model.to_lowercase();
                if model_lower.contains("qwq") {
                    "QwQ Thinking (<think> tags)"
                } else {
                    "Model Thinking (<think> tags)"
                }
            }
            _ => "Unknown thinking format",
        }
    }

    /// Get the provider name.
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Get the model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Reset the adapter state for a new stream.
    pub fn reset(&mut self) {
        self.adapter.reset();
    }
}

impl std::fmt::Debug for UnifiedStreamingService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedStreamingService")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("supports_thinking", &self.supports_thinking())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = UnifiedStreamingService::new("openai", "gpt-4", None);
        assert_eq!(service.provider(), "openai");
        assert_eq!(service.model(), "gpt-4");
        assert!(!service.supports_thinking());
    }

    #[test]
    fn test_thinking_format_descriptions() {
        let service = UnifiedStreamingService::new("claude-code", "claude-3-opus", None);
        assert_eq!(
            service.thinking_format(),
            "Claude Extended Thinking (native)"
        );

        let service = UnifiedStreamingService::new("openai", "o1-preview", None);
        assert_eq!(service.thinking_format(), "OpenAI o1 Reasoning");

        let service = UnifiedStreamingService::new("deepseek", "deepseek-r1", None);
        assert_eq!(
            service.thinking_format(),
            "DeepSeek R1 Thinking (<think> tags)"
        );

        let service = UnifiedStreamingService::new("ollama", "qwq:32b", None);
        assert_eq!(service.thinking_format(), "QwQ Thinking (<think> tags)");

        let service = UnifiedStreamingService::new("openai", "gpt-4", None);
        assert_eq!(service.thinking_format(), "No thinking support");
    }

    #[test]
    fn test_process_line_sync() {
        let mut service = UnifiedStreamingService::new("claude-code", "claude-3-opus", None);

        let events = service
            .process_line_sync(r#"{"type": "thinking", "thinking_id": "t1"}"#)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            UnifiedStreamEvent::ThinkingStart { .. }
        ));
    }

    #[tokio::test]
    async fn test_process_line_with_channel() {
        let (tx, mut rx) = mpsc::channel(10);
        let mut service = UnifiedStreamingService::new("claude-code", "claude-3-opus", Some(tx));

        let events = service.process_line(r#"{"type": "content_block_delta", "delta": {"type": "text_delta", "text": "Hello"}}"#).await.unwrap();

        assert_eq!(events.len(), 1);

        // Check event was also sent through channel
        let received = rx.recv().await.unwrap();
        match received {
            UnifiedStreamEvent::TextDelta { content } => {
                assert_eq!(content, "Hello");
            }
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_reset() {
        let mut service = UnifiedStreamingService::new("deepseek", "deepseek-r1", None);

        // Process some content that puts adapter in thinking state
        let _ = service
            .process_line_sync(r#"data: {"choices": [{"delta": {"content": "<think>thinking"}}]}"#);

        // Reset should clear state
        service.reset();

        // Process fresh content - should not be in thinking state
        let events = service
            .process_line_sync(r#"data: {"choices": [{"delta": {"content": "hello"}}]}"#)
            .unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, UnifiedStreamEvent::TextDelta { .. })));
    }
}
