//! LLM Types
//!
//! Core types for LLM provider interactions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported LLM provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Anthropic,
    OpenAI,
    DeepSeek,
    Glm,
    Qwen,
    Ollama,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Anthropic => write!(f, "anthropic"),
            ProviderType::OpenAI => write!(f, "openai"),
            ProviderType::DeepSeek => write!(f, "deepseek"),
            ProviderType::Glm => write!(f, "glm"),
            ProviderType::Qwen => write!(f, "qwen"),
            ProviderType::Ollama => write!(f, "ollama"),
        }
    }
}

/// Tool calling mode preference for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallMode {
    /// Provider chooses when to call tools.
    Auto,
    /// Provider should require tool calls when tools are available.
    Required,
    /// Disable tool calling for this request.
    None,
}

impl Default for ToolCallMode {
    fn default() -> Self {
        Self::Auto
    }
}

/// Tool call reliability classification for LLM providers.
///
/// Distinguishes between a provider's API-level tool support (`supports_tools()`)
/// and the actual reliability of the model's tool call emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallReliability {
    /// Provider's native tool calling works consistently (Anthropic, OpenAI).
    Reliable,
    /// Provider claims tool support but emission is inconsistent (Qwen, DeepSeek, GLM).
    /// The orchestrator should inject Soft fallback instructions alongside native tools.
    Unreliable,
    /// Provider does not support native tool calling (Ollama).
    /// The orchestrator uses full prompt-based fallback.
    None,
}

/// Prompt-fallback tool format mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackToolFormatMode {
    /// Do not include prompt fallback format instructions.
    Off,
    /// Include regular fallback format instructions.
    Soft,
    /// Include strict fallback instructions (analysis mode).
    Strict,
}

impl Default for FallbackToolFormatMode {
    fn default() -> Self {
        Self::Off
    }
}

/// Per-request options for provider behavior.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmRequestOptions {
    /// Tool calling behavior for this request.
    #[serde(default)]
    pub tool_call_mode: ToolCallMode,
    /// Prompt-fallback mode for tool-call text formats.
    #[serde(default)]
    pub fallback_tool_format_mode: FallbackToolFormatMode,
    /// Optional temperature override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_override: Option<f32>,
    /// Optional reasoning effort override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort_override: Option<String>,
    /// Optional analysis phase identifier for provider-side tuning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis_phase: Option<String>,
}

/// Configuration for an LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// The provider type
    pub provider: ProviderType,
    /// API key (not needed for Ollama)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Base URL override (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Model name to use
    pub model: String,
    /// Maximum tokens to generate
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Temperature (0.0 - 1.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Enable extended thinking/reasoning if supported
    #[serde(default)]
    pub enable_thinking: bool,
    /// Thinking budget tokens (for Claude extended thinking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<u32>,
    /// Reasoning effort level (for OpenAI o1/o3)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    /// Optional override for fallback tool format mode.
    /// When None, the orchestrator auto-determines based on provider reliability.
    /// When Some(mode), forces that mode regardless of provider reliability.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub fallback_tool_format_mode: Option<FallbackToolFormatMode>,
    /// Provider-specific options
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

fn default_max_tokens() -> u32 {
    4096
}

fn default_temperature() -> f32 {
    0.7
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            provider: ProviderType::Anthropic,
            api_key: None,
            base_url: None,
            model: "claude-3-5-sonnet-20241022".to_string(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            enable_thinking: false,
            thinking_budget: None,
            reasoning_effort: None,
            fallback_tool_format_mode: None,
            options: HashMap::new(),
        }
    }
}

/// Message role in a conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Content type within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    /// Plain text content
    Text { text: String },
    /// Tool use request from the assistant
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Tool result from execution
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    /// Thinking/reasoning content
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_id: Option<String>,
    },
    /// Image content (base64 encoded, for multimodal providers)
    Image { media_type: String, data: String },
    /// Tool result with multimodal content (text + images)
    ToolResultMultimodal {
        tool_use_id: String,
        content: Vec<ContentBlock>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// A content block that can be text or image (for multimodal tool results)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content
    Text { text: String },
    /// Base64-encoded image
    Image { media_type: String, data: String },
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: MessageRole,
    /// Message content (can be multiple blocks)
    pub content: Vec<MessageContent>,
}

