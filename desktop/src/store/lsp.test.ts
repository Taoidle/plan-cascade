import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../lib/lspApi', () => ({
  detectLspServers: vi.fn(),
  getLspStatus: vi.fn(),
  triggerLspEnrichment: vi.fn(),
  getEnrichmentReport: vi.fn(),
  getLspPreferences: vi.fn(),
  setLspPreferences: vi.fn(),
}));

import { useLspStore } from './lsp';
import * as lspApi from '../lib/lspApi';

describe('useLspStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useLspStore.setState({
      servers: [],
      isDetecting: false,
      enrichmentReport: null,
      isEnriching: false,
      autoEnrich: false,
      enrichmentDebounceMs: 3000,
      preferencesLoaded: false,
      error: null,
    });
    localStorage.clear();
  });

  it('loads preferences from backend source of truth', async () => {
    localStorage.setItem('plan-cascade-lsp-auto-enrich', JSON.stringify(false));
    localStorage.setItem('plan-cascade-lsp-enrichment-debounce', JSON.stringify(1000));

    vi.mocked(lspApi.getLspPreferences).mockResolvedValue({
      success: true,
      data: { autoEnrich: true, incrementalDebounceMs: 5000 },
      error: null,
    });

    await useLspStore.getState().loadPreferences();

    const state = useLspStore.getState();
    expect(state.autoEnrich).toBe(true);
    expect(state.enrichmentDebounceMs).toBe(5000);
    expect(state.preferencesLoaded).toBe(true);
  });

  it('persists debounce updates to backend', async () => {
    vi.mocked(lspApi.setLspPreferences).mockResolvedValue({
      success: true,
      data: { autoEnrich: false, incrementalDebounceMs: 2000 },
      error: null,
    });

    await useLspStore.getState().setEnrichmentDebounceMs(2000);

    expect(lspApi.setLspPreferences).toHaveBeenCalledWith({
      autoEnrich: false,
      incrementalDebounceMs: 2000,
    });
    expect(useLspStore.getState().enrichmentDebounceMs).toBe(2000);
  });

  it('rolls back UI when saving preferences fails', async () => {
    vi.mocked(lspApi.setLspPreferences).mockResolvedValue({
      success: false,
      data: null,
      error: 'incrementalDebounceMs must be between 500 and 60000 ms',
    });

    await useLspStore.getState().setAutoEnrich(true);

    const state = useLspStore.getState();
    expect(state.autoEnrich).toBe(false);
    expect(state.error).toContain('incrementalDebounceMs');
  });

  it('supports force-refresh detection and updates server list', async () => {
    vi.mocked(lspApi.detectLspServers).mockResolvedValue({
      success: true,
      data: [
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
      error: null,
    });

    await useLspStore.getState().detect(true);

    expect(lspApi.detectLspServers).toHaveBeenCalledWith(true);
    expect(useLspStore.getState().servers).toHaveLength(1);
    expect(useLspStore.getState().servers[0].detected).toBe(true);
  });

  it('keeps specific enrichment failure codes', async () => {
    vi.mocked(lspApi.triggerLspEnrichment).mockResolvedValue({
      success: false,
      data: null,
      error: 'LSP_NO_SERVERS_DETECTED: no language servers available',
    });

    await useLspStore.getState().enrich('/workspace');

    expect(useLspStore.getState().error).toContain('LSP_NO_SERVERS_DETECTED');
  });
});
