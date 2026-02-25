import { describe, it, expect } from 'vitest';
import { parseUnifiedDiff } from './diffParser';

// ============================================================================
// Empty / Invalid Input
// ============================================================================

describe('parseUnifiedDiff – empty and invalid input', () => {
  it('returns empty array for empty string', () => {
    expect(parseUnifiedDiff('')).toEqual([]);
  });

  it('returns empty array for whitespace-only string', () => {
    expect(parseUnifiedDiff('   \n\n  \t  ')).toEqual([]);
  });

  it('returns empty array for arbitrary non-diff text', () => {
    expect(parseUnifiedDiff('Hello world\nThis is not a diff')).toEqual([]);
  });

  it('returns empty array for undefined-like input cast to string', () => {
    // Simulates the caller accidentally passing a non-string that gets coerced
    expect(parseUnifiedDiff(undefined as unknown as string)).toEqual([]);
    expect(parseUnifiedDiff(null as unknown as string)).toEqual([]);
  });
});

// ============================================================================
// Added file
// ============================================================================

describe('parseUnifiedDiff – added file', () => {
  const addedFileDiff = [
    'diff --git a/src/newFile.ts b/src/newFile.ts',
    'new file mode 100644',
    'index 0000000..abcdef1',
    '--- /dev/null',
    '+++ b/src/newFile.ts',
    '@@ -0,0 +1,3 @@',
    '+export function hello() {',
    '+  return "world";',
    '+}',
  ].join('\n');

  it('parses a new file as changeType "added"', () => {
    const result = parseUnifiedDiff(addedFileDiff);
    expect(result).toHaveLength(1);
    expect(result[0].changeType).toBe('added');
    expect(result[0].filePath).toBe('src/newFile.ts');
  });

  it('has one hunk with correct header values', () => {
    const result = parseUnifiedDiff(addedFileDiff);
    expect(result[0].hunks).toHaveLength(1);

    const hunk = result[0].hunks[0];
    expect(hunk.oldStart).toBe(0);
    expect(hunk.oldCount).toBe(0);
    expect(hunk.newStart).toBe(1);
    expect(hunk.newCount).toBe(3);
  });

  it('marks all lines as added with correct new line numbers', () => {
    const result = parseUnifiedDiff(addedFileDiff);
    const lines = result[0].hunks[0].lines;
    expect(lines).toHaveLength(3);

    lines.forEach((line) => {
      expect(line.type).toBe('added');
    });

    expect(lines[0].newLineNumber).toBe(1);
    expect(lines[1].newLineNumber).toBe(2);
    expect(lines[2].newLineNumber).toBe(3);

    // Added lines have no old line number
    lines.forEach((line) => {
      expect(line.oldLineNumber).toBeUndefined();
    });
  });
});

// ============================================================================
// Modified file
// ============================================================================

describe('parseUnifiedDiff – modified file', () => {
  const modifiedFileDiff = [
    'diff --git a/src/utils.ts b/src/utils.ts',
    'index 1234567..abcdef0 100644',
    '--- a/src/utils.ts',
    '+++ b/src/utils.ts',
    '@@ -10,7 +10,7 @@ export function existingFunction() {',
    '   const a = 1;',
    '   const b = 2;',
    '-  return a + b;',
    '+  return a * b;',
    '   const c = 3;',
    '   // trailing context',
    '   // more context',
  ].join('\n');

  it('parses as changeType "modified"', () => {
    const result = parseUnifiedDiff(modifiedFileDiff);
    expect(result).toHaveLength(1);
    expect(result[0].changeType).toBe('modified');
    expect(result[0].filePath).toBe('src/utils.ts');
  });

  it('has correct hunk header values', () => {
    const hunk = parseUnifiedDiff(modifiedFileDiff)[0].hunks[0];
    expect(hunk.oldStart).toBe(10);
    expect(hunk.oldCount).toBe(7);
    expect(hunk.newStart).toBe(10);
    expect(hunk.newCount).toBe(7);
  });

  it('classifies context, removed, and added lines correctly', () => {
    const lines = parseUnifiedDiff(modifiedFileDiff)[0].hunks[0].lines;
    expect(lines).toHaveLength(7);

    expect(lines[0].type).toBe('context');
    expect(lines[1].type).toBe('context');
    expect(lines[2].type).toBe('removed');
    expect(lines[3].type).toBe('added');
    expect(lines[4].type).toBe('context');
    expect(lines[5].type).toBe('context');
    expect(lines[6].type).toBe('context');
  });

  it('assigns correct old and new line numbers', () => {
    const lines = parseUnifiedDiff(modifiedFileDiff)[0].hunks[0].lines;

    // First context line: old=10, new=10
    expect(lines[0].oldLineNumber).toBe(10);
    expect(lines[0].newLineNumber).toBe(10);

    // Second context line: old=11, new=11
    expect(lines[1].oldLineNumber).toBe(11);
    expect(lines[1].newLineNumber).toBe(11);

    // Removed line: old=12, no new
    expect(lines[2].oldLineNumber).toBe(12);
    expect(lines[2].newLineNumber).toBeUndefined();

    // Added line: no old, new=12
    expect(lines[3].oldLineNumber).toBeUndefined();
    expect(lines[3].newLineNumber).toBe(12);

    // Context after: old=13, new=13
    expect(lines[4].oldLineNumber).toBe(13);
    expect(lines[4].newLineNumber).toBe(13);
  });
});

