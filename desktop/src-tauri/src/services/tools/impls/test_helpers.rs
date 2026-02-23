//! Shared test utilities for tool unit tests.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::services::tools::trait_def::ToolExecutionContext;

/// Create a `ToolExecutionContext` for unit tests with a given directory as
/// both `project_root` and `working_directory`.
pub(crate) fn make_test_ctx(dir: &Path) -> ToolExecutionContext {
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
        file_change_tracker: None,
    }
}
