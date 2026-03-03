//! LSP Enricher — Enrichment Pass Orchestration
//!
//! Coordinates LSP-based enrichment after Tree-sitter indexing completes.
//! For each detected language server: starts an LspClient, queries
//! hover/references/definition for each symbol, and stores the results
//! in the IndexStore.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, info, warn};
use url::Url;

use super::index_store::IndexStore;
use super::lsp_client::LspClient;
use super::lsp_registry::LspServerRegistry;

/// Rate limit: maximum requests per second per language server.
const MAX_REQUESTS_PER_SECOND: u32 = 10;

/// Report returned after an enrichment pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentReport {
    pub languages_enriched: Vec<String>,
    pub symbols_enriched: usize,
    pub references_found: usize,
    pub duration_ms: u64,
}

/// Orchestrates LSP-based semantic enrichment of indexed symbols.
pub struct LspEnricher {
    registry: Arc<LspServerRegistry>,
    index_store: Arc<IndexStore>,
    /// Active LSP client connections, one per language.
    clients: RwLock<HashMap<String, Arc<LspClient>>>,
    /// Timestamp of the last enrichment activity.
    /// Used by [`shutdown_if_idle`] to decide when to reclaim idle clients.
    last_activity: RwLock<Option<Instant>>,
}

impl LspEnricher {
    /// Create a new enricher backed by the given registry and index store.
    pub fn new(registry: Arc<LspServerRegistry>, index_store: Arc<IndexStore>) -> Self {
        Self {
            registry,
            index_store,
            clients: RwLock::new(HashMap::new()),
            last_activity: RwLock::new(None),
        }
    }

