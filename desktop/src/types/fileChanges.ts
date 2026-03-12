/**
 * Types for LLM file change tracking and rollback.
 */

export type FileChangeSourceMode = 'chat' | 'plan' | 'task' | 'debug';
export type FileChangeActorKind = 'root_agent' | 'sub_agent' | 'debug_patch' | 'system';

/** A single file modification record from the backend. */
export interface FileChange {
  id: string;
  session_id: string;
  turn_index: number;
  tool_call_id: string;
  tool_name: 'Write' | 'Edit' | 'Bash';
  file_path: string;
  before_hash: string | null;
  after_hash: string | null;
  timestamp: number;
  description: string;
  source_mode?: FileChangeSourceMode | null;
  actor_kind?: FileChangeActorKind | null;
  actor_id?: string | null;
  actor_label?: string | null;
  sub_agent_depth?: number | null;
  origin_session_id?: string | null;
}

/** Changes grouped by conversation turn. */
export interface TurnChanges {
  turn_index: number;
  changes: FileChange[];
  timestamp: number;
}

/** Result of restoring a single file. */
export interface RestoredFile {
  path: string;
  action: 'restored' | 'deleted';
}

/** Preview row for restore operation. */
export interface RestorePreviewItem {
  path: string;
  action: 'restore' | 'delete';
  source_turn: number;
}

/** Restore execution result with optional undo handle. */
export interface RestoreExecutionResult {
  operation_id: string | null;
  restored: RestoredFile[];
}
