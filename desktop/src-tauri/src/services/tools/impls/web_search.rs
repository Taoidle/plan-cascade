//! WebSearch Tool Implementation
//!
//! Provides web search via pluggable providers (Tavily, Brave, DuckDuckGo).
//! Uses WebSearchService from ToolExecutionContext for actual searching.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::{ToolCitation, ToolResult};
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};
use crate::services::tools::web_search::SearchConstraints;

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
        "Search the web for current information. Returns titles, URLs, snippets, and structured citations. Supports optional domain allow/block filters and provider backends (Tavily, Brave Search, DuckDuckGo, SearXNG)."
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
        properties.insert(
            "allow_domains".to_string(),
            ParameterSchema::array(
                Some("Optional domain allowlist. Only these domains/subdomains are kept."),
                ParameterSchema::string(Some("Domain (e.g., docs.rs or rust-lang.org)")),
            ),
        );
        properties.insert(
            "block_domains".to_string(),
            ParameterSchema::array(
                Some("Optional domain blocklist. Matching domains/subdomains are excluded."),
                ParameterSchema::string(Some("Domain to block (e.g., reddit.com)")),
            ),
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

    fn is_parallel_safe(&self) -> bool {
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

        fn parse_domains(args: &Value, key: &str) -> Vec<String> {
            args.get(key)
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }

        let allow_domains = parse_domains(&args, "allow_domains");
        let block_domains = parse_domains(&args, "block_domains");
        let constraints = SearchConstraints::new(
            if allow_domains.is_empty() {
                None
            } else {
                Some(allow_domains.clone())
            },
            block_domains.clone(),
        );

        match &ctx.web_search {
            Some(service) => match service
                .search_with_constraints(query, Some(max_results), &constraints)
                .await
            {
                Ok(results) => {
                    let content = service.format_results_markdown(query, &results);
                    let citations = results
                        .iter()
                        .map(|r| {
                            let mut c = ToolCitation::new(r.url.clone());
                            c.title = if r.title.is_empty() {
                                None
                            } else {
                                Some(r.title.clone())
                            };
                            c.snippet = if r.snippet.is_empty() {
                                None
                            } else {
                                Some(r.snippet.clone())
                            };
                            c.source = Some(service.provider_name().to_string());
                            c
                        })
                        .collect::<Vec<_>>();
                    let metadata = serde_json::json!({
                        "provider": service.provider_name(),
                        "max_results": max_results,
                        "allow_domains": allow_domains,
                        "block_domains": block_domains,
                        "result_count": citations.len(),
                    });
                    ToolResult::ok(content)
                        .with_metadata(metadata)
                        .with_citations(citations)
                }
                Err(e) => ToolResult::err(e)
                    .with_error_code("web_search_failed")
                    .with_retryable(true),
            },
            None => ToolResult::err(
                "WebSearch is not configured. Set a search provider (tavily, brave, duckduckgo, or searxng) in Settings > LLM Backend > Search Provider, and provide an API key if required."
            )
            .with_error_code("web_search_not_configured"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_test_ctx;
    use super::*;
    use std::path::Path;

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
        let ctx = make_test_ctx(Path::new("/tmp"));
        let args = serde_json::json!({"query": "test query"});
        let result = tool.execute(&ctx, args).await;
        assert!(result.is_error());
        assert!(result
            .error_message_owned()
            .unwrap()
            .contains("not configured"));
    }

    #[tokio::test]
    async fn test_web_search_tool_missing_query() {
        let tool = WebSearchTool::new();
        let ctx = make_test_ctx(Path::new("/tmp"));
        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(result.is_error());
        assert!(result.error_message_owned().unwrap().contains("query"));
    }
}
