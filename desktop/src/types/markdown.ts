/**
 * Markdown Types
 *
 * TypeScript interfaces matching the Rust models in desktop/src-tauri/src/models/markdown.rs
 */

/** A discovered CLAUDE.md file */
export interface ClaudeMdFile {
  /** Full absolute path to the file */
  path: string;
  /** Display name (parent directory name or project name) */
  name: string;
  /** Relative path from the scanned root */
  relative_path: string;
  /** Last modification timestamp (ISO 8601) */
  modified_at: string;
  /** File size in bytes */
  size: number;
}

/** Content of a CLAUDE.md file */
export interface ClaudeMdContent {
  /** Full path to the file */
  path: string;
  /** File content as a string */
  content: string;
  /** Last modification timestamp (ISO 8601) */
  modified_at: string | null;
}

/** Metadata for a file */
export interface FileMetadata {
  /** Full path to the file */
  path: string;
  /** File size in bytes */
  size: number;
  /** Last modification timestamp (ISO 8601) */
  modified_at: string | null;
  /** Creation timestamp (ISO 8601) */
  created_at: string | null;
}

/** Result of a save operation */
export interface SaveResult {
  /** Whether the save was successful */
  success: boolean;
  /** Path that was saved */
  path: string;
  /** Error message if save failed */
  error: string | null;
}

/** View mode for the markdown editor */
export type ViewMode = 'split' | 'edit' | 'preview';

/** Save status for auto-save indicator */
export type SaveStatus = 'saved' | 'saving' | 'unsaved' | 'error';

/** Generic command response from Tauri */
export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}
