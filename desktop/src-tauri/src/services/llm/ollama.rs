//! Ollama Provider
//!
//! Implementation of the LlmProvider trait for Ollama local inference.
//! Supports local model inference without API keys.

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::provider::LlmProvider;
use super::types::{
    LlmError, LlmResponse, LlmResult, Message, MessageContent, MessageRole, ProviderConfig,
    StopReason, ToolCall, ToolDefinition, UsageStats,
};
use crate::services::streaming::adapters::OllamaAdapter;
use crate::services::streaming::{StreamAdapter, UnifiedStreamEvent};

/// Default Ollama API endpoint
const OLLAMA_DEFAULT_URL: &str = "http://localhost:11434";

/// Models known to support thinking via <think> tags
const THINKING_MODELS: &[&str] = &["deepseek-r1", "qwq", "qwen-qwq"];

/// Ollama provider for local inference
pub struct OllamaProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl OllamaProvider {
    /// Create a new Ollama provider with the given configuration
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Get the base URL for the Ollama server
    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or(OLLAMA_DEFAULT_URL)
    }

    /// Check if model supports thinking
    fn model_supports_thinking(&self) -> bool {
        let model_lower = self.config.model.to_lowercase();

        for known in THINKING_MODELS {
            if model_lower.contains(known) {
                return true;
            }
        }

        // Also check for r1/qwq patterns
        model_lower.contains("r1") || model_lower.contains("qwq")
    }

    /// Build the request body for the chat API
    fn build_request_body(
        &self,
        messages: &[Message],
        system: Option<&str>,
        _tools: &[ToolDefinition],
        stream: bool,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.config.model,
            "stream": stream,
        });

        // Add options
        let mut options = serde_json::json!({});
        options["temperature"] = serde_json::json!(self.config.temperature);
        if self.config.max_tokens > 0 {
            options["num_predict"] = serde_json::json!(self.config.max_tokens);
        }
        body["options"] = options;

        // Convert messages to Ollama format
        let mut ollama_messages: Vec<serde_json::Value> = Vec::new();

        // Add system message if provided
        if let Some(sys) = system {
            ollama_messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }

        // Add conversation messages
        for msg in messages {
            ollama_messages.push(self.message_to_ollama(msg));
        }

        body["messages"] = serde_json::json!(ollama_messages);

        // Note: Ollama tool support is model-dependent and limited
        // We skip tool definitions for now as not all models support it

        body
    }

    /// Convert a Message to Ollama API format
    fn message_to_ollama(&self, message: &Message) -> serde_json::Value {
        let role = match message.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        };

        // Collect text content
        let text_content: String = message
            .content
            .iter()
            .filter_map(|c| match c {
                MessageContent::Text { text } => Some(text.as_str()),
                MessageContent::ToolResult { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        serde_json::json!({
            "role": role,
            "content": text_content
        })
    }

    /// Parse a response from Ollama API
    fn parse_response(&self, response: &OllamaResponse) -> LlmResponse {
        let mut content = None;
        let mut thinking = None;

        if let Some(msg) = &response.message {
            if let Some(raw_content) = &msg.content {
                let (think, text) = self.extract_thinking(raw_content);
                thinking = think;
                content = text;
            }
        }

        let usage = UsageStats {
            input_tokens: response.prompt_eval_count.unwrap_or(0),
            output_tokens: response.eval_count.unwrap_or(0),
            thinking_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
        };

        LlmResponse {
            content,
            thinking,
            tool_calls: Vec::new(), // Ollama tool support is limited
            stop_reason: StopReason::EndTurn,
            usage,
            model: response
                .model
                .clone()
                .unwrap_or_else(|| self.config.model.clone()),
        }
    }

    /// Extract thinking content from <think> tags
    fn extract_thinking(&self, content: &str) -> (Option<String>, Option<String>) {
        if !self.model_supports_thinking() {
            return (
                None,
                if content.is_empty() {
                    None
                } else {
                    Some(content.to_string())
                },
            );
        }

        let mut thinking = String::new();
        let mut text = String::new();
        let mut in_thinking = false;
        let mut buffer = String::new();

        for c in content.chars() {
            buffer.push(c);

            if buffer.ends_with("<think>") {
                let len = buffer.len() - 7;
                text.push_str(&buffer[..len]);
                buffer.clear();
                in_thinking = true;
            } else if buffer.ends_with("</think>") {
                let len = buffer.len() - 8;
                thinking.push_str(&buffer[..len]);
                buffer.clear();
                in_thinking = false;
            } else if buffer.len() > 10 {
                let flush_len = buffer.len() - 10;
                if in_thinking {
                    thinking.push_str(&buffer[..flush_len]);
                } else {
                    text.push_str(&buffer[..flush_len]);
                }
                buffer = buffer[flush_len..].to_string();
            }
        }

        // Flush remaining buffer
        if in_thinking {
            thinking.push_str(&buffer);
        } else {
            text.push_str(&buffer);
        }

        let thinking_result = if thinking.is_empty() {
            None
        } else {
            Some(thinking.trim().to_string())
        };
        let text_result = if text.is_empty() {
            None
        } else {
            Some(text.trim().to_string())
        };

        (thinking_result, text_result)
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    fn supports_thinking(&self) -> bool {
        self.model_supports_thinking()
    }

    fn supports_tools(&self) -> bool {
        // Ollama tool support is model-dependent and generally limited
        false
    }

    fn context_window(&self) -> u32 {
        // Ollama models vary widely; use a conservative default.
        // Users running larger models (e.g., Llama 3.1 70B) may need to
        // adjust max_total_tokens in the orchestrator config.
        8_192
    }

    async fn send_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
    ) -> LlmResult<LlmResponse> {
        let url = format!("{}/api/chat", self.base_url());
        let body = self.build_request_body(&messages, system.as_deref(), &tools, false);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    LlmError::ProviderUnavailable {
                        message: format!("Cannot connect to Ollama at {}: {}", self.base_url(), e),
                    }
                } else {
                    LlmError::NetworkError {
                        message: e.to_string(),
                    }
                }
            })?;

        let status = response.status().as_u16();
        let body_text = response.text().await.map_err(|e| LlmError::NetworkError {
            message: e.to_string(),
        })?;

        if status != 200 {
            if status == 404 {
                return Err(LlmError::ModelNotFound {
                    model: self.config.model.clone(),
                });
            }
            return Err(LlmError::ServerError {
                message: body_text,
                status: Some(status),
            });
        }

        let ollama_response: OllamaResponse =
            serde_json::from_str(&body_text).map_err(|e| LlmError::ParseError {
                message: format!("Failed to parse response: {}", e),
            })?;

        Ok(self.parse_response(&ollama_response))
    }

    async fn stream_message(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Vec<ToolDefinition>,
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> LlmResult<LlmResponse> {
        let url = format!("{}/api/chat", self.base_url());
        let body = self.build_request_body(&messages, system.as_deref(), &tools, true);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    LlmError::ProviderUnavailable {
                        message: format!("Cannot connect to Ollama at {}: {}", self.base_url(), e),
                    }
                } else {
                    LlmError::NetworkError {
                        message: e.to_string(),
                    }
                }
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body_text = response.text().await.map_err(|e| LlmError::NetworkError {
                message: e.to_string(),
            })?;
            if status == 404 {
                return Err(LlmError::ModelNotFound {
                    model: self.config.model.clone(),
                });
            }
            return Err(LlmError::ServerError {
                message: body_text,
                status: Some(status),
            });
        }

        // Process stream
        let mut adapter = OllamaAdapter::new(&self.config.model);
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

            // Process complete JSON objects (each line is a JSON object)
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
                                    ..
                                } => {
                                    usage.input_tokens = *input_tokens;
                                    usage.output_tokens = *output_tokens;
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

        // Process any remaining data in buffer
        if !buffer.trim().is_empty() {
            if let Ok(events) = adapter.adapt(&buffer) {
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
                        _ => {}
                    }
                    if !matches!(
                        &event,
                        UnifiedStreamEvent::Complete { .. }
                            | UnifiedStreamEvent::Usage { .. }
                    ) {
                        let _ = tx.send(event).await;
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
        let url = format!("{}/api/tags", self.base_url());

        let response = self.client.get(&url).send().await.map_err(|e| {
            if e.is_connect() {
                LlmError::ProviderUnavailable {
                    message: format!("Cannot connect to Ollama at {}", self.base_url()),
                }
            } else {
                LlmError::NetworkError {
                    message: e.to_string(),
                }
            }
        })?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(LlmError::ProviderUnavailable {
                message: "Ollama server returned error".to_string(),
            })
        }
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn list_models(&self) -> LlmResult<Option<Vec<String>>> {
        let url = format!("{}/api/tags", self.base_url());

        let response = self.client.get(&url).send().await.map_err(|e| {
            if e.is_connect() {
                LlmError::ProviderUnavailable {
                    message: format!("Cannot connect to Ollama at {}", self.base_url()),
                }
            } else {
                LlmError::NetworkError {
                    message: e.to_string(),
                }
            }
        })?;

        let status = response.status().as_u16();
        if status != 200 {
            return Err(LlmError::ServerError {
                message: "Failed to list models".to_string(),
                status: Some(status),
            });
        }

        let body: serde_json::Value = response.json().await.map_err(|e| LlmError::ParseError {
            message: e.to_string(),
        })?;

        let models = body["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Some(models))
    }
}

/// Ollama API response format
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: Option<String>,
    message: Option<ResponseMessage>,
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ProviderConfig {
        ProviderConfig {
            provider: super::super::types::ProviderType::Ollama,
            api_key: None, // Ollama doesn't need API key
            model: "llama3.2".to_string(),
            base_url: Some("http://localhost:11434".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_provider_creation() {
        let provider = OllamaProvider::new(test_config());
        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.model(), "llama3.2");
        assert!(!provider.supports_thinking());
        assert!(!provider.supports_tools()); // Limited tool support
    }

    #[test]
    fn test_thinking_model() {
        let config = ProviderConfig {
            model: "deepseek-r1:14b".to_string(),
            ..test_config()
        };
        let provider = OllamaProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_qwq_model() {
        let config = ProviderConfig {
            model: "qwq:32b".to_string(),
            ..test_config()
        };
        let provider = OllamaProvider::new(config);
        assert!(provider.supports_thinking());
    }

    #[test]
    fn test_extract_thinking() {
        let config = ProviderConfig {
            model: "deepseek-r1".to_string(),
            ..test_config()
        };
        let provider = OllamaProvider::new(config);

        let (thinking, text) = provider.extract_thinking("<think>reasoning</think>answer");
        assert_eq!(thinking, Some("reasoning".to_string()));
        assert_eq!(text, Some("answer".to_string()));
    }

    #[test]
    fn test_message_conversion() {
        let provider = OllamaProvider::new(test_config());
        let message = Message::user("Hello!");

        let api_msg = provider.message_to_ollama(&message);
        assert_eq!(api_msg["role"], "user");
        assert_eq!(api_msg["content"], "Hello!");
    }

    #[test]
    fn test_base_url() {
        let provider = OllamaProvider::new(test_config());
        assert_eq!(provider.base_url(), "http://localhost:11434");

        let config = ProviderConfig {
            base_url: Some("http://192.168.1.100:11434".to_string()),
            ..test_config()
        };
        let provider = OllamaProvider::new(config);
        assert_eq!(provider.base_url(), "http://192.168.1.100:11434");
    }
}
