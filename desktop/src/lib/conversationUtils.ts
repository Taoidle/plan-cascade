/**
 * Conversation Utilities
 *
 * Shared utilities for deriving conversation turns from flat StreamLine arrays,
 * rebuilding StandaloneTurn history, and building prompts with file attachments.
 */

import type { StreamLine, StreamLineType } from '../store/execution';
import type { FileAttachmentData } from '../types/attachment';

// ============================================================================
// Types
// ============================================================================

/**
 * A derived conversation turn from the flat StreamLine array.
 * Groups lines by 'info' (user message) boundaries.
 */
export interface ConversationTurn {
  turnIndex: number;
  /** StreamLine.id of the 'info' type line representing the user message. */
  userLineId: number;
  userContent: string;
  /** Index in the lines array where assistant content starts (first line after info). */
  assistantStartIndex: number;
  /** Index in the lines array where assistant content ends (last line before next info or end). */
  assistantEndIndex: number;
  /** Concatenated 'text' type lines forming the assistant response. */
  assistantText: string;
}

/**
 * Standalone conversation turn for multi-turn context.
 * Matches the StandaloneTurn interface in execution.ts.
 */
export interface StandaloneTurn {
  user: string;
  assistant: string;
  createdAt: number;
}

// ============================================================================
// Functions
// ============================================================================

/**
 * Derive conversation turns from a flat StreamLine array.
 *
 * Groups lines by 'info' (user message) boundaries. Each 'info' line starts
 * a new turn, and all subsequent 'text' lines until the next 'info' line
 * form the assistant response.
 */
export function deriveConversationTurns(lines: StreamLine[]): ConversationTurn[] {
  const turns: ConversationTurn[] = [];
  let turnIndex = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.type !== 'info') continue;

    // Find the end of this turn (next info line or end of array)
    let endIndex = lines.length - 1;
    for (let j = i + 1; j < lines.length; j++) {
      if (lines[j].type === 'info') {
        endIndex = j - 1;
        break;
      }
    }

    // Concatenate assistant text from 'text' type lines
    const assistantSegments: string[] = [];
    const assistantStartIndex = i + 1;
    const assistantEndIndex = endIndex >= assistantStartIndex ? endIndex : assistantStartIndex;

    for (let j = assistantStartIndex; j <= endIndex; j++) {
      if (lines[j].type === 'text') {
        assistantSegments.push(lines[j].content);
      }
    }

    turns.push({
      turnIndex,
      userLineId: line.id,
      userContent: line.content,
      assistantStartIndex: assistantStartIndex < lines.length ? assistantStartIndex : i,
      assistantEndIndex: endIndex >= assistantStartIndex ? endIndex : i,
      assistantText: assistantSegments.join(''),
    });

    turnIndex++;
  }

  return turns;
}

/**
 * Rebuild StandaloneTurn[] from a flat StreamLine array.
 *
 * Reuses the pattern from execution.ts restoreFromHistory (lines 1411-1437).
 * Turns with empty assistant text are skipped.
 */
export function rebuildStandaloneTurns(lines: StreamLine[]): StandaloneTurn[] {
  const restoredTurns: StandaloneTurn[] = [];
  let pendingUser: string | null = null;
  let assistantSegments: string[] = [];

  for (const line of lines) {
    if (line.type === 'info') {
      if (pendingUser && assistantSegments.join('').trim().length > 0) {
        restoredTurns.push({
          user: pendingUser,
          assistant: assistantSegments.join(''),
          createdAt: line.timestamp,
        });
      }
      pendingUser = line.content;
      assistantSegments = [];
    } else if (line.type === 'text' && pendingUser) {
      assistantSegments.push(line.content);
    }
  }

  // Flush the final turn
  if (pendingUser && assistantSegments.join('').trim().length > 0) {
    restoredTurns.push({
      user: pendingUser,
      assistant: assistantSegments.join(''),
      createdAt: Date.now(),
    });
  }

  return restoredTurns;
}

/**
 * Build a prompt string with file attachment contents prepended.
 *
 * For text files, the full content is included. For images and other types,
 * only a reference with the file name and type is included (binary content
 * is not embedded in the text prompt).
 */
export function buildPromptWithAttachments(
  prompt: string,
  attachments: FileAttachmentData[]
): string {
  if (attachments.length === 0) return prompt;

  const sections: string[] = [];

  for (const attachment of attachments) {
    if (attachment.type === 'text' && attachment.content) {
      sections.push(
        `--- File: ${attachment.name} ---\n${attachment.content}\n--- End of ${attachment.name} ---`
      );
    } else if (attachment.type === 'image') {
      sections.push(
        `--- Attached image: ${attachment.name} (${formatFileSize(attachment.size)}) ---`
      );
    } else if (attachment.type === 'pdf') {
      sections.push(
        `--- Attached PDF: ${attachment.name} (${formatFileSize(attachment.size)}) ---`
      );
    } else {
      sections.push(
        `--- Attached file: ${attachment.name} (${attachment.type}, ${formatFileSize(attachment.size)}) ---`
      );
    }
  }

  sections.push(prompt);
  return sections.join('\n\n');
}

// ============================================================================
// Internal helpers
// ============================================================================

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// Re-export types for convenience
export type { StreamLine, StreamLineType, FileAttachmentData };
