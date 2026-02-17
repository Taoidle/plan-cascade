//! Ollama Provider
//!
//! Implementation of the LlmProvider trait for Ollama local inference
//! using the ollama-rs native SDK. Supports local model inference without
//! API keys, native tool calling (Unreliable reliability), and streaming.

use async_trait::async_trait;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::chat::{ChatMessage, ChatMessageResponse, MessageRole as OllamaRole};
use ollama_rs::generation::parameters::ThinkType;
use ollama_rs::generation::tools::{ToolCallFunction, ToolFunctionInfo, ToolInfo, ToolType};
use ollama_rs::models::ModelOptions;
use ollama_rs::Ollama;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

use super::provider::LlmProvider;
use super::types::{
    FallbackToolFormatMode, LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message,
    MessageContent, MessageRole, ProviderConfig, StopReason, ToolCall, ToolCallReliability,
    ToolDefinition, UsageStats,
};
use crate::services::proxy::{build_http_client, ProxyConfig};
use crate::services::streaming::UnifiedStreamEvent;

/// Default Ollama API endpoint
const OLLAMA_DEFAULT_URL: &str = "http://localhost:11434";

/// Models known to support thinking via <think> tags
const THINKING_MODELS: &[&str] = &["deepseek-r1", "qwq", "qwen-qwq"];

/// Ollama provider for local inference using the native ollama-rs SDK
pub struct OllamaProvider {
    config: ProviderConfig,
    client: Ollama,
}

impl OllamaProvider {
    /// Create a new Ollama provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(OLLAMA_DEFAULT_URL);

        let client = Self::create_client(base_url, config.proxy.as_ref());

