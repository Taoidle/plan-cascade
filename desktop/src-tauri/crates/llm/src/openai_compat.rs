//! Shared helpers for OpenAI-compatible providers (OpenAI, DeepSeek, Qwen).

use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::chat_completion::ChatCompletionRequest;
use openai_api_rs::v1::chat_completion::chat_completion_stream::ChatCompletionStreamRequest;
use openai_api_rs::v1::error::APIError;

use crate::provider::{missing_api_key_error, parse_http_error};
use crate::types::{LlmError, LlmResult, ProviderConfig};

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
}