    /// Run the enrichment pass for a project.
    ///
    /// 1. Detect available servers via registry
    /// 2. Start LSP clients for detected languages
    /// 3. Query symbols from index_store grouped by (language, file_path)
    /// 4. For each file: didOpen, hover for types, references for counts, didClose
    /// 5. Rate limit at 10 req/s per server
    /// 6. Shutdown all clients
    pub async fn enrich_project(&self, project_path: &str) -> anyhow::Result<EnrichmentReport> {
        let start = Instant::now();
        let mut report = EnrichmentReport {
            languages_enriched: Vec::new(),
            symbols_enriched: 0,
            references_found: 0,
            duration_ms: 0,
        };

        // Step 1: Detect available language servers
        let detected = self.registry.detect_all();
        if detected.is_empty() {
            return Err(anyhow::anyhow!("LSP_NO_SERVERS_DETECTED"));
        }

        // Full enrichment must be based on fresh semantic data.
        self.index_store.clear_enrichment_data(project_path)?;

        info!(
            servers = ?detected.keys().collect::<Vec<_>>(),
            "Starting LSP enrichment pass"
        );

        // Step 2: Start LSP clients for each detected language
        for (language, detected_server) in &detected {
            let args_refs: Vec<&str> = detected_server.args.iter().map(|s| s.as_str()).collect();

            match LspClient::start(&detected_server.command, &args_refs, project_path).await {
                Ok(client) => {
                    let mut clients = self.clients.write().await;
                    clients.insert(language.clone(), Arc::new(client));
                    report.languages_enriched.push(language.clone());
                }
                Err(e) => {
                    warn!(
                        language = language.as_str(),
                        command = detected_server.command.as_str(),
                        error = %e,
                        "Failed to start LSP client, skipping language"
                    );
                }
            }
        }

        // Step 3-4: For each language, enrich symbols
        let languages = report.languages_enriched.clone();
        for language in &languages {
            let mut symbols = Vec::new();
            for index_language in index_languages_for_server(language) {
                match self
                    .index_store
                    .get_symbols_for_enrichment(project_path, index_language)
                {
                    Ok(mut s) => symbols.append(&mut s),
                    Err(e) => {
                        warn!(
                            language = language.as_str(),
                            index_language,
                            error = %e,
                            "Failed to get symbols for enrichment"
                        );
                    }
                }
            }

            if symbols.is_empty() {
                debug!(
                    language = language.as_str(),
                    "No symbols found for enrichment"
                );
                continue;
            }

            let clients = self.clients.read().await;
            let client = match clients.get(language.as_str()) {
                Some(c) => Arc::clone(c),
                None => continue,
            };
            drop(clients);

            // Group symbols by file_path
            let mut files: HashMap<String, Vec<(i64, String, i64)>> = HashMap::new();
            for (rowid, file_path, symbol_name, line, _lang) in &symbols {
                files.entry(file_path.clone()).or_default().push((
                    *rowid,
                    symbol_name.clone(),
                    *line,
                ));
            }

            // Rate limiting: track request count within each second
            let mut request_count = 0u32;
            let mut window_start = Instant::now();

            for (file_path, file_symbols) in &files {
                let full_path = resolve_full_path(project_path, file_path);
                let uri = match path_to_lsp_uri(&full_path) {
                    Ok(u) => u,
                    Err(_) => continue,
                };

                // Read the file content for didOpen
                let content = match tokio::fs::read_to_string(&full_path).await {
                    Ok(c) => c,
                    Err(_) => continue, // Skip files we can't read
                };

                // didOpen
                let lang_id = language_to_lsp_id(language);

                if let Err(e) = client
                    .notify::<lsp_types::notification::DidOpenTextDocument>(
                        lsp_types::DidOpenTextDocumentParams {
                            text_document: lsp_types::TextDocumentItem {
                                uri: uri.clone(),
                                language_id: lang_id.to_string(),
                                version: 1,
                                text: content,
                            },
                        },
                    )
                    .await
                {
                    debug!(file = file_path.as_str(), error = %e, "didOpen failed");
                    continue;
                }

                // For each symbol: hover + references
                for (rowid, _symbol_name, line) in file_symbols {
                    // Rate limiting
                    request_count += 1;
                    if request_count >= MAX_REQUESTS_PER_SECOND {
                        let elapsed = window_start.elapsed();
                        if elapsed < Duration::from_secs(1) {
                            sleep(Duration::from_secs(1) - elapsed).await;
                        }
                        request_count = 0;
                        window_start = Instant::now();
                    }

                    let position = lsp_types::Position {
                        line: (*line as u32).saturating_sub(1), // LSP is 0-indexed
                        character: 0,
                    };

                    // Hover -> extract type
                    match client
                        .request::<lsp_types::request::HoverRequest>(lsp_types::HoverParams {
                            text_document_position_params: lsp_types::TextDocumentPositionParams {
                                text_document: lsp_types::TextDocumentIdentifier {
                                    uri: uri.clone(),
                                },
                                position,
                            },
                            work_done_progress_params: lsp_types::WorkDoneProgressParams {
                                work_done_token: None,
                            },
                        })
                        .await
                    {
                        Ok(Some(hover)) => {
                            if let Some(type_str) = extract_type_from_hover(&hover) {
                                let _ = self.index_store.update_symbol_type(*rowid, &type_str);
                                report.symbols_enriched += 1;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {}
                    }

                    // References -> count + cross-refs
                    match client
                        .request::<lsp_types::request::References>(lsp_types::ReferenceParams {
                            text_document_position: lsp_types::TextDocumentPositionParams {
                                text_document: lsp_types::TextDocumentIdentifier {
                                    uri: uri.clone(),
                                },
                                position,
                            },
                            work_done_progress_params: lsp_types::WorkDoneProgressParams {
                                work_done_token: None,
                            },
                            partial_result_params: lsp_types::PartialResultParams {
                                partial_result_token: None,
                            },
                            context: lsp_types::ReferenceContext {
                                include_declaration: false,
                            },
                        })
                        .await
                    {
                        Ok(Some(locations)) => {
                            let count = locations.len() as i64;
                            let _ = self.index_store.update_reference_count(*rowid, count);
                            report.references_found += locations.len();

                            // Insert cross-references
                            for location in &locations {
                                let uri_str = location.uri.as_str();
                                let target_path = uri_to_relative_path(uri_str, project_path);

                                let _ = self.index_store.insert_cross_reference(
                                    project_path,
                                    file_path,
                                    *line,
                                    Some(_symbol_name),
                                    &target_path,
                                    (location.range.start.line + 1) as i64,
                                    None,
                                    "usage",
                                );
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {}
                    }
                }

                // didClose
                let _ = client
                    .notify::<lsp_types::notification::DidCloseTextDocument>(
                        lsp_types::DidCloseTextDocumentParams {
                            text_document: lsp_types::TextDocumentIdentifier { uri },
                        },
                    )
                    .await;
            }
        }

        // Record activity timestamp (full enrichment is a one-shot operation
        // but we still track it for consistency).
        *self.last_activity.write().await = Some(Instant::now());

        // Step 6: Shutdown all clients
        self.shutdown_all().await;

        if report.languages_enriched.is_empty() {
            return Err(anyhow::anyhow!("LSP_NO_LIVE_CLIENTS"));
        }

        report.duration_ms = start.elapsed().as_millis() as u64;
        info!(
            languages = ?report.languages_enriched,
            symbols = report.symbols_enriched,
            references = report.references_found,
            duration_ms = report.duration_ms,
            "LSP enrichment complete"
        );

        Ok(report)
    }

    /// Ensure LSP clients are started for all detected languages.
    ///
    /// Reuses existing clients if still alive, otherwise starts new ones.
    /// Returns the list of active language identifiers.
    async fn ensure_clients_started(&self, project_path: &str) -> Vec<String> {
        let detected = self.registry.detect_all();
        if detected.is_empty() {
            return Vec::new();
        }

        let mut active_languages = Vec::new();
        for (language, detected_server) in &detected {
            // Check if a client already exists and is presumably alive.
            {
                let clients = self.clients.read().await;
                if clients.contains_key(language.as_str()) {
                    active_languages.push(language.clone());
                    continue;
                }
            }

            // Start a new client from detected command+args.
            let args_refs: Vec<&str> = detected_server.args.iter().map(|s| s.as_str()).collect();

            match LspClient::start(&detected_server.command, &args_refs, project_path).await {
                Ok(client) => {
                    let mut clients = self.clients.write().await;
                    clients.insert(language.clone(), Arc::new(client));
                    active_languages.push(language.clone());
                }
                Err(e) => {
                    warn!(
                        language = language.as_str(),
                        command = detected_server.command.as_str(),
                        error = %e,
                        "Failed to start LSP client for incremental enrichment"
                    );
                }
            }
        }

        *self.last_activity.write().await = Some(Instant::now());
        active_languages
    }

    /// Run incremental enrichment for specific changed files only.
    ///
    /// Reuses persistent LSP clients (started on demand, kept alive for
    /// subsequent calls).  Only queries symbols belonging to the changed
    /// files, and clears stale cross-references before re-enriching.
    pub async fn enrich_files(
        &self,
        project_path: &str,
        file_paths: &[String],
    ) -> anyhow::Result<EnrichmentReport> {
        let start = Instant::now();
        let mut report = EnrichmentReport {
            languages_enriched: Vec::new(),
            symbols_enriched: 0,
            references_found: 0,
            duration_ms: 0,
        };

        if file_paths.is_empty() {
            report.duration_ms = start.elapsed().as_millis() as u64;
            return Ok(report);
        }

        // Step 1: Ensure clients are running.
        let active_languages = self.ensure_clients_started(project_path).await;
        if active_languages.is_empty() {
            info!("No LSP clients available, skipping incremental enrichment");
            report.duration_ms = start.elapsed().as_millis() as u64;
            return Ok(report);
        }

        // Step 2: Clear stale cross-references for changed files.
        let file_refs: Vec<&str> = file_paths.iter().map(|s| s.as_str()).collect();
        if let Err(e) = self
            .index_store
            .delete_cross_references_for_files(project_path, &file_refs)
        {
            warn!(error = %e, "Failed to clear stale cross-references for incremental enrichment");
        }

        // Step 3: For each language, enrich symbols from changed files.
        for language in &active_languages {
            let mut symbols = Vec::new();
            for index_language in index_languages_for_server(language) {
                match self.index_store.get_symbols_for_enrichment_by_files(
                    project_path,
                    index_language,
                    &file_refs,
                ) {
                    Ok(mut s) => symbols.append(&mut s),
                    Err(e) => {
                        warn!(
                            language = language.as_str(),
                            index_language,
                            error = %e,
                            "Failed to get symbols for incremental enrichment"
                        );
                    }
                }
            }

            if symbols.is_empty() {
                continue;
            }

            let clients = self.clients.read().await;
            let client = match clients.get(language.as_str()) {
                Some(c) => Arc::clone(c),
                None => continue,
            };
            drop(clients);

            if !report.languages_enriched.contains(language) {
                report.languages_enriched.push(language.clone());
            }

            // Group symbols by file_path
            let mut files: HashMap<String, Vec<(i64, String, i64)>> = HashMap::new();
            for (rowid, file_path, symbol_name, line, _lang) in &symbols {
                files.entry(file_path.clone()).or_default().push((
                    *rowid,
                    symbol_name.clone(),
                    *line,
                ));
            }

            // Rate limiting
            let mut request_count = 0u32;
            let mut window_start = Instant::now();

            for (file_path, file_symbols) in &files {
                let full_path = resolve_full_path(project_path, file_path);
                let uri = match path_to_lsp_uri(&full_path) {
                    Ok(u) => u,
                    Err(_) => continue,
                };

                let content = match tokio::fs::read_to_string(&full_path).await {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let lang_id = language_to_lsp_id(language);

                if let Err(e) = client
                    .notify::<lsp_types::notification::DidOpenTextDocument>(
                        lsp_types::DidOpenTextDocumentParams {
                            text_document: lsp_types::TextDocumentItem {
                                uri: uri.clone(),
                                language_id: lang_id.to_string(),
                                version: 1,
                                text: content,
                            },
                        },
                    )
                    .await
                {
                    debug!(file = file_path.as_str(), error = %e, "didOpen failed");
                    continue;
                }

                for (rowid, _symbol_name, line) in file_symbols {
                    request_count += 1;
                    if request_count >= MAX_REQUESTS_PER_SECOND {
                        let elapsed = window_start.elapsed();
                        if elapsed < Duration::from_secs(1) {
                            sleep(Duration::from_secs(1) - elapsed).await;
                        }
                        request_count = 0;
                        window_start = Instant::now();
                    }

                    let position = lsp_types::Position {
                        line: (*line as u32).saturating_sub(1),
                        character: 0,
                    };

                    // Hover → extract type
                    match client
                        .request::<lsp_types::request::HoverRequest>(lsp_types::HoverParams {
                            text_document_position_params: lsp_types::TextDocumentPositionParams {
                                text_document: lsp_types::TextDocumentIdentifier {
                                    uri: uri.clone(),
                                },
                                position,
                            },
                            work_done_progress_params: lsp_types::WorkDoneProgressParams {
                                work_done_token: None,
                            },
                        })
                        .await
                    {
                        Ok(Some(hover)) => {
                            if let Some(type_str) = extract_type_from_hover(&hover) {
                                let _ = self.index_store.update_symbol_type(*rowid, &type_str);
                                report.symbols_enriched += 1;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {}
                    }

                    // References → count + cross-refs
                    match client
                        .request::<lsp_types::request::References>(lsp_types::ReferenceParams {
                            text_document_position: lsp_types::TextDocumentPositionParams {
                                text_document: lsp_types::TextDocumentIdentifier {
                                    uri: uri.clone(),
                                },
                                position,
                            },
                            work_done_progress_params: lsp_types::WorkDoneProgressParams {
                                work_done_token: None,
                            },
                            partial_result_params: lsp_types::PartialResultParams {
                                partial_result_token: None,
                            },
                            context: lsp_types::ReferenceContext {
                                include_declaration: false,
                            },
                        })
                        .await
                    {
                        Ok(Some(locations)) => {
                            let count = locations.len() as i64;
                            let _ = self.index_store.update_reference_count(*rowid, count);
                            report.references_found += locations.len();

                            for location in &locations {
                                let uri_str = location.uri.as_str();
                                let target_path = uri_to_relative_path(uri_str, project_path);

                                let _ = self.index_store.insert_cross_reference(
                                    project_path,
                                    file_path,
                                    *line,
                                    Some(_symbol_name),
                                    &target_path,
                                    (location.range.start.line + 1) as i64,
                                    None,
                                    "usage",
                                );
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {}
                    }
                }

                // didClose
                let _ = client
                    .notify::<lsp_types::notification::DidCloseTextDocument>(
                        lsp_types::DidCloseTextDocumentParams {
                            text_document: lsp_types::TextDocumentIdentifier { uri },
                        },
                    )
                    .await;
            }
        }

        // Update activity timestamp — do NOT shutdown (keep clients alive).
        *self.last_activity.write().await = Some(Instant::now());

        report.duration_ms = start.elapsed().as_millis() as u64;
        info!(
            languages = ?report.languages_enriched,
            symbols = report.symbols_enriched,
            references = report.references_found,
            files = file_paths.len(),
            duration_ms = report.duration_ms,
            "Incremental LSP enrichment complete"
        );

        Ok(report)
    }

    /// Shutdown clients if they have been idle for longer than the given duration.
    ///
    /// Returns `true` if clients were shut down.
    pub async fn shutdown_if_idle(&self, idle_timeout: Duration) -> bool {
        let last = *self.last_activity.read().await;
        match last {
            Some(ts) if ts.elapsed() > idle_timeout => {
                let clients = self.clients.read().await;
                if clients.is_empty() {
                    return false;
                }
                drop(clients);
                info!(
                    "Shutting down idle LSP clients (idle for {:?})",
                    ts.elapsed()
                );
                self.shutdown_all().await;
                true
            }
            _ => false,
        }
    }

    /// Shutdown all active LSP clients.
    pub async fn shutdown_all(&self) {
        let mut clients = self.clients.write().await;
        let entries: Vec<(String, Arc<LspClient>)> = clients.drain().collect();
        drop(clients);

        for (language, client) in entries {
            match Arc::try_unwrap(client) {
                Ok(c) => {
                    if let Err(e) = c.shutdown().await {
                        warn!(
                            language = language.as_str(),
                            error = %e,
                            "Failed to shutdown LSP client"
                        );
                    }
                }
                Err(_) => {
                    warn!(
                        language = language.as_str(),
                        "Could not shutdown LSP client: still referenced"
                    );
                }
            }
        }
    }
}

/// Extract a type string from a Hover response.
///
/// Looks for type information in the hover content (MarkupContent or MarkedString).
fn extract_type_from_hover(hover: &lsp_types::Hover) -> Option<String> {
    match &hover.contents {
        lsp_types::HoverContents::Markup(markup) => {
            let text = markup.value.trim();
            if text.is_empty() {
                return None;
            }
            // Try to extract the type from code blocks
            // Common patterns: ```rust\nfn foo() -> i32\n```
            if let Some(code) = extract_code_block(text) {
                Some(code)
            } else {
                // Return raw text, truncated
                let truncated = text.chars().take(200).collect::<String>();
                Some(truncated)
            }
        }
        lsp_types::HoverContents::Array(parts) => {
            // Take the first non-empty part
            for part in parts {
                match part {
                    lsp_types::MarkedString::String(s) if !s.is_empty() => {
                        return Some(s.chars().take(200).collect());
                    }
                    lsp_types::MarkedString::LanguageString(ls) if !ls.value.is_empty() => {
                        return Some(ls.value.chars().take(200).collect());
                    }
                    _ => {}
                }
            }
            None
        }
        lsp_types::HoverContents::Scalar(scalar) => match scalar {
            lsp_types::MarkedString::String(s) if !s.is_empty() => {
                Some(s.chars().take(200).collect())
            }
            lsp_types::MarkedString::LanguageString(ls) if !ls.value.is_empty() => {
                Some(ls.value.chars().take(200).collect())
            }
            _ => None,
        },
    }
}

/// Convert a file:// URI string to a relative path from the project root.
fn uri_to_relative_path(uri_str: &str, project_path: &str) -> String {
    let normalized_project = PathBuf::from(project_path);
    let decoded_path = Url::parse(uri_str)
        .ok()
        .and_then(|url| url.to_file_path().ok())
        .or_else(|| uri_str.strip_prefix("file://").map(PathBuf::from));

    let Some(path) = decoded_path else {
        return uri_str.to_string();
    };

    if let Ok(relative) = path.strip_prefix(&normalized_project) {
        return relative.to_string_lossy().replace('\\', "/");
    }

    path.to_string_lossy().replace('\\', "/")
}

fn language_to_lsp_id(language: &str) -> &str {
    match language {
        "typescript" | "javascript" => "typescript",
        "rust" => "rust",
        "python" => "python",
        "go" => "go",
        "java" => "java",
        _ => language,
    }
}

fn index_languages_for_server(language: &str) -> Vec<&str> {
    match language {
        "typescript" => vec!["typescript", "javascript"],
        _ => vec![language],
    }
}

fn resolve_full_path(project_path: &str, file_path: &str) -> PathBuf {
    let file = Path::new(file_path);
    if file.is_absolute() {
        file.to_path_buf()
    } else {
        Path::new(project_path).join(file)
    }
}

fn path_to_lsp_uri(path: &Path) -> anyhow::Result<lsp_types::Uri> {
    let url = Url::from_file_path(path)
        .map_err(|_| anyhow::anyhow!("Failed to convert path to file URI: {}", path.display()))?;
    lsp_types::Uri::from_str(url.as_str())
        .map_err(|e| anyhow::anyhow!("Invalid LSP URI '{}': {}", url, e))
}

/// Extract content from a markdown code block.
fn extract_code_block(text: &str) -> Option<String> {
    let start = text.find("```")?;
    let after_backticks = &text[start + 3..];
    // Skip the language identifier line
    let content_start = after_backticks.find('\n')? + 1;
    let content = &after_backticks[content_start..];
    let end = content.find("```")?;
    let code = content[..end].trim().to_string();
    if code.is_empty() {
        None
    } else {
        Some(code)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_type_from_hover_markup() {
        let hover = lsp_types::Hover {
            contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: "```rust\nfn my_func() -> i32\n```".to_string(),
            }),
            range: None,
        };

        let result = extract_type_from_hover(&hover);
        assert_eq!(result, Some("fn my_func() -> i32".to_string()));
    }

    #[test]
    fn test_extract_type_from_hover_plain() {
        let hover = lsp_types::Hover {
            contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::PlainText,
                value: "pub struct MyStruct".to_string(),
            }),
            range: None,
        };

        let result = extract_type_from_hover(&hover);
        assert_eq!(result, Some("pub struct MyStruct".to_string()));
    }

    #[test]
    fn test_extract_type_from_hover_empty() {
        let hover = lsp_types::Hover {
            contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::PlainText,
                value: "".to_string(),
            }),
            range: None,
        };

        let result = extract_type_from_hover(&hover);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_type_from_hover_scalar() {
        let hover = lsp_types::Hover {
            contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(
                "fn foo() -> Bar".to_string(),
            )),
            range: None,
        };

        let result = extract_type_from_hover(&hover);
        assert_eq!(result, Some("fn foo() -> Bar".to_string()));
    }

    #[test]
    fn test_extract_code_block() {
        let text = "Some text\n```rust\nfn hello() -> i32\n```\nMore text";
        let result = extract_code_block(text);
        assert_eq!(result, Some("fn hello() -> i32".to_string()));
    }

    #[test]
    fn test_extract_code_block_no_block() {
        let text = "Just plain text without code blocks";
        let result = extract_code_block(text);
        assert!(result.is_none());
    }

    #[test]
    fn test_enrichment_report_default() {
        let report = EnrichmentReport {
            languages_enriched: vec!["rust".to_string()],
            symbols_enriched: 100,
            references_found: 50,
            duration_ms: 1500,
        };

        assert_eq!(report.languages_enriched.len(), 1);
        assert_eq!(report.symbols_enriched, 100);
        assert_eq!(report.references_found, 50);
        assert_eq!(report.duration_ms, 1500);
    }

    #[test]
    fn test_enrichment_report_serialization() {
        let report = EnrichmentReport {
            languages_enriched: vec!["rust".to_string(), "python".to_string()],
            symbols_enriched: 42,
            references_found: 17,
            duration_ms: 3000,
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"symbols_enriched\":42"));

        let deserialized: EnrichmentReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.languages_enriched.len(), 2);
    }

    #[test]
    fn test_uri_to_relative_path_decodes_spaces() {
        let project = "/tmp/demo project";
        let uri = "file:///tmp/demo%20project/src/My%20File.ts";
        let rel = uri_to_relative_path(uri, project);
        assert_eq!(rel, "src/My File.ts");
    }

    #[test]
    fn test_uri_to_relative_path_decodes_non_ascii() {
        let project = "/tmp/项目";
        let uri = "file:///tmp/%E9%A1%B9%E7%9B%AE/src/%E6%B5%8B%E8%AF%95.ts";
        let rel = uri_to_relative_path(uri, project);
        assert_eq!(rel, "src/测试.ts");
    }

    #[test]
    fn test_index_languages_for_typescript_server() {
        let mapped = index_languages_for_server("typescript");
        assert_eq!(mapped, vec!["typescript", "javascript"]);
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_space() {
        let path = Path::new("/tmp/demo project/src/main.rs");
        let uri = path_to_lsp_uri(path).expect("uri conversion");
        assert!(uri.as_str().contains("demo%20project"));
    }
}
