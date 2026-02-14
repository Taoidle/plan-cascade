//! MiniMax Provider
//!
//! Implementation of the LlmProvider trait for MiniMax API.
//! Uses OpenAI-compatible format with reasoning_content for thinking models.

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::provider::{missing_api_key_error, parse_http_error, LlmProvider};
use super::types::{
    FallbackToolFormatMode, LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message,
    MessageContent, MessageRole, ProviderConfig, StopReason, ToolCall, ToolCallMode,
    ToolCallReliability, ToolDefinition, UsageStats,
};
use crate::services::streaming::adapters::MinimaxAdapter;
use crate::services::streaming::{StreamAdapter, UnifiedStreamEvent};

/// Default MiniMax OpenAI-compatible API endpoint
const MINIMAX_API_URL: &str = "https://api.minimax.io/v1/chat/completions";

/// MiniMax provider
pub struct MinimaxProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl MinimaxProvider {
    /// Create a new MiniMax provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Get the API base URL
    fn base_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(MINIMAX_API_URL)
    }

    /// Check if model supports reasoning (M2 series)
    fn model_supports_reasoning(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.contains("minimax-m2")
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
            "max_completion_tokens": self.config.max_tokens,
            "stream": stream,
            "temperature": request_options.temperature_override.unwrap_or(self.config.temperature),
        });

        // Enable thinking for M2 models if configured
        if self.config.enable_thinking && self.model_supports_reasoning() {
            body["reasoning_split"] = serde_json::json!(true);
        }

        // Convert messages to OpenAI-compatible format
        let mut api_messages: Vec<serde_json::Value> = Vec::new();

        if let Some(sys) = system {
            api_messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }

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
                api_messages.push(self.message_to_api(msg));
            }
        }

        body["messages"] = serde_json::json!(api_messages);

        if !tools.is_empty() {
            let api_tools: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(api_tools);

            let thinking_active =
                self.config.enable_thinking && self.model_supports_reasoning();
            if matches!(request_options.tool_call_mode, ToolCallMode::Required)
                && !thinking_active
            {
                body["tool_choice"] = serde_json::json!("required");
            }
        }

        if stream {
            body["stream_options"] = serde_json::json!({
                "include_usage": true
            });
        }

        body
    }

    /// Convert a Message to OpenAI-compatible format
    fn message_to_api(&self, message: &Message) -> serde_json::Value {
        let role = match message.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        };

        let has_tool_results = message
            .content
            .iter()
            .any(|c| matches!(c, MessageContent::ToolResult { .. }));
        if has_tool_results {
            let mut result_msg = serde_json::json!({"role": "tool"});
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

        let has_tool_calls = message
            .content
            .iter()
            .any(|c| matches!(c, MessageContent::ToolUse { .. }));
        if has_tool_calls {
            let tool_calls: Vec<serde_json::Value> = message.content.iter().filter_map(|c| {
                if let MessageContent::ToolUse { id, name, input } = c {
                    Some(serde_json::json!({"id": id, "type": "function", "function": {"name": name, "arguments": input.to_string()}}))
                } else { None }
            }).collect();

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

            let mut msg = serde_json::json!({"role": role, "tool_calls": tool_calls});
            if text_content.is_empty() {
                msg["content"] = serde_json::Value::Null;
            } else {
                msg["content"] = serde_json::json!(text_content);
            }
            return msg;
        }

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

        serde_json::json!({"role": role, "content": text_content})
    }

    /// Parse a non-streaming response
    fn parse_response(&self, response: &MinimaxResponse) -> LlmResponse {
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
                thinking_tokens: u.reasoning_tokens,
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
            model: response.model.clone(),
        }
    }
}

#[async_trait]
impl LlmProvider for MinimaxProvider {
    fn name(&self) -> &'static str {
        "minimax"
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

    fn tool_call_reliability(&self) -> ToolCallReliability {
        ToolCallReliability::Unreliable
    }

    fn default_fallback_mode(&self) -> FallbackToolFormatMode {
        if self.config.enable_thinking && self.model_supports_reasoning() {
            FallbackToolFormatMode::Off
        } else {
            FallbackToolFormatMode::Soft
        }
    }

