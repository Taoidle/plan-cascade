//! OpenAI Provider
//!
//! Implementation of the LlmProvider trait for OpenAI's API.
//! Supports GPT-4, o1, and o3 models with tool calling and reasoning.

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::provider::{missing_api_key_error, parse_http_error, LlmProvider};
use super::types::{
    LlmError, LlmResponse, LlmResult, Message, MessageContent, MessageRole,
    ProviderConfig, StopReason, ToolCall, ToolDefinition, UsageStats,
};
use crate::services::streaming::adapters::OpenAIAdapter;
use crate::services::streaming::{StreamAdapter, UnifiedStreamEvent};

/// Default OpenAI API endpoint
const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

/// OpenAI provider
pub struct OpenAIProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Get the API base URL
    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or(OPENAI_API_URL)
    }

    /// Check if model supports reasoning (o1/o3 models)
    fn model_supports_reasoning(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.starts_with("o1") || model.starts_with("o3")
    }

    /// Build the request body for the API
    fn build_request_body(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        stream: bool,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "stream": stream,
        });

        // Add temperature (not for o1/o3 models)
        if !self.model_supports_reasoning() {
            body["temperature"] = serde_json::json!(self.config.temperature);
        }

        // Add reasoning effort for o1/o3 models
        if self.model_supports_reasoning() {
            if let Some(effort) = &self.config.reasoning_effort {
                body["reasoning_effort"] = serde_json::json!(effort);
            }
        }

        // Convert messages to OpenAI format
        let mut openai_messages: Vec<serde_json::Value> = Vec::new();

        // Add system message if provided
        if let Some(sys) = system {
            openai_messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }

        // Add conversation messages
        for msg in messages {
            if msg.role == MessageRole::System {
                // Handle system messages from the conversation
                for content in &msg.content {
                    if let MessageContent::Text { text } = content {
                        openai_messages.push(serde_json::json!({
                            "role": "system",
                            "content": text
                        }));
                    }
                }
            } else {
                openai_messages.push(self.message_to_openai(msg));
            }
        }

        body["messages"] = serde_json::json!(openai_messages);

        // Add tools if provided
        if !tools.is_empty() {
            let openai_tools: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| self.tool_to_openai(t))
                .collect();
            body["tools"] = serde_json::json!(openai_tools);
        }

        // Add stream options for usage in streaming
        if stream {
            body["stream_options"] = serde_json::json!({
                "include_usage": true
            });
        }

        body
    }

    /// Convert a Message to OpenAI API format
    fn message_to_openai(&self, message: &Message) -> serde_json::Value {
        let role = match message.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        };

        // Check if message contains tool calls or tool results
        let has_tool_calls = message.content.iter().any(|c| matches!(c, MessageContent::ToolUse { .. }));
        let has_tool_results = message.content.iter().any(|c| matches!(c, MessageContent::ToolResult { .. }));

        if has_tool_results {
            // Tool results are sent as separate messages in OpenAI format
            let mut result_msg = serde_json::json!({
                "role": "tool"
            });

            for content in &message.content {
                if let MessageContent::ToolResult { tool_use_id, content, .. } = content {
                    result_msg["tool_call_id"] = serde_json::json!(tool_use_id);
                    result_msg["content"] = serde_json::json!(content);
                    break;
                }
            }

            return result_msg;
        }

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

            if !text_content.is_empty() {
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

    /// Convert a ToolDefinition to OpenAI API format
    fn tool_to_openai(&self, tool: &ToolDefinition) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.input_schema
            }
        })
    }

    /// Parse a response from OpenAI API
    fn parse_response(&self, response: &OpenAIResponse) -> LlmResponse {
        let choice = response.choices.first();

        let mut content = None;
        let mut thinking = None;
        let mut tool_calls = Vec::new();

        if let Some(choice) = choice {
            if let Some(msg) = &choice.message {
                content = msg.content.clone();
                thinking = msg.reasoning_content.clone();

                if let Some(tcs) = &msg.tool_calls {
                    for tc in tcs {
                        let arguments: serde_json::Value = serde_json::from_str(
                            &tc.function.arguments
                        ).unwrap_or(serde_json::Value::Null);

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

        let usage = response.usage.as_ref().map(|u| UsageStats {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            thinking_tokens: u.reasoning_tokens,
            cache_read_tokens: None,
            cache_creation_tokens: None,
        }).unwrap_or_default();

        LlmResponse {
            content,
            thinking,
            tool_calls,
            stop_reason,
            usage,
            model: response.model.clone(),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    fn supports_thinking(&self) -> bool {
        self.model_supports_reasoning()
    }

    fn supports_tools(&self) -> bool {
        true
    }

    async fn send_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
    ) -> LlmResult<LlmResponse> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| missing_api_key_error("openai"))?;

        let body = self.build_request_body(&messages, system.as_deref(), &tools, false);

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
            return Err(parse_http_error(status, &body_text, "openai"));
        }

        let openai_response: OpenAIResponse =
            serde_json::from_str(&body_text).map_err(|e| LlmError::ParseError {
                message: format!("Failed to parse response: {}", e),
            })?;

        Ok(self.parse_response(&openai_response))
    }

    async fn stream_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> LlmResult<LlmResponse> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| missing_api_key_error("openai"))?;

        let body = self.build_request_body(&messages, system.as_deref(), &tools, true);

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
            return Err(parse_http_error(status, &body_text, "openai"));
        }

        // Process SSE stream
        let mut adapter = OpenAIAdapter::new(&self.config.model);
        let mut accumulated_content = String::new();
        let mut accumulated_thinking = String::new();
        let mut tool_calls = Vec::new();
        let mut usage = UsageStats::default();
        let mut stop_reason = StopReason::EndTurn;

        // Track tool calls being accumulated
        let mut pending_tool_calls: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new();

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
                                UnifiedStreamEvent::ToolStart {
                                    tool_id,
                                    tool_name,
                                    ..
                                } => {
                                    pending_tool_calls.insert(
                                        tool_id.clone(),
                                        (tool_name.clone(), String::new()),
                                    );
                                }
                                UnifiedStreamEvent::Usage {
                                    input_tokens,
                                    output_tokens,
                                    thinking_tokens,
                                    ..
                                } => {
                                    usage.input_tokens = *input_tokens;
                                    usage.output_tokens = *output_tokens;
                                    usage.thinking_tokens = *thinking_tokens;
                                }
                                UnifiedStreamEvent::Complete {
                                    stop_reason: Some(reason),
                                } => {
                                    stop_reason = StopReason::from(reason.as_str());
                                }
                                _ => {}
                            }

                            // Forward event to channel
                            let _ = tx.send(event).await;
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

        // Finalize pending tool calls
        for (id, (name, args)) in pending_tool_calls {
            if let Ok(arguments) = serde_json::from_str(&args) {
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments,
                });
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
            .ok_or_else(|| missing_api_key_error("openai"))?;

        // List models to verify API key
        let response = self
            .client
            .get("https://api.openai.com/v1/models")
            .header("Authorization", format!("Bearer {}", api_key))
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
            Err(parse_http_error(status, &body, "openai"))
        }
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn list_models(&self) -> LlmResult<Option<Vec<String>>> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| missing_api_key_error("openai"))?;

        let response = self
            .client
            .get("https://api.openai.com/v1/models")
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(parse_http_error(status, &body, "openai"));
        }

        let body: serde_json::Value = response.json().await.map_err(|e| LlmError::ParseError {
            message: e.to_string(),
        })?;

        let models = body["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                    .filter(|id| id.starts_with("gpt") || id.starts_with("o1") || id.starts_with("o3"))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Some(models))
    }
}

