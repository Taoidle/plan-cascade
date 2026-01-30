/**
 * Claude Code Client (v5.0 Pure Rust Backend)
 *
 * Provides type-safe access to Claude Code functionality via Tauri IPC.
 * Replaces the legacy WebSocket-based client that connected to Python sidecar.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// ============================================================================
// Types from Rust Backend
// ============================================================================

export type SessionState = 'idle' | 'running' | 'waiting' | 'cancelled' | 'error';

export interface ClaudeCodeSession {
  id: string;
  project_path: string;
  created_at: string;
  last_message_at: string | null;
  resume_token: string | null;
  state: SessionState;
  model: string | null;
  error_message: string | null;
  message_count: number;
}

export interface ActiveSessionInfo {
  session: ClaudeCodeSession;
  pid: number | null;
  is_process_alive: boolean;
}

export interface StartChatRequest {
  project_path: string;
  model?: string;
  resume_session_id?: string;
}

export interface StartChatResponse {
  session_id: string;
  is_resumed: boolean;
}

export interface SendMessageRequest {
  session_id: string;
  prompt: string;
}

export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

// ============================================================================
// Event Types from Rust Backend
// ============================================================================

export interface StreamEventPayload {
  event: UnifiedStreamEvent;
  session_id: string;
}

export type UnifiedStreamEvent =
  | { TextDelta: { content: string } }
  | { ThinkingDelta: { content: string } }
  | { ToolUse: { tool_use_id: string; name: string; input: Record<string, unknown> } }
  | { ToolResult: { tool_use_id: string; success: boolean; output: string } }
  | { InputTokens: { count: number } }
  | { OutputTokens: { count: number } }
  | { Error: { message: string } }
  | { Done: Record<string, never> };

export interface ThinkingBlock {
  id: string;
  content: string;
  is_complete: boolean;
  started_at: string;
  completed_at: string | null;
}

export interface ThinkingUpdateEvent {
  block: ThinkingBlock;
  update_type: 'started' | 'updated' | 'completed';
  session_id: string;
}

export interface ToolExecution {
  id: string;
  tool_name: string;
  input: Record<string, unknown>;
  output: string | null;
  success: boolean | null;
  started_at: string;
  completed_at: string | null;
}

export interface ToolUpdateEvent {
  execution: ToolExecution;
  update_type: 'started' | 'completed';
  session_id: string;
}

export interface SessionUpdateEvent {
  session: ClaudeCodeSession;
  update_type: 'created' | 'state_changed' | 'message_sent' | 'removed';
  previous_state: SessionState | null;
}

// ============================================================================
// Event Channels
// ============================================================================

export const EventChannels = {
  STREAM: 'claude_code:stream',
  THINKING: 'claude_code:thinking',
  TOOL: 'claude_code:tool',
  SESSION: 'claude_code:session',
} as const;

// ============================================================================
// Connection Status (for backwards compatibility with store)
// ============================================================================

export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'reconnecting';

// ============================================================================
// Claude Code Client Class
// ============================================================================

export type StreamEventHandler = (event: StreamEventPayload) => void;
export type ThinkingEventHandler = (event: ThinkingUpdateEvent) => void;
export type ToolEventHandler = (event: ToolUpdateEvent) => void;
export type SessionEventHandler = (event: SessionUpdateEvent) => void;
export type StatusHandler = (status: ConnectionStatus) => void;

export class ClaudeCodeClient {
  private unlisteners: UnlistenFn[] = [];
  private statusHandlers: Set<StatusHandler> = new Set();
  private _status: ConnectionStatus = 'disconnected';

  /**
   * Get current connection status
   */
  get status(): ConnectionStatus {
    return this._status;
  }

  /**
   * Check if connected (in Tauri, always true when initialized)
   */
  get isConnected(): boolean {
    return this._status === 'connected';
  }

  /**
   * Initialize the client and set up event listeners
   */
  async connect(): Promise<void> {
    if (this._status === 'connected') {
      return;
    }

    this.setStatus('connecting');

    try {
      // Test connection by calling a simple command
      const result = await invoke<CommandResponse<boolean>>('get_health');
      if (result.success) {
        this.setStatus('connected');
      } else {
        // Health check might not exist, but Tauri IPC works
        this.setStatus('connected');
      }
    } catch {
      // Even if health check fails, IPC is available
      this.setStatus('connected');
    }
  }

  /**
   * Disconnect and clean up event listeners
   */
  async disconnect(): Promise<void> {
    for (const unlisten of this.unlisteners) {
      unlisten();
    }
    this.unlisteners = [];
    this.setStatus('disconnected');
  }

  /**
   * Subscribe to stream events
   */
  async onStreamEvent(handler: StreamEventHandler): Promise<() => void> {
    const unlisten = await listen<StreamEventPayload>(EventChannels.STREAM, (event) => {
      handler(event.payload);
    });
    this.unlisteners.push(unlisten);
    return unlisten;
  }

  /**
   * Subscribe to thinking events
   */
  async onThinkingEvent(handler: ThinkingEventHandler): Promise<() => void> {
    const unlisten = await listen<ThinkingUpdateEvent>(EventChannels.THINKING, (event) => {
      handler(event.payload);
    });
    this.unlisteners.push(unlisten);
    return unlisten;
  }

  /**
   * Subscribe to tool events
   */
  async onToolEvent(handler: ToolEventHandler): Promise<() => void> {
    const unlisten = await listen<ToolUpdateEvent>(EventChannels.TOOL, (event) => {
      handler(event.payload);
    });
    this.unlisteners.push(unlisten);
    return unlisten;
  }

  /**
   * Subscribe to session events
   */
  async onSessionEvent(handler: SessionEventHandler): Promise<() => void> {
    const unlisten = await listen<SessionUpdateEvent>(EventChannels.SESSION, (event) => {
      handler(event.payload);
    });
    this.unlisteners.push(unlisten);
    return unlisten;
  }

  /**
   * Subscribe to connection status changes
   */
  onStatusChange(handler: StatusHandler): () => void {
    this.statusHandlers.add(handler);
    // Immediately call with current status
    handler(this._status);

    return () => {
      this.statusHandlers.delete(handler);
    };
  }

  private setStatus(status: ConnectionStatus): void {
    this._status = status;
    this.statusHandlers.forEach((handler) => handler(status));
  }

  // ==========================================================================
  // Tauri Commands
  // ==========================================================================

  /**
   * Start a new Claude Code chat session
   */
  async startChat(request: StartChatRequest): Promise<StartChatResponse> {
    const result = await invoke<CommandResponse<StartChatResponse>>('start_chat', { request });
    if (!result.success || !result.data) {
      throw new Error(result.error || 'Failed to start chat');
    }
    return result.data;
  }

  /**
   * Send a message to a session
   * Events will be emitted through the event system
   */
  async sendMessage(sessionId: string, prompt: string): Promise<boolean> {
    const request: SendMessageRequest = { session_id: sessionId, prompt };
    const result = await invoke<CommandResponse<boolean>>('send_message', { request });
    if (!result.success) {
      throw new Error(result.error || 'Failed to send message');
    }
    return result.data ?? true;
  }

  /**
   * Cancel execution in a session
   */
  async cancelExecution(sessionId: string): Promise<boolean> {
    const result = await invoke<CommandResponse<boolean>>('cancel_execution', {
      session_id: sessionId,
    });
    if (!result.success) {
      throw new Error(result.error || 'Failed to cancel execution');
    }
    return result.data ?? true;
  }

  /**
   * Get session history
   */
  async getSessionHistory(sessionId: string): Promise<ClaudeCodeSession> {
    const result = await invoke<CommandResponse<ClaudeCodeSession>>('get_session_history', {
      session_id: sessionId,
    });
    if (!result.success || !result.data) {
      throw new Error(result.error || 'Failed to get session history');
    }
    return result.data;
  }

  /**
   * List all active sessions
   */
  async listActiveSessions(): Promise<ActiveSessionInfo[]> {
    const result = await invoke<CommandResponse<ActiveSessionInfo[]>>('list_active_sessions');
    if (!result.success) {
      throw new Error(result.error || 'Failed to list sessions');
    }
    return result.data ?? [];
  }

  /**
   * Get information about a specific session
   */
  async getSessionInfo(sessionId: string): Promise<ActiveSessionInfo> {
    const result = await invoke<CommandResponse<ActiveSessionInfo>>('get_session_info', {
      session_id: sessionId,
    });
    if (!result.success || !result.data) {
      throw new Error(result.error || 'Failed to get session info');
    }
    return result.data;
  }

  /**
   * Remove a session
   */
  async removeSession(sessionId: string): Promise<boolean> {
    const result = await invoke<CommandResponse<boolean>>('remove_session', {
      session_id: sessionId,
    });
    if (!result.success) {
      throw new Error(result.error || 'Failed to remove session');
    }
    return result.data ?? true;
  }
}

// ============================================================================
// Singleton Instance
// ============================================================================

let clientInstance: ClaudeCodeClient | null = null;

/**
 * Get the Claude Code client singleton
 */
export function getClaudeCodeClient(): ClaudeCodeClient {
  if (!clientInstance) {
    clientInstance = new ClaudeCodeClient();
  }
  return clientInstance;
}

/**
 * Initialize Claude Code client connection
 */
export async function initClaudeCodeClient(): Promise<ClaudeCodeClient> {
  const client = getClaudeCodeClient();
  await client.connect();
  return client;
}

/**
 * Close Claude Code client connection
 */
export async function closeClaudeCodeClient(): Promise<void> {
  if (clientInstance) {
    await clientInstance.disconnect();
  }
}

export default ClaudeCodeClient;
