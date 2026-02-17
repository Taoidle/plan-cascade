//! Anthropic Claude Provider
//!
//! Implementation of the LlmProvider trait for Anthropic's Claude API.

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::provider::{missing_api_key_error, parse_http_error, LlmProvider};
use super::types::{
    LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message, MessageContent, MessageRole,
    ProviderConfig, StopReason, ToolCall, ToolCallMode, ToolDefinition, UsageStats,
};
use crate::services::proxy::build_http_client;
use crate::services::streaming::adapters::ClaudeApiAdapter;
use crate::services::streaming::{StreamAdapter, UnifiedStreamEvent};

/// Default Anthropic API endpoint
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

/// Current API version
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic Claude provider
pub struct AnthropicProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        let client = build_http_client(config.proxy.as_ref());
        Self { config, client }
    }

    /// Get the API base URL
    fn base_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(ANTHROPIC_API_URL)
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
        });

        // Add system prompt as structured block with cache_control hint
        if let Some(sys) = system {
            body["system"] = serde_json::json!([{
                "type": "text",
                "text": sys,
                "cache_control": { "type": "ephemeral" }
            }]);
        }

        // Add temperature (only if not using extended thinking)
        if !self.config.enable_thinking {
            body["temperature"] = serde_json::json!(request_options
                .temperature_override
                .unwrap_or(self.config.temperature));
        }

        // Convert messages to Claude format
        let claude_messages: Vec<serde_json::Value> = messages
            .iter()
            .filter(|m| m.role != MessageRole::System) // System is separate in Claude
            .map(|m| self.message_to_claude(m))
            .collect();
        body["messages"] = serde_json::json!(claude_messages);

        // Add tools if provided, with cache_control on the last tool
        if !tools.is_empty() {
            let tool_count = tools.len();
            let claude_tools: Vec<serde_json::Value> = tools
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    if i == tool_count - 1 {
                        self.tool_to_claude_with_cache(t)
                    } else {
                        self.tool_to_claude(t)
                    }
                })
                .collect();
            body["tools"] = serde_json::json!(claude_tools);
            if matches!(request_options.tool_call_mode, ToolCallMode::Required) {
                body["tool_choice"] = serde_json::json!({
                    "type": "any"
                });
            }
        }

        // Add extended thinking if enabled
        if self.config.enable_thinking {
            if let Some(budget) = self.config.thinking_budget {
                body["thinking"] = serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": budget
                });
            }
        }

        body
    }

    /// Convert a Message to Claude API format
    fn message_to_claude(&self, message: &Message) -> serde_json::Value {
        let role = match message.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "user", // Shouldn't happen, filtered out
        };

        let content: Vec<serde_json::Value> = message
            .content
            .iter()
            .map(|c| match c {
                MessageContent::Text { text } => {
                    serde_json::json!({
                        "type": "text",
                        "text": text
                    })
                }
                MessageContent::ToolUse { id, name, input } => {
                    serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": input
                    })
                }
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
                    result
                }
                MessageContent::Thinking {
                    thinking,
                    thinking_id,
                } => {
                    let mut obj = serde_json::json!({
                        "type": "thinking",
                        "thinking": thinking
                    });
                    if let Some(id) = thinking_id {
                        obj["thinking_id"] = serde_json::json!(id);
                    }
                    obj
                }
                MessageContent::Image { media_type, data } => {
                    serde_json::json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": media_type,
                            "data": data
                        }
                    })
                }
                MessageContent::ToolResultMultimodal {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let blocks: Vec<serde_json::Value> = content
                        .iter()
                        .map(|block| match block {
                            super::types::ContentBlock::Text { text } => {
                                serde_json::json!({ "type": "text", "text": text })
                            }
                            super::types::ContentBlock::Image { media_type, data } => {
                                serde_json::json!({
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": media_type,
                                        "data": data
                                    }
                                })
                            }
                        })
                        .collect();
                    let mut result = serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": blocks
                    });
                    if let Some(true) = is_error {
                        result["is_error"] = serde_json::json!(true);
                    }
                    result
                }
            })
            .collect();

        serde_json::json!({
            "role": role,
            "content": content
        })
    }

    /// Convert a ToolDefinition to Claude API format
    fn tool_to_claude(&self, tool: &ToolDefinition) -> serde_json::Value {
        serde_json::json!({
            "name": tool.name,
            "description": tool.description,
            "input_schema": tool.input_schema
        })
    }

    /// Convert a ToolDefinition to Claude API format with cache_control hint
    fn tool_to_claude_with_cache(&self, tool: &ToolDefinition) -> serde_json::Value {
        serde_json::json!({
            "name": tool.name,
            "description": tool.description,
            "input_schema": tool.input_schema,
            "cache_control": { "type": "ephemeral" }
        })
    }

    /// Parse a response from Claude API
    fn parse_response(&self, response: &ClaudeResponse) -> LlmResponse {
        let mut content = None;
        let mut thinking = None;
        let mut tool_calls = Vec::new();

        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    content = Some(text.clone());
                }
                ContentBlock::Thinking {
                    thinking: think_text,
                    ..
                } => {
                    thinking = Some(think_text.clone());
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: input.clone(),
                    });
                }
            }
        }

        let stop_reason = match response.stop_reason.as_deref() {
            Some("end_turn") => StopReason::EndTurn,
            Some("max_tokens") => StopReason::MaxTokens,
            Some("stop_sequence") => StopReason::StopSequence,
            Some("tool_use") => StopReason::ToolUse,
            Some(other) => StopReason::Other(other.to_string()),
            None => StopReason::EndTurn,
        };

        LlmResponse {
            content,
            thinking,
            tool_calls,
            stop_reason,
            usage: UsageStats {
                input_tokens: response.usage.input_tokens,
                output_tokens: response.usage.output_tokens,
                thinking_tokens: None, // Claude doesn't separate thinking tokens in usage
                cache_read_tokens: response.usage.cache_read_input_tokens,
                cache_creation_tokens: response.usage.cache_creation_input_tokens,
            },
            model: response.model.clone(),
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    fn supports_thinking(&self) -> bool {
        // Extended thinking is supported on Claude 3.5 Sonnet and later
        let model = self.config.model.to_lowercase();
        model.contains("claude-3") || model.contains("claude-4")
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn supports_multimodal(&self) -> bool {
        true
    }

    fn context_window(&self) -> u32 {
        200_000 // Claude 3.5/4 models have 200k context
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
            .ok_or_else(|| missing_api_key_error("anthropic"))?;

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
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("anthropic-beta", "prompt-caching-2024-07-31")
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
            return Err(parse_http_error(status, &body_text, "anthropic"));
        }

        let claude_response: ClaudeResponse =
            serde_json::from_str(&body_text).map_err(|e| LlmError::ParseError {
                message: format!("Failed to parse response: {}", e),
            })?;

        Ok(self.parse_response(&claude_response))
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
            .ok_or_else(|| missing_api_key_error("anthropic"))?;

        let body =
            self.build_request_body(&messages, system.as_deref(), &tools, true, &request_options);

        let response = self
            .client
            .post(self.base_url())
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("anthropic-beta", "prompt-caching-2024-07-31")
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
            return Err(parse_http_error(status, &body_text, "anthropic"));
        }

        // Process SSE stream
        let mut adapter = ClaudeApiAdapter::new();
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
            .ok_or_else(|| missing_api_key_error("anthropic"))?;

        // Make a minimal request to verify the API key
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "Hi"}]
        });

        let response = self
            .client
            .post(self.base_url())
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
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
            let body = response.text().await.unwrap_or_default();
            Err(parse_http_error(status, &body, "anthropic"))
        }
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }
}

