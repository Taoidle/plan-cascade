/**
 * Unified Diff Parser
 *
 * Parses unified diff text (as produced by `git diff`) into structured
 * per-file diff data suitable for rendering by UI components such as
 * the SimpleMode diff viewer panels.
 */

// ============================================================================
// Types
// ============================================================================

/** A single line within a diff hunk. */
export interface DiffLine {
  /** Whether the line was added, removed, or is unchanged context. */
  type: 'added' | 'removed' | 'context';
  /** The line content with the leading +/-/space prefix stripped. */
  content: string;
  /** Line number in the old file. Undefined for added lines. */
  oldLineNumber?: number;
  /** Line number in the new file. Undefined for removed lines. */
  newLineNumber?: number;
}

/** A contiguous hunk of changes within a file diff. */
export interface DiffHunk {
  /** Starting line number in the old file. */
  oldStart: number;
  /** Number of lines from the old file in this hunk. */
  oldCount: number;
  /** Starting line number in the new file. */
  newStart: number;
  /** Number of lines from the new file in this hunk. */
  newCount: number;
  /** Ordered lines in this hunk (context, added, removed). */
  lines: DiffLine[];
}

/** Structured diff data for a single file. */
export interface FileDiff {
  /** Path to the file (new path for renames, otherwise the file path). */
  filePath: string;
  /** Original path before rename. Only set when changeType is "renamed". */
  oldPath?: string;
  /** The kind of change: added, modified, deleted, or renamed. */
  changeType: 'added' | 'modified' | 'deleted' | 'renamed';
  /** Hunks of changes within the file. Empty for binary or pure-rename diffs. */
  hunks: DiffHunk[];
}

// ============================================================================
// Regex patterns
// ============================================================================

/** Matches the `diff --git a/... b/...` header line. */
const DIFF_HEADER_RE = /^diff --git a\/(.+?) b\/(.+?)$/;

/** Matches a hunk header: `@@ -old,count +new,count @@` (count is optional). */
const HUNK_HEADER_RE = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;

// ============================================================================
// Parser
// ============================================================================

/**
 * Parse a unified diff string into structured per-file diff data.
 *
 * Handles added, modified, deleted, and renamed files. Returns an empty
 * array for empty, null, undefined, or non-diff input rather than throwing.
 *
 * @param rawDiff - The raw unified diff text (e.g., output of `git diff`).
 * @returns An array of `FileDiff` objects, one per file in the diff.
 */
export function parseUnifiedDiff(rawDiff: string): FileDiff[] {
  // Guard against null/undefined/non-string input
  if (!rawDiff || typeof rawDiff !== 'string') {
    return [];
  }

  const trimmed = rawDiff.trim();
  if (trimmed.length === 0) {
    return [];
  }

  const lines = rawDiff.split('\n');
  const fileDiffs: FileDiff[] = [];

  // Identify start indices of each file diff (each `diff --git` line)
  const fileStartIndices: number[] = [];
  for (let i = 0; i < lines.length; i++) {
    if (DIFF_HEADER_RE.test(lines[i])) {
      fileStartIndices.push(i);
    }
  }

  // If no diff headers found, return empty
  if (fileStartIndices.length === 0) {
    return [];
  }

  // Process each file section
  for (let idx = 0; idx < fileStartIndices.length; idx++) {
    const start = fileStartIndices[idx];
    const end = idx + 1 < fileStartIndices.length
      ? fileStartIndices[idx + 1]
      : lines.length;

    const fileSection = lines.slice(start, end);
    const fileDiff = parseFileSection(fileSection);
    if (fileDiff) {
      fileDiffs.push(fileDiff);
    }
  }

  return fileDiffs;
}

// ============================================================================
// Internal helpers
// ============================================================================

/**
 * Parse a single file section (from `diff --git` to the next or end).
 */
