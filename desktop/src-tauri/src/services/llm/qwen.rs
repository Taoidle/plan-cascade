//! Qwen (Alibaba Cloud DashScope) Provider
//!
//! Implementation of the LlmProvider trait for DashScope's Qwen API.
//! Uses the async-dashscope SDK for API communication.
//!
//! ## SDK Limitations
//! The async-dashscope SDK (v0.12) does not expose `temperature`, `max_tokens`,
//! or `tool_choice` parameters in its `Parameters` builder. These are omitted
//! from SDK-based requests, meaning DashScope will use its server-side defaults.
//! If precise control over temperature/max_tokens is required, consider using
//! the OpenAI-compatible endpoint with raw reqwest instead. See findings.md.

use async_dashscope::operation::common::{
    FunctionBuilder, FunctionCallBuilder, ParametersBuilder,
};
use async_dashscope::operation::generation::{GenerationOutput, GenerationParamBuilder};
use async_dashscope::Client as DashScopeClient;
use async_trait::async_trait;
use futures_util::StreamExt;
use std::collections::HashMap;
use tokio::sync::mpsc;

use super::provider::{missing_api_key_error, LlmProvider};
use super::types::{
    FallbackToolFormatMode, LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message,
    MessageContent, MessageRole, ProviderConfig, StopReason, ToolCall, ToolCallReliability,
    ToolDefinition, UsageStats,
};
use crate::services::streaming::UnifiedStreamEvent;

/// Qwen provider using the async-dashscope SDK
pub struct QwenProvider {
    config: ProviderConfig,
    client: DashScopeClient,
}

