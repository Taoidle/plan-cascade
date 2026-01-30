/**
 * Settings API (v5.0 Pure Rust Backend)
 *
 * Provides type-safe access to settings via Tauri IPC.
 * Replaces the legacy HTTP-based API that connected to Python sidecar.
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

export interface AppConfig {
  theme: string;
  language: string;
  default_provider: string;
  default_model: string;
  analytics_enabled: boolean;
  auto_save_interval: number;
  max_recent_projects: number;
  debug_mode: boolean;
}

export interface SettingsUpdate {
  theme?: string;
  language?: string;
  default_provider?: string;
  default_model?: string;
  analytics_enabled?: boolean;
  auto_save_interval?: number;
  max_recent_projects?: number;
  debug_mode?: boolean;
}

// ============================================================================
// Settings API Functions
// ============================================================================

/**
 * Get current application settings
 */
export async function getSettings(): Promise<AppConfig> {
  const result = await invoke<CommandResponse<AppConfig>>('get_settings');
  if (!result.success || !result.data) {
    throw new Error(result.error || 'Failed to get settings');
  }
  return result.data;
}

/**
 * Update application settings (partial update)
 */
export async function updateSettings(update: SettingsUpdate): Promise<AppConfig> {
  const result = await invoke<CommandResponse<AppConfig>>('update_settings', { update });
  if (!result.success || !result.data) {
    throw new Error(result.error || 'Failed to update settings');
  }
  return result.data;
}

/**
 * Export settings to JSON (client-side export using current store state)
 */
export function exportSettingsToJson(settings: object): string {
  const exportData = {
    version: '5.0',
    exported_at: new Date().toISOString(),
    settings,
  };
  return JSON.stringify(exportData, null, 2);
}

/**
 * Parse imported settings JSON
 */
export function parseImportedSettings(jsonContent: string): { version: string; settings: object } {
  const parsed = JSON.parse(jsonContent);

  if (!parsed.version || !parsed.settings) {
    throw new Error('Invalid settings file format. Expected "version" and "settings" fields.');
  }

  return parsed;
}

/**
 * Check if running in Tauri context
 */
export function isTauriAvailable(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export default {
  getSettings,
  updateSettings,
  exportSettingsToJson,
  parseImportedSettings,
  isTauriAvailable,
};
