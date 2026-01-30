//! Orchestrator Service
//!
//! Coordinates LLM provider calls with tool execution in an agentic loop.

use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::services::llm::{
    LlmProvider, LlmResponse, Message, MessageContent, ProviderConfig,
    ToolDefinition, UsageStats, AnthropicProvider, OpenAIProvider,
    DeepSeekProvider, OllamaProvider, ProviderType,
};
use crate::services::streaming::UnifiedStreamEvent;
use crate::services::tools::{ToolExecutor, get_tool_definitions};

/// Configuration for the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// LLM provider configuration
    pub provider: ProviderConfig,
    /// System prompt for the LLM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Maximum iterations before stopping (prevents infinite loops)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// Maximum total tokens to use
    #[serde(default = "default_max_tokens")]
    pub max_total_tokens: u32,
    /// Project root directory
    pub project_root: PathBuf,
    /// Whether to enable streaming
    #[serde(default = "default_streaming")]
    pub streaming: bool,
}

fn default_max_iterations() -> u32 {
    50
}

fn default_max_tokens() -> u32 {
    100_000
}

fn default_streaming() -> bool {
    true
}

/// Result of an orchestration execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Final response from the LLM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    /// Total usage across all iterations
    pub usage: UsageStats,
    /// Number of iterations performed
    pub iterations: u32,
    /// Whether execution completed successfully
    pub success: bool,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Orchestrator service for standalone LLM execution
pub struct OrchestratorService {
    config: OrchestratorConfig,
    provider: Arc<dyn LlmProvider>,
    tool_executor: ToolExecutor,
    cancellation_token: CancellationToken,
}

