/**
 * Timeline Types
 *
 * TypeScript interfaces matching the Rust models for checkpoint and timeline management.
 */

/** File snapshot entry within a checkpoint */
export interface FileSnapshot {
  path: string;
  hash: string;
  size: number;
  is_binary: boolean;
}

/** A checkpoint representing a snapshot of session state */
export interface Checkpoint {
  id: string;
  session_id: string;
  timestamp: string;
  label: string;
  parent_id: string | null;
  branch_id: string | null;
  files_snapshot: FileSnapshot[];
  description: string | null;
}

/** A branch in the timeline */
export interface CheckpointBranch {
  id: string;
  name: string;
  parent_checkpoint_id: string;
  created_at: string;
  description: string | null;
  is_main: boolean;
}

/** Timeline metadata for a session */
export interface TimelineMetadata {
  session_id: string;
  checkpoints: Checkpoint[];
  branches: CheckpointBranch[];
  current_checkpoint_id: string | null;
  current_branch_id: string | null;
}

/** Change type for a file between checkpoints */
export type FileChangeType = 'added' | 'modified' | 'deleted';

/** Individual file diff between two checkpoints */
export interface FileDiff {
  path: string;
  change_type: FileChangeType;
  is_binary: boolean;
  diff_content: string | null;
  old_hash: string | null;
  new_hash: string | null;
  old_size: number | null;
  new_size: number | null;
  lines_added: number;
  lines_removed: number;
}

/** Summary statistics for a diff */
export interface DiffSummary {
  files_added: number;
  files_modified: number;
  files_deleted: number;
  lines_added: number;
  lines_removed: number;
}

/** Diff result between two checkpoints */
export interface CheckpointDiff {
  from_checkpoint_id: string;
  to_checkpoint_id: string;
  added_files: FileDiff[];
  modified_files: FileDiff[];
  deleted_files: FileDiff[];
  total_files_changed: number;
  summary: DiffSummary;
}

/** Result of restoring to a checkpoint */
export interface RestoreResult {
  success: boolean;
  restored_checkpoint_id: string;
  backup_checkpoint_id: string | null;
  restored_files: string[];
  removed_files: string[];
  error: string | null;
}
