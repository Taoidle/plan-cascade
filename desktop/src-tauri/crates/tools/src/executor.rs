//! Tool Executor Core Types
//!
//! Portable types for tool execution results and file read deduplication.
//! These types are independent of the full tool executor implementation
//! (which lives in the main crate).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

use plan_cascade_core::event_actions::EventActions;

/// Cache entry for a previously read file, used for deduplication.
///
/// ADR-F001: Uses `Mutex<HashMap>` over `mini-moka` for deterministic behavior,
/// low cardinality (<100 files), and zero additional dependencies.
#[derive(Debug, Clone)]
pub struct ReadCacheEntry {
    /// Canonical path of the cached file
    pub path: PathBuf,
    /// File modification time at the time of caching
    pub modified_time: SystemTime,
    /// Number of lines in the file
    pub line_count: usize,
    /// Size of the file in bytes
    pub size_bytes: u64,
    /// Hash of the file content (using std DefaultHasher for speed, not crypto)
    pub content_hash: u64,
    /// Offset (1-based line number) used when the file was read
    pub offset: usize,
    /// Line limit used when the file was read
    pub limit: usize,
    /// File extension (e.g. "rs", "py", "ts") for the enhanced dedup message
    pub extension: String,
    /// First ~5 lines of the file content for the enhanced dedup message
    pub first_lines_preview: String,
}

/// Result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the execution was successful
    pub success: bool,
    /// Output from the tool (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Optional image data for multimodal responses: (mime_type, base64_data)
    #[serde(skip)]
    pub image_data: Option<(String, String)>,
    /// Whether this result is a dedup hit (file read cache).
    /// When true, the agentic loop should push a minimal tool_result to the LLM
    /// instead of the full content, to prevent weak models from re-reading files.
    #[serde(default)]
    pub is_dedup: bool,
    /// Optional EventActions declared by the tool alongside its result.
    ///
    /// When present, the orchestrator's agentic loop processes these actions
    /// after handling the tool result. This enables tools to declare side
    /// effects (state mutations, checkpoints, quality gate results, transfers)
    /// without directly causing them.
    #[serde(skip)]
    pub event_actions: Option<EventActions>,
}

impl ToolResult {
    /// Create a successful result
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            image_data: None,
            is_dedup: false,
            event_actions: None,
        }
    }

    /// Create an error result
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error.into()),
            image_data: None,
            is_dedup: false,
            event_actions: None,
        }
    }

    /// Create a successful result with image data for multimodal support
    pub fn ok_with_image(
        output: impl Into<String>,
        mime_type: String,
        base64_data: String,
    ) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            image_data: Some((mime_type, base64_data)),
            is_dedup: false,
            event_actions: None,
        }
    }

    /// Create a successful dedup result (file read cache hit).
    ///
    /// Marked with `is_dedup = true` so the agentic loop can suppress
    /// the full content from the LLM conversation.
    pub fn ok_dedup(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            image_data: None,
            is_dedup: true,
            event_actions: None,
        }
    }

    /// Attach EventActions to this tool result.
    ///
    /// The orchestrator will process these actions after handling the tool result.
    pub fn with_event_actions(mut self, actions: EventActions) -> Self {
        if actions.has_actions() {
            self.event_actions = Some(actions);
        }
        self
    }

    /// Convert to string for LLM consumption
    pub fn to_content(&self) -> String {
        if self.success {
            self.output.clone().unwrap_or_default()
        } else {
            format!(
                "Error: {}",
                self.error.as_deref().unwrap_or("Unknown error")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result_ok() {
        let result = ToolResult::ok("hello");
        assert!(result.success);
        assert_eq!(result.output.as_deref(), Some("hello"));
        assert!(result.error.is_none());
        assert!(!result.is_dedup);
        assert!(result.event_actions.is_none());
    }

    #[test]
    fn test_tool_result_err() {
        let result = ToolResult::err("something failed");
        assert!(!result.success);
        assert!(result.output.is_none());
        assert_eq!(result.error.as_deref(), Some("something failed"));
    }

    #[test]
    fn test_tool_result_ok_with_image() {
        let result =
            ToolResult::ok_with_image("desc", "image/png".to_string(), "base64data".to_string());
        assert!(result.success);
        assert!(result.image_data.is_some());
        let (mime, data) = result.image_data.unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(data, "base64data");
    }

    #[test]
    fn test_tool_result_ok_dedup() {
        let result = ToolResult::ok_dedup("cached");
        assert!(result.success);
        assert!(result.is_dedup);
    }

    #[test]
    fn test_tool_result_with_event_actions() {
        let actions = EventActions::none().with_checkpoint("cp1");
        let result = ToolResult::ok("done").with_event_actions(actions);
        assert!(result.event_actions.is_some());
    }

    #[test]
    fn test_tool_result_with_empty_event_actions() {
        let actions = EventActions::none();
        let result = ToolResult::ok("done").with_event_actions(actions);
        // Empty actions should not be attached
        assert!(result.event_actions.is_none());
    }

    #[test]
    fn test_tool_result_to_content_success() {
        let result = ToolResult::ok("output text");
        assert_eq!(result.to_content(), "output text");
    }

    #[test]
    fn test_tool_result_to_content_error() {
        let result = ToolResult::err("bad thing");
        assert_eq!(result.to_content(), "Error: bad thing");
    }

    #[test]
    fn test_read_cache_entry_creation() {
        let entry = ReadCacheEntry {
            path: PathBuf::from("/tmp/test.rs"),
            modified_time: SystemTime::now(),
            line_count: 42,
            size_bytes: 1024,
            content_hash: 12345,
            offset: 0,
            limit: 0,
            extension: "rs".to_string(),
            first_lines_preview: "fn main() {".to_string(),
        };
        assert_eq!(entry.line_count, 42);
        assert_eq!(entry.extension, "rs");
    }
}