/// Claude API response format
#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: ResponseUsage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        #[serde(default)]
        thinking_id: Option<String>,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Deserialize)]
struct ResponseUsage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ProviderConfig {
        ProviderConfig {
            api_key: Some("test-key".to_string()),
            model: "claude-3-5-sonnet-20241022".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_provider_creation() {
        let provider = AnthropicProvider::new(test_config());
        assert_eq!(provider.name(), "anthropic");
        assert_eq!(provider.model(), "claude-3-5-sonnet-20241022");
        assert!(provider.supports_thinking());
        assert!(provider.supports_tools());
    }

    #[test]
    fn test_message_conversion() {
        let provider = AnthropicProvider::new(test_config());
        let message = Message::user("Hello, Claude!");

        let claude_msg = provider.message_to_claude(&message);
        assert_eq!(claude_msg["role"], "user");
        assert!(claude_msg["content"].is_array());
    }

    #[test]
    fn test_tool_conversion() {
        let provider = AnthropicProvider::new(test_config());
        let tool = ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: super::super::types::ParameterSchema::object(
                None,
                std::collections::HashMap::new(),
                vec![],
            ),
        };

        let claude_tool = provider.tool_to_claude(&tool);
        assert_eq!(claude_tool["name"], "read_file");
        assert_eq!(claude_tool["description"], "Read a file");
    }

