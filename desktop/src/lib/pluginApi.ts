/**
 * Plugin API
 *
 * TypeScript wrapper for Tauri plugin commands.
 * Provides type-safe access to the plugin system backend.
 */

import { invoke } from '@tauri-apps/api/core';
import type { PluginInfo, PluginDetail, PluginSkill, MarketplacePlugin, MarketplaceInfo } from '../types/plugin';

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

/**
 * List all discovered plugins.
 */
export async function listPlugins(): Promise<CommandResponse<PluginInfo[]>> {
  try {
    return await invoke<CommandResponse<PluginInfo[]>>('list_plugins');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * List user-invocable plugin skills.
 * Returns skills from enabled plugins where user_invocable is true.
 */
export async function listInvocableSkills(): Promise<CommandResponse<PluginSkill[]>> {
  try {
    return await invoke<CommandResponse<PluginSkill[]>>('list_invocable_plugin_skills');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Toggle a plugin's enabled/disabled state.
 */
export async function togglePlugin(
  name: string,
  enabled: boolean
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('toggle_plugin', {
      name,
      enabled,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Refresh plugin discovery (re-scan all sources).
 */
export async function refreshPlugins(): Promise<CommandResponse<PluginInfo[]>> {
  try {
    return await invoke<CommandResponse<PluginInfo[]>>('refresh_plugins');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Get detailed information about a specific plugin.
 */
export async function getPluginDetail(
  name: string
): Promise<CommandResponse<PluginDetail>> {
  try {
    return await invoke<CommandResponse<PluginDetail>>('get_plugin_detail', {
      name,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Install a plugin from a source directory.
 */
export async function installPlugin(
  sourcePath: string
): Promise<CommandResponse<PluginInfo>> {
  try {
    return await invoke<CommandResponse<PluginInfo>>('install_plugin', {
      sourcePath,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Fetch marketplace plugins from the registry.
 */
export async function fetchMarketplace(
  registryUrl?: string
): Promise<CommandResponse<MarketplacePlugin[]>> {
  try {
    return await invoke<CommandResponse<MarketplacePlugin[]>>(
      'fetch_marketplace',
      { registryUrl: registryUrl ?? null }
    );
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Install a plugin from a git URL.
 */
export async function installPluginFromGit(
  gitUrl: string
): Promise<CommandResponse<PluginInfo>> {
  try {
    return await invoke<CommandResponse<PluginInfo>>(
      'install_plugin_from_git',
      { gitUrl }
    );
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Uninstall a plugin by name.
 */
export async function uninstallPlugin(
  name: string
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('uninstall_plugin', {
      name,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * List all configured marketplaces.
 */
export async function listMarketplaces(): Promise<CommandResponse<MarketplaceInfo[]>> {
  try {
    return await invoke<CommandResponse<MarketplaceInfo[]>>('list_marketplaces');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Add a new marketplace source.
 */
export async function addMarketplace(
  source: string
): Promise<CommandResponse<MarketplaceInfo>> {
  try {
    return await invoke<CommandResponse<MarketplaceInfo>>('add_marketplace', {
      source,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Remove a marketplace source.
 */
export async function removeMarketplace(
  name: string
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('remove_marketplace', {
      name,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Toggle a marketplace's enabled/disabled state.
 */
export async function toggleMarketplace(
  name: string,
  enabled: boolean
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('toggle_marketplace', {
      name,
      enabled,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Install a plugin from a specific marketplace.
 */
export async function installMarketplacePlugin(
  pluginName: string,
  marketplaceName: string
): Promise<CommandResponse<PluginInfo>> {
  try {
    return await invoke<CommandResponse<PluginInfo>>(
      'install_marketplace_plugin',
      { pluginName, marketplaceName }
    );
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
