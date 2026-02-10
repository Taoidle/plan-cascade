//! GLM (ZhipuAI) Provider
//!
//! Implementation of the LlmProvider trait for ZhipuAI's GLM API.
//! Uses OpenAI-compatible format with reasoning_content for thinking models.

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::provider::{missing_api_key_error, parse_http_error, LlmProvider};
use super::types::{
    LlmError, LlmResponse, LlmResult, Message, MessageContent, MessageRole, ProviderConfig,
    StopReason, ToolCall, ToolDefinition, UsageStats,
};
use crate::services::streaming::adapters::GlmAdapter;
use crate::services::streaming::{StreamAdapter, UnifiedStreamEvent};

/// Default GLM API endpoint
const GLM_API_URL: &str = "https://open.bigmodel.cn/api/paas/v4/chat/completions";
/// Coding plan endpoint (required for some GLM-4.7 key/plan combinations)
const GLM_CODING_API_URL: &str = "https://open.bigmodel.cn/api/coding/paas/v4/chat/completions";

/// GLM provider
pub struct GlmProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl GlmProvider {
    /// Create a new GLM provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Get the API base URL
    fn base_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(GLM_API_URL)
    }

    fn model_is_glm47_family(&self) -> bool {
        self.config.model.to_lowercase().starts_with("glm-4.7")
    }

    fn using_default_endpoint(&self) -> bool {
        self.config.base_url.is_none()
    }

    /// Check if model supports reasoning (GLM-4.5+, GLM-4.6, GLM-4.7 models)
    fn model_supports_reasoning(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.contains("4.5")
            || model.contains("4.6")
            || model.contains("4.7")
            || model.contains("thinking")
    }

    /// Stream tool call currently documented for GLM-4.6 / GLM-4.7.
    fn model_supports_tool_stream(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.contains("4.6") || model.contains("4.7")
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
            "temperature": self.config.temperature,
        });

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
        }

        if stream {
            body["stream_options"] = serde_json::json!({
                "include_usage": true
            });
            // GLM-4.6/4.7 tool streaming requires tool_stream=true in docs.
            if !tools.is_empty() && self.model_supports_tool_stream() {
                body["tool_stream"] = serde_json::json!(true);
            }
        }

        body
    }

    /// Some GLM accounts/models reject OpenAI-compatible optional fields (code 1210).
    /// Retry with a minimal request shape when that happens.
    fn build_compat_request_body(
        &self,
        messages: &[Message],
        system: Option<&str>,
        stream: bool,
    ) -> serde_json::Value {
        let mut body = self.build_request_body(messages, system, &[], stream);
        if let Some(obj) = body.as_object_mut() {
            obj.remove("stream_options");
            obj.remove("tools");
            obj.remove("tool_stream");
        }
        body
    }

    fn is_invalid_param_error(status: u16, body_text: &str) -> bool {
        if status != 400 {
            return false;
        }
        body_text.contains("\"code\":\"1210\"")
            || body_text.contains("\"code\":1210")
            || body_text.contains("API 调用参数有误")
    }

    async fn post_chat_completion(
        &self,
        url: &str,
        api_key: &str,
        body: &serde_json::Value,
    ) -> LlmResult<reqwest::Response> {
        self.client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })
    }

    fn invalid_param_with_endpoint_hint(&self, body_text: &str) -> LlmError {
        if self.model_is_glm47_family() {
            LlmError::InvalidRequest {
                message: format!(
                    "{} (GLM-4.7 may require Coding endpoint for your key/plan: {})",
                    body_text, GLM_CODING_API_URL
                ),
            }
        } else {
            LlmError::InvalidRequest {
                message: body_text.to_string(),
            }
        }
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
    fn parse_response(&self, response: &GlmResponse) -> LlmResponse {
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
impl LlmProvider for GlmProvider {
    fn name(&self) -> &'static str {
        "glm"
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
        // Vision models have smaller context windows — check these first
        // since "4.6v" contains "4.6" and "4.5v" contains "4.5".
        if model.contains("4v") || model.contains("4.5v") {
            // glm-4v-plus: 8k, glm-4v-flash: 8k, glm-4.5v: 16k
            if model.contains("4.5v") {
                16_384
            } else {
                8_192
            }
        } else if model.contains("4.6v") || model.contains("4.1v") {
            // glm-4.6v, glm-4.6v-flash, glm-4.6v-flashx: 32k context
            // glm-4.1v-thinking-flash/flashx: 32k context
            32_768
        } else if model.contains("4.7") || model.contains("4.6") {
            // glm-4.7, glm-4.6: 128k context, max output 131,072
            128_000
        } else if model.contains("4.5") {
            // glm-4.5, glm-4.5-air, glm-4.5-x, glm-4.5-flash: 128k context, max output 98,304
            128_000
        } else {
            // glm-4-plus, glm-4-air, glm-4-airx, glm-4-flash, glm-4-flashx: 128k context
            128_000
        }
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
            .ok_or_else(|| missing_api_key_error("glm"))?;
        let body = self.build_request_body(&messages, system.as_deref(), &tools, false);

        let mut response = self
            .post_chat_completion(self.base_url(), api_key, &body)
            .await?;

        let mut status = response.status().as_u16();
        let mut body_text = response.text().await.map_err(|e| LlmError::NetworkError {
            message: e.to_string(),
        })?;
        if status != 200 {
            if Self::is_invalid_param_error(status, &body_text) {
                let compat_body =
                    self.build_compat_request_body(&messages, system.as_deref(), false);
                response = self
                    .post_chat_completion(self.base_url(), api_key, &compat_body)
                    .await?;

                status = response.status().as_u16();
                body_text = response.text().await.map_err(|e| LlmError::NetworkError {
                    message: e.to_string(),
                })?;

                // Some keys/plans only allow GLM-4.7 on coding endpoint.
                if status != 200
                    && Self::is_invalid_param_error(status, &body_text)
                    && self.model_is_glm47_family()
                    && self.using_default_endpoint()
                {
                    response = self
                        .post_chat_completion(GLM_CODING_API_URL, api_key, &compat_body)
                        .await?;

                    status = response.status().as_u16();
                    body_text = response.text().await.map_err(|e| LlmError::NetworkError {
                        message: e.to_string(),
                    })?;
                }

                if status != 200 {
                    if Self::is_invalid_param_error(status, &body_text) {
                        return Err(self.invalid_param_with_endpoint_hint(&body_text));
                    }
                    return Err(parse_http_error(status, &body_text, "glm"));
                }
            } else {
                return Err(parse_http_error(status, &body_text, "glm"));
            }
        }

        let glm_response: GlmResponse =
            serde_json::from_str(&body_text).map_err(|e| LlmError::ParseError {
                message: format!("Failed to parse response: {}", e),
            })?;
        Ok(self.parse_response(&glm_response))
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
            .ok_or_else(|| missing_api_key_error("glm"))?;
        let body = self.build_request_body(&messages, system.as_deref(), &tools, true);

        let mut response = self
            .post_chat_completion(self.base_url(), api_key, &body)
            .await?;

        let mut status = response.status().as_u16();
        if status != 200 {
            let body_text = response.text().await.map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

            if Self::is_invalid_param_error(status, &body_text) {
                let compat_body =
                    self.build_compat_request_body(&messages, system.as_deref(), true);
                response = self
                    .post_chat_completion(self.base_url(), api_key, &compat_body)
                    .await?;

                status = response.status().as_u16();
                if status != 200 && self.model_is_glm47_family() && self.using_default_endpoint() {
                    let retry_text = response.text().await.map_err(|e| LlmError::NetworkError {
                        message: e.to_string(),
                    })?;

                    if Self::is_invalid_param_error(status, &retry_text) {
                        response = self
                            .post_chat_completion(GLM_CODING_API_URL, api_key, &compat_body)
                            .await?;
                        status = response.status().as_u16();
                    } else {
                        return Err(parse_http_error(status, &retry_text, "glm"));
                    }
                }

                if status != 200 {
                    let retry_text = response.text().await.map_err(|e| LlmError::NetworkError {
                        message: e.to_string(),
                    })?;
                    if Self::is_invalid_param_error(status, &retry_text) {
                        return Err(self.invalid_param_with_endpoint_hint(&retry_text));
                    }
                    return Err(parse_http_error(status, &retry_text, "glm"));
                }
            } else {
                return Err(parse_http_error(status, &body_text, "glm"));
            }
        }

        let mut adapter = GlmAdapter::new(&self.config.model);
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
                            // Forward streaming events to the frontend, but suppress
                            // Complete and Usage — those are internal to the LLM call
                            // and don't mean execution is done (tool calls may follow).
                            // The orchestrator emits its own Complete when truly finished.
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
            .ok_or_else(|| missing_api_key_error("glm"))?;
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
            Err(parse_http_error(status, &body, "glm"))
        }
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }
}

#[derive(Debug, Deserialize)]
struct GlmResponse {
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
            provider: super::super::types::ProviderType::Glm,
            api_key: Some("test-key".to_string()),
            model: "glm-4-flash-250414".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_provider_creation() {
        let provider = GlmProvider::new(test_config());
        assert_eq!(provider.name(), "glm");
        assert_eq!(provider.model(), "glm-4-flash-250414");
        assert!(!provider.supports_thinking());
        assert!(provider.supports_tools());
    }

    #[test]
    fn test_glm45_supports_reasoning() {
        let config = ProviderConfig {
            model: "glm-4.5-flash".to_string(),
            ..test_config()
        };
        let provider = GlmProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_glm47_supports_reasoning() {
        let config = ProviderConfig {
            model: "glm-4.7".to_string(),
            ..test_config()
        };
        let provider = GlmProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_message_conversion() {
        let provider = GlmProvider::new(test_config());
        let message = Message::user("Hello!");
        let api_msg = provider.message_to_api(&message);
        assert_eq!(api_msg["role"], "user");
        assert_eq!(api_msg["content"], "Hello!");
    }
}
