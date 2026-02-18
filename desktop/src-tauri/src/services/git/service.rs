//! Git Service
//!
//! High-level git operations built on top of GitOps.
//! Provides rich, parsed data structures for the UI layer.

use std::path::{Path, PathBuf};

use crate::services::worktree::GitOps;
use crate::utils::error::{AppError, AppResult};

use super::types::*;

/// High-level git service wrapping the low-level GitOps.
#[derive(Debug)]
pub struct GitService {
    git: GitOps,
}

impl GitService {
    /// Create a new GitService instance.
    pub fn new() -> Self {
        Self { git: GitOps::new() }
    }

    /// Get the underlying GitOps (for operations not yet wrapped).
    pub fn git_ops(&self) -> &GitOps {
        &self.git
    }

    // -----------------------------------------------------------------------
    // Status
    // -----------------------------------------------------------------------

    /// Get full repository status parsed from `git status --porcelain=v2 --branch`.
    pub fn full_status(&self, repo_path: &Path) -> AppResult<GitFullStatus> {
        let output = self
            .git
            .execute(repo_path, &["status", "--porcelain=v2", "--branch"])?
            .into_result()?;

        parse_porcelain_v2(&output)
    }

    // -----------------------------------------------------------------------
    // Staging
    // -----------------------------------------------------------------------

