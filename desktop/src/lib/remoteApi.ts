/**
 * Remote Control API (IPC Wrappers)
 *
 * Type-safe wrappers for the Tauri remote control IPC commands defined in
 * `src-tauri/src/commands/remote.rs`.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type RemoteAdapterType = 'Telegram';

export type StreamingMode =
  | 'WaitForComplete'
  | { PeriodicUpdate: { interval_secs: number } }
  | { LiveEdit: { throttle_ms: number } };

export interface GatewayStatus {
  running: boolean;
  adapter_type: RemoteAdapterType | null;
  connected_since: string | null;
  active_remote_sessions: number;
  total_commands_processed: number;
  last_command_at: string | null;
  error: string | null;
}

export interface RemoteGatewayConfig {
  enabled: boolean;
  adapter: RemoteAdapterType;
  auto_start: boolean;
}

export interface TelegramAdapterConfig {
  bot_token: string | null;
  allowed_chat_ids: number[];
  allowed_user_ids: number[];
  require_password: boolean;
  access_password: string | null;
  max_message_length: number;
  streaming_mode: StreamingMode;
}

export interface RemoteSessionMapping {
  chat_id: number;
  user_id: number;
  local_session_id: string | null;
  session_type: string;
  created_at: string;
}

export interface RemoteAuditEntry {
  id: string;
  adapter_type: string;
  chat_id: number;
  user_id: number;
  username: string | null;
  command_text: string;
  command_type: string;
  result_status: string;
  error_message: string | null;
  created_at: string;
}

export interface AuditLogResponse {
  entries: RemoteAuditEntry[];
  total: number;
}

export interface UpdateRemoteConfigRequest {
  enabled?: boolean;
  adapter?: RemoteAdapterType;
  auto_start?: boolean;
}

export interface UpdateTelegramConfigRequest {
  bot_token?: string;
  allowed_chat_ids?: number[];
  allowed_user_ids?: number[];
  require_password?: boolean;
  access_password?: string;
  max_message_length?: number;
  streaming_mode?: StreamingMode;
}

// ---------------------------------------------------------------------------
// IPC Wrappers
// ---------------------------------------------------------------------------

/**
 * Get the current remote gateway status.
 */
export async function getRemoteGatewayStatus(): Promise<CommandResponse<GatewayStatus>> {
  try {
    return await invoke<CommandResponse<GatewayStatus>>('get_remote_gateway_status');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Start the remote gateway with Telegram adapter.
 */
export async function startRemoteGateway(): Promise<CommandResponse<void>> {
  try {
    return await invoke<CommandResponse<void>>('start_remote_gateway');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Stop the remote gateway.
 */
export async function stopRemoteGateway(): Promise<CommandResponse<void>> {
  try {
    return await invoke<CommandResponse<void>>('stop_remote_gateway');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Get remote gateway configuration.
 */
export async function getRemoteConfig(): Promise<CommandResponse<RemoteGatewayConfig>> {
  try {
    return await invoke<CommandResponse<RemoteGatewayConfig>>('get_remote_config');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Update remote gateway configuration.
 */
export async function updateRemoteConfig(
  request: UpdateRemoteConfigRequest,
): Promise<CommandResponse<void>> {
  try {
    return await invoke<CommandResponse<void>>('update_remote_config', { request });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Get Telegram adapter configuration.
 */
export async function getTelegramConfig(): Promise<CommandResponse<TelegramAdapterConfig>> {
  try {
    return await invoke<CommandResponse<TelegramAdapterConfig>>('get_telegram_config');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Update Telegram adapter configuration.
 */
export async function updateTelegramConfig(
  request: UpdateTelegramConfigRequest,
): Promise<CommandResponse<void>> {
  try {
    return await invoke<CommandResponse<void>>('update_telegram_config', { request });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * List active remote session mappings.
 */
export async function listRemoteSessions(): Promise<CommandResponse<RemoteSessionMapping[]>> {
  try {
    return await invoke<CommandResponse<RemoteSessionMapping[]>>('list_remote_sessions');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Disconnect a remote session by chat_id.
 */
export async function disconnectRemoteSession(
  chatId: number,
): Promise<CommandResponse<void>> {
  try {
    return await invoke<CommandResponse<void>>('disconnect_remote_session', {
      chatId,
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
 * Query remote audit log with pagination.
 */
export async function getRemoteAuditLog(
  limit?: number,
  offset?: number,
): Promise<CommandResponse<AuditLogResponse>> {
  try {
    return await invoke<CommandResponse<AuditLogResponse>>('get_remote_audit_log', {
      limit: limit ?? null,
      offset: offset ?? null,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