impl Message {
    /// Create a simple text message
    pub fn text(role: MessageRole, text: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![MessageContent::Text { text: text.into() }],
        }
    }

    /// Create a user message
    pub fn user(text: impl Into<String>) -> Self {
        Self::text(MessageRole::User, text)
    }

    /// Create an assistant message
    pub fn assistant(text: impl Into<String>) -> Self {
        Self::text(MessageRole::Assistant, text)
    }

    /// Create a system message
    pub fn system(text: impl Into<String>) -> Self {
        Self::text(MessageRole::System, text)
    }

    /// Create a tool result message
    pub fn tool_result(
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            role: MessageRole::User,
            content: vec![MessageContent::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: content.into(),
                is_error: if is_error { Some(true) } else { None },
            }],
        }
    }

    /// Create a multimodal tool result message (text + images)
    pub fn tool_result_multimodal(
        tool_use_id: impl Into<String>,
        blocks: Vec<ContentBlock>,
        is_error: bool,
    ) -> Self {
        Self {
            role: MessageRole::User,
            content: vec![MessageContent::ToolResultMultimodal {
                tool_use_id: tool_use_id.into(),
                content: blocks,
                is_error: if is_error { Some(true) } else { None },
            }],
        }
    }
}

/// JSON Schema for tool parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, ParameterSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<ParameterSchema>>,
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

impl ParameterSchema {
    /// Create a string schema
    pub fn string(description: Option<&str>) -> Self {
        Self {
            schema_type: "string".to_string(),
            description: description.map(|s| s.to_string()),
            properties: None,
            required: None,
            items: None,
            enum_values: None,
            default: None,
        }
    }

    /// Create an integer schema
    pub fn integer(description: Option<&str>) -> Self {
        Self {
            schema_type: "integer".to_string(),
            description: description.map(|s| s.to_string()),
            properties: None,
            required: None,
            items: None,
            enum_values: None,
            default: None,
        }
    }

    /// Create a boolean schema
    pub fn boolean(description: Option<&str>) -> Self {
        Self {
            schema_type: "boolean".to_string(),
            description: description.map(|s| s.to_string()),
            properties: None,
            required: None,
            items: None,
            enum_values: None,
            default: None,
        }
    }

    /// Create an object schema
    pub fn object(
        description: Option<&str>,
        properties: HashMap<String, ParameterSchema>,
        required: Vec<String>,
    ) -> Self {
        Self {
            schema_type: "object".to_string(),
            description: description.map(|s| s.to_string()),
            properties: Some(properties),
            required: Some(required),
            items: None,
            enum_values: None,
            default: None,
        }
    }

    /// Create an array schema
    pub fn array(description: Option<&str>, items: ParameterSchema) -> Self {
        Self {
            schema_type: "array".to_string(),
            description: description.map(|s| s.to_string()),
            properties: None,
            required: None,
            items: Some(Box::new(items)),
            enum_values: None,
            default: None,
        }
    }
}

/// Definition of a tool that can be called by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique name of the tool
    pub name: String,
    /// Description of what the tool does
    pub description: String,
    /// JSON schema for the tool's input parameters
    pub input_schema: ParameterSchema,
}

/// A tool call requested by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call
    pub id: String,
    /// Name of the tool to call
    pub name: String,
    /// Arguments to pass to the tool
    pub arguments: serde_json::Value,
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStats {
    /// Number of input/prompt tokens
    pub input_tokens: u32,
    /// Number of output/completion tokens
    pub output_tokens: u32,
    /// Number of thinking/reasoning tokens (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<u32>,
    /// Number of cache read tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u32>,
    /// Number of cache creation tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u32>,
}

impl UsageStats {
    /// Total tokens used
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens + self.output_tokens + self.thinking_tokens.unwrap_or(0)
    }
}