    /// Stage specific files.
    pub fn stage_files(&self, repo_path: &Path, paths: &[String]) -> AppResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        self.git.add(repo_path, &refs)
    }

    /// Unstage specific files (git reset HEAD -- <paths>).
    pub fn unstage_files(&self, repo_path: &Path, paths: &[String]) -> AppResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args: Vec<&str> = vec!["reset", "HEAD", "--"];
        for p in paths {
            args.push(p.as_str());
        }
        self.git.execute(repo_path, &args)?.into_result()?;
        Ok(())
    }

    /// Stage all changes.
    pub fn stage_all(&self, repo_path: &Path) -> AppResult<()> {
        self.git.add_all(repo_path)
    }

    // -----------------------------------------------------------------------
    // Commit
    // -----------------------------------------------------------------------

    /// Create a commit with the given message.
    pub fn commit(&self, repo_path: &Path, message: &str) -> AppResult<String> {
        self.git.commit(repo_path, message)
    }

    /// Amend the last commit with a new message.
    pub fn amend_commit(&self, repo_path: &Path, message: &str) -> AppResult<String> {
        let result = self.git.execute(
            repo_path,
            &["commit", "--amend", "--no-gpg-sign", "-m", message],
        )?;
        if result.success {
            let sha = self
                .git
                .execute(repo_path, &["rev-parse", "HEAD"])?
                .into_result()?
                .trim()
                .to_string();
            Ok(sha)
        } else {
            Err(AppError::command(format!(
                "Amend commit failed: {}",
                result.stderr.trim()
            )))
        }
    }

    // -----------------------------------------------------------------------
    // Discard
    // -----------------------------------------------------------------------

    /// Discard changes in working tree for specific files.
    pub fn discard_changes(&self, repo_path: &Path, paths: &[String]) -> AppResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args: Vec<&str> = vec!["checkout", "--"];
        for p in paths {
            args.push(p.as_str());
        }
        self.git.execute(repo_path, &args)?.into_result()?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Diff
    // -----------------------------------------------------------------------

    /// Get diff of staged changes.
    pub fn diff_staged(&self, repo_path: &Path) -> AppResult<DiffOutput> {
        let output = self
            .git
            .execute(repo_path, &["diff", "--cached"])?
            .into_result()?;
        Ok(parse_unified_diff(&output))
    }

    /// Get diff of unstaged changes.
    pub fn diff_unstaged(&self, repo_path: &Path) -> AppResult<DiffOutput> {
        let output = self
            .git
            .execute(repo_path, &["diff"])?
            .into_result()?;
        Ok(parse_unified_diff(&output))
    }

    /// Get diff for a specific file (working tree vs HEAD).
    pub fn diff_file(&self, repo_path: &Path, file_path: &str) -> AppResult<DiffOutput> {
        let output = self
            .git
            .execute(repo_path, &["diff", "HEAD", "--", file_path])?
            .into_result()?;
        Ok(parse_unified_diff(&output))
    }

    // -----------------------------------------------------------------------
    // Log
    // -----------------------------------------------------------------------

    /// Get commit log with parent information for graph layout.
    pub fn log(
        &self,
        repo_path: &Path,
        count: usize,
        all_branches: bool,
    ) -> AppResult<Vec<CommitNode>> {
        let count_arg = format!("-{}", count);
        let format_arg =
            "--format=%H%x00%h%x00%P%x00%an%x00%ae%x00%aI%x00%s%x00%D";

        let mut args = vec!["log", &count_arg, format_arg];
        if all_branches {
            args.push("--all");
        }

        let output = self.git.execute(repo_path, &args)?.into_result()?;
        Ok(parse_log_output(&output))
    }

    // -----------------------------------------------------------------------
    // Branches
    // -----------------------------------------------------------------------

    /// List all local branches with tracking information.
    pub fn list_branches(&self, repo_path: &Path) -> AppResult<Vec<BranchInfo>> {
        let output = self
            .git
            .execute(
                repo_path,
                &[
                    "for-each-ref",
                    "--format=%(refname:short)%00%(objectname:short)%00%(HEAD)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(subject)",
                    "refs/heads/",
                ],
            )?
            .into_result()?;

        Ok(parse_branch_list(&output))
    }

    /// Create a new branch from a base.
    pub fn create_branch(
        &self,
        repo_path: &Path,
        name: &str,
        base: &str,
    ) -> AppResult<()> {
        self.git.create_branch(repo_path, name, base)
    }

    /// Delete a branch.
    pub fn delete_branch(
        &self,
        repo_path: &Path,
        name: &str,
        force: bool,
    ) -> AppResult<()> {
        self.git.delete_branch(repo_path, name, force)
    }

    /// Checkout a branch.
    pub fn checkout_branch(&self, repo_path: &Path, name: &str) -> AppResult<()> {
        self.git.checkout(repo_path, name)
    }

    /// Rename a branch.
    pub fn rename_branch(
        &self,
        repo_path: &Path,
        old_name: &str,
        new_name: &str,
    ) -> AppResult<()> {
        self.git
            .execute(repo_path, &["branch", "-m", old_name, new_name])?
            .into_result()?;
        Ok(())
    }

    /// Merge a branch into the current branch.
    pub fn merge_branch(
        &self,
        repo_path: &Path,
        branch: &str,
    ) -> AppResult<MergeBranchResult> {
        let result = self
            .git
            .execute(repo_path, &["merge", "--no-ff", branch])?;

        if result.success {
            Ok(MergeBranchResult {
                success: true,
                has_conflicts: false,
                conflicting_files: Vec::new(),
                error: None,
            })
        } else if result.stdout.contains("CONFLICT") || result.stderr.contains("CONFLICT") {
            // Get conflicting files
            let conflict_output = self
                .git
                .execute(repo_path, &["diff", "--name-only", "--diff-filter=U"])?
                .into_result()
                .unwrap_or_default();
            let conflicts: Vec<String> = conflict_output
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Ok(MergeBranchResult {
                success: false,
                has_conflicts: true,
                conflicting_files: conflicts,
                error: None,
            })
        } else {
            Ok(MergeBranchResult {
                success: false,
                has_conflicts: false,
                conflicting_files: Vec::new(),
                error: Some(result.stderr.trim().to_string()),
            })
        }
    }

    /// Abort a merge in progress.
    pub fn merge_abort(&self, repo_path: &Path) -> AppResult<()> {
        self.git
            .execute(repo_path, &["merge", "--abort"])?
            .into_result()?;
        Ok(())
    }

    /// Complete a merge (commit after all conflicts resolved).
    pub fn merge_continue(&self, repo_path: &Path) -> AppResult<String> {
        // Stage all resolved files and commit
        self.git
            .execute(repo_path, &["commit", "--no-edit"])?
            .into_result()?;
        let sha = self
            .git
            .execute(repo_path, &["rev-parse", "HEAD"])?
            .into_result()?
            .trim()
            .to_string();
        Ok(sha)
    }

    /// List remote branches.
    pub fn list_remote_branches(&self, repo_path: &Path) -> AppResult<Vec<RemoteBranchInfo>> {
        let output = self
            .git
            .execute(
                repo_path,
                &[
                    "for-each-ref",
                    "--format=%(refname:short)%00%(objectname:short)",
                    "refs/remotes/",
                ],
            )?
            .into_result()?;

        Ok(parse_remote_branch_list(&output))
    }

    /// Read file content (for conflict resolution).
    pub fn read_file_content(&self, repo_path: &Path, file_path: &str) -> AppResult<String> {
        let full_path = repo_path.join(file_path);
        std::fs::read_to_string(&full_path).map_err(|e| {
            AppError::command(format!("Failed to read file {}: {}", file_path, e))
        })
    }

    /// Write resolved content and stage the file.
    pub fn resolve_file_and_stage(
        &self,
        repo_path: &Path,
        file_path: &str,
        content: &str,
    ) -> AppResult<()> {
        let full_path = repo_path.join(file_path);
        std::fs::write(&full_path, content).map_err(|e| {
            AppError::command(format!("Failed to write file {}: {}", file_path, e))
        })?;
        self.git.add(repo_path, &[file_path])?;
        Ok(())
    }

    /// Parse conflict regions from a file.
    pub fn parse_file_conflicts(
        &self,
        repo_path: &Path,
        file_path: &str,
    ) -> AppResult<Vec<ConflictRegion>> {
        let content = self.read_file_content(repo_path, file_path)?;
        Ok(super::conflict::parse_conflicts(&content))
    }

    // -----------------------------------------------------------------------
    // Stash
    // -----------------------------------------------------------------------

    /// List all stash entries.
    pub fn list_stashes(&self, repo_path: &Path) -> AppResult<Vec<StashEntry>> {
        let output = self
            .git
            .execute(
                repo_path,
                &["stash", "list", "--format=%gd%x00%gs%x00%ai"],
            )?
            .into_result()?;

        Ok(parse_stash_list(&output))
    }

    /// Save current changes to stash.
    pub fn stash_save(&self, repo_path: &Path, message: Option<&str>) -> AppResult<()> {
        let mut args = vec!["stash", "push"];
        if let Some(msg) = message {
            args.push("-m");
            args.push(msg);
        }
        self.git.execute(repo_path, &args)?.into_result()?;
        Ok(())
    }

    /// Pop the most recent stash (or a specific index).
    pub fn stash_pop(&self, repo_path: &Path, index: Option<u32>) -> AppResult<()> {
        let idx = index.map(|i| format!("stash@{{{}}}", i));
        let mut args = vec!["stash", "pop"];
        if let Some(ref i) = idx {
            args.push(i);
        }
        self.git.execute(repo_path, &args)?.into_result()?;
        Ok(())
    }

    /// Drop a stash entry.
    pub fn stash_drop(&self, repo_path: &Path, index: u32) -> AppResult<()> {
        let idx = format!("stash@{{{}}}", index);
        self.git
            .execute(repo_path, &["stash", "drop", &idx])?
            .into_result()?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Merge State
    // -----------------------------------------------------------------------

    /// Detect current merge state by checking .git sentinel files.
    pub fn get_merge_state(&self, repo_path: &Path) -> AppResult<MergeState> {
        let git_dir = self.resolve_git_dir(repo_path)?;
        let head = self
            .git
            .execute(repo_path, &["rev-parse", "HEAD"])?
            .into_result()?
            .trim()
            .to_string();

        // Check sentinel files in priority order
        if git_dir.join("MERGE_HEAD").exists() {
            let merge_head = std::fs::read_to_string(git_dir.join("MERGE_HEAD"))
                .ok()
                .map(|s| s.trim().to_string());
            return Ok(MergeState {
                kind: MergeStateKind::Merging,
                head,
                merge_head,
                branch_name: None,
            });
        }

        if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
            let rebase_head = std::fs::read_to_string(git_dir.join("REBASE_HEAD"))
                .ok()
                .map(|s| s.trim().to_string());
            return Ok(MergeState {
                kind: MergeStateKind::Rebasing,
                head,
                merge_head: rebase_head,
                branch_name: None,
            });
        }

        if git_dir.join("CHERRY_PICK_HEAD").exists() {
            let pick_head = std::fs::read_to_string(git_dir.join("CHERRY_PICK_HEAD"))
                .ok()
                .map(|s| s.trim().to_string());
            return Ok(MergeState {
                kind: MergeStateKind::CherryPicking,
                head,
                merge_head: pick_head,
                branch_name: None,
            });
        }

        if git_dir.join("REVERT_HEAD").exists() {
            let revert_head = std::fs::read_to_string(git_dir.join("REVERT_HEAD"))
                .ok()
                .map(|s| s.trim().to_string());
            return Ok(MergeState {
                kind: MergeStateKind::Reverting,
                head,
                merge_head: revert_head,
                branch_name: None,
            });
        }

        Ok(MergeState {
            kind: MergeStateKind::None,
            head,
            merge_head: None,
            branch_name: None,
        })
    }

    // -----------------------------------------------------------------------
    // Remotes
    // -----------------------------------------------------------------------

    /// List all remotes with their URLs.
    pub fn get_remotes(&self, repo_path: &Path) -> AppResult<Vec<RemoteInfo>> {
        let output = self
            .git
            .execute(repo_path, &["remote", "-v"])?
            .into_result()?;

        Ok(parse_remotes(&output))
    }

    /// Fetch from a remote (or all).
    pub fn fetch(&self, repo_path: &Path, remote: Option<&str>) -> AppResult<()> {
        let mut args = vec!["fetch"];
        if let Some(r) = remote {
            args.push(r);
        } else {
            args.push("--all");
        }
        self.git.execute(repo_path, &args)?.into_result()?;
        Ok(())
    }

    /// Pull from remote.
    pub fn pull(&self, repo_path: &Path, remote: Option<&str>, branch: Option<&str>) -> AppResult<()> {
        let mut args = vec!["pull"];
        if let Some(r) = remote {
            args.push(r);
        }
        if let Some(b) = branch {
            args.push(b);
        }
        self.git.execute(repo_path, &args)?.into_result()?;
        Ok(())
    }

    /// Push to remote.
    pub fn push(
        &self,
        repo_path: &Path,
        remote: Option<&str>,
        branch: Option<&str>,
        set_upstream: bool,
        force: bool,
    ) -> AppResult<()> {
        let mut args = vec!["push"];
        if set_upstream {
            args.push("-u");
        }
        if force {
            args.push("--force-with-lease");
        }
        if let Some(r) = remote {
            args.push(r);
        }
        if let Some(b) = branch {
            args.push(b);
        }
        self.git.execute(repo_path, &args)?.into_result()?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Resolve the .git directory (handles worktrees where .git is a file).
    fn resolve_git_dir(&self, repo_path: &Path) -> AppResult<PathBuf> {
        let output = self
            .git
            .execute(repo_path, &["rev-parse", "--git-dir"])?
            .into_result()?;
        let git_dir = output.trim();
        let path = if Path::new(git_dir).is_absolute() {
            PathBuf::from(git_dir)
        } else {
            repo_path.join(git_dir)
        };
        Ok(path)
    }
}

