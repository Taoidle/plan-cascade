//! MiniMax Provider
//!
//! Implementation of the LlmProvider trait for MiniMax API.
//! Uses a hybrid approach: reqwest for HTTP transport with serde_json::Value
//! request bodies, and anthropic-async SDK for response types and SSE streaming.
//!
//! MiniMax exposes an Anthropic-compatible endpoint at:
//!   - Global: api.minimax.io/anthropic/v1/messages
//!   - China:  api.minimaxi.com/anthropic/v1/messages
//!
//! ADR-003: SDK migration from async-anthropic to anthropic-async (hybrid approach).
//! ADR-004: Extended thinking resolved — ClaudeApiAdapter handles ThinkingDelta
//!          in streaming, enabling thinking content forwarding for M2 models.

use anthropic_async::config::Config as _; // trait: .headers(), .url()
use anthropic_async::AnthropicConfig;
use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::sync::mpsc;

use super::provider::{missing_api_key_error, parse_http_error, LlmProvider};
use super::types::{
    FallbackToolFormatMode, LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message,
    MessageContent, MessageRole, ProviderConfig, StopReason, ToolCall, ToolCallMode,
    ToolCallReliability, ToolDefinition, UsageStats,
};
use crate::http_client::build_http_client;
use crate::streaming_adapters::ClaudeApiAdapter;
use plan_cascade_core::streaming::{StreamAdapter, UnifiedStreamEvent};

/// Default MiniMax Anthropic-compatible API base URL (global).
const MINIMAX_ANTHROPIC_BASE_URL: &str = "https://api.minimax.io/anthropic";

/// China mainland base URL.
#[allow(dead_code)]
const MINIMAX_ANTHROPIC_BASE_URL_CN: &str = "https://api.minimaxi.com/anthropic";

/// MiniMax provider using hybrid reqwest + anthropic-async SDK
pub struct MinimaxProvider {
    config: ProviderConfig,
    http_client: reqwest::Client,
    anthropic_config: AnthropicConfig,
    /// Full messages API URL (base + /v1/messages), computed once at construction.
    messages_url: String,
}

impl MinimaxProvider {
    /// Create a new MiniMax provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let api_key = config.api_key.clone().unwrap_or_default();
        let raw_base = config
            .base_url
            .as_deref()
            .unwrap_or(MINIMAX_ANTHROPIC_BASE_URL);

        let base_url = Self::normalize_base_url(raw_base);
        let messages_url = format!("{}/v1/messages", base_url);

        let anthropic_config = AnthropicConfig::new()
            .with_api_base(&base_url)
            .with_api_key(api_key);

        tracing::info!("MiniMax provider initialized: url={}", messages_url);

        let http_client = build_http_client(config.proxy.as_ref());

