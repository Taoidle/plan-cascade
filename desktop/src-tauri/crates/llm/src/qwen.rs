//! Qwen Provider (OpenAI-compatible).
//!
//! Uses `openai-api-rs` against DashScope OpenAI-compatible endpoint.

use async_trait::async_trait;
use futures_util::StreamExt;
use openai_api_rs::v1::chat_completion::chat_completion_stream::ChatCompletionStreamResponse;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

use super::openai_compat::{
    build_client, map_api_error, value_to_chat_request, value_to_chat_stream_request,
};
use super::provider::LlmProvider;
use super::types::{
    FallbackToolFormatMode, LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message,
    MessageContent, MessageRole, ProviderConfig, StopReason, ToolCall, ToolCallMode,
    ToolCallReliability, ToolDefinition, UsageStats,
};
use crate::reliable_catalog::is_reliable_model;
use plan_cascade_core::streaming::UnifiedStreamEvent;

/// Default DashScope OpenAI-compatible endpoint.
const QWEN_API_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

/// Qwen provider backed by OpenAI-compatible API.
pub struct QwenProvider {
    config: ProviderConfig,
}

impl QwenProvider {
    /// Create a new Qwen provider with the given configuration.
    pub fn new(config: ProviderConfig) -> Self {
        Self { config }
    }

    fn build_compat_client(&self) -> LlmResult<openai_api_rs::v1::api::OpenAIClient> {
        // Qwen accepts both OpenAI-style chat/completions URLs and legacy DashScope
        // regional base URLs; normalization is handled in `openai_compat`.
        build_client(&self.config, "qwen", QWEN_API_URL, false)
    }

    /// Check if model supports reasoning (Qwen3 series, QwQ models).
    fn model_supports_reasoning(&self) -> bool {
        let model = self.config.model.to_lowercase();
        model.contains("qwen3") || model.contains("qwq") || model.contains("thinking")
    }

