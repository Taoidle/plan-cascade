import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { ChatTranscript } from './ChatTranscript';
import type { StreamLine } from '../../store/execution';

const mockForkSessionAtTurn = vi.hoisted(() => vi.fn());

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) =>
      (
        ({
          'messageActions.fork': 'Fork',
        }) as Record<string, string>
      )[key] ??
      options?.defaultValue ??
      key,
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
    forkSessionAtTurn: mockForkSessionAtTurn,
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

  it('does not use JSON fallback for legacy card lines without cardPayload', () => {
    const lines: StreamLine[] = [
      {
        id: 1,
        content: 'user message',
        type: 'info',
        timestamp: 1,
        turnId: 1,
        turnBoundary: 'user',
      },
      {
        id: 2,
        content: JSON.stringify({
          cardType: 'workflow_info',
          cardId: 'legacy-card-1',
          interactive: false,
          data: { message: 'legacy card' },
        }),
        type: 'card',
        timestamp: 2,
        turnId: 1,
      } as unknown as StreamLine,
    ];

    render(<ChatTranscript lines={lines} status="idle" />);

    expect(screen.getByText('Invalid workflow card payload')).toBeInTheDocument();
  });

  it('does not render the pending placeholder when explicitly disabled', () => {
    const lines: StreamLine[] = [
      {
        id: 1,
        content: 'user message',
        type: 'info',
        timestamp: 1,
        turnId: 1,
        turnBoundary: 'user',
      },
    ];

    render(<ChatTranscript lines={lines} status="running" showPendingPlaceholder={false} />);

    expect(screen.queryByText('Thinking...')).not.toBeInTheDocument();
  });

  it('renders the pending placeholder for the last turn when enabled', () => {
    const lines: StreamLine[] = [
      {
        id: 1,
        content: 'user message',
        type: 'info',
        timestamp: 1,
        turnId: 1,
        turnBoundary: 'user',
      },
    ];

    render(<ChatTranscript lines={lines} status="running" showPendingPlaceholder />);

    expect(screen.getByText('Thinking...')).toBeInTheDocument();
  });

  it('uses the provided fork handler instead of the legacy execution-store fork', () => {
    const customFork = vi.fn();
    mockForkSessionAtTurn.mockClear();

    render(<ChatTranscript lines={buildLines(1)} status="idle" onFork={customFork} />);

    fireEvent.click(screen.getByLabelText('Fork'));
    expect(customFork).toHaveBeenCalledWith(1);
    expect(mockForkSessionAtTurn).not.toHaveBeenCalled();
  });
});
