/**
 * Permission Policy API (IPC Wrappers)
 *
 * Type-safe wrappers for the Tauri permission policy IPC commands defined in
 * `src-tauri/src/commands/permissions.rs`.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';

export interface PermissionPolicyConfig {
  network_domain_allowlist: string[];
  builtin_network_domain_allowlist: string[];
  builtin_network_domain_allowlist_version: string;
  builtin_network_domain_allowlist_available_versions: string[];
}

export interface SetPermissionPolicyConfigRequest {
  network_domain_allowlist: string[];
}

/**
 * Get the current runtime permission policy config.
 */
export async function getPermissionPolicyConfig(): Promise<CommandResponse<PermissionPolicyConfig>> {
  try {
    return await invoke<CommandResponse<PermissionPolicyConfig>>('get_permission_policy_config');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Replace the runtime permission policy config.
 */
export async function setPermissionPolicyConfig(
  request: SetPermissionPolicyConfigRequest,
): Promise<CommandResponse<null>> {
  try {
    return await invoke<CommandResponse<null>>('set_permission_policy_config', { request });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
