//! Git Commands
//!
//! Tauri IPC commands for the git source control service.
//! Exposes ~35 commands for status, staging, commits, diffs, branches,
//! stash, merge state, conflicts, remotes, tags, and LLM-assisted operations.
//!
//! All synchronous git operations are wrapped in `spawn_blocking` to avoid
//! blocking the Tokio runtime. Network operations (fetch/pull/push) additionally
//! use `tokio::time::timeout` to prevent indefinite hangs.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::commands::standalone::normalize_provider_name;
use crate::models::response::CommandResponse;
use crate::services::git::conflict;
use crate::services::git::graph::compute_graph_layout;
use crate::services::git::llm_assist::GitLlmAssist;
use crate::services::git::service::GitService;
use crate::services::git::types::*;
use crate::services::git::watcher::GitWatcher;
use crate::services::llm::{
    AnthropicProvider, DeepSeekProvider, GlmProvider, LlmProvider, MinimaxProvider, OllamaProvider,
    OpenAIProvider, ProviderConfig, ProviderType, QwenProvider,
};
use crate::state::AppState;
use crate::storage::KeyringService;

// ===========================================================================
// Helpers — spawn_blocking wrappers
// ===========================================================================

/// Run a synchronous git operation on a blocking thread to avoid blocking
/// the Tokio runtime. GitService methods call `std::process::Command::output()`
/// which is synchronous and would block the async executor.
async fn run_git_blocking<F, T>(svc: &Arc<RwLock<GitService>>, f: F) -> Result<T, String>
where
    F: FnOnce(&GitService) -> T + Send + 'static,
    T: Send + 'static,
{
    let svc = Arc::clone(svc);
    tokio::task::spawn_blocking(move || {
        let guard = svc.blocking_read();
        f(&guard)
    })
    .await
    .map_err(|e| format!("Git task panicked: {}", e))
}

/// Like `run_git_blocking` but with a timeout. Used for network operations
/// (fetch, pull, push) that could hang indefinitely on network issues.
async fn run_git_blocking_with_timeout<F, T>(
    svc: &Arc<RwLock<GitService>>,
    timeout_secs: u64,
    f: F,
) -> Result<T, String>
where
    F: FnOnce(&GitService) -> T + Send + 'static,
    T: Send + 'static,
{
    let svc = Arc::clone(svc);
    match tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tokio::task::spawn_blocking(move || {
            let guard = svc.blocking_read();
            f(&guard)
        }),
    )
    .await
    {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(e)) => Err(format!("Git task panicked: {}", e)),
        Err(_) => Err(format!(
            "Git operation timed out after {}s. Check your network connection.",
            timeout_secs
        )),
    }
}

/// State for the git service, managed by Tauri.
pub struct GitState {
    service: Arc<RwLock<GitService>>,
    llm_assist: Arc<RwLock<Option<GitLlmAssist>>>,
    watcher: Arc<RwLock<Option<GitWatcher>>>,
}