/// Stop reason for the response
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of response
    EndTurn,
    /// Hit max tokens limit
    MaxTokens,
    /// Stopped at a stop sequence
    StopSequence,
    /// Model wants to use a tool
    ToolUse,
    /// Other/unknown reason
    Other(String),
}

impl From<&str> for StopReason {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "end_turn" | "stop" => StopReason::EndTurn,
            "max_tokens" | "length" => StopReason::MaxTokens,
            "stop_sequence" => StopReason::StopSequence,
            "tool_use" | "tool_calls" | "function_call" => StopReason::ToolUse,
            other => StopReason::Other(other.to_string()),
        }
    }
}

/// Response from an LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Text content of the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Thinking/reasoning content (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    /// Tool calls requested by the model
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// Why the response ended
    pub stop_reason: StopReason,
    /// Token usage statistics
    pub usage: UsageStats,
    /// The model that generated the response
    pub model: String,
}

impl LlmResponse {
    /// Check if the response has tool calls
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    /// Check if this is a final response (no more tool calls needed)
    pub fn is_final(&self) -> bool {
        self.tool_calls.is_empty() && self.stop_reason != StopReason::ToolUse
    }
}

/// Error types for LLM operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LlmError {
    /// Authentication failed (invalid API key)
    AuthenticationFailed { message: String },
    /// Rate limit exceeded
    RateLimited {
        message: String,
        retry_after: Option<u32>,
    },
    /// Model not found or not available
    ModelNotFound { model: String },
    /// Invalid request (bad parameters)
    InvalidRequest { message: String },
    /// Server error from the provider
    ServerError {
        message: String,
        status: Option<u16>,
    },
    /// Network/connection error
    NetworkError { message: String },
    /// Response parsing error
    ParseError { message: String },
    /// Provider not available (e.g., Ollama not running)
    ProviderUnavailable { message: String },
    /// Context length exceeded
    ContextLengthExceeded {
        message: String,
        max_tokens: Option<u32>,
    },
    /// Other error
    Other { message: String },
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::AuthenticationFailed { message } => {
                write!(f, "Authentication failed: {}", message)
            }
            LlmError::RateLimited { message, .. } => {
                write!(f, "Rate limited: {}", message)
            }
            LlmError::ModelNotFound { model } => {
                write!(f, "Model not found: {}", model)
            }
            LlmError::InvalidRequest { message } => {
                write!(f, "Invalid request: {}", message)
            }
            LlmError::ServerError { message, status } => {
                if let Some(s) = status {
                    write!(f, "Server error ({}): {}", s, message)
                } else {
                    write!(f, "Server error: {}", message)
                }
            }
            LlmError::NetworkError { message } => {
                write!(f, "Network error: {}", message)
            }
            LlmError::ParseError { message } => {
                write!(f, "Parse error: {}", message)
            }
            LlmError::ProviderUnavailable { message } => {
                write!(f, "Provider unavailable: {}", message)
            }
            LlmError::ContextLengthExceeded { message, .. } => {
                write!(f, "Context length exceeded: {}", message)
            }
            LlmError::Other { message } => {
                write!(f, "Error: {}", message)
            }
        }
    }
}

impl std::error::Error for LlmError {}

