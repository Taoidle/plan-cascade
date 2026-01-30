/**
 * Tauri API Wrapper (v5.0 Pure Rust Backend)
 *
 * Provides type-safe access to Tauri commands for the Pure Rust backend.
 * No longer uses Python sidecar - all functionality is in Rust.
 */

import { invoke } from '@tauri-apps/api/core';

// Response types
export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

export interface HealthResponse {
  status: string;
  version: string;
  service: string;
}

/**
 * Check backend health
 */
export async function getHealth(): Promise<CommandResponse<HealthResponse>> {
  try {
    return await invoke<CommandResponse<HealthResponse>>('get_health');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Utility to check if running in Tauri context
 */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// Re-export Claude Code client for convenience
export {
  ClaudeCodeClient,
  getClaudeCodeClient,
  initClaudeCodeClient,
  closeClaudeCodeClient,
  type ConnectionStatus,
  type ClaudeCodeSession,
  type ActiveSessionInfo,
  type StartChatRequest,
  type StartChatResponse,
  type SendMessageRequest,
  type StreamEventPayload,
  type ThinkingUpdateEvent,
  type ToolUpdateEvent,
  type SessionUpdateEvent,
  EventChannels,
} from './claudeCodeClient';