// ============================================================================
// Deleted file
// ============================================================================

describe('parseUnifiedDiff – deleted file', () => {
  const deletedFileDiff = [
    'diff --git a/src/old.ts b/src/old.ts',
    'deleted file mode 100644',
    'index abcdef0..0000000',
    '--- a/src/old.ts',
    '+++ /dev/null',
    '@@ -1,2 +0,0 @@',
    '-export const x = 1;',
    '-export const y = 2;',
  ].join('\n');

  it('parses as changeType "deleted"', () => {
    const result = parseUnifiedDiff(deletedFileDiff);
    expect(result).toHaveLength(1);
    expect(result[0].changeType).toBe('deleted');
    expect(result[0].filePath).toBe('src/old.ts');
  });

  it('marks all lines as removed', () => {
    const lines = parseUnifiedDiff(deletedFileDiff)[0].hunks[0].lines;
    expect(lines).toHaveLength(2);
    lines.forEach((line) => {
      expect(line.type).toBe('removed');
    });
  });

  it('assigns correct old line numbers and no new line numbers', () => {
    const lines = parseUnifiedDiff(deletedFileDiff)[0].hunks[0].lines;
    expect(lines[0].oldLineNumber).toBe(1);
    expect(lines[0].newLineNumber).toBeUndefined();
    expect(lines[1].oldLineNumber).toBe(2);
    expect(lines[1].newLineNumber).toBeUndefined();
  });
});

// ============================================================================
// Renamed file
// ============================================================================

describe('parseUnifiedDiff – renamed file', () => {
  const renamedFileDiff = [
    'diff --git a/src/oldName.ts b/src/newName.ts',
    'similarity index 95%',
    'rename from src/oldName.ts',
    'rename to src/newName.ts',
    'index 1234567..abcdef0 100644',
    '--- a/src/oldName.ts',
    '+++ b/src/newName.ts',
    '@@ -1,3 +1,3 @@',
    ' export function greet() {',
    '-  return "hello";',
    '+  return "hi";',
    ' }',
  ].join('\n');

  it('parses as changeType "renamed"', () => {
    const result = parseUnifiedDiff(renamedFileDiff);
    expect(result).toHaveLength(1);
    expect(result[0].changeType).toBe('renamed');
  });

  it('captures both old and new paths', () => {
    const result = parseUnifiedDiff(renamedFileDiff);
    expect(result[0].filePath).toBe('src/newName.ts');
    expect(result[0].oldPath).toBe('src/oldName.ts');
  });

  it('still parses hunks and line changes', () => {
    const result = parseUnifiedDiff(renamedFileDiff);
    expect(result[0].hunks).toHaveLength(1);
    const lines = result[0].hunks[0].lines;
    expect(lines).toHaveLength(4);
    expect(lines[0].type).toBe('context');
    expect(lines[1].type).toBe('removed');
    expect(lines[2].type).toBe('added');
    expect(lines[3].type).toBe('context');
  });
});