impl QwenProvider {
    /// Create a new Qwen provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let client = Self::build_client(&config);
        Self { config, client }
    }

    /// Build a DashScope SDK client from provider configuration
    fn build_client(config: &ProviderConfig) -> DashScopeClient {
        let api_key = config.api_key.as_deref().unwrap_or("");

        // If a custom base_url is configured, use ConfigBuilder for full control.
        if let Some(base_url) = &config.base_url {
            if let Ok(sdk_config) = async_dashscope::config::ConfigBuilder::default()
                .api_base(base_url.as_str())
                .api_key(api_key)
                .build()
            {
                return DashScopeClient::with_config(sdk_config);
            }
        }

        DashScopeClient::new().with_api_key(api_key.to_string())
    }

    /// Check if model supports reasoning (Qwen3 series, QwQ models)
    fn model_supports_reasoning(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.contains("qwen3") || model.contains("qwq") || model.contains("thinking")
    }

    /// Build the SDK Input from unified messages.
    ///
    /// The SDK's param-level Message enum and ToolCall struct are not publicly
    /// exported. To construct assistant messages with tool_calls (and the Input
    /// type itself), we build a JSON representation and deserialize it via serde.
    /// This works because both Input and the param::Message enum implement
    /// Deserialize.
    fn build_sdk_input_json(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> serde_json::Value {
        let mut api_messages: Vec<serde_json::Value> = Vec::new();

        // Add system prompt if provided
        if let Some(sys) = system {
            api_messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    for content in &msg.content {
                        if let MessageContent::Text { text } = content {
                            api_messages.push(serde_json::json!({
                                "role": "system",
                                "content": text
                            }));
                        }
                    }
                }
                MessageRole::User => {
                    let has_tool_results = msg
                        .content
                        .iter()
                        .any(|c| matches!(c, MessageContent::ToolResult { .. }));

                    if has_tool_results {
                        for content in &msg.content {
                            if let MessageContent::ToolResult {
                                tool_use_id,
                                content,
                                ..
                            } = content
                            {
                                api_messages.push(serde_json::json!({
                                    "role": "tool",
                                    "content": content,
                                    "tool_call_id": tool_use_id
                                }));
                                break;
                            }
                        }
                    } else {
                        let text_content = Self::extract_text_content(&msg.content);
                        api_messages.push(serde_json::json!({
                            "role": "user",
                            "content": text_content
                        }));
                    }
                }
                MessageRole::Assistant => {
                    let text_content = Self::extract_text_content(&msg.content);

                    let tool_calls_json: Vec<serde_json::Value> = msg
                        .content
                        .iter()
                        .filter_map(|c| {
                            if let MessageContent::ToolUse { id, name, input } = c {
                                Some(serde_json::json!({
                                    "id": id,
                                    "type": "function",
                                    "index": 0,
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

                    let mut msg_json = serde_json::json!({
                        "role": "assistant",
                        "content": text_content
                    });

                    if !tool_calls_json.is_empty() {
                        msg_json["tool_calls"] = serde_json::json!(tool_calls_json);
                    }

                    api_messages.push(msg_json);
                }
            }
        }

        serde_json::json!({ "messages": api_messages })
    }

    /// Extract text content from message content blocks
    fn extract_text_content(content: &[MessageContent]) -> String {
        content
            .iter()
            .filter_map(|c| {
                if let MessageContent::Text { text } = c {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Convert unified tool definitions to SDK FunctionCall format
    fn convert_tools(
        tools: &[ToolDefinition],
    ) -> Vec<async_dashscope::operation::common::FunctionCall> {
        tools
            .iter()
            .filter_map(|t| {
                let parameters = Self::convert_parameter_schema(&t.input_schema);

                let function = FunctionBuilder::default()
                    .name(t.name.as_str())
                    .description(t.description.as_str())
                    .parameters(parameters)
                    .build()
                    .ok()?;

                FunctionCallBuilder::default()
                    .typ("function")
                    .function(function)
                    .build()
                    .ok()
            })
            .collect()
    }

    /// Convert a ParameterSchema to SDK FunctionParameters via serde.
    fn convert_parameter_schema(
        schema: &super::types::ParameterSchema,
    ) -> async_dashscope::operation::common::FunctionParameters {
        let properties: HashMap<String, serde_json::Value> = schema
            .properties
            .as_ref()
            .map(|props| {
                props
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            serde_json::to_value(v).unwrap_or(serde_json::Value::Null),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        let json = serde_json::json!({
            "type": schema.schema_type,
            "properties": properties,
            "required": schema.required,
        });

        serde_json::from_value(json).unwrap_or_else(|_| {
            serde_json::from_value(serde_json::json!({
                "type": "object",
                "properties": {},
            }))
            .expect("fallback FunctionParameters should always deserialize")
        })
    }

    /// Build SDK parameters for a request
    fn build_sdk_parameters(
        &self,
        tools: &[ToolDefinition],
    ) -> Option<async_dashscope::operation::common::Parameters> {
        let mut builder = ParametersBuilder::default();
        builder.result_format("message");

        if self.config.enable_thinking && self.model_supports_reasoning() {
            builder.enable_thinking(true);
            if let Some(budget) = self.config.thinking_budget {
                builder.thinking_budget(budget as usize);
            }
        }

        if !tools.is_empty() {
            let sdk_tools = Self::convert_tools(tools);
            builder.tools(sdk_tools);
            builder.parallel_tool_calls(true);
        }

        builder.build().ok()
    }

    /// Build a GenerationParam from messages, tools, and streaming flag.
    ///
    /// Uses serde to construct the Input (since the SDK doesn't publicly export
    /// the param::Message enum or param::ToolCall struct), then uses the SDK
    /// builders for GenerationParam and Parameters.
    fn build_generation_param(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        stream: bool,
    ) -> LlmResult<async_dashscope::operation::generation::GenerationParam> {
        // Build Input via serde (handles assistant messages with tool_calls)
        let input_json = self.build_sdk_input_json(messages, system);
        let input = serde_json::from_value(input_json).map_err(|e| LlmError::InvalidRequest {
            message: format!("Failed to build SDK input: {}", e),
        })?;

        let parameters = self.build_sdk_parameters(tools);

        let mut param_builder = GenerationParamBuilder::default();
        param_builder
            .model(self.config.model.clone())
            .input(input)
            .stream(stream);

        if stream {
            param_builder.stream_options(async_dashscope::operation::common::StreamOptions {
                include_usage: true,
            });
        }

        if let Some(params) = parameters {
            param_builder.parameters(params);
        }

        param_builder.build().map_err(|e| LlmError::InvalidRequest {
            message: format!("Failed to build generation params: {}", e),
        })
    }

    /// Convert SDK GenerationOutput to unified LlmResponse
    fn convert_output_to_response(&self, output: &GenerationOutput) -> LlmResponse {
        let mut content = None;
        let mut thinking = None;
        let mut tool_calls = Vec::new();
        let mut stop_reason = StopReason::EndTurn;

        if let Some(choices) = &output.output.choices {
            if let Some(choice) = choices.first() {
                let msg = &choice.message;

                if !msg.content.is_empty() {
                    content = Some(msg.content.clone());
                }

                thinking = msg.reasoning_content.clone();

                if let Some(tcs) = &msg.tool_calls {
                    for tc in tcs {
                        let arguments: serde_json::Value = tc
                            .function
                            .arguments
                            .as_ref()
                            .and_then(|a| serde_json::from_str(a).ok())
                            .unwrap_or(serde_json::Value::Null);
                        tool_calls.push(ToolCall {
                            id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            arguments,
                        });
                    }
                }

                if let Some(reason) = &choice.finish_reason {
                    stop_reason = StopReason::from(reason.as_str());
                }
            }
        }

        if let Some(reason) = &output.output.finish_reason {
            stop_reason = StopReason::from(reason.as_str());
        }

        let usage = output
            .usage
            .as_ref()
            .map(|u| UsageStats {
                input_tokens: u.input_tokens.unwrap_or(0) as u32,
                output_tokens: u.output_tokens.unwrap_or(0) as u32,
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
            model: self.config.model.clone(),
        }
    }

    /// Convert a DashScope SDK error to unified LlmError
    fn convert_sdk_error(err: async_dashscope::error::DashScopeError) -> LlmError {
        let msg = err.to_string();
        let msg_lower = msg.to_lowercase();
        if msg.contains("401") || msg_lower.contains("unauthorized") || msg_lower.contains("invalid api") {
            LlmError::AuthenticationFailed {
                message: format!("qwen: {}", msg),
            }
        } else if msg.contains("429") || msg_lower.contains("rate limit") {
            LlmError::RateLimited {
                message: msg,
                retry_after: None,
            }
        } else if msg.contains("404") || msg_lower.contains("not found") {
            LlmError::ModelNotFound { model: msg }
        } else if msg.contains("400") || msg_lower.contains("invalid") {
            LlmError::InvalidRequest { message: msg }
        } else if msg.contains("500") || msg.contains("502") || msg.contains("503") {
            LlmError::ServerError {
                message: msg,
                status: None,
            }
        } else {
            LlmError::NetworkError { message: msg }
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
        if model.contains("qwen3-max") {
            262_144
        } else if model.contains("qwen-max-latest")
            || model.contains("qwen-max-2025")
            || model.contains("qwen-max-2026")
        {
            131_072
        } else if model.contains("qwen-max") {
            32_768
        } else if model.contains("qwen-plus")
            || model.contains("qwen-turbo")
            || model.contains("qwen-flash")
            || model.contains("qwen-long")
        {
            1_000_000
        } else if model.contains("qwen3-coder") {
            1_000_000
        } else if model.contains("qwq") {
            131_072
        } else if model.contains("qwen2.5-turbo") {
            1_000_000
        } else if model.contains("qwen2.5") {
            131_072
        } else if model.contains("qwen3") {
            128_000
        } else {
            32_768
        }
    }

    async fn send_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        _request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        if self.config.api_key.is_none() || self.config.api_key.as_deref() == Some("") {
            return Err(missing_api_key_error("qwen"));
        }

        let param = self.build_generation_param(&messages, system.as_deref(), &tools, false)?;

        let output = self
            .client
            .generation()
            .call(param)
            .await
            .map_err(Self::convert_sdk_error)?;

        Ok(self.convert_output_to_response(&output))
    }

    async fn stream_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        tx: mpsc::Sender<UnifiedStreamEvent>,
        _request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        if self.config.api_key.is_none() || self.config.api_key.as_deref() == Some("") {
            return Err(missing_api_key_error("qwen"));
        }

        let param = self.build_generation_param(&messages, system.as_deref(), &tools, true)?;

        let mut stream = self
            .client
            .generation()
            .call_stream(param)
            .await
            .map_err(Self::convert_sdk_error)?;

        let mut accumulated_content = String::new();
        let mut accumulated_thinking = String::new();
        let mut tool_calls = Vec::new();
        let mut usage = UsageStats::default();
        let mut stop_reason = StopReason::EndTurn;
        let mut in_reasoning = false;

        let mut pending_tool_id: Option<String> = None;
        let mut pending_tool_name: Option<String> = None;
        let mut pending_tool_args = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(output) => {
                    if let Some(u) = &output.usage {
                        usage.input_tokens = u.input_tokens.unwrap_or(0) as u32;
                        usage.output_tokens = u.output_tokens.unwrap_or(0) as u32;
                    }

                    if let Some(choices) = &output.output.choices {
                        for choice in choices {
                            if let Some(reason) = &choice.finish_reason {
                                if let (Some(id), Some(name)) =
                                    (pending_tool_id.take(), pending_tool_name.take())
                                {
                                    let args = std::mem::take(&mut pending_tool_args);
                                    if let Ok(parsed) = serde_json::from_str(&args) {
                                        tool_calls.push(ToolCall {
                                            id,
                                            name,
                                            arguments: parsed,
                                        });
                                    }
                                }

                                if in_reasoning {
                                    in_reasoning = false;
                                    let _ = tx
                                        .send(UnifiedStreamEvent::ThinkingEnd {
                                            thinking_id: None,
                                        })
                                        .await;
                                }

                                stop_reason = StopReason::from(reason.as_str());
                                continue;
                            }

                            let msg = &choice.message;

                            if let Some(reasoning) = &msg.reasoning_content {
                                if !reasoning.is_empty() {
                                    if !in_reasoning {
                                        in_reasoning = true;
                                        let _ = tx
                                            .send(UnifiedStreamEvent::ThinkingStart {
                                                thinking_id: None,
                                            })
                                            .await;
                                    }
                                    accumulated_thinking.push_str(reasoning);
                                    let _ = tx
                                        .send(UnifiedStreamEvent::ThinkingDelta {
                                            content: reasoning.clone(),
                                            thinking_id: None,
                                        })
                                        .await;
                                }
                            }

                            if !msg.content.is_empty() {
                                if in_reasoning {
                                    in_reasoning = false;
                                    let _ = tx
                                        .send(UnifiedStreamEvent::ThinkingEnd {
                                            thinking_id: None,
                                        })
                                        .await;
                                }
                                accumulated_content.push_str(&msg.content);
                                let _ = tx
                                    .send(UnifiedStreamEvent::TextDelta {
                                        content: msg.content.clone(),
                                    })
                                    .await;
                            }

                            if let Some(tcs) = &msg.tool_calls {
                                for tc in tcs {
                                    let is_new_tool = !tc.id.is_empty()
                                        && pending_tool_id.as_deref() != Some(&tc.id);

                                    if is_new_tool {
                                        if let (Some(id), Some(name)) =
                                            (pending_tool_id.take(), pending_tool_name.take())
                                        {
                                            let args = std::mem::take(&mut pending_tool_args);
                                            if let Ok(parsed) = serde_json::from_str(&args) {
                                                tool_calls.push(ToolCall {
                                                    id,
                                                    name,
                                                    arguments: parsed,
                                                });
                                            }
                                        }

                                        pending_tool_id = Some(tc.id.clone());
                                        if !tc.function.name.is_empty() {
                                            pending_tool_name =
                                                Some(tc.function.name.clone());
                                        }
                                        pending_tool_args.clear();
                                    }

                                    if pending_tool_name.is_none()
                                        && !tc.function.name.is_empty()
                                    {
                                        pending_tool_name = Some(tc.function.name.clone());
                                    }

                                    if let Some(args) = &tc.function.arguments {
                                        pending_tool_args.push_str(args);
                                    }
                                }
                            }
                        }
                    }

                    if let Some(reason) = &output.output.finish_reason {
                        stop_reason = StopReason::from(reason.as_str());
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

        if let (Some(id), Some(name)) = (pending_tool_id.take(), pending_tool_name.take()) {
            let args = std::mem::take(&mut pending_tool_args);
            if let Ok(parsed) = serde_json::from_str(&args) {
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments: parsed,
                });
            }
        }

        if in_reasoning {
            let _ = tx
                .send(UnifiedStreamEvent::ThinkingEnd {
                    thinking_id: None,
                })
                .await;
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
        if self.config.api_key.is_none() || self.config.api_key.as_deref() == Some("") {
            return Err(missing_api_key_error("qwen"));
        }

        let param = self.build_generation_param(&[Message::user("Hi")], None, &[], false)?;

        self.client
            .generation()
            .call(param)
            .await
            .map_err(Self::convert_sdk_error)?;

        Ok(())
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
    fn test_context_window_matrix() {
        let config = ProviderConfig {
            model: "qwen3-max".to_string(),
            ..test_config()
        };
        assert_eq!(QwenProvider::new(config).context_window(), 262_144);

        let config = ProviderConfig {
            model: "qwen-plus".to_string(),
            ..test_config()
        };
        assert_eq!(QwenProvider::new(config).context_window(), 1_000_000);

        let config = ProviderConfig {
            model: "qwq-plus".to_string(),
            ..test_config()
        };
        assert_eq!(QwenProvider::new(config).context_window(), 131_072);

        let config = ProviderConfig {
            model: "unknown-model".to_string(),
            ..test_config()
        };
        assert_eq!(QwenProvider::new(config).context_window(), 32_768);
    }

    #[test]
    fn test_tool_call_reliability() {
        let provider = QwenProvider::new(test_config());
        assert_eq!(
            provider.tool_call_reliability(),
            ToolCallReliability::Unreliable
        );
    }

    #[test]
    fn test_default_fallback_mode_thinking() {
        let config = ProviderConfig {
            model: "qwen3-plus".to_string(),
            enable_thinking: true,
            ..test_config()
        };
        let provider = QwenProvider::new(config);
        assert_eq!(provider.default_fallback_mode(), FallbackToolFormatMode::Off);
    }

    #[test]
    fn test_default_fallback_mode_non_thinking() {
        let provider = QwenProvider::new(test_config());
        assert_eq!(
            provider.default_fallback_mode(),
            FallbackToolFormatMode::Soft
        );
    }

    #[test]
    fn test_input_json_user() {
        let provider = QwenProvider::new(test_config());
        let messages = vec![Message::user("Hello!")];
        let json = provider.build_sdk_input_json(&messages, None);
        let msgs = json["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "Hello!");
    }

    #[test]
    fn test_input_json_with_system() {
        let provider = QwenProvider::new(test_config());
        let messages = vec![Message::user("Hello!")];
        let json = provider.build_sdk_input_json(&messages, Some("You are a helper."));
        let msgs = json["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "You are a helper.");
        assert_eq!(msgs[1]["role"], "user");
    }

    #[test]
    fn test_input_json_tool_result() {
        let provider = QwenProvider::new(test_config());
        let messages = vec![Message::tool_result("call_123", "file contents here", false)];
        let json = provider.build_sdk_input_json(&messages, None);
        let msgs = json["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "tool");
        assert_eq!(msgs[0]["tool_call_id"], "call_123");
        assert_eq!(msgs[0]["content"], "file contents here");
    }

    #[test]
    fn test_input_json_assistant_with_tool_calls() {
        let provider = QwenProvider::new(test_config());
        let messages = vec![Message {
            role: MessageRole::Assistant,
            content: vec![
                MessageContent::Text {
                    text: "Let me read that file.".to_string(),
                },
                MessageContent::ToolUse {
                    id: "call_abc".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "/tmp/test.rs"}),
                },
            ],
        }];
        let json = provider.build_sdk_input_json(&messages, None);
        let msgs = json["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "assistant");
        let tcs = msgs[0]["tool_calls"].as_array().unwrap();
        assert_eq!(tcs.len(), 1);
        assert_eq!(tcs[0]["id"], "call_abc");
        assert_eq!(tcs[0]["function"]["name"], "read_file");
    }

    #[test]
    fn test_tool_conversion() {
        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: super::super::types::ParameterSchema::object(
                Some("Parameters"),
                {
                    let mut props = std::collections::HashMap::new();
                    props.insert(
                        "path".to_string(),
                        super::super::types::ParameterSchema::string(Some("File path")),
                    );
                    props
                },
                vec!["path".to_string()],
            ),
        }];
        let sdk_tools = QwenProvider::convert_tools(&tools);
        assert_eq!(sdk_tools.len(), 1);
    }

    #[test]
    fn test_build_generation_param() {
        let provider = QwenProvider::new(test_config());
        let messages = vec![Message::user("Test")];
        let result = provider.build_generation_param(&messages, None, &[], false);
        assert!(result.is_ok());
    }
}