    /// Check if native web search is enabled via provider options.
    fn native_search_enabled(&self) -> bool {
        self.config
            .options
            .get("enable_search")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Build Qwen search_options JSON from provider config options.
    fn build_search_options_json(&self) -> Option<serde_json::Value> {
        if !self.native_search_enabled() {
            return None;
        }
        let mut obj = serde_json::Map::new();
        if let Some(v) = self
            .config
            .options
            .get("search_forced")
            .and_then(|v| v.as_bool())
        {
            obj.insert("forced_search".to_string(), serde_json::json!(v));
        }
        if let Some(v) = self
            .config
            .options
            .get("search_enable_source")
            .and_then(|v| v.as_bool())
        {
            obj.insert("enable_source".to_string(), serde_json::json!(v));
        }
        if let Some(v) = self
            .config
            .options
            .get("search_enable_citation")
            .and_then(|v| v.as_bool())
        {
            obj.insert("enable_citation".to_string(), serde_json::json!(v));
        }
        if let Some(v) = self
            .config
            .options
            .get("search_strategy")
            .and_then(|v| v.as_str())
        {
            obj.insert("search_strategy".to_string(), serde_json::json!(v));
        }
        Some(serde_json::Value::Object(obj))
    }

    /// Build OpenAI-compatible messages with strict tool-result sequencing.
    fn build_input_messages_json(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> Vec<serde_json::Value> {
        let mut api_messages: Vec<serde_json::Value> = Vec::new();
        // Track unresolved tool ids from latest assistant tool call turn.
        let mut pending_tool_call_ids: HashSet<String> = HashSet::new();

        if let Some(sys) = system {
            api_messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    pending_tool_call_ids.clear();
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
                    let mut tool_results: Vec<(String, String)> = Vec::new();
                    for content in &msg.content {
                        match content {
                            MessageContent::ToolResult {
                                tool_use_id,
                                content,
                                ..
                            } => {
                                tool_results.push((tool_use_id.clone(), content.clone()));
                            }
                            MessageContent::ToolResultMultimodal {
                                tool_use_id,
                                content,
                                ..
                            } => {
                                let text = content
                                    .iter()
                                    .filter_map(|block| {
                                        if let crate::types::ContentBlock::Text { text } = block {
                                            Some(text.as_str())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                let normalized = if text.trim().is_empty() {
                                    "[multimodal tool result omitted non-text blocks]".to_string()
                                } else {
                                    text
                                };
                                tool_results.push((tool_use_id.clone(), normalized));
                            }
                            _ => {}
                        }
                    }

                    if tool_results.is_empty() {
                        pending_tool_call_ids.clear();
                        api_messages.push(serde_json::json!({
                            "role": "user",
                            "content": Self::extract_text_content(&msg.content)
                        }));
                    } else if tool_results
                        .iter()
                        .all(|(tool_use_id, _)| pending_tool_call_ids.contains(tool_use_id))
                    {
                        for (tool_use_id, result_content) in tool_results {
                            pending_tool_call_ids.remove(&tool_use_id);
                            api_messages.push(serde_json::json!({
                                "role": "tool",
                                "content": result_content,
                                "tool_call_id": tool_use_id
                            }));
                        }
                    } else {
                        // Orphan tool results -> degrade to user text to avoid hard API errors.
                        pending_tool_call_ids.clear();
                        let fallback_text = tool_results
                            .into_iter()
                            .map(|(tool_use_id, result_content)| {
                                format!("[tool_result:{}]\n{}", tool_use_id, result_content)
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n");
                        api_messages.push(serde_json::json!({
                            "role": "user",
                            "content": fallback_text
                        }));
                    }
                }
                MessageRole::Assistant => {
                    let text_content = Self::extract_text_content(&msg.content);
                    let mut assistant_tool_ids: Vec<String> = Vec::new();
                    let mut tool_calls_json: Vec<serde_json::Value> = Vec::new();

                    for (index, c) in msg.content.iter().enumerate() {
                        if let MessageContent::ToolUse { id, name, input } = c {
                            assistant_tool_ids.push(id.clone());
                            tool_calls_json.push(serde_json::json!({
                                "id": id,
                                "type": "function",
                                "index": index as i32,
                                "function": {
                                    "name": name,
                                    "arguments": input.to_string()
                                }
                            }));
                        }
                    }

                    let mut message_json = serde_json::json!({
                        "role": "assistant",
                        "content": text_content
                    });
                    if !tool_calls_json.is_empty() {
                        message_json["tool_calls"] = serde_json::json!(tool_calls_json);
                        pending_tool_call_ids = assistant_tool_ids
                            .into_iter()
                            .filter(|id| !id.is_empty())
                            .collect();
                    } else {
                        pending_tool_call_ids.clear();
                    }
                    api_messages.push(message_json);
                }
            }
        }

        api_messages
    }

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

    fn convert_tools(tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
        tools
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
            .collect()
    }

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
            "messages": self.build_input_messages_json(messages, system),
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::json!(Self::convert_tools(tools));
            body["parallel_tool_calls"] = serde_json::json!(true);
            if matches!(request_options.tool_call_mode, ToolCallMode::Required) {
                body["tool_choice"] = serde_json::json!("required");
            }
        }

        if stream {
            body["stream_options"] = serde_json::json!({
                "include_usage": true
            });
            // DashScope compatible-mode may return non-incremental chunks unless
            // this flag is set. Keep it enabled to improve real-time rendering.
            body["incremental_output"] = serde_json::json!(true);
        }

        if self.config.enable_thinking && self.model_supports_reasoning() {
            body["enable_thinking"] = serde_json::json!(true);
            if let Some(budget) = self.config.thinking_budget {
                body["thinking_budget"] = serde_json::json!(budget);
            }
        }

        if self.native_search_enabled() {
            body["enable_search"] = serde_json::json!(true);
            if let Some(search_options) = self.build_search_options_json() {
                body["search_options"] = search_options;
            }
        }

        body
    }

    fn parse_response(&self, response: &QwenResponse) -> LlmResponse {
        let choice = response.choices.first();

        let mut content = None;
        let mut thinking = None;
        let mut tool_calls = Vec::new();
        let mut stop_reason = StopReason::EndTurn;

        if let Some(choice) = choice {
            if let Some(msg) = &choice.message {
                content = msg.content.clone();
                thinking = msg.reasoning_content.clone();

                if let Some(tcs) = &msg.tool_calls {
                    for tc in tcs {
                        if tc.id.trim().is_empty() {
                            continue;
                        }
                        let Some(name) = tc.function.name.as_ref().filter(|n| !n.is_empty()) else {
                            continue;
                        };
                        let arguments = tc
                            .function
                            .arguments
                            .as_deref()
                            .and_then(|a| serde_json::from_str::<serde_json::Value>(a).ok())
                            .unwrap_or(serde_json::Value::Null);
                        tool_calls.push(ToolCall {
                            id: tc.id.clone(),
                            name: name.clone(),
                            arguments,
                        });
                    }
                }
            }
            if let Some(reason) = &choice.finish_reason {
                stop_reason = StopReason::from(reason.as_str());
            }
        }

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
            model: response
                .model
                .clone()
                .unwrap_or_else(|| self.config.model.clone()),
            search_citations: Vec::new(),
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

    fn supports_native_search(&self) -> bool {
        self.native_search_enabled()
    }

    fn tool_call_reliability(&self) -> ToolCallReliability {
        if is_reliable_model(self.config.provider, &self.config.model) {
            ToolCallReliability::Reliable
        } else {
            ToolCallReliability::Unreliable
        }
    }

    fn default_fallback_mode(&self) -> FallbackToolFormatMode {
        if matches!(self.tool_call_reliability(), ToolCallReliability::Reliable) {
            return FallbackToolFormatMode::Off;
        }
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
        } else if model.contains("qwen3.5-plus")
            || model.contains("qwen3-plus")
            || model.contains("qwen-plus")
            || model.contains("qwen-turbo")
            || model.contains("qwen-flash")
            || model.contains("qwen-long")
        {
            1_000_000
        } else if model.contains("qwen-max-latest")
            || model.contains("qwen-max-2025")
            || model.contains("qwen-max-2026")
        {
            131_072
        } else if model.contains("qwen-max") {
            32_768
        } else if model.contains("qwen3-coder") {
            1_000_000
        } else if model.contains("qwq") {
            131_072
        } else if model.contains("qwen2.5-turbo") {
            1_000_000
        } else if model.contains("qwen2.5") {
            131_072
        } else if model.contains("qwen3") {
            131_072
        } else {
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
        let body = self.build_request_body(
            &messages,
            system.as_deref(),
            &tools,
            false,
            &request_options,
        );
        let request = value_to_chat_request("qwen", body)?;
        let mut client = self.build_compat_client()?;
        let response = client
            .chat_completion(request)
            .await
            .map_err(|e| map_api_error("qwen", e))?;

        let qwen_response: QwenResponse =
            serde_json::from_value(serde_json::to_value(response).map_err(|e| {
                LlmError::ParseError {
                    message: format!("Failed to serialize response: {}", e),
                }
            })?)
            .map_err(|e| LlmError::ParseError {
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
        let body =
            self.build_request_body(&messages, system.as_deref(), &tools, true, &request_options);
        let request = value_to_chat_stream_request("qwen", body)?;
        let mut client = self.build_compat_client()?;
        let mut stream = client
            .chat_completion_stream(request)
            .await
            .map_err(|e| map_api_error("qwen", e))?;

        let mut accumulated_content = String::new();
        let mut accumulated_thinking = String::new();
        let mut tool_calls = Vec::new();
        let usage = UsageStats::default();
        let mut stop_reason = StopReason::EndTurn;
        let mut in_thinking = false;

        let mut pending_tools: HashMap<String, (Option<String>, String)> = HashMap::new();
        let mut pending_order: Vec<String> = Vec::new();
        let mut last_tool_id: Option<String> = None;

        while let Some(event) = stream.next().await {
            match event {
                ChatCompletionStreamResponse::Content(content) => {
                    if !content.is_empty() {
                        accumulated_content.push_str(&content);
                        let _ = tx.send(UnifiedStreamEvent::TextDelta { content }).await;
                    }
                }
                ChatCompletionStreamResponse::ReasoningContent(content) => {
                    if !content.is_empty() {
                        if !in_thinking {
                            in_thinking = true;
                            let _ = tx
                                .send(UnifiedStreamEvent::ThinkingStart { thinking_id: None })
                                .await;
                        }
                        accumulated_thinking.push_str(&content);
                        let _ = tx
                            .send(UnifiedStreamEvent::ThinkingDelta {
                                content,
                                thinking_id: None,
                            })
                            .await;
                    }
                }
                ChatCompletionStreamResponse::ToolCall(chunks) => {
                    for chunk in chunks {
                        let id = if chunk.id.is_empty() {
                            match last_tool_id.as_ref() {
                                Some(existing) => existing.clone(),
                                None => continue,
                            }
                        } else {
                            last_tool_id = Some(chunk.id.clone());
                            chunk.id
                        };
                        if !pending_tools.contains_key(&id) {
                            pending_order.push(id.clone());
                        }
                        let entry = pending_tools
                            .entry(id)
                            .or_insert_with(|| (None, String::new()));
                        if let Some(name) = chunk.function.name {
                            if !name.is_empty() {
                                entry.0 = Some(name);
                            }
                        }
                        if let Some(arguments) = chunk.function.arguments {
                            entry.1.push_str(&arguments);
                        }
                    }
                }
                ChatCompletionStreamResponse::Done => break,
            }
        }

        for id in pending_order {
            if let Some((name, args)) = pending_tools.remove(&id) {
                if let Some(name) = name {
                    let arguments = serde_json::from_str::<serde_json::Value>(&args)
                        .unwrap_or(serde_json::Value::Null);
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments,
                    });
                }
            }
        }

        if !tool_calls.is_empty() {
            stop_reason = StopReason::ToolUse;
        }

        if in_thinking {
            let _ = tx
                .send(UnifiedStreamEvent::ThinkingEnd { thinking_id: None })
                .await;
        }

        // Only fallback when stream produced absolutely no usable events.
        // If thinking already streamed, keep stream semantics instead of downgrading
        // to a non-stream retry (which causes long "burst" output).
        if accumulated_content.trim().is_empty()
            && accumulated_thinking.trim().is_empty()
            && tool_calls.is_empty()
        {
            let fallback = self
                .send_message(messages, system, tools, request_options.clone())
                .await?;
            if let Some(content) = &fallback.content {
                let _ = tx
                    .send(UnifiedStreamEvent::TextDelta {
                        content: content.clone(),
                    })
                    .await;
            }
            return Ok(fallback);
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
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "Hi"}],
            "stream": false
        });
        let request = value_to_chat_request("qwen", body)?;
        let mut client = self.build_compat_client()?;
        client
            .chat_completion(request)
            .await
            .map(|_| ())
            .map_err(|e| map_api_error("qwen", e))
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }
}

#[derive(Debug, Deserialize)]
struct QwenResponse {
    model: Option<String>,
    choices: Vec<QwenChoice>,
    usage: Option<QwenUsage>,
}

#[derive(Debug, Deserialize)]
struct QwenChoice {
    message: Option<QwenMessage>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QwenMessage {
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<QwenToolCall>>,
}

#[derive(Debug, Deserialize)]
struct QwenToolCall {
    id: String,
    function: QwenFunction,
}

#[derive(Debug, Deserialize)]
struct QwenFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QwenUsage {
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
    fn test_context_window_matrix() {
        let config_max = ProviderConfig {
            model: "qwen3-max".to_string(),
            ..test_config()
        };
        assert_eq!(QwenProvider::new(config_max).context_window(), 262_144);

        let config_plus = ProviderConfig {
            model: "qwen-plus".to_string(),
            ..test_config()
        };
        assert_eq!(QwenProvider::new(config_plus).context_window(), 1_000_000);

        let config_35_plus = ProviderConfig {
            model: "qwen3.5-plus".to_string(),
            ..test_config()
        };
        assert_eq!(
            QwenProvider::new(config_35_plus).context_window(),
            1_000_000
        );
    }

