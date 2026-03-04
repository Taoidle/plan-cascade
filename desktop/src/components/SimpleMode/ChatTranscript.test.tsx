import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ChatTranscript } from './ChatTranscript';
import type { StreamLine } from '../../store/execution';

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? '',
  }),
}));

vi.mock('../../store/settings', () => ({
  useSettingsStore: (selector: (state: { backend: string; showReasoningOutput: boolean }) => unknown) =>
    selector({ backend: 'claude-code', showReasoningOutput: true }),
}));

vi.mock('../../store/execution', () => {
  const state = {
    editAndResend: vi.fn(),
    regenerateResponse: vi.fn(),
    rollbackToTurn: vi.fn(),
    forkSessionAtTurn: vi.fn(),
  };

  const useExecutionStore = ((selector?: (input: typeof state) => unknown) => (selector ? selector(state) : state)) as {
    (selector?: (input: typeof state) => unknown): unknown;
    getState: () => typeof state;
  };
  useExecutionStore.getState = () => state;

  return { useExecutionStore };
});

function buildLines(turnCount: number): StreamLine[] {
  const lines: StreamLine[] = [];
  for (let i = 0; i < turnCount; i++) {
    lines.push({
      id: i * 2 + 1,
      content: `user-${i}`,
      type: 'info',
      timestamp: i + 1,
      turnId: i + 1,
      turnBoundary: 'user',
    });
    lines.push({
      id: i * 2 + 2,
      content: `assistant-${i}`,
      type: 'text',
      timestamp: i + 1,
      turnId: i + 1,
    });
  }
  return lines;
}

describe('ChatTranscript render mode', () => {
  it('uses full render for short transcripts', () => {
    render(<ChatTranscript lines={buildLines(10)} status="idle" />);

    expect(screen.getByTestId('chat-transcript-scroll')).toHaveAttribute('data-render-mode', 'full');
  });

  it('uses virtual render for long transcripts by default', () => {
    render(<ChatTranscript lines={buildLines(60)} status="idle" />);

    expect(screen.getByTestId('chat-transcript-scroll')).toHaveAttribute('data-render-mode', 'virtual');
  });

  it('forces full render during capture mode', () => {
    render(<ChatTranscript lines={buildLines(60)} status="idle" forceFullRender />);

    expect(screen.getByTestId('chat-transcript-scroll')).toHaveAttribute('data-render-mode', 'full');
  });
});
