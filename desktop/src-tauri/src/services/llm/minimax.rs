//! MiniMax Provider
//!
//! Implementation of the LlmProvider trait for MiniMax API.
//! Uses the async-anthropic SDK via MiniMax's Anthropic-compatible endpoint
//! (api.minimax.io/anthropic/v1).
//!
//! ADR-003: SDK migration from raw reqwest/OpenAI-compat to async-anthropic.
//! ADR-004: Extended thinking temporarily degraded - async-anthropic v0.6
//!          doesn't expose extended thinking fields. MiniMax M2's reasoning_split
//!          cannot be passed through the SDK initially.

use async_anthropic::types::{
    ContentBlockDelta, CreateMessagesRequestBuilder, CreateMessagesResponse,
    MessageBuilder, MessageContent as AnthropicMessageContent,
    MessageContentList, MessageRole as AnthropicMessageRole,
    MessagesStreamEvent, Text as AnthropicText,
    ToolChoice as AnthropicToolChoice, ToolResult as AnthropicToolResult,
    ToolUse as AnthropicToolUse,
};
use async_anthropic::Client as AnthropicClient;
use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::sync::mpsc;

use super::provider::{missing_api_key_error, LlmProvider};
use super::types::{
    FallbackToolFormatMode, LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message,
    MessageContent, MessageRole, ProviderConfig, StopReason, ToolCall, ToolCallMode,
    ToolCallReliability, ToolDefinition, UsageStats,
};
use crate::services::streaming::UnifiedStreamEvent;

/// Default MiniMax Anthropic-compatible API base URL
const MINIMAX_ANTHROPIC_BASE_URL: &str = "https://api.minimax.io/anthropic/v1";

/// MiniMax provider using async-anthropic SDK
pub struct MinimaxProvider {
    config: ProviderConfig,
    client: AnthropicClient,
}

impl MinimaxProvider {
    /// Create a new MiniMax provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let api_key = config.api_key.clone().unwrap_or_default();
        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(MINIMAX_ANTHROPIC_BASE_URL);

        let client = AnthropicClient::builder()
            .api_key(api_key)
            .base_url(base_url)
            .build()
            .expect("Failed to build MiniMax Anthropic client: invalid configuration");

