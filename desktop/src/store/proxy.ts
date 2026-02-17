/**
 * Proxy Settings Store
 *
 * Zustand store for managing proxy configuration state.
 */

import { create } from 'zustand';
import {
  getProxyConfig,
  setProxyConfig,
  setProviderProxyStrategy,
  testProxy,
  type ProxyConfig,
  type ProxyStrategy,
  type ProxyTestResult,
} from '../lib/proxyApi';

interface ProxyState {
  // Global proxy
  globalProxy: ProxyConfig | null;
  globalPassword: string;

  // Per-provider strategies and custom configs
  providerStrategies: Record<string, ProxyStrategy>;
  providerConfigs: Record<string, ProxyConfig>;

  // UI state
  loading: boolean;
  saving: boolean;
  testing: boolean;
  testResult: ProxyTestResult | null;
  error: string | null;

  // Actions
  fetchProxyConfig: () => Promise<void>;
  setGlobalProxy: (proxy: ProxyConfig | null, password?: string) => Promise<boolean>;
  setProviderStrategy: (
    provider: string,
    strategy: ProxyStrategy,
    customProxy?: ProxyConfig,
    customPassword?: string,
  ) => Promise<boolean>;
  testProxyConnection: (proxy: ProxyConfig, password?: string) => Promise<void>;
  clearTestResult: () => void;
  clearError: () => void;
}

export const useProxyStore = create<ProxyState>((set, get) => ({
  globalProxy: null,
  globalPassword: '',
  providerStrategies: {},
  providerConfigs: {},
  loading: false,
  saving: false,
  testing: false,
  testResult: null,
  error: null,

  fetchProxyConfig: async () => {
    set({ loading: true, error: null });
    try {
      const response = await getProxyConfig();
      if (response.success && response.data) {
        set({
          globalProxy: response.data.global,
          providerStrategies: response.data.provider_strategies,
          providerConfigs: response.data.provider_configs,
          loading: false,
        });
      } else {
        set({ loading: false, error: response.error ?? 'Failed to load proxy config' });
      }
    } catch (e) {
      set({ loading: false, error: e instanceof Error ? e.message : String(e) });
    }
  },

  setGlobalProxy: async (proxy, password) => {
    set({ saving: true, error: null });
    try {
      const response = await setProxyConfig({ proxy, password });
      if (response.success) {
        set({ globalProxy: proxy, saving: false });
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to save proxy config' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  setProviderStrategy: async (provider, strategy, customProxy, customPassword) => {
    set({ saving: true, error: null });
    try {
      const response = await setProviderProxyStrategy({
        provider,
        strategy,
        custom_proxy: customProxy,
        custom_password: customPassword,
      });
      if (response.success) {
        const strategies = { ...get().providerStrategies, [provider]: strategy };
        const configs = { ...get().providerConfigs };
        if (strategy === 'custom' && customProxy) {
          configs[provider] = customProxy;
        } else {
          delete configs[provider];
        }
        set({ providerStrategies: strategies, providerConfigs: configs, saving: false });
        return true;
      } else {
        set({ saving: false, error: response.error ?? 'Failed to save provider strategy' });
        return false;
      }
    } catch (e) {
      set({ saving: false, error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  testProxyConnection: async (proxy, password) => {
    set({ testing: true, testResult: null });
    try {
      const response = await testProxy({ proxy, password });
      if (response.success && response.data) {
        set({ testing: false, testResult: response.data });
      } else {
        set({
          testing: false,
          testResult: {
            success: false,
            error: response.error ?? 'Test failed',
          },
        });
      }
    } catch (e) {
      set({
        testing: false,
        testResult: {
          success: false,
          error: e instanceof Error ? e.message : String(e),
        },
      });
    }
  },

  clearTestResult: () => set({ testResult: null }),
  clearError: () => set({ error: null }),
}));
