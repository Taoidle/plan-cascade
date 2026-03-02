//! CodebaseSearch Tool Implementation
//!
//! Uses the unified `CodebaseSearchService` so Tool and IPC share identical
//! filtering, ranking, pagination, and diagnostics semantics.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::orchestrator::codebase_search_service::{
    CodeSearchResponse, CodebaseSearchFilters, CodebaseSearchRequest, CodebaseSearchService,
    SearchHit,
};
use crate::services::orchestrator::index_store::IndexStore;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

pub struct CodebaseSearchTool;

impl CodebaseSearchTool {
    pub fn new() -> Self {
        Self
    }

    fn normalized_arg(value: Option<&Value>) -> Option<String> {
        value
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    fn validate_scope(scope_input: &str) -> Result<&'static str, String> {
        let normalized = scope_input.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "hybrid" => Ok("hybrid"),
            "symbol" => Ok("symbol"),
            "path" => Ok("path"),
            "semantic" => Ok("semantic"),
            _ => Err(format!(
                "Invalid scope '{}'. Use one of: hybrid, symbol, path, semantic.",
                scope_input
            )),
        }
    }

    fn scope_to_modes(scope: &str) -> Vec<String> {
        vec![scope.to_string()]
    }

    fn render_diagnostics(output: &mut String, response: &CodeSearchResponse) {
        if let Some(diag) = response.diagnostics.as_ref() {
            if let Some(ref provider) = diag.provider_display {
                let dim = if diag.embedding_dimension > 0 {
                    format!(", {}-dim", diag.embedding_dimension)
                } else {
                    String::new()
                };
                output.push_str(&format!("> Provider: {}{}\n", provider, dim));
            }

            if diag.semantic_degraded {
                let reason = diag.semantic_error.as_deref().unwrap_or("unknown");
                output.push_str(&format!("> Semantic: degraded ({})\n", reason));
            } else if diag.hnsw_used {
                output.push_str(&format!(
                    "> Semantic: active (HNSW, {} vectors)\n",
                    diag.hnsw_vector_count
                ));
            } else {
                output.push_str("> Semantic: not configured\n");
            }

            if !diag.active_channels.is_empty() {
                output.push_str(&format!(
                    "> Channels: {}\n",
                    diag.active_channels.join(", ")
                ));
            }
            output.push('\n');
        }
    }

    fn render_hit(hit: &SearchHit, include_snippet: bool) -> String {
        let mut line = format!("  {} (score: {:.4})", hit.file_path, hit.score);
        if !hit.channels.is_empty() {
            line.push_str(&format!(" [{}]", hit.channels.join(", ")));
        }
        line.push('\n');

        if let Some(ref symbol) = hit.symbol_name {
            if let Some(start) = hit.line_start {
                if let Some(end) = hit.line_end {
                    line.push_str(&format!("    symbol: {} ({}-{})\n", symbol, start, end));
                } else {
                    line.push_str(&format!("    symbol: {} ({})\n", symbol, start));
                }
            } else {
                line.push_str(&format!("    symbol: {}\n", symbol));
            }
        }

        if let Some(ref component) = hit.component {
            line.push_str(&format!("    component: {}\n", component));
        }
        if let Some(ref language) = hit.language {
            line.push_str(&format!("    language: {}\n", language));
        }
        if let Some(sim) = hit.similarity {
            line.push_str(&format!("    similarity: {:.3}\n", sim));
        }
        if include_snippet {
            if let Some(ref snippet) = hit.snippet {
                let compact = snippet.replace('\n', " ");
                let display = if compact.len() > 200 {
                    let mut end = 200;
                    while end > 0 && !compact.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}...", &compact[..end])
                } else {
                    compact
                };
                line.push_str(&format!("    snippet: {}\n", display));
            }
        }
        line
    }

    fn no_results_hint(query: &str, scope: &str, project_path: &str, store: &IndexStore) -> String {
        let mut hint = format!(
            "No results found for '{}' (scope: {}).\n\
             Hint: Try a shorter keyword. Use scope=\"path\" for files, scope=\"symbol\" for declarations, \
             or scope=\"hybrid\" for fused ranking.",
            query, scope
        );

        if let Ok(summary) = store.get_project_summary(project_path) {
            if !summary.components.is_empty() {
                let names: Vec<&str> = summary
                    .components
                    .iter()
                    .take(15)
                    .map(|c| c.name.as_str())
                    .collect();
                hint.push_str(&format!("\nAvailable components: {}", names.join(", ")));
            }
        }

        hint
    }

    fn render_response(
        query: &str,
        scope: &str,
        include_snippet: bool,
        response: &CodeSearchResponse,
    ) -> String {
        let mut output = String::new();
        Self::render_diagnostics(&mut output, response);
        output.push_str(&format!(
            "## {} search for '{}' ({} of {} results)\n",
            scope,
            query,
            response.hits.len(),
            response.total
        ));
        for hit in &response.hits {
            output.push_str(&Self::render_hit(hit, include_snippet));
        }
        if response.total > response.hits.len() {
            output.push_str(&format!(
                "  ... and {} more\n",
                response.total - response.hits.len()
            ));
        }
        output
    }
}

