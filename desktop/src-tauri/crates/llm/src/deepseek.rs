//! DeepSeek Provider
//!
//! Implementation of the LlmProvider trait for DeepSeek's API.
//! Supports deepseek-chat and deepseek-r1 models with <think> tag handling.

use async_trait::async_trait;
use futures_util::StreamExt;
use openai_api_rs::v1::chat_completion::chat_completion_stream::ChatCompletionStreamResponse;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::openai_compat::{
    build_client, build_openai_compatible_messages, map_api_error, value_to_chat_request,
    value_to_chat_stream_request,
};
use super::provider::LlmProvider;
use super::types::{
    FallbackToolFormatMode, LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message,
    ProviderConfig, StopReason, ToolCall, ToolCallMode, ToolCallReliability, ToolDefinition,
    UsageStats,
};
use crate::reliable_catalog::is_reliable_model;
use plan_cascade_core::streaming::UnifiedStreamEvent;

#[cfg(test)]
use super::types::{MessageContent, MessageRole};

/// Default DeepSeek API endpoint
const DEEPSEEK_API_URL: &str = "https://api.deepseek.com/v1/chat/completions";

/// DeepSeek provider
pub struct DeepSeekProvider {
    config: ProviderConfig,
}

impl DeepSeekProvider {
    /// Create a new DeepSeek provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        Self { config }
    }

    fn build_compat_client(&self) -> LlmResult<openai_api_rs::v1::api::OpenAIClient> {
        build_client(&self.config, "deepseek", DEEPSEEK_API_URL, false)
    }

    /// Check if model supports thinking (R1 models)
    fn model_supports_thinking(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.contains("r1") || model.contains("reasoner")
    }

    /// Build the request body for the API
    fn build_request_body(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        stream: bool,
        request_options: &LlmRequestOptions,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "stream": stream,
            "temperature": request_options.temperature_override.unwrap_or(self.config.temperature),
        });

        body["messages"] =
            serde_json::json!(build_openai_compatible_messages(messages, system));

        // Add tools if provided (DeepSeek uses OpenAI-compatible format)
        if !tools.is_empty() {
            let api_tools: Vec<serde_json::Value> =
                tools.iter().map(|t| self.tool_to_deepseek(t)).collect();
            body["tools"] = serde_json::json!(api_tools);
            let thinking_active = self.config.enable_thinking && self.model_supports_thinking();
            if matches!(request_options.tool_call_mode, ToolCallMode::Required) && !thinking_active
            {
                // DeepSeek R1 (thinking model) does not reliably support
                // tool_choice "required" — skip it and let the model default
                // to "auto".
                body["tool_choice"] = serde_json::json!("required");
            }
        }

        body
    }

    #[cfg(test)]
    /// Convert a Message to DeepSeek API format (OpenAI-compatible)
    fn message_to_deepseek(&self, message: &Message) -> serde_json::Value {
        let role = match message.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        };

        // Check for tool results
        let has_tool_results = message
            .content
            .iter()
            .any(|c| matches!(c, MessageContent::ToolResult { .. }));

        if has_tool_results {
            let mut result_msg = serde_json::json!({
                "role": "tool"
            });

            for content in &message.content {
                if let MessageContent::ToolResult {
                    tool_use_id,
                    content,
                    ..
                } = content
                {
                    result_msg["tool_call_id"] = serde_json::json!(tool_use_id);
                    result_msg["content"] = serde_json::json!(content);
                    break;
                }
            }

            return result_msg;
        }

        // Check for tool calls
        let has_tool_calls = message
            .content
            .iter()
            .any(|c| matches!(c, MessageContent::ToolUse { .. }));

        if has_tool_calls {
            let tool_calls: Vec<serde_json::Value> = message
                .content
                .iter()
                .filter_map(|c| {
                    if let MessageContent::ToolUse { id, name, input } = c {
                        Some(serde_json::json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": input.to_string()
                            }
                        }))
                    } else {
                        None
                    }
                })
                .collect();

            let text_content: String = message
                .content
                .iter()
                .filter_map(|c| {
                    if let MessageContent::Text { text } = c {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");

            let mut msg = serde_json::json!({
                "role": role,
                "tool_calls": tool_calls
            });

            // Always include content field — some OpenAI-compatible APIs
            // require it even when the assistant only emits tool calls.
            if text_content.is_empty() {
                msg["content"] = serde_json::Value::Null;
            } else {
                msg["content"] = serde_json::json!(text_content);
            }

            return msg;
        }

        // Simple text message
        let text_content: String = message
            .content
            .iter()
            .filter_map(|c| {
                if let MessageContent::Text { text } = c {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        serde_json::json!({
            "role": role,
            "content": text_content
        })
    }

    /// Convert a ToolDefinition to DeepSeek API format
    fn tool_to_deepseek(&self, tool: &ToolDefinition) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.input_schema
            }
        })
    }

    /// Parse a response from DeepSeek API
    fn parse_response(&self, response: &DeepSeekResponse) -> LlmResponse {
        let choice = response.choices.first();

        let mut content = None;
        let mut thinking = None;
        let mut tool_calls = Vec::new();

        if let Some(choice) = choice {
            if let Some(msg) = &choice.message {
                // Extract thinking from <think> tags if present
                if let Some(raw_content) = &msg.content {
                    let (think, text) = self.extract_thinking(raw_content);
                    thinking = think;
                    content = text;
                }

                if let Some(tcs) = &msg.tool_calls {
                    for tc in tcs {
                        let arguments: serde_json::Value =
                            serde_json::from_str(&tc.function.arguments)
                                .unwrap_or(serde_json::Value::Null);

                        tool_calls.push(ToolCall {
                            id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            arguments,
                        });
                    }
                }
            }
        }

        let stop_reason = choice
            .and_then(|c| c.finish_reason.as_ref())
            .map(|r| StopReason::from(r.as_str()))
            .unwrap_or(StopReason::EndTurn);

        let usage = response
            .usage
            .as_ref()
            .map(|u| UsageStats {
                input_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
                thinking_tokens: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
            })
            .unwrap_or_default();

        LlmResponse {
            content,
            thinking,
            tool_calls,
            stop_reason,
            usage,
            model: response
                .model
                .clone()
                .unwrap_or_else(|| self.config.model.clone()),
            search_citations: Vec::new(),
        }
    }

    /// Extract thinking content from <think> tags
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
        let mut chars = content.chars().peekable();
        let mut buffer = String::new();

        while let Some(c) = chars.next() {
            buffer.push(c);

            if buffer.ends_with("<think>") {
                // Remove the tag from buffer and switch to thinking mode
                let len = buffer.len() - 7;
                text.push_str(&buffer[..len]);
                buffer.clear();
                in_thinking = true;
            } else if buffer.ends_with("</think>") {
                // Remove the tag from buffer and switch to text mode
                let len = buffer.len() - 8;
                thinking.push_str(&buffer[..len]);
                buffer.clear();
                in_thinking = false;
            } else if buffer.len() > 10 {
                // Flush buffer if no tag is being formed
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
impl LlmProvider for DeepSeekProvider {
    fn name(&self) -> &'static str {
        "deepseek"
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    fn supports_thinking(&self) -> bool {
        self.model_supports_thinking()
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn tool_call_reliability(&self) -> ToolCallReliability {
        if is_reliable_model(self.config.provider, &self.config.model) {
            ToolCallReliability::Reliable
        } else {
            ToolCallReliability::Unreliable
        }
    }

    fn default_fallback_mode(&self) -> FallbackToolFormatMode {
        if matches!(self.tool_call_reliability(), ToolCallReliability::Reliable) {
            return FallbackToolFormatMode::Off;
        }
        if self.config.enable_thinking && self.model_supports_thinking() {
            // DeepSeek R1 with thinking enabled: disable prompt-based fallback
            // to avoid confusing the model with dual-channel tool calling
            // (native tools API + prompt instructions compete).
            FallbackToolFormatMode::Off
        } else {
            FallbackToolFormatMode::Soft
        }
    }

    fn context_window(&self) -> u32 {
        let model = self.config.model.to_lowercase();
        if model.contains("v2.5") {
            128_000 // DeepSeek-V2.5: 128k context
        } else {
            128_000 // DeepSeek-V3.2+/R1/deepseek-chat/deepseek-reasoner: 128k context
        }
    }

    async fn send_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        let body = self.build_request_body(
            &messages,
            system.as_deref(),
            &tools,
            false,
            &request_options,
        );
        let request = value_to_chat_request("deepseek", body)?;
        let mut client = self.build_compat_client()?;

        let response = client
            .chat_completion(request)
            .await
            .map_err(|e| map_api_error("deepseek", e))?;

        let deepseek_response: DeepSeekResponse =
            serde_json::from_value(serde_json::to_value(response).map_err(|e| {
                LlmError::ParseError {
                    message: format!("Failed to serialize response: {}", e),
                }
            })?)
            .map_err(|e| LlmError::ParseError {
                message: format!("Failed to parse response: {}", e),
            })?;

        Ok(self.parse_response(&deepseek_response))
    }

    async fn stream_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        tx: mpsc::Sender<UnifiedStreamEvent>,
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        let body =
            self.build_request_body(&messages, system.as_deref(), &tools, true, &request_options);
        let request = value_to_chat_stream_request("deepseek", body)?;
        let mut client = self.build_compat_client()?;

        let mut stream = client
            .chat_completion_stream(request)
            .await
            .map_err(|e| map_api_error("deepseek", e))?;

        let mut accumulated_content = String::new();
        let mut accumulated_thinking = String::new();
        let mut tool_calls = Vec::new();
        let usage = UsageStats::default();
        let mut stop_reason = StopReason::EndTurn;
        let mut in_thinking = false;

        let mut pending_tools: std::collections::HashMap<String, (Option<String>, String)> =
            std::collections::HashMap::new();
        let mut pending_order: Vec<String> = Vec::new();

        while let Some(event) = stream.next().await {
            match event {
                ChatCompletionStreamResponse::Content(content) => {
                    if !content.is_empty() {
                        accumulated_content.push_str(&content);
                        let _ = tx.send(UnifiedStreamEvent::TextDelta { content }).await;
                    }
                }
                ChatCompletionStreamResponse::ReasoningContent(content) => {
                    if !content.is_empty() {
                        if !in_thinking {
                            in_thinking = true;
                            let _ = tx
                                .send(UnifiedStreamEvent::ThinkingStart { thinking_id: None })
                                .await;
                        }
                        accumulated_thinking.push_str(&content);
                        let _ = tx
                            .send(UnifiedStreamEvent::ThinkingDelta {
                                content,
                                thinking_id: None,
                            })
                            .await;
                    }
                }
                ChatCompletionStreamResponse::ToolCall(chunks) => {
                    for chunk in chunks {
                        let id = chunk.id;
                        if id.is_empty() {
                            continue;
                        }
                        if !pending_tools.contains_key(&id) {
                            pending_order.push(id.clone());
                        }
                        let entry = pending_tools
                            .entry(id)
                            .or_insert_with(|| (None, String::new()));
                        if let Some(name) = chunk.function.name {
                            if !name.is_empty() {
                                entry.0 = Some(name);
                            }
                        }
                        if let Some(arguments) = chunk.function.arguments {
                            entry.1.push_str(&arguments);
                        }
                    }
                }
                ChatCompletionStreamResponse::Done => break,
            }
        }

        if in_thinking {
            let _ = tx
                .send(UnifiedStreamEvent::ThinkingEnd { thinking_id: None })
                .await;
        }

        for id in pending_order {
            if let Some((name, args)) = pending_tools.remove(&id) {
                if let Some(name) = name {
                    if let Ok(arguments) = serde_json::from_str::<serde_json::Value>(&args) {
                        tool_calls.push(ToolCall {
                            id,
                            name,
                            arguments,
                        });
                    }
                }
            }
        }

        if !tool_calls.is_empty() {
            stop_reason = StopReason::ToolUse;
        }

        let (thinking, final_content) = if !accumulated_thinking.is_empty() {
            (
                Some(accumulated_thinking),
                if accumulated_content.is_empty() {
                    None
                } else {
                    Some(accumulated_content)
                },
            )
        } else if self.model_supports_thinking() {
            if accumulated_content.is_empty() {
                (None, None)
            } else {
                self.extract_thinking(&accumulated_content)
            }
        } else {
            (
                None,
                if accumulated_content.is_empty() {
                    None
                } else {
                    Some(accumulated_content)
                },
            )
        };

        Ok(LlmResponse {
            content: final_content,
            thinking,
            tool_calls,
            stop_reason,
            usage,
            model: self.config.model.clone(),
            search_citations: Vec::new(),
        })
    }

    async fn health_check(&self) -> LlmResult<()> {
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "Hi"}],
            "stream": false,
        });
        let request = value_to_chat_request("deepseek", body)?;
        let mut client = self.build_compat_client()?;
        client
            .chat_completion(request)
            .await
            .map(|_| ())
            .map_err(|e| map_api_error("deepseek", e))
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }
}

