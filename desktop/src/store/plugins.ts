/**
 * Plugin Store
 *
 * Zustand store for managing plugin state with IPC actions
 * to the Tauri Rust backend.
 */

import { create } from 'zustand';
import type { PluginInfo, PluginDetail } from '../types/plugin';
import {
  listPlugins,
  togglePlugin as togglePluginApi,
  refreshPlugins as refreshPluginsApi,
  getPluginDetail as getPluginDetailApi,
} from '../lib/pluginApi';

// ============================================================================
// State Types
// ============================================================================

interface PluginState {
  // Data
  plugins: PluginInfo[];
  selectedPlugin: PluginDetail | null;

  // Loading states
  loading: boolean;
  detailLoading: boolean;
  refreshing: boolean;

  // Error state
  error: string | null;

  // Toast
  toastMessage: string | null;
  toastType: 'success' | 'error' | 'info';

  // Actions
  loadPlugins: () => Promise<void>;
  togglePlugin: (name: string, enabled: boolean) => Promise<void>;
  refresh: () => Promise<void>;
  loadPluginDetail: (name: string) => Promise<void>;
  clearSelectedPlugin: () => void;
  showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
  clearToast: () => void;
  reset: () => void;
}

// ============================================================================
// Default State
// ============================================================================

const defaultState = {
  plugins: [] as PluginInfo[],
  selectedPlugin: null as PluginDetail | null,
  loading: false,
  detailLoading: false,
  refreshing: false,
  error: null as string | null,
  toastMessage: null as string | null,
  toastType: 'info' as const,
};

// ============================================================================
// Store
// ============================================================================

export const usePluginStore = create<PluginState>()((set, get) => ({
  ...defaultState,

  loadPlugins: async () => {
    set({ loading: true, error: null });
    try {
      const response = await listPlugins();
      if (response.success && response.data) {
        set({ plugins: response.data, loading: false });
      } else {
        set({
          error: response.error || 'Failed to load plugins',
          loading: false,
        });
      }
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
        loading: false,
      });
    }
  },

  togglePlugin: async (name: string, enabled: boolean) => {
    // Optimistic update
    set((state) => ({
      plugins: state.plugins.map((p) =>
        p.name === name ? { ...p, enabled } : p
      ),
    }));

    try {
      const response = await togglePluginApi(name, enabled);
      if (!response.success) {
        // Revert on failure
        set((state) => ({
          plugins: state.plugins.map((p) =>
            p.name === name ? { ...p, enabled: !enabled } : p
          ),
        }));
        get().showToast(response.error || 'Failed to toggle plugin', 'error');
      } else {
        get().showToast(
          `Plugin "${name}" ${enabled ? 'enabled' : 'disabled'}`,
          'success'
        );
      }
    } catch (error) {
      // Revert on error
      set((state) => ({
        plugins: state.plugins.map((p) =>
          p.name === name ? { ...p, enabled: !enabled } : p
        ),
      }));
      get().showToast(
        error instanceof Error ? error.message : String(error),
        'error'
      );
    }
  },

  refresh: async () => {
    set({ refreshing: true });
    try {
      const response = await refreshPluginsApi();
      if (response.success && response.data) {
        set({ plugins: response.data, refreshing: false });
        get().showToast('Plugins refreshed', 'success');
      } else {
        set({ refreshing: false });
        get().showToast(response.error || 'Refresh failed', 'error');
      }
    } catch (error) {
      set({ refreshing: false });
      get().showToast(
        error instanceof Error ? error.message : String(error),
        'error'
      );
    }
  },

  loadPluginDetail: async (name: string) => {
    set({ detailLoading: true, selectedPlugin: null });
    try {
      const response = await getPluginDetailApi(name);
      if (response.success && response.data) {
        set({ selectedPlugin: response.data, detailLoading: false });
      } else {
        set({ detailLoading: false });
        get().showToast(response.error || 'Plugin not found', 'error');
      }
    } catch (error) {
      set({ detailLoading: false });
      get().showToast(
        error instanceof Error ? error.message : String(error),
        'error'
      );
    }
  },

  clearSelectedPlugin: () => set({ selectedPlugin: null }),

  showToast: (message: string, type: 'success' | 'error' | 'info' = 'info') =>
    set({ toastMessage: message, toastType: type }),

  clearToast: () => set({ toastMessage: null }),

  reset: () => set(defaultState),
}));

export default usePluginStore;
