import { describe, it, expect, beforeEach } from 'vitest';
import { deriveConversationTurns, rebuildStandaloneTurns, buildPromptWithAttachments } from './conversationUtils';
import type { StreamLine } from '../store/execution';
import type { FileAttachmentData } from '../types/attachment';

// ============================================================================
// Helper to create StreamLine objects
// ============================================================================

let lineIdCounter = 0;
function makeLine(type: StreamLine['type'], content: string, timestamp?: number): StreamLine {
  lineIdCounter += 1;
  return {
    id: lineIdCounter,
    type,
    content,
    timestamp: timestamp ?? Date.now(),
  };
}

function resetLineCounter() {
  lineIdCounter = 0;
}

// ============================================================================
// deriveConversationTurns
// ============================================================================

describe('deriveConversationTurns', () => {
  beforeEach(resetLineCounter);

  it('returns empty array for empty input', () => {
    expect(deriveConversationTurns([])).toEqual([]);
  });

  it('returns empty array when no info lines exist', () => {
    const lines: StreamLine[] = [makeLine('text', 'some assistant text'), makeLine('text', 'more text')];
    expect(deriveConversationTurns(lines)).toEqual([]);
  });

  it('derives a single turn correctly', () => {
    const lines: StreamLine[] = [
      makeLine('info', 'User prompt'),
      makeLine('text', 'Hello '),
      makeLine('text', 'world'),
    ];
    const turns = deriveConversationTurns(lines);
    expect(turns).toHaveLength(1);
    expect(turns[0].turnIndex).toBe(0);
    expect(turns[0].userContent).toBe('User prompt');
    expect(turns[0].assistantText).toBe('Hello world');
    expect(turns[0].userLineId).toBe(1);
    expect(turns[0].assistantStartIndex).toBe(1);
    expect(turns[0].assistantEndIndex).toBe(2);
  });

  it('derives multiple turns correctly', () => {
    const lines: StreamLine[] = [
      makeLine('info', 'First question'),
      makeLine('text', 'First answer part 1'),
      makeLine('text', 'First answer part 2'),
      makeLine('info', 'Second question'),
      makeLine('text', 'Second answer'),
    ];
    const turns = deriveConversationTurns(lines);
    expect(turns).toHaveLength(2);
    expect(turns[0].turnIndex).toBe(0);
    expect(turns[0].userContent).toBe('First question');
    expect(turns[0].assistantText).toBe('First answer part 1First answer part 2');
    expect(turns[1].turnIndex).toBe(1);
    expect(turns[1].userContent).toBe('Second question');
    expect(turns[1].assistantText).toBe('Second answer');
  });

  it('handles info line at the end with no assistant response', () => {
    const lines: StreamLine[] = [
      makeLine('info', 'First question'),
      makeLine('text', 'Answer'),
      makeLine('info', 'Second question, no response yet'),
    ];
    const turns = deriveConversationTurns(lines);
    // The second turn has no assistant text, so it should still be included
    // but with empty assistant text
    expect(turns).toHaveLength(2);
    expect(turns[1].assistantText).toBe('');
  });

  it('ignores non-text lines in assistant text', () => {
    const lines: StreamLine[] = [
      makeLine('info', 'User prompt'),
      makeLine('text', 'Part 1'),
      makeLine('tool', 'tool call data'),
      makeLine('tool_result', 'tool result data'),
      makeLine('text', 'Part 2'),
    ];
    const turns = deriveConversationTurns(lines);
    expect(turns).toHaveLength(1);
    expect(turns[0].assistantText).toBe('Part 1Part 2');
  });

  it('sets correct assistantStartIndex and assistantEndIndex', () => {
    const lines: StreamLine[] = [
      makeLine('info', 'User prompt'), // index 0
      makeLine('text', 'A'), // index 1
      makeLine('tool', 'T'), // index 2
      makeLine('text', 'B'), // index 3
    ];
    const turns = deriveConversationTurns(lines);
    expect(turns[0].assistantStartIndex).toBe(1);
    expect(turns[0].assistantEndIndex).toBe(3);
  });
});

// ============================================================================
// rebuildStandaloneTurns
// ============================================================================