/// Result type for LLM operations
pub type LlmResult<T> = Result<T, LlmError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_default() {
        let config = ProviderConfig::default();
        assert_eq!(config.provider, ProviderType::Anthropic);
        assert_eq!(config.max_tokens, 4096);
        assert!((config.temperature - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_provider_config_serialization() {
        let config = ProviderConfig {
            provider: ProviderType::OpenAI,
            api_key: Some("sk-test".to_string()),
            base_url: None,
            model: "gpt-4".to_string(),
            max_tokens: 2048,
            temperature: 0.5,
            enable_thinking: false,
            thinking_budget: None,
            reasoning_effort: None,
            fallback_tool_format_mode: None,
            options: HashMap::new(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: ProviderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.model, "gpt-4");
        assert_eq!(parsed.max_tokens, 2048);
    }

    #[test]
    fn test_message_creation() {
        let user_msg = Message::user("Hello");
        assert_eq!(user_msg.role, MessageRole::User);
        assert_eq!(user_msg.content.len(), 1);

        let assistant_msg = Message::assistant("Hi there");
        assert_eq!(assistant_msg.role, MessageRole::Assistant);

        let tool_result = Message::tool_result("tool_123", "result data", false);
        assert_eq!(tool_result.role, MessageRole::User);
    }

    #[test]
    fn test_message_content_serialization() {
        let content = MessageContent::ToolUse {
            id: "tool_123".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "/test"}),
        };

        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"tool_use\""));
        assert!(json.contains("\"name\":\"read_file\""));
    }

    #[test]
    fn test_tool_definition() {
        let mut properties = HashMap::new();
        properties.insert(
            "file_path".to_string(),
            ParameterSchema::string(Some("Path to file")),
        );

        let tool = ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: ParameterSchema::object(
                Some("Read file parameters"),
                properties,
                vec!["file_path".to_string()],
            ),
        };

        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("\"name\":\"read_file\""));
    }

    #[test]
    fn test_usage_stats() {
        let usage = UsageStats {
            input_tokens: 100,
            output_tokens: 50,
            thinking_tokens: Some(20),
            cache_read_tokens: None,
            cache_creation_tokens: None,
        };

        assert_eq!(usage.total_tokens(), 170);
    }

    #[test]
    fn test_stop_reason_from_str() {
        assert_eq!(StopReason::from("end_turn"), StopReason::EndTurn);
        assert_eq!(StopReason::from("stop"), StopReason::EndTurn);
        assert_eq!(StopReason::from("max_tokens"), StopReason::MaxTokens);
        assert_eq!(StopReason::from("length"), StopReason::MaxTokens);
        assert_eq!(StopReason::from("stop_sequence"), StopReason::StopSequence);
        assert_eq!(StopReason::from("tool_use"), StopReason::ToolUse);
        assert_eq!(StopReason::from("tool_calls"), StopReason::ToolUse);
        assert_eq!(StopReason::from("function_call"), StopReason::ToolUse);
        assert_eq!(StopReason::from("TOOL_CALLS"), StopReason::ToolUse);
        assert_eq!(
            StopReason::from("unknown_reason"),
            StopReason::Other("unknown_reason".to_string())
        );
    }

    #[test]
    fn test_tool_call_reliability_serialization() {
        let reliable = ToolCallReliability::Reliable;
        let json = serde_json::to_string(&reliable).unwrap();
        assert_eq!(json, "\"reliable\"");

        let unreliable: ToolCallReliability = serde_json::from_str("\"unreliable\"").unwrap();
        assert_eq!(unreliable, ToolCallReliability::Unreliable);

        let none: ToolCallReliability = serde_json::from_str("\"none\"").unwrap();
        assert_eq!(none, ToolCallReliability::None);
    }

    #[test]
    fn test_provider_config_with_fallback_mode() {
        let config = ProviderConfig {
            fallback_tool_format_mode: Some(FallbackToolFormatMode::Soft),
            ..ProviderConfig::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("fallback_tool_format_mode"));

        let config_none = ProviderConfig::default();
        let json_none = serde_json::to_string(&config_none).unwrap();
        assert!(!json_none.contains("fallback_tool_format_mode"));
    }

    #[test]
    fn test_llm_response() {
        let response = LlmResponse {
            content: Some("Hello!".to_string()),
            thinking: None,
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
            usage: UsageStats::default(),
            model: "claude-3-5-sonnet".to_string(),
        };

        assert!(!response.has_tool_calls());
        assert!(response.is_final());
    }

    #[test]
    fn test_llm_error_display() {
        let err = LlmError::AuthenticationFailed {
            message: "Invalid API key".to_string(),
        };
        assert!(err.to_string().contains("Authentication failed"));

        let err = LlmError::RateLimited {
            message: "Too many requests".to_string(),
            retry_after: Some(60),
        };
        assert!(err.to_string().contains("Rate limited"));
    }
}