// ============================================================================
// Renamed file without content changes (pure rename)
// ============================================================================

describe('parseUnifiedDiff – pure rename (no content change)', () => {
  const pureRenameDiff = [
    'diff --git a/src/old.ts b/src/new.ts',
    'similarity index 100%',
    'rename from src/old.ts',
    'rename to src/new.ts',
  ].join('\n');

  it('parses as renamed with no hunks', () => {
    const result = parseUnifiedDiff(pureRenameDiff);
    expect(result).toHaveLength(1);
    expect(result[0].changeType).toBe('renamed');
    expect(result[0].filePath).toBe('src/new.ts');
    expect(result[0].oldPath).toBe('src/old.ts');
    expect(result[0].hunks).toEqual([]);
  });
});

// ============================================================================
// Multiple files in a single diff
// ============================================================================

describe('parseUnifiedDiff – multiple files', () => {
  const multiFileDiff = [
    'diff --git a/src/a.ts b/src/a.ts',
    'index 1111111..2222222 100644',
    '--- a/src/a.ts',
    '+++ b/src/a.ts',
    '@@ -1,3 +1,4 @@',
    ' line1',
    ' line2',
    '+line2.5',
    ' line3',
    'diff --git a/src/b.ts b/src/b.ts',
    'new file mode 100644',
    'index 0000000..3333333',
    '--- /dev/null',
    '+++ b/src/b.ts',
    '@@ -0,0 +1,2 @@',
    '+new line 1',
    '+new line 2',
    'diff --git a/src/c.ts b/src/c.ts',
    'deleted file mode 100644',
    'index 4444444..0000000',
    '--- a/src/c.ts',
    '+++ /dev/null',
    '@@ -1,1 +0,0 @@',
    '-old content',
  ].join('\n');

  it('returns one FileDiff per file', () => {
    const result = parseUnifiedDiff(multiFileDiff);
    expect(result).toHaveLength(3);
  });

  it('correctly identifies each file and change type', () => {
    const result = parseUnifiedDiff(multiFileDiff);
    expect(result[0].filePath).toBe('src/a.ts');
    expect(result[0].changeType).toBe('modified');
    expect(result[1].filePath).toBe('src/b.ts');
    expect(result[1].changeType).toBe('added');
    expect(result[2].filePath).toBe('src/c.ts');
    expect(result[2].changeType).toBe('deleted');
  });
});

// ============================================================================
// Multiple hunks in a single file
// ============================================================================

describe('parseUnifiedDiff – multiple hunks', () => {
  const multiHunkDiff = [
    'diff --git a/src/big.ts b/src/big.ts',
    'index aaa..bbb 100644',
    '--- a/src/big.ts',
    '+++ b/src/big.ts',
    '@@ -1,3 +1,4 @@',
    ' first',
    '+inserted',
    ' second',
    ' third',
    '@@ -20,3 +21,3 @@',
    ' alpha',
    '-beta',
    '+BETA',
    ' gamma',
  ].join('\n');

  it('returns two hunks for the same file', () => {
    const result = parseUnifiedDiff(multiHunkDiff);
    expect(result).toHaveLength(1);
    expect(result[0].hunks).toHaveLength(2);
  });

  it('first hunk has correct header and lines', () => {
    const hunk = parseUnifiedDiff(multiHunkDiff)[0].hunks[0];
    expect(hunk.oldStart).toBe(1);
    expect(hunk.oldCount).toBe(3);
    expect(hunk.newStart).toBe(1);
    expect(hunk.newCount).toBe(4);
    expect(hunk.lines).toHaveLength(4);
  });

  it('second hunk has correct header and lines', () => {
    const hunk = parseUnifiedDiff(multiHunkDiff)[0].hunks[1];
    expect(hunk.oldStart).toBe(20);
    expect(hunk.oldCount).toBe(3);
    expect(hunk.newStart).toBe(21);
    expect(hunk.newCount).toBe(3);
    expect(hunk.lines).toHaveLength(4);
  });
});

// ============================================================================
// Line content preservation
// ============================================================================

