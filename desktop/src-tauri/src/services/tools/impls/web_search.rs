//! WebSearch Tool Implementation
//!
//! Provides web search via pluggable providers (Tavily, Brave, DuckDuckGo).

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// WebSearch tool — searches the web via configured providers.
///
/// The actual search is handled by WebSearchService which is owned by ToolExecutor.
/// This trait implementation provides the tool definition. The ToolExecutor
/// intercepts "WebSearch" calls and delegates to its internal WebSearchService.
pub struct WebSearchTool;

impl WebSearchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "WebSearch"
    }

    fn description(&self) -> &str {
        "Search the web for current information. Returns titles, URLs, and snippets. Supports Tavily, Brave Search, and DuckDuckGo providers (configured in settings)."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "query".to_string(),
            ParameterSchema::string(Some("The search query")),
        );
        properties.insert(
            "max_results".to_string(),
            ParameterSchema::integer(Some("Maximum number of results (default: 5, max: 10)")),
        );
        ParameterSchema::object(
            Some("WebSearch parameters"),
            properties,
            vec!["query".to_string()],
        )
    }

    fn is_long_running(&self) -> bool {
        true
    }

    async fn execute(&self, _ctx: &ToolExecutionContext, _args: Value) -> ToolResult {
        // WebSearch execution is handled by ToolExecutor which owns the WebSearchService.
        ToolResult::err(
            "WebSearch is not configured. Set a search provider (tavily, brave, or duckduckgo) in Settings > LLM Backend > Search Provider, and provide an API key if required."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_search_tool_name() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "WebSearch");
    }

    #[test]
    fn test_web_search_tool_is_long_running() {
        let tool = WebSearchTool::new();
        assert!(tool.is_long_running());
    }
}