impl Default for GitService {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Parsing helpers
// ===========================================================================

/// Parse `git status --porcelain=v2 --branch` output.
pub fn parse_porcelain_v2(output: &str) -> AppResult<GitFullStatus> {
    let mut status = GitFullStatus::default();

    for line in output.lines() {
        if line.starts_with("# branch.head ") {
            status.branch = line
                .strip_prefix("# branch.head ")
                .unwrap_or("")
                .to_string();
        } else if line.starts_with("# branch.upstream ") {
            status.upstream = Some(
                line.strip_prefix("# branch.upstream ")
                    .unwrap_or("")
                    .to_string(),
            );
        } else if line.starts_with("# branch.ab ") {
            // Format: "# branch.ab +N -M"
            let ab = line.strip_prefix("# branch.ab ").unwrap_or("");
            for part in ab.split_whitespace() {
                if let Some(val) = part.strip_prefix('+') {
                    status.ahead = val.parse().unwrap_or(0);
                } else if let Some(val) = part.strip_prefix('-') {
                    status.behind = val.parse().unwrap_or(0);
                }
            }
        } else if line.starts_with("1 ") {
            // Ordinary changed entry: "1 XY sub mH mI mW hH hI path"
            parse_ordinary_entry(line, &mut status);
        } else if line.starts_with("2 ") {
            // Renamed/copied entry: "2 XY sub mH mI mW hH hI X_score path\torigPath"
            parse_rename_entry(line, &mut status);
        } else if line.starts_with("u ") {
            // Unmerged entry: "u XY sub m1 m2 m3 mW h1 h2 h3 path"
            parse_unmerged_entry(line, &mut status);
        } else if line.starts_with("? ") {
            // Untracked: "? path"
            let path = line.strip_prefix("? ").unwrap_or("").to_string();
            if !path.is_empty() {
                status.untracked.push(FileStatus {
                    path,
                    original_path: None,
                    kind: FileStatusKind::Untracked,
                });
            }
        }
        // Ignore "!" (ignored) entries
    }

    Ok(status)
}

fn parse_ordinary_entry(line: &str, status: &mut GitFullStatus) {
    // "1 XY sub mH mI mW hH hI path"
    let parts: Vec<&str> = line.splitn(9, ' ').collect();
    if parts.len() < 9 {
        return;
    }
    let xy = parts[1];
    let path = parts[8].to_string();
    let x = xy.chars().next().unwrap_or('.');
    let y = xy.chars().nth(1).unwrap_or('.');

    // Index (staged) changes
    if x != '.' {
        status.staged.push(FileStatus {
            path: path.clone(),
            original_path: None,
            kind: char_to_status_kind(x),
        });
    }

    // Worktree (unstaged) changes
    if y != '.' {
        status.unstaged.push(FileStatus {
            path,
            original_path: None,
            kind: char_to_status_kind(y),
        });
    }
}

fn parse_rename_entry(line: &str, status: &mut GitFullStatus) {
    // "2 XY sub mH mI mW hH hI Xscore path\torigPath"
    // 10 space-separated fields; field 9 (index 8) = score, field 10 (index 9) = path\torigPath
    let parts: Vec<&str> = line.splitn(10, ' ').collect();
    if parts.len() < 10 {
        return;
    }
    let xy = parts[1];
    let path_part = parts[9]; // path\torigPath
    let x = xy.chars().next().unwrap_or('.');

    let (path, orig) = if let Some(tab_pos) = path_part.find('\t') {
        (
            path_part[..tab_pos].to_string(),
            Some(path_part[tab_pos + 1..].to_string()),
        )
    } else {
        (path_part.to_string(), None)
    };

    if x == 'R' || x == 'C' {
        status.staged.push(FileStatus {
            path,
            original_path: orig,
            kind: if x == 'R' {
                FileStatusKind::Renamed
            } else {
                FileStatusKind::Copied
            },
        });
    }
}

fn parse_unmerged_entry(line: &str, status: &mut GitFullStatus) {
    // "u XY sub m1 m2 m3 mW h1 h2 h3 path"
    let parts: Vec<&str> = line.splitn(11, ' ').collect();
    if parts.len() < 11 {
        return;
    }
    let path = parts[10].to_string();
    status.conflicted.push(FileStatus {
        path,
        original_path: None,
        kind: FileStatusKind::Unmerged,
    });
}

fn char_to_status_kind(c: char) -> FileStatusKind {
    match c {
        'M' => FileStatusKind::Modified,
        'A' => FileStatusKind::Added,
        'D' => FileStatusKind::Deleted,
        'R' => FileStatusKind::Renamed,
        'C' => FileStatusKind::Copied,
        'T' => FileStatusKind::TypeChanged,
        _ => FileStatusKind::Modified,
    }
}

/// Parse unified diff output into DiffOutput.
pub fn parse_unified_diff(output: &str) -> DiffOutput {
    let mut diff = DiffOutput::default();
    let mut current_file: Option<FileDiff> = None;
    let mut current_hunk: Option<DiffHunk> = None;
    let mut old_line: u32 = 0;
    let mut new_line: u32 = 0;

    for line in output.lines() {
        if line.starts_with("diff --git ") {
            // Flush current hunk/file
            if let Some(hunk) = current_hunk.take() {
                if let Some(ref mut file) = current_file {
                    file.hunks.push(hunk);
                }
            }
            if let Some(file) = current_file.take() {
                diff.files.push(file);
            }

            // Parse file path from "diff --git a/path b/path"
            let path = parse_diff_header_path(line);
            current_file = Some(FileDiff {
                path,
                is_new: false,
                is_deleted: false,
                is_renamed: false,
                old_path: None,
                hunks: Vec::new(),
            });
        } else if line.starts_with("new file mode") {
            if let Some(ref mut file) = current_file {
                file.is_new = true;
            }
        } else if line.starts_with("deleted file mode") {
            if let Some(ref mut file) = current_file {
                file.is_deleted = true;
            }
        } else if line.starts_with("rename from ") {
            if let Some(ref mut file) = current_file {
                file.is_renamed = true;
                file.old_path = Some(
                    line.strip_prefix("rename from ")
                        .unwrap_or("")
                        .to_string(),
                );
            }
        } else if line.starts_with("@@ ") {
            // Flush previous hunk
            if let Some(hunk) = current_hunk.take() {
                if let Some(ref mut file) = current_file {
                    file.hunks.push(hunk);
                }
            }

            let (os, oc, ns, nc) = parse_hunk_header(line);
            old_line = os;
            new_line = ns;

            current_hunk = Some(DiffHunk {
                header: line.to_string(),
                old_start: os,
                old_count: oc,
                new_start: ns,
                new_count: nc,
                lines: vec![DiffLine {
                    kind: DiffLineKind::HunkHeader,
                    content: line.to_string(),
                    old_line_no: None,
                    new_line_no: None,
                }],
            });
        } else if let Some(ref mut hunk) = current_hunk {
            if line.starts_with('+') {
                hunk.lines.push(DiffLine {
                    kind: DiffLineKind::Addition,
                    content: line[1..].to_string(),
                    old_line_no: None,
                    new_line_no: Some(new_line),
                });
                new_line += 1;
                diff.total_additions += 1;
            } else if line.starts_with('-') {
                hunk.lines.push(DiffLine {
                    kind: DiffLineKind::Deletion,
                    content: line[1..].to_string(),
                    old_line_no: Some(old_line),
                    new_line_no: None,
                });
                old_line += 1;
                diff.total_deletions += 1;
            } else if line.starts_with(' ') || line.is_empty() {
                let content = if line.is_empty() {
                    String::new()
                } else {
                    line[1..].to_string()
                };
                hunk.lines.push(DiffLine {
                    kind: DiffLineKind::Context,
                    content,
                    old_line_no: Some(old_line),
                    new_line_no: Some(new_line),
                });
                old_line += 1;
                new_line += 1;
            }
            // Skip "\ No newline at end of file" and other noise
        }
    }

    // Flush remaining
    if let Some(hunk) = current_hunk.take() {
        if let Some(ref mut file) = current_file {
            file.hunks.push(hunk);
        }
    }
    if let Some(file) = current_file.take() {
        diff.files.push(file);
    }

    diff
}

fn parse_diff_header_path(line: &str) -> String {
    // "diff --git a/path b/path" -> "path"
    if let Some(b_part) = line.rsplit_once(" b/") {
        b_part.1.to_string()
    } else {
        line.to_string()
    }
}

fn parse_hunk_header(line: &str) -> (u32, u32, u32, u32) {
    // "@@ -old_start,old_count +new_start,new_count @@"
    let mut old_start = 1u32;
    let mut old_count = 1u32;
    let mut new_start = 1u32;
    let mut new_count = 1u32;

    if let Some(at_content) = line.strip_prefix("@@ ") {
        if let Some(end) = at_content.find(" @@") {
            let range_str = &at_content[..end];
            let parts: Vec<&str> = range_str.split_whitespace().collect();
            for part in parts {
                if let Some(old) = part.strip_prefix('-') {
                    let nums: Vec<&str> = old.split(',').collect();
                    if !nums.is_empty() {
                        old_start = nums[0].parse().unwrap_or(1);
                    }
                    if nums.len() > 1 {
                        old_count = nums[1].parse().unwrap_or(1);
                    }
                } else if let Some(new) = part.strip_prefix('+') {
                    let nums: Vec<&str> = new.split(',').collect();
                    if !nums.is_empty() {
                        new_start = nums[0].parse().unwrap_or(1);
                    }
                    if nums.len() > 1 {
                        new_count = nums[1].parse().unwrap_or(1);
                    }
                }
            }
        }
    }

    (old_start, old_count, new_start, new_count)
}

/// Parse `git log` output with NUL-separated fields.
pub fn parse_log_output(output: &str) -> Vec<CommitNode> {
    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\0').collect();
            if parts.len() < 7 {
                return None;
            }
            let parents: Vec<String> = if parts[2].is_empty() {
                Vec::new()
            } else {
                parts[2].split(' ').map(|s| s.to_string()).collect()
            };
            let refs: Vec<String> = if parts.len() > 7 && !parts[7].is_empty() {
                parts[7]
                    .split(", ")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            } else {
                Vec::new()
            };
            Some(CommitNode {
                sha: parts[0].to_string(),
                short_sha: parts[1].to_string(),
                parents,
                author_name: parts[3].to_string(),
                author_email: parts[4].to_string(),
                date: parts[5].to_string(),
                message: parts[6].to_string(),
                refs,
            })
        })
        .collect()
}

