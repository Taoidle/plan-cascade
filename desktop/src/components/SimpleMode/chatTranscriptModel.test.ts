import { describe, expect, it } from 'vitest';
import type { NonCardStreamLineType, StreamLine } from '../../store/execution';
import { buildTurnViewModels } from './chatTranscriptModel';

function line(partial: Partial<StreamLine> & { type?: NonCardStreamLineType }): StreamLine {
  return {
    id: partial.id ?? 1,
    content: partial.content ?? '',
    type: partial.type ?? 'text',
    timestamp: partial.timestamp ?? 1,
    subAgentId: partial.subAgentId,
    subAgentDepth: partial.subAgentDepth,
    turnId: partial.turnId,
    turnBoundary: partial.turnBoundary,
  };
}

describe('buildTurnViewModels', () => {
  it('returns empty list for empty lines', () => {
    expect(buildTurnViewModels([])).toEqual([]);
  });

  it('builds turn ranges with explicit user boundaries', () => {
    const lines: StreamLine[] = [
      line({ id: 1, type: 'info', content: 'u1', turnBoundary: 'user', turnId: 1 }),
      line({ id: 2, type: 'text', content: 'a1', turnId: 1 }),
      line({ id: 3, type: 'tool', content: 'tool', turnId: 1 }),
      line({ id: 4, type: 'info', content: 'u2', turnBoundary: 'user', turnId: 2 }),
      line({ id: 5, type: 'text', content: 'a2', turnId: 2 }),
    ];

    expect(buildTurnViewModels(lines)).toEqual([
      {
        turnIndex: 0,
        userLineIndex: 0,
        assistantStartIndex: 1,
        assistantEndIndex: 2,
        userLineId: 1,
      },
      {
        turnIndex: 1,
        userLineIndex: 3,
        assistantStartIndex: 4,
        assistantEndIndex: 4,
        userLineId: 4,
      },
    ]);
  });

  it('normalizes legacy info lines into user boundaries', () => {
    const lines: StreamLine[] = [
      line({ id: 10, type: 'info', content: 'legacy-user-1' }),
      line({ id: 11, type: 'text', content: 'assistant-1' }),
      line({ id: 12, type: 'info', content: 'legacy-user-2' }),
      line({ id: 13, type: 'text', content: 'assistant-2' }),
    ];

    expect(buildTurnViewModels(lines)).toEqual([
      {
        turnIndex: 0,
        userLineIndex: 0,
        assistantStartIndex: 1,
        assistantEndIndex: 1,
        userLineId: 10,
      },
      {
        turnIndex: 1,
        userLineIndex: 2,
        assistantStartIndex: 3,
        assistantEndIndex: 3,
        userLineId: 12,
      },
    ]);
  });

  it('creates synthetic user turn for assistant-only transcripts', () => {
    const lines: StreamLine[] = [
      line({ id: 20, type: 'text', content: 'assistant-1' }),
      line({ id: 21, type: 'tool', content: 'tool' }),
    ];

    expect(buildTurnViewModels(lines)).toEqual([
      {
        turnIndex: 0,
        userLineIndex: -1,
        assistantStartIndex: 0,
        assistantEndIndex: 1,
        userLineId: -1,
      },
    ]);
  });
});