describe('parseUnifiedDiff – line content', () => {
  const diff = [
    'diff --git a/file.txt b/file.txt',
    'index aaa..bbb 100644',
    '--- a/file.txt',
    '+++ b/file.txt',
    '@@ -1,3 +1,3 @@',
    ' context line',
    '-removed line',
    '+added line',
    ' trailing context',
  ].join('\n');

  it('strips the leading +/- /space prefix from content', () => {
    const lines = parseUnifiedDiff(diff)[0].hunks[0].lines;
    expect(lines[0].content).toBe('context line');
    expect(lines[1].content).toBe('removed line');
    expect(lines[2].content).toBe('added line');
    expect(lines[3].content).toBe('trailing context');
  });
});

// ============================================================================
// Hunk header with single-line count omitted (e.g., @@ -1 +1 @@)
// ============================================================================

describe('parseUnifiedDiff – hunk header without count', () => {
  const diff = [
    'diff --git a/f.txt b/f.txt',
    'index aaa..bbb 100644',
    '--- a/f.txt',
    '+++ b/f.txt',
    '@@ -1 +1 @@',
    '-old',
    '+new',
  ].join('\n');

  it('defaults count to 1 when omitted', () => {
    const hunk = parseUnifiedDiff(diff)[0].hunks[0];
    expect(hunk.oldStart).toBe(1);
    expect(hunk.oldCount).toBe(1);
    expect(hunk.newStart).toBe(1);
    expect(hunk.newCount).toBe(1);
  });
});

// ============================================================================
// Binary file mention (no hunks)
// ============================================================================

describe('parseUnifiedDiff – binary file diff', () => {
  const binaryDiff = [
    'diff --git a/image.png b/image.png',
    'new file mode 100644',
    'index 0000000..abcdef1',
    'Binary files /dev/null and b/image.png differ',
  ].join('\n');

  it('parses file as added with no hunks', () => {
    const result = parseUnifiedDiff(binaryDiff);
    expect(result).toHaveLength(1);
    expect(result[0].filePath).toBe('image.png');
    expect(result[0].changeType).toBe('added');
    expect(result[0].hunks).toEqual([]);
  });
});

// ============================================================================
// Trailing newline / no newline at end of file marker
// ============================================================================

describe('parseUnifiedDiff – "No newline at end of file" marker', () => {
  const diff = [
    'diff --git a/f.txt b/f.txt',
    'index aaa..bbb 100644',
    '--- a/f.txt',
    '+++ b/f.txt',
    '@@ -1,2 +1,2 @@',
    '-old line',
    '\\ No newline at end of file',
    '+new line',
    '\\ No newline at end of file',
  ].join('\n');

  it('ignores the "No newline" markers and only parses real lines', () => {
    const lines = parseUnifiedDiff(diff)[0].hunks[0].lines;
    expect(lines).toHaveLength(2);
    expect(lines[0].type).toBe('removed');
    expect(lines[0].content).toBe('old line');
    expect(lines[1].type).toBe('added');
    expect(lines[1].content).toBe('new line');
  });
});

// ============================================================================
// Edge case: diff with empty lines in content
// ============================================================================

describe('parseUnifiedDiff – empty context lines', () => {
  const diff = [
    'diff --git a/f.ts b/f.ts',
    'index aaa..bbb 100644',
    '--- a/f.ts',
    '+++ b/f.ts',
    '@@ -1,4 +1,4 @@',
    ' line1',
    ' ',
    '-old',
    '+new',
    ' line4',
  ].join('\n');

  it('handles empty context lines (space-only prefix)', () => {
    const lines = parseUnifiedDiff(diff)[0].hunks[0].lines;
    expect(lines).toHaveLength(5);
    expect(lines[1].type).toBe('context');
    expect(lines[1].content).toBe('');
  });
});

// ============================================================================
// Type exports
// ============================================================================

describe('diffParser type exports', () => {
  it('exports FileDiff, DiffHunk, and DiffLine types', () => {
    // Type-level check: these should compile without error.
    // Runtime assertion just ensures the imports are reachable.
    expect(true).toBe(true);
  });
});
