//! WebSearch Tool Implementation
//!
//! Provides web search via pluggable providers (Tavily, Brave, DuckDuckGo).
//! Uses WebSearchService from ToolExecutionContext for actual searching.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// WebSearch tool — searches the web via configured providers.
///
/// Uses `ctx.web_search` (Option<Arc<WebSearchService>>) from the execution context.
/// Returns a helpful error when no search provider is configured.
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

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return ToolResult::err("Missing required parameter: query"),
        };

        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .min(10) as u32;

        match &ctx.web_search {
            Some(service) => match service.search(query, Some(max_results)).await {
                Ok(content) => ToolResult::ok(content),
                Err(e) => ToolResult::err(e),
            },
            None => ToolResult::err(
                "WebSearch is not configured. Set a search provider (tavily, brave, or duckduckgo) in Settings > LLM Backend > Search Provider, and provide an API key if required."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    fn make_ctx(dir: &Path) -> ToolExecutionContext {
        ToolExecutionContext {
            session_id: "test".to_string(),
            project_root: dir.to_path_buf(),
            working_directory: Arc::new(Mutex::new(dir.to_path_buf())),
            read_cache: Arc::new(Mutex::new(HashMap::new())),
            read_files: Arc::new(Mutex::new(HashSet::new())),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            web_fetch: Arc::new(crate::services::tools::web_fetch::WebFetchService::new()),
            web_search: None,
            index_store: None,
            embedding_service: None,
            embedding_manager: None,
            hnsw_index: None,
            task_dedup_cache: Arc::new(Mutex::new(HashMap::new())),
            task_context: None,
            core_context: None,
        }
    }

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

    #[tokio::test]
    async fn test_web_search_tool_no_provider() {
        let tool = WebSearchTool::new();
        let ctx = make_ctx(Path::new("/tmp"));
        let args = serde_json::json!({"query": "test query"});
        let result = tool.execute(&ctx, args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not configured"));
    }

    #[tokio::test]
    async fn test_web_search_tool_missing_query() {
        let tool = WebSearchTool::new();
        let ctx = make_ctx(Path::new("/tmp"));
        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("query"));
    }
}