impl OrchestratorService {
    /// Create a new orchestrator service
    pub fn new(config: OrchestratorConfig) -> Self {
        let provider: Arc<dyn LlmProvider> = match config.provider.provider {
            ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config.provider.clone())),
            ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config.provider.clone())),
            ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(config.provider.clone())),
            ProviderType::Ollama => Arc::new(OllamaProvider::new(config.provider.clone())),
        };

        let tool_executor = ToolExecutor::new(&config.project_root);

        Self {
            config,
            provider,
            tool_executor,
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Get the cancellation token for external cancellation
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    /// Execute a user message through the agentic loop
    pub async fn execute(
        &self,
        message: String,
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        let tools = get_tool_definitions();
        let mut messages = vec![Message::user(message)];
        let mut total_usage = UsageStats::default();
        let mut iterations = 0;

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some("Execution cancelled".to_string()),
                };
            }

            // Check iteration limit
            if iterations >= self.config.max_iterations {
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some(format!(
                        "Maximum iterations ({}) reached",
                        self.config.max_iterations
                    )),
                };
            }

            // Check token budget
            if total_usage.total_tokens() >= self.config.max_total_tokens {
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some(format!(
                        "Token budget ({}) exceeded",
                        self.config.max_total_tokens
                    )),
                };
            }

            iterations += 1;

            // Call LLM
            let response = if self.config.streaming {
                self.call_llm_streaming(&messages, &tools, tx.clone()).await
            } else {
                self.call_llm(&messages, &tools).await
            };

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    // Emit error event
                    let _ = tx.send(UnifiedStreamEvent::Error {
                        message: e.to_string(),
                        code: None,
                    }).await;

                    return ExecutionResult {
                        response: None,
                        usage: total_usage,
                        iterations,
                        success: false,
                        error: Some(e.to_string()),
                    };
                }
            };

            // Update usage
            total_usage.input_tokens += response.usage.input_tokens;
            total_usage.output_tokens += response.usage.output_tokens;
            if let Some(thinking) = response.usage.thinking_tokens {
                total_usage.thinking_tokens = Some(
                    total_usage.thinking_tokens.unwrap_or(0) + thinking
                );
            }

            // Check if we have tool calls
            if response.has_tool_calls() {
                // Add assistant message with tool calls
                let mut content = Vec::new();
                if let Some(text) = &response.content {
                    content.push(MessageContent::Text { text: text.clone() });
                }
                for tc in &response.tool_calls {
                    content.push(MessageContent::ToolUse {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: tc.arguments.clone(),
                    });
                }
                messages.push(Message {
                    role: crate::services::llm::MessageRole::Assistant,
                    content,
                });

                // Execute each tool call
                for tc in &response.tool_calls {
                    // Emit tool start event
                    let _ = tx.send(UnifiedStreamEvent::ToolStart {
                        tool_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        arguments: Some(tc.arguments.to_string()),
                    }).await;

                    // Execute the tool
                    let result = self.tool_executor.execute(&tc.name, &tc.arguments).await;

                    // Emit tool result event
                    let _ = tx.send(UnifiedStreamEvent::ToolResult {
                        tool_id: tc.id.clone(),
                        result: if result.success { result.output.clone() } else { None },
                        error: if !result.success { result.error.clone() } else { None },
                    }).await;

                    // Add tool result to messages
                    messages.push(Message::tool_result(
                        &tc.id,
                        result.to_content(),
                        !result.success,
                    ));
                }
            } else {
                // No tool calls - this is the final response
                // Emit completion event
                let _ = tx.send(UnifiedStreamEvent::Complete {
                    stop_reason: Some("end_turn".to_string()),
                }).await;

                // Emit usage event
                let _ = tx.send(UnifiedStreamEvent::Usage {
                    input_tokens: total_usage.input_tokens,
                    output_tokens: total_usage.output_tokens,
                    thinking_tokens: total_usage.thinking_tokens,
                    cache_read_tokens: total_usage.cache_read_tokens,
                    cache_creation_tokens: total_usage.cache_creation_tokens,
                }).await;

                return ExecutionResult {
                    response: response.content,
                    usage: total_usage,
                    iterations,
                    success: true,
                    error: None,
                };
            }
        }
    }

    /// Call the LLM with non-streaming mode
    async fn call_llm(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse, crate::services::llm::LlmError> {
        self.provider
            .send_message(
                messages.to_vec(),
                self.config.system_prompt.clone(),
                tools.to_vec(),
            )
            .await
    }

    /// Call the LLM with streaming mode
    async fn call_llm_streaming(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> Result<LlmResponse, crate::services::llm::LlmError> {
        self.provider
            .stream_message(
                messages.to_vec(),
                self.config.system_prompt.clone(),
                tools.to_vec(),
                tx,
            )
            .await
    }

    /// Execute a simple message without the agentic loop (single turn)
    pub async fn execute_single(
        &self,
        message: String,
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        let messages = vec![Message::user(message)];

        let response = if self.config.streaming {
            self.call_llm_streaming(&messages, &[], tx.clone()).await
        } else {
            self.call_llm(&messages, &[]).await
        };

        match response {
            Ok(r) => {
                let _ = tx.send(UnifiedStreamEvent::Complete {
                    stop_reason: Some("end_turn".to_string()),
                }).await;

                ExecutionResult {
                    response: r.content,
                    usage: r.usage,
                    iterations: 1,
                    success: true,
                    error: None,
                }
            }
            Err(e) => {
                let _ = tx.send(UnifiedStreamEvent::Error {
                    message: e.to_string(),
                    code: None,
                }).await;

                ExecutionResult {
                    response: None,
                    usage: UsageStats::default(),
                    iterations: 1,
                    success: false,
                    error: Some(e.to_string()),
                }
            }
        }
    }

    /// Check if the provider is healthy
    pub async fn health_check(&self) -> Result<(), crate::services::llm::LlmError> {
        self.provider.health_check().await
    }

    /// Get the current configuration
    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }

    /// Get provider information
    pub fn provider_info(&self) -> ProviderInfo {
        ProviderInfo {
            name: self.provider.name().to_string(),
            model: self.provider.model().to_string(),
            supports_thinking: self.provider.supports_thinking(),
            supports_tools: self.provider.supports_tools(),
        }
    }
}

/// Information about the current provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub model: String,
    pub supports_thinking: bool,
    pub supports_tools: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> OrchestratorConfig {
        OrchestratorConfig {
            provider: ProviderConfig {
                provider: ProviderType::Anthropic,
                api_key: Some("test-key".to_string()),
                model: "claude-3-5-sonnet-20241022".to_string(),
                ..Default::default()
            },
            system_prompt: Some("You are a helpful assistant.".to_string()),
            max_iterations: 10,
            max_total_tokens: 10000,
            project_root: std::env::temp_dir(),
            streaming: true,
        }
    }

    #[test]
    fn test_orchestrator_creation() {
        let config = test_config();
        let orchestrator = OrchestratorService::new(config);

        let info = orchestrator.provider_info();
        assert_eq!(info.name, "anthropic");
        assert_eq!(info.model, "claude-3-5-sonnet-20241022");
        assert!(info.supports_tools);
    }

    #[test]
    fn test_execution_result() {
        let result = ExecutionResult {
            response: Some("Hello!".to_string()),
            usage: UsageStats {
                input_tokens: 100,
                output_tokens: 50,
                thinking_tokens: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
            },
            iterations: 1,
            success: true,
            error: None,
        };

        assert!(result.success);
        assert_eq!(result.response, Some("Hello!".to_string()));
    }

    #[test]
    fn test_cancellation_token() {
        let config = test_config();
        let orchestrator = OrchestratorService::new(config);

        let token = orchestrator.cancellation_token();
        assert!(!token.is_cancelled());

        token.cancel();
        assert!(token.is_cancelled());
    }
}
