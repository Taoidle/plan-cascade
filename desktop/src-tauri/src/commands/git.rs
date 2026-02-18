//! Git Commands
//!
//! Tauri IPC commands for the git source control service.
//! Exposes ~28 commands for status, staging, commits, diffs, branches,
//! stash, merge state, conflicts, remotes, and LLM-assisted operations.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::services::git::conflict;
use crate::services::git::graph::compute_graph_layout;
use crate::services::git::llm_assist::GitLlmAssist;
use crate::services::git::service::GitService;
use crate::services::git::types::*;
use crate::services::git::watcher::GitWatcher;
use crate::services::llm::LlmProvider;

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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.full_status(&path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.stage_files(&path, &paths) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.unstage_files(&path, &paths) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.commit(&path, &message) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.amend_commit(&path, &message) {
        Ok(sha) => Ok(CommandResponse::ok(sha)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Discard Commands
// ===========================================================================

/// Discard unstaged changes for specific files.
#[tauri::command]
pub async fn git_discard_changes(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    paths: Vec<String>,
) -> Result<CommandResponse<()>, String> {
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.discard_changes(&path, &paths) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.diff_staged(&path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.diff_unstaged(&path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.diff_file(&path, &file_path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.log(&path, count.unwrap_or(100), all_branches.unwrap_or(false)) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.log(&path, count.unwrap_or(100), true) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.list_branches(&path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.create_branch(&path, &name, &base) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.delete_branch(&path, &name, force.unwrap_or(false)) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.checkout_branch(&path, &name) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.list_stashes(&path) {
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
) -> Result<CommandResponse<()>, String> {
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.stash_save(&path, message.as_deref()) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.stash_pop(&path, index) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.stash_drop(&path, index) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.get_merge_state(&path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);

    // First get the list of conflicted files from git status
    match service.full_status(&path) {
        Ok(status) => {
            let conflicted_paths: Vec<String> =
                status.conflicted.iter().map(|f| f.path.clone()).collect();
            match conflict::get_conflict_files(&path, &conflicted_paths) {
                Ok(files) => Ok(CommandResponse::ok(files)),
                Err(e) => Ok(CommandResponse::err(e.to_string())),
            }
        }
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
    let path = PathBuf::from(&repo_path);
    let full_path = path.join(&file_path);

    // Read the file
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(e) => return Ok(CommandResponse::err(format!("Failed to read file: {}", e))),
    };

    // Resolve
    let resolved = conflict::resolve_file(&content, strategy);

    // Write back
    match conflict::write_resolved(&path, &file_path, &resolved) {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ===========================================================================
// Remote Commands
// ===========================================================================

/// Fetch from remotes.
#[tauri::command]
pub async fn git_fetch(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    remote: Option<String>,
) -> Result<CommandResponse<()>, String> {
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.fetch(&path, remote.as_deref()) {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Pull from remote.
#[tauri::command]
pub async fn git_pull(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<CommandResponse<()>, String> {
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.pull(&path, remote.as_deref(), branch.as_deref()) {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Push to remote.
#[tauri::command]
pub async fn git_push(
    state: tauri::State<'_, GitState>,
    repo_path: String,
    remote: Option<String>,
    branch: Option<String>,
    set_upstream: Option<bool>,
    force: Option<bool>,
) -> Result<CommandResponse<()>, String> {
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.push(
        &path,
        remote.as_deref(),
        branch.as_deref(),
        set_upstream.unwrap_or(false),
        force.unwrap_or(false),
    ) {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get list of remotes.
#[tauri::command]
pub async fn git_get_remotes(
    state: tauri::State<'_, GitState>,
    repo_path: String,
) -> Result<CommandResponse<Vec<RemoteInfo>>, String> {
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.get_remotes(&path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.merge_branch(&path, &branch) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.merge_abort(&path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.merge_continue(&path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.rename_branch(&path, &old_name, &new_name) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.list_remote_branches(&path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.read_file_content(&path, &file_path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.parse_file_conflicts(&path, &file_path) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);
    match service.resolve_file_and_stage(&path, &file_path, &content) {
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);

    // Get staged diff
    let diff = match service.diff_staged(&path) {
        Ok(d) => d,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    if diff.files.is_empty() {
        return Ok(CommandResponse::err(
            "No staged changes to generate commit message from".to_string(),
        ));
    }

    // Convert diff to text for LLM
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
    let service = state.service.read().await;
    let path = PathBuf::from(&repo_path);

    // Get staged diff (or unstaged if nothing staged)
    let diff = match service.diff_staged(&path) {
        Ok(d) if !d.files.is_empty() => d,
        _ => match service.diff_unstaged(&path) {
            Ok(d) => d,
            Err(e) => return Ok(CommandResponse::err(e.to_string())),
        },
    };

    if diff.files.is_empty() {
        return Ok(CommandResponse::err(
            "No changes to review".to_string(),
        ));
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
