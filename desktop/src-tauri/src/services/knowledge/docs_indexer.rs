//! Documentation File Indexer
//!
//! Automatically discovers and indexes documentation files (.md, .mdx, .txt,
//! .pdf, .doc, .docx) from a workspace into a dedicated "[Docs]" knowledge
//! collection. Watches for file changes and notifies the frontend so users
//! can trigger incremental sync.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use tauri::Emitter;
use tokio::sync::RwLock;

use crate::services::knowledge::chunker::Document;
use crate::services::knowledge::pipeline::{KnowledgeCollection, RagPipeline};
use crate::utils::error::{AppError, AppResult};

/// File extensions treated as documentation.
const DOC_EXTENSIONS: &[&str] = &["md", "mdx", "txt", "pdf", "doc", "docx"];

/// File names to exclude from documentation indexing (case-insensitive).
/// These are AI tool configuration files that are not general documentation.
const EXCLUDED_FILENAMES: &[&str] = &[
    "claude.md",
    "agents.md",
    "memory.md",
    "copilot-instructions.md",
    ".cursorrules",
    "rules.md",
];

/// Prefix for auto-created docs collections.
const DOCS_COLLECTION_PREFIX: &str = "[Docs] ";

/// Tauri event emitted when doc file changes are detected.
const DOCS_CHANGES_EVENT: &str = "knowledge:docs-changes-detected";

/// Status of a docs knowledge base for a workspace.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocsKbStatus {
    pub collection_id: Option<String>,
    pub collection_name: Option<String>,
    pub total_docs: usize,
    pub pending_changes: Vec<String>,
    /// "none" | "indexing" | "indexed" | "changes_pending"
    pub status: String,
}

/// Manages per-workspace documentation file watchers and change queues.
pub struct DocsIndexer {
    /// File watchers keyed by normalized workspace path.
    watchers: RwLock<HashMap<String, notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>>>,
    /// Pending file changes keyed by workspace path.
    pending_changes: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// App handle for emitting events.
    app_handle: RwLock<Option<tauri::AppHandle>>,
}

impl DocsIndexer {
    pub fn new() -> Self {
        Self {
            watchers: RwLock::new(HashMap::new()),
            pending_changes: Arc::new(RwLock::new(HashMap::new())),
            app_handle: RwLock::new(None),
        }
    }

    /// Set the app handle (called once during initialization).
    pub async fn set_app_handle(&self, handle: tauri::AppHandle) {
        *self.app_handle.write().await = Some(handle);
    }

