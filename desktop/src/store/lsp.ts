/**
 * LSP Settings Store
 *
 * Zustand store for LSP server detection and enrichment state. Keeps the UI
 * in sync with the backend detection / enrichment results via Tauri IPC.
 */

import { create } from 'zustand';
import type { LspServerStatus, EnrichmentReport } from '../types/lsp';
import { detectLspServers, getLspStatus, triggerLspEnrichment, getEnrichmentReport } from '../lib/lspApi';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface LspState {
  // Server detection state
  servers: LspServerStatus[];
  isDetecting: boolean;

  // Enrichment state
  enrichmentReport: EnrichmentReport | null;
  isEnriching: boolean;

  // Preference
  autoEnrich: boolean;

  // Incremental enrichment debounce (ms)
  enrichmentDebounceMs: number;

  // UI state
  error: string | null;

  // Actions
  detect: () => Promise<void>;
  fetchStatus: () => Promise<void>;
  enrich: (projectPath: string) => Promise<void>;
  fetchReport: () => Promise<void>;
  setAutoEnrich: (enabled: boolean) => void;
  setEnrichmentDebounceMs: (ms: number) => void;
  clearError: () => void;
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_STATE = {
  servers: [],
  isDetecting: false,
  enrichmentReport: null,
  isEnriching: false,
  autoEnrich: false,
  enrichmentDebounceMs: 3000,
  error: null,
};

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useLspStore = create<LspState>()((set) => ({
  ...DEFAULT_STATE,

  detect: async () => {
    set({ isDetecting: true, error: null });
    try {
      const result = await detectLspServers();
      if (result.success && result.data) {
        set({ servers: result.data, isDetecting: false });
      } else {
        set({
          isDetecting: false,
          error: result.error ?? 'Failed to detect LSP servers',
        });
      }
    } catch (err) {
      set({
        isDetecting: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  fetchStatus: async () => {
    try {
      const result = await getLspStatus();
      if (result.success && result.data) {
        set({ servers: result.data });
      }
    } catch {
      // Silently fail — status fetch is non-critical for initial render
    }
  },

  enrich: async (projectPath: string) => {
    set({ isEnriching: true, error: null });
    try {
      const result = await triggerLspEnrichment(projectPath);
      if (result.success && result.data) {
        set({ enrichmentReport: result.data, isEnriching: false });
      } else {
        set({
          isEnriching: false,
          error: result.error ?? 'Enrichment failed',
        });
      }
    } catch (err) {
      set({
        isEnriching: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  fetchReport: async () => {
    try {
      const result = await getEnrichmentReport();
      if (result.success && result.data) {
        set({ enrichmentReport: result.data });
      }
    } catch {
      // Silently fail — report fetch is non-critical
    }
  },

  setAutoEnrich: (enabled: boolean) => {
    set({ autoEnrich: enabled });
    // Persist preference to localStorage
    try {
      localStorage.setItem('plan-cascade-lsp-auto-enrich', JSON.stringify(enabled));
    } catch {
      // Ignore storage errors
    }
  },

  setEnrichmentDebounceMs: (ms: number) => {
    set({ enrichmentDebounceMs: ms });
    try {
      localStorage.setItem('plan-cascade-lsp-enrichment-debounce', JSON.stringify(ms));
    } catch {
      // Ignore storage errors
    }
  },

  clearError: () => set({ error: null }),
}));

// Hydrate preferences from localStorage on module load
try {
  const stored = localStorage.getItem('plan-cascade-lsp-auto-enrich');
  if (stored !== null) {
    useLspStore.setState({ autoEnrich: JSON.parse(stored) });
  }
} catch {
  // Ignore parse/storage errors
}
try {
  const debounce = localStorage.getItem('plan-cascade-lsp-enrichment-debounce');
  if (debounce !== null) {
    const ms = JSON.parse(debounce);
    if (typeof ms === 'number' && ms > 0) {
      useLspStore.setState({ enrichmentDebounceMs: ms });
    }
  }
} catch {
  // Ignore parse/storage errors
}

export default useLspStore;
