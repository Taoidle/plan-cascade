/**
 * Settings API (v6.0 Unified Export/Import)
 *
 * Provides type-safe access to settings via Tauri IPC.
 * Includes unified export/import covering frontend + backend + encrypted secrets.
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

/** Unified settings export (v6.0) */
export interface UnifiedSettingsExport {
  version: string;
  exported_at: string;
  has_encrypted_secrets: boolean;
  frontend: Record<string, unknown>;
  backend: {
    config: Record<string, unknown>;
    embedding: Record<string, unknown> | null;
    proxy: {
      global: Record<string, unknown> | null;
      strategies: Record<string, unknown>;
      custom_configs: Record<string, unknown>;
    };
    webhooks: Record<string, unknown>[];
    guardrails: { id: string; name: string; pattern: string; action: string; enabled: boolean }[];
    remote: {
      gateway: Record<string, unknown> | null;
      telegram: Record<string, unknown> | null;
    };
    a2a_agents: Record<string, unknown>[];
    mcp_servers: Record<string, unknown>[];
    plugin_settings: Record<string, unknown> | null;
  };
  encrypted_secrets: string | null;
}

/** Import result from the backend */
export interface ImportResult {
  success: boolean;
  frontend: Record<string, unknown> | null;
  imported_sections: string[];
  skipped_sections: string[];
  warnings: string[];
  errors: string[];
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
 * Export all settings (frontend + backend + optionally encrypted API keys)
 */
export async function exportAllSettings(
  frontendState: Record<string, unknown>,
  password: string | null,
): Promise<UnifiedSettingsExport> {
  const result = await invoke<CommandResponse<UnifiedSettingsExport>>('export_all_settings', {
    frontendState,
    password,
  });
  if (!result.success || !result.data) {
    throw new Error(result.error || 'Failed to export settings');
  }
  return result.data;
}

/**
 * Import settings from a unified export JSON string
 */
export async function importAllSettings(exportJson: string, password: string | null): Promise<ImportResult> {
  const result = await invoke<CommandResponse<ImportResult>>('import_all_settings', {
    exportJson,
    password,
  });
  if (!result.success || !result.data) {
    throw new Error(result.error || 'Failed to import settings');
  }
  return result.data;
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
  exportAllSettings,
  importAllSettings,
  isTauriAvailable,
};
