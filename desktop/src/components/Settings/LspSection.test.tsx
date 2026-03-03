import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { LspSection } from './LspSection';

let mockLspStore: ReturnType<typeof createMockLspStore>;

function createMockLspStore() {
  return {
    servers: [
      {
        language: 'rust' as const,
        server_name: 'rust-analyzer',
        detected: true,
        binary_path: '/usr/local/bin/rust-analyzer',
        version: '1.82.0',
        detected_at: '2026-03-03T12:00:00Z',
        install_hint: 'rustup component add rust-analyzer',
      },
    ],
    isDetecting: false,
    enrichmentReport: null,
    isEnriching: false,
    autoEnrich: false,
    enrichmentDebounceMs: 3000,
    preferencesLoaded: true,
    error: null as string | null,
    detect: vi.fn().mockResolvedValue(undefined),
    fetchStatus: vi.fn().mockResolvedValue(undefined),
    enrich: vi.fn().mockResolvedValue(undefined),
    fetchReport: vi.fn().mockResolvedValue(undefined),
    loadPreferences: vi.fn().mockResolvedValue(undefined),
    setAutoEnrich: vi.fn().mockResolvedValue(undefined),
    setEnrichmentDebounceMs: vi.fn().mockResolvedValue(undefined),
    clearError: vi.fn(),
  };
}

vi.mock('../../store/lsp', () => ({
  useLspStore: () => mockLspStore,
}));

vi.mock('../../store/settings', () => ({
  useSettingsStore: (selector: (state: { workspacePath: string }) => unknown) =>
    selector({ workspacePath: '/workspace' }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe('LspSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockLspStore = createMockLspStore();
  });

  it('loads status/report/preferences on mount', async () => {
    render(<LspSection />);

    await waitFor(() => {
      expect(mockLspStore.fetchStatus).toHaveBeenCalledTimes(1);
      expect(mockLspStore.fetchReport).toHaveBeenCalledTimes(1);
      expect(mockLspStore.loadPreferences).toHaveBeenCalledTimes(1);
    });
  });

  it('forces refresh when detect button is clicked', async () => {
    render(<LspSection />);

    fireEvent.click(screen.getByRole('button', { name: 'lsp.servers.detectButton' }));

    await waitFor(() => {
      expect(mockLspStore.detect).toHaveBeenCalledWith(true);
    });
  });

  it('shows binary path and version for detected server', () => {
    render(<LspSection />);

    expect(screen.getByText('lsp.servers.version', { exact: false })).toBeInTheDocument();
    expect(screen.getByText('/usr/local/bin/rust-analyzer')).toBeInTheDocument();
  });

  it('maps known LSP error codes to i18n keys', () => {
    mockLspStore.error = 'LSP_NO_SERVERS_DETECTED: missing';

    render(<LspSection />);

    expect(screen.getByText('lsp.errors.LSP_NO_SERVERS_DETECTED')).toBeInTheDocument();
  });

  it('persists debounce changes through store action', async () => {
    render(<LspSection />);

    fireEvent.change(screen.getByRole('combobox'), {
      target: { value: '5000' },
    });

    await waitFor(() => {
      expect(mockLspStore.setEnrichmentDebounceMs).toHaveBeenCalledWith(5000);
    });
  });
});