function parseFileSection(sectionLines: string[]): FileDiff | null {
  if (sectionLines.length === 0) {
    return null;
  }

  const headerMatch = DIFF_HEADER_RE.exec(sectionLines[0]);
  if (!headerMatch) {
    return null;
  }

  const aPath = headerMatch[1];
  const bPath = headerMatch[2];

  // Scan metadata lines (between diff header and first hunk / end)
  let isNewFile = false;
  let isDeletedFile = false;
  let isRenamed = false;
  let renameFrom: string | undefined;
  let renameTo: string | undefined;

  // Find first hunk header or end of section
  let metaEnd = sectionLines.length;
  for (let i = 1; i < sectionLines.length; i++) {
    if (HUNK_HEADER_RE.test(sectionLines[i])) {
      metaEnd = i;
      break;
    }
  }

  // Parse metadata lines
  for (let i = 1; i < metaEnd; i++) {
    const line = sectionLines[i];
    if (line.startsWith('new file mode')) {
      isNewFile = true;
    } else if (line.startsWith('deleted file mode')) {
      isDeletedFile = true;
    } else if (line.startsWith('rename from ')) {
      isRenamed = true;
      renameFrom = line.slice('rename from '.length);
    } else if (line.startsWith('rename to ')) {
      isRenamed = true;
      renameTo = line.slice('rename to '.length);
    }
  }

  // Determine change type
  let changeType: FileDiff['changeType'];
  if (isRenamed) {
    changeType = 'renamed';
  } else if (isNewFile) {
    changeType = 'added';
  } else if (isDeletedFile) {
    changeType = 'deleted';
  } else {
    changeType = 'modified';
  }

  // Determine file paths
  let filePath: string;
  let oldPath: string | undefined;

  if (isRenamed) {
    filePath = renameTo ?? bPath;
    oldPath = renameFrom ?? aPath;
  } else {
    filePath = bPath;
  }

  // Parse hunks
  const hunks = parseHunks(sectionLines, metaEnd);

  const result: FileDiff = {
    filePath,
    changeType,
    hunks,
  };

  if (oldPath !== undefined) {
    result.oldPath = oldPath;
  }

  return result;
}

/**
 * Parse all hunks from the section lines starting at the given offset.
 */
function parseHunks(sectionLines: string[], startIndex: number): DiffHunk[] {
  const hunks: DiffHunk[] = [];
  let i = startIndex;

  while (i < sectionLines.length) {
    const hunkMatch = HUNK_HEADER_RE.exec(sectionLines[i]);
    if (!hunkMatch) {
      i++;
      continue;
    }

    const oldStart = parseInt(hunkMatch[1], 10);
    const oldCount = hunkMatch[2] !== undefined ? parseInt(hunkMatch[2], 10) : 1;
    const newStart = parseInt(hunkMatch[3], 10);
    const newCount = hunkMatch[4] !== undefined ? parseInt(hunkMatch[4], 10) : 1;

    const diffLines: DiffLine[] = [];
    let oldLine = oldStart;
    let newLine = newStart;

    i++; // Move past the hunk header

    while (i < sectionLines.length) {
      const line = sectionLines[i];

      // Stop if we hit another hunk header or diff header
      if (HUNK_HEADER_RE.test(line) || DIFF_HEADER_RE.test(line)) {
        break;
      }

      // Skip "No newline at end of file" markers
      if (line.startsWith('\\ ')) {
        i++;
        continue;
      }

      if (line.startsWith('+')) {
        diffLines.push({
          type: 'added',
          content: line.slice(1),
          newLineNumber: newLine,
        });
        newLine++;
      } else if (line.startsWith('-')) {
        diffLines.push({
          type: 'removed',
          content: line.slice(1),
          oldLineNumber: oldLine,
        });
        oldLine++;
      } else if (line.startsWith(' ') || line === '') {
        // Context line: starts with a space, or could be an empty line
        // (git sometimes outputs truly empty lines for empty context)
        diffLines.push({
          type: 'context',
          content: line.length > 0 ? line.slice(1) : '',
          oldLineNumber: oldLine,
          newLineNumber: newLine,
        });
        oldLine++;
        newLine++;
      } else {
        // Non-diff content line (e.g., "Binary files ... differ"); skip it.
        i++;
        continue;
      }

      i++;
    }

    hunks.push({
      oldStart,
      oldCount,
      newStart,
      newCount,
      lines: diffLines,
    });
  }

  return hunks;
}
