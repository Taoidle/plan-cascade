//! Git Service Shared Data Types
//!
//! Core types for the git service module, used across service, graph,
//! conflict resolution, LLM assist, and Tauri commands.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// File Status
// ---------------------------------------------------------------------------

/// Kind of change for a single file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatusKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Ignored,
    Unmerged,
    TypeChanged,
}

/// Status of a single file in the working tree or index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    /// Relative path within the repository.
    pub path: String,
    /// Original path (populated for renames/copies).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    /// Kind of change.
    pub kind: FileStatusKind,
}

/// Full git status combining staged, unstaged, and untracked files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitFullStatus {
    /// Files staged in the index.
    pub staged: Vec<FileStatus>,
    /// Files modified in the working tree (unstaged).
    pub unstaged: Vec<FileStatus>,
    /// Untracked files.
    pub untracked: Vec<FileStatus>,
    /// Files with merge conflicts.
    pub conflicted: Vec<FileStatus>,
    /// Current branch name (may be empty for detached HEAD).
    pub branch: String,
    /// Upstream branch name if tracking is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    /// Commits ahead of upstream.
    pub ahead: u32,
    /// Commits behind upstream.
    pub behind: u32,
}

impl GitFullStatus {
    /// Check if the working tree is completely clean.
    pub fn is_clean(&self) -> bool {
        self.staged.is_empty()
            && self.unstaged.is_empty()
            && self.untracked.is_empty()
            && self.conflicted.is_empty()
    }

    /// Total number of changed entries.
    pub fn change_count(&self) -> usize {
        self.staged.len() + self.unstaged.len() + self.untracked.len() + self.conflicted.len()
    }
}

// ---------------------------------------------------------------------------
// Commit / Log
// ---------------------------------------------------------------------------

/// A single commit node in the history graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitNode {
    /// Full SHA-1 hash.
    pub sha: String,
    /// Abbreviated SHA.
    pub short_sha: String,
    /// Parent SHA(s). Empty for root commits, two+ for merges.
    pub parents: Vec<String>,
    /// Author name.
    pub author_name: String,
    /// Author email.
    pub author_email: String,
    /// Author date in ISO-8601 format.
    pub date: String,
    /// Full commit message (first line = subject).
    pub message: String,
    /// Ref names decorating this commit (branches, tags).
    #[serde(default)]
    pub refs: Vec<String>,
}

// ---------------------------------------------------------------------------
// Branches
// ---------------------------------------------------------------------------

/// Information about a local branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    /// Branch name (e.g. "main", "feature/xyz").
    pub name: String,
    /// Whether this is the currently checked-out branch.
    pub is_head: bool,
    /// SHA of the branch tip.
    pub tip_sha: String,
    /// Upstream tracking branch (e.g. "origin/main").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    /// Commits ahead of upstream.
    pub ahead: u32,
    /// Commits behind upstream.
    pub behind: u32,
    /// Last commit message on this branch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit_message: Option<String>,
}

/// Information about a remote branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteBranchInfo {
    /// Full ref name (e.g. "origin/main").
    pub name: String,
    /// Remote name (e.g. "origin").
    pub remote: String,
    /// Branch name on the remote.
    pub branch: String,
    /// SHA of the remote branch tip.
    pub tip_sha: String,
}

// ---------------------------------------------------------------------------
// Stash
// ---------------------------------------------------------------------------

/// A single stash entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashEntry {
    /// Stash index (0 = most recent).
    pub index: u32,
    /// Stash message.
    pub message: String,
    /// Date when stash was created.
    pub date: String,
}

// ---------------------------------------------------------------------------
// Conflicts
// ---------------------------------------------------------------------------

/// Side of a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictSide {
    Ours,
    Theirs,
    Ancestor,
}

/// A single conflicting file with marker positions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictFile {
    /// File path relative to repo root.
    pub path: String,
    /// Number of conflict regions in the file.
    pub conflict_count: u32,
}

/// A parsed region of conflict within a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictRegion {
    /// Content from the "ours" side.
    pub ours: String,
    /// Content from the "theirs" side.
    pub theirs: String,
    /// Content from the common ancestor (diff3 style). None for 2-way conflicts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ancestor: Option<String>,
    /// Start line (1-based) in the original file.
    pub start_line: u32,
    /// End line (1-based) in the original file.
    pub end_line: u32,
}

