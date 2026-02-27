/**
 * Embedding Settings Store
 *
 * Zustand store for embedding provider configuration state. Keeps the UI in
 * sync with the backend config persisted via Tauri IPC commands.
 */

import { create } from 'zustand';
import type { EmbeddingProviderCapability, EmbeddingConfigResponse, EmbeddingHealthResponse } from '../types/embedding';
import {
  getEmbeddingConfig,
  setEmbeddingConfig,
  listEmbeddingProviders,
  checkEmbeddingProviderHealth,
  getEmbeddingApiKey,
  setEmbeddingApiKey,
  getCodebaseIndexConfig,
  setCodebaseIndexConfig,
} from '../lib/embeddingApi';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface EmbeddingState {
  // Current config (mirrors backend state)
  provider: string;
  model: string;
  baseUrl: string;
  dimension: number;
  batchSize: number;
  fallbackProvider: string;

  // Provider catalog
  providers: EmbeddingProviderCapability[];

  // Index exclusion config
  builtinExcludedDirs: string[];
  builtinExcludedExtensions: string[];
  extraExcludedDirs: string[];
  extraExcludedExtensions: string[];
  exclusionsLoading: boolean;

  // UI state
  loading: boolean;
  saving: boolean;
  healthChecking: boolean;
  healthResult: EmbeddingHealthResponse | null;
  error: string | null;
  reindexRequired: boolean;

  // Actions
  fetchConfig: () => Promise<void>;
  fetchProviders: () => Promise<void>;
  fetchIndexConfig: () => Promise<void>;
  setProvider: (provider: string) => void;
  setModel: (model: string) => void;
  setBaseUrl: (baseUrl: string) => void;
  setDimension: (dimension: number) => void;
  setBatchSize: (batchSize: number) => void;
  setFallbackProvider: (fallbackProvider: string) => void;
  saveConfig: () => Promise<boolean>;
  checkHealth: () => Promise<void>;
  saveApiKey: (provider: string, apiKey: string) => Promise<boolean>;
  loadApiKey: (provider: string) => Promise<string | null>;
  addExcludedDir: (dir: string) => void;
  removeExcludedDir: (dir: string) => void;
  addExcludedExtension: (ext: string) => void;
  removeExcludedExtension: (ext: string) => void;
  saveIndexConfig: () => Promise<boolean>;
  clearError: () => void;
  clearHealthResult: () => void;
  clearReindexRequired: () => void;
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_STATE = {
  provider: 'tf_idf',
  model: 'tfidf',
  baseUrl: '',
  dimension: 0,
  batchSize: 32,
  fallbackProvider: '',
  providers: [],
  builtinExcludedDirs: [] as string[],
  builtinExcludedExtensions: [] as string[],
  extraExcludedDirs: [] as string[],
  extraExcludedExtensions: [] as string[],
  exclusionsLoading: false,
  loading: false,
  saving: false,
  healthChecking: false,
  healthResult: null,
  error: null,
  reindexRequired: false,
};

// ---------------------------------------------------------------------------
// Helper: Apply config response to store state
// ---------------------------------------------------------------------------

function configToState(config: EmbeddingConfigResponse): Partial<EmbeddingState> {
  return {
    provider: config.provider,
    model: config.model,
    baseUrl: config.base_url ?? '',
    dimension: config.dimension,
    batchSize: config.batch_size,
    fallbackProvider: config.fallback_provider ?? '',
  };
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useEmbeddingStore = create<EmbeddingState>()((set, get) => ({
  ...DEFAULT_STATE,

  fetchConfig: async () => {
    set({ loading: true, error: null });
    try {
      const result = await getEmbeddingConfig();
      if (result.success && result.data) {
        set({ ...configToState(result.data), loading: false });
      } else {
        set({ loading: false, error: result.error ?? 'Failed to fetch embedding config' });
      }
    } catch (err) {
      set({
        loading: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  fetchProviders: async () => {
    try {
      const result = await listEmbeddingProviders();
      if (result.success && result.data) {
        set({ providers: result.data });
      }
    } catch {
      // Silently fail — providers list is non-critical for initial render
    }
  },

  fetchIndexConfig: async () => {
    set({ exclusionsLoading: true });
    try {
      const result = await getCodebaseIndexConfig();
      if (result.success && result.data) {
        set({
          builtinExcludedDirs: result.data.builtin_excluded_dirs,
          builtinExcludedExtensions: result.data.builtin_excluded_extensions,
          extraExcludedDirs: result.data.extra_excluded_dirs,
          extraExcludedExtensions: result.data.extra_excluded_extensions,
          exclusionsLoading: false,
        });
      } else {
        set({ exclusionsLoading: false });
      }
    } catch {
      set({ exclusionsLoading: false });
    }
  },

  setProvider: (provider: string) => {
    // When switching providers, apply provider defaults
    const capability = get().providers.find((p) => p.provider_type === provider);
    if (capability) {
      set({
        provider,
        model: capability.default_model,
        dimension: capability.default_dimension,
        batchSize: Math.min(capability.max_batch_size, 32),
        baseUrl: provider === 'ollama' ? 'http://localhost:11434' : '',
      });
    } else {
      set({ provider });
    }
  },

  setModel: (model: string) => {
    const capability = get().providers.find((p) => p.provider_type === get().provider);
    const preset = capability?.models?.find((m) => m.model_id === model);
    if (preset) {
      set({
        model,
        dimension: preset.default_dimension,
        batchSize: Math.min(preset.max_batch_size, get().batchSize),
      });
    } else {
      set({ model });
    }
  },
  setBaseUrl: (baseUrl: string) => set({ baseUrl }),
  setDimension: (dimension: number) => set({ dimension }),
  setBatchSize: (batchSize: number) => set({ batchSize }),
  setFallbackProvider: (fallbackProvider: string) => set({ fallbackProvider }),

  saveConfig: async () => {
    const state = get();
    set({ saving: true, error: null, reindexRequired: false });
    try {
      const result = await setEmbeddingConfig({
        provider: state.provider,
        model: state.model || undefined,
        base_url: state.baseUrl || undefined,
        dimension: state.dimension || undefined,
        batch_size: state.batchSize || undefined,
        fallback_provider: state.fallbackProvider || undefined,
      });
      if (result.success && result.data) {
        set({
          saving: false,
          reindexRequired: result.data.reindex_required,
        });
        return true;
      } else {
        set({
          saving: false,
          error: result.error ?? 'Failed to save embedding config',
        });
        return false;
      }
    } catch (err) {
      set({
        saving: false,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  checkHealth: async () => {
    const state = get();
    set({ healthChecking: true, healthResult: null, error: null });
    try {
      const result = await checkEmbeddingProviderHealth({
        provider: state.provider,
        model: state.model || undefined,
        base_url: state.baseUrl || undefined,
      });
      if (result.success && result.data) {
        set({ healthChecking: false, healthResult: result.data });
      } else {
        set({
          healthChecking: false,
          error: result.error ?? 'Health check failed',
        });
      }
    } catch (err) {
      set({
        healthChecking: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  saveApiKey: async (provider: string, apiKey: string) => {
    set({ error: null });
    try {
      const result = await setEmbeddingApiKey({
        provider,
        api_key: apiKey,
      });
      if (result.success && result.data?.success) {
        return true;
      } else {
        set({ error: result.error ?? 'Failed to save API key' });
        return false;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      return false;
    }
  },

  loadApiKey: async (provider: string) => {
    try {
      const result = await getEmbeddingApiKey({ provider });
      if (result.success && typeof result.data === 'string' && result.data.trim().length > 0) {
        return result.data;
      }
      return null;
    } catch {
      return null;
    }
  },

  addExcludedDir: (dir: string) => {
    const trimmed = dir.trim();
    if (!trimmed) return;
    const state = get();
    if (state.extraExcludedDirs.includes(trimmed) || state.builtinExcludedDirs.includes(trimmed)) return;
    set({ extraExcludedDirs: [...state.extraExcludedDirs, trimmed] });
  },

  removeExcludedDir: (dir: string) => {
    set({ extraExcludedDirs: get().extraExcludedDirs.filter((d) => d !== dir) });
  },

  addExcludedExtension: (ext: string) => {
    const normalized = ext.trim().replace(/^\./, '').toLowerCase();
    if (!normalized) return;
    const state = get();
    if (state.extraExcludedExtensions.includes(normalized) || state.builtinExcludedExtensions.includes(normalized))
      return;
    set({ extraExcludedExtensions: [...state.extraExcludedExtensions, normalized] });
  },

  removeExcludedExtension: (ext: string) => {
    set({ extraExcludedExtensions: get().extraExcludedExtensions.filter((e) => e !== ext) });
  },

  saveIndexConfig: async () => {
    const state = get();
    set({ saving: true, error: null });
    try {
      const result = await setCodebaseIndexConfig({
        extra_excluded_dirs: state.extraExcludedDirs,
        extra_excluded_extensions: state.extraExcludedExtensions,
      });
      if (result.success) {
        set({ saving: false, reindexRequired: true });
        return true;
      } else {
        set({ saving: false, error: result.error ?? 'Failed to save index exclusion config' });
        return false;
      }
    } catch (err) {
      set({ saving: false, error: err instanceof Error ? err.message : String(err) });
      return false;
    }
  },

  clearError: () => set({ error: null }),
  clearHealthResult: () => set({ healthResult: null }),
  clearReindexRequired: () => set({ reindexRequired: false }),
}));

export default useEmbeddingStore;
