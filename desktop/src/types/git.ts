/**
 * Git TypeScript Types
 *
 * TypeScript equivalents of the Rust types defined in
 * desktop/src-tauri/src/services/git/types.rs
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

// ---------------------------------------------------------------------------
// Generic Command Response (mirrors models/response.rs)
// ---------------------------------------------------------------------------

export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

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

export type MergeStateKind =
  | 'none'
  | 'merging'
  | 'rebasing'
  | 'cherry_picking'
  | 'reverting';

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
// Worktree (mirrors models/worktree.rs)
// ---------------------------------------------------------------------------

export type WorktreeStatus =
  | 'creating'
  | 'active'
  | 'in_progress'
  | 'ready'
  | 'merging'
  | 'completed'
  | 'error';

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
// Diff (for reference)
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
// Commit / Log
// ---------------------------------------------------------------------------

export interface CommitNode {
  sha: string;
  short_sha: string;
  parents: string[];
  author_name: string;
  author_email: string;
  date: string;
  message: string;
  refs: string[];
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
