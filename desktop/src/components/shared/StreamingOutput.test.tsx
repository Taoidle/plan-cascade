import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { StreamingOutput } from './StreamingOutput';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) => {
        const translations: Record<string, string> = {
          'output.headerLines': '1 lines',
          'output.done': 'Done',
          'export.capturing': 'Capturing...',
        };
        return translations[key] ?? key;
      },
    }),
  };
});

vi.mock('../../store/mode', () => ({
  useModeStore: () => 'simple',
}));

vi.mock('../../store/execution', async () => {
  const actual = await vi.importActual<typeof import('../../store/execution')>('../../store/execution');
  return {
    ...actual,
    useExecutionStore: () => ({
      streamingOutput: [],
      clearStreamingOutput: vi.fn(),
      status: 'completed',
    }),
  };
});

vi.mock('../ClaudeCodeMode/MarkdownRenderer', () => ({
  MarkdownRenderer: ({ content }: { content: string }) => <div>{content}</div>,
}));

describe('StreamingOutput', () => {
  it('keeps export visible when clear is hidden', () => {
    render(
      <StreamingOutput
        showClear={false}
        lines={[
          {
            id: 1,
            type: 'text',
            content: 'hello world',
            timestamp: Date.now(),
          },
        ]}
        statusOverride="completed"
      />,
    );

    expect(screen.getByTitle('Export output')).toBeInTheDocument();
    expect(screen.queryByTitle('Clear output')).not.toBeInTheDocument();
  });
});
