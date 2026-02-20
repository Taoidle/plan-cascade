//! CodebaseSearch Tool Implementation
//!
//! Searches the project's indexed codebase for symbols, files, or semantic similarity.
//! Uses IndexStore, EmbeddingManager, EmbeddingService, and HnswIndex from
//! ToolExecutionContext for all index queries.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::services::llm::types::ParameterSchema;
use crate::services::orchestrator::embedding_service::SemanticSearchResult;
use crate::services::orchestrator::hybrid_search::{HybridSearchEngine, HybridSearchResult};
use crate::services::orchestrator::index_store::IndexStore;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};
use crate::utils::error::AppError;

/// CodebaseSearch tool -- searches the project index for symbols, files, and semantic matches.
///
/// Uses `ctx.index_store`, `ctx.embedding_manager`, `ctx.embedding_service`,
/// and `ctx.hnsw_index` from the ToolExecutionContext. When `scope="all"`,
/// uses `HybridSearchEngine` for RRF-fused multi-channel search
/// (FTS5 BM25 + semantic HNSW). Falls back to manual channel logic on error.
pub struct CodebaseSearchTool;

impl CodebaseSearchTool {
    pub fn new() -> Self {
        Self
    }

    /// Execute hybrid search using HybridSearchEngine with RRF fusion.
    async fn execute_hybrid_search(
        ctx: &ToolExecutionContext,
        query: &str,
        project_path: &str,
        component: Option<&str>,
        index_store: &Arc<IndexStore>,
    ) -> Result<String, String> {
        let mut engine = HybridSearchEngine::with_defaults(
            Arc::clone(index_store),
            ctx.embedding_manager.clone(),
        );
        if let Some(ref hnsw) = ctx.hnsw_index {
            engine.set_hnsw_index(Arc::clone(hnsw));
        }

        let outcome = engine
            .search(query, project_path)
            .await
            .map_err(|e| format!("{}", e))?;

        let results = outcome.results;

        if results.is_empty() {
            return Ok(format!(
                "No results found for '{}' (scope: all, hybrid RRF).",
                query
            ));
        }

        // Apply component filter if specified
        let results: Vec<&HybridSearchResult> = if let Some(comp) = component {
            results.iter().filter(|r| r.file_path.contains(comp)).collect()
        } else {
            results.iter().collect()
        };

        if results.is_empty() {
            return Ok(format!(
                "No results found for '{}' in component '{}'.",
                query,
                component.unwrap_or("unknown")
            ));
        }

        // Prepend semantic degradation notice if applicable
        let mut output = String::new();
        if outcome.semantic_degraded {
            let reason = outcome.semantic_error.as_deref().unwrap_or("unknown error");
            output.push_str(&format!(
                "> Note: Semantic search unavailable ({}), using keyword search only.\n\n",
                reason
            ));
        }

        // Format results with provenance info
        output.push_str(&format!(
            "## Hybrid search for '{}' ({} results, RRF fusion)\n",
            query,
            results.len()
        ));
        for result in results.iter().take(30) {
            // Build provenance tag
            let channels: Vec<String> = result
                .provenance
                .iter()
                .map(|c| format!("{}#{}", c.channel, c.rank))
                .collect();
            let provenance_str = channels.join(", ");

            let mut line = format!(
                "  {} (score: {:.4}) [{}]\n",
                result.file_path, result.score, provenance_str
            );

            if let Some(ref sym) = result.symbol_name {
                line.push_str(&format!("    symbol: {}\n", sym));
            }
            if let Some(ref chunk) = result.chunk_text {
                let display_text = if chunk.len() > 200 {
                    let mut end = 200;
                    while end > 0 && !chunk.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}...", &chunk[..end])
                } else {
                    chunk.clone()
                };
                let display_text = display_text.replace('\n', " ");
                line.push_str(&format!("    snippet: {}\n", display_text));
            }
            if let Some(sim) = result.semantic_similarity {
                line.push_str(&format!("    similarity: {:.3}\n", sim));
            }
            output.push_str(&line);
        }
        if results.len() > 30 {
            output.push_str(&format!("  ... and {} more\n", results.len() - 30));
        }

