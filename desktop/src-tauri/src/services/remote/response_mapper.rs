//! Response Mapper
//!
//! Converts streaming events and session responses into platform-friendly text.
//! Full implementation in story-004.

use super::types::{GatewayStatus, RemoteError, RemoteResponse, RemoteSessionMapping};

/// Response formatter for remote platform display.
pub struct ResponseMapper;

impl ResponseMapper {
    /// Format a session response for display.
    pub fn format_response(response: &RemoteResponse) -> String {
        let mut result = response.text.clone();
        if let Some(ref thinking) = response.thinking {
            let truncated = Self::truncate(thinking, 500);
            result = format!("{}\n\n[Thinking]: {}", result, truncated);
        }
        if let Some(ref tools) = response.tool_summary {
            result = format!("{}\n\nTools used:\n{}", result, tools);
        }
        result
    }

    /// Format an error for display.
    pub fn format_error(error: &RemoteError) -> String {
        format!("Error: {}", error)
    }

    /// Format session created message.
    pub fn format_session_created(session_id: &str, project_path: &str) -> String {
        format!(
            "Session created: {}\nProject: {}",
            session_id, project_path
        )
    }

    /// Format session list for display.
    pub fn format_session_list(mappings: &[RemoteSessionMapping]) -> String {
        if mappings.is_empty() {
            return "No active remote sessions.".to_string();
        }
        let mut text = "Active Remote Sessions:\n".to_string();
        for mapping in mappings {
            text.push_str(&format!(
                "  Chat {} -> {} ({})\n",
                mapping.chat_id,
                mapping
                    .local_session_id
                    .as_deref()
                    .unwrap_or("no session"),
                mapping.session_type
            ));
        }
        text
    }

    /// Format gateway status for display.
    pub fn format_status(mapping: &RemoteSessionMapping, gateway: &GatewayStatus) -> String {
        format!(
            "Session: {}\nType: {}\nGateway: {}\nCommands processed: {}",
            mapping
                .local_session_id
                .as_deref()
                .unwrap_or("none"),
            mapping.session_type,
            if gateway.running { "Running" } else { "Stopped" },
            gateway.total_commands_processed
        )
    }

    /// Smart truncation that adds ellipsis indicator.
    pub fn truncate(text: &str, max_len: usize) -> String {
        if text.len() <= max_len {
            return text.to_string();
        }
        if max_len <= 3 {
            return "...".to_string();
        }
        format!("{}...", &text[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::remote::types::{
        GatewayStatus, RemoteAdapterType, RemoteError, RemoteResponse, RemoteSessionMapping,
        SessionType,
    };

    #[test]
    fn test_format_response_text_only() {
        let response = RemoteResponse {
            text: "Hello world".to_string(),
            thinking: None,
            tool_summary: None,
        };
        let formatted = ResponseMapper::format_response(&response);
        assert_eq!(formatted, "Hello world");
    }

    #[test]
    fn test_format_response_with_thinking() {
        let response = RemoteResponse {
            text: "Answer".to_string(),
            thinking: Some("Let me think about this...".to_string()),
            tool_summary: None,
        };
        let formatted = ResponseMapper::format_response(&response);
        assert!(formatted.contains("Answer"));
        assert!(formatted.contains("[Thinking]"));
    }

    #[test]
    fn test_format_response_with_tools() {
        let response = RemoteResponse {
            text: "Result".to_string(),
            thinking: None,
            tool_summary: Some("[Grep]: found matches\n[Read]: read file".to_string()),
        };
        let formatted = ResponseMapper::format_response(&response);
        assert!(formatted.contains("Result"));
        assert!(formatted.contains("Tools used"));
    }

    #[test]
    fn test_format_error() {
        let error = RemoteError::NoActiveSession;
        let formatted = ResponseMapper::format_error(&error);
        assert!(formatted.contains("No active session"));
    }

    #[test]
    fn test_format_session_created() {
        let formatted =
            ResponseMapper::format_session_created("sess-123", "~/projects/myapp");
        assert!(formatted.contains("sess-123"));
        assert!(formatted.contains("~/projects/myapp"));
    }

    #[test]
    fn test_format_session_list_empty() {
        let formatted = ResponseMapper::format_session_list(&[]);
        assert!(formatted.contains("No active"));
    }

    #[test]
    fn test_format_session_list_with_entries() {
        let mappings = vec![RemoteSessionMapping {
            chat_id: 123,
            user_id: 456,
            local_session_id: Some("sess-abc".to_string()),
            session_type: SessionType::ClaudeCode,
            created_at: "2026-02-18T14:30:00Z".to_string(),
            adapter_type_name: Some("Telegram".to_string()),
            username: Some("testuser".to_string()),
        }];
        let formatted = ResponseMapper::format_session_list(&mappings);
        assert!(formatted.contains("123"));
        assert!(formatted.contains("sess-abc"));
    }

    #[test]
    fn test_format_status() {
        let mapping = RemoteSessionMapping {
            chat_id: 123,
            user_id: 456,
            local_session_id: Some("sess-abc".to_string()),
            session_type: SessionType::ClaudeCode,
            created_at: "2026-02-18T14:30:00Z".to_string(),
            adapter_type_name: None,
            username: None,
        };
        let gateway = GatewayStatus {
            running: true,
            total_commands_processed: 42,
            ..Default::default()
        };
        let formatted = ResponseMapper::format_status(&mapping, &gateway);
        assert!(formatted.contains("Running"));
        assert!(formatted.contains("42"));
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(ResponseMapper::truncate("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_long() {
        let text = "a".repeat(200);
        let truncated = ResponseMapper::truncate(&text, 50);
        assert_eq!(truncated.len(), 50);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_truncate_exact() {
        assert_eq!(ResponseMapper::truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_very_small_limit() {
        assert_eq!(ResponseMapper::truncate("hello world", 3), "...");
    }
}