describe('rebuildStandaloneTurns', () => {
  beforeEach(resetLineCounter);

  it('returns empty array for empty input', () => {
    expect(rebuildStandaloneTurns([])).toEqual([]);
  });

  it('returns empty array when no info lines exist', () => {
    const lines: StreamLine[] = [makeLine('text', 'orphan text')];
    expect(rebuildStandaloneTurns(lines)).toEqual([]);
  });

  it('rebuilds a single turn', () => {
    const ts = 1700000000000;
    const lines: StreamLine[] = [
      makeLine('info', 'User message', ts),
      makeLine('text', 'Hello '),
      makeLine('text', 'world'),
    ];
    const turns = rebuildStandaloneTurns(lines);
    expect(turns).toHaveLength(1);
    expect(turns[0].user).toBe('User message');
    expect(turns[0].assistant).toBe('Hello world');
    expect(turns[0].createdAt).toBeGreaterThan(0);
  });

  it('rebuilds multiple turns', () => {
    const lines: StreamLine[] = [
      makeLine('info', 'Q1'),
      makeLine('text', 'A1'),
      makeLine('info', 'Q2'),
      makeLine('text', 'A2-part1'),
      makeLine('text', 'A2-part2'),
    ];
    const turns = rebuildStandaloneTurns(lines);
    expect(turns).toHaveLength(2);
    expect(turns[0].user).toBe('Q1');
    expect(turns[0].assistant).toBe('A1');
    expect(turns[1].user).toBe('Q2');
    expect(turns[1].assistant).toBe('A2-part1A2-part2');
  });

  it('skips turns with empty assistant text', () => {
    const lines: StreamLine[] = [
      makeLine('info', 'Question with no answer'),
      makeLine('info', 'Second question'),
      makeLine('text', 'Answer to second'),
    ];
    const turns = rebuildStandaloneTurns(lines);
    expect(turns).toHaveLength(1);
    expect(turns[0].user).toBe('Second question');
    expect(turns[0].assistant).toBe('Answer to second');
  });

  it('only concatenates text-type lines', () => {
    const lines: StreamLine[] = [
      makeLine('info', 'User prompt'),
      makeLine('text', 'A'),
      makeLine('tool', 'tool data'),
      makeLine('error', 'error data'),
      makeLine('text', 'B'),
    ];
    const turns = rebuildStandaloneTurns(lines);
    expect(turns).toHaveLength(1);
    expect(turns[0].assistant).toBe('AB');
  });
});

// ============================================================================
// buildPromptWithAttachments
// ============================================================================

describe('buildPromptWithAttachments', () => {
  it('returns prompt as-is when no attachments', () => {
    expect(buildPromptWithAttachments('Hello', [])).toBe('Hello');
  });

  it('prepends text file content', () => {
    const attachments: FileAttachmentData[] = [
      {
        id: '1',
        name: 'readme.md',
        path: '/tmp/readme.md',
        size: 100,
        type: 'text',
        content: '# Title\nSome content',
      },
    ];
    const result = buildPromptWithAttachments('Summarize this file', attachments);
    expect(result).toContain('readme.md');
    expect(result).toContain('# Title\nSome content');
    expect(result).toContain('Summarize this file');
  });

  it('includes multiple text attachments', () => {
    const attachments: FileAttachmentData[] = [
      {
        id: '1',
        name: 'a.txt',
        path: '/tmp/a.txt',
        size: 10,
        type: 'text',
        content: 'File A',
      },
      {
        id: '2',
        name: 'b.txt',
        path: '/tmp/b.txt',
        size: 10,
        type: 'text',
        content: 'File B',
      },
    ];
    const result = buildPromptWithAttachments('Process files', attachments);
    expect(result).toContain('a.txt');
    expect(result).toContain('File A');
    expect(result).toContain('b.txt');
    expect(result).toContain('File B');
    expect(result).toContain('Process files');
  });

  it('handles image attachments as reference only', () => {
    const attachments: FileAttachmentData[] = [
      {
        id: '1',
        name: 'photo.png',
        path: '/tmp/photo.png',
        size: 50000,
        type: 'image',
        preview: 'data:image/png;base64,abc123',
      },
    ];
    const result = buildPromptWithAttachments('Describe this image', attachments);
    expect(result).toContain('photo.png');
    expect(result).toContain('image');
    expect(result).toContain('Describe this image');
    // Should NOT include the base64 data in the text prompt
    expect(result).not.toContain('abc123');
  });

  it('handles attachments without content gracefully', () => {
    const attachments: FileAttachmentData[] = [
      {
        id: '1',
        name: 'binary.dat',
        path: '/tmp/binary.dat',
        size: 1000,
        type: 'unknown',
      },
    ];
    const result = buildPromptWithAttachments('Check this', attachments);
    expect(result).toContain('binary.dat');
    expect(result).toContain('Check this');
  });

  it('puts prompt after attachments', () => {
    const attachments: FileAttachmentData[] = [
      {
        id: '1',
        name: 'file.txt',
        path: '/tmp/file.txt',
        size: 10,
        type: 'text',
        content: 'File content',
      },
    ];
    const result = buildPromptWithAttachments('My prompt', attachments);
    const attachmentPos = result.indexOf('File content');
    const promptPos = result.indexOf('My prompt');
    expect(attachmentPos).toBeLessThan(promptPos);
  });
});