impl GitState {
    /// Create a new GitState without LLM provider.
    pub fn new() -> Self {
        Self {
            service: Arc::new(RwLock::new(GitService::new())),
            llm_assist: Arc::new(RwLock::new(None)),
            watcher: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new GitState with an LLM provider for assisted operations.
    pub fn with_llm(provider: Arc<dyn LlmProvider>) -> Self {
        Self {
            service: Arc::new(RwLock::new(GitService::new())),
            llm_assist: Arc::new(RwLock::new(Some(GitLlmAssist::new(provider)))),
            watcher: Arc::new(RwLock::new(None)),
        }
    }

    /// Set or replace the LLM provider at runtime.
    pub async fn set_llm_provider(&self, provider: Arc<dyn LlmProvider>) {
        let mut assist = self.llm_assist.write().await;
        *assist = Some(GitLlmAssist::new(provider));
    }

    /// Set the watcher instance.
    pub async fn set_watcher(&self, w: GitWatcher) {
        let mut watcher = self.watcher.write().await;
        *watcher = Some(w);
    }
}

impl Default for GitState {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Status Commands
// ===========================================================================

/// Get full repository status (staged, unstaged, untracked, conflicted).
#[tauri::command]
pub async fn git_full_status(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<GitFullStatus>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.full_status(&path)).await? {
        Ok(status) => Ok(CommandResponse::ok(status)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Staging Commands
// ===========================================================================

/// Stage specific files.
#[tauri::command]
pub async fn git_stage_files(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    paths: Vec<String>,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.stage_files(&path, &paths)).await? {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Unstage specific files.
#[tauri::command]
pub async fn git_unstage_files(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    paths: Vec<String>,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.unstage_files(&path, &paths)).await? {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Stage or unstage a specific hunk from a file's diff.
#[tauri::command]
pub async fn git_stage_hunk(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    file_path: String,
    hunk_index: usize,
    reverse: bool,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| {
        svc.stage_hunk(&path, &file_path, hunk_index, reverse)
    })
    .await?
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Commit Commands
// ===========================================================================

/// Create a new commit with the given message.
#[tauri::command]
pub async fn git_commit(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    message: String,
) -> Result<CommandResponse<String>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.commit(&path, &message)).await? {
        Ok(sha) => Ok(CommandResponse::ok(sha)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Amend the last commit with a new message.
#[tauri::command]
pub async fn git_amend_commit(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    message: String,
) -> Result<CommandResponse<String>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.amend_commit(&path, &message)).await? {
        Ok(sha) => Ok(CommandResponse::ok(sha)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Discard Commands
// ===========================================================================

/// Discard changes for specific files (supports both tracked and untracked).
#[tauri::command]
pub async fn git_discard_changes(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    paths: Vec<String>,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.smart_discard(&path, &paths)).await? {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Diff Commands
// ===========================================================================

/// Get diff of staged changes.
#[tauri::command]
pub async fn git_diff_staged(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<DiffOutput>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.diff_staged(&path)).await? {
        Ok(diff) => Ok(CommandResponse::ok(diff)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get diff of unstaged changes.
#[tauri::command]
pub async fn git_diff_unstaged(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<DiffOutput>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.diff_unstaged(&path)).await? {
        Ok(diff) => Ok(CommandResponse::ok(diff)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get diff for a specific file.
#[tauri::command]
pub async fn git_diff_file(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    file_path: String,
) -> Result<CommandResponse<DiffOutput>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.diff_file(&path, &file_path)).await? {
        Ok(diff) => Ok(CommandResponse::ok(diff)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Log / Graph Commands
// ===========================================================================

/// Get commit log.
#[tauri::command]
pub async fn git_log(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    count: Option<usize>,
    all_branches: Option<bool>,
) -> Result<CommandResponse<Vec<CommitNode>>, String> {
    let path = PathBuf::from(&repo_path);
    let c = count.unwrap_or(100);
    let all = all_branches.unwrap_or(false);
    match run_git_blocking(&state.service, move |svc| svc.log(&path, c, all)).await? {
        Ok(commits) => Ok(CommandResponse::ok(commits)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get commit log with graph layout for visualization.
#[tauri::command]
pub async fn git_log_graph(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    count: Option<usize>,
) -> Result<CommandResponse<GraphLayout>, String> {
    let path = PathBuf::from(&repo_path);
    let c = count.unwrap_or(100);
    match run_git_blocking(&state.service, move |svc| svc.log(&path, c, true)).await? {
        Ok(commits) => {
            let layout = compute_graph_layout(&commits);
            Ok(CommandResponse::ok(layout))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Branch Commands
// ===========================================================================

/// List all local branches with tracking info.
#[tauri::command]
pub async fn git_list_branches(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<Vec<BranchInfo>>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.list_branches(&path)).await? {
        Ok(branches) => Ok(CommandResponse::ok(branches)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a new branch.
#[tauri::command]
pub async fn git_create_branch(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    name: String,
    base: String,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| {
        svc.create_branch(&path, &name, &base)
    })
    .await?
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete a branch.
#[tauri::command]
pub async fn git_delete_branch(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    name: String,
    force: Option<bool>,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    let f = force.unwrap_or(false);
    match run_git_blocking(&state.service, move |svc| svc.delete_branch(&path, &name, f)).await? {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Checkout a branch.
#[tauri::command]
pub async fn git_checkout_branch(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    name: String,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.checkout_branch(&path, &name)).await? {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Stash Commands
// ===========================================================================

/// List all stash entries.
#[tauri::command]
pub async fn git_list_stashes(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<Vec<StashEntry>>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.list_stashes(&path)).await? {
        Ok(stashes) => Ok(CommandResponse::ok(stashes)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Save current changes to stash.
#[tauri::command]
pub async fn git_stash_save(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    message: Option<String>,
    include_untracked: Option<bool>,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    let untracked = include_untracked.unwrap_or(true);
    match run_git_blocking(&state.service, move |svc| {
        svc.stash_save(&path, message.as_deref(), untracked)
    })
    .await?
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Pop a stash entry.
#[tauri::command]
pub async fn git_stash_pop(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    index: Option<u32>,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.stash_pop(&path, index)).await? {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Drop a stash entry.
#[tauri::command]
pub async fn git_stash_drop(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    index: u32,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.stash_drop(&path, index)).await? {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Merge State & Conflict Commands
// ===========================================================================

/// Get current merge state.
#[tauri::command]
pub async fn git_get_merge_state(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<MergeState>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.get_merge_state(&path)).await? {
        Ok(merge_state) => Ok(CommandResponse::ok(merge_state)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get list of files with conflict markers.
#[tauri::command]
pub async fn git_get_conflict_files(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<Vec<ConflictFile>>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| {
        match svc.full_status(&path) {
            Ok(status) => {
                let conflicted_paths: Vec<String> =
                    status.conflicted.iter().map(|f| f.path.clone()).collect();
                conflict::get_conflict_files(&path, &conflicted_paths)
            }
            Err(e) => Err(e),
        }
    })
    .await?
    {
        Ok(files) => Ok(CommandResponse::ok(files)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Resolve conflicts in a file with a given strategy.
#[tauri::command]
pub async fn git_resolve_conflict(
    _state: tauri::State<'_, GitState>,
    repo_path: String,
    file_path: String,
    strategy: ConflictStrategy,
) -> Result<CommandResponse<()>, String> {
    let result = tokio::task::spawn_blocking(move || {
        let path = PathBuf::from(&repo_path);
        let full_path = path.join(&file_path);

        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to read file: {}", e)),
        };

        let resolved = conflict::resolve_file(&content, strategy);

        match conflict::write_resolved(&path, &file_path, &resolved) {
            Ok(()) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| format!("Task panicked: {}", e))?;

    match result {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e)),
    }
}

// ===========================================================================
// Remote Commands (with network timeout)
// ===========================================================================

/// Fetch from remotes (60s timeout).
#[tauri::command]
pub async fn git_fetch(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    remote: Option<String>,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking_with_timeout(&state.service, 60, move |svc| {
        svc.fetch(&path, remote.as_deref())
    })
    .await
    {
        Ok(Ok(())) => Ok(CommandResponse::ok(())),
        Ok(Err(e)) => Ok(CommandResponse::err(e.to_string())),
        Err(e) => Ok(CommandResponse::err(e)),
    }
}

/// Pull from remote (120s timeout).
#[tauri::command]
pub async fn git_pull(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking_with_timeout(&state.service, 120, move |svc| {
        svc.pull(&path, remote.as_deref(), branch.as_deref())
    })
    .await
    {
        Ok(Ok(())) => Ok(CommandResponse::ok(())),
        Ok(Err(e)) => Ok(CommandResponse::err(e.to_string())),
        Err(e) => Ok(CommandResponse::err(e)),
    }
}

/// Push to remote (120s timeout).
#[tauri::command]
pub async fn git_push(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    remote: Option<String>,
    branch: Option<String>,
    set_upstream: Option<bool>,
    force: Option<bool>,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    let su = set_upstream.unwrap_or(false);
    let f = force.unwrap_or(false);
    match run_git_blocking_with_timeout(&state.service, 120, move |svc| {
        svc.push(&path, remote.as_deref(), branch.as_deref(), su, f)
    })
    .await
    {
        Ok(Ok(())) => Ok(CommandResponse::ok(())),
        Ok(Err(e)) => Ok(CommandResponse::err(e.to_string())),
        Err(e) => Ok(CommandResponse::err(e)),
    }
}

/// Get list of remotes.
#[tauri::command]
pub async fn git_get_remotes(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<Vec<RemoteInfo>>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.get_remotes(&path)).await? {
        Ok(remotes) => Ok(CommandResponse::ok(remotes)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Merge Commands
// ===========================================================================

/// Merge a branch into the current branch.
#[tauri::command]
pub async fn git_merge_branch(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    branch: String,
) -> Result<CommandResponse<MergeBranchResult>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.merge_branch(&path, &branch)).await? {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Abort a merge in progress.
#[tauri::command]
pub async fn git_merge_abort(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.merge_abort(&path)).await? {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Complete a merge after resolving all conflicts.
#[tauri::command]
pub async fn git_merge_continue(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<String>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.merge_continue(&path)).await? {
        Ok(sha) => Ok(CommandResponse::ok(sha)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Rename a branch.
#[tauri::command]
pub async fn git_rename_branch(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    old_name: String,
    new_name: String,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| {
        svc.rename_branch(&path, &old_name, &new_name)
    })
    .await?
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List remote branches.
#[tauri::command]
pub async fn git_list_remote_branches(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<Vec<RemoteBranchInfo>>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.list_remote_branches(&path)).await? {
        Ok(branches) => Ok(CommandResponse::ok(branches)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Read a file's content (for conflict resolution).
#[tauri::command]
pub async fn git_read_file_content(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    file_path: String,
) -> Result<CommandResponse<String>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| {
        svc.read_file_content(&path, &file_path)
    })
    .await?
    {
        Ok(content) => Ok(CommandResponse::ok(content)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Parse conflict regions from a file.
#[tauri::command]
pub async fn git_parse_file_conflicts(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    file_path: String,
) -> Result<CommandResponse<Vec<ConflictRegion>>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| {
        svc.parse_file_conflicts(&path, &file_path)
    })
    .await?
    {
        Ok(regions) => Ok(CommandResponse::ok(regions)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Resolve a file by writing content and staging it.
#[tauri::command]
pub async fn git_resolve_file_and_stage(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    file_path: String,
    content: String,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| {
        svc.resolve_file_and_stage(&path, &file_path, &content)
    })
    .await?
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Operation Abort/Continue Commands (rebase, cherry-pick, revert)
// ===========================================================================

/// Abort a rebase, cherry-pick, or revert operation.
#[tauri::command]
pub async fn git_operation_abort(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    kind: String,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| match kind.as_str() {
        "merging" => svc.merge_abort(&path),
        "rebasing" => svc.rebase_abort(&path),
        "cherry_picking" => svc.cherry_pick_abort(&path),
        "reverting" => svc.revert_abort(&path),
        _ => Err(crate::utils::error::AppError::command(format!(
            "Unknown operation kind: {}",
            kind
        ))),
    })
    .await?
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Continue a rebase, cherry-pick, or revert operation.
#[tauri::command]
pub async fn git_operation_continue(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    kind: String,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| match kind.as_str() {
        "merging" => svc.merge_continue(&path).map(|_| ()),
        "rebasing" => svc.rebase_continue(&path),
        "cherry_picking" => svc.cherry_pick_continue(&path),
        "reverting" => svc.revert_continue(&path),
        _ => Err(crate::utils::error::AppError::command(format!(
            "Unknown operation kind: {}",
            kind
        ))),
    })
    .await?
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Tag Commands
// ===========================================================================

/// List all tags.
#[tauri::command]
pub async fn git_list_tags(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<Vec<TagInfo>>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.list_tags(&path)).await? {
        Ok(tags) => Ok(CommandResponse::ok(tags)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a tag (lightweight or annotated).
#[tauri::command]
pub async fn git_create_tag(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    name: String,
    message: Option<String>,
    target: Option<String>,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| {
        if let Some(ref msg) = message {
            svc.create_annotated_tag(&path, &name, msg, target.as_deref())
        } else {
            svc.create_tag(&path, &name, target.as_deref())
        }
    })
    .await?
    {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete a tag.
#[tauri::command]
pub async fn git_delete_tag(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    name: String,
) -> Result<CommandResponse<()>, String> {
    let path = PathBuf::from(&repo_path);
    match run_git_blocking(&state.service, move |svc| svc.delete_tag(&path, &name)).await? {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// LLM-Assisted Commands
// ===========================================================================

/// Generate a commit message from staged diff using LLM.
#[tauri::command]
pub async fn git_generate_commit_message(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<String>, String> {
    let path = PathBuf::from(&repo_path);

    // Get staged diff on a blocking thread
    let diff = match run_git_blocking(&state.service, move |svc| svc.diff_staged(&path)).await? {
        Ok(d) => d,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    if diff.files.is_empty() {
        return Ok(CommandResponse::err(
            "No staged changes to generate commit message from".to_string(),
        ));
    }

    let diff_text = diff_output_to_text(&diff);

    let assist = state.llm_assist.read().await;
    match assist.as_ref() {
        Some(llm) => match llm.generate_commit_message(&diff_text).await {
            Ok(message) => Ok(CommandResponse::ok(message)),
            Err(e) => Ok(CommandResponse::err(e.to_string())),
        },
        None => Ok(CommandResponse::err(
            "LLM provider not configured for git operations".to_string(),
        )),
    }
}

/// Review staged diff using LLM.
#[tauri::command]
pub async fn git_review_diff(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<String>, String> {
    let path_staged = PathBuf::from(&repo_path);
    let path_unstaged = PathBuf::from(&repo_path);

    // Get staged diff
    let staged = run_git_blocking(&state.service, move |svc| svc.diff_staged(&path_staged))
        .await?
        .ok();

    let diff = if staged.as_ref().map_or(true, |d| d.files.is_empty()) {
        match run_git_blocking(&state.service, move |svc| svc.diff_unstaged(&path_unstaged))
            .await?
        {
            Ok(d) => d,
            Err(e) => return Ok(CommandResponse::err(e.to_string())),
        }
    } else {
        staged.unwrap()
    };

    if diff.files.is_empty() {
        return Ok(CommandResponse::err("No changes to review".to_string()));
    }

    let diff_text = diff_output_to_text(&diff);

    let assist = state.llm_assist.read().await;
    match assist.as_ref() {
        Some(llm) => match llm.review_diff(&diff_text).await {
            Ok(review) => Ok(CommandResponse::ok(review)),
            Err(e) => Ok(CommandResponse::err(e.to_string())),
        },
        None => Ok(CommandResponse::err(
            "LLM provider not configured for git operations".to_string(),
        )),
    }
}

/// Resolve a conflict in a file using LLM.
#[tauri::command]
pub async fn git_resolve_conflict_ai(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    file_path: String,
) -> Result<CommandResponse<String>, String> {
    let content = tokio::task::spawn_blocking(move || {
        let path = PathBuf::from(&repo_path);
        let full_path = path.join(&file_path);
        std::fs::read_to_string(&full_path)
            .map_err(|e| format!("Failed to read file: {}", e))
    })
    .await
    .map_err(|e| format!("Task panicked: {}", e))??;

    let assist = state.llm_assist.read().await;
    match assist.as_ref() {
        Some(llm) => match llm.suggest_conflict_resolution(&content).await {
            Ok(resolved) => Ok(CommandResponse::ok(resolved)),
            Err(e) => Ok(CommandResponse::err(e.to_string())),
        },
        None => Ok(CommandResponse::err(
            "LLM provider not configured for git operations".to_string(),
        )),
    }
}

/// Summarize a commit using LLM.
#[tauri::command]
pub async fn git_summarize_commit(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    sha: String,
) -> Result<CommandResponse<String>, String> {
    let path = PathBuf::from(&repo_path);
    let sha_clone = sha.clone();

    let (diff_text, commit_message) = run_git_blocking(&state.service, move |svc| {
        let dt = match svc.diff_for_commit(&path, &sha_clone) {
            Ok(d) => diff_output_to_text(&d),
            Err(_) => String::new(),
        };
        let cm = match svc
            .git_ops()
            .execute(&path, &["log", "-1", "--format=%s%n%n%b", &sha_clone])
        {
            Ok(output) => output.into_result().unwrap_or_default().trim().to_string(),
            Err(_) => String::new(),
        };
        (dt, cm)
    })
    .await?;

    if commit_message.is_empty() && diff_text.is_empty() {
        return Ok(CommandResponse::err(
            "Could not retrieve commit information".to_string(),
        ));
    }

    let assist = state.llm_assist.read().await;
    match assist.as_ref() {
        Some(llm) => {
            let messages = vec![commit_message];
            let diff_ref = if diff_text.is_empty() {
                None
            } else {
                Some(diff_text.as_str())
            };
            match llm.summarize_changes(&messages, diff_ref).await {
                Ok(summary) => Ok(CommandResponse::ok(summary)),
                Err(e) => Ok(CommandResponse::err(e.to_string())),
            }
        }
        None => Ok(CommandResponse::err(
            "LLM provider not configured for git operations".to_string(),
        )),
    }
}

/// Check if LLM provider is configured for git operations.
#[tauri::command]
pub async fn git_check_llm_available(
    state: tauri::State<'_, GitState>,
) -> Result<CommandResponse<bool>, String> {
    let assist = state.llm_assist.read().await;
    Ok(CommandResponse::ok(assist.is_some()))
}

/// Configure the LLM provider for git operations at runtime.
#[tauri::command]
pub async fn git_configure_llm(
    state: tauri::State<'_, GitState>,
    app_state: tauri::State<'_, AppState>,
    provider: String,
    model: String,
    api_key: String,
    base_url: Option<String>,
) -> Result<CommandResponse<bool>, String> {
    let canonical = match normalize_provider_name(&provider) {
        Some(c) => c,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown provider: {}",
                provider
            )));
        }
    };

    let provider_type = match canonical {
        "anthropic" => ProviderType::Anthropic,
        "openai" => ProviderType::OpenAI,
        "deepseek" => ProviderType::DeepSeek,
        "glm" => ProviderType::Glm,
        "qwen" => ProviderType::Qwen,
        "minimax" => ProviderType::Minimax,
        "ollama" => ProviderType::Ollama,
        _ => {
            return Ok(CommandResponse::err(format!(
                "Unsupported provider: {}",
                canonical
            )));
        }
    };

    let api_key_opt = if api_key.is_empty() {
        None
    } else {
        Some(api_key)
    };

    if provider_type != ProviderType::Ollama && api_key_opt.is_none() {
        return Ok(CommandResponse::err(format!(
            "API key required for provider '{}'",
            canonical
        )));
    }

    let resolved_model = if model.trim().is_empty() {
        match provider_type {
            ProviderType::Anthropic => "claude-3-5-sonnet-20241022".to_string(),
            ProviderType::OpenAI => "gpt-4o".to_string(),
            ProviderType::DeepSeek => "deepseek-chat".to_string(),
            ProviderType::Glm => "glm-4.7".to_string(),
            ProviderType::Qwen => "qwen3-plus".to_string(),
            ProviderType::Minimax => "MiniMax-M2.5".to_string(),
            ProviderType::Ollama => "llama3.2".to_string(),
        }
    } else {
        model
    };

    let frontend_base_url = base_url.filter(|u| !u.is_empty());
    let resolved_base_url = if frontend_base_url.is_some() {
        frontend_base_url
    } else {
        app_state
            .with_database(|db| {
                let key = format!("provider_{}_base_url", canonical);
                db.get_setting(&key)
            })
            .await
            .ok()
            .flatten()
            .filter(|u| !u.is_empty())
    };

    let keyring = KeyringService::new();
    let proxy = app_state
        .with_database(|db| {
            Ok(crate::commands::proxy::resolve_provider_proxy(
                &keyring, db, canonical,
            ))
        })
        .await
        .unwrap_or(None);

    let config = ProviderConfig {
        provider: provider_type,
        api_key: api_key_opt,
        base_url: resolved_base_url,
        model: resolved_model,
        proxy,
        ..Default::default()
    };

    let llm_provider: Arc<dyn LlmProvider> = match config.provider {
        ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config)),
        ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config)),
        ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(config)),
        ProviderType::Glm => Arc::new(GlmProvider::new(config)),
        ProviderType::Qwen => Arc::new(QwenProvider::new(config)),
        ProviderType::Minimax => Arc::new(MinimaxProvider::new(config)),
        ProviderType::Ollama => Arc::new(OllamaProvider::new(config)),
    };

    state.set_llm_provider(llm_provider).await;

    Ok(CommandResponse::ok(true))
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Convert a DiffOutput to a plain-text representation for LLM consumption.
fn diff_output_to_text(diff: &DiffOutput) -> String {
    let mut text = String::new();
    for file in &diff.files {
        text.push_str(&format!("--- a/{}\n+++ b/{}\n", file.path, file.path));
        for hunk in &file.hunks {
            text.push_str(&hunk.header);
            text.push('\n');
            for line in &hunk.lines {
                match line.kind {
                    DiffLineKind::Addition => text.push_str(&format!("+{}\n", line.content)),
                    DiffLineKind::Deletion => text.push_str(&format!("-{}\n", line.content)),
                    DiffLineKind::Context => text.push_str(&format!(" {}\n", line.content)),
                    DiffLineKind::HunkHeader => {} // Already printed above
                }
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_state_creation() {
        let state = GitState::new();
        let _ = state;
    }

    #[test]
    fn test_git_state_default() {
        let state = GitState::default();
        let _ = state;
    }

    #[test]
    fn test_diff_output_to_text() {
        let diff = DiffOutput {
            files: vec![FileDiff {
                path: "test.rs".to_string(),
                is_new: false,
                is_deleted: false,
                is_renamed: false,
                old_path: None,
                is_binary: false,
                hunks: vec![DiffHunk {
                    header: "@@ -1,2 +1,3 @@".to_string(),
                    old_start: 1,
                    old_count: 2,
                    new_start: 1,
                    new_count: 3,
                    lines: vec![
                        DiffLine {
                            kind: DiffLineKind::Context,
                            content: "existing line".to_string(),
                            old_line_no: Some(1),
                            new_line_no: Some(1),
                        },
                        DiffLine {
                            kind: DiffLineKind::Addition,
                            content: "new line".to_string(),
                            old_line_no: None,
                            new_line_no: Some(2),
                        },
                    ],
                }],
            }],
            total_additions: 1,
            total_deletions: 0,
        };

        let text = diff_output_to_text(&diff);
        assert!(text.contains("--- a/test.rs"));
        assert!(text.contains("+++ b/test.rs"));
        assert!(text.contains("+new line"));
        assert!(text.contains(" existing line"));
    }

    #[tokio::test]
    async fn test_git_state_service_access() {
        let state = GitState::new();
        let service = state.service.read().await;
        let _ = service;
    }

    #[tokio::test]
    async fn test_git_state_llm_assist_initially_none() {
        let state = GitState::new();
        let assist = state.llm_assist.read().await;
        assert!(assist.is_none());
    }
}
