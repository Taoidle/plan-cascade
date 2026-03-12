import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { FileChangeEntry } from './FileChangeEntry';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) =>
      (
        ({
          'aiChanges.mode.task': 'Task',
          'aiChanges.actor.sub_agent': 'Sub-agent',
        }) as Record<string, string>
      )[key] ??
      options?.defaultValue ??
      key,
  }),
}));

const mockStoreState = {
  expandedChangeIds: new Set<string>(),
  toggleExpanded: vi.fn(),
  fetchDiff: vi.fn().mockResolvedValue(null),
  diffCache: new Map<string, string>(),
};

vi.mock('../../../../store/fileChanges', () => ({
  useFileChangesStore: (selector: (state: typeof mockStoreState) => unknown) => selector(mockStoreState),
}));

vi.mock('../../../shared/DiffViewer', () => ({
  DiffViewer: () => <div>diff</div>,
}));

describe('FileChangeEntry', () => {
  it('renders source mode and actor badges for attributed changes', () => {
    render(
      <FileChangeEntry
        sessionId="root-session"
        projectRoot="/repo"
        change={{
          id: 'change-1',
          session_id: 'root-session',
          turn_index: 2,
          tool_call_id: 'tool-1',
          tool_name: 'Edit',
          file_path: 'src/app.ts',
          before_hash: 'before',
          after_hash: 'after',
          timestamp: Date.now(),
          description: 'Edited file',
          source_mode: 'task',
          actor_kind: 'sub_agent',
          actor_label: 'Story Agent',
        }}
      />,
    );

    expect(screen.getByText('Task')).toBeInTheDocument();
    expect(screen.getByText('Sub-agent')).toBeInTheDocument();
    expect(screen.getByText('Story Agent')).toBeInTheDocument();
    expect(screen.getByText('src/app.ts')).toBeInTheDocument();
  });
});
