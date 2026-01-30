/**
 * WebSocket Client
 *
 * Manages WebSocket connection to the Plan Cascade server.
 * Handles automatic reconnection with exponential backoff.
 */

const WS_URL = 'ws://127.0.0.1:8765/ws';

// ============================================================================
// Types
// ============================================================================

export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'reconnecting';

export interface WebSocketMessage {
  type: string;
  data?: Record<string, unknown>;
  timestamp?: string;
}

// Event types from the server
export type ServerEventType =
  // Execution lifecycle
  | 'execution_started'
  | 'execution_completed'
  | 'execution_failed'
  | 'execution_cancelled'
  | 'execution_paused'
  | 'execution_resumed'
  | 'execution_update'
  // Strategy events
  | 'strategy_decided'
  // Batch events
  | 'batch_started'
  | 'batch_completed'
  | 'batch_failed'
  // Story events
  | 'story_started'
  | 'story_progress'
  | 'story_completed'
  | 'story_failed'
  | 'story_update'
  // Quality gate events
  | 'quality_gate_started'
  | 'quality_gate_passed'
  | 'quality_gate_failed'
  // Retry events
  | 'retry_started'
  // PRD events
  | 'prd_generated'
  | 'prd_approved'
  | 'prd_updated'
  // Log events
  | 'log_entry'
  // Claude Code events
  | 'claude_code_response'
  | 'claude_code_complete'
  | 'claude_code_tool_call'
  | 'claude_code_tool_result'
  | 'claude_code_error'
  // Connection events
  | 'connected'
  | 'ping'
  | 'pong'
  | 'error';

export type EventHandler = (data: Record<string, unknown>) => void;

// ============================================================================
// WebSocket Manager
// ============================================================================

export class WebSocketManager {
  private ws: WebSocket | null = null;
  private url: string;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private baseReconnectDelay = 1000;
  private maxReconnectDelay = 30000;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private pingInterval: ReturnType<typeof setInterval> | null = null;
  private eventHandlers: Map<string, Set<EventHandler>> = new Map();
  private statusHandlers: Set<(status: ConnectionStatus) => void> = new Set();
  private _status: ConnectionStatus = 'disconnected';

  constructor(url: string = WS_URL) {
    this.url = url;
  }

  /**
   * Get current connection status
   */
  get status(): ConnectionStatus {
    return this._status;
  }

  /**
   * Check if connected
   */
  get isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  /**
   * Connect to the WebSocket server
   */
  connect(): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      return;
    }

    this.setStatus('connecting');
    this.ws = new WebSocket(this.url);

    this.ws.onopen = () => {
      this.reconnectAttempts = 0;
      this.setStatus('connected');
      this.startPingInterval();
    };

    this.ws.onmessage = (event) => {
      try {
        const message: WebSocketMessage = JSON.parse(event.data);
        this.handleMessage(message);
      } catch (error) {
        console.error('Failed to parse WebSocket message:', error);
      }
    };

    this.ws.onclose = (event) => {
      this.stopPingInterval();

      if (!event.wasClean) {
        this.scheduleReconnect();
      } else {
        this.setStatus('disconnected');
      }
    };

    this.ws.onerror = (error) => {
      console.error('WebSocket error:', error);
    };
  }

  /**
   * Disconnect from the WebSocket server
   */
  disconnect(): void {
    this.stopPingInterval();
    this.cancelReconnect();

    if (this.ws) {
      this.ws.close(1000, 'Client disconnect');
      this.ws = null;
    }

    this.setStatus('disconnected');
  }

  /**
   * Subscribe to an event type
   */
  on(eventType: ServerEventType | '*', handler: EventHandler): () => void {
    if (!this.eventHandlers.has(eventType)) {
      this.eventHandlers.set(eventType, new Set());
    }
    this.eventHandlers.get(eventType)!.add(handler);

    // Return unsubscribe function
    return () => {
      this.eventHandlers.get(eventType)?.delete(handler);
    };
  }

  /**
   * Unsubscribe from an event type
   */
  off(eventType: ServerEventType | '*', handler: EventHandler): void {
    this.eventHandlers.get(eventType)?.delete(handler);
  }

  /**
   * Subscribe to connection status changes
   */
  onStatusChange(handler: (status: ConnectionStatus) => void): () => void {
    this.statusHandlers.add(handler);
    // Immediately call with current status
    handler(this._status);

    // Return unsubscribe function
    return () => {
      this.statusHandlers.delete(handler);
    };
  }

  /**
   * Send a message to the server
   */
  send(message: WebSocketMessage): boolean {
    if (this.ws?.readyState !== WebSocket.OPEN) {
      console.warn('WebSocket is not connected');
      return false;
    }

    try {
      this.ws.send(JSON.stringify(message));
      return true;
    } catch (error) {
      console.error('Failed to send WebSocket message:', error);
      return false;
    }
  }

  /**
   * Send a ping to keep the connection alive
   */
  ping(): void {
    this.send({ type: 'ping' });
  }

  /**
   * Request current status from server
   */
  requestStatus(): void {
    this.send({ type: 'get_status' });
  }

  // --------------------------------------------------------------------------
  // Private Methods
  // --------------------------------------------------------------------------

  private setStatus(status: ConnectionStatus): void {
    this._status = status;
    this.statusHandlers.forEach((handler) => handler(status));
  }

  private handleMessage(message: WebSocketMessage): void {
    const { type, data } = message;

    // Handle pong
    if (type === 'pong') {
      return;
    }

    // Emit to specific event handlers
    const handlers = this.eventHandlers.get(type);
    if (handlers) {
      handlers.forEach((handler) => handler(data || {}));
    }

    // Emit to wildcard handlers
    const wildcardHandlers = this.eventHandlers.get('*');
    if (wildcardHandlers) {
      wildcardHandlers.forEach((handler) => handler({ type, ...data }));
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error('Max reconnection attempts reached');
      this.setStatus('disconnected');
      return;
    }

    this.setStatus('reconnecting');

    // Calculate delay with exponential backoff
    const delay = Math.min(
      this.baseReconnectDelay * Math.pow(2, this.reconnectAttempts),
      this.maxReconnectDelay
    );

    this.reconnectAttempts++;

    console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

    this.reconnectTimer = setTimeout(() => {
      this.connect();
    }, delay);
  }

  private cancelReconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.reconnectAttempts = 0;
  }

  private startPingInterval(): void {
    // Send ping every 25 seconds to keep connection alive
    this.pingInterval = setInterval(() => {
      this.ping();
    }, 25000);
  }

  private stopPingInterval(): void {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
  }
}

// ============================================================================
// Singleton Instance
// ============================================================================

let wsManager: WebSocketManager | null = null;

/**
 * Get the WebSocket manager singleton
 */
export function getWebSocketManager(): WebSocketManager {
  if (!wsManager) {
    wsManager = new WebSocketManager();
  }
  return wsManager;
}

/**
 * Initialize WebSocket connection
 */
export function initWebSocket(): WebSocketManager {
  const manager = getWebSocketManager();
  manager.connect();
  return manager;
}

/**
 * Close WebSocket connection
 */
export function closeWebSocket(): void {
  wsManager?.disconnect();
}

export default WebSocketManager;
