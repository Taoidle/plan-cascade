/**
 * Plugin Store
 *
 * Zustand store for managing plugin state with IPC actions
 * to the Tauri Rust backend.
 */

import { create } from 'zustand';
import type { PluginInfo, PluginDetail, MarketplacePlugin, MarketplaceInfo, InstallProgress } from '../types/plugin';
import {
  listPlugins,
  togglePlugin as togglePluginApi,
  refreshPlugins as refreshPluginsApi,
  getPluginDetail as getPluginDetailApi,
  fetchMarketplace as fetchMarketplaceApi,
  installPluginFromGit as installPluginFromGitApi,
  uninstallPlugin as uninstallPluginApi,
  listMarketplaces as listMarketplacesApi,
  addMarketplace as addMarketplaceApi,
  removeMarketplace as removeMarketplaceApi,
  toggleMarketplace as toggleMarketplaceApi,
  installMarketplacePlugin as installMarketplacePluginApi,
} from '../lib/pluginApi';

// ============================================================================
// State Types
// ============================================================================

interface PluginState {
  // Data
  plugins: PluginInfo[];
  selectedPlugin: PluginDetail | null;

  // Marketplace
  marketplacePlugins: MarketplacePlugin[];
  marketplaceLoading: boolean;
  marketplaceError: string | null;

  // Marketplace sources
  marketplaces: MarketplaceInfo[];
  marketplacesLoading: boolean;
  addingMarketplace: boolean;
  addMarketplaceDialogOpen: boolean;

  // Install
  installing: boolean;
  installProgress: InstallProgress | null;

  // Uninstall
  uninstalling: string | null;

  // Loading states
  loading: boolean;
  detailLoading: boolean;
  refreshing: boolean;

  // Error state
  error: string | null;

  // Toast
  toastMessage: string | null;
  toastType: 'success' | 'error' | 'info';

  // Panel & Dialog
  panelOpen: boolean;
  dialogOpen: boolean;
  activeTab: 'installed' | 'marketplace';
  installDialogOpen: boolean;

  // Actions
  loadPlugins: () => Promise<void>;
  togglePlugin: (name: string, enabled: boolean) => Promise<void>;
  refresh: () => Promise<void>;
  loadPluginDetail: (name: string) => Promise<void>;
  clearSelectedPlugin: () => void;
  showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
  clearToast: () => void;
  togglePanel: () => void;
  openDialog: () => void;
  closeDialog: () => void;
  setActiveTab: (tab: 'installed' | 'marketplace') => void;
  loadMarketplace: () => Promise<void>;
  installFromGit: (gitUrl: string) => Promise<void>;
  installFromMarketplace: (pluginName: string, marketplaceName: string) => Promise<void>;
  uninstallPlugin: (name: string) => Promise<void>;
  openInstallDialog: () => void;
  closeInstallDialog: () => void;
  setInstallProgress: (progress: InstallProgress | null) => void;
  loadMarketplaces: () => Promise<void>;
  addMarketplace: (source: string) => Promise<void>;
  removeMarketplace: (name: string) => Promise<void>;
  toggleMarketplaceSource: (name: string, enabled: boolean) => Promise<void>;
  openAddMarketplaceDialog: () => void;
  closeAddMarketplaceDialog: () => void;
  reset: () => void;
}

// ============================================================================
// Default State
// ============================================================================

