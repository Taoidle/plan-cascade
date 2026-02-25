//! Git Operations
//!
//! Safe wrapper around git CLI operations for worktree management.

use std::path::Path;
use std::process::Command;

use crate::utils::error::{AppError, AppResult};

/// Result of a git command execution
#[derive(Debug)]
pub struct GitResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl GitResult {
    /// Check if the command was successful and return stdout or error
    pub fn into_result(self) -> AppResult<String> {
        if self.success {
            Ok(self.stdout)
        } else {
            Err(AppError::command(format!(
                "Git command failed (exit {}): {}",
                self.exit_code,
                self.stderr.trim()
            )))
        }
    }
}

/// Safe git operations wrapper
#[derive(Debug, Default)]
pub struct GitOps;

impl GitOps {
    /// Create a new GitOps instance
    pub fn new() -> Self {
        Self
    }

    /// Execute a git command in the specified directory
    pub fn execute(&self, cwd: &Path, args: &[&str]) -> AppResult<GitResult> {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            // Disable interactive prompts to avoid hanging automation flows/tests.
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .output()
            .map_err(|e| AppError::command(format!("Failed to execute git: {}", e)))?;

        Ok(GitResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    /// Execute a git command with stdin input piped in.
    pub fn execute_with_stdin(
        &self,
        cwd: &Path,
        args: &[&str],
        stdin_data: &[u8],
    ) -> AppResult<GitResult> {
        use std::io::Write;

        let mut child = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AppError::command(format!("Failed to spawn git: {}", e)))?;

        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(stdin_data)
                .map_err(|e| AppError::command(format!("Failed to write stdin: {}", e)))?;
        }
        // Drop stdin to signal EOF
        drop(child.stdin.take());

        let output = child
            .wait_with_output()
            .map_err(|e| AppError::command(format!("Failed to wait for git: {}", e)))?;

        Ok(GitResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    /// Get the repository root directory
    pub fn get_repo_root(&self, cwd: &Path) -> AppResult<String> {
        self.execute(cwd, &["rev-parse", "--show-toplevel"])?
            .into_result()
            .map(|s| s.trim().to_string())
    }

    /// Get the current branch name
    pub fn get_current_branch(&self, cwd: &Path) -> AppResult<String> {
        self.execute(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?
            .into_result()
            .map(|s| s.trim().to_string())
    }

    /// Check if a branch exists locally
    pub fn branch_exists(&self, cwd: &Path, branch: &str) -> AppResult<bool> {
        let result = self.execute(
            cwd,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{}", branch),
            ],
        )?;
        Ok(result.success)
    }

    /// Check if a remote branch exists
    pub fn remote_branch_exists(&self, cwd: &Path, branch: &str, remote: &str) -> AppResult<bool> {
        let result = self.execute(cwd, &["ls-remote", "--heads", remote, branch])?;
        Ok(result.success && !result.stdout.trim().is_empty())
    }

    /// Create a new branch from a base branch
    pub fn create_branch(&self, cwd: &Path, branch: &str, base: &str) -> AppResult<()> {
        self.execute(cwd, &["branch", branch, base])?
            .into_result()?;
        Ok(())
    }

    /// Delete a local branch
    pub fn delete_branch(&self, cwd: &Path, branch: &str, force: bool) -> AppResult<()> {
        let flag = if force { "-D" } else { "-d" };
        self.execute(cwd, &["branch", flag, branch])?
            .into_result()?;
        Ok(())
    }

    /// Add a worktree
    pub fn add_worktree(&self, cwd: &Path, path: &Path, branch: &str) -> AppResult<()> {
        let path_str = path.to_string_lossy();
        self.execute(cwd, &["worktree", "add", &path_str, branch])?
            .into_result()?;
        Ok(())
    }

    /// Add a worktree with a new branch
    pub fn add_worktree_with_new_branch(
        &self,
        cwd: &Path,
        path: &Path,
        branch: &str,
        base: &str,
    ) -> AppResult<()> {
        let path_str = path.to_string_lossy();
        self.execute(cwd, &["worktree", "add", "-b", branch, &path_str, base])?
            .into_result()?;
        Ok(())
    }

    /// Remove a worktree
    pub fn remove_worktree(&self, cwd: &Path, path: &Path, force: bool) -> AppResult<()> {
        let path_str = path.to_string_lossy();
        let args = if force {
            vec!["worktree", "remove", "--force", &path_str]
        } else {
            vec!["worktree", "remove", &path_str]
        };
        self.execute(cwd, &args)?.into_result()?;
        Ok(())
    }

    /// Prune stale worktrees
    pub fn prune_worktrees(&self, cwd: &Path) -> AppResult<()> {
        self.execute(cwd, &["worktree", "prune"])?.into_result()?;
        Ok(())
    }

    /// List all worktrees
    pub fn list_worktrees(&self, cwd: &Path) -> AppResult<Vec<WorktreeInfo>> {
        let output = self
            .execute(cwd, &["worktree", "list", "--porcelain"])?
            .into_result()?;

        let mut worktrees = Vec::new();
        let mut current = WorktreeInfo::default();

        for line in output.lines() {
            if line.starts_with("worktree ") {
                if !current.path.is_empty() {
                    worktrees.push(current);
                    current = WorktreeInfo::default();
                }
                current.path = line.strip_prefix("worktree ").unwrap_or("").to_string();
            } else if line.starts_with("HEAD ") {
                current.head = line.strip_prefix("HEAD ").unwrap_or("").to_string();
            } else if line.starts_with("branch ") {
                current.branch = line
                    .strip_prefix("branch refs/heads/")
                    .unwrap_or(line.strip_prefix("branch ").unwrap_or(""))
                    .to_string();
            } else if line == "bare" {
                current.is_bare = true;
            } else if line == "detached" {
                current.is_detached = true;
            } else if line == "locked" {
                current.is_locked = true;
            } else if line.starts_with("prunable ") {
                current.is_prunable = true;
            }
        }

        if !current.path.is_empty() {
            worktrees.push(current);
        }

        Ok(worktrees)
    }

    /// Stage files (git add)
    pub fn add(&self, cwd: &Path, paths: &[&str]) -> AppResult<()> {
        let mut args = vec!["add"];
        args.extend(paths);
        self.execute(cwd, &args)?.into_result()?;
        Ok(())
    }

    /// Stage all changes
    pub fn add_all(&self, cwd: &Path) -> AppResult<()> {
        self.execute(cwd, &["add", "-A"])?.into_result()?;
        Ok(())
    }

    /// Commit changes
    pub fn commit(&self, cwd: &Path, message: &str) -> AppResult<String> {
        let result = self.execute(cwd, &["commit", "-m", message])?;
        if result.success {
            // Get the commit SHA
            let sha = self
                .execute(cwd, &["rev-parse", "HEAD"])?
                .into_result()?
                .trim()
                .to_string();
            Ok(sha)
        } else {
            Err(AppError::command(format!(
                "Commit failed: {}",
                result.stderr.trim()
            )))
        }
    }

    /// Checkout a branch
    pub fn checkout(&self, cwd: &Path, branch: &str) -> AppResult<()> {
        self.execute(cwd, &["checkout", branch])?.into_result()?;
        Ok(())
    }

    /// Merge a branch into the current branch
    pub fn merge(&self, cwd: &Path, branch: &str, message: Option<&str>) -> AppResult<MergeResult> {
        let args = match message {
            Some(msg) => vec!["merge", "--no-ff", "-m", msg, branch],
            None => vec!["merge", "--no-ff", branch],
        };

        let result = self.execute(cwd, &args)?;

        if result.success {
            Ok(MergeResult::Success)
        } else if result.stdout.contains("CONFLICT") || result.stderr.contains("CONFLICT") {
            // Get conflicting files
            let conflicts = self.get_conflicting_files(cwd)?;
            Ok(MergeResult::Conflict(conflicts))
        } else {
            Ok(MergeResult::Error(result.stderr.trim().to_string()))
        }
    }

    /// Abort a merge in progress
    pub fn merge_abort(&self, cwd: &Path) -> AppResult<()> {
        self.execute(cwd, &["merge", "--abort"])?.into_result()?;
        Ok(())
    }

    /// Get list of conflicting files
    pub fn get_conflicting_files(&self, cwd: &Path) -> AppResult<Vec<String>> {
        let output = self
            .execute(cwd, &["diff", "--name-only", "--diff-filter=U"])?
            .into_result()?;

        Ok(output
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    /// Get status of the working directory
    pub fn status(&self, cwd: &Path) -> AppResult<GitStatus> {
        let output = self
            .execute(cwd, &["status", "--porcelain"])?
            .into_result()?;

        let mut status = GitStatus::default();

        for line in output.lines() {
            if line.len() < 3 {
                continue;
            }

            let index_status = line.chars().next().unwrap_or(' ');
            let work_status = line.chars().nth(1).unwrap_or(' ');
            let file = line[3..].to_string();

            match (index_status, work_status) {
                ('?', '?') => status.untracked.push(file),
                ('A', _) => status.staged.push(file),
                ('M', _) | (_, 'M') => {
                    if index_status != ' ' {
                        status.staged.push(file.clone());
                    }
                    if work_status != ' ' {
                        status.modified.push(file);
                    }
                }
                ('D', _) | (_, 'D') => status.deleted.push(file),
                ('R', _) => status.renamed.push(file),
                ('U', _) | (_, 'U') => status.conflicted.push(file),
                _ => {}
            }
        }

        Ok(status)
    }

    /// Check if working directory is clean
    pub fn is_clean(&self, cwd: &Path) -> AppResult<bool> {
        let status = self.status(cwd)?;
        Ok(status.is_clean())
    }

    /// Get commit history
    pub fn log(&self, cwd: &Path, count: usize) -> AppResult<Vec<CommitInfo>> {
        let output = self
            .execute(
                cwd,
                &["log", &format!("-{}", count), "--format=%H|%s|%an|%ai"],
            )?
            .into_result()?;

        let commits = output
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(4, '|').collect();
                if parts.len() >= 4 {
                    Some(CommitInfo {
                        sha: parts[0].to_string(),
                        message: parts[1].to_string(),
                        author: parts[2].to_string(),
                        date: parts[3].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(commits)
    }

    /// Reset staging area (unstage all files)
    pub fn reset_staging(&self, cwd: &Path) -> AppResult<()> {
        self.execute(cwd, &["reset", "HEAD"])?.into_result()?;
        Ok(())
    }

    /// Check if path is excluded by gitignore pattern
    pub fn check_ignore(&self, cwd: &Path, path: &str) -> AppResult<bool> {
        let result = self.execute(cwd, &["check-ignore", "-q", path])?;
        Ok(result.success)
    }
}

/// Information about a git worktree
#[derive(Debug, Default, Clone)]
pub struct WorktreeInfo {
    pub path: String,
    pub head: String,
    pub branch: String,
    pub is_bare: bool,
    pub is_detached: bool,
    pub is_locked: bool,
    pub is_prunable: bool,
}

/// Result of a merge operation
#[derive(Debug)]
pub enum MergeResult {
    Success,
    Conflict(Vec<String>),
    Error(String),
}

/// Git working directory status
#[derive(Debug, Default)]
pub struct GitStatus {
    pub staged: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
    pub renamed: Vec<String>,
    pub untracked: Vec<String>,
    pub conflicted: Vec<String>,
}

impl GitStatus {
    /// Check if the working directory is clean
    pub fn is_clean(&self) -> bool {
        self.staged.is_empty()
            && self.modified.is_empty()
            && self.deleted.is_empty()
            && self.renamed.is_empty()
            && self.untracked.is_empty()
            && self.conflicted.is_empty()
    }

    /// Get total number of changes
    pub fn change_count(&self) -> usize {
        self.staged.len()
            + self.modified.len()
            + self.deleted.len()
            + self.renamed.len()
            + self.untracked.len()
            + self.conflicted.len()
    }
}

/// Git commit information
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_status_is_clean() {
        let status = GitStatus::default();
        assert!(status.is_clean());

        let mut status = GitStatus::default();
        status.modified.push("file.txt".to_string());
        assert!(!status.is_clean());
    }

    #[test]
    fn test_git_status_change_count() {
        let mut status = GitStatus::default();
        status.modified.push("a.txt".to_string());
        status.staged.push("b.txt".to_string());
        status.untracked.push("c.txt".to_string());
        assert_eq!(status.change_count(), 3);
    }

    #[test]
    fn test_git_result_into_result() {
        let success = GitResult {
            success: true,
            stdout: "output".to_string(),
            stderr: "".to_string(),
            exit_code: 0,
        };
        assert_eq!(success.into_result().unwrap(), "output");

        let failure = GitResult {
            success: false,
            stdout: "".to_string(),
            stderr: "error message".to_string(),
            exit_code: 1,
        };
        assert!(failure.into_result().is_err());
    }
}
