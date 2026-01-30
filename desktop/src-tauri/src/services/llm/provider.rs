//! LLM Provider Trait
//!
//! Defines the common interface for all LLM providers.

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::types::{
    LlmError, LlmResponse, LlmResult, Message, ProviderConfig, ToolDefinition,
};
use crate::services::streaming::UnifiedStreamEvent;

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
