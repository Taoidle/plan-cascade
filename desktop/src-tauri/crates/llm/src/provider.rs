//! LLM Provider Trait
//!
//! Defines the common interface for all LLM providers.

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::types::{
    FallbackToolFormatMode, LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message,
    ProviderConfig, ToolCallReliability, ToolDefinition,
};
use plan_cascade_core::streaming::UnifiedStreamEvent;

/// Trait that all LLM providers must implement.
///
/// Provides a unified interface for:
/// - Single message completions (send_message)
/// - Streaming completions (stream_message)
/// - Health checking
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Returns the provider name for identification.
    fn name(&self) -> &'static str;

    /// Returns the current model being used.
    fn model(&self) -> &str;

    /// Returns whether this provider supports extended thinking/reasoning.
    fn supports_thinking(&self) -> bool;

    /// Returns whether this provider supports tool calling.
    fn supports_tools(&self) -> bool;

    /// Returns the reliability classification for this provider's tool calling.
    ///
    /// - `Reliable`: Native tool calls work consistently (Anthropic, OpenAI).
    /// - `Unreliable`: API claims tool support but emission is inconsistent (Qwen, DeepSeek, GLM).
    /// - `None`: No native tool calling support (Ollama).
    ///
    /// Default returns `Reliable` for backward compatibility.
    fn tool_call_reliability(&self) -> ToolCallReliability {
        ToolCallReliability::Reliable
    }

    /// Returns the recommended FallbackToolFormatMode for this provider.
    ///
    /// Based on reliability: Reliable -> Off, Unreliable -> Soft, None -> Soft.
    /// Can be overridden by `ProviderConfig.fallback_tool_format_mode`.
    fn default_fallback_mode(&self) -> FallbackToolFormatMode {
        match self.tool_call_reliability() {
            ToolCallReliability::Reliable => FallbackToolFormatMode::Off,
            ToolCallReliability::Unreliable => FallbackToolFormatMode::Soft,
            ToolCallReliability::None => FallbackToolFormatMode::Soft,
        }
    }

    /// Returns whether this provider supports multimodal content (images).
    fn supports_multimodal(&self) -> bool {
        false // Default: text-only
    }

    /// Returns whether this provider has native web search enabled.
    ///
    /// When true, the provider injects search parameters at the API level
    /// (e.g., Qwen `enable_search`, GLM `web_search` tool type) and may
    /// return search citations alongside the response.
    fn supports_native_search(&self) -> bool {
        false
    }

    /// Returns the model's context window size in tokens.
    ///
    /// Used to derive token budgets for sub-agents. Providers should override
    /// this based on the model name. Default: 128,000.
    fn context_window(&self) -> u32 {
        128_000
    }

    /// Send a message and get a complete response.
    ///
    /// # Arguments
    /// * `messages` - Conversation history
    /// * `system` - Optional system prompt
    /// * `tools` - Available tools for the model to use
    ///
    /// # Returns
    /// Complete response from the model
    async fn send_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse>;

    /// Stream a message response via a channel.
    ///
    /// # Arguments
    /// * `messages` - Conversation history
    /// * `system` - Optional system prompt
    /// * `tools` - Available tools for the model to use
    /// * `tx` - Channel sender for streaming events
    ///
    /// # Returns
    /// Final complete response after streaming
    async fn stream_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        tx: mpsc::Sender<UnifiedStreamEvent>,
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse>;

    /// Check if the provider is healthy and reachable.
    ///
    /// For API providers, this validates the API key.
    /// For Ollama, this checks if the server is running.
    async fn health_check(&self) -> LlmResult<()>;

    /// Get the configuration for this provider.
    fn config(&self) -> &ProviderConfig;

    /// List available models (if supported by provider).
    ///
    /// Returns None if the provider doesn't support model listing.
    async fn list_models(&self) -> LlmResult<Option<Vec<String>>> {
        Ok(None)
    }
}

/// Helper function to create an error for missing API key
pub fn missing_api_key_error(provider: &str) -> LlmError {
    LlmError::AuthenticationFailed {
        message: format!("API key not configured for {}", provider),
    }
}

/// Helper function to parse HTTP error status codes
pub fn parse_http_error(status: u16, body: &str, provider: &str) -> LlmError {
    match status {
        401 => LlmError::AuthenticationFailed {
            message: format!("{}: Invalid API key", provider),
        },
        403 => LlmError::AuthenticationFailed {
            message: format!("{}: Access denied", provider),
        },
        404 => {
            // Try to extract model name from body
            LlmError::ModelNotFound {
                model: body.to_string(),
            }
        }
        429 => {
            // Try to parse retry-after from body
            LlmError::RateLimited {
                message: body.to_string(),
                retry_after: None,
            }
        }
        400 => LlmError::InvalidRequest {
            message: body.to_string(),
        },
        500..=599 => LlmError::ServerError {
            message: body.to_string(),
            status: Some(status),
        },
        _ => LlmError::Other {
            message: format!("HTTP {}: {}", status, body),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_api_key_error() {
        let err = missing_api_key_error("anthropic");
        match err {
            LlmError::AuthenticationFailed { message } => {
                assert!(message.contains("anthropic"));
            }
            _ => panic!("Expected AuthenticationFailed"),
        }
    }

    #[test]
    fn test_parse_http_error() {
        let err = parse_http_error(401, "unauthorized", "openai");
        assert!(matches!(err, LlmError::AuthenticationFailed { .. }));

        let err = parse_http_error(429, "rate limited", "openai");
        assert!(matches!(err, LlmError::RateLimited { .. }));

        let err = parse_http_error(500, "internal error", "openai");
        assert!(matches!(err, LlmError::ServerError { .. }));
    }
}