        Self { config, client }
    }

    /// Check if model supports reasoning (M2 series)
    fn model_supports_reasoning(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.contains("minimax-m2")
    }

    /// Build a CreateMessagesRequest from unified types
    fn build_request(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        request_options: &LlmRequestOptions,
    ) -> Result<async_anthropic::types::CreateMessagesRequest, LlmError> {
        // Convert messages to Anthropic SDK format
        let sdk_messages = self.convert_messages(messages)?;

        let mut builder = CreateMessagesRequestBuilder::default();
        builder
            .model(self.config.model.clone())
            .max_tokens(self.config.max_tokens as i32)
            .messages(sdk_messages);

        // Set system prompt
        if let Some(sys) = system {
            builder.system(sys.to_string());
        }

        // Set temperature (not supported when thinking is active in Anthropic protocol)
        let temperature = request_options
            .temperature_override
            .unwrap_or(self.config.temperature);
        builder.temperature(temperature);

        // Convert tools to SDK format (serde_json::Map<String, Value>)
        if !tools.is_empty() {
            let sdk_tools: Vec<serde_json::Map<String, serde_json::Value>> = tools
                .iter()
                .map(|t| {
                    let mut map = serde_json::Map::new();
                    map.insert("name".to_string(), serde_json::json!(t.name));
                    map.insert(
                        "description".to_string(),
                        serde_json::json!(t.description),
                    );
                    map.insert(
                        "input_schema".to_string(),
                        serde_json::to_value(&t.input_schema).unwrap_or_default(),
                    );
                    map
                })
                .collect();
            builder.tools(sdk_tools);

            // Set tool_choice based on request options
            let thinking_active =
                self.config.enable_thinking && self.model_supports_reasoning();
            if matches!(request_options.tool_call_mode, ToolCallMode::Required) && !thinking_active
            {
                builder.tool_choice(AnthropicToolChoice::Any);
            }
        }

        // ADR-004: Extended thinking not supported via async-anthropic v0.6.
        // Log a warning if thinking is requested but cannot be passed through.
        if self.config.enable_thinking && self.model_supports_reasoning() {
            tracing::warn!(
                "MiniMax: Extended thinking requested for model '{}' but async-anthropic v0.6 \
                 does not support extended thinking fields. Thinking is temporarily degraded. \
                 (ADR-004)",
                self.config.model
            );
        }

        builder.build().map_err(|e| LlmError::InvalidRequest {
            message: format!("Failed to build request: {}", e),
        })
    }

    /// Convert unified Message slice to Anthropic SDK Message vec
    fn convert_messages(
        &self,
        messages: &[Message],
    ) -> Result<Vec<async_anthropic::types::Message>, LlmError> {
        let mut sdk_messages = Vec::new();

        for msg in messages {
            // System messages are handled via the system parameter, skip here
            if msg.role == MessageRole::System {
                continue;
            }

            let role = match msg.role {
                MessageRole::User => AnthropicMessageRole::User,
                MessageRole::Assistant => AnthropicMessageRole::Assistant,
                MessageRole::System => continue, // Already filtered
            };

            let content_list = self.convert_content(&msg.content)?;

            let sdk_msg = MessageBuilder::default()
                .role(role)
                .content(content_list)
                .build()
                .map_err(|e| LlmError::InvalidRequest {
                    message: format!("Failed to build message: {}", e),
                })?;

            sdk_messages.push(sdk_msg);
        }

        Ok(sdk_messages)
    }

    /// Convert unified MessageContent vec to Anthropic SDK MessageContentList
    fn convert_content(
        &self,
        content: &[MessageContent],
    ) -> Result<MessageContentList, LlmError> {
        let mut blocks: Vec<AnthropicMessageContent> = Vec::new();

        for c in content {
            match c {
                MessageContent::Text { text } => {
                    blocks.push(AnthropicMessageContent::Text(AnthropicText {
                        text: text.clone(),
                    }));
                }
                MessageContent::ToolUse { id, name, input } => {
                    blocks.push(AnthropicMessageContent::ToolUse(AnthropicToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    }));
                }
                MessageContent::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    blocks.push(AnthropicMessageContent::ToolResult(AnthropicToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: Some(content.clone()),
                        is_error: is_error.unwrap_or(false),
                    }));
                }
                MessageContent::Thinking { thinking, .. } => {
                    // ADR-004: Thinking blocks cannot be round-tripped via async-anthropic v0.6.
                    // Include as text content for context preservation.
                    if !thinking.is_empty() {
                        blocks.push(AnthropicMessageContent::Text(AnthropicText {
                            text: thinking.clone(),
                        }));
                    }
                }
                MessageContent::Image { .. } => {
                    // async-anthropic v0.6 doesn't have an Image variant in MessageContent.
                    // Skip image content (MiniMax image support is limited anyway).
                    tracing::warn!("MiniMax: Skipping image content - not supported via async-anthropic SDK");
                }
                MessageContent::ToolResultMultimodal {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    // Flatten multimodal tool result to text-only since SDK doesn't support images
                    let text_content: String = content
                        .iter()
                        .filter_map(|block| {
                            if let super::types::ContentBlock::Text { text } = block {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    blocks.push(AnthropicMessageContent::ToolResult(AnthropicToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: if text_content.is_empty() {
                            None
                        } else {
                            Some(text_content)
                        },
                        is_error: is_error.unwrap_or(false),
                    }));
                }
            }
        }

        Ok(MessageContentList(blocks))
    }

    /// Parse a non-streaming CreateMessagesResponse into LlmResponse
    fn parse_response(&self, response: &CreateMessagesResponse) -> LlmResponse {
        let mut content = None;
        let mut tool_calls = Vec::new();

        if let Some(content_blocks) = &response.content {
            for block in content_blocks {
                match block {
                    AnthropicMessageContent::Text(text) => {
                        content = Some(text.text.clone());
                    }
                    AnthropicMessageContent::ToolUse(tu) => {
                        tool_calls.push(ToolCall {
                            id: tu.id.clone(),
                            name: tu.name.clone(),
                            arguments: tu.input.clone(),
                        });
                    }
                    AnthropicMessageContent::ToolResult(_) => {
                        // Shouldn't appear in response content
                    }
                }
            }
        }

        let stop_reason = response
            .stop_reason
            .as_ref()
            .map(|r| StopReason::from(r.as_str()))
            .unwrap_or(StopReason::EndTurn);

        let usage = response
            .usage
            .as_ref()
            .map(|u| UsageStats {
                input_tokens: u.input_tokens.unwrap_or(0),
                output_tokens: u.output_tokens.unwrap_or(0),
                thinking_tokens: None, // ADR-004: Not available via SDK
                cache_read_tokens: None,
                cache_creation_tokens: None,
            })
            .unwrap_or_default();

        LlmResponse {
            content,
            thinking: None, // ADR-004: Thinking not available via async-anthropic v0.6
            tool_calls,
            stop_reason,
            usage,
            model: response
                .model
                .clone()
                .unwrap_or_else(|| self.config.model.clone()),
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
        // ADR-004: Still report true for M2 models so the orchestrator knows
        // the model has reasoning capability, even though it's temporarily
        // degraded at the SDK level.
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
        if self.config.api_key.is_none() {
            return Err(missing_api_key_error("minimax"));
        }

        let request = self.build_request(
            &messages,
            system.as_deref(),
            &tools,
            &request_options,
        )?;

        let response = self
            .client
            .messages()
            .create(request)
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                // Map common Anthropic SDK errors to our error types
                if err_str.contains("401") || err_str.contains("authentication") {
                    LlmError::AuthenticationFailed {
                        message: format!("minimax: {}", err_str),
                    }
                } else if err_str.contains("429") || err_str.contains("rate") {
                    LlmError::RateLimited {
                        message: err_str,
                        retry_after: None,
                    }
                } else if err_str.contains("404") || err_str.contains("not found") {
                    LlmError::ModelNotFound {
                        model: self.config.model.clone(),
                    }
                } else if err_str.contains("400") || err_str.contains("invalid") {
                    LlmError::InvalidRequest { message: err_str }
                } else if err_str.contains("500")
                    || err_str.contains("502")
                    || err_str.contains("503")
                {
                    LlmError::ServerError {
                        message: err_str,
                        status: None,
                    }
                } else {
                    LlmError::NetworkError { message: err_str }
                }
            })?;

        Ok(self.parse_response(&response))
    }

    async fn stream_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        tx: mpsc::Sender<UnifiedStreamEvent>,
        request_options: LlmRequestOptions,
    ) -> LlmResult<LlmResponse> {
        if self.config.api_key.is_none() {
            return Err(missing_api_key_error("minimax"));
        }

        let request = self.build_request(
            &messages,
            system.as_deref(),
            &tools,
            &request_options,
        )?;

        let mut stream = self
            .client
            .messages()
            .create_stream(request)
            .await;

        let mut accumulated_content = String::new();
        let mut tool_calls = Vec::new();
        let mut usage = UsageStats::default();
        let mut stop_reason = StopReason::EndTurn;

        // State for tracking tool input accumulation
        let mut current_tool_id: Option<String> = None;
        let mut current_tool_name: Option<String> = None;
        let mut tool_input_buffer = String::new();

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    match event {
                        MessagesStreamEvent::MessageStart { message, usage: msg_usage } => {
                            // Capture initial usage from message_start
                            if let Some(u) = msg_usage.or(message.usage) {
                                usage.input_tokens = u.input_tokens.unwrap_or(0);
                                usage.output_tokens = u.output_tokens.unwrap_or(0);
                            }
                        }
                        MessagesStreamEvent::ContentBlockStart {
                            content_block,
                            ..
                        } => {
                            match &content_block {
                                AnthropicMessageContent::ToolUse(tu) => {
                                    current_tool_id = Some(tu.id.clone());
                                    current_tool_name = Some(tu.name.clone());
                                    tool_input_buffer.clear();
                                    // Don't forward ToolStart to tx; the orchestrator
                                    // handles tool lifecycle events internally.
                                }
                                _ => {}
                            }
                        }
                        MessagesStreamEvent::ContentBlockDelta { delta, .. } => match delta {
                            ContentBlockDelta::TextDelta { text } => {
                                accumulated_content.push_str(&text);
                                let _ = tx
                                    .send(UnifiedStreamEvent::TextDelta { content: text })
                                    .await;
                            }
                            ContentBlockDelta::InputJsonDelta { partial_json } => {
                                tool_input_buffer.push_str(&partial_json);
                            }
                        },
                        MessagesStreamEvent::ContentBlockStop { .. } => {
                            // If we were accumulating a tool call, finalize it
                            if let (Some(id), Some(name)) =
                                (current_tool_id.take(), current_tool_name.take())
                            {
                                let args = std::mem::take(&mut tool_input_buffer);
                                if let Ok(input) = serde_json::from_str(&args) {
                                    tool_calls.push(ToolCall {
                                        id: id.clone(),
                                        name: name.clone(),
                                        arguments: input,
                                    });
                                }
                                // ToolComplete is consumed internally by the orchestrator,
                                // not forwarded to the frontend.
                            }
                        }
                        MessagesStreamEvent::MessageDelta {
                            delta,
                            usage: delta_usage,
                        } => {
                            if let Some(u) = delta_usage {
                                usage.output_tokens += u.output_tokens.unwrap_or(0);
                            }
                            if let Some(reason) = delta.stop_reason {
                                stop_reason = StopReason::from(reason.as_str());
                            }
                        }
                        MessagesStreamEvent::MessageStop => {
                            // Stream complete; stop_reason already captured from MessageDelta
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

        Ok(LlmResponse {
            content: if accumulated_content.is_empty() {
                None
            } else {
                Some(accumulated_content)
            },
            thinking: None, // ADR-004: Thinking not available via async-anthropic v0.6
            tool_calls,
            stop_reason,
            usage,
            model: self.config.model.clone(),
        })
    }

    async fn health_check(&self) -> LlmResult<()> {
        if self.config.api_key.is_none() {
            return Err(missing_api_key_error("minimax"));
        }

        // Make a minimal request to verify connectivity and API key
        let request = CreateMessagesRequestBuilder::default()
            .model(self.config.model.clone())
            .max_tokens(1_i32)
            .messages(vec![MessageBuilder::default()
                .role(AnthropicMessageRole::User)
                .content(MessageContentList(vec![
                    AnthropicMessageContent::Text(AnthropicText {
                        text: "Hi".to_string(),
                    }),
                ]))
                .build()
                .map_err(|e| LlmError::InvalidRequest {
                    message: format!("Failed to build health check message: {}", e),
                })?])
            .build()
            .map_err(|e| LlmError::InvalidRequest {
                message: format!("Failed to build health check request: {}", e),
            })?;

        self.client
            .messages()
            .create(request)
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("401") || err_str.contains("authentication") {
                    LlmError::AuthenticationFailed {
                        message: "Invalid API key".to_string(),
                    }
                } else {
                    LlmError::NetworkError { message: err_str }
                }
            })?;

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
    fn test_text_01_no_reasoning() {
        let config = ProviderConfig {
            model: "MiniMax-Text-01".to_string(),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert!(!provider.supports_thinking());
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
    fn test_convert_text_content() {
        let provider = MinimaxProvider::new(test_config());
        let content = vec![MessageContent::Text {
            text: "Hello!".to_string(),
        }];
        let result = provider.convert_content(&content).unwrap();
        assert_eq!(result.0.len(), 1);
        match &result.0[0] {
            AnthropicMessageContent::Text(t) => assert_eq!(t.text, "Hello!"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_convert_tool_use_content() {
        let provider = MinimaxProvider::new(test_config());
        let content = vec![MessageContent::ToolUse {
            id: "tool_123".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "/test"}),
        }];
        let result = provider.convert_content(&content).unwrap();
        assert_eq!(result.0.len(), 1);
        match &result.0[0] {
            AnthropicMessageContent::ToolUse(tu) => {
                assert_eq!(tu.id, "tool_123");
                assert_eq!(tu.name, "read_file");
            }
            _ => panic!("Expected ToolUse content"),
        }
    }

    #[test]
    fn test_convert_tool_result_content() {
        let provider = MinimaxProvider::new(test_config());
        let content = vec![MessageContent::ToolResult {
            tool_use_id: "tool_123".to_string(),
            content: "file contents".to_string(),
            is_error: None,
        }];
        let result = provider.convert_content(&content).unwrap();
        assert_eq!(result.0.len(), 1);
        match &result.0[0] {
            AnthropicMessageContent::ToolResult(tr) => {
                assert_eq!(tr.tool_use_id, "tool_123");
                assert_eq!(tr.content, Some("file contents".to_string()));
                assert!(!tr.is_error);
            }
            _ => panic!("Expected ToolResult content"),
        }
    }

    #[test]
    fn test_convert_tool_result_with_error() {
        let provider = MinimaxProvider::new(test_config());
        let content = vec![MessageContent::ToolResult {
            tool_use_id: "tool_456".to_string(),
            content: "error occurred".to_string(),
            is_error: Some(true),
        }];
        let result = provider.convert_content(&content).unwrap();
        assert_eq!(result.0.len(), 1);
        match &result.0[0] {
            AnthropicMessageContent::ToolResult(tr) => {
                assert!(tr.is_error);
            }
            _ => panic!("Expected ToolResult content"),
        }
    }

    #[test]
    fn test_convert_messages_skips_system() {
        let provider = MinimaxProvider::new(test_config());
        let messages = vec![
            Message::system("Be helpful"),
            Message::user("Hello"),
            Message::assistant("Hi there"),
        ];
        let result = provider.convert_messages(&messages).unwrap();
        assert_eq!(result.len(), 2); // System message filtered out
    }

    #[test]
    fn test_build_request_basic() {
        let provider = MinimaxProvider::new(test_config());
        let messages = vec![Message::user("test")];
        let request = provider
            .build_request(&messages, None, &[], &LlmRequestOptions::default())
            .unwrap();
        assert_eq!(request.model, "MiniMax-M2.5");
    }

    #[test]
    fn test_build_request_with_system() {
        let provider = MinimaxProvider::new(test_config());
        let messages = vec![Message::user("test")];
        let request = provider
            .build_request(
                &messages,
                Some("Be helpful"),
                &[],
                &LlmRequestOptions::default(),
            )
            .unwrap();
        assert_eq!(request.system, Some("Be helpful".to_string()));
    }

    #[test]
    fn test_build_request_with_tools() {
        let provider = MinimaxProvider::new(test_config());
        let messages = vec![Message::user("test")];
        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: super::super::types::ParameterSchema::object(
                None,
                std::collections::HashMap::new(),
                vec![],
            ),
        }];
        let request = provider
            .build_request(&messages, None, &tools, &LlmRequestOptions::default())
            .unwrap();
        assert!(request.tools.is_some());
        assert_eq!(request.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_parse_response_text_only() {
        let provider = MinimaxProvider::new(test_config());
        let response = CreateMessagesResponse {
            id: Some("msg_123".to_string()),
            content: Some(vec![AnthropicMessageContent::Text(AnthropicText {
                text: "Hello world".to_string(),
            })]),
            model: Some("MiniMax-M2.5".to_string()),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: Some(async_anthropic::types::Usage {
                input_tokens: Some(10),
                output_tokens: Some(20),
            }),
        };
        let result = provider.parse_response(&response);
        assert_eq!(result.content, Some("Hello world".to_string()));
        assert!(result.tool_calls.is_empty());
        assert_eq!(result.stop_reason, StopReason::EndTurn);
        assert_eq!(result.usage.input_tokens, 10);
        assert_eq!(result.usage.output_tokens, 20);
    }

    #[test]
    fn test_parse_response_with_tool_use() {
        let provider = MinimaxProvider::new(test_config());
        let response = CreateMessagesResponse {
            id: Some("msg_456".to_string()),
            content: Some(vec![AnthropicMessageContent::ToolUse(AnthropicToolUse {
                id: "tool_call_1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "/test.rs"}),
            })]),
            model: Some("MiniMax-M2.5".to_string()),
            stop_reason: Some("tool_use".to_string()),
            stop_sequence: None,
            usage: Some(async_anthropic::types::Usage {
                input_tokens: Some(15),
                output_tokens: Some(25),
            }),
        };
        let result = provider.parse_response(&response);
        assert!(result.content.is_none());
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name, "read_file");
        assert_eq!(result.stop_reason, StopReason::ToolUse);
    }

    #[test]
    fn test_parse_response_no_usage() {
        let provider = MinimaxProvider::new(test_config());
        let response = CreateMessagesResponse {
            id: None,
            content: Some(vec![AnthropicMessageContent::Text(AnthropicText {
                text: "Hi".to_string(),
            })]),
            model: None,
            stop_reason: None,
            stop_sequence: None,
            usage: None,
        };
        let result = provider.parse_response(&response);
        assert_eq!(result.usage.input_tokens, 0);
        assert_eq!(result.usage.output_tokens, 0);
        assert_eq!(result.model, "MiniMax-M2.5"); // Falls back to config model
    }
}