        Self {
            config,
            http_client,
            anthropic_config,
            messages_url,
        }
    }

    /// Normalize base URL to the Anthropic-compatible endpoint.
    ///
    /// Handles several user configuration patterns:
    ///   - OpenAI-style URL (`/v1/chat/completions`) → converted to `/anthropic`
    ///   - Full Anthropic URL (`/anthropic/v1/messages`) → stripped to `/anthropic`
    ///   - Just the base (`/anthropic`) → used as-is
    ///   - Host only (`https://api.minimax.io`) → `/anthropic` appended
    fn normalize_base_url(raw: &str) -> String {
        let url = raw.trim_end_matches('/');

        // If the URL contains /v1 but NOT /anthropic, the user configured an
        // OpenAI-compatible endpoint.  Extract the host and switch to /anthropic.
        if url.contains("/v1") && !url.contains("/anthropic") {
            if let Some(pos) = url.find("/v1") {
                let host = url[..pos].trim_end_matches('/');
                return format!("{}/anthropic", host);
            }
        }

        // Strip known Anthropic endpoint suffixes to get the base.
        let base = url
            .trim_end_matches("/messages")
            .trim_end_matches("/v1")
            .trim_end_matches('/');

        // If base is just a host (no /anthropic path), append /anthropic.
        if !base.contains("/anthropic") {
            return format!("{}/anthropic", base);
        }

        base.to_string()
    }

    /// Check if model supports reasoning (M2 series)
    fn model_supports_reasoning(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.contains("minimax-m2")
    }

    /// Build the JSON request body for the MiniMax Anthropic-compatible API.
    ///
    /// Uses serde_json::Value to support ToolUse blocks in assistant messages,
    /// which ContentBlockParam in anthropic-async lacks.
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
        });

        // System prompt as simple string (MiniMax doesn't support cache_control)
        if let Some(sys) = system {
            body["system"] = serde_json::json!(sys);
        }

        // Temperature (not supported when thinking is active in Anthropic protocol)
        let thinking_active = self.config.enable_thinking && self.model_supports_reasoning();
        if !thinking_active {
            let temperature = request_options
                .temperature_override
                .unwrap_or(self.config.temperature);
            body["temperature"] = serde_json::json!(temperature);
        }

        // Convert messages to Anthropic JSON format
        let api_messages: Vec<serde_json::Value> = messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| self.message_to_json(m))
            .collect();
        body["messages"] = serde_json::json!(api_messages);

        // Tools
        if !tools.is_empty() {
            let api_tools: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(api_tools);

            // tool_choice (not allowed with thinking active)
            if matches!(request_options.tool_call_mode, ToolCallMode::Required) && !thinking_active
            {
                body["tool_choice"] = serde_json::json!({"type": "any"});
            }
        }

        body
    }

    /// Convert a unified Message to Anthropic-compatible JSON.
    fn message_to_json(&self, message: &Message) -> serde_json::Value {
        let role = match message.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "user", // Shouldn't happen, filtered out
        };

        let content: Vec<serde_json::Value> = message
            .content
            .iter()
            .filter_map(|c| match c {
                MessageContent::Text { text } => Some(serde_json::json!({
                    "type": "text",
                    "text": text
                })),
                MessageContent::ToolUse { id, name, input } => Some(serde_json::json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": input
                })),
                MessageContent::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let mut result = serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": content
                    });
                    if let Some(true) = is_error {
                        result["is_error"] = serde_json::json!(true);
                    }
                    Some(result)
                }
                MessageContent::Thinking { thinking, .. } => {
                    // ADR-004: Thinking blocks included as text for context preservation.
                    // MiniMax may not support round-tripping thinking blocks.
                    if thinking.is_empty() {
                        None
                    } else {
                        Some(serde_json::json!({
                            "type": "text",
                            "text": thinking
                        }))
                    }
                }
                MessageContent::Image { .. } => {
                    tracing::warn!("MiniMax: Skipping image content - not supported");
                    None
                }
                MessageContent::ToolResultMultimodal {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    // Flatten multimodal tool result to text-only
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

                    let mut result = serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                    });
                    if !text_content.is_empty() {
                        result["content"] = serde_json::json!(text_content);
                    }
                    if let Some(true) = is_error {
                        result["is_error"] = serde_json::json!(true);
                    }
                    Some(result)
                }
            })
            .collect();

        serde_json::json!({
            "role": role,
            "content": content
        })
    }

    /// Parse a non-streaming MessagesCreateResponse into LlmResponse.
    fn parse_response(
        &self,
        response: &anthropic_async::types::MessagesCreateResponse,
    ) -> LlmResponse {
        let mut content = None;
        let mut tool_calls = Vec::new();

        for block in &response.content {
            match block {
                anthropic_async::types::ContentBlock::Text { text } => {
                    content = Some(text.clone());
                }
                anthropic_async::types::ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: input.clone(),
                    });
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
                input_tokens: u.input_tokens.unwrap_or(0) as u32,
                output_tokens: u.output_tokens.unwrap_or(0) as u32,
                thinking_tokens: None,
                cache_read_tokens: u.cache_read_input_tokens.map(|v| v as u32),
                cache_creation_tokens: u.cache_creation_input_tokens.map(|v| v as u32),
            })
            .unwrap_or_default();

        LlmResponse {
            content,
            thinking: None,
            tool_calls,
            stop_reason,
            usage,
            model: response.model.clone(),
            search_citations: Vec::new(),
        }
    }

    /// Parse a non-streaming response from raw JSON Value.
    /// Handles `thinking` content blocks that the anthropic-async crate doesn't support.
    fn parse_response_from_value(&self, raw: &serde_json::Value) -> LlmResponse {
        let mut content = None;
        let mut thinking = None;
        let mut tool_calls = Vec::new();

        if let Some(blocks) = raw.get("content").and_then(|c| c.as_array()) {
            for block in blocks {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            content = Some(text.to_string());
                        }
                    }
                    "thinking" => {
                        if let Some(text) = block.get("thinking").and_then(|t| t.as_str()) {
                            thinking = Some(text.to_string());
                        }
                    }
                    "tool_use" => {
                        let id = block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let input = block
                            .get("input")
                            .cloned()
                            .unwrap_or(serde_json::Value::Object(Default::default()));
                        tool_calls.push(ToolCall {
                            id,
                            name,
                            arguments: input,
                        });
                    }
                    other => {
                        tracing::debug!("MiniMax: skipping unknown content block type: {}", other);
                    }
                }
            }
        }

        let stop_reason = raw
            .get("stop_reason")
            .and_then(|r| r.as_str())
            .map(StopReason::from)
            .unwrap_or(StopReason::EndTurn);

        let usage = raw
            .get("usage")
            .map(|u| UsageStats {
                input_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                output_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                thinking_tokens: None,
                cache_read_tokens: u
                    .get("cache_read_input_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
                cache_creation_tokens: u
                    .get("cache_creation_input_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
            })
            .unwrap_or_default();

        let model = raw
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        LlmResponse {
            content,
            thinking,
            tool_calls,
            stop_reason,
            usage,
            model,
            search_citations: Vec::new(),
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
        // ADR-004: Report true for M2 models — the model has reasoning capability,
        // and ClaudeApiAdapter now handles ThinkingDelta events in streaming.
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
        if model.contains("m2.5") || model.contains("m2.1") {
            // MiniMax-M2.5, M2.5-highspeed, M2.1, M2.1-highspeed
            245_760
        } else if model.contains("m2") {
            200_000
        } else if model.contains("text-01") {
            4_000_000
        } else {
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

        let body = self.build_request_body(
            &messages,
            system.as_deref(),
            &tools,
            false,
            &request_options,
        );

        let headers = self
            .anthropic_config
            .headers()
            .map_err(|e| LlmError::InvalidRequest {
                message: format!("Failed to build headers: {}", e),
            })?;

        let url = &self.messages_url;
        tracing::debug!("MiniMax send_message POST {}", url);

        let response = self
            .http_client
            .post(url.as_str())
            .headers(headers)
            .header("content-type", "application/json")
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
            tracing::warn!(
                "MiniMax API error: HTTP {} from {} — {}",
                status,
                url,
                body_text
            );
            return Err(parse_http_error(status, &body_text, "minimax"));
        }

        // Parse as serde_json::Value first to handle `thinking` content blocks
        // that the anthropic-async crate's ContentBlock enum doesn't support.
        // MiniMax M2.5 returns thinking blocks in non-streaming responses.
        let raw: serde_json::Value =
            serde_json::from_str(&body_text).map_err(|e| LlmError::ParseError {
                message: format!("Failed to parse response: {}", e),
            })?;

        Ok(self.parse_response_from_value(&raw))
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

        let body =
            self.build_request_body(&messages, system.as_deref(), &tools, true, &request_options);

        let headers = self
            .anthropic_config
            .headers()
            .map_err(|e| LlmError::InvalidRequest {
                message: format!("Failed to build headers: {}", e),
            })?;

        let url = &self.messages_url;
        tracing::debug!("MiniMax stream_message POST {}", url);

        let response = self
            .http_client
            .post(url.as_str())
            .headers(headers)
            .header("content-type", "application/json")
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
            tracing::warn!(
                "MiniMax API error: HTTP {} from {} — {}",
                status,
                url,
                body_text
            );
            return Err(parse_http_error(status, &body_text, "minimax"));
        }

        // Process SSE stream using ClaudeApiAdapter (Anthropic-compatible SSE format).
        // ADR-004: ClaudeApiAdapter handles ThinkingDelta for M2 reasoning models.
        let mut adapter = ClaudeApiAdapter::new();
        let mut accumulated_content = String::new();
        let mut accumulated_thinking = String::new();
        let mut tool_calls = Vec::new();
        let mut usage = UsageStats::default();
        let mut stop_reason = StopReason::EndTurn;

        let mut stream = response.bytes_stream();
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
                                    thinking_tokens,
                                    cache_read_tokens,
                                    cache_creation_tokens,
                                } => {
                                    if *input_tokens > 0 {
                                        usage.input_tokens = *input_tokens;
                                    }
                                    usage.output_tokens += *output_tokens;
                                    if thinking_tokens.is_some() {
                                        usage.thinking_tokens = *thinking_tokens;
                                    }
                                    if cache_read_tokens.is_some() {
                                        usage.cache_read_tokens = *cache_read_tokens;
                                    }
                                    if cache_creation_tokens.is_some() {
                                        usage.cache_creation_tokens = *cache_creation_tokens;
                                    }
                                }
                                UnifiedStreamEvent::Complete {
                                    stop_reason: Some(reason),
                                } => {
                                    stop_reason = StopReason::from(reason.as_str());
                                }
                                _ => {}
                            }

                            // Forward streaming events but suppress internal signals
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
            search_citations: Vec::new(),
        })
    }

    async fn health_check(&self) -> LlmResult<()> {
        if self.config.api_key.is_none() {
            return Err(missing_api_key_error("minimax"));
        }

        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "Hi"}]
        });

        let headers = self
            .anthropic_config
            .headers()
            .map_err(|e| LlmError::InvalidRequest {
                message: format!("Failed to build headers: {}", e),
            })?;

        let url = &self.messages_url;

        let response = self
            .http_client
            .post(url.as_str())
            .headers(headers)
            .header("content-type", "application/json")
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
            let body_text = response.text().await.unwrap_or_default();
            Err(parse_http_error(status, &body_text, "minimax"))
        }
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anthropic_async::types::{
        ContentBlock as ApiContentBlock, MessagesCreateResponse, Usage as ApiUsage,
    };

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
        assert_eq!(
            provider.default_fallback_mode(),
            FallbackToolFormatMode::Soft
        );
    }

    #[test]
    fn test_default_fallback_mode_thinking_on() {
        let config = ProviderConfig {
            enable_thinking: true,
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(
            provider.default_fallback_mode(),
            FallbackToolFormatMode::Off
        );
    }

    #[test]
    fn test_base_url_constants() {
        assert_eq!(
            MINIMAX_ANTHROPIC_BASE_URL,
            "https://api.minimax.io/anthropic"
        );
        assert_eq!(
            MINIMAX_ANTHROPIC_BASE_URL_CN,
            "https://api.minimaxi.com/anthropic"
        );
    }

    #[test]
    fn test_message_to_json_text() {
        let provider = MinimaxProvider::new(test_config());
        let msg = Message::user("Hello!");
        let json = provider.message_to_json(&msg);
        assert_eq!(json["role"], "user");
        let content = json["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Hello!");
    }

    #[test]
    fn test_message_to_json_tool_use() {
        let provider = MinimaxProvider::new(test_config());
        let msg = Message {
            role: MessageRole::Assistant,
            content: vec![MessageContent::ToolUse {
                id: "tool_123".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "/test"}),
            }],
        };
        let json = provider.message_to_json(&msg);
        assert_eq!(json["role"], "assistant");
        let content = json["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_use");
        assert_eq!(content[0]["id"], "tool_123");
        assert_eq!(content[0]["name"], "read_file");
    }

    #[test]
    fn test_message_to_json_tool_result() {
        let provider = MinimaxProvider::new(test_config());
        let msg = Message {
            role: MessageRole::User,
            content: vec![MessageContent::ToolResult {
                tool_use_id: "tool_123".to_string(),
                content: "file contents".to_string(),
                is_error: None,
            }],
        };
        let json = provider.message_to_json(&msg);
        let content = json["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_result");
        assert_eq!(content[0]["tool_use_id"], "tool_123");
        assert_eq!(content[0]["content"], "file contents");
        assert!(content[0].get("is_error").is_none());
    }

    #[test]
    fn test_message_to_json_tool_result_with_error() {
        let provider = MinimaxProvider::new(test_config());
        let msg = Message {
            role: MessageRole::User,
            content: vec![MessageContent::ToolResult {
                tool_use_id: "tool_456".to_string(),
                content: "error occurred".to_string(),
                is_error: Some(true),
            }],
        };
        let json = provider.message_to_json(&msg);
        let content = json["content"].as_array().unwrap();
        assert_eq!(content[0]["is_error"], true);
    }

    #[test]
    fn test_build_request_body_basic() {
        let provider = MinimaxProvider::new(test_config());
        let messages = vec![Message::user("test")];
        let body =
            provider.build_request_body(&messages, None, &[], false, &LlmRequestOptions::default());
        assert_eq!(body["model"], "MiniMax-M2.5");
        assert_eq!(body["stream"], false);
        assert!(body.get("system").is_none());
    }

    #[test]
    fn test_build_request_body_with_system() {
        let provider = MinimaxProvider::new(test_config());
        let messages = vec![Message::user("test")];
        let body = provider.build_request_body(
            &messages,
            Some("Be helpful"),
            &[],
            false,
            &LlmRequestOptions::default(),
        );
        assert_eq!(body["system"], "Be helpful");
    }

    #[test]
    fn test_build_request_body_with_tools() {
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
        let body = provider.build_request_body(
            &messages,
            None,
            &tools,
            false,
            &LlmRequestOptions::default(),
        );
        let tools_arr = body["tools"].as_array().unwrap();
        assert_eq!(tools_arr.len(), 1);
        assert_eq!(tools_arr[0]["name"], "read_file");
    }

    #[test]
    fn test_build_request_body_filters_system_messages() {
        let provider = MinimaxProvider::new(test_config());
        let messages = vec![
            Message::system("Be helpful"),
            Message::user("Hello"),
            Message::assistant("Hi there"),
        ];
        let body =
            provider.build_request_body(&messages, None, &[], false, &LlmRequestOptions::default());
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2); // System message filtered out
    }

    #[test]
    fn test_parse_response_text_only() {
        let provider = MinimaxProvider::new(test_config());
        let response = MessagesCreateResponse {
            id: "msg_123".to_string(),
            kind: "message".to_string(),
            role: anthropic_async::types::MessageRole::Assistant,
            content: vec![ApiContentBlock::Text {
                text: "Hello world".to_string(),
            }],
            model: "MiniMax-M2.5".to_string(),
            stop_reason: Some("end_turn".to_string()),
            usage: Some(ApiUsage {
                input_tokens: Some(10),
                output_tokens: Some(20),
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
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
        let response = MessagesCreateResponse {
            id: "msg_456".to_string(),
            kind: "message".to_string(),
            role: anthropic_async::types::MessageRole::Assistant,
            content: vec![ApiContentBlock::ToolUse {
                id: "tool_call_1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "/test.rs"}),
            }],
            model: "MiniMax-M2.5".to_string(),
            stop_reason: Some("tool_use".to_string()),
            usage: Some(ApiUsage {
                input_tokens: Some(15),
                output_tokens: Some(25),
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
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
        let response = MessagesCreateResponse {
            id: "msg_789".to_string(),
            kind: "message".to_string(),
            role: anthropic_async::types::MessageRole::Assistant,
            content: vec![ApiContentBlock::Text {
                text: "Hi".to_string(),
            }],
            model: "MiniMax-M2.5".to_string(),
            stop_reason: None,
            usage: None,
        };
        let result = provider.parse_response(&response);
        assert_eq!(result.usage.input_tokens, 0);
        assert_eq!(result.usage.output_tokens, 0);
        assert_eq!(result.model, "MiniMax-M2.5");
    }

    #[test]
    fn test_parse_response_with_cache_tokens() {
        let provider = MinimaxProvider::new(test_config());
        let response = MessagesCreateResponse {
            id: "msg_cache".to_string(),
            kind: "message".to_string(),
            role: anthropic_async::types::MessageRole::Assistant,
            content: vec![ApiContentBlock::Text {
                text: "cached".to_string(),
            }],
            model: "MiniMax-M2.5".to_string(),
            stop_reason: Some("end_turn".to_string()),
            usage: Some(ApiUsage {
                input_tokens: Some(100),
                output_tokens: Some(50),
                cache_creation_input_tokens: Some(80),
                cache_read_input_tokens: Some(60),
            }),
        };
        let result = provider.parse_response(&response);
        assert_eq!(result.usage.cache_creation_tokens, Some(80));
        assert_eq!(result.usage.cache_read_tokens, Some(60));
    }

    #[test]
    fn test_custom_base_url_cn() {
        let config = ProviderConfig {
            base_url: Some(MINIMAX_ANTHROPIC_BASE_URL_CN.to_string()),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(
            provider.messages_url,
            "https://api.minimaxi.com/anthropic/v1/messages"
        );
    }

    #[test]
    fn test_default_messages_url() {
        let provider = MinimaxProvider::new(test_config());
        assert_eq!(
            provider.messages_url,
            "https://api.minimax.io/anthropic/v1/messages"
        );
    }

    #[test]
    fn test_base_url_with_v1_suffix_normalized() {
        // User may configure base_url with /v1 already included
        let config = ProviderConfig {
            base_url: Some("https://api.minimaxi.com/anthropic/v1".to_string()),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(
            provider.messages_url,
            "https://api.minimaxi.com/anthropic/v1/messages"
        );
    }

    #[test]
    fn test_base_url_with_full_path_normalized() {
        // User may configure full URL (like OpenAI/DeepSeek providers)
        let config = ProviderConfig {
            base_url: Some("https://api.minimaxi.com/anthropic/v1/messages".to_string()),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(
            provider.messages_url,
            "https://api.minimaxi.com/anthropic/v1/messages"
        );
    }

    #[test]
    fn test_base_url_with_trailing_slash_normalized() {
        let config = ProviderConfig {
            base_url: Some("https://api.minimax.io/anthropic/".to_string()),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(
            provider.messages_url,
            "https://api.minimax.io/anthropic/v1/messages"
        );
    }

    #[test]
    fn test_openai_url_converted_to_anthropic() {
        // User configured the OpenAI-compatible URL (from old default_base_url)
        let config = ProviderConfig {
            base_url: Some("https://api.minimax.io/v1/chat/completions".to_string()),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(
            provider.messages_url,
            "https://api.minimax.io/anthropic/v1/messages"
        );
    }

    #[test]
    fn test_openai_base_url_converted_to_anthropic() {
        // User configured the OpenAI base URL without /chat/completions
        let config = ProviderConfig {
            base_url: Some("https://api.minimax.io/v1".to_string()),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(
            provider.messages_url,
            "https://api.minimax.io/anthropic/v1/messages"
        );
    }

    #[test]
    fn test_openai_url_cn_converted_to_anthropic() {
        // China user with OpenAI-compatible URL
        let config = ProviderConfig {
            base_url: Some("https://api.minimaxi.com/v1/chat/completions".to_string()),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(
            provider.messages_url,
            "https://api.minimaxi.com/anthropic/v1/messages"
        );
    }

    #[test]
    fn test_host_only_url_gets_anthropic_path() {
        // User configured just the host
        let config = ProviderConfig {
            base_url: Some("https://api.minimax.io".to_string()),
            ..test_config()
        };
        let provider = MinimaxProvider::new(config);
        assert_eq!(
            provider.messages_url,
            "https://api.minimax.io/anthropic/v1/messages"
        );
    }
}
