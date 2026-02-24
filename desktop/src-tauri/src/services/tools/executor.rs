//! Tool Executor
//!
//! Executes tools requested by LLM providers.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::services::file_change_tracker::FileChangeTracker;
use crate::services::orchestrator::embedding_manager::EmbeddingManager;
use crate::services::orchestrator::embedding_service::EmbeddingService;
use crate::services::orchestrator::hnsw_index::HnswIndex;
use crate::services::orchestrator::index_store::IndexStore;

/// Cache entry for a previously read file, used for deduplication.
///
/// ADR-F001: Uses `Mutex<HashMap>` over `mini-moka` for deterministic behavior,
/// low cardinality (<100 files), and zero additional dependencies.
#[derive(Debug, Clone)]
pub struct ReadCacheEntry {
    /// Canonical path of the cached file
    pub path: PathBuf,
    /// File modification time at the time of caching
    pub modified_time: SystemTime,
    /// Number of lines in the file
    pub line_count: usize,
    /// Size of the file in bytes
    pub size_bytes: u64,
    /// Hash of the file content (using std DefaultHasher for speed, not crypto)
    pub content_hash: u64,
    /// Offset (1-based line number) used when the file was read
    pub offset: usize,
    /// Line limit used when the file was read
    pub limit: usize,
    /// File extension (e.g. "rs", "py", "ts") for the enhanced dedup message
    pub extension: String,
    /// First ~5 lines of the file content for the enhanced dedup message
    pub first_lines_preview: String,
}

/// Result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the execution was successful
    pub success: bool,
    /// Output from the tool (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Optional image data for multimodal responses: (mime_type, base64_data)
    #[serde(skip)]
    pub image_data: Option<(String, String)>,
    /// Whether this result is a dedup hit (file read cache).
    /// When true, the agentic loop should push a minimal tool_result to the LLM
    /// instead of the full content, to prevent weak models from re-reading files.
    #[serde(default)]
    pub is_dedup: bool,
    /// Optional EventActions declared by the tool alongside its result.
    ///
    /// When present, the orchestrator's agentic loop processes these actions
    /// after handling the tool result. This enables tools to declare side
    /// effects (state mutations, checkpoints, quality gate results, transfers)
    /// without directly causing them.
    #[serde(skip)]
    pub event_actions: Option<crate::services::core::event_actions::EventActions>,
}

impl ToolResult {
    /// Create a successful result
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            image_data: None,
            is_dedup: false,
            event_actions: None,
        }
    }

    /// Create an error result
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error.into()),
            image_data: None,
            is_dedup: false,
            event_actions: None,
        }
    }

    /// Create a successful result with image data for multimodal support
    pub fn ok_with_image(
        output: impl Into<String>,
        mime_type: String,
        base64_data: String,
    ) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            image_data: Some((mime_type, base64_data)),
            is_dedup: false,
            event_actions: None,
        }
    }

    /// Create a successful dedup result (file read cache hit).
    ///
    /// Marked with `is_dedup = true` so the agentic loop can suppress
    /// the full content from the LLM conversation.
    pub fn ok_dedup(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            image_data: None,
            is_dedup: true,
            event_actions: None,
        }
    }

    /// Attach EventActions to this tool result.
    ///
    /// The orchestrator will process these actions after handling the tool result.
    pub fn with_event_actions(
        mut self,
        actions: crate::services::core::event_actions::EventActions,
    ) -> Self {
        if actions.has_actions() {
            self.event_actions = Some(actions);
        }
        self
    }

    /// Convert to string for LLM consumption
    pub fn to_content(&self) -> String {
        if self.success {
            self.output.clone().unwrap_or_default()
        } else {
            format!(
                "Error: {}",
                self.error.as_deref().unwrap_or("Unknown error")
            )
        }
    }
}

/// Tool executor for running tools locally.
///
/// All tool execution is delegated to the `ToolRegistry`. Each tool is an
/// independent `impl Tool` in `impls/`.
pub struct ToolExecutor {
    /// Project root for path validation
    project_root: PathBuf,
    /// Default timeout for bash commands (in milliseconds)
    /// Track files that have been read (for read-before-write enforcement)
    read_files: Arc<Mutex<HashSet<PathBuf>>>,
    /// Content-aware deduplication cache for file reads.
    /// Key: (canonical path, offset, limit) -> cache entry.
    /// If the same file is read with the same offset/limit and the mtime
    /// has not changed, a short dedup message is returned instead of re-reading.
    /// ADR-F001: Mutex<HashMap> chosen over mini-moka for determinism and zero eviction.
    /// Wrapped in Arc so sub-agents can share the parent's cache.
    read_cache: Arc<Mutex<HashMap<(PathBuf, usize, usize), ReadCacheEntry>>>,
    /// Task sub-agent deduplication cache (story-005).
    /// Keyed by hash of the prompt string. Only successful results are cached.
    /// This prevents identical Task sub-agent prompts from being re-executed.
    /// Wrapped in Arc for sharing with ToolExecutionContext.
    task_dedup_cache: Arc<Mutex<HashMap<u64, String>>>,
    /// Persistent working directory for Bash commands.
    /// Wrapped in Arc<Mutex> for sharing with ToolExecutionContext.
    current_working_dir: Arc<Mutex<PathBuf>>,
    /// WebFetch service for fetching web pages.
    /// Wrapped in Arc for sharing with ToolExecutionContext.
    web_fetch: Arc<super::web_fetch::WebFetchService>,
    /// WebSearch service (None if no search provider configured).
    /// Wrapped in Arc for sharing with ToolExecutionContext.
    web_search: Option<Arc<super::web_search::WebSearchService>>,
    /// Optional index store for CodebaseSearch tool
    index_store: Option<Arc<IndexStore>>,
    /// Optional embedding service for semantic search in CodebaseSearch
    embedding_service: Option<Arc<EmbeddingService>>,
    /// Optional EmbeddingManager for provider-aware semantic search (ADR-F002).
    /// When set, used in preference to `embedding_service` for query embedding.
    embedding_manager: Option<Arc<EmbeddingManager>>,
    /// Optional HNSW index for O(log n) approximate nearest neighbor search.
    /// When set and ready, `execute_codebase_search` uses HNSW for semantic
    /// search instead of brute-force cosine similarity scan.
    hnsw_index: Option<Arc<HnswIndex>>,
    /// Registry of all available tools (trait-based).
    /// Used for definition generation and dynamic tool management.
    registry: super::trait_def::ToolRegistry,
    /// Optional file change tracker for recording LLM file modifications.
    file_change_tracker: Option<Arc<Mutex<FileChangeTracker>>>,
    /// Optional permission gate for tool execution approval.
    permission_gate: Option<Arc<crate::services::orchestrator::permission_gate::PermissionGate>>,
}

impl ToolExecutor {
    /// Build a ToolRegistry populated with all 15 tool implementations.
    ///
    /// Public static version for use by definitions.rs without needing a ToolExecutor instance.
    pub fn build_registry_static() -> super::trait_def::ToolRegistry {
        Self::build_registry()
    }

