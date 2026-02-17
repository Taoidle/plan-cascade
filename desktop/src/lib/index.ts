/**
 * Library Module Exports (v5.0 Pure Rust Backend)
 *
 * Central export point for API and Tauri utilities.
 */

// Legacy API (deprecated - use Tauri commands)
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

// Deprecated WebSocket (kept for backwards compatibility)
export {
  WebSocketManager,
  getWebSocketManager,
  initWebSocket,
  closeWebSocket,
} from './websocket';
export type {
  ConnectionStatus,
  ServerEventType,
  EventHandler,
} from './websocket';

// New v5.0 Tauri-based Claude Code client
export {
  ClaudeCodeClient,
  getClaudeCodeClient,
  initClaudeCodeClient,
  closeClaudeCodeClient,
  EventChannels,
} from './claudeCodeClient';
export type {
  ClaudeCodeSession,
  ActiveSessionInfo,
  StartChatRequest,
  StartChatResponse,
  SendMessageRequest,
  StreamEventPayload,
  ThinkingUpdateEvent,
  ToolUpdateEvent,
  SessionUpdateEvent,
  UnifiedStreamEvent,
  ThinkingBlock,
  ToolExecution,
  SessionState,
  StreamEventHandler,
  ThinkingEventHandler,
  ToolEventHandler,
  SessionEventHandler,
  StatusHandler,
} from './claudeCodeClient';

// Settings API (v5.0)
export {
  getSettings,
  updateSettings,
  exportSettingsToJson,
  parseImportedSettings,
  isTauriAvailable,
} from './settingsApi';
export type {
  AppConfig,
  SettingsUpdate,
} from './settingsApi';

// Embedding API (v5.0)
export {
  getEmbeddingConfig,
  setEmbeddingConfig,
  listEmbeddingProviders,
  checkEmbeddingProviderHealth,
  setEmbeddingApiKey,
} from './embeddingApi';

// Tauri utilities
export {
  getHealth,
  isTauri,
} from './tauri';
