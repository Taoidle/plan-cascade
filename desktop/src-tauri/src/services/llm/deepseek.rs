//! DeepSeek Provider
//!
//! Implementation of the LlmProvider trait for DeepSeek's API.
//! Supports deepseek-chat and deepseek-r1 models with <think> tag handling.

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::provider::{missing_api_key_error, parse_http_error, LlmProvider};
use super::types::{
    LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message, MessageContent, MessageRole,
    ProviderConfig, StopReason, ToolCall, ToolCallMode, ToolCallReliability, ToolDefinition,
    UsageStats,
};
use crate::services::streaming::adapters::DeepSeekAdapter;
use crate::services::streaming::{StreamAdapter, UnifiedStreamEvent};

/// Default DeepSeek API endpoint
const DEEPSEEK_API_URL: &str = "https://api.deepseek.com/v1/chat/completions";

/// DeepSeek provider
pub struct DeepSeekProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl DeepSeekProvider {
    /// Create a new DeepSeek provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Get the API base URL
    fn base_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(DEEPSEEK_API_URL)
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

        // Convert messages to OpenAI-compatible format
        let mut api_messages: Vec<serde_json::Value> = Vec::new();

        // Add system message if provided
        if let Some(sys) = system {
            api_messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }

        // Add conversation messages
        for msg in messages {
            if msg.role == MessageRole::System {
                for content in &msg.content {
                    if let MessageContent::Text { text } = content {
                        api_messages.push(serde_json::json!({
                            "role": "system",
                            "content": text
                        }));
                    }
                }
            } else {
                api_messages.push(self.message_to_deepseek(msg));
            }
        }

        body["messages"] = serde_json::json!(api_messages);

        // Add tools if provided (DeepSeek uses OpenAI-compatible format)
        if !tools.is_empty() {
            let api_tools: Vec<serde_json::Value> =
                tools.iter().map(|t| self.tool_to_deepseek(t)).collect();
            body["tools"] = serde_json::json!(api_tools);
            if matches!(request_options.tool_call_mode, ToolCallMode::Required) {
                body["tool_choice"] = serde_json::json!("required");
            }
        }

        body
    }

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
        ToolCallReliability::Unreliable
    }

    fn context_window(&self) -> u32 {
        let model = self.config.model.to_lowercase();
        if model.contains("v2.5") {
            128_000 // DeepSeek-V2.5: 128k context
        } else {
            64_000 // DeepSeek-V3, DeepSeek-R1, deepseek-chat, deepseek-reasoner: 64k
        }
    }

    async fn send_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| missing_api_key_error("deepseek"))?;

        let body = self.build_request_body(
            &messages,
            system.as_deref(),
            &tools,
            false,
            &request_options,
        );

        let response = self
            .client
            .post(self.base_url())
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        let body_text = response.text().await.map_err(|e| LlmError::NetworkError {
            message: e.to_string(),
        })?;

        if status != 200 {
            return Err(parse_http_error(status, &body_text, "deepseek"));
        }

        let deepseek_response: DeepSeekResponse =
            serde_json::from_str(&body_text).map_err(|e| LlmError::ParseError {
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
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| missing_api_key_error("deepseek"))?;

        let body =
            self.build_request_body(&messages, system.as_deref(), &tools, true, &request_options);

        let response = self
            .client
            .post(self.base_url())
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body_text = response.text().await.map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;
            return Err(parse_http_error(status, &body_text, "deepseek"));
        }

        // Process SSE stream
        let mut adapter = DeepSeekAdapter::new(&self.config.model);
        let mut accumulated_content = String::new();
        let mut accumulated_thinking = String::new();
        let mut tool_calls = Vec::new();
        let mut usage = UsageStats::default();
        let mut stop_reason = StopReason::EndTurn;

        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete lines
            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.trim().is_empty() {
                    continue;
                }

                match adapter.adapt(&line) {
                    Ok(events) => {
                        for event in events {
                            match &event {
                                UnifiedStreamEvent::TextDelta { content } => {
                                    accumulated_content.push_str(content);
                                }
                                UnifiedStreamEvent::ThinkingDelta { content, .. } => {
                                    accumulated_thinking.push_str(content);
                                }
                                UnifiedStreamEvent::ToolComplete {
                                    tool_id,
                                    tool_name,
                                    arguments,
                                } => {
                                    if let Ok(input) = serde_json::from_str(arguments) {
                                        tool_calls.push(ToolCall {
                                            id: tool_id.clone(),
                                            name: tool_name.clone(),
                                            arguments: input,
                                        });
                                    }
                                }
                                UnifiedStreamEvent::Usage {
                                    input_tokens,
                                    output_tokens,
                                    ..
                                } => {
                                    usage.input_tokens = *input_tokens;
                                    usage.output_tokens = *output_tokens;
                                }
                                UnifiedStreamEvent::Complete {
                                    stop_reason: Some(reason),
                                } => {
                                    stop_reason = StopReason::from(reason.as_str());
                                }
                                _ => {}
                            }

                            // Forward streaming events but suppress internal signals —
                            // the orchestrator emits its own Complete, Usage, and
                            // tool lifecycle events after executing tools.
                            if !matches!(
                                &event,
                                UnifiedStreamEvent::Complete { .. }
                                    | UnifiedStreamEvent::Usage { .. }
                                    | UnifiedStreamEvent::ToolStart { .. }
                                    | UnifiedStreamEvent::ToolComplete { .. }
                            ) {
                                let _ = tx.send(event).await;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(UnifiedStreamEvent::Error {
                                message: e.to_string(),
                                code: None,
                            })
                            .await;
                    }
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
            tool_calls,
            stop_reason,
            usage,
            model: self.config.model.clone(),
        })
    }

    async fn health_check(&self) -> LlmResult<()> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| missing_api_key_error("deepseek"))?;

        // Make a minimal request to verify the API key
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "Hi"}]
        });

        let response = self
            .client
            .post(self.base_url())
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        if status == 200 {
            Ok(())
        } else if status == 401 {
            Err(LlmError::AuthenticationFailed {
                message: "Invalid API key".to_string(),
            })
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(parse_http_error(status, &body, "deepseek"))
        }
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
