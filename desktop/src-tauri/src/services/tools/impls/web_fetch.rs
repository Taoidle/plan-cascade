//! WebFetch Tool Implementation
//!
//! Fetches web pages and converts them to markdown.
//! Uses WebFetchService from ToolExecutionContext for actual fetching.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// WebFetch tool — fetches web pages and converts to markdown.
///
/// Uses `ctx.web_fetch` (Arc<WebFetchService>) from the execution context.
/// The service is always available since WebFetchService is created unconditionally.
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

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => return ToolResult::err("Missing required parameter: url"),
        };

        let prompt = args.get("prompt").and_then(|v| v.as_str());

        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30)
            .min(60);

        match ctx.web_fetch.fetch(url, Some(timeout_secs)).await {
            Ok(content) => {
                let mut output = String::new();
                if let Some(p) = prompt {
                    output.push_str(&format!("## Fetched: {}\n### Context: {}\n\n", url, p));
                } else {
                    output.push_str(&format!("## Fetched: {}\n\n", url));
                }
                output.push_str(&content);
                ToolResult::ok(output)
            }
            Err(e) => ToolResult::err(e),
        }
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

    #[test]
    fn test_web_fetch_tool_schema() {
        let tool = WebFetchTool::new();
        let schema = tool.parameters_schema();
        let props = schema.properties.as_ref().unwrap();
        assert!(props.contains_key("url"));
        assert!(props.contains_key("prompt"));
        assert!(props.contains_key("timeout"));
        let required = schema.required.as_ref().unwrap();
        assert!(required.contains(&"url".to_string()));
    }
}
