/**
 * Types for LLM file change tracking and rollback.
 */

/** A single file modification record from the backend. */
export interface FileChange {
  id: string;
  session_id: string;
  turn_index: number;
  tool_call_id: string;
  tool_name: 'Write' | 'Edit';
  file_path: string;
  before_hash: string | null;
  after_hash: string;
  timestamp: number;
  description: string;
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
