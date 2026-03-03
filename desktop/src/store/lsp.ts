/**
 * LSP Settings Store
 *
 * Zustand store for LSP server detection and enrichment state.
 * Preferences are persisted in backend settings via Tauri IPC.
 */

import { create } from 'zustand';
import type { LspServerStatus, EnrichmentReport, LspPreferences } from '../types/lsp';
import {
  detectLspServers,
  getLspStatus,
  triggerLspEnrichment,
  getEnrichmentReport,
  getLspPreferences,
  setLspPreferences,
} from '../lib/lspApi';

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
  preferencesLoaded: boolean;

  // UI state
  error: string | null;

  // Actions
  detect: (forceRefresh?: boolean) => Promise<void>;
  fetchStatus: () => Promise<void>;
  enrich: (projectPath: string) => Promise<void>;
  fetchReport: () => Promise<void>;
  loadPreferences: () => Promise<void>;
  setAutoEnrich: (enabled: boolean) => Promise<void>;
  setEnrichmentDebounceMs: (ms: number) => Promise<void>;
  clearError: () => void;
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_DEBOUNCE_MS = 3000;

const DEFAULT_STATE = {
  servers: [],
  isDetecting: false,
  enrichmentReport: null,
  isEnriching: false,
  autoEnrich: false,
  enrichmentDebounceMs: DEFAULT_DEBOUNCE_MS,
  preferencesLoaded: false,
  error: null,
};

function asPreferences(state: Pick<LspState, 'autoEnrich' | 'enrichmentDebounceMs'>): LspPreferences {
  return {
    autoEnrich: state.autoEnrich,
    incrementalDebounceMs: state.enrichmentDebounceMs,
  };
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useLspStore = create<LspState>()((set, get) => ({
  ...DEFAULT_STATE,

  detect: async (forceRefresh = false) => {
    set({ isDetecting: true, error: null });
    try {
      const result = await detectLspServers(forceRefresh);
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
          error: result.error ?? 'LSP_ENRICHMENT_FAILED',
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

  loadPreferences: async () => {
    try {
      const result = await getLspPreferences();
      if (result.success && result.data) {
        set({
          autoEnrich: result.data.autoEnrich,
          enrichmentDebounceMs: result.data.incrementalDebounceMs,
          preferencesLoaded: true,
        });
      } else {
        set({
          preferencesLoaded: true,
          error: result.error ?? 'Failed to load LSP preferences',
        });
      }
    } catch (err) {
      set({
        preferencesLoaded: true,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  setAutoEnrich: async (enabled: boolean) => {
    const previous = get();
    set({ autoEnrich: enabled, error: null });

    const payload = asPreferences({
      autoEnrich: enabled,
      enrichmentDebounceMs: previous.enrichmentDebounceMs,
    });

    const result = await setLspPreferences(payload);
    if (result.success && result.data) {
      set({
        autoEnrich: result.data.autoEnrich,
        enrichmentDebounceMs: result.data.incrementalDebounceMs,
      });
      return;
    }

    set({
      autoEnrich: previous.autoEnrich,
      error: result.error ?? 'Failed to save LSP preferences',
    });
  },

  setEnrichmentDebounceMs: async (ms: number) => {
    const previous = get();
    set({ enrichmentDebounceMs: ms, error: null });

    const payload = asPreferences({
      autoEnrich: previous.autoEnrich,
      enrichmentDebounceMs: ms,
    });

    const result = await setLspPreferences(payload);
    if (result.success && result.data) {
      set({
        autoEnrich: result.data.autoEnrich,
        enrichmentDebounceMs: result.data.incrementalDebounceMs,
      });
      return;
    }

    set({
      enrichmentDebounceMs: previous.enrichmentDebounceMs,
      error: result.error ?? 'Failed to save LSP preferences',
    });
  },

  clearError: () => set({ error: null }),
}));

export default useLspStore;
