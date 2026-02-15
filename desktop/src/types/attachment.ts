/**
 * Shared types for the file attachment system.
 *
 * Used by file attachment UI, conversation utilities, and prompt builders.
 */

export interface FileAttachmentData {
  id: string;
  name: string;
  path: string;
  size: number;
  type: 'text' | 'image' | 'pdf' | 'unknown';
  content?: string;       // Text content for text files
  preview?: string;       // Base64 data URL for images
}

export interface WorkspaceFile {
  name: string;
  path: string;
  size: number;
  is_dir: boolean;
}