    /// Check if a file name should be excluded from documentation indexing.
    fn is_excluded_filename(path: &Path) -> bool {
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_lowercase(),
            None => return false,
        };
        EXCLUDED_FILENAMES.contains(&file_name.as_str())
    }

    /// Scan a workspace for documentation files, respecting .gitignore.
    ///
    /// Excludes hidden files/directories and AI tool config files
    /// (CLAUDE.md, AGENTS.md, MEMORY.md, etc.).
    pub fn scan_doc_files(workspace_path: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        if !workspace_path.is_dir() {
            return files;
        }

        let walker = ignore::WalkBuilder::new(workspace_path)
            .hidden(true)
            .git_ignore(true)
            .build();

        for entry in walker.flatten() {
            if entry.file_type().map_or(true, |ft| !ft.is_file()) {
                continue;
            }
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !DOC_EXTENSIONS.contains(&ext) {
                continue;
            }
            if Self::is_excluded_filename(path) {
                tracing::debug!(path = %path.display(), "Excluded AI config file from docs indexing");
                continue;
            }
            files.push(path.to_path_buf());
        }

        files.sort();
        files
    }

    /// Generate the collection name for a workspace.
    pub fn collection_name_for_workspace(workspace_path: &str) -> String {
        let dir_name = Path::new(workspace_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");
        format!("{}{}", DOCS_COLLECTION_PREFIX, dir_name)
    }

    /// Build Document objects from a list of scanned file paths.
    fn build_documents(doc_files: &[PathBuf]) -> Vec<Document> {
        let mut documents = Vec::new();
        for file_path in doc_files {
            let ext = file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("txt");

            let content = match RagPipeline::read_file_content(file_path, ext) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %file_path.display(), error = %e, "Skipping unreadable doc file");
                    continue;
                }
            };

            if content.trim().is_empty() {
                tracing::debug!(path = %file_path.display(), "Skipping empty doc file");
                continue;
            }

            let doc_id = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let abs_path = file_path.to_string_lossy().to_string();

            documents.push(Document::from_parsed_content(
                doc_id,
                content,
                abs_path,
                ext.to_string(),
            ));
        }
        documents
    }

    /// Ensure a docs collection exists for a workspace, creating one if needed.
    ///
    /// Returns the collection if docs were found, or None if no doc files exist.
    /// If the collection already exists but is empty (0 chunks from a prior
    /// failed attempt), re-ingests the documents.
    /// Always ensures `workspace_path` is set on the collection.
    pub async fn ensure_docs_collection(
        &self,
        pipeline: &RagPipeline,
        workspace_path: &str,
        project_id: &str,
        app_handle: Option<&tauri::AppHandle>,
    ) -> AppResult<Option<KnowledgeCollection>> {
        let ws_path = Path::new(workspace_path);
        let doc_files = Self::scan_doc_files(ws_path);

        tracing::info!(
            workspace = %workspace_path,
            doc_files = doc_files.len(),
            "Docs indexer: scanned workspace"
        );

        if doc_files.is_empty() {
            return Ok(None);
        }

        let collection_name = Self::collection_name_for_workspace(workspace_path);

        // Check if collection already exists
        let collections = pipeline.list_collections(project_id)?;
        let existing = collections.iter().find(|c| c.name == collection_name);

        if let Some(col) = existing {
            // Collection exists — ensure workspace_path is set
            let col = if col.workspace_path.is_none()
                || col.workspace_path.as_deref() != Some(workspace_path)
            {
                tracing::info!(
                    collection = %col.name,
                    "Setting missing workspace_path on existing docs collection"
                );
                pipeline.update_collection(
                    &col.id,
                    None,
                    None,
                    Some(Some(workspace_path)),
                )?
            } else {
                col.clone()
            };

            // If collection is empty (prior failed ingest), re-ingest
            if col.chunk_count == 0 {
                tracing::info!(
                    collection = %col.name,
                    "Docs collection has 0 chunks, re-ingesting documents"
                );
                let documents = Self::build_documents(&doc_files);
                if !documents.is_empty() {
                    let updated = pipeline
                        .ingest_with_progress(
                            &collection_name,
                            project_id,
                            &col.description,
                            documents,
                            app_handle,
                        )
                        .await?;
                    self.start_doc_watcher(workspace_path).await;
                    return Ok(Some(updated));
                }
            }

            self.start_doc_watcher(workspace_path).await;
            return Ok(Some(col));
        }

        // Build documents
        let documents = Self::build_documents(&doc_files);

        tracing::info!(
            documents = documents.len(),
            "Docs indexer: built documents for ingestion"
        );

        if documents.is_empty() {
            return Ok(None);
        }

        // Ingest
        let description = format!("Auto-indexed documentation from {}", workspace_path);
        let collection = pipeline
            .ingest_with_progress(
                &collection_name,
                project_id,
                &description,
                documents,
                app_handle,
            )
            .await?;

        tracing::info!(
            collection = %collection.name,
            chunk_count = collection.chunk_count,
            "Docs indexer: ingestion complete"
        );

        // Set workspace_path on the collection
        let collection = pipeline.update_collection(
            &collection.id,
            None,
            None,
            Some(Some(workspace_path)),
        )?;

        // Start file watcher
        self.start_doc_watcher(workspace_path).await;

        Ok(Some(collection))
    }

    /// Start watching a workspace for documentation file changes.
    pub async fn start_doc_watcher(&self, workspace_path: &str) {
        let normalized = Self::normalize_path(workspace_path);

        // Skip if already watching
        {
            let watchers = self.watchers.read().await;
            if watchers.contains_key(&normalized) {
                return;
            }
        }

        let ws_path = PathBuf::from(workspace_path);
        if !ws_path.is_dir() {
            return;
        }

        let pending = Arc::clone(&self.pending_changes);
        let ws_key = normalized.clone();
        let app_handle = self.app_handle.read().await.clone();

        let debouncer = new_debouncer(
            std::time::Duration::from_millis(500),
            move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                let Ok(events) = events else { return };

                let mut changed = Vec::new();
                for event in events {
                    if event.kind != DebouncedEventKind::Any {
                        continue;
                    }
                    let path = &event.path;
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    if !DOC_EXTENSIONS.contains(&ext) {
                        continue;
                    }
                    // Also filter excluded files in the watcher
                    if Self::is_excluded_filename(path) {
                        continue;
                    }
                    changed.push(path.to_string_lossy().to_string());
                }

                if changed.is_empty() {
                    return;
                }

                // Add to pending changes
                let pending = pending.clone();
                let ws_key = ws_key.clone();
                let handle = app_handle.clone();
                let changed_clone = changed.clone();

                // Use try_write since we're in a sync callback
                if let Ok(mut guard) = pending.try_write() {
                    let set = guard.entry(ws_key.clone()).or_default();
                    for path in &changed {
                        set.insert(path.clone());
                    }
                }

                // Emit event to frontend
                if let Some(ref h) = handle {
                    let _ = h.emit(
                        DOCS_CHANGES_EVENT,
                        serde_json::json!({
                            "workspace_path": ws_key,
                            "changed_files": changed_clone,
                        }),
                    );
                }
            },
        );

        match debouncer {
            Ok(mut watcher) => {
                if watcher
                    .watcher()
                    .watch(&ws_path, notify::RecursiveMode::Recursive)
                    .is_ok()
                {
                    self.watchers.write().await.insert(normalized, watcher);
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create docs file watcher");
            }
        }
    }

    /// Drain and return pending changes for a workspace.
    pub async fn take_pending_changes(&self, workspace_path: &str) -> Vec<String> {
        let normalized = Self::normalize_path(workspace_path);
        let mut guard = self.pending_changes.write().await;
        guard
            .remove(&normalized)
            .map(|set| set.into_iter().collect())
            .unwrap_or_default()
    }

    /// Get pending changes for a workspace without draining.
    pub async fn peek_pending_changes(&self, workspace_path: &str) -> Vec<String> {
        let normalized = Self::normalize_path(workspace_path);
        let guard = self.pending_changes.read().await;
        guard
            .get(&normalized)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Remove watcher and pending changes for a workspace.
    pub async fn remove_workspace(&self, workspace_path: &str) {
        let normalized = Self::normalize_path(workspace_path);
        self.watchers.write().await.remove(&normalized);
        self.pending_changes.write().await.remove(&normalized);
    }

    fn normalize_path(p: &str) -> String {
        p.replace('\\', "/").trim_end_matches('/').to_string()
    }
}

impl Default for DocsIndexer {
    fn default() -> Self {
        Self::new()
    }
}