/// OpenAI API response format
#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    model: String,
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
    reasoning_content: Option<String>,
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
    #[serde(default)]
    reasoning_tokens: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ProviderConfig {
        ProviderConfig {
            provider: super::super::types::ProviderType::OpenAI,
            api_key: Some("sk-test".to_string()),
            model: "gpt-4".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_provider_creation() {
        let provider = OpenAIProvider::new(test_config());
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.model(), "gpt-4");
        assert!(!provider.supports_thinking()); // GPT-4 doesn't support reasoning
        assert!(provider.supports_tools());
    }

    #[test]
    fn test_o1_supports_reasoning() {
        let config = ProviderConfig {
            model: "o1-preview".to_string(),
            ..test_config()
        };
        let provider = OpenAIProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_message_conversion() {
        let provider = OpenAIProvider::new(test_config());
        let message = Message::user("Hello!");

        let openai_msg = provider.message_to_openai(&message);
        assert_eq!(openai_msg["role"], "user");
        assert_eq!(openai_msg["content"], "Hello!");
    }

    #[test]
    fn test_tool_conversion() {
        let provider = OpenAIProvider::new(test_config());
        let tool = ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather".to_string(),
            input_schema: super::super::types::ParameterSchema::object(
                None,
                std::collections::HashMap::new(),
                vec![],
            ),
        };

        let openai_tool = provider.tool_to_openai(&tool);
        assert_eq!(openai_tool["type"], "function");
        assert_eq!(openai_tool["function"]["name"], "get_weather");
    }
}