/// Parse `git for-each-ref` output for branches.
pub fn parse_branch_list(output: &str) -> Vec<BranchInfo> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\0').collect();
            if parts.len() < 6 {
                return None;
            }
            let name = parts[0].to_string();
            let tip_sha = parts[1].to_string();
            let is_head = parts[2].trim() == "*";
            let upstream = if parts[3].is_empty() {
                None
            } else {
                Some(parts[3].to_string())
            };

            // Parse ahead/behind from track info like "ahead 2, behind 1"
            let (ahead, behind) = parse_track_info(parts[4]);

            let last_commit_message = if parts[5].is_empty() {
                None
            } else {
                Some(parts[5].to_string())
            };

            Some(BranchInfo {
                name,
                is_head,
                tip_sha,
                upstream,
                ahead,
                behind,
                last_commit_message,
            })
        })
        .collect()
}

fn parse_track_info(track: &str) -> (u32, u32) {
    let mut ahead = 0u32;
    let mut behind = 0u32;

    if track.is_empty() {
        return (ahead, behind);
    }

    for part in track.split(", ") {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("ahead ") {
            ahead = val.parse().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("behind ") {
            behind = val.parse().unwrap_or(0);
        }
    }

    (ahead, behind)
}

/// Parse `git stash list` output.
pub fn parse_stash_list(output: &str) -> Vec<StashEntry> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\0').collect();
            if parts.len() < 3 {
                return None;
            }
            // parts[0] = "stash@{0}", parts[1] = message, parts[2] = date
            let index = parts[0]
                .strip_prefix("stash@{")
                .and_then(|s| s.strip_suffix('}'))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            Some(StashEntry {
                index,
                message: parts[1].to_string(),
                date: parts[2].to_string(),
            })
        })
        .collect()
}

