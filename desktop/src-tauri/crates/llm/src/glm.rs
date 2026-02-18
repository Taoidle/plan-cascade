//! GLM (ZhipuAI) Provider
//!
//! Implementation of the LlmProvider trait for ZhipuAI's GLM API.
//! Uses zai-rs SDK types for response parsing and tool definitions,
//! with reqwest HTTP transport for dynamic model names and custom retry logic.
//!
//! Retry strategy (ADR-005):
//!   1. First attempt: full parameters (tools + stream_options + tool_stream)
//!   2. If 1210 error: stripped body retry (remove tools/stream_options/tool_stream)
//!   3. If GLM-4.7 still fails: switch to coding endpoint retry

use async_trait::async_trait;
use tokio::sync::mpsc;
use zai_rs::model::chat_base_response::{ChatCompletionResponse as ZaiResponse, Usage as ZaiUsage};
use zai_rs::model::tools::{Function as ZaiFunction, Tools as ZaiTools};

use super::provider::{missing_api_key_error, parse_http_error, LlmProvider};
use super::types::{
    FallbackToolFormatMode, LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message,
    MessageContent, MessageRole, ProviderConfig, StopReason, ToolCall, ToolCallMode,
    ToolCallReliability, ToolDefinition, UsageStats,
};
use crate::http_client::build_http_client;
use crate::streaming_adapters::GlmAdapter;
use plan_cascade_core::streaming::{StreamAdapter, UnifiedStreamEvent};

/// Default GLM API endpoint
const GLM_API_URL: &str = "https://open.bigmodel.cn/api/paas/v4/chat/completions";
/// Coding plan endpoint (required for some GLM-4.7 key/plan combinations)
const GLM_CODING_API_URL: &str = "https://open.bigmodel.cn/api/coding/paas/v4/chat/completions";

/// GLM provider backed by zai-rs SDK types for serialization/deserialization.
///
/// Uses reqwest for HTTP transport to support dynamic model names at runtime,
/// since the zai-rs SDK's `ChatCompletion` type requires compile-time model
/// type parameters (e.g. `GLM4_5_flash`, `GLM4_7`).
pub struct GlmProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl GlmProvider {
    /// Create a new GLM provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let client = build_http_client(config.proxy.as_ref());
        Self { config, client }
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

    /// Convert our ToolDefinition to zai-rs Tools::Function.
    fn tool_to_zai(tool: &ToolDefinition) -> ZaiTools {
        let params = serde_json::to_value(&tool.input_schema).unwrap_or_default();
        ZaiTools::Function {
            function: ZaiFunction::new(&tool.name, &tool.description, params),
        }
    }

