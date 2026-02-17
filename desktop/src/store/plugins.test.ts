/**
 * Plugin Store Tests
 *
 * Tests for the Zustand plugin store including state management,
 * optimistic toggle updates, and IPC action mocking.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { usePluginStore } from './plugins';
import type { PluginInfo, PluginDetail } from '../types/plugin';

// Mock invoke is already mocked in test setup
const mockInvoke = vi.mocked(invoke);

// Helper factories
function createMockPluginInfo(overrides: Partial<PluginInfo> = {}): PluginInfo {
  return {
    name: 'test-plugin',
    version: '1.0.0',
    description: 'A test plugin',
    source: 'installed',
    enabled: true,
    skill_count: 2,
    command_count: 1,
    hook_count: 3,
    has_instructions: true,
    author: 'Test Author',
    ...overrides,
  };
}

function createMockPluginDetail(overrides: Partial<PluginDetail> = {}): PluginDetail {
  return {
    plugin: {
      manifest: {
        name: 'test-plugin',
        version: '1.0.0',
        description: 'A test plugin',
        author: 'Test Author',
        repository: null,
        license: 'MIT',
        keywords: ['test'],
      },
      source: 'installed',
      enabled: true,
      root_path: '/plugins/test-plugin',
      skills: [],
      commands: [],
      hooks: [],
      instructions: null,
      permissions: { allow: [], deny: [], always_approve: [] },
    },
    root_path: '/plugins/test-plugin',
    ...overrides,
  };
}

describe('usePluginStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    usePluginStore.getState().reset();
  });

  // ========================================================================
  // Initial State Tests
  // ========================================================================

  describe('Initial State', () => {
    it('should initialize with empty plugins and no selection', () => {
      const state = usePluginStore.getState();
      expect(state.plugins).toHaveLength(0);
      expect(state.selectedPlugin).toBeNull();
      expect(state.loading).toBe(false);
      expect(state.detailLoading).toBe(false);
      expect(state.refreshing).toBe(false);
      expect(state.error).toBeNull();
      expect(state.toastMessage).toBeNull();
      expect(state.toastType).toBe('info');
    });

    it('should reset to default state', () => {
      const store = usePluginStore.getState();
      store.showToast('test message', 'success');
      store.reset();
      const state = usePluginStore.getState();
      expect(state.toastMessage).toBeNull();
      expect(state.toastType).toBe('info');
      expect(state.plugins).toHaveLength(0);
    });
  });

  // ========================================================================
  // Toast Tests
  // ========================================================================

  describe('Toast', () => {
    it('should show and clear toast', () => {
      const { showToast, clearToast } = usePluginStore.getState();
      showToast('Plugin enabled', 'success');
      expect(usePluginStore.getState().toastMessage).toBe('Plugin enabled');
      expect(usePluginStore.getState().toastType).toBe('success');
      clearToast();
      expect(usePluginStore.getState().toastMessage).toBeNull();
    });

    it('should default toast type to info', () => {
      const { showToast } = usePluginStore.getState();
      showToast('Info message');
      expect(usePluginStore.getState().toastType).toBe('info');
    });
  });

  // ========================================================================
  // loadPlugins Tests
  // ========================================================================

  describe('loadPlugins', () => {
    it('should load plugins successfully', async () => {
      const mockPlugins = [
        createMockPluginInfo({ name: 'plugin-a' }),
        createMockPluginInfo({ name: 'plugin-b' }),
      ];
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: mockPlugins,
        error: null,
      });

      await usePluginStore.getState().loadPlugins();

      const state = usePluginStore.getState();
      expect(state.plugins).toHaveLength(2);
      expect(state.plugins[0].name).toBe('plugin-a');
      expect(state.loading).toBe(false);
      expect(state.error).toBeNull();
      expect(mockInvoke).toHaveBeenCalledWith('list_plugins');
    });

    it('should handle load plugins error response', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Discovery failed',
      });

      await usePluginStore.getState().loadPlugins();

      const state = usePluginStore.getState();
      expect(state.plugins).toHaveLength(0);
      expect(state.error).toBe('Discovery failed');
      expect(state.loading).toBe(false);
    });

    it('should handle load plugins exception', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('IPC error'));

      await usePluginStore.getState().loadPlugins();

      const state = usePluginStore.getState();
      expect(state.error).toBe('IPC error');
      expect(state.loading).toBe(false);
    });
  });

  // ========================================================================
  // togglePlugin Tests
  // ========================================================================

  describe('togglePlugin', () => {
    it('should toggle plugin with optimistic update', async () => {
      usePluginStore.setState({
        plugins: [createMockPluginInfo({ name: 'my-plugin', enabled: true })],
      });

      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await usePluginStore.getState().togglePlugin('my-plugin', false);

      const plugin = usePluginStore.getState().plugins.find((p) => p.name === 'my-plugin');
      expect(plugin?.enabled).toBe(false);
    });

    it('should revert toggle on failure', async () => {
      usePluginStore.setState({
        plugins: [createMockPluginInfo({ name: 'my-plugin', enabled: true })],
      });

      mockInvoke.mockResolvedValueOnce({ success: false, data: null, error: 'Toggle failed' });

      await usePluginStore.getState().togglePlugin('my-plugin', false);

      const plugin = usePluginStore.getState().plugins.find((p) => p.name === 'my-plugin');
      expect(plugin?.enabled).toBe(true);
      expect(usePluginStore.getState().toastMessage).toBe('Toggle failed');
      expect(usePluginStore.getState().toastType).toBe('error');
    });

    it('should revert toggle on exception', async () => {
      usePluginStore.setState({
        plugins: [createMockPluginInfo({ name: 'my-plugin', enabled: false })],
      });

      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      await usePluginStore.getState().togglePlugin('my-plugin', true);

      const plugin = usePluginStore.getState().plugins.find((p) => p.name === 'my-plugin');
      expect(plugin?.enabled).toBe(false);
      expect(usePluginStore.getState().toastMessage).toBe('Network error');
    });

    it('should show success toast on successful toggle', async () => {
      usePluginStore.setState({
        plugins: [createMockPluginInfo({ name: 'my-plugin', enabled: false })],
      });

      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await usePluginStore.getState().togglePlugin('my-plugin', true);

      expect(usePluginStore.getState().toastMessage).toBe('Plugin "my-plugin" enabled');
      expect(usePluginStore.getState().toastType).toBe('success');
    });
  });

  // ========================================================================
  // refresh Tests
  // ========================================================================

  describe('refresh', () => {
    it('should refresh plugins successfully', async () => {
      const mockPlugins = [
        createMockPluginInfo({ name: 'refreshed-plugin' }),
      ];
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: mockPlugins,
        error: null,
      });

      await usePluginStore.getState().refresh();

      const state = usePluginStore.getState();
      expect(state.plugins).toHaveLength(1);
      expect(state.plugins[0].name).toBe('refreshed-plugin');
      expect(state.refreshing).toBe(false);
      expect(state.toastMessage).toBe('Plugins refreshed');
    });

    it('should handle refresh failure', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Refresh failed',
      });

      await usePluginStore.getState().refresh();

      const state = usePluginStore.getState();
      expect(state.refreshing).toBe(false);
      expect(state.toastMessage).toBe('Refresh failed');
      expect(state.toastType).toBe('error');
    });

    it('should handle refresh exception', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Connection lost'));

      await usePluginStore.getState().refresh();

      const state = usePluginStore.getState();
      expect(state.refreshing).toBe(false);
      expect(state.toastMessage).toBe('Connection lost');
      expect(state.toastType).toBe('error');
    });
  });

  // ========================================================================
  // loadPluginDetail Tests
  // ========================================================================

  describe('loadPluginDetail', () => {
    it('should load plugin detail successfully', async () => {
      const mockDetail = createMockPluginDetail();
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: mockDetail,
        error: null,
      });

      await usePluginStore.getState().loadPluginDetail('test-plugin');

      const state = usePluginStore.getState();
      expect(state.selectedPlugin).not.toBeNull();
      expect(state.selectedPlugin?.plugin.manifest.name).toBe('test-plugin');
      expect(state.detailLoading).toBe(false);
    });

    it('should handle detail load failure', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Plugin not found',
      });

      await usePluginStore.getState().loadPluginDetail('missing-plugin');

      const state = usePluginStore.getState();
      expect(state.selectedPlugin).toBeNull();
      expect(state.detailLoading).toBe(false);
      expect(state.toastMessage).toBe('Plugin not found');
      expect(state.toastType).toBe('error');
    });

    it('should handle detail load exception', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('IPC timeout'));

      await usePluginStore.getState().loadPluginDetail('some-plugin');

      const state = usePluginStore.getState();
      expect(state.selectedPlugin).toBeNull();
      expect(state.detailLoading).toBe(false);
      expect(state.toastMessage).toBe('IPC timeout');
    });
  });

  // ========================================================================
  // clearSelectedPlugin Tests
  // ========================================================================

  describe('clearSelectedPlugin', () => {
    it('should clear selected plugin', () => {
      usePluginStore.setState({
        selectedPlugin: createMockPluginDetail(),
      });

      usePluginStore.getState().clearSelectedPlugin();

      expect(usePluginStore.getState().selectedPlugin).toBeNull();
    });
  });
});
