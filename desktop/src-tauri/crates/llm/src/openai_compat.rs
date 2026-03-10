//! Shared helpers for OpenAI-compatible providers (OpenAI, DeepSeek, GLM, Qwen).

use std::collections::HashSet;

use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::chat_completion::ChatCompletionRequest;
use openai_api_rs::v1::chat_completion::chat_completion_stream::ChatCompletionStreamRequest;
use openai_api_rs::v1::error::APIError;

use crate::provider::{missing_api_key_error, parse_http_error};
use crate::types::{LlmError, LlmResult, Message, MessageContent, MessageRole, ProviderConfig};

const CHAT_COMPLETIONS_SUFFIX: &str = "/chat/completions";
const QWEN_COMPATIBLE_MODE_PATH: &str = "/compatible-mode/v1";

fn normalize_qwen_base_path(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');

    if trimmed.is_empty() || trimmed == "/" {
        return Some(QWEN_COMPATIBLE_MODE_PATH.to_string());
    }

    if trimmed.ends_with("/api/v1") {
        let prefix = &trimmed[..trimmed.len() - "/api/v1".len()];
        if prefix.is_empty() {
            return Some(QWEN_COMPATIBLE_MODE_PATH.to_string());
        }
        return Some(format!("{prefix}{QWEN_COMPATIBLE_MODE_PATH}"));
    }

    if trimmed.ends_with(QWEN_COMPATIBLE_MODE_PATH) {
        return Some(trimmed.to_string());
    }

    None
}

fn normalize_endpoint(
    chat_completions_url: &str,
    provider: &str,
    strict_chat_completions_url: bool,
) -> LlmResult<String> {
    let mut parsed =
        url::Url::parse(chat_completions_url).map_err(|e| LlmError::InvalidRequest {
            message: format!(
                "{}: invalid base_url '{}': {}",
                provider, chat_completions_url, e
            ),
        })?;

    let normalized_path = parsed.path().trim_end_matches('/').to_string();
    if normalized_path.ends_with(CHAT_COMPLETIONS_SUFFIX) {
        let base_path =
            normalized_path[..normalized_path.len() - CHAT_COMPLETIONS_SUFFIX.len()].to_string();
        parsed.set_path(if base_path.is_empty() {
            "/"
        } else {
            base_path.as_str()
        });
    } else if provider == "qwen" {
        if let Some(base_path) = normalize_qwen_base_path(&normalized_path) {
            parsed.set_path(&base_path);
        } else if strict_chat_completions_url {
            return Err(LlmError::InvalidRequest {
                message: format!(
                    "{}: base_url must point to an OpenAI-compatible chat/completions endpoint",
                    provider
                ),
            });
        }
    } else if strict_chat_completions_url {
        return Err(LlmError::InvalidRequest {
            message: format!(
                "{}: base_url must point to an OpenAI-compatible chat/completions endpoint",
                provider
            ),
        });
    }

    let endpoint = parsed.to_string();
    Ok(endpoint.trim_end_matches('/').to_string())
}

pub fn build_client(
    config: &ProviderConfig,
    provider: &str,
    default_chat_completions_url: &str,
    strict_chat_completions_url: bool,
) -> LlmResult<OpenAIClient> {
    let api_key = config
        .api_key
        .as_ref()
        .ok_or_else(|| missing_api_key_error(provider))?;

    let chat_url = config
        .base_url
        .as_deref()
        .unwrap_or(default_chat_completions_url);
    let endpoint = normalize_endpoint(chat_url, provider, strict_chat_completions_url)?;

    let mut builder = OpenAIClient::builder()
        .with_api_key(api_key.clone())
        .with_endpoint(endpoint);

    if let Some(proxy) = config.proxy.as_ref() {
        builder = builder.with_proxy(proxy.url());
    } else {
        builder = builder.with_no_proxy(true);
    }

    builder.build().map_err(|e| LlmError::Other {
        message: format!(
            "{}: failed to build OpenAI-compatible client: {}",
            provider, e
        ),
    })
}

pub fn value_to_chat_request(
    provider: &str,
    body: serde_json::Value,
) -> LlmResult<ChatCompletionRequest> {
    serde_json::from_value(body).map_err(|e| LlmError::InvalidRequest {
        message: format!("{}: failed to build chat request: {}", provider, e),
    })
}

pub fn value_to_chat_stream_request(
    provider: &str,
    body: serde_json::Value,
) -> LlmResult<ChatCompletionStreamRequest> {
    serde_json::from_value(body).map_err(|e| LlmError::InvalidRequest {
        message: format!("{}: failed to build streaming request: {}", provider, e),
    })
}

