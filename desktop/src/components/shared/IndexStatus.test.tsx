import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { IndexStatus } from './IndexStatus';
import { useSettingsStore } from '../../store/settings';
import { useLspStore } from '../../store/lsp';

const { mockInvoke, mockListen } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  mockListen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: mockListen,
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (key === 'indexing.readyFiles') {
        return `${options?.count ?? 0} files`;
      }
      return key;
    },
  }),
}));

describe('IndexStatus LSP auto-enrich gating', () => {
  const applyInvokeMock = (detectedServers: Array<Record<string, unknown>>) => {
    mockInvoke.mockImplementation((command: string) => {
      if (command === 'get_index_status') {
        return Promise.resolve({
          success: true,
          data: {
            project_path: '/workspace/demo',
            status: 'indexed',
            indexed_files: 12,
            total_files: 12,
            error_message: null,
            total_symbols: 42,
            embedding_chunks: 30,
            embedding_provider_name: 'tfidf',
            lsp_enrichment: 'none',
          },
          error: null,
        });
      }
      if (command === 'get_lsp_status') {
        return Promise.resolve({
          success: true,
          data: detectedServers,
          error: null,
        });
      }
      if (command === 'get_lsp_preferences') {
        return Promise.resolve({
          success: true,
          data: {
            autoEnrich: true,
            incrementalDebounceMs: 3000,
          },
          error: null,
        });
      }
      return Promise.resolve({ success: true, data: null, error: null });
    });
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mockListen.mockResolvedValue(() => {});
    applyInvokeMock([]);

    useSettingsStore.setState({ workspacePath: '/workspace/demo' });
  });

  it('does not auto-trigger enrichment when no servers are detected', async () => {
    const enrichSpy = vi.fn();
    useLspStore.setState({
      autoEnrich: true,
      preferencesLoaded: true,
      servers: [],
      isEnriching: false,
      enrich: enrichSpy,
    });

    render(<IndexStatus />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_index_status', {
        projectPath: '/workspace/demo',
      });
    });

    await waitFor(() => {
      expect(enrichSpy).not.toHaveBeenCalled();
    });
  });

  it('auto-triggers enrichment when preferences are loaded and a server exists', async () => {
    applyInvokeMock([
      {
        language: 'rust',
        server_name: 'rust-analyzer',
        detected: true,
        binary_path: '/usr/bin/rust-analyzer',
        version: '1.82.0',
        detected_at: '2026-03-03T12:00:00Z',
        install_hint: 'rustup component add rust-analyzer',
      },
    ]);

    const enrichSpy = vi.fn();
    useLspStore.setState({
      autoEnrich: true,
      preferencesLoaded: true,
      servers: [
        {
          language: 'rust',
          server_name: 'rust-analyzer',
          detected: true,
          binary_path: '/usr/bin/rust-analyzer',
          version: '1.82.0',
          detected_at: '2026-03-03T12:00:00Z',
          install_hint: 'rustup component add rust-analyzer',
        },
      ],
      isEnriching: false,
      enrich: enrichSpy,
    });

    render(<IndexStatus />);

    await waitFor(() => {
      expect(enrichSpy).toHaveBeenCalledWith('/workspace/demo');
    });
  });
});
