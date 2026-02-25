/**
 * Git Types
 *
 * TypeScript interfaces matching Rust backend types in
 * desktop/src-tauri/src/services/git/types.rs
 */

// ---------------------------------------------------------------------------
// File Status
// ---------------------------------------------------------------------------

export type FileStatusKind =
  | 'added'
  | 'modified'
  | 'deleted'
  | 'renamed'
  | 'copied'
  | 'untracked'
  | 'ignored'
  | 'unmerged'
  | 'type_changed';

export interface FileStatus {
  path: string;
  original_path?: string;
  kind: FileStatusKind;
}

export interface GitFullStatus {
  staged: FileStatus[];
  unstaged: FileStatus[];
  untracked: FileStatus[];
  conflicted: FileStatus[];
  branch: string;
  upstream?: string;
  ahead: number;
  behind: number;
}

// ---------------------------------------------------------------------------
// Commit / Log
// ---------------------------------------------------------------------------

export interface CommitNode {
  /** Full SHA-1 hash */
  sha: string;
  /** Abbreviated SHA */
  short_sha: string;
  /** Parent SHA(s). Empty for root commits, two+ for merges. */
  parents: string[];
  /** Author name */
  author_name: string;
  /** Author email */
  author_email: string;
  /** Author date in ISO-8601 format */
  date: string;
  /** Full commit message (first line = subject) */
  message: string;
  /** Ref names decorating this commit (branches, tags) */
  refs: string[];
}

// ---------------------------------------------------------------------------
// Branches
// ---------------------------------------------------------------------------

export interface BranchInfo {
  /** Branch name (e.g. "main", "feature/xyz"). */
  name: string;
  /** Whether this is the currently checked-out branch. */
  is_head: boolean;
  /** SHA of the branch tip. */
  tip_sha: string;
  /** Upstream tracking branch (e.g. "origin/main"). */
  upstream?: string;
  /** Commits ahead of upstream. */
  ahead: number;
  /** Commits behind upstream. */
  behind: number;
  /** Last commit message on this branch. */
  last_commit_message?: string;
}

export interface RemoteBranchInfo {
  /** Full ref name (e.g. "origin/main"). */
  name: string;
  /** Remote name (e.g. "origin"). */
  remote: string;
  /** Branch name on the remote. */
  branch: string;
  /** SHA of the remote branch tip. */
  tip_sha: string;
}

// ---------------------------------------------------------------------------
// Graph Layout (DAG visualization)
// ---------------------------------------------------------------------------

export interface GraphNode {
  /** SHA of the commit */
  sha: string;
  /** Row index (0 = most recent commit) */
  row: number;
  /** Lane (column) assignment */
  lane: number;
}

export interface GraphEdge {
  /** SHA of the child (newer) commit */
  from_sha: string;
  /** SHA of the parent (older) commit */
  to_sha: string;
  /** Lane of the child commit */
  from_lane: number;
  /** Lane of the parent commit */
  to_lane: number;
  /** Row of the child commit */
  from_row: number;
  /** Row of the parent commit */
  to_row: number;
}

export interface GraphLayout {
  /** Positioned nodes */
  nodes: GraphNode[];
  /** Edges between nodes */
  edges: GraphEdge[];
  /** Maximum lane used (for sizing the graph width) */
  max_lane: number;
}

// ---------------------------------------------------------------------------
// Conflicts
// ---------------------------------------------------------------------------

export type ConflictSide = 'ours' | 'theirs' | 'ancestor';

export interface ConflictFile {
  /** File path relative to repo root. */
  path: string;
  /** Number of conflict regions in the file. */
  conflict_count: number;
}

export interface ConflictRegion {
  /** Content from the "ours" side. */
  ours: string;
  /** Content from the "theirs" side. */
  theirs: string;
  /** Content from the common ancestor (diff3 style). */
  ancestor?: string;
  /** Start line (1-based) in the original file. */
  start_line: number;
  /** End line (1-based) in the original file. */
  end_line: number;
}

export type ConflictStrategy = 'ours' | 'theirs' | 'both';

// ---------------------------------------------------------------------------
// Merge State
// ---------------------------------------------------------------------------

export type MergeStateKind = 'none' | 'merging' | 'rebasing' | 'cherry_picking' | 'reverting';

export interface MergeState {
  /** Kind of operation. */
  kind: MergeStateKind;
  /** Current HEAD SHA. */
  head: string;
  /** MERGE_HEAD / REBASE_HEAD SHA (if applicable). */
  merge_head?: string;
  /** Name of the branch being merged / rebased onto. */
  branch_name?: string;
}

export interface MergeBranchResult {
  /** Whether the merge was successful (no conflicts). */
  success: boolean;
  /** Whether there are merge conflicts. */
  has_conflicts: boolean;
  /** List of files with conflicts (if any). */
  conflicting_files: string[];
  /** Error message if the merge failed for non-conflict reasons. */
  error?: string;
}

// ---------------------------------------------------------------------------
// Worktree
// ---------------------------------------------------------------------------

export type WorktreeStatus = 'creating' | 'active' | 'in_progress' | 'ready' | 'merging' | 'completed' | 'error';

export interface Worktree {
  id: string;
  name: string;
  path: string;
  branch: string;
  target_branch: string;
  status: WorktreeStatus;
  created_at: string;
  updated_at: string;
  error?: string;
}

// ---------------------------------------------------------------------------
// Diff
// ---------------------------------------------------------------------------

export type DiffLineKind = 'context' | 'addition' | 'deletion' | 'hunk_header';

export interface DiffLine {
  kind: DiffLineKind;
  content: string;
  old_line_no?: number;
  new_line_no?: number;
}

export interface DiffHunk {
  header: string;
  old_start: number;
  old_count: number;
  new_start: number;
  new_count: number;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  is_new: boolean;
  is_deleted: boolean;
  is_renamed: boolean;
  old_path?: string;
  hunks: DiffHunk[];
}

export interface DiffOutput {
  files: FileDiff[];
  total_additions: number;
  total_deletions: number;
}

// ---------------------------------------------------------------------------
// Stash
// ---------------------------------------------------------------------------

export interface StashEntry {
  index: number;
  message: string;
  date: string;
}

// ---------------------------------------------------------------------------
// Remote
// ---------------------------------------------------------------------------

export interface RemoteInfo {
  name: string;
  fetch_url: string;
  push_url: string;
}

// ---------------------------------------------------------------------------
// Watcher
// ---------------------------------------------------------------------------

export interface GitWatchEvent {
  repo_path: string;
  change_kind: string;
}

// ---------------------------------------------------------------------------
// Commit Graph UI Types (frontend-only)
// ---------------------------------------------------------------------------

/** Combined data for a single row in the commit graph */
export interface CommitGraphRow {
  commit: CommitNode;
  node: GraphNode;
}

/** Selection state for compare mode */
export interface CompareSelection {
  baseSha: string;
  compareSha: string;
}