        Ok(output)
    }

    /// Symbol search channel: prefers FTS5 BM25, falls back to SQL LIKE.
    fn search_symbols_channel(
        query: &str,
        component: Option<&str>,
        project_path: &str,
        scope: &str,
        index_store: &Arc<IndexStore>,
        output_sections: &mut Vec<String>,
    ) {
        // Try FTS5 BM25 first, fall back to LIKE
        let symbols_result = match index_store.fts_search_symbols(query, 50) {
            Ok(fts_results) if !fts_results.is_empty() => Ok(fts_results),
            _ => {
                // Fallback to LIKE
                let pattern = format!("%{}%", query);
                index_store.query_symbols(&pattern)
            }
        };

        match symbols_result {
            Ok(symbols) => {
                let filtered: Vec<_> = if let Some(comp) = component {
                    symbols
                        .into_iter()
                        .filter(|s| {
                            s.file_path.contains(comp) || s.project_path == *project_path
                        })
                        .collect()
                } else {
                    symbols
                };

                if !filtered.is_empty() {
                    let mut section = format!(
                        "## Symbols matching '{}' ({} results)\n",
                        query,
                        filtered.len()
                    );
                    for sym in filtered.iter().take(50) {
                        let mut line = format!(
                            "  {} ({}) — {}:{}",
                            sym.symbol_name, sym.symbol_kind, sym.file_path, sym.line_number
                        );
                        if sym.end_line > 0 && sym.end_line != sym.line_number {
                            line.push_str(&format!("-{}", sym.end_line));
                        }
                        if let Some(ref parent) = sym.parent_symbol {
                            line.push_str(&format!(" [in {}]", parent));
                        }
                        line.push('\n');
                        section.push_str(&line);
                        if let Some(ref sig) = sym.signature {
                            section.push_str(&format!("    sig: {}\n", sig));
                        }
                        if let Some(ref doc) = sym.doc_comment {
                            let truncated = if doc.len() > 100 {
                                let mut end = 100;
                                while end > 0 && !doc.is_char_boundary(end) {
                                    end -= 1;
                                }
                                format!("{}...", &doc[..end])
                            } else {
                                doc.clone()
                            };
                            section.push_str(&format!("    doc: {}\n", truncated));
                        }
                    }
                    if filtered.len() > 50 {
                        section.push_str(&format!("  ... and {} more\n", filtered.len() - 50));
                    }
                    output_sections.push(section);
                } else if scope == "symbols" {
                    output_sections.push(format!("No symbols matching '{}'.", query));
                }
            }
            Err(e) => {
                output_sections.push(format!("Symbol search error: {}", e));
            }
        }
    }

    /// File search channel: prefers FTS5 BM25, falls back to component/LIKE search.
    fn search_files_channel(
        query: &str,
        component: Option<&str>,
        project_path: &str,
        scope: &str,
        index_store: &Arc<IndexStore>,
        output_sections: &mut Vec<String>,
    ) {
        if let Some(comp) = component {
            // Search files by component with query filter
            match index_store.query_files_by_component(project_path, comp) {
                Ok(files) => {
                    let query_lower = query.to_lowercase();
                    let filtered: Vec<_> = files
                        .into_iter()
                        .filter(|f| f.file_path.to_lowercase().contains(&query_lower))
                        .collect();

                    if !filtered.is_empty() {
                        let mut section = format!(
                            "## Files matching '{}' in component '{}' ({} results)\n",
                            query, comp, filtered.len()
                        );
                        for file in filtered.iter().take(50) {
                            section.push_str(&format!(
                                "  {} ({}, {} lines)\n",
                                file.file_path, file.language, file.line_count
                            ));
                        }
                        if filtered.len() > 50 {
                            section
                                .push_str(&format!("  ... and {} more\n", filtered.len() - 50));
                        }
                        output_sections.push(section);
                    } else if scope == "files" {
                        output_sections.push(format!(
                            "No files matching '{}' in component '{}'.",
                            query, comp
                        ));
                    }
                }
                Err(e) => {
                    output_sections.push(format!("File search error: {}", e));
                }
            }
        } else {
            // No component filter -- try FTS5 first, then fall back to component scan
            match index_store.fts_search_files(query, project_path, 50) {
                Ok(fts_results) if !fts_results.is_empty() => {
                    let mut section = format!(
                        "## Files matching '{}' ({} results)\n",
                        query,
                        fts_results.len()
                    );
                    for file in fts_results.iter().take(50) {
                        section.push_str(&format!(
                            "  {} ({}, {} lines)\n",
                            file.file_path, file.language, file.line_count
                        ));
                    }
                    if fts_results.len() > 50 {
                        section.push_str(&format!(
                            "  ... and {} more\n",
                            fts_results.len() - 50
                        ));
                    }
                    output_sections.push(section);
                }
                _ => {
                    // Fallback: scan all components
                    match index_store.get_project_summary(project_path) {
                        Ok(summary) => {
                            let query_lower = query.to_lowercase();
                            let mut matching_files: Vec<String> = Vec::new();

                            for comp_summary in &summary.components {
                                if let Ok(files) = index_store
                                    .query_files_by_component(project_path, &comp_summary.name)
                                {
                                    for file in files {
                                        if file.file_path.to_lowercase().contains(&query_lower) {
                                            matching_files.push(format!(
                                                "  {} [{}] ({}, {} lines)",
                                                file.file_path,
                                                file.component,
                                                file.language,
                                                file.line_count
                                            ));
                                        }
                                    }
                                }
                            }

                            if !matching_files.is_empty() {
                                let count = matching_files.len();
                                let mut section =
                                    format!("## Files matching '{}' ({} results)\n", query, count);
                                for line in matching_files.iter().take(50) {
                                    section.push_str(line);
                                    section.push('\n');
                                }
                                if count > 50 {
                                    section
                                        .push_str(&format!("  ... and {} more\n", count - 50));
                                }
                                output_sections.push(section);
                            } else if scope == "files" {
                                output_sections
                                    .push(format!("No files matching '{}'.", query));
                            }
                        }
                        Err(e) => {
                            output_sections.push(format!("File search error: {}", e));
                        }
                    }
                }
            }
        }
    }

    /// Semantic search channel using EmbeddingManager or EmbeddingService.
    async fn search_semantic_channel(
        ctx: &ToolExecutionContext,
        query: &str,
        project_path: &str,
        scope: &str,
        index_store: &Arc<IndexStore>,
        output_sections: &mut Vec<String>,
    ) {
        let is_standalone_semantic = scope == "semantic";

        // Prefer EmbeddingManager (ADR-F002) over raw EmbeddingService.
        if let Some(ref emb_mgr) = ctx.embedding_manager {
            let stored_dim = index_store
                .get_embedding_metadata(project_path)
                .ok()
                .and_then(|meta| meta.first().map(|m| m.embedding_dimension));
            let manager_dim = emb_mgr.dimension();
            let dimension_compatible = stored_dim
                .map(|d| d == 0 || manager_dim == 0 || d == manager_dim)
                .unwrap_or(true);

            if !dimension_compatible {
                let msg = format!(
                    "Semantic search not available: embedding dimension mismatch. \
                     Index was built with {}-dimensional embeddings, but the current \
                     embedding provider produces {}-dimensional vectors. \
                     Re-index the project to resolve this.",
                    stored_dim.unwrap_or(0),
                    manager_dim,
                );
                if is_standalone_semantic {
                    output_sections.push(msg);
                } else {
                    output_sections.push(format!(
                        "Semantic search: dimension mismatch (stored={}, provider={})",
                        stored_dim.unwrap_or(0),
                        manager_dim
                    ));
                }
            } else {
                match emb_mgr.embed_query(query).await {
                    Ok(query_embedding) if !query_embedding.is_empty() => {
                        let search_result = if let Some(ref hnsw) = ctx.hnsw_index {
                            if hnsw.is_ready().await {
                                let hnsw_hits = hnsw.search(&query_embedding, 10).await;
                                if !hnsw_hits.is_empty() {
                                    let rowids: Vec<usize> =
                                        hnsw_hits.iter().map(|(id, _)| *id).collect();
                                    match index_store.get_embeddings_by_rowids(&rowids) {
                                        Ok(metadata) => {
                                            let results: Vec<SemanticSearchResult> = hnsw_hits
                                                .into_iter()
                                                .filter_map(|(id, distance)| {
                                                    metadata.get(&id).map(
                                                        |(file_path, chunk_index, chunk_text)| {
                                                            SemanticSearchResult {
                                                                file_path: file_path.clone(),
                                                                chunk_index: *chunk_index,
                                                                chunk_text: chunk_text.clone(),
                                                                similarity: 1.0 - distance,
                                                            }
                                                        },
                                                    )
                                                })
                                                .collect();
                                            Ok(results)
                                        }
                                        Err(e) => Err(e),
                                    }
                                } else {
                                    Ok(Vec::new())
                                }
                            } else {
                                index_store
                                    .semantic_search(&query_embedding, project_path, 10)
                            }
                        } else {
                            index_store.semantic_search(&query_embedding, project_path, 10)
                        };

                        Self::format_semantic_results(query, search_result, output_sections);
                    }
                    Ok(_) => {
                        output_sections.push(
                            "Semantic search: embedding provider produced empty vector. \
                             The vocabulary may not cover the query terms."
                                .to_string(),
                        );
                    }
                    Err(e) => {
                        let msg = format!(
                            "Semantic search failed: embedding provider error — {}. \
                             The provider may be unhealthy or unreachable. \
                             Use 'symbols' or 'files' scope instead.",
                            e
                        );
                        if is_standalone_semantic {
                            output_sections.push(msg);
                        } else {
                            output_sections
                                .push(format!("Semantic search: provider error ({})", e));
                        }
                    }
                }
            }
        } else if let Some(ref emb_svc) = ctx.embedding_service {
            // Legacy fallback: use raw EmbeddingService when no manager is set
            if emb_svc.is_ready() {
                let query_embedding = emb_svc.embed_text(query);
                if !query_embedding.is_empty() {
                    let search_result =
                        index_store.semantic_search(&query_embedding, project_path, 10);
                    Self::format_semantic_results(query, search_result, output_sections);
                } else {
                    output_sections.push(
                        "Semantic search: embedding service produced empty vector. \
                         The vocabulary may not cover the query terms."
                            .to_string(),
                    );
                }
            } else if is_standalone_semantic {
                output_sections.push(
                    "Semantic search not available: embedding vocabulary has not been built yet. \
                     The project needs to be re-indexed with embedding generation enabled. \
                     Use 'symbols' or 'files' scope instead."
                        .to_string(),
                );
            } else {
                output_sections.push(
                    "Semantic search: not available (vocabulary not built)".to_string(),
                );
            }
        } else {
            // Neither EmbeddingManager nor EmbeddingService configured
            if is_standalone_semantic {
                output_sections.push(
                    "Semantic search not available: no embedding provider configured. \
                     The project has not been indexed with embedding support. \
                     Use 'symbols' or 'files' scope instead."
                        .to_string(),
                );
            } else {
                output_sections.push("Semantic search: not configured".to_string());
            }
        }
    }

    /// Format semantic search results into output sections.
    fn format_semantic_results(
        query: &str,
        search_result: Result<Vec<SemanticSearchResult>, AppError>,
        output_sections: &mut Vec<String>,
    ) {
        match search_result {
            Ok(results) if !results.is_empty() => {
                let mut section = format!(
                    "## Semantic search for '{}' ({} results)\n",
                    query,
                    results.len()
                );
                for result in &results {
                    let display_text = if result.chunk_text.len() > 200 {
                        let mut end = 200;
                        while end > 0 && !result.chunk_text.is_char_boundary(end) {
                            end -= 1;
                        }
                        format!("{}...", &result.chunk_text[..end])
                    } else {
                        result.chunk_text.clone()
                    };
                    let display_text = display_text.replace('\n', " ");
                    section.push_str(&format!(
                        "  {} (chunk {}, similarity: {:.3})\n    {}\n",
                        result.file_path, result.chunk_index, result.similarity, display_text
                    ));
                }
                output_sections.push(section);
            }
            Ok(_) => {
                output_sections.push(format!("No semantic matches found for '{}'.", query));
            }
            Err(e) => {
                output_sections.push(format!("Semantic search error: {}", e));
            }
        }
    }
}