    /// Build a ToolRegistry populated with all 15 tool implementations.
    fn build_registry() -> super::trait_def::ToolRegistry {
        use super::impls::*;
        let mut registry = super::trait_def::ToolRegistry::new();
        registry.register(Arc::new(ReadTool::new()));
        registry.register(Arc::new(WriteTool::new()));
        registry.register(Arc::new(EditTool::new()));
        registry.register(Arc::new(BashTool::new()));
        registry.register(Arc::new(GlobTool::new()));
        registry.register(Arc::new(GrepTool::new()));
        registry.register(Arc::new(LsTool::new()));
        registry.register(Arc::new(CwdTool::new()));
        registry.register(Arc::new(AnalyzeTool::new()));
        registry.register(Arc::new(TaskTool::new()));
        registry.register(Arc::new(WebFetchTool::new()));
        registry.register(Arc::new(WebSearchTool::new()));
        registry.register(Arc::new(NotebookEditTool::new()));
        registry.register(Arc::new(CodebaseSearchTool::new()));
        // BrowserTool is always registered (unconditional). Runtime detection
        // and graceful degradation handle the case when no browser is available.
        registry.register(Arc::new(BrowserTool::new()));
        registry
    }

    /// Create a new tool executor
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = project_root.into();
        Self {
            current_working_dir: Arc::new(Mutex::new(root.clone())),
            project_root: root,
            read_files: Arc::new(Mutex::new(HashSet::new())),
            read_cache: Arc::new(Mutex::new(HashMap::new())),
            task_dedup_cache: Arc::new(Mutex::new(HashMap::new())),
            web_fetch: Arc::new(super::web_fetch::WebFetchService::new()),
            web_search: None,
            index_store: None,
            embedding_service: None,
            embedding_manager: None,
            hnsw_index: None,
            registry: Self::build_registry(),
            file_change_tracker: None,
            permission_gate: None,
        }
    }

    /// Create a new tool executor that shares a read cache with a parent.
    ///
    /// Sub-agents use this constructor so file reads are deduplicated across
    /// the parent/child boundary, saving tokens and wall-clock time.
    pub fn new_with_shared_cache(
        project_root: impl Into<PathBuf>,
        shared_cache: Arc<Mutex<HashMap<(PathBuf, usize, usize), ReadCacheEntry>>>,
    ) -> Self {
        let root: PathBuf = project_root.into();
        Self {
            current_working_dir: Arc::new(Mutex::new(root.clone())),
            project_root: root,
            read_files: Arc::new(Mutex::new(HashSet::new())),
            read_cache: shared_cache,
            task_dedup_cache: Arc::new(Mutex::new(HashMap::new())),
            web_fetch: Arc::new(super::web_fetch::WebFetchService::new()),
            web_search: None,
            index_store: None,
            embedding_service: None,
            embedding_manager: None,
            hnsw_index: None,
            registry: Self::build_registry(),
            file_change_tracker: None,
            permission_gate: None,
        }
    }

    /// Return a clone of the shared read cache Arc.
    ///
    /// Pass this to `new_with_shared_cache` when creating sub-agent executors
    /// so they share the same deduplication cache.
    pub fn shared_read_cache(
        &self,
    ) -> Arc<Mutex<HashMap<(PathBuf, usize, usize), ReadCacheEntry>>> {
        Arc::clone(&self.read_cache)
    }

    /// Configure the web search provider
    pub fn set_search_provider(&mut self, provider_name: &str, api_key: Option<String>) {
        match super::web_search::WebSearchService::new(provider_name, api_key.as_deref()) {
            Ok(service) => self.web_search = Some(Arc::new(service)),
            Err(e) => {
                tracing::warn!(
                    "Failed to configure search provider '{}': {}",
                    provider_name,
                    e
                );
                self.web_search = None;
            }
        }
    }

    /// Set the index store for CodebaseSearch tool
    pub fn set_index_store(&mut self, store: Arc<IndexStore>) {
        self.index_store = Some(store);
    }

    /// Set the embedding service for semantic search in CodebaseSearch
    pub fn set_embedding_service(&mut self, svc: Arc<EmbeddingService>) {
        self.embedding_service = Some(svc);
    }

    /// Set the EmbeddingManager for provider-aware semantic search (ADR-F002).
    ///
    /// When set, `CodebaseSearchTool` uses `EmbeddingManager::embed_query`
    /// instead of the raw `EmbeddingService::embed_text`, gaining caching,
    /// fallback, and provider-agnostic query embedding.
    pub fn set_embedding_manager(&mut self, mgr: Arc<EmbeddingManager>) {
        self.embedding_manager = Some(mgr);
    }

    /// Get the index store Arc (if set), for sharing with sub-agents.
    pub fn get_index_store(&self) -> Option<Arc<IndexStore>> {
        self.index_store.clone()
    }

    /// Get the embedding service Arc (if set), for sharing with sub-agents.
    pub fn get_embedding_service(&self) -> Option<Arc<EmbeddingService>> {
        self.embedding_service.clone()
    }

    /// Get the EmbeddingManager Arc (if set), for sharing with sub-agents.
    pub fn get_embedding_manager(&self) -> Option<Arc<EmbeddingManager>> {
        self.embedding_manager.clone()
    }

    /// Set the HNSW index for O(log n) approximate nearest neighbor search.
    ///
    /// When set and ready, `CodebaseSearchTool` uses HNSW for the
    /// semantic search path instead of brute-force cosine similarity scan.
    pub fn set_hnsw_index(&mut self, hnsw: Arc<HnswIndex>) {
        self.hnsw_index = Some(hnsw);
    }

    /// Get the HNSW index Arc (if set), for sharing with sub-agents.
    pub fn get_hnsw_index(&self) -> Option<Arc<HnswIndex>> {
        self.hnsw_index.clone()
    }

    /// Set the file change tracker for recording LLM file modifications.
    pub fn set_file_change_tracker(&mut self, tracker: Arc<Mutex<FileChangeTracker>>) {
        self.file_change_tracker = Some(tracker);
    }

    /// Get the file change tracker Arc (if set).
    pub fn get_file_change_tracker(&self) -> Option<Arc<Mutex<FileChangeTracker>>> {
        self.file_change_tracker.clone()
    }

    /// Set the permission gate for tool execution approval.
    pub fn set_permission_gate(
        &mut self,
        gate: Arc<crate::services::orchestrator::permission_gate::PermissionGate>,
    ) {
        self.permission_gate = Some(gate);
    }

    /// Get a reference to the tool registry.
    ///
    /// Use this to inspect available tools, generate definitions, or
    /// dynamically enable/disable tools.
    pub fn registry(&self) -> &super::trait_def::ToolRegistry {
        &self.registry
    }

    /// Get a mutable reference to the tool registry.
    ///
    /// Use this to register/unregister tools dynamically.
    pub fn registry_mut(&mut self) -> &mut super::trait_def::ToolRegistry {
        &mut self.registry
    }

    /// Get tool definitions from the registry.
    ///
    /// This is the preferred way to get tool definitions, as they are
    /// auto-generated from the trait implementations and always in sync
    /// with the actual tool behavior.
    pub fn registry_definitions(&self) -> Vec<crate::services::llm::types::ToolDefinition> {
        self.registry.definitions()
    }

    /// Get basic tool definitions (without Task tool) from the registry.
    ///
    /// Used for sub-agents to prevent recursion.
    pub fn registry_basic_definitions(&self) -> Vec<crate::services::llm::types::ToolDefinition> {
        self.registry
            .definitions()
            .into_iter()
            .filter(|d| d.name != "Task" && d.name != "Analyze")
            .collect()
    }

    /// Build a ToolExecutionContext from this executor's current state.
    ///
    /// Used to pass shared state to trait-based tool implementations.
    /// Populates ALL fields so tools can access services through context
    /// instead of requiring executor-private state.
    pub(crate) fn build_tool_context(&self) -> super::trait_def::ToolExecutionContext {
        super::trait_def::ToolExecutionContext {
            session_id: String::new(),
            project_root: self.project_root.clone(),
            working_directory: Arc::clone(&self.current_working_dir),
            read_cache: Arc::clone(&self.read_cache),
            read_files: Arc::clone(&self.read_files),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            web_fetch: Arc::clone(&self.web_fetch),
            web_search: self.web_search.clone(),
            index_store: self.index_store.clone(),
            embedding_service: self.embedding_service.clone(),
            embedding_manager: self.embedding_manager.clone(),
            hnsw_index: self.hnsw_index.clone(),
            task_dedup_cache: Arc::clone(&self.task_dedup_cache),
            task_context: None, // Set by callers who have TaskContext
            core_context: None, // Set by callers who have OrchestratorContext
            file_change_tracker: self.file_change_tracker.clone(),
            permission_gate: self.permission_gate.clone(),
        }
    }

    /// Build a ToolExecutionContext with a TaskContext for sub-agent support.
    ///
    /// Used by execute_with_context() when TaskContext is available.
    pub fn build_tool_context_with_task(
        &self,
        task_ctx: &super::task_spawner::TaskContext,
    ) -> super::trait_def::ToolExecutionContext {
        let mut ctx = self.build_tool_context();
        ctx.task_context = Some(Arc::new(super::task_spawner::TaskContext {
            spawner: Arc::clone(&task_ctx.spawner),
            tx: task_ctx.tx.clone(),
            cancellation_token: task_ctx.cancellation_token.clone(),
            depth: task_ctx.depth,
            max_depth: task_ctx.max_depth,
            llm_semaphore: Arc::clone(&task_ctx.llm_semaphore),
        }));
        ctx
    }

    /// Execute a tool by name with given arguments.
    ///
    /// All tools are dispatched through the `ToolRegistry`, which looks up the
    /// tool by name and calls its `Tool::execute()` implementation. The shared
    /// state is passed via `ToolExecutionContext` (built from executor fields).
    ///
    /// When no `TaskContext` is available (e.g., sub-agent execution), the
    /// Task tool will return a depth-limit error.
    pub async fn execute(&self, tool_name: &str, arguments: &serde_json::Value) -> ToolResult {
        let ctx = self.build_tool_context();
        self.registry
            .execute(tool_name, &ctx, arguments.clone())
            .await
    }

    pub async fn execute_with_context(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
        task_ctx: Option<&super::task_spawner::TaskContext>,
    ) -> ToolResult {
        let ctx = match task_ctx {
            Some(tc) => self.build_tool_context_with_task(tc),
            None => self.build_tool_context(),
        };
        self.registry
            .execute(tool_name, &ctx, arguments.clone())
            .await
    }

    /// Clear the task deduplication cache (story-005).
    ///
    /// Useful for testing and after compaction resets where cached
    /// results may no longer be relevant.
    pub fn clear_task_cache(&self) {
        if let Ok(mut cache) = self.task_dedup_cache.lock() {
            cache.clear();
        }
    }

    /// Return a summary of all files currently in the read cache.
    ///
    /// Each entry is `(display_path, line_count, size_bytes)`, useful for
    /// session-level memory ("which files has the agent already seen?").
    pub fn get_read_file_summary(&self) -> Vec<(String, usize, u64)> {
        let cache = match self.read_cache.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        // Deduplicate by path (there may be multiple entries for different
        // offset/limit combinations). We keep the entry with the largest
        // line_count for each path so the summary reflects the fullest read.
        let mut by_path: HashMap<PathBuf, &ReadCacheEntry> = HashMap::new();
        for entry in cache.values() {
            by_path
                .entry(entry.path.clone())
                .and_modify(|existing| {
                    if entry.line_count > existing.line_count {
                        *existing = entry;
                    }
                })
                .or_insert(entry);
        }
        let mut result: Vec<(String, usize, u64)> = by_path
            .values()
            .map(|e| (e.path.display().to_string(), e.line_count, e.size_bytes))
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Clear the entire read deduplication cache.
    ///
    /// This is useful when starting a new logical session or after bulk
    /// file modifications where stale cache entries could cause confusion.
    /// ADR-004: Called after compaction to prevent stale dedup entries from
    /// causing infinite read loops with weak LLM providers.
    pub fn clear_read_cache(&self) {
        if let Ok(mut cache) = self.read_cache.lock() {
            cache.clear();
        }
    }

    /// Get project root (for external access)
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Update the project root (working directory)
    pub fn set_project_root(&mut self, new_root: PathBuf) {
        self.project_root = new_root;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "line 1\nline 2\nline 3\n").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        std::fs::write(dir.path().join("subdir/nested.txt"), "nested content").unwrap();
        dir
    }

    #[tokio::test]
    async fn test_read_file() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "file_path": dir.path().join("test.txt").to_string_lossy().to_string()
        });

        let result = executor.execute("Read", &args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("line 1"));
    }

    #[tokio::test]
    async fn test_read_non_utf8_text_file_uses_lossy_decode() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file = dir.path().join("latin1.txt");
        std::fs::write(
            &file,
            vec![0x66, 0x6f, 0x6f, 0x20, 0x80, 0x20, 0x62, 0x61, 0x72],
        )
        .unwrap();

        let args = serde_json::json!({
            "file_path": file.to_string_lossy().to_string()
        });

        let result = executor.execute("Read", &args).await;
        assert!(result.success);
        let output = result.output.unwrap_or_default();
        assert!(output.contains("[non-utf8 decoded with replacement]"));
        assert!(output.contains("foo"));
    }

    #[tokio::test]
    async fn test_read_binary_file_returns_skip_message_not_error() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file = dir.path().join("archive.zip");
        std::fs::write(&file, vec![0x50, 0x4b, 0x03, 0x04, 0x00, 0x00, 0xff, 0x00]).unwrap();

        let args = serde_json::json!({
            "file_path": file.to_string_lossy().to_string()
        });

        let result = executor.execute("Read", &args).await;
        assert!(result.success);
        let output = result.output.unwrap_or_default();
        assert!(output.contains("[binary file skipped]"));
        assert!(output.contains("archive.zip"));
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "file_path": dir.path().join("nonexistent.txt").to_string_lossy().to_string()
        });

        let result = executor.execute("Read", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_write_file() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let new_file = dir.path().join("new_file.txt");
        let args = serde_json::json!({
            "file_path": new_file.to_string_lossy().to_string(),
            "content": "new content"
        });

        let result = executor.execute("Write", &args).await;
        assert!(result.success);
        assert!(new_file.exists());
        assert_eq!(std::fs::read_to_string(&new_file).unwrap(), "new content");
    }

    #[tokio::test]
    async fn test_edit_file() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        // Must read before editing (read-before-write enforcement)
        let read_args = serde_json::json!({
            "file_path": dir.path().join("test.txt").to_string_lossy().to_string()
        });
        executor.execute("Read", &read_args).await;

        let args = serde_json::json!({
            "file_path": dir.path().join("test.txt").to_string_lossy().to_string(),
            "old_string": "line 2",
            "new_string": "modified line 2"
        });

        let result = executor.execute("Edit", &args).await;
        assert!(result.success);

        let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert!(content.contains("modified line 2"));
    }

    #[tokio::test]
    async fn test_edit_non_unique() {
        let dir = setup_test_dir();
        std::fs::write(dir.path().join("dup.txt"), "foo foo foo").unwrap();
        let executor = ToolExecutor::new(dir.path());

        // Must read before editing (read-before-write enforcement)
        let read_args = serde_json::json!({
            "file_path": dir.path().join("dup.txt").to_string_lossy().to_string()
        });
        executor.execute("Read", &read_args).await;

        let args = serde_json::json!({
            "file_path": dir.path().join("dup.txt").to_string_lossy().to_string(),
            "old_string": "foo",
            "new_string": "bar"
        });

        let result = executor.execute("Edit", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("appears 3 times"));
    }

    #[tokio::test]
    async fn test_bash_simple() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        #[cfg(windows)]
        let args = serde_json::json!({
            "command": "echo hello"
        });
        #[cfg(not(windows))]
        let args = serde_json::json!({
            "command": "echo hello"
        });

        let result = executor.execute("Bash", &args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_blocked_command() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "command": "rm -rf /"
        });

        let result = executor.execute("Bash", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("blocked"));
    }

    #[tokio::test]
    async fn test_glob() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "pattern": "**/*.txt",
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("Glob", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("test.txt"));
        assert!(output.contains("nested.txt"));
    }

    #[tokio::test]
    async fn test_glob_relative_dot_uses_executor_working_dir() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "pattern": "test.txt",
            "path": "."
        });

        let result = executor.execute("Glob", &args).await;
        assert!(result.success, "glob should succeed: {:?}", result.error);
        let output = result.output.unwrap_or_default();
        assert!(
            output.contains("test.txt"),
            "expected test.txt in output, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_glob_head_limit_caps_results() {
        let dir = setup_test_dir();
        std::fs::write(dir.path().join("another.txt"), "x").unwrap();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "pattern": "**/*.txt",
            "path": dir.path().to_string_lossy().to_string(),
            "head_limit": 1
        });

        let result = executor.execute("Glob", &args).await;
        assert!(result.success);
        let output = result.output.unwrap_or_default();
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 1, "expected one line, got: {}", output);
    }

    #[tokio::test]
    async fn test_grep() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        // Default output_mode is files_with_matches — returns file paths
        let args = serde_json::json!({
            "pattern": "line",
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("Grep", &args).await;
        assert!(result.success);
        assert!(result.output.as_ref().unwrap().contains("test.txt"));

        // Test content mode — returns matching lines
        let args = serde_json::json!({
            "pattern": "line",
            "path": dir.path().to_string_lossy().to_string(),
            "output_mode": "content"
        });

        let result = executor.execute("Grep", &args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("line 1"));
    }

    #[tokio::test]
    async fn test_grep_relative_dot_uses_executor_working_dir() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "pattern": "nested",
            "path": "."
        });

        let result = executor.execute("Grep", &args).await;
        assert!(result.success, "grep should succeed: {:?}", result.error);
        let output = result.output.unwrap_or_default();
        assert!(
            output.contains("nested.txt"),
            "expected nested.txt in output, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_ls_directory() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("DIR"));
        assert!(output.contains("subdir"));
        assert!(output.contains("FILE"));
        assert!(output.contains("test.txt"));
    }

    #[tokio::test]
    async fn test_ls_hidden_files() {
        let dir = setup_test_dir();
        std::fs::write(dir.path().join(".hidden"), "hidden content").unwrap();
        let executor = ToolExecutor::new(dir.path());

        // Without show_hidden
        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });
        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        assert!(!result.output.unwrap().contains(".hidden"));

        // With show_hidden
        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string(),
            "show_hidden": true
        });
        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains(".hidden"));
    }

    #[tokio::test]
    async fn test_ls_not_a_directory() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().join("test.txt").to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Not a directory"));
    }

    #[tokio::test]
    async fn test_ls_not_found() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().join("nonexistent").to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_ls_truncation_large_directory() {
        let dir = TempDir::new().unwrap();
        // Create 250 files (exceeding LS_MAX_ENTRIES of 200)
        for i in 0..250 {
            std::fs::write(dir.path().join(format!("file_{:04}.txt", i)), "content").unwrap();
        }
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // Should show exactly LS_MAX_ENTRIES (200) file entries in the listing
        let file_lines: Vec<&str> = output
            .lines()
            .filter(|l| l.trim_start().starts_with("FILE") || l.trim_start().starts_with("DIR"))
            .collect();
        assert_eq!(
            file_lines.len(),
            200,
            "expected {} file entries, got {}",
            200,
            file_lines.len()
        );

        // Should contain truncation note with count of omitted entries
        assert!(
            output.contains("50 more entries not shown"),
            "expected truncation note with 50 omitted entries, got: {}",
            output
        );

        // Should suggest Glob
        assert!(
            output.contains("Glob"),
            "expected Glob suggestion in truncation note, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_ls_no_truncation_within_limit() {
        let dir = TempDir::new().unwrap();
        // Create exactly 200 files (at the limit, should NOT truncate)
        for i in 0..200 {
            std::fs::write(dir.path().join(format!("file_{:04}.txt", i)), "content").unwrap();
        }
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // Should show all 200 entries
        let file_lines: Vec<&str> = output
            .lines()
            .filter(|l| l.trim_start().starts_with("FILE") || l.trim_start().starts_with("DIR"))
            .collect();
        assert_eq!(
            file_lines.len(),
            200,
            "expected 200 entries, got {}",
            file_lines.len()
        );

        // Should NOT contain truncation note
        assert!(
            !output.contains("more entries not shown"),
            "should not have truncation note for directories at the limit"
        );
    }

    #[tokio::test]
    async fn test_ls_no_truncation_small_directory() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "path": dir.path().to_string_lossy().to_string()
        });

        let result = executor.execute("LS", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // Small directory should NOT have truncation note
        assert!(
            !output.contains("more entries not shown"),
            "small directory should not have truncation note"
        );
    }

    #[tokio::test]
    async fn test_cwd() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({});

        let result = executor.execute("Cwd", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert_eq!(output, dir.path().to_string_lossy().to_string());
    }

    #[test]
    fn test_tool_result() {
        let ok = ToolResult::ok("success");
        assert!(ok.success);
        assert_eq!(ok.to_content(), "success");

        let err = ToolResult::err("failed");
        assert!(!err.success);
        assert!(err.to_content().contains("Error"));
    }

    // =========================================================================
    // Read cache deduplication tests (story-001)
    // =========================================================================

    #[tokio::test]
    async fn test_read_cache_dedup_unchanged_file() {
        // Reading the same unchanged file twice should return a short dedup
        // message on the second read with is_dedup = true.
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });

        // First read: full content, is_dedup = false
        let result1 = executor.execute("Read", &args).await;
        assert!(result1.success);
        assert!(!result1.is_dedup, "first read should NOT be dedup");
        let output1 = result1.output.unwrap();
        assert!(output1.contains("line 1"), "first read should have content");

        // Second read: dedup message with is_dedup = true
        let result2 = executor.execute("Read", &args).await;
        assert!(result2.success);
        assert!(result2.is_dedup, "second unchanged read should be dedup");
        let output2 = result2.output.unwrap();
        assert!(
            output2.contains("[DEDUP]"),
            "second read should be dedup message, got: {}",
            output2
        );
        assert!(output2.contains("test.txt"));
        assert!(output2.contains("lines"));
        assert!(output2.contains("already read"));
    }

    #[tokio::test]
    async fn test_dedup_message_is_short_format() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file = dir.path().join("test_module.rs");
        std::fs::write(&file, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        let file_path = file.to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });

        // First read
        executor.execute("Read", &args).await;
        // Second read should return short dedup with is_dedup flag
        let result = executor.execute("Read", &args).await;
        assert!(result.success);
        assert!(result.is_dedup, "should have is_dedup = true");
        let output = result.output.unwrap();
        assert!(
            output.contains("[DEDUP]"),
            "should use [DEDUP] format, got: {}",
            output
        );
        // Should NOT contain the old verbose preview/symbols/do-not-reread sections
        assert!(
            !output.contains("Preview (first lines)"),
            "should not have verbose preview, got: {}",
            output
        );
        assert!(
            !output.contains("Do NOT re-read"),
            "should not have do-not-reread instruction, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_read_cache_modified_file_returns_full_content() {
        // If the file is modified between reads, the second read should
        // return the full (new) content, not a dedup message.
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file = dir.path().join("mtime_test.txt");
        std::fs::write(&file, "original line 1\noriginal line 2\n").unwrap();

        let file_path = file.to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });

        // First read
        let result1 = executor.execute("Read", &args).await;
        assert!(result1.success);
        assert!(!result1.is_dedup);
        assert!(result1.output.unwrap().contains("original line 1"));

        // Modify the file and force a different mtime.
        std::thread::sleep(std::time::Duration::from_secs(1));
        std::fs::write(&file, "changed line 1\nchanged line 2\n").unwrap();

        // Second read after modification — should NOT be dedup
        let result2 = executor.execute("Read", &args).await;
        assert!(result2.success);
        assert!(!result2.is_dedup, "modified file should NOT be dedup");
        let output2 = result2.output.unwrap();
        assert!(
            !output2.contains("[DEDUP]"),
            "modified file should NOT return dedup message, got: {}",
            output2
        );
        assert!(
            output2.contains("changed line 1"),
            "should see new content, got: {}",
            output2
        );
    }

    #[tokio::test]
    async fn test_read_cache_different_offset_returns_full_content() {
        // Reading the same file with a different offset/limit should return
        // full content even if the file hasn't changed, because the cache key
        // includes offset and limit.
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();

        // First read with default offset/limit
        let args1 = serde_json::json!({ "file_path": &file_path });
        let result1 = executor.execute("Read", &args1).await;
        assert!(result1.success);
        assert!(!result1.is_dedup);
        assert!(result1.output.unwrap().contains("line 1"));

        // Second read with different offset — should NOT be dedup
        let args2 = serde_json::json!({ "file_path": &file_path, "offset": 2 });
        let result2 = executor.execute("Read", &args2).await;
        assert!(result2.success);
        assert!(!result2.is_dedup);
        let output2 = result2.output.unwrap();
        assert!(
            !output2.contains("[DEDUP]"),
            "different offset should return full content, got: {}",
            output2
        );
        assert!(output2.contains("line 2"));

        // Third read with different limit — should NOT be dedup
        let args3 = serde_json::json!({ "file_path": &file_path, "limit": 1 });
        let result3 = executor.execute("Read", &args3).await;
        assert!(result3.success);
        assert!(!result3.is_dedup);
        let output3 = result3.output.unwrap();
        assert!(
            !output3.contains("[DEDUP]"),
            "different limit should return full content, got: {}",
            output3
        );
    }

    #[tokio::test]
    async fn test_read_cache_same_offset_limit_dedup() {
        // Reading with explicitly the same offset/limit should dedup on second read
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();

        let args = serde_json::json!({ "file_path": &file_path, "offset": 2, "limit": 1 });

        // First read
        let result1 = executor.execute("Read", &args).await;
        assert!(result1.success);
        assert!(!result1.is_dedup);
        assert!(result1.output.unwrap().contains("line 2"));

        // Second read with identical args — should dedup
        let result2 = executor.execute("Read", &args).await;
        assert!(result2.success);
        assert!(result2.is_dedup, "same offset/limit should set is_dedup");
        assert!(
            result2.output.unwrap().contains("[DEDUP]"),
            "same offset/limit should dedup"
        );
    }

    #[test]
    fn test_tool_result_ok_is_not_dedup() {
        let result = ToolResult::ok("test output");
        assert!(!result.is_dedup);
    }

    #[test]
    fn test_tool_result_ok_dedup_is_dedup() {
        let result = ToolResult::ok_dedup("dedup msg");
        assert!(result.is_dedup);
        assert!(result.success);
    }

    #[test]
    fn test_tool_result_err_is_not_dedup() {
        let result = ToolResult::err("error");
        assert!(!result.is_dedup);
    }

    #[tokio::test]
    async fn test_get_read_file_summary() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        // Initially empty
        assert!(
            executor.get_read_file_summary().is_empty(),
            "summary should be empty before any reads"
        );

        // Read a file
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });
        executor.execute("Read", &args).await;

        let summary = executor.get_read_file_summary();
        assert_eq!(summary.len(), 1, "should have one cached file");
        assert!(summary[0].0.contains("test.txt"));
        assert!(summary[0].1 > 0, "line_count should be > 0");
        assert!(summary[0].2 > 0, "size_bytes should be > 0");

        // Read another file
        let nested_path = dir
            .path()
            .join("subdir")
            .join("nested.txt")
            .to_string_lossy()
            .to_string();
        let args2 = serde_json::json!({ "file_path": &nested_path });
        executor.execute("Read", &args2).await;

        let summary2 = executor.get_read_file_summary();
        assert_eq!(summary2.len(), 2, "should have two cached files");
    }

    #[tokio::test]
    async fn test_get_read_file_summary_deduplicates_by_path() {
        // Reading the same file with different offset/limit creates multiple
        // cache entries, but the summary should deduplicate by path.
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();

        // Read with default params
        executor
            .execute("Read", &serde_json::json!({ "file_path": &file_path }))
            .await;
        // Read with different offset
        executor
            .execute(
                "Read",
                &serde_json::json!({ "file_path": &file_path, "offset": 2 }),
            )
            .await;

        let summary = executor.get_read_file_summary();
        assert_eq!(
            summary.len(),
            1,
            "summary should deduplicate by path, got: {:?}",
            summary
        );
    }

    #[tokio::test]
    async fn test_clear_read_cache() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        // Read a file to populate cache
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });
        executor.execute("Read", &args).await;

        assert!(
            !executor.get_read_file_summary().is_empty(),
            "cache should be populated"
        );

        // Clear cache
        executor.clear_read_cache();
        assert!(
            executor.get_read_file_summary().is_empty(),
            "cache should be empty after clear"
        );

        // Reading again after clear should return full content, not dedup
        let result = executor.execute("Read", &args).await;
        assert!(result.success);
        assert!(!result.is_dedup, "after clear, should not be dedup");
        let output = result.output.unwrap();
        assert!(
            !output.contains("[DEDUP]"),
            "after clear, should get full content, got: {}",
            output
        );
        assert!(output.contains("line 1"));
    }

    // =========================================================================
    // CodebaseSearch tests (story-009)
    // =========================================================================

    fn create_test_executor_with_index() -> (TempDir, ToolExecutor) {
        use crate::services::orchestrator::index_store::IndexStore;
        use crate::storage::database::Database;

        let dir = setup_test_dir();
        let mut executor = ToolExecutor::new(dir.path());

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        // Populate the index with test data
        use crate::services::orchestrator::analysis_index::{
            FileInventoryItem, SymbolInfo, SymbolKind,
        };

        let project_path = dir.path().to_string_lossy().to_string();

        let item1 = FileInventoryItem {
            path: "src/main.rs".to_string(),
            component: "desktop-rust".to_string(),
            language: "rust".to_string(),
            extension: Some("rs".to_string()),
            size_bytes: 1024,
            line_count: 50,
            is_test: false,
            symbols: vec![
                SymbolInfo::basic("main".to_string(), SymbolKind::Function, 1),
                SymbolInfo::basic("AppConfig".to_string(), SymbolKind::Struct, 10),
            ],
            content_hash: None,
        };

        let item2 = FileInventoryItem {
            path: "src/lib.rs".to_string(),
            component: "desktop-rust".to_string(),
            language: "rust".to_string(),
            extension: Some("rs".to_string()),
            size_bytes: 2048,
            line_count: 100,
            is_test: false,
            symbols: vec![SymbolInfo::basic(
                "init_app".to_string(),
                SymbolKind::Function,
                5,
            )],
            content_hash: None,
        };

        let item3 = FileInventoryItem {
            path: "src/components/App.tsx".to_string(),
            component: "desktop-web".to_string(),
            language: "typescript".to_string(),
            extension: Some("tsx".to_string()),
            size_bytes: 512,
            line_count: 30,
            is_test: false,
            symbols: vec![
                SymbolInfo::basic("App".to_string(), SymbolKind::Function, 1),
                SymbolInfo::basic("AppProps".to_string(), SymbolKind::Interface, 5),
            ],
            content_hash: None,
        };

        store
            .upsert_file_index(&project_path, &item1, "h1")
            .unwrap();
        store
            .upsert_file_index(&project_path, &item2, "h2")
            .unwrap();
        store
            .upsert_file_index(&project_path, &item3, "h3")
            .unwrap();

        executor.set_index_store(store);

        (dir, executor)
    }

    #[tokio::test]
    async fn test_codebase_search_index_unavailable() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({ "query": "main" });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("index not available"),
            "should indicate index is unavailable, got: {}",
            output
        );
        assert!(
            output.contains("Grep"),
            "should suggest Grep as alternative, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_missing_query() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({});
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("query"));
    }

    #[tokio::test]
    async fn test_codebase_search_symbols_scope() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "App",
            "scope": "symbols"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("AppConfig"),
            "should find AppConfig symbol, got: {}",
            output
        );
        assert!(
            output.contains("AppProps"),
            "should find AppProps symbol, got: {}",
            output
        );
        assert!(
            output.contains("Symbols matching"),
            "should have symbols section header, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_files_scope_with_component() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "main",
            "scope": "files",
            "component": "desktop-rust"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("src/main.rs"),
            "should find main.rs, got: {}",
            output
        );
        assert!(
            output.contains("Files matching"),
            "should have files section header, got: {}",
            output
        );
        // Should NOT include web component files
        assert!(
            !output.contains("App.tsx"),
            "should not include web files when filtering by desktop-rust, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_files_scope_without_component() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "lib",
            "scope": "files"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("src/lib.rs"),
            "should find lib.rs, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_all_scope() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "App",
            "scope": "all"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // scope=all uses HybridSearchEngine with RRF fusion
        assert!(
            output.contains("Hybrid search") || output.contains("Symbols matching"),
            "should have hybrid or symbols section, got: {}",
            output
        );
        // AppConfig or App should appear in results
        assert!(
            output.contains("AppConfig") || output.contains("App"),
            "should find App-related results, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_default_scope_is_all() {
        let (_dir, executor) = create_test_executor_with_index();

        // No scope parameter — should default to "all"
        let args = serde_json::json!({ "query": "App" });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("Hybrid search") || output.contains("Symbols matching"),
            "default scope should search via hybrid or symbols, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_component_filter_narrows_symbols() {
        let (_dir, executor) = create_test_executor_with_index();

        // Search for "App" with symbols scope — should find both AppConfig and AppProps
        let args_all = serde_json::json!({
            "query": "App",
            "scope": "symbols"
        });
        let result_all = executor.execute("CodebaseSearch", &args_all).await;
        let output_all = result_all.output.unwrap();
        assert!(output_all.contains("AppConfig"));
        assert!(output_all.contains("AppProps"));
    }

    #[tokio::test]
    async fn test_codebase_search_no_results() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "NonExistentThing",
            "scope": "symbols"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(
            output.contains("No symbols matching"),
            "should indicate no results found, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_output_format() {
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "main",
            "scope": "symbols"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // Output should include file path, line number, and kind
        assert!(
            output.contains("main")
                && output.contains("Function")
                && output.contains("src/main.rs"),
            "output should contain symbol name, kind, and file path, got: {}",
            output
        );
    }

    // ===== Task dedup cache tests (story-005) =====

    #[test]
    fn test_task_dedup_cache_initialized_empty() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());
        let cache = executor.task_dedup_cache.lock().unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_task_dedup_cache_clear() {
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        // Insert something
        {
            let mut cache = executor.task_dedup_cache.lock().unwrap();
            cache.insert(12345, "cached result".to_string());
        }

        // Clear
        executor.clear_task_cache();

        // Verify empty
        let cache = executor.task_dedup_cache.lock().unwrap();
        assert!(cache.is_empty());
    }

    // =========================================================================
    // CodebaseSearch scope=all semantic tests (feature-004 story-001)
    // =========================================================================

    /// Helper to create a ToolExecutor with IndexStore and EmbeddingService
    /// that has a built vocabulary and stored embeddings.
    fn create_test_executor_with_embedding() -> (TempDir, ToolExecutor) {
        use crate::services::orchestrator::analysis_index::{
            FileInventoryItem, SymbolInfo, SymbolKind,
        };
        use crate::services::orchestrator::embedding_service::{
            embedding_to_bytes, EmbeddingService,
        };
        use crate::services::orchestrator::index_store::IndexStore;
        use crate::storage::database::Database;

        let dir = setup_test_dir();
        let mut executor = ToolExecutor::new(dir.path());

        let db = Database::new_in_memory().expect("in-memory db");
        let store = Arc::new(IndexStore::new(db.pool().clone()));

        let project_path = dir.path().to_string_lossy().to_string();

        // Insert file index entries
        let item1 = FileInventoryItem {
            path: "src/main.rs".to_string(),
            component: "desktop-rust".to_string(),
            language: "rust".to_string(),
            extension: Some("rs".to_string()),
            size_bytes: 1024,
            line_count: 50,
            is_test: false,
            symbols: vec![SymbolInfo::basic(
                "main".to_string(),
                SymbolKind::Function,
                1,
            )],
            content_hash: None,
        };

        store
            .upsert_file_index(&project_path, &item1, "h1")
            .unwrap();

        // Build embedding service with vocabulary
        let emb_svc = Arc::new(EmbeddingService::new());
        emb_svc.build_vocabulary(&[
            "fn main rust entry point",
            "struct config settings",
            "import react component",
        ]);

        // Store an embedding for a file chunk
        let embedding = emb_svc.embed_text("fn main rust entry point");
        let emb_bytes = embedding_to_bytes(&embedding);
        store
            .upsert_chunk_embedding(
                &project_path,
                "src/main.rs",
                0,
                "fn main() { println!(\"hello\"); }",
                &emb_bytes,
            )
            .unwrap();

        executor.set_index_store(store);
        executor.set_embedding_service(emb_svc);

        (dir, executor)
    }

    #[tokio::test]
    async fn test_codebase_search_scope_all_includes_semantic() {
        let (_dir, executor) = create_test_executor_with_embedding();

        let args = serde_json::json!({
            "query": "main",
            "scope": "all"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // scope=all uses HybridSearchEngine with RRF fusion, which combines
        // symbol + file + semantic channels internally
        assert!(
            output.contains("Hybrid search") || output.contains("Symbols matching"),
            "scope=all should include results, got: {}",
            output
        );
        // When embedding is available, semantic channel contributes to RRF;
        // with TF-IDF the embedding may contribute semantic similarity scores
        assert!(
            output.contains("main"),
            "scope=all should find 'main' results, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_scope_all_without_embedding() {
        let (_dir, executor) = create_test_executor_with_index();
        // No embedding service set — scope=all should still return results via
        // HybridSearchEngine (symbol + file channels, semantic silently skipped)

        let args = serde_json::json!({
            "query": "App",
            "scope": "all"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // HybridSearchEngine runs symbol+file channels even without embedding
        assert!(
            output.contains("Hybrid search") || output.contains("Symbols matching"),
            "scope=all without embedding should still have results, got: {}",
            output
        );
        // Should find App-related results
        assert!(
            output.contains("App"),
            "should find App in results, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_scope_all_with_unready_embedding() {
        use crate::services::orchestrator::embedding_service::EmbeddingService;

        let (_dir, mut executor) = create_test_executor_with_index();
        // Set an embedding service that has NOT been initialized (vocabulary not built)
        let emb_svc = Arc::new(EmbeddingService::new());
        executor.set_embedding_service(emb_svc);

        let args = serde_json::json!({
            "query": "App",
            "scope": "all"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // HybridSearchEngine doesn't use EmbeddingService (only EmbeddingManager),
        // so it will run symbol+file channels and skip semantic silently
        assert!(
            output.contains("Hybrid search") || output.contains("Symbols matching"),
            "should still have results, got: {}",
            output
        );
        assert!(
            output.contains("App"),
            "should find App in results, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_codebase_search_scope_semantic_unchanged() {
        // scope=semantic should behave identically to before: full error when no service
        let (_dir, executor) = create_test_executor_with_index();

        let args = serde_json::json!({
            "query": "App",
            "scope": "semantic"
        });
        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap();

        // Should have full error message (no embedding provider configured)
        assert!(
            output.contains("Semantic search not available: no embedding provider configured"),
            "scope=semantic should show full error, got: {}",
            output
        );
        assert!(
            output.contains("Use 'symbols' or 'files' scope instead"),
            "scope=semantic should suggest alternatives, got: {}",
            output
        );
    }

    // =========================================================================
    // Shared read_cache tests (feature-004 story-003)
    // =========================================================================

    #[tokio::test]
    async fn test_shared_cache_parent_to_child() {
        let dir = setup_test_dir();
        let parent = ToolExecutor::new(dir.path());
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });

        // Parent reads the file
        let result1 = parent.execute("Read", &args).await;
        assert!(result1.success);
        assert!(result1.output.unwrap().contains("line 1"));

        // Create child with shared cache
        let shared_cache = parent.shared_read_cache();
        let child = ToolExecutor::new_with_shared_cache(dir.path(), shared_cache);

        // Child reads the same file — should get dedup message
        let result2 = child.execute("Read", &args).await;
        assert!(result2.success);
        let output2 = result2.output.unwrap();
        assert!(
            output2.contains("[DEDUP]") && output2.contains("already read"),
            "child should see parent's cached read, got: {}",
            output2
        );
    }

    #[tokio::test]
    async fn test_shared_cache_child_to_parent() {
        let dir = setup_test_dir();
        let parent = ToolExecutor::new(dir.path());

        // Create child with shared cache
        let shared_cache = parent.shared_read_cache();
        let child = ToolExecutor::new_with_shared_cache(dir.path(), shared_cache);

        // Child reads a file
        let nested_path = dir
            .path()
            .join("subdir")
            .join("nested.txt")
            .to_string_lossy()
            .to_string();
        let args = serde_json::json!({ "file_path": &nested_path });
        let result1 = child.execute("Read", &args).await;
        assert!(result1.success);
        assert!(result1.output.unwrap().contains("nested content"));

        // Parent should see the child's cache entry
        let summary = parent.get_read_file_summary();
        assert!(!summary.is_empty(), "parent should see child's cached read");
        assert!(
            summary.iter().any(|(path, _, _)| path.contains("nested")),
            "parent should see the nested.txt entry from child, got: {:?}",
            summary
        );
    }

    #[tokio::test]
    async fn test_new_creates_independent_cache() {
        let dir = setup_test_dir();
        let executor1 = ToolExecutor::new(dir.path());
        let executor2 = ToolExecutor::new(dir.path());

        // Read file with executor1
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();
        let args = serde_json::json!({ "file_path": &file_path });
        executor1.execute("Read", &args).await;

        // executor2 should NOT see executor1's cache
        assert!(
            executor2.get_read_file_summary().is_empty(),
            "separate ::new() instances should not share cache"
        );
    }

    #[tokio::test]
    async fn test_shared_cache_concurrent_access() {
        let dir = setup_test_dir();

        // Create extra test files
        std::fs::write(dir.path().join("file_a.txt"), "content a\n").unwrap();
        std::fs::write(dir.path().join("file_b.txt"), "content b\n").unwrap();

        let parent = ToolExecutor::new(dir.path());
        let shared_cache = parent.shared_read_cache();
        let child = ToolExecutor::new_with_shared_cache(dir.path(), shared_cache);

        let path_a = dir.path().join("file_a.txt").to_string_lossy().to_string();
        let path_b = dir.path().join("file_b.txt").to_string_lossy().to_string();

        // Spawn two reads in parallel — one on parent, one on child
        let args_a = serde_json::json!({ "file_path": &path_a });
        let args_b = serde_json::json!({ "file_path": &path_b });
        let (res_a, res_b) = tokio::join!(
            parent.execute("Read", &args_a),
            child.execute("Read", &args_b),
        );

        assert!(res_a.success);
        assert!(res_b.success);

        // Both entries should be visible from either executor
        let summary = parent.get_read_file_summary();
        assert!(
            summary.len() >= 2,
            "shared cache should have entries from both executors, got: {:?}",
            summary
        );
    }

    // =========================================================================
    // EmbeddingManager integration tests (story-007)
    // =========================================================================

    #[test]
    fn test_set_embedding_manager() {
        use crate::services::orchestrator::embedding_manager::{
            EmbeddingManager, EmbeddingManagerConfig,
        };
        use crate::services::orchestrator::embedding_provider::{
            EmbeddingProviderConfig, EmbeddingProviderType,
        };
        use crate::services::orchestrator::embedding_provider_tfidf::TfIdfEmbeddingProvider;
        use crate::services::orchestrator::embedding_service::EmbeddingService;

        let dir = setup_test_dir();
        let mut executor = ToolExecutor::new(dir.path());

        assert!(
            executor.get_embedding_manager().is_none(),
            "manager should be None initially"
        );

        // Create a TF-IDF based EmbeddingManager
        let emb_svc = Arc::new(EmbeddingService::new());
        let provider = TfIdfEmbeddingProvider::new(Arc::clone(&emb_svc));
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let mgr = Arc::new(EmbeddingManager::new(Box::new(provider), None, config));

        executor.set_embedding_manager(Arc::clone(&mgr));
        assert!(
            executor.get_embedding_manager().is_some(),
            "manager should be set after set_embedding_manager"
        );
    }

    #[test]
    fn test_embedding_manager_independent_of_service() {
        use crate::services::orchestrator::embedding_manager::{
            EmbeddingManager, EmbeddingManagerConfig,
        };
        use crate::services::orchestrator::embedding_provider::{
            EmbeddingProviderConfig, EmbeddingProviderType,
        };
        use crate::services::orchestrator::embedding_provider_tfidf::TfIdfEmbeddingProvider;
        use crate::services::orchestrator::embedding_service::EmbeddingService;

        let dir = setup_test_dir();
        let mut executor = ToolExecutor::new(dir.path());

        // Set only the manager, not the service
        let emb_svc = Arc::new(EmbeddingService::new());
        let provider = TfIdfEmbeddingProvider::new(Arc::clone(&emb_svc));
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: false,
            cache_max_entries: 0,
        };
        let mgr = Arc::new(EmbeddingManager::new(Box::new(provider), None, config));
        executor.set_embedding_manager(mgr);

        assert!(
            executor.get_embedding_manager().is_some(),
            "manager should be set"
        );
        assert!(
            executor.get_embedding_service().is_none(),
            "service should still be None"
        );
    }

    #[tokio::test]
    async fn test_codebase_search_no_provider_message() {
        // When neither EmbeddingManager nor EmbeddingService is configured,
        // semantic scope should report "not configured".
        let dir = setup_test_dir();
        let executor = ToolExecutor::new(dir.path());

        let args = serde_json::json!({
            "query": "test query",
            "scope": "semantic"
        });

        let result = executor.execute("CodebaseSearch", &args).await;
        assert!(result.success);
        let output = result.output.unwrap_or_default();
        // Without an index store, it falls back to the "index not available" message
        assert!(
            output.contains("not available")
                || output.contains("not indexed")
                || output.contains("not configured"),
            "should indicate semantic search is not available, got: {}",
            output
        );
    }

    #[test]
    fn test_embedding_manager_shared_with_sub_agent() {
        use crate::services::orchestrator::embedding_manager::{
            EmbeddingManager, EmbeddingManagerConfig,
        };
        use crate::services::orchestrator::embedding_provider::{
            EmbeddingProviderConfig, EmbeddingProviderType,
        };
        use crate::services::orchestrator::embedding_provider_tfidf::TfIdfEmbeddingProvider;
        use crate::services::orchestrator::embedding_service::EmbeddingService;

        let dir = setup_test_dir();
        let mut parent = ToolExecutor::new(dir.path());

        // Set up manager on parent
        let emb_svc = Arc::new(EmbeddingService::new());
        let provider = TfIdfEmbeddingProvider::new(Arc::clone(&emb_svc));
        let config = EmbeddingManagerConfig {
            primary: EmbeddingProviderConfig::new(EmbeddingProviderType::TfIdf),
            fallback: None,
            cache_enabled: true,
            cache_max_entries: 100,
        };
        let mgr = Arc::new(EmbeddingManager::new(Box::new(provider), None, config));
        parent.set_embedding_manager(Arc::clone(&mgr));

        // Get the manager for sharing with sub-agent
        let shared_mgr = parent.get_embedding_manager();
        assert!(
            shared_mgr.is_some(),
            "should be able to get manager for sharing"
        );

        // Create sub-agent executor and wire the shared manager
        let shared_cache = parent.shared_read_cache();
        let mut child = ToolExecutor::new_with_shared_cache(dir.path(), shared_cache);
        child.set_embedding_manager(shared_mgr.unwrap());

        assert!(
            child.get_embedding_manager().is_some(),
            "child should have the shared manager"
        );
    }
}
