//! WebFetch Tool Implementation
//!
//! Fetches web pages and converts them to markdown.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// WebFetch tool — fetches web pages and converts to markdown.
///
/// The actual fetching is handled by WebFetchService which is owned by ToolExecutor.
/// This trait implementation provides the tool definition. The ToolExecutor
/// intercepts "WebFetch" calls and delegates to its internal WebFetchService.
pub struct WebFetchTool;

impl WebFetchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "WebFetch"
    }

    fn description(&self) -> &str {
        "Fetch a web page and convert it to markdown. Supports HTML pages, documentation, and other web content. Private/local URLs are blocked for security. Results are cached for 15 minutes."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "url".to_string(),
            ParameterSchema::string(Some(
                "The URL to fetch content from. HTTP URLs are auto-upgraded to HTTPS.",
            )),
        );
        properties.insert(
            "prompt".to_string(),
            ParameterSchema::string(Some("Description of what to extract from the page (included as context, not processed locally)")),
        );
        properties.insert(
            "timeout".to_string(),
            ParameterSchema::integer(Some("Timeout in seconds (default: 30, max: 60)")),
        );
        ParameterSchema::object(
            Some("WebFetch parameters"),
            properties,
            vec!["url".to_string()],
        )
    }

    fn is_long_running(&self) -> bool {
        true
    }

    async fn execute(&self, _ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        // WebFetch execution is handled by ToolExecutor which owns the WebFetchService.
        // This fallback handles the case where the tool is called directly.
        let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("(no url)");
        ToolResult::err(format!(
            "WebFetch for '{}' requires WebFetchService which is not available in this context.",
            url
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_fetch_tool_name() {
        let tool = WebFetchTool::new();
        assert_eq!(tool.name(), "WebFetch");
    }

    #[test]
    fn test_web_fetch_tool_is_long_running() {
        let tool = WebFetchTool::new();
        assert!(tool.is_long_running());
    }
}