/// Parse `git remote -v` output.
pub fn parse_remotes(output: &str) -> Vec<RemoteInfo> {
    let mut map = std::collections::HashMap::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let name = parts[0].to_string();
        let url = parts[1].to_string();
        let kind = parts[2]; // "(fetch)" or "(push)"

        let entry = map.entry(name.clone()).or_insert(RemoteInfo {
            name,
            fetch_url: String::new(),
            push_url: String::new(),
        });

        if kind.contains("fetch") {
            entry.fetch_url = url;
        } else if kind.contains("push") {
            entry.push_url = url;
        }
    }

    map.into_values().collect()
}

/// Parse `git for-each-ref` output for remote branches.
pub fn parse_remote_branch_list(output: &str) -> Vec<RemoteBranchInfo> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\0').collect();
            if parts.len() < 2 {
                return None;
            }
            let full_name = parts[0].to_string(); // e.g. "origin/main"
            let tip_sha = parts[1].to_string();

            // Skip HEAD references like "origin/HEAD"
            if full_name.ends_with("/HEAD") {
                return None;
            }

            // Split into remote and branch
            let (remote, branch) = if let Some(pos) = full_name.find('/') {
                (
                    full_name[..pos].to_string(),
                    full_name[pos + 1..].to_string(),
                )
            } else {
                return None;
            };

            Some(RemoteBranchInfo {
                name: full_name,
                remote,
                branch,
                tip_sha,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_porcelain_v2_branch_info() {
        let output = "# branch.oid abc123\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +3 -1\n";
        let status = parse_porcelain_v2(output).unwrap();
        assert_eq!(status.branch, "main");
        assert_eq!(status.upstream, Some("origin/main".to_string()));
        assert_eq!(status.ahead, 3);
        assert_eq!(status.behind, 1);
    }

    #[test]
    fn test_parse_porcelain_v2_ordinary_entries() {
        let output = "# branch.head main\n1 M. N... 100644 100644 100644 abc123 def456 src/main.rs\n1 .M N... 100644 100644 100644 abc123 def456 src/lib.rs\n";
        let status = parse_porcelain_v2(output).unwrap();
        assert_eq!(status.staged.len(), 1);
        assert_eq!(status.staged[0].path, "src/main.rs");
        assert_eq!(status.staged[0].kind, FileStatusKind::Modified);
        assert_eq!(status.unstaged.len(), 1);
        assert_eq!(status.unstaged[0].path, "src/lib.rs");
    }

    #[test]
    fn test_parse_porcelain_v2_untracked() {
        let output = "# branch.head main\n? new_file.rs\n? another.txt\n";
        let status = parse_porcelain_v2(output).unwrap();
        assert_eq!(status.untracked.len(), 2);
        assert_eq!(status.untracked[0].path, "new_file.rs");
        assert_eq!(status.untracked[0].kind, FileStatusKind::Untracked);
    }

    #[test]
    fn test_parse_porcelain_v2_unmerged() {
        let output = "# branch.head main\nu UU N... 100644 100644 100644 100644 abc123 def456 ghi789 conflict.rs\n";
        let status = parse_porcelain_v2(output).unwrap();
        assert_eq!(status.conflicted.len(), 1);
        assert_eq!(status.conflicted[0].path, "conflict.rs");
        assert_eq!(status.conflicted[0].kind, FileStatusKind::Unmerged);
    }

    #[test]
    fn test_parse_porcelain_v2_rename() {
        let output = "# branch.head main\n2 R. N... 100644 100644 100644 abc123 def456 R100 new.rs\told.rs\n";
        let status = parse_porcelain_v2(output).unwrap();
        assert_eq!(status.staged.len(), 1);
        assert_eq!(status.staged[0].kind, FileStatusKind::Renamed);
        assert_eq!(status.staged[0].path, "new.rs");
        assert_eq!(status.staged[0].original_path, Some("old.rs".to_string()));
    }

    #[test]
    fn test_parse_porcelain_v2_empty() {
        let output = "# branch.head main\n";
        let status = parse_porcelain_v2(output).unwrap();
        assert!(status.is_clean());
        assert_eq!(status.branch, "main");
    }

    #[test]
    fn test_parse_unified_diff_simple() {
        let output = r#"diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 use std;
+use serde;
 fn main() {
 }
"#;
        let diff = parse_unified_diff(output);
        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].path, "src/main.rs");
        assert_eq!(diff.files[0].hunks.len(), 1);
        assert_eq!(diff.total_additions, 1);
        assert_eq!(diff.total_deletions, 0);
    }

    #[test]
    fn test_parse_unified_diff_new_file() {
        let output = r#"diff --git a/new.rs b/new.rs
new file mode 100644
index 0000000..abc123
--- /dev/null
+++ b/new.rs
@@ -0,0 +1,2 @@
+fn new_func() {
+}
"#;
        let diff = parse_unified_diff(output);
        assert_eq!(diff.files.len(), 1);
        assert!(diff.files[0].is_new);
        assert_eq!(diff.total_additions, 2);
    }

    #[test]
    fn test_parse_unified_diff_deleted_file() {
        let output = r#"diff --git a/old.rs b/old.rs
deleted file mode 100644
index abc123..0000000
--- a/old.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-fn old_func() {
-}
"#;
        let diff = parse_unified_diff(output);
        assert_eq!(diff.files.len(), 1);
        assert!(diff.files[0].is_deleted);
        assert_eq!(diff.total_deletions, 2);
    }

    #[test]
    fn test_parse_unified_diff_multiple_files() {
        let output = r#"diff --git a/a.rs b/a.rs
index abc..def 100644
--- a/a.rs
+++ b/a.rs
@@ -1,1 +1,2 @@
 line1
+line2
diff --git a/b.rs b/b.rs
index ghi..jkl 100644
--- a/b.rs
+++ b/b.rs
@@ -1,2 +1,1 @@
 line1
-line2
"#;
        let diff = parse_unified_diff(output);
        assert_eq!(diff.files.len(), 2);
        assert_eq!(diff.total_additions, 1);
        assert_eq!(diff.total_deletions, 1);
    }

    #[test]
    fn test_parse_unified_diff_empty() {
        let diff = parse_unified_diff("");
        assert!(diff.files.is_empty());
        assert_eq!(diff.total_additions, 0);
        assert_eq!(diff.total_deletions, 0);
    }

    #[test]
    fn test_parse_log_output() {
        let output = "abc123full\x00abc123\x00parent1 parent2\x00Alice\x00alice@ex.com\x002026-02-19T10:00:00+00:00\x00feat: add stuff\x00HEAD -> main, origin/main\n\
                       def456full\x00def456\x00\x00Bob\x00bob@ex.com\x002026-02-18T10:00:00+00:00\x00initial commit\x00\n";
        let commits = parse_log_output(output);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].sha, "abc123full");
        assert_eq!(commits[0].parents, vec!["parent1", "parent2"]);
        assert_eq!(commits[0].refs, vec!["HEAD -> main", "origin/main"]);
        assert_eq!(commits[1].sha, "def456full");
        assert!(commits[1].parents.is_empty());
    }

    #[test]
    fn test_parse_branch_list() {
        let output = "main\x00abc123\x00*\x00origin/main\x00ahead 2, behind 1\x00last msg\n\
                       feature\x00def456\x00 \x00\x00\x00wip\n";
        let branches = parse_branch_list(output);
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].name, "main");
        assert!(branches[0].is_head);
        assert_eq!(branches[0].ahead, 2);
        assert_eq!(branches[0].behind, 1);
        assert_eq!(branches[1].name, "feature");
        assert!(!branches[1].is_head);
        assert!(branches[1].upstream.is_none());
    }

    #[test]
    fn test_parse_stash_list() {
        let output = "stash@{0}\x00WIP on main: abc123 work\x002026-02-19 10:00:00 +0000\n\
                       stash@{1}\x00temp save\x002026-02-18 10:00:00 +0000\n";
        let stashes = parse_stash_list(output);
        assert_eq!(stashes.len(), 2);
        assert_eq!(stashes[0].index, 0);
        assert!(stashes[0].message.contains("WIP"));
        assert_eq!(stashes[1].index, 1);
    }

    #[test]
    fn test_parse_remotes() {
        let output = "origin\thttps://github.com/user/repo.git (fetch)\n\
                       origin\tgit@github.com:user/repo.git (push)\n\
                       upstream\thttps://github.com/org/repo.git (fetch)\n\
                       upstream\thttps://github.com/org/repo.git (push)\n";
        let remotes = parse_remotes(output);
        assert_eq!(remotes.len(), 2);

        let origin = remotes.iter().find(|r| r.name == "origin").unwrap();
        assert_eq!(origin.fetch_url, "https://github.com/user/repo.git");
        assert_eq!(origin.push_url, "git@github.com:user/repo.git");
    }

    #[test]
    fn test_parse_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -1,5 +1,7 @@"), (1, 5, 1, 7));
        assert_eq!(parse_hunk_header("@@ -10,3 +12,5 @@ fn main()"), (10, 3, 12, 5));
        assert_eq!(parse_hunk_header("@@ -0,0 +1,2 @@"), (0, 0, 1, 2));
    }

    #[test]
    fn test_parse_track_info() {
        assert_eq!(parse_track_info("ahead 2, behind 1"), (2, 1));
        assert_eq!(parse_track_info("ahead 5"), (5, 0));
        assert_eq!(parse_track_info("behind 3"), (0, 3));
        assert_eq!(parse_track_info(""), (0, 0));
    }

    #[test]
    fn test_char_to_status_kind() {
        assert_eq!(char_to_status_kind('M'), FileStatusKind::Modified);
        assert_eq!(char_to_status_kind('A'), FileStatusKind::Added);
        assert_eq!(char_to_status_kind('D'), FileStatusKind::Deleted);
        assert_eq!(char_to_status_kind('R'), FileStatusKind::Renamed);
        assert_eq!(char_to_status_kind('C'), FileStatusKind::Copied);
        assert_eq!(char_to_status_kind('T'), FileStatusKind::TypeChanged);
        assert_eq!(char_to_status_kind('X'), FileStatusKind::Modified); // fallback
    }

    #[test]
    fn test_git_service_default() {
        let service = GitService::default();
        let _ = service.git_ops(); // ensure we can access it
    }

    #[test]
    fn test_parse_porcelain_v2_added_and_deleted() {
        let output = "# branch.head dev\n1 A. N... 000000 100644 100644 0000000 abc1234 added.rs\n1 D. N... 100644 000000 000000 abc1234 0000000 deleted.rs\n";
        let status = parse_porcelain_v2(output).unwrap();
        assert_eq!(status.staged.len(), 2);
        assert_eq!(status.staged[0].kind, FileStatusKind::Added);
        assert_eq!(status.staged[1].kind, FileStatusKind::Deleted);
    }

    #[test]
    fn test_parse_diff_header_path() {
        assert_eq!(parse_diff_header_path("diff --git a/src/main.rs b/src/main.rs"), "src/main.rs");
        assert_eq!(parse_diff_header_path("diff --git a/file.txt b/file.txt"), "file.txt");
    }
}
