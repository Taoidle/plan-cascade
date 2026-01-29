/**
 * Library Module Exports
 *
 * Central export point for API and WebSocket utilities.
 */

export { api, ApiError } from './api';
export type {
  ExecuteRequest,
  ExecuteResponse,
  PRD,
  PRDStory,
  PRDRequest,
  PRDResponse,
  StatusResponse,
  StoryStatus,
  HealthResponse,
  AnalyzeResponse,
} from './api';

export {
  WebSocketManager,
  getWebSocketManager,
  initWebSocket,
  closeWebSocket,
} from './websocket';
export type {
  ConnectionStatus,
  WebSocketMessage,
  ServerEventType,
  EventHandler,
} from './websocket';