        Self { config, client }
    }

    /// Create an Ollama SDK client from a base URL string.
    ///
    /// Parses the URL to extract host and port for `Ollama::new()`.
    /// Falls back to `Ollama::default()` if parsing fails.
    /// When a proxy config is provided, injects a custom reqwest client.
    fn create_client(base_url: &str, proxy: Option<&ProxyConfig>) -> Ollama {
        // Try to parse the URL to extract host and port
        if let Ok(parsed) = url::Url::parse(base_url) {
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("localhost");
            let port = parsed.port().unwrap_or(11434);
            // Reconstruct the host URL without port (Ollama::new takes them separately)
            let host_url = format!("{}://{}", scheme, host);
            if proxy.is_some() {
                let http_client = build_http_client(proxy);
                Ollama::new_with_client(host_url, port, http_client)
            } else {
                Ollama::new(host_url, port)
            }
        } else {
            Ollama::default()
        }
    }

    /// Get the base URL for the Ollama server (used in error messages)
    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or(OLLAMA_DEFAULT_URL)
    }

    /// Check if model supports thinking
    fn model_supports_thinking(&self) -> bool {
        let model_lower = self.config.model.to_lowercase();

        for known in THINKING_MODELS {
            if model_lower.contains(known) {
                return true;
            }
        }

        // Also check for r1/qwq patterns
        model_lower.contains("r1") || model_lower.contains("qwq")
    }

    /// Build a ChatMessageRequest from our unified types
    fn build_chat_request(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        request_options: &LlmRequestOptions,
    ) -> ChatMessageRequest {
        // Convert messages to ollama-rs ChatMessage format
        let mut chat_messages: Vec<ChatMessage> = Vec::new();

        // Add system message if provided
        if let Some(sys) = system {
            chat_messages.push(ChatMessage::system(sys.to_string()));
        }

        // Add conversation messages
        for msg in messages {
            chat_messages.extend(self.convert_message(msg));
        }

        // Create the base request
        let mut request = ChatMessageRequest::new(self.config.model.clone(), chat_messages);

        // Set model options
        let temperature = request_options
            .temperature_override
            .unwrap_or(self.config.temperature);
        let mut opts = ModelOptions::default().temperature(temperature);
        if self.config.max_tokens > 0 {
            opts = opts.num_predict(self.config.max_tokens as i32);
        }
        request = request.options(opts);

        // Enable thinking for models that support it
        if self.model_supports_thinking() && self.config.enable_thinking {
            request = request.think(ThinkType::True);
        }

        // Convert and set tools if provided
        if !tools.is_empty() {
            let ollama_tools: Vec<ToolInfo> = tools
                .iter()
                .filter_map(|t| self.convert_tool_definition(t))
                .collect();
            if !ollama_tools.is_empty() {
                request = request.tools(ollama_tools);
            }
        }

        request
    }

    /// Convert a unified Message to ollama-rs ChatMessage(s).
    ///
    /// A single unified Message may contain multiple content blocks (text, tool_use,
    /// tool_result, thinking), so this can return multiple ChatMessages.
    fn convert_message(&self, message: &Message) -> Vec<ChatMessage> {
        let mut result = Vec::new();

        // Collect text content from the message
        let mut text_parts: Vec<String> = Vec::new();

        for content in &message.content {
            match content {
                MessageContent::Text { text } => {
                    text_parts.push(text.clone());
                }
                MessageContent::ToolResult { content, .. } => {
                    // Tool results go as tool-role messages
                    result.push(ChatMessage::tool(content.clone()));
                }
                MessageContent::ToolUse {
                    id: _,
                    name,
                    input,
                } => {
                    // Tool use from assistant - create an assistant message with tool_calls
                    let mut msg = ChatMessage::assistant(String::new());
                    msg.tool_calls = vec![ollama_rs::generation::tools::ToolCall {
                        function: ToolCallFunction {
                            name: name.clone(),
                            arguments: input.clone(),
                        },
                    }];
                    result.push(msg);
                }
                MessageContent::Thinking { thinking, .. } => {
                    // Include thinking content in text for models that support it
                    text_parts.push(thinking.clone());
                }
                MessageContent::Image { .. }
                | MessageContent::ToolResultMultimodal { .. } => {
                    // Images/multimodal not fully supported by Ollama SDK in text mode
                    // Skip or convert to text placeholder
                }
            }
        }

        // If we have text content, push a single message with the combined text
        if !text_parts.is_empty() {
            let combined_text = text_parts.join("\n");
            let role = match message.role {
                MessageRole::User => OllamaRole::User,
                MessageRole::Assistant => OllamaRole::Assistant,
                MessageRole::System => OllamaRole::System,
            };
            result.insert(0, ChatMessage::new(role, combined_text));
        }

        // If no content was generated, push at least a minimal message
        if result.is_empty() {
            let role = match message.role {
                MessageRole::User => OllamaRole::User,
                MessageRole::Assistant => OllamaRole::Assistant,
                MessageRole::System => OllamaRole::System,
            };
            result.push(ChatMessage::new(role, String::new()));
        }

        result
    }

    /// Convert a unified ToolDefinition to ollama-rs ToolInfo.
    ///
    /// Uses schemars Schema to represent the parameter schema.
    fn convert_tool_definition(&self, tool: &ToolDefinition) -> Option<ToolInfo> {
        // Convert our ParameterSchema to a schemars Schema via JSON round-trip.
        // Our ParameterSchema is already a JSON Schema-like structure, so we serialize
        // it to JSON and then deserialize it as a schemars Schema.
        let schema_json = match serde_json::to_value(&tool.input_schema) {
            Ok(v) => v,
            Err(_) => return None,
        };

        let schema: schemars::Schema = match serde_json::from_value(schema_json) {
            Ok(s) => s,
            Err(_) => return None,
        };

        Some(ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: schema,
            },
        })
    }

    /// Convert an ollama-rs ChatMessageResponse to our unified LlmResponse (non-streaming).
    fn convert_response(&self, response: &ChatMessageResponse) -> LlmResponse {
        let msg = &response.message;

        // Extract thinking and text from content
        let (thinking, content) = self.extract_thinking(&msg.content);

        // Convert tool calls
        let tool_calls: Vec<ToolCall> = msg
            .tool_calls
            .iter()
            .enumerate()
            .map(|(i, tc)| ToolCall {
                id: format!("call_{}", i),
                name: tc.function.name.clone(),
                arguments: tc.function.arguments.clone(),
            })
            .collect();

        // Determine stop reason
        let stop_reason = if !tool_calls.is_empty() {
            StopReason::ToolUse
        } else {
            StopReason::EndTurn
        };

        // Extract usage from final_data
        let usage = if let Some(final_data) = &response.final_data {
            UsageStats {
                input_tokens: final_data.prompt_eval_count as u32,
                output_tokens: final_data.eval_count as u32,
                thinking_tokens: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
            }
        } else {
            UsageStats::default()
        };

        LlmResponse {
            content,
            thinking,
            tool_calls,
            stop_reason,
            usage,
            model: response.model.clone(),
        }
    }

    /// Process a single streaming ChatMessageResponse chunk and emit UnifiedStreamEvents.
    ///
    /// Returns any text or thinking content accumulated in this chunk for the final response.
    async fn process_stream_chunk(
        &self,
        response: &ChatMessageResponse,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
        in_thinking: &mut bool,
        thinking_started: &mut bool,
    ) -> (Option<String>, Option<String>, Vec<ToolCall>) {
        let msg = &response.message;
        let mut text_content = None;
        let mut thinking_content = None;
        let mut tool_calls = Vec::new();

        // Handle thinking field from SDK (if present)
        if let Some(ref thinking_text) = msg.thinking {
            if !thinking_text.is_empty() {
                if !*thinking_started {
                    *thinking_started = true;
                    *in_thinking = true;
                    let _ = tx
                        .send(UnifiedStreamEvent::ThinkingStart { thinking_id: None })
                        .await;
                }
                thinking_content = Some(thinking_text.clone());
                let _ = tx
                    .send(UnifiedStreamEvent::ThinkingDelta {
                        content: thinking_text.clone(),
                        thinking_id: None,
                    })
                    .await;
            }
        }

        // Handle text content
        if !msg.content.is_empty() {
            // If we were in thinking mode and now getting text, end thinking
            if *in_thinking {
                *in_thinking = false;
                let _ = tx
                    .send(UnifiedStreamEvent::ThinkingEnd { thinking_id: None })
                    .await;
            }

            // For models that use <think> tags in content (when SDK doesn't extract them),
            // we need to handle it at the content level
            if self.model_supports_thinking() && !*thinking_started {
                // Check for <think> tags inline
                let (think, text) = self.extract_thinking_streaming_chunk(
                    &msg.content,
                    in_thinking,
                    thinking_started,
                    tx,
                )
                .await;
                thinking_content = think;
                text_content = text;
            } else {
                text_content = Some(msg.content.clone());
                let _ = tx
                    .send(UnifiedStreamEvent::TextDelta {
                        content: msg.content.clone(),
                    })
                    .await;
            }
        }

        // Handle tool calls
        for (i, tc) in msg.tool_calls.iter().enumerate() {
            let tool_call = ToolCall {
                id: format!("call_{}", i),
                name: tc.function.name.clone(),
                arguments: tc.function.arguments.clone(),
            };
            tool_calls.push(tool_call);
        }

        // Handle completion
        if response.done {
            // End thinking if still in progress
            if *in_thinking {
                *in_thinking = false;
                let _ = tx
                    .send(UnifiedStreamEvent::ThinkingEnd { thinking_id: None })
                    .await;
            }

            // Emit usage if final_data available
            if let Some(final_data) = &response.final_data {
                let _ = tx
                    .send(UnifiedStreamEvent::Usage {
                        input_tokens: final_data.prompt_eval_count as u32,
                        output_tokens: final_data.eval_count as u32,
                        thinking_tokens: None,
                        cache_read_tokens: None,
                        cache_creation_tokens: None,
                    })
                    .await;
            }
        }

        (text_content, thinking_content, tool_calls)
    }

    /// Handle <think> tag extraction during streaming for models that embed
    /// thinking in the content field (rather than the SDK's thinking field).
    async fn extract_thinking_streaming_chunk(
        &self,
        content: &str,
        in_thinking: &mut bool,
        thinking_started: &mut bool,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) -> (Option<String>, Option<String>) {
        let mut thinking_result = None;
        let mut text_result = None;

        // Simple chunk-level processing:
        // The content may contain partial <think> or </think> tags.
        // Since we process chunk-by-chunk, we handle full tags here.
        let mut remaining = content.to_string();

        while !remaining.is_empty() {
            if *in_thinking {
                // Look for </think> end tag
                if let Some(end_pos) = remaining.find("</think>") {
                    let thinking_part = &remaining[..end_pos];
                    if !thinking_part.is_empty() {
                        thinking_result = Some(thinking_part.to_string());
                        let _ = tx
                            .send(UnifiedStreamEvent::ThinkingDelta {
                                content: thinking_part.to_string(),
                                thinking_id: None,
                            })
                            .await;
                    }
                    *in_thinking = false;
                    let _ = tx
                        .send(UnifiedStreamEvent::ThinkingEnd { thinking_id: None })
                        .await;
                    remaining = remaining[end_pos + 8..].to_string();
                } else {
                    // All of remaining is thinking content
                    thinking_result = Some(remaining.clone());
                    let _ = tx
                        .send(UnifiedStreamEvent::ThinkingDelta {
                            content: remaining.clone(),
                            thinking_id: None,
                        })
                        .await;
                    remaining.clear();
                }
            } else {
                // Look for <think> start tag
                if let Some(start_pos) = remaining.find("<think>") {
                    let text_part = &remaining[..start_pos];
                    if !text_part.is_empty() {
                        text_result = Some(text_part.to_string());
                        let _ = tx
                            .send(UnifiedStreamEvent::TextDelta {
                                content: text_part.to_string(),
                            })
                            .await;
                    }
                    *in_thinking = true;
                    if !*thinking_started {
                        *thinking_started = true;
                        let _ = tx
                            .send(UnifiedStreamEvent::ThinkingStart { thinking_id: None })
                            .await;
                    }
                    remaining = remaining[start_pos + 7..].to_string();
                } else {
                    // All of remaining is text content
                    text_result = Some(remaining.clone());
                    let _ = tx
                        .send(UnifiedStreamEvent::TextDelta {
                            content: remaining.clone(),
                        })
                        .await;
                    remaining.clear();
                }
            }
        }

        (thinking_result, text_result)
    }

    /// Extract thinking content from <think> tags (non-streaming version).
    ///
    /// Preserved from the original implementation for the non-streaming `send_message` path.
    fn extract_thinking(&self, content: &str) -> (Option<String>, Option<String>) {
        if !self.model_supports_thinking() {
            return (
                None,
                if content.is_empty() {
                    None
                } else {
                    Some(content.to_string())
                },
            );
        }

        let mut thinking = String::new();
        let mut text = String::new();
        let mut in_thinking = false;
        let mut buffer = String::new();

        for c in content.chars() {
            buffer.push(c);

            if buffer.ends_with("<think>") {
                let len = buffer.len() - 7;
                text.push_str(&buffer[..len]);
                buffer.clear();
                in_thinking = true;
            } else if buffer.ends_with("</think>") {
                let len = buffer.len() - 8;
                thinking.push_str(&buffer[..len]);
                buffer.clear();
                in_thinking = false;
            } else if buffer.len() > 10 {
                let flush_len = buffer.len() - 10;
                if in_thinking {
                    thinking.push_str(&buffer[..flush_len]);
                } else {
                    text.push_str(&buffer[..flush_len]);
                }
                buffer = buffer[flush_len..].to_string();
            }
        }

        // Flush remaining buffer
        if in_thinking {
            thinking.push_str(&buffer);
        } else {
            text.push_str(&buffer);
        }

        let thinking_result = if thinking.is_empty() {
            None
        } else {
            Some(thinking.trim().to_string())
        };
        let text_result = if text.is_empty() {
            None
        } else {
            Some(text.trim().to_string())
        };

        (thinking_result, text_result)
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    fn supports_thinking(&self) -> bool {
        self.model_supports_thinking()
    }

    fn supports_tools(&self) -> bool {
        // With ollama-rs SDK, native tool calling is available
        true
    }

    fn tool_call_reliability(&self) -> ToolCallReliability {
        // ADR-002: Upgrade from None to Unreliable - enables dual-channel tool calling
        ToolCallReliability::Unreliable
    }

    fn default_fallback_mode(&self) -> FallbackToolFormatMode {
        // Dual-channel: native tools + soft prompt fallback for reliability
        FallbackToolFormatMode::Soft
    }

    fn context_window(&self) -> u32 {
        // Ollama models vary widely; use a conservative default.
        // Users running larger models (e.g., Llama 3.1 70B) may need to
        // adjust max_total_tokens in the orchestrator config.
        8_192
    }

    async fn send_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        let request = self.build_chat_request(
            &messages,
            system.as_deref(),
            &tools,
            &request_options,
        );

        let response = self
            .client
            .send_chat_messages(request)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect") || msg.contains("Connection refused") {
                    LlmError::ProviderUnavailable {
                        message: format!(
                            "Cannot connect to Ollama at {}: {}",
                            self.base_url(),
                            msg
                        ),
                    }
                } else if msg.contains("not found") || msg.contains("404") {
                    LlmError::ModelNotFound {
                        model: self.config.model.clone(),
                    }
                } else {
                    LlmError::NetworkError { message: msg }
                }
            })?;

        Ok(self.convert_response(&response))
    }

    async fn stream_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        tx: mpsc::Sender<UnifiedStreamEvent>,
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        let request = self.build_chat_request(
            &messages,
            system.as_deref(),
            &tools,
            &request_options,
        );

        let mut stream = self
            .client
            .send_chat_messages_stream(request)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect") || msg.contains("Connection refused") {
                    LlmError::ProviderUnavailable {
                        message: format!(
                            "Cannot connect to Ollama at {}: {}",
                            self.base_url(),
                            msg
                        ),
                    }
                } else if msg.contains("not found") || msg.contains("404") {
                    LlmError::ModelNotFound {
                        model: self.config.model.clone(),
                    }
                } else {
                    LlmError::NetworkError { message: msg }
                }
            })?;

        let mut accumulated_content = String::new();
        let mut accumulated_thinking = String::new();
        let mut all_tool_calls: Vec<ToolCall> = Vec::new();
        let mut usage = UsageStats::default();
        let mut stop_reason = StopReason::EndTurn;
        let mut in_thinking = false;
        let mut thinking_started = false;
        let mut response_model = self.config.model.clone();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(response) => {
                    response_model = response.model.clone();

                    let (text, thinking, tool_calls) = self
                        .process_stream_chunk(
                            &response,
                            &tx,
                            &mut in_thinking,
                            &mut thinking_started,
                        )
                        .await;

                    if let Some(t) = text {
                        accumulated_content.push_str(&t);
                    }
                    if let Some(th) = thinking {
                        accumulated_thinking.push_str(&th);
                    }
                    if !tool_calls.is_empty() {
                        stop_reason = StopReason::ToolUse;
                        all_tool_calls.extend(tool_calls);
                    }

                    // Capture usage from final response
                    if response.done {
                        if let Some(final_data) = &response.final_data {
                            usage = UsageStats {
                                input_tokens: final_data.prompt_eval_count as u32,
                                output_tokens: final_data.eval_count as u32,
                                thinking_tokens: None,
                                cache_read_tokens: None,
                                cache_creation_tokens: None,
                            };
                        }
                    }
                }
                Err(_) => {
                    let _ = tx
                        .send(UnifiedStreamEvent::Error {
                            message: "Stream error from Ollama".to_string(),
                            code: None,
                        })
                        .await;
                    break;
                }
            }
        }

        Ok(LlmResponse {
            content: if accumulated_content.is_empty() {
                None
            } else {
                Some(accumulated_content)
            },
            thinking: if accumulated_thinking.is_empty() {
                None
            } else {
                Some(accumulated_thinking)
            },
            tool_calls: all_tool_calls,
            stop_reason,
            usage,
            model: response_model,
        })
    }

    async fn health_check(&self) -> LlmResult<()> {
        // Use the SDK's list_local_models as a health check
        self.client
            .list_local_models()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect") || msg.contains("Connection refused") {
                    LlmError::ProviderUnavailable {
                        message: format!("Cannot connect to Ollama at {}", self.base_url()),
                    }
                } else {
                    LlmError::NetworkError { message: msg }
                }
            })?;

        Ok(())
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn list_models(&self) -> LlmResult<Option<Vec<String>>> {
        let models = self
            .client
            .list_local_models()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect") || msg.contains("Connection refused") {
                    LlmError::ProviderUnavailable {
                        message: format!("Cannot connect to Ollama at {}", self.base_url()),
                    }
                } else {
                    LlmError::NetworkError { message: msg }
                }
            })?;

        let model_names: Vec<String> = models.into_iter().map(|m| m.name).collect();

        Ok(Some(model_names))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ProviderConfig {
        ProviderConfig {
            provider: super::super::types::ProviderType::Ollama,
            api_key: None, // Ollama doesn't need API key
            model: "llama3.2".to_string(),
            base_url: Some("http://localhost:11434".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_provider_creation() {
        let provider = OllamaProvider::new(test_config());
        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.model(), "llama3.2");
        assert!(!provider.supports_thinking());
        assert!(provider.supports_tools()); // Now true with SDK
        assert_eq!(
            provider.tool_call_reliability(),
            ToolCallReliability::Unreliable
        );
    }

    #[test]
    fn test_thinking_model() {
        let config = ProviderConfig {
            model: "deepseek-r1:14b".to_string(),
            ..test_config()
        };
        let provider = OllamaProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_qwq_model() {
        let config = ProviderConfig {
            model: "qwq:32b".to_string(),
            ..test_config()
        };
        let provider = OllamaProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_extract_thinking() {
        let config = ProviderConfig {
            model: "deepseek-r1".to_string(),
            ..test_config()
        };
        let provider = OllamaProvider::new(config);

        let (thinking, text) = provider.extract_thinking("<think>reasoning</think>answer");
        assert_eq!(thinking, Some("reasoning".to_string()));
        assert_eq!(text, Some("answer".to_string()));
    }

    #[test]
    fn test_base_url() {
        let provider = OllamaProvider::new(test_config());
        assert_eq!(provider.base_url(), "http://localhost:11434");

        let config = ProviderConfig {
            base_url: Some("http://192.168.1.100:11434".to_string()),
            ..test_config()
        };
        let provider = OllamaProvider::new(config);
        assert_eq!(provider.base_url(), "http://192.168.1.100:11434");
    }

    #[test]
    fn test_tool_call_reliability() {
        let provider = OllamaProvider::new(test_config());
        assert_eq!(
            provider.tool_call_reliability(),
            ToolCallReliability::Unreliable
        );
    }

    #[test]
    fn test_default_fallback_mode() {
        let provider = OllamaProvider::new(test_config());
        assert_eq!(
            provider.default_fallback_mode(),
            FallbackToolFormatMode::Soft
        );
    }

    #[test]
    fn test_convert_tool_definition() {
        let provider = OllamaProvider::new(test_config());
        let tool = ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: super::super::types::ParameterSchema {
                schema_type: "object".to_string(),
                description: Some("Read file params".to_string()),
                properties: Some(
                    [("path".to_string(), super::super::types::ParameterSchema::string(Some("File path")))]
                        .into_iter()
                        .collect(),
                ),
                required: Some(vec!["path".to_string()]),
                items: None,
                enum_values: None,
                default: None,
            },
        };

        let result = provider.convert_tool_definition(&tool);
        assert!(result.is_some());
        let tool_info = result.unwrap();
        assert_eq!(tool_info.function.name, "read_file");
        assert_eq!(tool_info.function.description, "Read a file");
    }

    #[test]
    fn test_convert_message_user() {
        let provider = OllamaProvider::new(test_config());
        let message = Message::user("Hello!");
        let converted = provider.convert_message(&message);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].content, "Hello!");
    }

    #[test]
    fn test_context_window() {
        let provider = OllamaProvider::new(test_config());
        assert_eq!(provider.context_window(), 8_192);
    }
}
