/**
 * WebSocket Client (DEPRECATED - v5.0 Pure Rust Backend)
 *
 * This file is deprecated. The v5.0 architecture uses Tauri IPC instead of WebSocket.
 *
 * For Claude Code functionality, use:
 * - import { ... } from './claudeCodeClient'
 *
 * For execution functionality, use:
 * - The execution store uses Tauri events directly
 *
 * @deprecated Use claudeCodeClient.ts instead
 */

// Re-export types for backwards compatibility
export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'reconnecting';

// Deprecated - do not use
export class WebSocketManager {
  private _status: ConnectionStatus = 'disconnected';

  get status(): ConnectionStatus {
    console.warn('WebSocketManager is deprecated. Use ClaudeCodeClient from claudeCodeClient.ts');
    return this._status;
  }

  get isConnected(): boolean {
    return false;
  }

  connect(): void {
    console.warn('WebSocketManager.connect() is deprecated. Use initClaudeCodeClient() instead');
  }

  disconnect(): void {
    console.warn('WebSocketManager.disconnect() is deprecated. Use closeClaudeCodeClient() instead');
  }

  on(_eventType: string, _handler: (data: Record<string, unknown>) => void): () => void {
    console.warn('WebSocketManager.on() is deprecated. Use ClaudeCodeClient event methods instead');
    return () => {};
  }

  off(_eventType: string, _handler: (data: Record<string, unknown>) => void): void {
    console.warn('WebSocketManager.off() is deprecated');
  }

  onStatusChange(handler: (status: ConnectionStatus) => void): () => void {
    console.warn('WebSocketManager.onStatusChange() is deprecated. Use ClaudeCodeClient.onStatusChange() instead');
    handler('disconnected');
    return () => {};
  }

  send(_message: { type: string; data?: Record<string, unknown> }): boolean {
    console.warn('WebSocketManager.send() is deprecated. Use ClaudeCodeClient methods instead');
    return false;
  }

  ping(): void {}
  requestStatus(): void {}
}

// Deprecated singleton
let wsManager: WebSocketManager | null = null;

export function getWebSocketManager(): WebSocketManager {
  console.warn('getWebSocketManager() is deprecated. Use getClaudeCodeClient() instead');
  if (!wsManager) {
    wsManager = new WebSocketManager();
  }
  return wsManager;
}

export function initWebSocket(): WebSocketManager {
  console.warn('initWebSocket() is deprecated. Use initClaudeCodeClient() instead');
  return getWebSocketManager();
}

export function closeWebSocket(): void {
  console.warn('closeWebSocket() is deprecated. Use closeClaudeCodeClient() instead');
}

// Re-export event types for compatibility
export type ServerEventType = string;
export type EventHandler = (data: Record<string, unknown>) => void;

export default WebSocketManager;
