//! SearchKnowledge Tool
//!
//! Provides on-demand semantic search of the project knowledge base.
//! Instead of pre-injecting knowledge context into every system prompt,
//! this tool lets the AI query relevant documents when needed, saving
//! context tokens and improving result relevance.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::services::knowledge::context_provider::KnowledgeContextConfig;
use crate::services::knowledge::context_provider::KnowledgeContextProvider;
use crate::services::knowledge::pipeline::RagPipeline;
use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// Tool for on-demand semantic search of project knowledge collections.
///
/// Reads `knowledge_pipeline`, `knowledge_project_id`, and optional
/// filter fields from `ToolExecutionContext`. When the pipeline is not
/// configured, returns a helpful message instead of failing.
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

        let mut top_k_schema = ParameterSchema::integer(Some(
            "Number of results to return (default: 5, max: 20).",
        ));
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
            Some(p) => Arc::clone(p),
            None => {
                return ToolResult::ok(
                    "Knowledge base is not configured for this project. \
                     No collections are available to search.",
                );
            }
        };

        let project_id = match &ctx.knowledge_project_id {
            Some(id) => id.clone(),
            None => "default".to_string(),
        };

        // 2. Parse arguments
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.trim().is_empty() => q.trim(),
            _ => return ToolResult::err("Missing required parameter: query"),
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

        // 3. Execute search
        if let Some(ref specific_collection) = collection_filter {
            // Search a specific collection by name
            match pipeline
                .query(specific_collection, &project_id, query, top_k)
                .await
            {
                Ok(result) => {
                    if result.results.is_empty() {
                        return ToolResult::ok(format!(
                            "No relevant results found in collection '{}' for query: {}",
                            specific_collection, query,
                        ));
                    }
                    ToolResult::ok(format_search_results(
                        &result.results,
                        Some(specific_collection),
                    ))
                }
                Err(e) => ToolResult::ok(format!(
                    "Failed to search collection '{}': {}. \
                     Try omitting the collection parameter to search all collections.",
                    specific_collection, e,
                )),
            }
        } else {
            // Search across all collections (respecting user filters)
            let provider = KnowledgeContextProvider::new(Arc::clone(&pipeline));
            let config = KnowledgeContextConfig {
                enabled: true,
                max_context_chunks: top_k,
                minimum_relevance_score: 0.2,
                collection_ids: ctx.knowledge_collection_filter.clone(),
                document_ids: ctx.knowledge_document_filter.clone(),
            };

            match provider.query_for_context(&project_id, query, &config).await {
                Ok(chunks) => {
                    if chunks.is_empty() {
                        return ToolResult::ok(format!(
                            "No relevant results found in the knowledge base for query: {}",
                            query,
                        ));
                    }
                    ToolResult::ok(format_context_chunks(&chunks))
                }
                Err(e) => ToolResult::ok(format!(
                    "Knowledge base search failed: {}. \
                     The knowledge base may not be fully initialized.",
                    e,
                )),
            }
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

/// Format ContextChunk entries into a readable markdown block.
fn format_context_chunks(
    chunks: &[crate::services::knowledge::context_provider::ContextChunk],
) -> String {
    let mut output = format!("Found {} relevant knowledge chunks:\n\n", chunks.len());

    for (i, chunk) in chunks.iter().enumerate() {
        output.push_str(&format!(
            "### Result {} (relevance: {:.2}, source: {}, collection: {})\n\n",
            i + 1,
            chunk.relevance_score,
            chunk.source_document,
            chunk.collection_name,
        ));
        output.push_str(&chunk.content);
        output.push_str("\n\n---\n\n");
    }

    output
}