const defaultState = {
  plugins: [] as PluginInfo[],
  selectedPlugin: null as PluginDetail | null,
  marketplacePlugins: [] as MarketplacePlugin[],
  marketplaceLoading: false,
  marketplaceError: null as string | null,
  marketplaces: [] as MarketplaceInfo[],
  marketplacesLoading: false,
  addingMarketplace: false,
  addMarketplaceDialogOpen: false,
  installing: false,
  installProgress: null as InstallProgress | null,
  uninstalling: null as string | null,
  loading: false,
  detailLoading: false,
  refreshing: false,
  error: null as string | null,
  toastMessage: null as string | null,
  toastType: 'info' as const,
  panelOpen: false,
  dialogOpen: false,
  activeTab: 'installed' as const,
  installDialogOpen: false,
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

  togglePanel: () => set((state) => ({ panelOpen: !state.panelOpen })),

  openDialog: () => set({ dialogOpen: true }),

  closeDialog: () => set({ dialogOpen: false, selectedPlugin: null }),

  setActiveTab: (tab: 'installed' | 'marketplace') => {
    set({ activeTab: tab });
    if (tab === 'marketplace' && get().marketplacePlugins.length === 0) {
      get().loadMarketplace();
    }
  },

  loadMarketplace: async () => {
    set({ marketplaceLoading: true, marketplaceError: null });
    try {
      const response = await fetchMarketplaceApi();
      if (response.success && response.data) {
        set({ marketplacePlugins: response.data, marketplaceLoading: false });
      } else {
        set({
          marketplaceError: response.error || 'Failed to load marketplace',
          marketplaceLoading: false,
        });
      }
    } catch (error) {
      set({
        marketplaceError: error instanceof Error ? error.message : String(error),
        marketplaceLoading: false,
      });
    }
  },

  installFromGit: async (gitUrl: string) => {
    set({ installing: true, installProgress: null });
    try {
      const response = await installPluginFromGitApi(gitUrl);
      if (response.success && response.data) {
        set({ installing: false, installProgress: null, installDialogOpen: false });
        get().showToast(`Plugin "${response.data.name}" installed successfully`, 'success');
        // Refresh both lists
        await get().loadPlugins();
        if (get().marketplacePlugins.length > 0) {
          await get().loadMarketplace();
        }
      } else {
        set({ installing: false, installProgress: null });
        get().showToast(response.error || 'Installation failed', 'error');
      }
    } catch (error) {
      set({ installing: false, installProgress: null });
      get().showToast(
        error instanceof Error ? error.message : String(error),
        'error'
      );
    }
  },

  uninstallPlugin: async (name: string) => {
    set({ uninstalling: name });
    try {
      const response = await uninstallPluginApi(name);
      if (response.success) {
        set({ uninstalling: null });
        get().showToast(`Plugin "${name}" uninstalled`, 'success');
        // Refresh both lists
        await get().loadPlugins();
        if (get().marketplacePlugins.length > 0) {
          await get().loadMarketplace();
        }
      } else {
        set({ uninstalling: null });
        get().showToast(response.error || 'Uninstall failed', 'error');
      }
    } catch (error) {
      set({ uninstalling: null });
      get().showToast(
        error instanceof Error ? error.message : String(error),
        'error'
      );
    }
  },

  installFromMarketplace: async (pluginName: string, marketplaceName: string) => {
    set({ installing: true, installProgress: null });
    try {
      const response = await installMarketplacePluginApi(pluginName, marketplaceName);
      if (response.success && response.data) {
        set({ installing: false, installProgress: null });
        get().showToast(`Plugin "${response.data.name}" installed successfully`, 'success');
        await get().loadPlugins();
        if (get().marketplacePlugins.length > 0) {
          await get().loadMarketplace();
        }
      } else {
        set({ installing: false, installProgress: null });
        get().showToast(response.error || 'Installation failed', 'error');
      }
    } catch (error) {
      set({ installing: false, installProgress: null });
      get().showToast(
        error instanceof Error ? error.message : String(error),
        'error'
      );
    }
  },

  openInstallDialog: () => set({ installDialogOpen: true }),

  closeInstallDialog: () => set({ installDialogOpen: false, installProgress: null }),

  setInstallProgress: (progress: InstallProgress | null) => set({ installProgress: progress }),

  loadMarketplaces: async () => {
    set({ marketplacesLoading: true });
    try {
      const response = await listMarketplacesApi();
      if (response.success && response.data) {
        set({ marketplaces: response.data, marketplacesLoading: false });
      } else {
        set({ marketplacesLoading: false });
      }
    } catch {
      set({ marketplacesLoading: false });
    }
  },

  addMarketplace: async (source: string) => {
    set({ addingMarketplace: true });
    try {
      const response = await addMarketplaceApi(source);
      if (response.success && response.data) {
        set((state) => ({
          addingMarketplace: false,
          addMarketplaceDialogOpen: false,
          marketplaces: [...state.marketplaces, response.data!],
        }));
        get().showToast(`Marketplace "${response.data.name}" added`, 'success');
        // Refresh marketplace plugins
        await get().loadMarketplace();
      } else {
        set({ addingMarketplace: false });
        get().showToast(response.error || 'Failed to add marketplace', 'error');
      }
    } catch (error) {
      set({ addingMarketplace: false });
      get().showToast(
        error instanceof Error ? error.message : String(error),
        'error'
      );
    }
  },

  removeMarketplace: async (name: string) => {
    try {
      const response = await removeMarketplaceApi(name);
      if (response.success) {
        set((state) => ({
          marketplaces: state.marketplaces.filter((m) => m.name !== name),
        }));
        get().showToast(`Marketplace "${name}" removed`, 'success');
        await get().loadMarketplace();
      } else {
        get().showToast(response.error || 'Failed to remove marketplace', 'error');
      }
    } catch (error) {
      get().showToast(
        error instanceof Error ? error.message : String(error),
        'error'
      );
    }
  },

  toggleMarketplaceSource: async (name: string, enabled: boolean) => {
    // Optimistic update
    set((state) => ({
      marketplaces: state.marketplaces.map((m) =>
        m.name === name ? { ...m, enabled } : m
      ),
    }));

    try {
      const response = await toggleMarketplaceApi(name, enabled);
      if (!response.success) {
        // Revert
        set((state) => ({
          marketplaces: state.marketplaces.map((m) =>
            m.name === name ? { ...m, enabled: !enabled } : m
          ),
        }));
        get().showToast(response.error || 'Failed to toggle marketplace', 'error');
      } else {
        await get().loadMarketplace();
      }
    } catch {
      // Revert
      set((state) => ({
        marketplaces: state.marketplaces.map((m) =>
          m.name === name ? { ...m, enabled: !enabled } : m
        ),
      }));
    }
  },

  openAddMarketplaceDialog: () => set({ addMarketplaceDialogOpen: true }),

  closeAddMarketplaceDialog: () => set({ addMarketplaceDialogOpen: false }),

  reset: () => set(defaultState),
}));

export default usePluginStore;