#[async_trait]
impl Tool for CodebaseSearchTool {
    fn name(&self) -> &str {
        "CodebaseSearch"
    }

    fn description(&self) -> &str {
        "Search the project's indexed codebase for symbols, files, or semantic similarity. Uses the pre-built SQLite index for fast lookups without scanning the filesystem. The 'semantic' scope performs vector similarity search over code chunks. Preferred over Grep/Glob for initial code exploration when the index is available."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "query".to_string(),
            ParameterSchema::string(Some(
                "Search pattern — symbol name, file path fragment, or keyword to search for",
            )),
        );

        let mut scope_schema = ParameterSchema::string(Some(
            "Search scope: 'symbols' (search symbol names), 'files' (search file paths/components), 'semantic' (vector similarity search over code chunks), 'all' (merge symbols + files). Default: 'all'",
        ));
        scope_schema.enum_values = Some(vec![
            "files".to_string(),
            "symbols".to_string(),
            "semantic".to_string(),
            "all".to_string(),
        ]);
        scope_schema.default = Some(serde_json::Value::String("all".to_string()));
        properties.insert("scope".to_string(), scope_schema);

        properties.insert(
            "component".to_string(),
            ParameterSchema::string(Some(
                "Optional component name to narrow results (e.g., 'desktop-rust', 'desktop-web')",
            )),
        );

