/**
 * Shared types for Simple/Claude file references and explicit attachments.
 *
 * `WorkspaceFileReferenceData` models `@file` mentions.
 * `ExplicitAttachmentData` models user-attached files that may be resolved on demand.
 */

export type AttachmentKind = 'text' | 'image' | 'pdf' | 'unknown';

export interface WorkspaceFileReferenceData {
  id: string;
  name: string;
  relativePath: string;
  absolutePath: string;
  mentionText: string;
}

export interface ExplicitAttachmentData {
  id: string;
  name: string;
  path: string;
  size: number;
  type: AttachmentKind;
  mimeType?: string;
  isWorkspaceFile?: boolean;
  isAccessible?: boolean;
  // Runtime-only fallback payloads for cases where a native filesystem path is unavailable.
  inlineContent?: string;
  inlinePreview?: string;
}

// Backwards-compatible alias while the rest of the codebase is migrated.
export type FileAttachmentData = ExplicitAttachmentData;

export interface WorkspaceFile {
  name: string;
  path: string;
  size: number;
  is_dir: boolean;
}
