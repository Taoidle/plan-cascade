/**
 * Guardrails API (IPC Wrappers)
 *
 * Type-safe wrappers for the Tauri guardrail commands defined in
 * `src-tauri/src/commands/guardrails.rs`. Each function follows the project
 * IPC pattern: `invoke<CommandResponse<T>>('command_name', { params })`.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Information about a guardrail for display. */
export interface GuardrailInfo {
  name: string;
  guardrail_type: string;
  enabled: boolean;
  description: string;
}

/** A single trigger log entry. */
export interface TriggerLogEntry {
  id: number;
  guardrail_name: string;
  direction: string;
  result_type: string;
  content_snippet: string;
  timestamp: string;
}

// ---------------------------------------------------------------------------
// list_guardrails
// ---------------------------------------------------------------------------

/**
 * List all guardrails with their name, type, enabled status, and description.
 */
export async function listGuardrails(): Promise<CommandResponse<GuardrailInfo[]>> {
  try {
    return await invoke<CommandResponse<GuardrailInfo[]>>('list_guardrails');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// toggle_guardrail
// ---------------------------------------------------------------------------

/**
 * Toggle a guardrail on or off by name.
 */
export async function toggleGuardrailEnabled(
  name: string,
  enabled: boolean,
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('toggle_guardrail', { name, enabled });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// add_custom_rule
// ---------------------------------------------------------------------------

/**
 * Add a new custom guardrail rule.
 */
export async function addCustomRule(
  name: string,
  pattern: string,
  action: string,
): Promise<CommandResponse<GuardrailInfo>> {
  try {
    return await invoke<CommandResponse<GuardrailInfo>>('add_custom_rule', {
      name,
      pattern,
      action,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// remove_custom_rule
// ---------------------------------------------------------------------------

/**
 * Remove a custom guardrail rule by name.
 */
export async function removeCustomRule(
  name: string,
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('remove_custom_rule', { name });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// get_trigger_log
// ---------------------------------------------------------------------------

/**
 * Get paginated trigger log entries.
 */
export async function getTriggerLog(
  limit?: number,
  offset?: number,
): Promise<CommandResponse<TriggerLogEntry[]>> {
  try {
    return await invoke<CommandResponse<TriggerLogEntry[]>>('get_trigger_log', {
      limit: limit ?? 50,
      offset: offset ?? 0,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// clear_trigger_log
// ---------------------------------------------------------------------------

/**
 * Clear all trigger log entries.
 */
export async function clearTriggerLog(): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('clear_trigger_log');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