    #[test]
    fn test_request_body_building() {
        let provider = AnthropicProvider::new(test_config());
        let messages = vec![Message::user("Hello")];

        let body = provider.build_request_body(
            &messages,
            Some("Be helpful"),
            &[],
            false,
            &LlmRequestOptions::default(),
        );
        assert_eq!(body["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(body["stream"], false);
    }

    #[test]
    fn test_system_prompt_structured_block_with_cache_control() {
        let provider = AnthropicProvider::new(test_config());
        let messages = vec![Message::user("Hello")];

        let body = provider.build_request_body(
            &messages,
            Some("Be helpful"),
            &[],
            false,
            &LlmRequestOptions::default(),
        );

        // System prompt should be an array of structured blocks
        let system = &body["system"];
        assert!(system.is_array(), "system should be an array of blocks");

        let blocks = system.as_array().unwrap();
        assert_eq!(blocks.len(), 1, "should have exactly one system block");

        let block = &blocks[0];
        assert_eq!(block["type"], "text");
        assert_eq!(block["text"], "Be helpful");
        assert_eq!(
            block["cache_control"]["type"], "ephemeral",
            "system block must have cache_control with type ephemeral"
        );
    }

    #[test]
    fn test_no_system_prompt_omits_system_field() {
        let provider = AnthropicProvider::new(test_config());
        let messages = vec![Message::user("Hello")];

        let body =
            provider.build_request_body(&messages, None, &[], false, &LlmRequestOptions::default());

        assert!(
            body.get("system").is_none(),
            "system field should be absent when no system prompt"
        );
    }

    #[test]
    fn test_last_tool_has_cache_control() {
        let provider = AnthropicProvider::new(test_config());
        let messages = vec![Message::user("Hello")];

        let tools = vec![
            ToolDefinition {
                name: "read_file".to_string(),
                description: "Read a file".to_string(),
                input_schema: super::super::types::ParameterSchema::object(
                    None,
                    std::collections::HashMap::new(),
                    vec![],
                ),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "Write a file".to_string(),
                input_schema: super::super::types::ParameterSchema::object(
                    None,
                    std::collections::HashMap::new(),
                    vec![],
                ),
            },
            ToolDefinition {
                name: "list_dir".to_string(),
                description: "List directory".to_string(),
                input_schema: super::super::types::ParameterSchema::object(
                    None,
                    std::collections::HashMap::new(),
                    vec![],
                ),
            },
        ];

        let body = provider.build_request_body(
            &messages,
            None,
            &tools,
            false,
            &LlmRequestOptions::default(),
        );

        let tool_array = body["tools"].as_array().unwrap();
        assert_eq!(tool_array.len(), 3);

        // First tool should NOT have cache_control
        assert!(
            tool_array[0].get("cache_control").is_none(),
            "first tool should not have cache_control"
        );

        // Second tool should NOT have cache_control
        assert!(
            tool_array[1].get("cache_control").is_none(),
            "middle tool should not have cache_control"
        );

        // Last tool MUST have cache_control
        assert_eq!(
            tool_array[2]["cache_control"]["type"], "ephemeral",
            "last tool must have cache_control with type ephemeral"
        );
    }

    #[test]
    fn test_single_tool_has_cache_control() {
        let provider = AnthropicProvider::new(test_config());
        let messages = vec![Message::user("Hello")];

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

        let tool_array = body["tools"].as_array().unwrap();
        assert_eq!(tool_array.len(), 1);

        // Single tool is also the last tool, should have cache_control
        assert_eq!(
            tool_array[0]["cache_control"]["type"], "ephemeral",
            "single tool (which is last) must have cache_control"
        );
    }

    #[test]
    fn test_no_tools_no_cache_control() {
        let provider = AnthropicProvider::new(test_config());
        let messages = vec![Message::user("Hello")];

        let body =
            provider.build_request_body(&messages, None, &[], false, &LlmRequestOptions::default());

        assert!(
            body.get("tools").is_none(),
            "tools field should be absent when no tools provided"
        );
    }

    #[test]
    fn test_tool_to_claude_no_cache_control() {
        let provider = AnthropicProvider::new(test_config());
        let tool = ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: super::super::types::ParameterSchema::object(
                None,
                std::collections::HashMap::new(),
                vec![],
            ),
        };

        let result = provider.tool_to_claude(&tool);
        assert!(
            result.get("cache_control").is_none(),
            "tool_to_claude should not include cache_control"
        );
    }

    #[test]
    fn test_tool_to_claude_with_cache_has_cache_control() {
        let provider = AnthropicProvider::new(test_config());
        let tool = ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: super::super::types::ParameterSchema::object(
                None,
                std::collections::HashMap::new(),
                vec![],
            ),
        };

        let result = provider.tool_to_claude_with_cache(&tool);
        assert_eq!(result["name"], "read_file");
        assert_eq!(result["description"], "Read a file");
        assert_eq!(
            result["cache_control"]["type"], "ephemeral",
            "tool_to_claude_with_cache must include cache_control"
        );
    }
}
