import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';

export type GuardrailScope = 'input' | 'tool_call' | 'tool_result' | 'assistant_output' | 'artifact';
export type GuardrailMode = 'off' | 'monitor_only' | 'balanced' | 'strict';

export interface GuardrailInfo {
  id: string;
  name: string;
  guardrail_type: 'builtin' | 'custom';
  builtin_key?: string | null;
  pattern?: string | null;
  enabled: boolean;
  scope: GuardrailScope[];
  action: string;
  editable: boolean;
  description: string;
}

export interface GuardrailRuntimeStatus {
  mode: GuardrailMode;
  strict_mode: boolean;
  native_runtime_managed: boolean;
  claude_code_managed: boolean;
  init_error?: string | null;
}

export interface GuardrailOverview {
  guardrails: GuardrailInfo[];
  runtime: GuardrailRuntimeStatus;
}

export interface GuardrailEventEntry {
  id: number;
  rule_id: string;
  rule_name: string;
  surface: string;
  tool_name?: string | null;
  session_id?: string | null;
  execution_id?: string | null;
  decision: string;
  content_hash: string;
  safe_preview: string;
  timestamp: string;
}

export interface CustomGuardrailInput {
  id?: string;
  name: string;
  pattern: string;
  action: string;
  enabled: boolean;
  scope: GuardrailScope[];
  description: string;
}

function toErrorResponse<T>(error: unknown): CommandResponse<T> {
  return {
    success: false,
    data: null,
    error: error instanceof Error ? error.message : String(error),
  };
}

export async function listGuardrails(): Promise<CommandResponse<GuardrailOverview>> {
  try {
    return await invoke<CommandResponse<GuardrailOverview>>('list_guardrails');
  } catch (error) {
    return toErrorResponse(error);
  }
}

export async function toggleGuardrailEnabled(id: string, enabled: boolean): Promise<CommandResponse<GuardrailInfo>> {
  try {
    return await invoke<CommandResponse<GuardrailInfo>>('toggle_guardrail', { id, enabled });
  } catch (error) {
    return toErrorResponse(error);
  }
}

export async function setGuardrailMode(mode: GuardrailMode): Promise<CommandResponse<GuardrailRuntimeStatus>> {
  try {
    return await invoke<CommandResponse<GuardrailRuntimeStatus>>('set_guardrail_mode', { mode });
  } catch (error) {
    return toErrorResponse(error);
  }
}

export async function createCustomGuardrail(rule: CustomGuardrailInput): Promise<CommandResponse<GuardrailInfo>> {
  try {
    return await invoke<CommandResponse<GuardrailInfo>>('create_custom_guardrail', { rule });
  } catch (error) {
    return toErrorResponse(error);
  }
}

export async function updateGuardrail(rule: CustomGuardrailInput): Promise<CommandResponse<GuardrailInfo>> {
  try {
    return await invoke<CommandResponse<GuardrailInfo>>('update_guardrail', { rule });
  } catch (error) {
    return toErrorResponse(error);
  }
}

export async function deleteGuardrail(id: string): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('delete_guardrail', { id });
  } catch (error) {
    return toErrorResponse(error);
  }
}

export async function listGuardrailEvents(
  limit?: number,
  offset?: number,
): Promise<CommandResponse<GuardrailEventEntry[]>> {
  try {
    return await invoke<CommandResponse<GuardrailEventEntry[]>>('list_guardrail_events', {
      limit: limit ?? 50,
      offset: offset ?? 0,
    });
  } catch (error) {
    return toErrorResponse(error);
  }
}

export async function clearGuardrailEvents(): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('clear_guardrail_events');
  } catch (error) {
    return toErrorResponse(error);
  }
}

// Compatibility exports for existing tests/callers.
export const addCustomRule = (name: string, pattern: string, action: string) =>
  createCustomGuardrail({
    name,
    pattern,
    action,
    enabled: true,
    scope: ['input', 'assistant_output', 'tool_result'],
    description: '',
  });

export const removeCustomRule = deleteGuardrail;
export const getTriggerLog = listGuardrailEvents;
export const clearTriggerLog = clearGuardrailEvents;