        ParameterSchema::object(
            Some("CodebaseSearch parameters"),
            properties,
            vec!["query".to_string()],
        )
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return ToolResult::err("Missing required parameter: query"),
        };

        let scope = args
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let component = args.get("component").and_then(|v| v.as_str());

        let index_store = match &ctx.index_store {
            Some(store) => store,
            None => {
                return ToolResult::ok(
                    "Codebase index not available. The project has not been indexed yet. \
                     Use Grep for content search or Glob/LS for file discovery instead.",
                );
            }
        };

        let project_path = ctx.project_root.to_string_lossy().to_string();

        // --- scope="all": Use HybridSearchEngine with RRF fusion ---
        if scope == "all" {
            match Self::execute_hybrid_search(ctx, query, &project_path, component, index_store)
                .await
            {
                Ok(output) => return ToolResult::ok(output),
                Err(e) => {
                    // Fallback: log the error and try individual channels below
                    tracing::warn!(
                        "HybridSearchEngine failed, falling back to manual channels: {}",
                        e
                    );
                }
            }
        }

        // --- Individual scope handling (or fallback from HybridSearch failure) ---
        let mut output_sections: Vec<String> = Vec::new();

        // --- Symbol search ---
        if scope == "symbols" || scope == "all" {
            Self::search_symbols_channel(
                query,
                component,
                &project_path,
                scope,
                index_store,
                &mut output_sections,
            );
        }

        // --- File search ---
        if scope == "files" || scope == "all" {
            Self::search_files_channel(
                query,
                component,
                &project_path,
                scope,
                index_store,
                &mut output_sections,
            );
        }

        // --- Semantic search ---
        if scope == "semantic" || scope == "all" {
            Self::search_semantic_channel(ctx, query, &project_path, scope, index_store, &mut output_sections).await;
        }

        if output_sections.is_empty() {
            ToolResult::ok(format!(
                "No results found for '{}' (scope: {}).",
                query, scope
            ))
        } else {
            ToolResult::ok(output_sections.join("\n"))
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
        assert!(props.contains_key("component"));

        let scope = props.get("scope").unwrap();
        let enum_vals = scope.enum_values.as_ref().unwrap();
        assert!(enum_vals.contains(&"files".to_string()));
        assert!(enum_vals.contains(&"symbols".to_string()));
        assert!(enum_vals.contains(&"semantic".to_string()));
        assert!(enum_vals.contains(&"all".to_string()));
    }

    #[test]
    fn test_codebase_search_tool_not_long_running() {
        let tool = CodebaseSearchTool::new();
        assert!(!tool.is_long_running());
    }

    #[tokio::test]
    async fn test_codebase_search_no_index() {
        let tool = CodebaseSearchTool::new();
        let ctx = make_ctx(Path::new("/tmp"));
        let args = serde_json::json!({"query": "test_function"});
        let result = tool.execute(&ctx, args).await;
        assert!(result.success);
        assert!(result
            .output
            .as_ref()
            .unwrap()
            .contains("not been indexed"));
    }

    #[tokio::test]
    async fn test_codebase_search_missing_query() {
        let tool = CodebaseSearchTool::new();
        let ctx = make_ctx(Path::new("/tmp"));
        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("query"));
    }
}