#[async_trait]
impl Tool for CodebaseSearchTool {
    fn name(&self) -> &str {
        "CodebaseSearch"
    }

    fn description(&self) -> &str {
        "Search the indexed codebase with V2 scopes and filters. Supports 'hybrid', 'symbol', 'path', and 'semantic' scopes with optional project/workspace targeting, result limits, and structured filters."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "query".to_string(),
            ParameterSchema::string(Some(
                "Search query — a symbol name, file path fragment, or keyword. \
                 Best with short queries (1-2 keywords, e.g. 'auth' not 'authentication service handler'). \
                 Make separate calls for different concepts.",
            )),
        );

        let mut scope_schema = ParameterSchema::string(Some(
            "Search scope: 'hybrid' (RRF merged channels), 'symbol', 'path', or 'semantic'. Default: 'hybrid'.",
        ));
        scope_schema.enum_values = Some(vec![
            "hybrid".to_string(),
            "symbol".to_string(),
            "path".to_string(),
            "semantic".to_string(),
        ]);
        scope_schema.default = Some(serde_json::Value::String("hybrid".to_string()));
        properties.insert("scope".to_string(), scope_schema);

        properties.insert(
            "project_path".to_string(),
            ParameterSchema::string(Some(
                "Optional project path override. Defaults to current project root in tool context.",
            )),
        );
        properties.insert(
            "workspace_root_id".to_string(),
            ParameterSchema::string(Some(
                "Optional workspace root identifier for multi-root routing (reserved for workspace-aware dispatch).",
            )),
        );

        let mut limit_schema = ParameterSchema::integer(Some(
            "Maximum number of results to return (1-100). Default: 20.",
        ));
        limit_schema.default = Some(serde_json::json!(20));
        properties.insert("limit".to_string(), limit_schema);

        let mut include_snippet_schema =
            ParameterSchema::boolean(Some("Whether to include snippet/chunk text in results."));
        include_snippet_schema.default = Some(serde_json::json!(true));
        properties.insert("include_snippet".to_string(), include_snippet_schema);

        let mut filter_props = HashMap::new();
        filter_props.insert(
            "component".to_string(),
            ParameterSchema::string(Some("Filter by component name (exact component mapping).")),
        );
        filter_props.insert(
            "language".to_string(),
            ParameterSchema::string(Some("Filter by file language (e.g. 'rust', 'typescript').")),
        );
        filter_props.insert(
            "file_path_prefix".to_string(),
            ParameterSchema::string(Some("Filter by file path prefix.")),
        );
        properties.insert(
            "filters".to_string(),
            ParameterSchema::object(Some("Optional search filters."), filter_props, vec![]),
        );

