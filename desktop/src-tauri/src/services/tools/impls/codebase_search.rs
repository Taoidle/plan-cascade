//! CodebaseSearch Tool Implementation
//!
//! Searches the project's indexed codebase for symbols, files, or semantic similarity.
//! Uses IndexStore, EmbeddingManager, EmbeddingService, and HnswIndex from
//! ToolExecutionContext for all index queries.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
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
/// and `ctx.hnsw_index` from the ToolExecutionContext. When `scope="hybrid"`,
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
        file_path_prefix: Option<&str>,
        allowed_language_paths: Option<&HashSet<String>>,
        limit: usize,
        include_snippet: bool,
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
            let mut hint = format!(
                "No results found for '{}' (scope: hybrid, RRF).\n\
                 Hint: Try shorter queries (1-2 keywords). \
                 Use scope=\"path\" or scope=\"symbol\" for narrower search. \
                 Use LS to browse directories or Grep for full-text content search.",
                query
            );
            // List available components to help the LLM refine its search
            if let Ok(summary) = index_store.get_project_summary(project_path) {
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
            return Ok(hint);
        }

        // Apply optional V2 filters
        let results: Vec<&HybridSearchResult> = results
            .iter()
            .filter(|r| {
                if let Some(comp) = component {
                    if !r.file_path.contains(comp) {
                        return false;
                    }
                }
                if let Some(prefix) = file_path_prefix {
                    if !r.file_path.starts_with(prefix) {
                        return false;
                    }
                }
                if let Some(paths) = allowed_language_paths {
                    if !paths.contains(&r.file_path) {
                        return false;
                    }
                }
                true
            })
            .collect();

        if results.is_empty() {
            return Ok(format!(
                "No results found for '{}' in component '{}'.",
                query,
                component.unwrap_or("unknown")
            ));
        }

        let mut output = String::new();

        // --- Search metadata header ---
        if let Some(ref provider) = outcome.provider_display {
            let dim_info = if outcome.embedding_dimension > 0 {
                format!(", {}-dim", outcome.embedding_dimension)
            } else {
                String::new()
            };
            output.push_str(&format!("> Provider: {}{}\n", provider, dim_info));
        }

        if outcome.semantic_degraded {
            let reason = outcome.semantic_error.as_deref().unwrap_or("unknown");
            output.push_str(&format!("> Semantic: degraded ({})\n", reason));
        } else if outcome.hnsw_used {
            output.push_str(&format!(
                "> Semantic: active (HNSW, {} vectors)\n",
                outcome.hnsw_vector_count
            ));
        } else {
            output.push_str("> Semantic: not configured\n");
        }

        if !outcome.active_channels.is_empty() {
            let ch_names: Vec<String> = outcome
                .active_channels
                .iter()
                .map(|c| format!("{}", c))
                .collect();
            output.push_str(&format!("> Channels: {}\n", ch_names.join(", ")));
        }
        output.push('\n');

        // Prepend semantic degradation notice if applicable (context for LLM)
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
        for result in results.iter().take(limit) {
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
            if include_snippet {
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
            }
            if let Some(sim) = result.semantic_similarity {
                line.push_str(&format!("    similarity: {:.3}\n", sim));
            }
            if let Some(ref rtype) = result.resolved_type {
                line.push_str(&format!("    type: {}\n", rtype));
            }
            if result.reference_count > 0 {
                line.push_str(&format!("    refs: {}\n", result.reference_count));
            }
            output.push_str(&line);
        }
        if results.len() > limit {
            output.push_str(&format!("  ... and {} more\n", results.len() - limit));
        }

        Ok(output)
    }

    /// Symbol search channel: prefers FTS5 BM25, falls back to SQL LIKE.
    fn search_symbols_channel(
        query: &str,
        component: Option<&str>,
        file_path_prefix: Option<&str>,
        allowed_language_paths: Option<&HashSet<String>>,
        project_path: &str,
        scope: &str,
        limit: usize,
        index_store: &Arc<IndexStore>,
        output_sections: &mut Vec<String>,
    ) {
        // Try FTS5 BM25 first, fall back to LIKE
        let symbols_result = match index_store.fts_search_symbols(query, project_path, limit) {
            Ok(fts_results) if !fts_results.is_empty() => Ok(fts_results),
            _ => {
                // Fallback to LIKE
                let pattern = format!("%{}%", query);
                index_store.query_symbols(project_path, &pattern)
            }
        };

        match symbols_result {
            Ok(symbols) => {
                let filtered: Vec<_> = symbols
                    .into_iter()
                    .filter(|s| {
                        if let Some(comp) = component {
                            if !s.file_path.contains(comp) {
                                return false;
                            }
                        }
                        if let Some(prefix) = file_path_prefix {
                            if !s.file_path.starts_with(prefix) {
                                return false;
                            }
                        }
                        if let Some(paths) = allowed_language_paths {
                            if !paths.contains(&s.file_path) {
                                return false;
                            }
                        }
                        true
                    })
                    .collect();

                if !filtered.is_empty() {
                    let mut section = format!(
                        "## Symbols matching '{}' ({} results)\n",
                        query,
                        filtered.len()
                    );
                    for sym in filtered.iter().take(limit) {
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
                        if let Some(ref rtype) = sym.resolved_type {
                            section.push_str(&format!("    type: {}\n", rtype));
                        }
                        if sym.reference_count > 0 {
                            section.push_str(&format!("    refs: {}\n", sym.reference_count));
                        }
                    }
                    if filtered.len() > limit {
                        section.push_str(&format!("  ... and {} more\n", filtered.len() - limit));
                    }
                    output_sections.push(section);
                } else if scope == "symbol" {
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
        file_path_prefix: Option<&str>,
        allowed_language_paths: Option<&HashSet<String>>,
        project_path: &str,
        scope: &str,
        limit: usize,
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
                        .filter(|f| {
                            if let Some(prefix) = file_path_prefix {
                                if !f.file_path.starts_with(prefix) {
                                    return false;
                                }
                            }
                            if let Some(paths) = allowed_language_paths {
                                if !paths.contains(&f.file_path) {
                                    return false;
                                }
                            }
                            true
                        })
                        .collect();

                    if !filtered.is_empty() {
                        let mut section = format!(
                            "## Files matching '{}' in component '{}' ({} results)\n",
                            query,
                            comp,
                            filtered.len()
                        );
                        for file in filtered.iter().take(limit) {
                            section.push_str(&format!(
                                "  {} ({}, {} lines)\n",
                                file.file_path, file.language, file.line_count
                            ));
                        }
                        if filtered.len() > limit {
                            section
                                .push_str(&format!("  ... and {} more\n", filtered.len() - limit));
                        }
                        output_sections.push(section);
                    } else if scope == "path" {
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
            match index_store.fts_search_files(query, project_path, limit) {
                Ok(fts_results) if !fts_results.is_empty() => {
                    let filtered: Vec<_> = fts_results
                        .into_iter()
                        .filter(|f| {
                            if let Some(prefix) = file_path_prefix {
                                if !f.file_path.starts_with(prefix) {
                                    return false;
                                }
                            }
                            if let Some(paths) = allowed_language_paths {
                                if !paths.contains(&f.file_path) {
                                    return false;
                                }
                            }
                            true
                        })
                        .collect();
                    if filtered.is_empty() {
                        if scope == "path" {
                            output_sections.push(format!("No files matching '{}'.", query));
                        }
                        return;
                    }
                    let mut section = format!(
                        "## Files matching '{}' ({} results)\n",
                        query,
                        filtered.len()
                    );
                    for file in filtered.iter().take(limit) {
                        section.push_str(&format!(
                            "  {} ({}, {} lines)\n",
                            file.file_path, file.language, file.line_count
                        ));
                    }
                    if filtered.len() > limit {
                        section.push_str(&format!("  ... and {} more\n", filtered.len() - limit));
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
                                            if let Some(prefix) = file_path_prefix {
                                                if !file.file_path.starts_with(prefix) {
                                                    continue;
                                                }
                                            }
                                            if let Some(paths) = allowed_language_paths {
                                                if !paths.contains(&file.file_path) {
                                                    continue;
                                                }
                                            }
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
                                for line in matching_files.iter().take(limit) {
                                    section.push_str(line);
                                    section.push('\n');
                                }
                                if count > limit {
                                    section
                                        .push_str(&format!("  ... and {} more\n", count - limit));
                                }
                                output_sections.push(section);
                            } else if scope == "path" {
                                output_sections.push(format!("No files matching '{}'.", query));
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
        component: Option<&str>,
        file_path_prefix: Option<&str>,
        allowed_language_paths: Option<&HashSet<String>>,
        project_path: &str,
        scope: &str,
        limit: usize,
        include_snippet: bool,
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
                                let hnsw_hits = hnsw.search(&query_embedding, limit).await;
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
                                index_store.semantic_search(&query_embedding, project_path, limit)
                            }
                        } else {
                            index_store.semantic_search(&query_embedding, project_path, limit)
                        };

                        Self::format_semantic_results(
                            query,
                            component,
                            file_path_prefix,
                            allowed_language_paths,
                            limit,
                            include_snippet,
                            search_result,
                            output_sections,
                        );
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
                             Use 'symbol' or 'path' scope instead.",
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
                        index_store.semantic_search(&query_embedding, project_path, limit);
                    Self::format_semantic_results(
                        query,
                        component,
                        file_path_prefix,
                        allowed_language_paths,
                        limit,
                        include_snippet,
                        search_result,
                        output_sections,
                    );
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
                     Use 'symbol' or 'path' scope instead."
                        .to_string(),
                );
            } else {
                output_sections
                    .push("Semantic search: not available (vocabulary not built)".to_string());
            }
        } else {
            // Neither EmbeddingManager nor EmbeddingService configured
            if is_standalone_semantic {
                output_sections.push(
                    "Semantic search not available: no embedding provider configured. \
                     The project has not been indexed with embedding support. \
                     Use 'symbol' or 'path' scope instead."
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
        component: Option<&str>,
        file_path_prefix: Option<&str>,
        allowed_language_paths: Option<&HashSet<String>>,
        limit: usize,
        include_snippet: bool,
        search_result: Result<Vec<SemanticSearchResult>, AppError>,
        output_sections: &mut Vec<String>,
    ) {
        match search_result {
            Ok(results) if !results.is_empty() => {
                let filtered: Vec<SemanticSearchResult> = results
                    .into_iter()
                    .filter(|result| {
                        if let Some(comp) = component {
                            if !result.file_path.contains(comp) {
                                return false;
                            }
                        }
                        if let Some(prefix) = file_path_prefix {
                            if !result.file_path.starts_with(prefix) {
                                return false;
                            }
                        }
                        if let Some(paths) = allowed_language_paths {
                            if !paths.contains(&result.file_path) {
                                return false;
                            }
                        }
                        true
                    })
                    .collect();

                if filtered.is_empty() {
                    output_sections.push(format!("No semantic matches found for '{}'.", query));
                    return;
                }

                let mut section = format!(
                    "## Semantic search for '{}' ({} results)\n",
                    query,
                    filtered.len()
                );
                for result in filtered.iter().take(limit) {
                    let mut line = format!(
                        "  {} (chunk {}, similarity: {:.3})",
                        result.file_path, result.chunk_index, result.similarity
                    );
                    if include_snippet {
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
                        line.push_str(&format!("\n    {}", display_text));
                    }
                    line.push('\n');
                    section.push_str(&line);
                }
                if filtered.len() > limit {
                    section.push_str(&format!("  ... and {} more\n", filtered.len() - limit));
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
            ParameterSchema::string(Some("Filter by component/path fragment.")),
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
        let scope_normalized = scope_input.trim().to_ascii_lowercase();
        let scope = match scope_normalized.as_str() {
            "hybrid" | "symbol" | "path" | "semantic" => scope_normalized.as_str(),
            _ => {
                return ToolResult::err(format!(
                    "Invalid scope '{}'. Use one of: hybrid, symbol, path, semantic.",
                    scope_input
                ))
                .with_error_code("invalid_scope");
            }
        };

        let normalized_arg = |value: Option<&Value>| -> Option<String> {
            value
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        };

        let filters = args.get("filters").and_then(|v| v.as_object());
        let component = normalized_arg(filters.and_then(|f| f.get("component")));
        let language_filter = normalized_arg(filters.and_then(|f| f.get("language")));
        let file_path_prefix = normalized_arg(filters.and_then(|f| f.get("file_path_prefix")));

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

        let project_path = normalized_arg(args.get("project_path"))
            .unwrap_or_else(|| ctx.project_root.to_string_lossy().to_string());
        if let Some(workspace_root_id) = normalized_arg(args.get("workspace_root_id")) {
            tracing::debug!(
                "CodebaseSearch workspace_root_id provided but not yet used in tool routing: {}",
                workspace_root_id
            );
        }

        let allowed_language_paths: Option<HashSet<String>> = if let Some(language) =
            language_filter.as_deref()
        {
            match index_store.list_project_files(&project_path, Some(language), None, 0, 100_000) {
                Ok((files, _)) => Some(files.into_iter().map(|f| f.file_path).collect()),
                Err(e) => {
                    tracing::warn!(
                        "CodebaseSearch language filter failed for project '{}': {}",
                        project_path,
                        e
                    );
                    Some(HashSet::new())
                }
            }
        } else {
            None
        };

        // --- scope="hybrid": Use HybridSearchEngine with RRF fusion ---
        if scope == "hybrid" {
            match Self::execute_hybrid_search(
                ctx,
                query,
                &project_path,
                component.as_deref(),
                file_path_prefix.as_deref(),
                allowed_language_paths.as_ref(),
                limit,
                include_snippet,
                index_store,
            )
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
        if scope == "symbol" || scope == "hybrid" {
            Self::search_symbols_channel(
                query,
                component.as_deref(),
                file_path_prefix.as_deref(),
                allowed_language_paths.as_ref(),
                &project_path,
                scope,
                limit,
                index_store,
                &mut output_sections,
            );
        }

        // --- File search ---
        if scope == "path" || scope == "hybrid" {
            Self::search_files_channel(
                query,
                component.as_deref(),
                file_path_prefix.as_deref(),
                allowed_language_paths.as_ref(),
                &project_path,
                scope,
                limit,
                index_store,
                &mut output_sections,
            );
        }

        // --- Semantic search ---
        if scope == "semantic" || scope == "hybrid" {
            Self::search_semantic_channel(
                ctx,
                query,
                component.as_deref(),
                file_path_prefix.as_deref(),
                allowed_language_paths.as_ref(),
                &project_path,
                scope,
                limit,
                include_snippet,
                index_store,
                &mut output_sections,
            )
            .await;
        }

        if output_sections.is_empty() {
            let mut hint = format!(
                "No results found for '{}' (scope: {}).\n\
                 Hint: Try a shorter or different keyword. \
                 Use scope=\"path\" for file paths, scope=\"symbol\" for function/class names. \
                 You can also use Grep for regex search or LS to browse directories.",
                query, scope
            );
            // List available components to help the LLM refine its search
            if let Ok(summary) = index_store.get_project_summary(&project_path) {
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
            ToolResult::ok(hint)
        } else {
            ToolResult::ok(output_sections.join("\n"))
        }
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