/// DeepSeek API response format (OpenAI-compatible)
#[derive(Debug, Deserialize)]
struct DeepSeekResponse {
    model: Option<String>,
    choices: Vec<Choice>,
    usage: Option<ResponseUsage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<ResponseMessage>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ResponseToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ResponseToolCall {
    id: String,
    function: ResponseFunction,
}

#[derive(Debug, Deserialize)]
struct ResponseFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ResponseUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ProviderConfig {
        ProviderConfig {
            provider: super::super::types::ProviderType::DeepSeek,
            api_key: Some("sk-test".to_string()),
            model: "deepseek-chat".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_provider_creation() {
        let provider = DeepSeekProvider::new(test_config());
        assert_eq!(provider.name(), "deepseek");
        assert_eq!(provider.model(), "deepseek-chat");
        assert!(!provider.supports_thinking());
        assert!(provider.supports_tools());
    }

    #[test]
    fn test_r1_supports_thinking() {
        let config = ProviderConfig {
            model: "deepseek-r1".to_string(),
            ..test_config()
        };
        let provider = DeepSeekProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_extract_thinking() {
        let config = ProviderConfig {
            model: "deepseek-r1".to_string(),
            ..test_config()
        };
        let provider = DeepSeekProvider::new(config);

        let (thinking, text) =
            provider.extract_thinking("<think>I need to think</think>Here is the answer");
        assert_eq!(thinking, Some("I need to think".to_string()));
        assert_eq!(text, Some("Here is the answer".to_string()));

        let (thinking, text) = provider.extract_thinking("No thinking here");
        assert!(thinking.is_none());
        assert_eq!(text, Some("No thinking here".to_string()));
    }

    #[test]
    fn test_message_conversion() {
        let provider = DeepSeekProvider::new(test_config());
        let message = Message::user("Hello!");

        let api_msg = provider.message_to_deepseek(&message);
        assert_eq!(api_msg["role"], "user");
        assert_eq!(api_msg["content"], "Hello!");
    }
}
