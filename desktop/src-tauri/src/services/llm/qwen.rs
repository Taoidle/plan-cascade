//! Qwen (Alibaba Cloud DashScope) Provider
//!
//! Implementation of the LlmProvider trait for DashScope's Qwen API.
//! Uses OpenAI-compatible format with reasoning_content for thinking models.

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::provider::{missing_api_key_error, parse_http_error, LlmProvider};
use super::types::{
    LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message, MessageContent, MessageRole,
    ProviderConfig, StopReason, ToolCall, ToolCallMode, ToolDefinition, UsageStats,
};
use crate::services::streaming::adapters::QwenAdapter;
use crate::services::streaming::{StreamAdapter, UnifiedStreamEvent};

/// Default DashScope OpenAI-compatible API endpoint
const QWEN_API_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

/// Qwen provider
pub struct QwenProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl QwenProvider {
    /// Create a new Qwen provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Get the API base URL
    fn base_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(QWEN_API_URL)
    }

    /// Check if model supports reasoning (Qwen3 series, QwQ models)
    fn model_supports_reasoning(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.contains("qwen3") || model.contains("qwq") || model.contains("thinking")
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

        // Enable thinking for Qwen3 models if configured
        if self.config.enable_thinking && self.model_supports_reasoning() {
            body["enable_thinking"] = serde_json::json!(true);
            if let Some(budget) = self.config.thinking_budget {
                body["thinking_budget"] = serde_json::json!(budget);
            }
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
            if matches!(request_options.tool_call_mode, ToolCallMode::Required) {
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
            if !text_content.is_empty() {
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
    fn parse_response(&self, response: &QwenResponse) -> LlmResponse {
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
impl LlmProvider for QwenProvider {
    fn name(&self) -> &'static str {
        "qwen"
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

    fn context_window(&self) -> u32 {
        let model = self.config.model.to_lowercase();
        // Qwen model context windows vary significantly by model variant.
        // Source: https://help.aliyun.com/zh/model-studio/what-is-qwen-llm
        if model.contains("qwen3-max") {
            // qwen3-max, qwen3-max-2025-09-23, qwen3-max-2026-01-23: 262k
            262_144
        } else if model.contains("qwen-max-latest")
            || model.contains("qwen-max-2025")
            || model.contains("qwen-max-2026")
        {
            // qwen-max-latest, qwen-max-2025-01-25: 131k
            131_072
        } else if model.contains("qwen-max") {
            // qwen-max (stable, older): 32k
            32_768
        } else if model.contains("qwen-plus")
            || model.contains("qwen-turbo")
            || model.contains("qwen-flash")
            || model.contains("qwen-long")
        {
            // qwen-plus, qwen-turbo, qwen-flash, qwen-long: 1M context
            1_000_000
        } else if model.contains("qwen3-coder") {
            // qwen3-coder-plus, qwen3-coder-flash: 1M context
            1_000_000
        } else if model.contains("qwq") {
            // QwQ-Plus, QwQ series: 131k
            131_072
        } else if model.contains("qwen2.5-turbo") {
            // qwen2.5-turbo: 1M context
            1_000_000
        } else if model.contains("qwen2.5") {
            // qwen2.5 open-source models: 128k for 7B+, 32k for smaller
            131_072
        } else if model.contains("qwen3") {
            // qwen3 open-source (8B+: 128k, 0.6B-4B: 32k)
            // Most users on DashScope API use the larger models
            128_000
        } else {
            // Conservative default for unrecognized Qwen models
            32_768
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
            .ok_or_else(|| missing_api_key_error("qwen"))?;
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
            return Err(parse_http_error(status, &body_text, "qwen"));
        }

        let qwen_response: QwenResponse =
            serde_json::from_str(&body_text).map_err(|e| LlmError::ParseError {
                message: format!("Failed to parse response: {}", e),
            })?;
        Ok(self.parse_response(&qwen_response))
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
            .ok_or_else(|| missing_api_key_error("qwen"))?;
        let body = self.build_request_body(
            &messages,
            system.as_deref(),
            &tools,
            true,
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
        if status != 200 {
            let body_text = response.text().await.map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;
            return Err(parse_http_error(status, &body_text, "qwen"));
        }

        let mut adapter = QwenAdapter::new(&self.config.model);
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
                            // Forward streaming events but suppress Complete/Usage —
                            // those are internal signals; the orchestrator emits its own
                            // Complete after tool calls are done.
                            if !matches!(
                                &event,
                                UnifiedStreamEvent::Complete { .. }
                                    | UnifiedStreamEvent::Usage { .. }
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
            .ok_or_else(|| missing_api_key_error("qwen"))?;
        let body = serde_json::json!({"model": self.config.model, "max_tokens": 1, "messages": [{"role": "user", "content": "Hi"}]});

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
            Err(parse_http_error(status, &body, "qwen"))
        }
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }
}

#[derive(Debug, Deserialize)]
struct QwenResponse {
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
            provider: super::super::types::ProviderType::Qwen,
            api_key: Some("sk-test".to_string()),
            model: "qwen-plus".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_provider_creation() {
        let provider = QwenProvider::new(test_config());
        assert_eq!(provider.name(), "qwen");
        assert_eq!(provider.model(), "qwen-plus");
        assert!(!provider.supports_thinking());
        assert!(provider.supports_tools());
    }

    #[test]
    fn test_qwen3_supports_reasoning() {
        let config = ProviderConfig {
            model: "qwen3-plus".to_string(),
            ..test_config()
        };
        let provider = QwenProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_qwq_supports_reasoning() {
        let config = ProviderConfig {
            model: "qwq-plus".to_string(),
            ..test_config()
        };
        let provider = QwenProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_message_conversion() {
        let provider = QwenProvider::new(test_config());
        let message = Message::user("Hello!");
        let api_msg = provider.message_to_api(&message);
        assert_eq!(api_msg["role"], "user");
        assert_eq!(api_msg["content"], "Hello!");
    }
}