    /// Build the request body for the API using zai-rs types where possible.
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
            // Serialize tools using zai-rs Function type for type-safe tool definitions
            let api_tools: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    let zai_tool = Self::tool_to_zai(t);
                    serde_json::to_value(&zai_tool).unwrap_or_default()
                })
                .collect();
            body["tools"] = serde_json::json!(api_tools);
            let thinking_active =
                self.config.enable_thinking && self.model_supports_reasoning();
            if matches!(request_options.tool_call_mode, ToolCallMode::Required)
                && !thinking_active
            {
                // GLM thinking models may not support tool_choice "required" --
                // skip it and let the model default to "auto".
                body["tool_choice"] = serde_json::json!("required");
            }
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
        request_options: &LlmRequestOptions,
    ) -> serde_json::Value {
        let mut body = self.build_request_body(messages, system, &[], stream, request_options);
        if let Some(obj) = body.as_object_mut() {
            obj.remove("stream_options");
            obj.remove("tools");
            obj.remove("tool_stream");
            obj.remove("tool_choice");
        }
        body
    }

    /// Detect the GLM 1210 "invalid parameter" error from response status and body.
    ///
    /// The zai-rs SDK classifies this as `ZaiError::ApiError { code: 1210, .. }`.
    /// We check the raw response text for the same code pattern since we handle
    /// HTTP transport ourselves for dynamic model name support.
    fn is_invalid_param_error(status: u16, body_text: &str) -> bool {
        if status != 400 {
            return false;
        }
        body_text.contains("\"code\":\"1210\"")
            || body_text.contains("\"code\":1210")
            || body_text.contains("API \u{8c03}\u{7528}\u{53c2}\u{6570}\u{6709}\u{8bef}")
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
            // Always include content field -- some OpenAI-compatible APIs
            // require it even when the assistant only emits tool calls.
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

    /// Parse a non-streaming response using zai-rs ChatCompletionResponse type.
    fn parse_zai_response(&self, response: &ZaiResponse) -> LlmResponse {
        let choices = response.choices().unwrap_or(&[]);
        let choice = choices.first();
        let mut content = None;
        let mut thinking = None;
        let mut tool_calls = Vec::new();

        if let Some(choice) = choice {
            let msg = &choice.message;
            // Extract text content -- zai-rs stores content as Option<serde_json::Value>
            if let Some(c) = msg.content() {
                match c {
                    serde_json::Value::String(s) => {
                        if !s.is_empty() {
                            content = Some(s.clone());
                        }
                    }
                    serde_json::Value::Null => {}
                    other => {
                        // Handle non-string content by converting to string
                        let s = other.to_string();
                        if !s.is_empty() {
                            content = Some(s);
                        }
                    }
                }
            }
            thinking = msg.reasoning_content().map(|s| s.to_string());
            if let Some(tcs) = msg.tool_calls() {
                for tc in tcs {
                    if let Some(func) = tc.function() {
                        let name = func.name().unwrap_or("").to_string();
                        let id = tc.id().unwrap_or("").to_string();
                        let arguments: serde_json::Value = func
                            .arguments()
                            .and_then(|a| serde_json::from_str(a).ok())
                            .unwrap_or(serde_json::Value::Null);
                        tool_calls.push(ToolCall {
                            id,
                            name,
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
        let usage = Self::convert_zai_usage(response.usage());

        LlmResponse {
            content,
            thinking,
            tool_calls,
            stop_reason,
            usage,
            model: response.model().unwrap_or(&self.config.model).to_string(),
        }
    }

    /// Convert zai-rs Usage to our UsageStats.
    fn convert_zai_usage(usage: Option<&ZaiUsage>) -> UsageStats {
        usage
            .map(|u| UsageStats {
                input_tokens: u.prompt_tokens().unwrap_or(0),
                output_tokens: u.completion_tokens().unwrap_or(0),
                thinking_tokens: None, // zai-rs Usage does not expose reasoning_tokens
                cache_read_tokens: u
                    .prompt_tokens_details()
                    .and_then(|d| d.cached_tokens()),
                cache_creation_tokens: None,
            })
            .unwrap_or_default()
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

    fn tool_call_reliability(&self) -> ToolCallReliability {
        // GLM API supports native function calling, but in practice
        // models with thinking/reasoning may not emit native tool_calls reliably.
        // Keep Unreliable to use prompt-based fallback which works consistently.
        ToolCallReliability::Unreliable
    }

    fn default_fallback_mode(&self) -> FallbackToolFormatMode {
        if self.config.enable_thinking && self.model_supports_reasoning() {
            // GLM thinking models: disable prompt-based fallback to avoid
            // dual-channel confusion between native tools API and prompt
            // instructions.
            FallbackToolFormatMode::Off
        } else {
            FallbackToolFormatMode::Soft
        }
    }

    fn context_window(&self) -> u32 {
        let model = self.config.model.to_lowercase();
        // Vision models have smaller context windows -- check these first
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
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| missing_api_key_error("glm"))?;
        let body = self.build_request_body(
            &messages,
            system.as_deref(),
            &tools,
            false,
            &request_options,
        );

        // --- ADR-005 retry logic ---
        // Step 1: Full parameters
        let mut response = self
            .post_chat_completion(self.base_url(), api_key, &body)
            .await?;

        let mut status = response.status().as_u16();
        let mut body_text = response.text().await.map_err(|e| LlmError::NetworkError {
            message: e.to_string(),
        })?;
        if status != 200 {
            if Self::is_invalid_param_error(status, &body_text) {
                // Step 2: Stripped body retry (remove tools/stream_options/tool_stream)
                let compat_body = self.build_compat_request_body(
                    &messages,
                    system.as_deref(),
                    false,
                    &request_options,
                );
                response = self
                    .post_chat_completion(self.base_url(), api_key, &compat_body)
                    .await?;

                status = response.status().as_u16();
                body_text = response.text().await.map_err(|e| LlmError::NetworkError {
                    message: e.to_string(),
                })?;

                // Step 3: GLM-4.7 coding endpoint fallback
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

        // Parse response using zai-rs ChatCompletionResponse type
        let zai_response: ZaiResponse =
            serde_json::from_str(&body_text).map_err(|e| LlmError::ParseError {
                message: format!("Failed to parse response: {}", e),
            })?;
        Ok(self.parse_zai_response(&zai_response))
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
            .ok_or_else(|| missing_api_key_error("glm"))?;
        let body =
            self.build_request_body(&messages, system.as_deref(), &tools, true, &request_options);

        // --- ADR-005 retry logic ---
        // Step 1: Full parameters
        let mut response = self
            .post_chat_completion(self.base_url(), api_key, &body)
            .await?;

        let mut status = response.status().as_u16();
        if status != 200 {
            let body_text = response.text().await.map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;

            if Self::is_invalid_param_error(status, &body_text) {
                // Step 2: Stripped body retry
                let compat_body = self.build_compat_request_body(
                    &messages,
                    system.as_deref(),
                    true,
                    &request_options,
                );
                response = self
                    .post_chat_completion(self.base_url(), api_key, &compat_body)
                    .await?;

                status = response.status().as_u16();
                if status != 200 && self.model_is_glm47_family() && self.using_default_endpoint() {
                    let retry_text = response.text().await.map_err(|e| LlmError::NetworkError {
                        message: e.to_string(),
                    })?;

                    if Self::is_invalid_param_error(status, &retry_text) {
                        // Step 3: GLM-4.7 coding endpoint fallback
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
                            // internal signals -- the orchestrator emits its own
                            // Complete, Usage, and tool lifecycle events after
                            // executing tools.
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

    #[test]
    fn test_tool_to_zai_conversion() {
        use std::collections::HashMap;
        let tool = ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather for a city".to_string(),
            input_schema: super::super::types::ParameterSchema::object(
                Some("Weather parameters"),
                {
                    let mut props = HashMap::new();
                    props.insert(
                        "city".to_string(),
                        super::super::types::ParameterSchema::string(Some("City name")),
                    );
                    props
                },
                vec!["city".to_string()],
            ),
        };
        let zai_tool = GlmProvider::tool_to_zai(&tool);
        let serialized = serde_json::to_value(&zai_tool).unwrap();
        assert_eq!(serialized["type"], "function");
        assert_eq!(serialized["function"]["name"], "get_weather");
        assert_eq!(
            serialized["function"]["description"],
            "Get weather for a city"
        );
    }

    #[test]
    fn test_zai_response_parsing() {
        let provider = GlmProvider::new(test_config());
        let json_str = r#"{
            "id": "test-123",
            "model": "glm-4-flash-250414",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello! How can I help you?"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        }"#;
        let zai_resp: ZaiResponse = serde_json::from_str(json_str).unwrap();
        let response = provider.parse_zai_response(&zai_resp);
        assert_eq!(response.content, Some("Hello! How can I help you?".to_string()));
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert_eq!(response.usage.input_tokens, 10);
        assert_eq!(response.usage.output_tokens, 8);
        assert_eq!(response.model, "glm-4-flash-250414");
    }

    #[test]
    fn test_zai_response_with_tool_calls() {
        let provider = GlmProvider::new(test_config());
        let json_str = r#"{
            "id": "test-456",
            "model": "glm-4-flash-250414",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "tool_calls": [
                            {
                                "id": "call_001",
                                "type": "function",
                                "function": {
                                    "name": "get_weather",
                                    "arguments": "{\"city\":\"Beijing\"}"
                                }
                            }
                        ]
                    },
                    "finish_reason": "tool_calls"
                }
            ],
            "usage": {
                "prompt_tokens": 15,
                "completion_tokens": 12,
                "total_tokens": 27
            }
        }"#;
        let zai_resp: ZaiResponse = serde_json::from_str(json_str).unwrap();
        let response = provider.parse_zai_response(&zai_resp);
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id, "call_001");
        assert_eq!(response.tool_calls[0].name, "get_weather");
        assert_eq!(response.tool_calls[0].arguments["city"], "Beijing");
        assert_eq!(response.stop_reason, StopReason::ToolUse);
    }

    #[test]
    fn test_zai_response_with_reasoning() {
        let provider = GlmProvider::new(ProviderConfig {
            model: "glm-4.5-flash".to_string(),
            ..test_config()
        });
        let json_str = r#"{
            "id": "test-789",
            "model": "glm-4.5-flash",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "The answer is 42.",
                        "reasoning_content": "Let me think about this step by step..."
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 20,
                "completion_tokens": 30,
                "total_tokens": 50
            }
        }"#;
        let zai_resp: ZaiResponse = serde_json::from_str(json_str).unwrap();
        let response = provider.parse_zai_response(&zai_resp);
        assert_eq!(response.content, Some("The answer is 42.".to_string()));
        assert_eq!(
            response.thinking,
            Some("Let me think about this step by step...".to_string())
        );
    }

    #[test]
    fn test_is_invalid_param_error() {
        assert!(GlmProvider::is_invalid_param_error(
            400,
            r#"{"code":"1210","message":"API param error"}"#
        ));
        assert!(GlmProvider::is_invalid_param_error(
            400,
            r#"{"code":1210,"message":"error"}"#
        ));
        assert!(!GlmProvider::is_invalid_param_error(
            200,
            r#"{"code":"1210"}"#
        ));
        assert!(!GlmProvider::is_invalid_param_error(
            400,
            r#"{"code":"1234"}"#
        ));
    }

    #[test]
    fn test_glm47_family_detection() {
        let provider = GlmProvider::new(ProviderConfig {
            model: "glm-4.7".to_string(),
            ..test_config()
        });
        assert!(provider.model_is_glm47_family());

        let provider2 = GlmProvider::new(ProviderConfig {
            model: "glm-4.5-flash".to_string(),
            ..test_config()
        });
        assert!(!provider2.model_is_glm47_family());
    }

    #[test]
    fn test_tool_stream_support() {
        let provider46 = GlmProvider::new(ProviderConfig {
            model: "glm-4.6".to_string(),
            ..test_config()
        });
        assert!(provider46.model_supports_tool_stream());

        let provider47 = GlmProvider::new(ProviderConfig {
            model: "glm-4.7".to_string(),
            ..test_config()
        });
        assert!(provider47.model_supports_tool_stream());

        let provider45 = GlmProvider::new(ProviderConfig {
            model: "glm-4.5-flash".to_string(),
            ..test_config()
        });
        assert!(!provider45.model_supports_tool_stream());
    }
}