        ParameterSchema::object(
            Some("CodebaseSearch parameters"),
            properties,
            vec!["query".to_string()],
        )
    }

    fn is_parallel_safe(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => {
                return ToolResult::err("Missing required parameter: query")
                    .with_error_code("missing_query");
            }
        };

        let scope_input = args
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("hybrid");
        let scope = match Self::validate_scope(scope_input) {
            Ok(scope) => scope,
            Err(error) => return ToolResult::err(error).with_error_code("invalid_scope"),
        };

        let filters = args.get("filters").and_then(|v| v.as_object());
        let component = Self::normalized_arg(filters.and_then(|f| f.get("component")));
        let language = Self::normalized_arg(filters.and_then(|f| f.get("language")));
        let file_path_prefix =
            Self::normalized_arg(filters.and_then(|f| f.get("file_path_prefix")));

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(20)
            .clamp(1, 100);
        let include_snippet = args
            .get("include_snippet")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let index_store = match &ctx.index_store {
            Some(store) => store,
            None => {
                return ToolResult::err("Codebase index not available. The project has not been indexed yet. Use Grep for content search or Glob/LS for file discovery instead.")
                    .with_error_code("codebase_index_unavailable");
            }
        };

        let project_path = Self::normalized_arg(args.get("project_path"))
            .unwrap_or_else(|| ctx.project_root.to_string_lossy().to_string());
        if let Some(workspace_root_id) = Self::normalized_arg(args.get("workspace_root_id")) {
            tracing::debug!(
                "CodebaseSearch workspace_root_id provided but not yet used in tool routing: {}",
                workspace_root_id
            );
        }

        let request = CodebaseSearchRequest {
            project_path: project_path.clone(),
            query: query.to_string(),
            modes: Self::scope_to_modes(scope),
            limit: Some(limit),
            offset: Some(0),
            include_snippet: Some(include_snippet),
            filters: Some(CodebaseSearchFilters {
                component,
                language,
                file_path_prefix,
            }),
        };

        let service = CodebaseSearchService::new(
            index_store.clone(),
            ctx.embedding_manager.clone(),
            ctx.hnsw_index.clone(),
        );

        let response = match service.search(request).await {
            Ok(response) => response,
            Err(error) => return ToolResult::err(error).with_error_code("search_failed"),
        };

        if response.hits.is_empty() {
            return ToolResult::ok(Self::no_results_hint(
                query,
                scope,
                &project_path,
                index_store,
            ));
        }

        ToolResult::ok(Self::render_response(
            query,
            scope,
            include_snippet,
            &response,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_test_ctx;
    use super::*;
    use std::path::Path;

    #[test]
    fn test_codebase_search_tool_name() {
        let tool = CodebaseSearchTool::new();
        assert_eq!(tool.name(), "CodebaseSearch");
    }

    #[test]
    fn test_codebase_search_tool_schema() {
        let tool = CodebaseSearchTool::new();
        let schema = tool.parameters_schema();
        let props = schema.properties.as_ref().unwrap();
        assert!(props.contains_key("query"));
        assert!(props.contains_key("scope"));
        assert!(props.contains_key("project_path"));
        assert!(props.contains_key("workspace_root_id"));
        assert!(props.contains_key("limit"));
        assert!(props.contains_key("include_snippet"));
        assert!(props.contains_key("filters"));

        let scope = props.get("scope").unwrap();
        let enum_vals = scope.enum_values.as_ref().unwrap();
        assert!(enum_vals.contains(&"hybrid".to_string()));
        assert!(enum_vals.contains(&"symbol".to_string()));
        assert!(enum_vals.contains(&"path".to_string()));
        assert!(enum_vals.contains(&"semantic".to_string()));
        let default_val = scope.default.as_ref().unwrap();
        assert_eq!(
            default_val,
            &serde_json::Value::String("hybrid".to_string())
        );
    }

    #[test]
    fn test_codebase_search_tool_not_long_running() {
        let tool = CodebaseSearchTool::new();
        assert!(!tool.is_long_running());
    }

    #[tokio::test]
    async fn test_codebase_search_no_index() {
        let tool = CodebaseSearchTool::new();
        let ctx = make_test_ctx(Path::new("/tmp"));
        let args = serde_json::json!({"query": "test_function"});
        let result = tool.execute(&ctx, args).await;
        assert!(result.is_error());
        assert!(result.error_message().unwrap().contains("not been indexed"));
    }

    #[tokio::test]
    async fn test_codebase_search_missing_query() {
        let tool = CodebaseSearchTool::new();
        let ctx = make_test_ctx(Path::new("/tmp"));
        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(result.is_error());
        assert!(result.error_message().unwrap().contains("query"));
    }

    #[tokio::test]
    async fn test_codebase_search_rejects_legacy_scope_aliases() {
        let tool = CodebaseSearchTool::new();
        let ctx = make_test_ctx(Path::new("/tmp"));
        let result = tool
            .execute(
                &ctx,
                serde_json::json!({
                    "query": "App",
                    "scope": "all"
                }),
            )
            .await;
        assert!(result.is_error());
        assert!(result.error_message().unwrap().contains("Invalid scope"));
    }
}