    #[test]
    fn test_native_search_enabled_via_options() {
        let mut config = test_config();
        config
            .options
            .insert("enable_search".to_string(), serde_json::json!(true));
        let provider = QwenProvider::new(config);
        assert!(provider.supports_native_search());
    }

    #[test]
    fn test_input_json_orphan_tool_result_falls_back_to_user() {
        let provider = QwenProvider::new(test_config());
        let messages = vec![Message {
            role: MessageRole::User,
            content: vec![MessageContent::ToolResult {
                tool_use_id: "call_orphan".to_string(),
                content: "result".to_string(),
                is_error: None,
            }],
        }];

        let msgs = provider.build_input_messages_json(&messages, None);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert!(msgs[0]["content"]
            .as_str()
            .unwrap_or_default()
            .contains("[tool_result:call_orphan]"));
    }

    #[test]
    fn test_input_json_assistant_with_tool_calls() {
        let provider = QwenProvider::new(test_config());
        let messages = vec![Message {
            role: MessageRole::Assistant,
            content: vec![
                MessageContent::Text {
                    text: "Let me call a tool".to_string(),
                },
                MessageContent::ToolUse {
                    id: "call_1".to_string(),
                    name: "Read".to_string(),
                    input: serde_json::json!({"path": "README.md"}),
                },
            ],
        }];

        let msgs = provider.build_input_messages_json(&messages, None);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "assistant");
        assert!(msgs[0]["tool_calls"].is_array());
    }

    #[test]
    fn test_qwen_legacy_api_v1_base_url_is_normalized() {
        let provider = QwenProvider::new(ProviderConfig {
            base_url: Some("https://dashscope.aliyuncs.com/api/v1".to_string()),
            ..test_config()
        });
        assert!(provider.build_compat_client().is_ok());
    }
}