    fn context_window(&self) -> u32 {
        let model = self.config.model.to_lowercase();
        if model.contains("m2.5") {
            // MiniMax-M2.5, MiniMax-M2.5-highspeed
            245_760
        } else if model.contains("m2.1") {
            // MiniMax-M2.1, MiniMax-M2.1-highspeed
            245_760
        } else if model.contains("m2") {
            // MiniMax-M2
            200_000
        } else if model.contains("text-01") {
            // MiniMax-Text-01
            4_000_000
        } else {
            // Conservative default for unrecognized MiniMax models
            200_000
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
            .ok_or_else(|| missing_api_key_error("minimax"))?;
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
            return Err(parse_http_error(status, &body_text, "minimax"));
        }

        let minimax_response: MinimaxResponse =
            serde_json::from_str(&body_text).map_err(|e| LlmError::ParseError {
                message: format!("Failed to parse response: {}", e),
            })?;
        Ok(self.parse_response(&minimax_response))
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
            .ok_or_else(|| missing_api_key_error("minimax"))?;
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
            return Err(parse_http_error(status, &body_text, "minimax"));
        }

        let mut adapter = MinimaxAdapter::new(&self.config.model);
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
            .ok_or_else(|| missing_api_key_error("minimax"))?;
        let body = serde_json::json!({"model": self.config.model, "max_completion_tokens": 1, "messages": [{"role": "user", "content": "Hi"}]});

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
            Err(parse_http_error(status, &body, "minimax"))
        }
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }
}

#[derive(Debug, Deserialize)]
struct MinimaxResponse {
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
            provider: super::super::types::ProviderType::Minimax,
            api_key: Some("sk-test".to_string()),
            model: "MiniMax-M2.5".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_provider_creation() {
        let provider = MinimaxProvider::new(test_config());
        assert_eq!(provider.name(), "minimax");
        assert_eq!(provider.model(), "MiniMax-M2.5");
        assert!(provider.supports_thinking());
        assert!(provider.supports_tools());
    }

    #[test]
    fn test_m2_supports_reasoning() {
        let config = ProviderConfig {
            model: "MiniMax-M2".to_string(),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_m2_1_supports_reasoning() {
        let config = ProviderConfig {
            model: "MiniMax-M2.1".to_string(),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_m2_5_highspeed_supports_reasoning() {
        let config = ProviderConfig {
            model: "MiniMax-M2.5-highspeed".to_string(),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_context_window_m2_5() {
        let provider = MinimaxProvider::new(test_config());
        assert_eq!(provider.context_window(), 245_760);
    }

    #[test]
    fn test_context_window_m2() {
        let config = ProviderConfig {
            model: "MiniMax-M2".to_string(),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(provider.context_window(), 200_000);
    }

    #[test]
    fn test_context_window_text_01() {
        let config = ProviderConfig {
            model: "MiniMax-Text-01".to_string(),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(provider.context_window(), 4_000_000);
    }

    #[test]
    fn test_tool_call_reliability() {
        let provider = MinimaxProvider::new(test_config());
        assert_eq!(
            provider.tool_call_reliability(),
            ToolCallReliability::Unreliable
        );
    }

    #[test]
    fn test_default_fallback_mode_thinking_off() {
        let config = ProviderConfig {
            enable_thinking: false,
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(provider.default_fallback_mode(), FallbackToolFormatMode::Soft);
    }

    #[test]
    fn test_default_fallback_mode_thinking_on() {
        let config = ProviderConfig {
            enable_thinking: true,
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(provider.default_fallback_mode(), FallbackToolFormatMode::Off);
    }

    #[test]
    fn test_message_conversion() {
        let provider = MinimaxProvider::new(test_config());
        let message = Message::user("Hello!");
        let api_msg = provider.message_to_api(&message);
        assert_eq!(api_msg["role"], "user");
        assert_eq!(api_msg["content"], "Hello!");
    }

    #[test]
    fn test_build_request_body_uses_max_completion_tokens() {
        let provider = MinimaxProvider::new(test_config());
        let body = provider.build_request_body(
            &[Message::user("test")],
            None,
            &[],
            false,
            &LlmRequestOptions::default(),
        );
        assert!(body.get("max_completion_tokens").is_some());
        assert!(body.get("max_tokens").is_none());
    }

    #[test]
    fn test_build_request_body_reasoning_split() {
        let config = ProviderConfig {
            enable_thinking: true,
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        let body = provider.build_request_body(
            &[Message::user("test")],
            None,
            &[],
            false,
            &LlmRequestOptions::default(),
        );
        assert_eq!(body["reasoning_split"], true);
    }

    #[test]
    fn test_build_request_body_no_reasoning_split_when_thinking_disabled() {
        let config = ProviderConfig {
            enable_thinking: false,
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        let body = provider.build_request_body(
            &[Message::user("test")],
            None,
            &[],
            false,
            &LlmRequestOptions::default(),
        );
        assert!(body.get("reasoning_split").is_none());
    }
}
