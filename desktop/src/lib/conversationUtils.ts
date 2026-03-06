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
 * Groups lines by explicit user turn boundaries.
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
 * Check whether a line marks a user turn boundary.
 *
 * New protocol relies on explicit turn metadata instead of inferring from
 * `type: "info"` content semantics.
 */
export function isUserTurnBoundary(line: Pick<StreamLine, 'turnBoundary'>): boolean {
  return line.turnBoundary === 'user';
}

/**
 * Normalize legacy lines to explicit turn metadata.
 *
 * For modern lines this is a no-op. For old persisted history where user
 * messages were encoded only as `type: "info"`, this promotes those lines to
 * `turnBoundary: "user"` once so downstream logic can stay boundary-based.
 */
export function normalizeTurnBoundaries(lines: StreamLine[]): StreamLine[] {
  if (lines.length === 0) return lines;

  const hasExplicitBoundary = lines.some((line) => line.turnBoundary === 'user');
  const hasLegacyInfo = lines.some((line) => line.type === 'info');

  if (!hasExplicitBoundary && !hasLegacyInfo) return lines;

  let changed = false;
  const normalized = lines.map((line) => ({ ...line }));

  if (!hasExplicitBoundary) {
    let turnId = 0;
    for (const line of normalized) {
      if (line.type === 'info') {
        turnId += 1;
        if (line.turnBoundary !== 'user') {
          line.turnBoundary = 'user';
          changed = true;
        }
        if (line.turnId !== turnId) {
          line.turnId = turnId;
          changed = true;
        }
      } else if (turnId > 0 && line.turnId == null) {
        line.turnId = turnId;
        changed = true;
      }
    }
    return changed ? normalized : lines;
  }

  let fallbackTurnId = 0;
  for (const line of normalized) {
    if (line.turnBoundary === 'user') {
      const nextTurnId = line.turnId ?? Math.max(fallbackTurnId + 1, 1);
      if (line.turnId !== nextTurnId) {
        line.turnId = nextTurnId;
        changed = true;
      }
      fallbackTurnId = nextTurnId;
    } else if (fallbackTurnId > 0 && line.turnId == null) {
      line.turnId = fallbackTurnId;
      changed = true;
    }
  }

  return changed ? normalized : lines;
}

/**
 * Compute the next user turn id for a newly appended user message.
 */
export function getNextTurnId(lines: StreamLine[]): number {
  const normalized = normalizeTurnBoundaries(lines);
  let maxTurnId = 0;
  for (const line of normalized) {
    if (!isUserTurnBoundary(line)) continue;
    if (typeof line.turnId === 'number' && Number.isFinite(line.turnId)) {
      maxTurnId = Math.max(maxTurnId, line.turnId);
    }
  }
  return maxTurnId + 1;
}

export function countUserTurnBoundaries(lines: StreamLine[]): number {
  const normalized = normalizeTurnBoundaries(lines);
  let count = 0;
  for (const line of normalized) {
    if (isUserTurnBoundary(line)) count += 1;
  }
  return count;
}

/**
 * Prevent chat transcript regression when a transient runtime snapshot contains
 * only assistant-side lines while the cached transcript still has real user
 * turn boundaries.
 */
export function selectStableConversationLines(primary: StreamLine[], fallback: StreamLine[]): StreamLine[] {
  const normalizedPrimary = normalizeTurnBoundaries(primary);
  const normalizedFallback = normalizeTurnBoundaries(fallback);
  const primaryTurns = countUserTurnBoundaries(normalizedPrimary);
  const fallbackTurns = countUserTurnBoundaries(normalizedFallback);

  if (primaryTurns === 0 && fallbackTurns > 0) {
    return normalizedFallback;
  }

  return normalizedPrimary;
}

/**
 * Derive conversation turns from a flat StreamLine array.
 *
 * Groups lines by explicit user boundaries. Legacy history is normalized first
 * so callers do not need to infer turns from line type.
 */
export function deriveConversationTurns(lines: StreamLine[]): ConversationTurn[] {
  const normalizedLines = normalizeTurnBoundaries(lines);
  const turns: ConversationTurn[] = [];
  let turnIndex = 0;

  for (let i = 0; i < normalizedLines.length; i++) {
    const line = normalizedLines[i];
    if (!isUserTurnBoundary(line)) continue;

    // Find the end of this turn (next user boundary line or end of array)
    let endIndex = normalizedLines.length - 1;
    for (let j = i + 1; j < normalizedLines.length; j++) {
      if (isUserTurnBoundary(normalizedLines[j])) {
        endIndex = j - 1;
        break;
      }
    }

    // Concatenate assistant text from text stream lines only.
    const assistantSegments: string[] = [];
    const assistantStartIndex = i + 1;

    for (let j = assistantStartIndex; j <= endIndex; j++) {
      const current = normalizedLines[j];
      if (current.type === 'text') assistantSegments.push(current.content);
    }

    turns.push({
      turnIndex,
      userLineId: line.id,
      userContent: line.content,
      assistantStartIndex: assistantStartIndex < normalizedLines.length ? assistantStartIndex : i,
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
  const normalizedLines = normalizeTurnBoundaries(lines);
  const restoredTurns: StandaloneTurn[] = [];
  let pendingUser: string | null = null;
  let assistantSegments: string[] = [];

  for (const line of normalizedLines) {
    if (isUserTurnBoundary(line)) {
      if (pendingUser && assistantSegments.join('').trim().length > 0) {
        restoredTurns.push({
          user: pendingUser,
          assistant: assistantSegments.join(''),
          createdAt: line.timestamp,
        });
      }
      pendingUser = line.content;
      assistantSegments = [];
    } else if (pendingUser && line.type === 'text') {
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
export function buildPromptWithAttachments(prompt: string, attachments: FileAttachmentData[]): string {
  if (attachments.length === 0) return prompt;

  const sections: string[] = [];

  for (const attachment of attachments) {
    if (attachment.type === 'text' && attachment.content) {
      sections.push(`--- File: ${attachment.name} ---\n${attachment.content}\n--- End of ${attachment.name} ---`);
    } else if (attachment.type === 'image') {
      sections.push(`--- Attached image: ${attachment.name} (${formatFileSize(attachment.size)}) ---`);
    } else if (attachment.type === 'pdf') {
      sections.push(`--- Attached PDF: ${attachment.name} (${formatFileSize(attachment.size)}) ---`);
    } else {
      sections.push(
        `--- Attached file: ${attachment.name} (${attachment.type}, ${formatFileSize(attachment.size)}) ---`,
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
