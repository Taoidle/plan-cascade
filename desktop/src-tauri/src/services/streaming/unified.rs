//! Unified Stream Event Types
//!
//! Provider-agnostic event types that all adapters convert to.

use serde::{Deserialize, Serialize};

/// Unified streaming event that all provider adapters convert to.
/// This provides a consistent interface for the frontend regardless of LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnifiedStreamEvent {
    /// Text content delta from the model
    TextDelta {
        content: String,
    },

    /// Start of a thinking/reasoning block
    ThinkingStart {
        /// Optional thinking block ID for correlation
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_id: Option<String>,
    },

    /// Thinking content delta
    ThinkingDelta {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_id: Option<String>,
    },

    /// End of a thinking/reasoning block
    ThinkingEnd {
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_id: Option<String>,
    },

    /// Start of a tool call
    ToolStart {
        tool_id: String,
        tool_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        arguments: Option<String>,
    },

    /// Tool execution result
    ToolResult {
        tool_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// Token usage information
    Usage {
        input_tokens: u32,
        output_tokens: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_tokens: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_read_tokens: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_creation_tokens: Option<u32>,
    },

    /// Error during streaming
    Error {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },

    /// Stream complete
    Complete {
        #[serde(skip_serializing_if = "Option::is_none")]
        stop_reason: Option<String>,
    },

    // ========================================================================
    // Session-based execution events (for standalone mode)
    // ========================================================================

    /// Session progress update
    SessionProgress {
        session_id: String,
        progress: serde_json::Value,
    },

    /// Session execution complete
    SessionComplete {
        session_id: String,
        success: bool,
        completed_stories: usize,
        total_stories: usize,
    },

    /// Story execution started
    StoryStart {
        session_id: String,
        story_id: String,
        story_title: String,
        story_index: usize,
        total_stories: usize,
    },

    /// Story execution complete
    StoryComplete {
        session_id: String,
        story_id: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// Quality gates execution result
    QualityGatesResult {
        session_id: String,
        story_id: String,
        passed: bool,
        summary: serde_json::Value,
    },
}

/// Errors that can occur during stream adaptation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AdapterError {
    /// Invalid format that couldn't be parsed
    InvalidFormat(String),
    /// JSON/data parsing error
    ParseError(String),
    /// Event type not supported by this adapter
    UnsupportedEvent(String),
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
            AdapterError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            AdapterError::UnsupportedEvent(msg) => write!(f, "Unsupported event: {}", msg),
        }
    }
}

impl std::error::Error for AdapterError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_delta_serialization() {
        let event = UnifiedStreamEvent::TextDelta {
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"text_delta\""));
        assert!(json.contains("\"content\":\"Hello\""));

        let parsed: UnifiedStreamEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_thinking_events_serialization() {
        let start = UnifiedStreamEvent::ThinkingStart {
            thinking_id: Some("t1".to_string()),
        };
        let json = serde_json::to_string(&start).unwrap();
        assert!(json.contains("\"type\":\"thinking_start\""));

        let delta = UnifiedStreamEvent::ThinkingDelta {
            content: "reasoning...".to_string(),
            thinking_id: None,
        };
        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains("\"type\":\"thinking_delta\""));
        assert!(!json.contains("thinking_id")); // None should be skipped

        let end = UnifiedStreamEvent::ThinkingEnd { thinking_id: None };
        let json = serde_json::to_string(&end).unwrap();
        assert!(json.contains("\"type\":\"thinking_end\""));
    }

    #[test]
    fn test_tool_events_serialization() {
        let start = UnifiedStreamEvent::ToolStart {
            tool_id: "tool_1".to_string(),
            tool_name: "read_file".to_string(),
            arguments: Some("{\"path\": \"/foo\"}".to_string()),
        };
        let json = serde_json::to_string(&start).unwrap();
        assert!(json.contains("\"type\":\"tool_start\""));
        assert!(json.contains("\"tool_name\":\"read_file\""));

        let result = UnifiedStreamEvent::ToolResult {
            tool_id: "tool_1".to_string(),
            result: Some("file contents".to_string()),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"type\":\"tool_result\""));
    }

    #[test]
    fn test_usage_serialization() {
        let usage = UnifiedStreamEvent::Usage {
            input_tokens: 100,
            output_tokens: 50,
            thinking_tokens: Some(20),
            cache_read_tokens: None,
            cache_creation_tokens: None,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"type\":\"usage\""));
        assert!(json.contains("\"input_tokens\":100"));
        assert!(json.contains("\"thinking_tokens\":20"));
        assert!(!json.contains("cache_read_tokens")); // None skipped
    }

    #[test]
    fn test_error_serialization() {
        let error = UnifiedStreamEvent::Error {
            message: "Rate limit exceeded".to_string(),
            code: Some("rate_limit".to_string()),
        };
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"message\":\"Rate limit exceeded\""));
    }

    #[test]
    fn test_complete_serialization() {
        let complete = UnifiedStreamEvent::Complete {
            stop_reason: Some("end_turn".to_string()),
        };
        let json = serde_json::to_string(&complete).unwrap();
        assert!(json.contains("\"type\":\"complete\""));
        assert!(json.contains("\"stop_reason\":\"end_turn\""));
    }

    #[test]
    fn test_adapter_error_display() {
        let err = AdapterError::InvalidFormat("bad json".to_string());
        assert_eq!(err.to_string(), "Invalid format: bad json");

        let err = AdapterError::ParseError("unexpected token".to_string());
        assert_eq!(err.to_string(), "Parse error: unexpected token");

        let err = AdapterError::UnsupportedEvent("ping".to_string());
        assert_eq!(err.to_string(), "Unsupported event: ping");
    }
}
