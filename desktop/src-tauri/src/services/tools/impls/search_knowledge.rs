//! SearchKnowledge Tool
//!
//! Provides on-demand semantic search of the project knowledge base.
//! Instead of pre-injecting knowledge context into every system prompt,
//! this tool lets the AI query relevant documents when needed, saving
//! context tokens and improving result relevance.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// Tool for on-demand semantic search of project knowledge collections.
///
/// Reads `knowledge_pipeline`, `knowledge_project_id`, and optional
/// filter fields from `ToolExecutionContext`.
pub struct SearchKnowledgeTool;

impl SearchKnowledgeTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for SearchKnowledgeTool {
    fn name(&self) -> &str {
        "SearchKnowledge"
    }

    fn description(&self) -> &str {
        "Search the project knowledge base for relevant documents using semantic search. \
         This is the HIGHEST PRIORITY search tool when the knowledge base is enabled. \
         ALWAYS use this tool FIRST before CodebaseSearch or Grep when looking for \
         documentation, specifications, design decisions, API references, standards, \
         or any domain-specific knowledge."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();

        properties.insert(
            "query".to_string(),
            ParameterSchema::string(Some(
                "Search query describing the information you need. \
                 Be specific for better semantic matching.",
            )),
        );

        properties.insert(
            "collection".to_string(),
            ParameterSchema::string(Some(
                "Optional: limit search to a specific collection name. \
                 Omit to search all available collections.",
            )),
        );

        let mut top_k_schema =
            ParameterSchema::integer(Some("Number of results to return (default: 5, max: 20)."));
        top_k_schema.default = Some(Value::Number(serde_json::Number::from(5)));
        properties.insert("top_k".to_string(), top_k_schema);

        ParameterSchema::object(
            Some("SearchKnowledge parameters"),
            properties,
            vec!["query".to_string()],
        )
    }

    fn is_parallel_safe(&self) -> bool {
        true // read-only operation
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        // 1. Get pipeline from context
        let pipeline = match &ctx.knowledge_pipeline {
            Some(p) => p.clone(),
            None => {
                return ToolResult::err(
                    "Knowledge base is not configured for this project. No collections are available to search.",
                )
                .with_error_code("knowledge_not_configured");
            }
        };

        let project_id = match &ctx.knowledge_project_id {
            Some(id) => id.clone(),
            None => "default".to_string(),
        };

        // 2. Parse arguments
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.trim().is_empty() => q.trim(),
            _ => {
                return ToolResult::err("Missing required parameter: query")
                    .with_error_code("missing_query");
            }
        };

        let collection_filter = args
            .get("collection")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let top_k = args
            .get("top_k")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(5)
            .min(20);

        // 3. Build effective scoped collection filter
        let mut effective_collection_ids = ctx.knowledge_collection_filter.clone();
        let mut selected_collection_name: Option<String> = None;

        if let Some(ref specific_collection) = collection_filter {
            let all_collections = match pipeline.list_collections(&project_id) {
                Ok(cols) => cols,
                Err(e) => {
                    return ToolResult::err(format!(
                        "Failed to load knowledge collections: {}",
                        e
                    ))
                    .with_error_code("knowledge_collections_load_failed")
                    .with_retryable(true);
                }
            };
            let selected = all_collections
                .iter()
                .find(|c| c.name == *specific_collection)
                .cloned();
            let Some(selected) = selected else {
                return ToolResult::ok(format!(
                    "Collection '{}' was not found in this project.",
                    specific_collection
                ));
            };

            selected_collection_name = Some(selected.name.clone());
            effective_collection_ids = match effective_collection_ids {
                Some(existing) => {
                    if existing.iter().any(|id| id == &selected.id) {
                        Some(vec![selected.id])
                    } else {
                        return ToolResult::ok(format!(
                            "Collection '{}' is outside the current knowledge filter scope.",
                            specific_collection
                        ));
                    }
                }
                None => Some(vec![selected.id]),
            };
        }

        // 4. Execute scoped query with unified retrieval pipeline
        match pipeline
            .query_scoped(
                &project_id,
                query,
                top_k,
                effective_collection_ids.as_deref(),
                ctx.knowledge_document_filter.as_deref(),
                None,
            )
            .await
        {
            Ok(result) => {
                if result.results.is_empty() {
                    return ToolResult::ok(format!(
                        "No relevant results found in the knowledge base for query: {}",
                        query,
                    ));
                }
                ToolResult::ok(format_search_results(
                    &result.results,
                    selected_collection_name.as_deref(),
                ))
            }
            Err(e) => ToolResult::err(format!(
                "Knowledge base search failed: {}. The knowledge base may not be fully initialized.",
                e,
            ))
            .with_error_code("knowledge_search_failed")
            .with_retryable(true),
        }
    }
}

/// Format raw SearchResult entries into a readable markdown block.
fn format_search_results(
    results: &[crate::services::knowledge::reranker::SearchResult],
    collection_name: Option<&str>,
) -> String {
    let mut output = String::new();

    if let Some(name) = collection_name {
        output.push_str(&format!(
            "Found {} results in collection '{}':\n\n",
            results.len(),
            name,
        ));
    } else {
        output.push_str(&format!("Found {} results:\n\n", results.len()));
    }

    for (i, result) in results.iter().enumerate() {
        output.push_str(&format!(
            "### Result {} (relevance: {:.2}, source: {}, collection: {})\n\n",
            i + 1,
            result.score,
            result.document_id,
            result.collection_name,
        ));
        output.push_str(&result.chunk_text);
        output.push_str("\n\n---\n\n");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_test_ctx;
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn test_search_knowledge_without_pipeline_is_error() {
        let tool = SearchKnowledgeTool::new();
        let ctx = make_test_ctx(Path::new("/tmp"));
        let result = tool
            .execute(&ctx, serde_json::json!({ "query": "architecture" }))
            .await;
        assert!(result.is_error());
        assert!(result.error_message().unwrap().contains("not configured"));
    }

    #[tokio::test]
    async fn test_search_knowledge_missing_query_is_error() {
        let tool = SearchKnowledgeTool::new();
        let ctx = make_test_ctx(Path::new("/tmp"));
        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(result.is_error());
        assert!(result.error_message().unwrap().contains("query"));
    }
}