/// Strategy for resolving a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    /// Keep our version.
    Ours,
    /// Keep their version.
    Theirs,
    /// Keep both (ours then theirs).
    Both,
}

// ---------------------------------------------------------------------------
// Merge State
// ---------------------------------------------------------------------------

/// Kind of in-progress merge-like operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStateKind {
    /// No merge in progress.
    None,
    /// A regular merge is in progress.
    Merging,
    /// A rebase is in progress.
    Rebasing,
    /// A cherry-pick is in progress.
    CherryPicking,
    /// A revert is in progress.
    Reverting,
}

/// State of any in-progress merge-like operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeState {
    /// Kind of operation.
    pub kind: MergeStateKind,
    /// Current HEAD SHA.
    pub head: String,
    /// MERGE_HEAD / REBASE_HEAD SHA (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_head: Option<String>,
    /// Name of the branch being merged / rebased onto (if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Graph Layout (DAG visualization)
// ---------------------------------------------------------------------------

/// A node in the commit graph layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// SHA of the commit.
    pub sha: String,
    /// Row index (0 = most recent commit).
    pub row: u32,
    /// Lane (column) assignment.
    pub lane: u32,
}

/// An edge connecting two nodes in the commit graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// SHA of the child (newer) commit.
    pub from_sha: String,
    /// SHA of the parent (older) commit.
    pub to_sha: String,
    /// Lane of the child commit.
    pub from_lane: u32,
    /// Lane of the parent commit.
    pub to_lane: u32,
    /// Row of the child commit.
    pub from_row: u32,
    /// Row of the parent commit.
    pub to_row: u32,
}

/// Complete graph layout for rendering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphLayout {
    /// Positioned nodes.
    pub nodes: Vec<GraphNode>,
    /// Edges between nodes.
    pub edges: Vec<GraphEdge>,
    /// Maximum lane used (for sizing the graph width).
    pub max_lane: u32,
}

// ---------------------------------------------------------------------------
// Diff
// ---------------------------------------------------------------------------

/// Kind of change in a diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    /// Context line (unchanged).
    Context,
    /// Added line.
    Addition,
    /// Deleted line.
    Deletion,
    /// Hunk header.
    HunkHeader,
}

/// A single line in a diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    /// Kind of this line.
    pub kind: DiffLineKind,
    /// The content of the line (without the leading +/-/space).
    pub content: String,
    /// Old file line number (None for additions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_line_no: Option<u32>,
    /// New file line number (None for deletions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_line_no: Option<u32>,
}

/// A hunk (section) within a diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    /// Hunk header string (e.g. "@@ -1,5 +1,7 @@").
    pub header: String,
    /// Old file start line.
    pub old_start: u32,
    /// Old file line count.
    pub old_count: u32,
    /// New file start line.
    pub new_start: u32,
    /// New file line count.
    pub new_count: u32,
    /// Lines within this hunk.
    pub lines: Vec<DiffLine>,
}

/// Diff output for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    /// Path of the file.
    pub path: String,
    /// Whether this is a new file.
    pub is_new: bool,
    /// Whether this is a deleted file.
    pub is_deleted: bool,
    /// Whether this is a renamed file.
    pub is_renamed: bool,
    /// Original path (for renames).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    /// Hunks in this diff.
    pub hunks: Vec<DiffHunk>,
}

/// Complete diff output (may span multiple files).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffOutput {
    /// Per-file diffs.
    pub files: Vec<FileDiff>,
    /// Total additions across all files.
    pub total_additions: u32,
    /// Total deletions across all files.
    pub total_deletions: u32,
}

// ---------------------------------------------------------------------------
// Remote
// ---------------------------------------------------------------------------

/// A git remote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInfo {
    /// Remote name (e.g. "origin").
    pub name: String,
    /// Fetch URL.
    pub fetch_url: String,
    /// Push URL.
    pub push_url: String,
}

// ---------------------------------------------------------------------------
// Git watcher events
// ---------------------------------------------------------------------------