fn extract_text_content(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|content| {
            if let MessageContent::Text { text } = content {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_tool_result_content(content: &MessageContent) -> Option<(String, String)> {
    match content {
        MessageContent::ToolResult {
            tool_use_id,
            content,
            ..
        } => Some((tool_use_id.clone(), content.clone())),
        MessageContent::ToolResultMultimodal {
            tool_use_id,
            content,
            ..
        } => {
            let text = content
                .iter()
                .map(|block| match block {
                    crate::types::ContentBlock::Text { text } => text.clone(),
                    crate::types::ContentBlock::Image { media_type, data } => {
                        format!("[Image: data:{};base64,<{} bytes>]", media_type, data.len())
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some((tool_use_id.clone(), text))
        }
        _ => None,
    }
}

pub fn build_openai_compatible_messages(
    messages: &[Message],
    system: Option<&str>,
) -> Vec<serde_json::Value> {
    let mut api_messages: Vec<serde_json::Value> = Vec::new();
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
                let tool_results: Vec<(String, String)> = msg
                    .content
                    .iter()
                    .filter_map(normalize_tool_result_content)
                    .collect();

                if tool_results.is_empty() {
                    pending_tool_call_ids.clear();
                    api_messages.push(serde_json::json!({
                        "role": "user",
                        "content": extract_text_content(msg)
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
                let text_content = extract_text_content(msg);
                let tool_calls: Vec<serde_json::Value> = msg
                    .content
                    .iter()
                    .filter_map(|content| {
                        if let MessageContent::ToolUse { id, name, input } = content {
                            Some(serde_json::json!({
                                "id": id,
                                "type": "function",
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

                if tool_calls.is_empty() {
                    pending_tool_call_ids.clear();
                    api_messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": text_content
                    }));
                } else {
                    pending_tool_call_ids = tool_calls
                        .iter()
                        .filter_map(|tool_call| {
                            tool_call
                                .get("id")
                                .and_then(|value| value.as_str())
                                .map(ToOwned::to_owned)
                        })
                        .collect();

                    let mut message_json = serde_json::json!({
                        "role": "assistant",
                        "tool_calls": tool_calls
                    });
                    if text_content.is_empty() {
                        message_json["content"] = serde_json::Value::Null;
                    } else {
                        message_json["content"] = serde_json::json!(text_content);
                    }
                    api_messages.push(message_json);
                }
            }
        }
    }

    api_messages
}

fn parse_status_code(text: &str) -> Option<u16> {
    for token in text.split(|c: char| !c.is_ascii_digit()) {
        if token.len() == 3 {
            if let Ok(code) = token.parse::<u16>() {
                if (100..=599).contains(&code) {
                    return Some(code);
                }
            }
        }
    }
    None
}

pub fn map_api_error(provider: &str, err: APIError) -> LlmError {
    let message = err.to_string();

    if let Some(status) = parse_status_code(&message) {
        return parse_http_error(status, &message, provider);
    }

    let signal = message.to_lowercase();
    if signal.contains("timeout")
        || signal.contains("connect")
        || signal.contains("dns")
        || signal.contains("network")
    {
        LlmError::NetworkError { message }
    } else {
        LlmError::Other { message }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ContentBlock;

    #[test]
    fn test_stream_request_accepts_integer_tool_schema() {
        let body = serde_json::json!({
            "model": "qwen3-max",
            "messages": [{ "role": "user", "content": "hi" }],
            "stream": true,
            "tools": [{
                "type": "function",
                "function": {
                    "name": "sum",
                    "description": "sum numbers",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "count": { "type": "integer" }
                        },
                        "required": ["count"]
                    }
                }
            }]
        });

        let result = value_to_chat_stream_request("qwen", body);
        assert!(result.is_ok(), "unexpected error: {:?}", result.err());
    }

    #[test]
    fn test_non_stream_request_accepts_integer_tool_schema() {
        let body = serde_json::json!({
            "model": "qwen3-max",
            "messages": [{ "role": "user", "content": "hi" }],
            "stream": false,
            "tools": [{
                "type": "function",
                "function": {
                    "name": "sum",
                    "description": "sum numbers",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "count": { "type": "integer" }
                        },
                        "required": ["count"]
                    }
                }
            }]
        });

        let result = value_to_chat_request("qwen", body);
        assert!(result.is_ok(), "unexpected error: {:?}", result.err());
    }

    #[test]
    fn test_build_openai_compatible_messages_orphan_tool_result_falls_back_to_user() {
        let messages = vec![Message {
            role: MessageRole::User,
            content: vec![MessageContent::ToolResult {
                tool_use_id: "call_orphan".to_string(),
                content: "result".to_string(),
                is_error: None,
            }],
        }];

        let payload = build_openai_compatible_messages(&messages, None);
        assert_eq!(payload.len(), 1);
        assert_eq!(payload[0]["role"], "user");
        assert!(payload[0]["content"]
            .as_str()
            .unwrap_or_default()
            .contains("[tool_result:call_orphan]"));
    }

    #[test]
    fn test_build_openai_compatible_messages_preserves_valid_tool_pair() {
        let messages = vec![
            Message {
                role: MessageRole::Assistant,
                content: vec![MessageContent::ToolUse {
                    id: "call_1".to_string(),
                    name: "Read".to_string(),
                    input: serde_json::json!({"file_path": "README.md"}),
                }],
            },
            Message::tool_result("call_1", "contents", false),
            Message {
                role: MessageRole::User,
                content: vec![MessageContent::ToolResultMultimodal {
                    tool_use_id: "call_orphan_mm".to_string(),
                    content: vec![
                        ContentBlock::Text {
                            text: "text".to_string(),
                        },
                        ContentBlock::Image {
                            media_type: "image/png".to_string(),
                            data: "abcd".to_string(),
                        },
                    ],
                    is_error: None,
                }],
            },
        ];

        let payload = build_openai_compatible_messages(&messages, None);
        assert_eq!(payload.len(), 3);
        assert_eq!(payload[0]["role"], "assistant");
        assert_eq!(payload[1]["role"], "tool");
        assert_eq!(payload[1]["tool_call_id"], "call_1");
        assert_eq!(payload[2]["role"], "user");
        assert!(payload[2]["content"]
            .as_str()
            .unwrap_or_default()
            .contains("call_orphan_mm"));
    }
}
