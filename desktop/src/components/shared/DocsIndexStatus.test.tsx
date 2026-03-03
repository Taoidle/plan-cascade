import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { DocsIndexStatus } from './DocsIndexStatus';
import { useProjectsStore } from '../../store/projects';
import { useSettingsStore } from '../../store/settings';

const { mockRagGetDocsStatus, mockRagSyncDocsCollection, mockRagRebuildDocsCollection } = vi.hoisted(() => ({
  mockRagGetDocsStatus: vi.fn(),
  mockRagSyncDocsCollection: vi.fn(),
  mockRagRebuildDocsCollection: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      const translations: Record<string, string> = {
        'docsIndexing.queued': 'Docs queued',
        'docsIndexing.scanning': 'Scanning docs...',
        'docsIndexing.indexing': `Docs ${opts?.progress ?? 0}%`,
        'docsIndexing.indexed': `${opts?.count ?? 0} docs`,
        'docsIndexing.retryWaiting': 'Retry waiting',
        'docsIndexing.retryNow': 'Retry now',
        'docsIndexing.changesPending': 'Docs changed',
        'docsIndexing.syncing': 'Syncing docs...',
        'docsIndexing.syncDocs': 'Sync',
        'docsIndexing.rebuild': 'Rebuild docs',
        'docsIndexing.rebuilding': 'Rebuilding docs...',
        'docsIndexing.error': 'Docs error',
      };
      return translations[key] ?? key;
    },
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation(() => Promise.resolve(() => {})),
}));

vi.mock('../../lib/knowledgeApi', () => ({
  ragGetDocsStatus: mockRagGetDocsStatus,
  ragSyncDocsCollection: mockRagSyncDocsCollection,
  ragRebuildDocsCollection: mockRagRebuildDocsCollection,
}));

describe('DocsIndexStatus', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSettingsStore.setState({ workspacePath: '/workspace/demo' });
    useProjectsStore.setState({
      selectedProject: {
        id: 'proj-1',
        name: 'Project 1',
        path: '/workspace/demo',
        last_activity: new Date().toISOString(),
        session_count: 0,
        message_count: 0,
      },
    });
    mockRagSyncDocsCollection.mockResolvedValue({ success: true, data: null, error: null });
    mockRagRebuildDocsCollection.mockResolvedValue({ success: true, data: null, error: null });
  });

  it('shows indexed state even when total docs is zero', async () => {
    mockRagGetDocsStatus.mockResolvedValue({
      success: true,
      data: {
        collection_id: 'docs-col',
        collection_name: '[Docs] demo',
        total_docs: 0,
        pending_changes: [],
        status: 'indexed',
      },
      error: null,
    });

    render(<DocsIndexStatus />);

    await waitFor(() => {
      expect(screen.getByText('0 docs')).toBeInTheDocument();
    });
  });

  it('renders rebuild action for error state and triggers rebuild', async () => {
    mockRagGetDocsStatus.mockResolvedValue({
      success: true,
      data: {
        collection_id: 'docs-col',
        collection_name: '[Docs] demo',
        total_docs: 0,
        pending_changes: [],
        status: 'error',
      },
      error: null,
    });

    render(<DocsIndexStatus />);

    const rebuildButton = await screen.findByRole('button', { name: 'Rebuild docs' });
    fireEvent.click(rebuildButton);

    await waitFor(() => {
      expect(mockRagRebuildDocsCollection).toHaveBeenCalledWith('/workspace/demo', 'proj-1', 'safe_swap');
    });
  });
});