/// Payload emitted for git file watcher events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitWatchEvent {
    /// Repository path that triggered the event.
    pub repo_path: String,
    /// What changed: "index" or "head".
    pub change_kind: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_full_status_is_clean() {
        let status = GitFullStatus::default();
        assert!(status.is_clean());
        assert_eq!(status.change_count(), 0);
    }

    #[test]
    fn test_git_full_status_not_clean() {
        let mut status = GitFullStatus::default();
        status.staged.push(FileStatus {
            path: "file.rs".to_string(),
            original_path: None,
            kind: FileStatusKind::Modified,
        });
        assert!(!status.is_clean());
        assert_eq!(status.change_count(), 1);
    }

    #[test]
    fn test_git_full_status_change_count() {
        let mut status = GitFullStatus::default();
        status.staged.push(FileStatus {
            path: "a.rs".to_string(),
            original_path: None,
            kind: FileStatusKind::Added,
        });
        status.unstaged.push(FileStatus {
            path: "b.rs".to_string(),
            original_path: None,
            kind: FileStatusKind::Modified,
        });
        status.untracked.push(FileStatus {
            path: "c.rs".to_string(),
            original_path: None,
            kind: FileStatusKind::Untracked,
        });
        status.conflicted.push(FileStatus {
            path: "d.rs".to_string(),
            original_path: None,
            kind: FileStatusKind::Unmerged,
        });
        assert_eq!(status.change_count(), 4);
    }

    #[test]
    fn test_file_status_serialization_roundtrip() {
        let status = FileStatus {
            path: "src/main.rs".to_string(),
            original_path: Some("src/old.rs".to_string()),
            kind: FileStatusKind::Renamed,
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: FileStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, "src/main.rs");
        assert_eq!(parsed.original_path, Some("src/old.rs".to_string()));
        assert_eq!(parsed.kind, FileStatusKind::Renamed);
    }

    #[test]
    fn test_commit_node_serialization_roundtrip() {
        let node = CommitNode {
            sha: "abc123def456".to_string(),
            short_sha: "abc123d".to_string(),
            parents: vec!["parent1".to_string(), "parent2".to_string()],
            author_name: "Alice".to_string(),
            author_email: "alice@example.com".to_string(),
            date: "2026-02-19T10:00:00Z".to_string(),
            message: "feat: add feature".to_string(),
            refs: vec!["HEAD -> main".to_string(), "origin/main".to_string()],
        };
        let json = serde_json::to_string(&node).unwrap();
        let parsed: CommitNode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.sha, "abc123def456");
        assert_eq!(parsed.parents.len(), 2);
        assert_eq!(parsed.refs.len(), 2);
    }

    #[test]
    fn test_branch_info_serialization_roundtrip() {
        let branch = BranchInfo {
            name: "feature/test".to_string(),
            is_head: true,
            tip_sha: "abc123".to_string(),
            upstream: Some("origin/feature/test".to_string()),
            ahead: 2,
            behind: 1,
            last_commit_message: Some("wip".to_string()),
        };
        let json = serde_json::to_string(&branch).unwrap();
        let parsed: BranchInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "feature/test");
        assert!(parsed.is_head);
        assert_eq!(parsed.ahead, 2);
        assert_eq!(parsed.behind, 1);
    }

    #[test]
    fn test_stash_entry_serialization_roundtrip() {
        let entry = StashEntry {
            index: 0,
            message: "WIP on main: abc123 some work".to_string(),
            date: "2026-02-19T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: StashEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.index, 0);
        assert!(parsed.message.contains("WIP"));
    }

    #[test]
    fn test_conflict_region_serialization_roundtrip() {
        let region = ConflictRegion {
            ours: "our content\n".to_string(),
            theirs: "their content\n".to_string(),
            ancestor: Some("ancestor content\n".to_string()),
            start_line: 10,
            end_line: 20,
        };
        let json = serde_json::to_string(&region).unwrap();
        let parsed: ConflictRegion = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ours, "our content\n");
        assert_eq!(parsed.ancestor, Some("ancestor content\n".to_string()));
    }

    #[test]
    fn test_merge_state_serialization_roundtrip() {
        let state = MergeState {
            kind: MergeStateKind::Merging,
            head: "abc123".to_string(),
            merge_head: Some("def456".to_string()),
            branch_name: Some("feature/x".to_string()),
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: MergeState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.kind, MergeStateKind::Merging);
        assert_eq!(parsed.merge_head, Some("def456".to_string()));
    }

    #[test]
    fn test_merge_state_kind_none() {
        let state = MergeState {
            kind: MergeStateKind::None,
            head: "abc123".to_string(),
            merge_head: None,
            branch_name: None,
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"none\""));
    }

    #[test]
    fn test_graph_layout_serialization_roundtrip() {
        let layout = GraphLayout {
            nodes: vec![GraphNode {
                sha: "abc123".to_string(),
                row: 0,
                lane: 0,
            }],
            edges: vec![GraphEdge {
                from_sha: "abc123".to_string(),
                to_sha: "def456".to_string(),
                from_lane: 0,
                to_lane: 0,
                from_row: 0,
                to_row: 1,
            }],
            max_lane: 0,
        };
        let json = serde_json::to_string(&layout).unwrap();
        let parsed: GraphLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.nodes.len(), 1);
        assert_eq!(parsed.edges.len(), 1);
        assert_eq!(parsed.max_lane, 0);
    }

    #[test]
    fn test_diff_output_serialization_roundtrip() {
        let diff = DiffOutput {
            files: vec![FileDiff {
                path: "src/main.rs".to_string(),
                is_new: false,
                is_deleted: false,
                is_renamed: false,
                old_path: None,
                hunks: vec![DiffHunk {
                    header: "@@ -1,5 +1,7 @@".to_string(),
                    old_start: 1,
                    old_count: 5,
                    new_start: 1,
                    new_count: 7,
                    lines: vec![
                        DiffLine {
                            kind: DiffLineKind::Context,
                            content: "use std;".to_string(),
                            old_line_no: Some(1),
                            new_line_no: Some(1),
                        },
                        DiffLine {
                            kind: DiffLineKind::Addition,
                            content: "use serde;".to_string(),
                            old_line_no: None,
                            new_line_no: Some(2),
                        },
                    ],
                }],
            }],
            total_additions: 1,
            total_deletions: 0,
        };
        let json = serde_json::to_string(&diff).unwrap();
        let parsed: DiffOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].hunks[0].lines.len(), 2);
        assert_eq!(parsed.total_additions, 1);
    }

    #[test]
    fn test_conflict_strategy_serialization() {
        assert_eq!(
            serde_json::to_string(&ConflictStrategy::Ours).unwrap(),
            "\"ours\""
        );
        assert_eq!(
            serde_json::to_string(&ConflictStrategy::Theirs).unwrap(),
            "\"theirs\""
        );
        assert_eq!(
            serde_json::to_string(&ConflictStrategy::Both).unwrap(),
            "\"both\""
        );
    }

    #[test]
    fn test_file_status_kind_all_variants() {
        let kinds = vec![
            FileStatusKind::Added,
            FileStatusKind::Modified,
            FileStatusKind::Deleted,
            FileStatusKind::Renamed,
            FileStatusKind::Copied,
            FileStatusKind::Untracked,
            FileStatusKind::Ignored,
            FileStatusKind::Unmerged,
            FileStatusKind::TypeChanged,
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            let parsed: FileStatusKind = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn test_remote_info_serialization_roundtrip() {
        let remote = RemoteInfo {
            name: "origin".to_string(),
            fetch_url: "https://github.com/user/repo.git".to_string(),
            push_url: "git@github.com:user/repo.git".to_string(),
        };
        let json = serde_json::to_string(&remote).unwrap();
        let parsed: RemoteInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "origin");
    }

    #[test]
    fn test_git_watch_event_serialization() {
        let event = GitWatchEvent {
            repo_path: "/path/to/repo".to_string(),
            change_kind: "index".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: GitWatchEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.change_kind, "index");
    }

    #[test]
    fn test_diff_line_kind_all_variants() {
        let kinds = vec![
            DiffLineKind::Context,
            DiffLineKind::Addition,
            DiffLineKind::Deletion,
            DiffLineKind::HunkHeader,
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            let parsed: DiffLineKind = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn test_remote_branch_info_serialization() {
        let info = RemoteBranchInfo {
            name: "origin/main".to_string(),
            remote: "origin".to_string(),
            branch: "main".to_string(),
            tip_sha: "abc123".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: RemoteBranchInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "origin/main");
        assert_eq!(parsed.remote, "origin");
    }

    #[test]
    fn test_graph_layout_default() {
        let layout = GraphLayout::default();
        assert!(layout.nodes.is_empty());
        assert!(layout.edges.is_empty());
        assert_eq!(layout.max_lane, 0);
    }

    #[test]
    fn test_diff_output_default() {
        let diff = DiffOutput::default();
        assert!(diff.files.is_empty());
        assert_eq!(diff.total_additions, 0);
        assert_eq!(diff.total_deletions, 0);
    }
}
