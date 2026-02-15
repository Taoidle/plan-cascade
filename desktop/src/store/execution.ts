/**
 * Execution Store (v5.0 Pure Rust Backend)
 *
 * Manages task execution state with real-time updates from Tauri events.
 * Replaces the legacy WebSocket-based approach with Tauri IPC.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useSettingsStore } from './settings';
import { useModeStore } from './mode';
import type { StreamEventPayload } from '../lib/claudeCodeClient';
import { ToolCallStreamFilter } from '../utils/toolCallFilter';
import type { FileAttachmentData } from '../types/attachment';
import { buildPromptWithAttachments } from '../lib/conversationUtils';

export type ExecutionStatus = 'idle' | 'running' | 'paused' | 'completed' | 'failed';

export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'reconnecting';

export type Strategy = 'direct' | 'hybrid_auto' | 'hybrid_worktree' | 'mega_plan' | null;

/** Dimension scores from strategy analysis (0.0 - 1.0 each) */
export interface DimensionScores {
  scope: number;
  complexity: number;
  risk: number;
  parallelization: number;
}

/** Result of automatic strategy analysis from the Rust backend */
export interface StrategyAnalysis {
  strategy: string;
  confidence: number;
  reasoning: string;
  estimated_stories: number;
  estimated_features: number;
  estimated_duration_hours: number;
  complexity_indicators: string[];
  recommendations: string[];
  dimension_scores: DimensionScores;
}

/** A strategy option returned by get_strategy_options */
export interface StrategyOptionInfo {
  value: string;
  label: string;
  description: string;
  min_stories: number;
  max_stories: number;
}

export interface Story {
  id: string;
  title: string;
  description?: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  progress: number;
  error?: string;
  startedAt?: string;
  completedAt?: string;
  retryCount?: number;
}

// ============================================================================
// Streaming Output Types
// ============================================================================

export type StreamLineType =
  | 'text'
  | 'info'
  | 'error'
  | 'success'
  | 'warning'
  | 'tool'
  | 'tool_result'
  | 'sub_agent'
  | 'analysis'
  | 'thinking'
  | 'code';

export interface StreamLine {
  id: number;
  content: string;
  type: StreamLineType;
  timestamp: number;
}

export interface HistoryConversationLine {
  type: StreamLineType;
  content: string;
}

export interface AnalysisCoverageSnapshot {
  runId?: string;
  status: 'idle' | 'running' | 'completed' | 'failed';
  successfulPhases: number;
  partialPhases: number;
  failedPhases: number;
  observedPaths: number;
  inventoryTotalFiles: number;
  sampledReadFiles: number;
  testFilesTotal: number;
  testFilesRead: number;
  coverageRatio: number;
  sampledReadRatio: number;
  testCoverageRatio: number;
  observedTestCoverageRatio: number;
  coverageTargetRatio?: number;
  sampledReadTargetRatio?: number;
  testCoverageTargetRatio?: number;
  validationIssues: string[];
  manifestPath?: string;
  reportPath?: string;
  updatedAt: number;
}

// ============================================================================
// Quality Gate Result Types
// ============================================================================

export type QualityGateStatus = 'pending' | 'running' | 'passed' | 'failed';

export interface QualityGateResult {
  gateId: string;
  gateName: string;
  storyId: string;
  status: QualityGateStatus;
  output?: string;
  duration?: number;
  startedAt?: number;
  completedAt?: number;
}

// ============================================================================
// Error State Types
// ============================================================================

export type ErrorSeverity = 'warning' | 'error' | 'critical';

export interface ExecutionError {
  id: string;
  storyId?: string;
  severity: ErrorSeverity;
  title: string;
  description: string;
  suggestedFix?: string;
  stackTrace?: string;
  timestamp: number;
  dismissed: boolean;
}

export interface Batch {
  batchNum: number;
  totalBatches: number;
  storyIds: string[];
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  startedAt?: string;
  completedAt?: string;
}

export interface ExecutionResult {
  success: boolean;
  message: string;
  completedStories: number;
  totalStories: number;
  duration: number;
  error?: string;
}

export interface ExecutionHistoryItem {
  id: string;
  title?: string;
  taskDescription: string;
  workspacePath?: string | null;
  strategy: Strategy;
  status: ExecutionStatus;
  startedAt: number;
  completedAt?: number;
  duration: number;
  completedStories: number;
  totalStories: number;
  success: boolean;
  error?: string;
  /** Serialized conversation content from streamingOutput */
  conversationContent?: string;
  /** Structured conversation lines for lossless restore */
  conversationLines?: HistoryConversationLine[];
  /** Session ID for potential reconnection */
  sessionId?: string;
}

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

interface StandaloneTurn {
  user: string;
  assistant: string;
  createdAt: number;
}

interface LegacyTaskStartData {
  task_id: string;
}

interface BackendUsageStats {
  input_tokens: number;
  output_tokens: number;
  thinking_tokens?: number | null;
  cache_read_tokens?: number | null;
  cache_creation_tokens?: number | null;
}

interface BackendStandaloneExecutionResult {
  response?: string | null;
  usage: BackendUsageStats;
  iterations: number;
  success: boolean;
  error?: string | null;
}

interface ExecutionState {
  /** Current execution status */
  status: ExecutionStatus;

  /** Backend connection status (always connected in Tauri) */
  connectionStatus: ConnectionStatus;

  /** Task ID from server */
  taskId: string | null;

  /** Task description */
  taskDescription: string;

  /** Selected strategy */
  strategy: Strategy;

  /** List of stories */
  stories: Story[];

  /** List of batches */
  batches: Batch[];

  /** Current batch number */
  currentBatch: number;

  /** Currently executing story ID */
  currentStoryId: string | null;

  /** Overall progress (0-100) */
  progress: number;

  /** Execution result */
  result: ExecutionResult | null;

  /** Start timestamp */
  startedAt: number | null;

  /** Execution logs */
  logs: string[];

  /** Execution history */
  history: ExecutionHistoryItem[];

  /** Is submitting (API call in progress) */
  isSubmitting: boolean;

  /** API error message */
  apiError: string | null;

  /** Strategy analysis result from auto-analyzer */
  strategyAnalysis: StrategyAnalysis | null;

  /** Whether strategy analysis is in progress */
  isAnalyzingStrategy: boolean;

  /** Available strategy options metadata */
  strategyOptions: StrategyOptionInfo[];

  /** Streaming output buffer for real-time display */
  streamingOutput: StreamLine[];

  /** Counter for unique stream line IDs */
  streamLineCounter: number;

  /** Line ID at the start of the current turn (for scoping text_replace) */
  currentTurnStartLineId: number;

  /** Structured analysis coverage snapshot for Simple mode visualization */
  analysisCoverage: AnalysisCoverageSnapshot | null;

  /** Quality gate results per story */
  qualityGateResults: QualityGateResult[];

  /** Actionable error states */
  executionErrors: ExecutionError[];

  /** Estimated time remaining in milliseconds */
  estimatedTimeRemaining: number | null;

  /** Whether we're in an active Claude Code chat session (supports multi-turn) */
  isChatSession: boolean;

  /** Local multi-turn context for standalone providers (glm/openai/deepseek/qwen/ollama) */
  standaloneTurns: StandaloneTurn[];

  /** Session identifier for standalone conversation-scoped analysis reuse */
  standaloneSessionId: string | null;

  /** Last usage payload reported by backend for current turn */
  latestUsage: BackendUsageStats | null;

  /** Accumulated token usage for current chat session */
  sessionUsageTotals: BackendUsageStats | null;

  /** Filter for stripping tool_call code blocks from streaming text */
  toolCallFilter: ToolCallStreamFilter;

  /** File attachments pending to be sent with the next message */
  attachments: FileAttachmentData[];

  // Actions
  /** Add a file attachment */
  addAttachment: (file: FileAttachmentData) => void;

  /** Remove a file attachment by ID */
  removeAttachment: (id: string) => void;

  /** Clear all file attachments */
  clearAttachments: () => void;
  /** Initialize Tauri event listeners */
  initialize: () => void;

  /** Cleanup event listeners */
  cleanup: () => void;

  /** Start execution */
  start: (description: string, mode: 'simple' | 'expert') => Promise<void>;

  /** Pause execution */
  pause: () => Promise<void>;

  /** Resume execution */
  resume: () => Promise<void>;

  /** Cancel execution */
  cancel: () => Promise<void>;

  /** Send a follow-up message in an existing Claude Code chat session */
  sendFollowUp: (prompt: string) => Promise<void>;

  /** Reset state */
  reset: () => void;

  /** Update story status */
  updateStory: (storyId: string, updates: Partial<Story>) => void;

  /** Add log entry */
  addLog: (message: string) => void;

  /** Set stories from server */
  setStories: (stories: Story[]) => void;

  /** Set strategy */
  setStrategy: (strategy: Strategy) => void;

  /** Load history from localStorage */
  loadHistory: () => void;

  /** Save to history */
  saveToHistory: () => void;

  /** Clear history */
  clearHistory: () => void;

  /** Delete a single history item */
  deleteHistory: (historyId: string) => void;

  /** Rename a history item */
  renameHistory: (historyId: string, title: string) => void;

  /** Restore a conversation from history into the streaming output view */
  restoreFromHistory: (historyId: string) => void;

  /** Analyze task strategy via Rust backend */
  analyzeStrategy: (description: string) => Promise<StrategyAnalysis | null>;

  /** Load available strategy options */
  loadStrategyOptions: () => Promise<void>;

  /** Clear strategy analysis */
  clearStrategyAnalysis: () => void;

  /** Append a streaming output line */
  appendStreamLine: (content: string, type: StreamLineType) => void;

  /** Clear streaming output buffer */
  clearStreamingOutput: () => void;

  /** Update quality gate result for a story */
  updateQualityGate: (result: QualityGateResult) => void;

  /** Add an execution error */
  addExecutionError: (error: Omit<ExecutionError, 'id' | 'timestamp' | 'dismissed'>) => void;

  /** Dismiss an execution error */
  dismissError: (errorId: string) => void;

  /** Clear all execution errors */
  clearExecutionErrors: () => void;

  /** Retry a failed story */
  retryStory: (storyId: string) => Promise<void>;
}

const HISTORY_KEY = 'plan-cascade-execution-history';
const MAX_HISTORY_ITEMS = 10;
const DEFAULT_STANDALONE_CONTEXT_TURNS = 8;
const STANDALONE_CONTEXT_UNLIMITED = -1;
const LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY = 'plan-cascade-provider-api-key-cache';

const PROVIDER_ALIASES: Record<string, string> = {
  anthropic: 'anthropic',
  claude: 'anthropic',
  'claude-api': 'anthropic',
  openai: 'openai',
  deepseek: 'deepseek',
  glm: 'glm',
  'glm-api': 'glm',
  zhipu: 'glm',
  zhipuai: 'glm',
  qwen: 'qwen',
  'qwen-api': 'qwen',
  dashscope: 'qwen',
  alibaba: 'qwen',
  aliyun: 'qwen',
  minimax: 'minimax',
  'minimax-api': 'minimax',
  ollama: 'ollama',
};

function normalizeProviderName(value: string | null | undefined): string | null {
  if (!value) return null;
  const normalized = value.trim().toLowerCase();
  return PROVIDER_ALIASES[normalized] || null;
}

function isClaudeCodeBackend(value: string | null | undefined): boolean {
  if (!value) return false;
  const normalized = value.trim().toLowerCase();
  return normalized === 'claude-code' || normalized === 'claude_code' || normalized === 'claudecode';
}

function inferProviderFromModel(model: string | null | undefined): string | null {
  if (!model) return null;
  const normalized = model.trim().toLowerCase();
  if (!normalized) return null;

  if (normalized.includes('glm')) return 'glm';
  if (normalized.includes('qwen') || normalized.includes('qwq')) return 'qwen';
  if (normalized.includes('deepseek')) return 'deepseek';
  if (normalized.includes('minimax')) return 'minimax';
  if (normalized.includes('claude')) return 'anthropic';
  if (normalized.startsWith('gpt') || normalized.startsWith('o1') || normalized.startsWith('o3')) return 'openai';
  return null;
}

function resolveStandaloneProvider(
  rawBackend: string | null | undefined,
  rawProvider: string | null | undefined,
  rawModel: string | null | undefined
): string {
  const backendCandidate = normalizeProviderName(rawBackend);
  const providerCandidate = normalizeProviderName(rawProvider);
  const modelCandidate = inferProviderFromModel(rawModel);

  // When backend/provider conflict, trust model hint first, then provider setting.
  if (backendCandidate && providerCandidate && backendCandidate !== providerCandidate) {
    if (modelCandidate === providerCandidate) return providerCandidate;
    if (modelCandidate === backendCandidate) return backendCandidate;
    return providerCandidate;
  }

  return backendCandidate || providerCandidate || modelCandidate || 'anthropic';
}

const GLM_CODING_BASE_URL = 'https://open.bigmodel.cn/api/coding/paas/v4/chat/completions';
const MINIMAX_CHINA_BASE_URL = 'https://api.minimaxi.com/v1/chat/completions';

/** Default model per provider, used when user selects "Provider default". */
const DEFAULT_MODEL_BY_PROVIDER: Record<string, string> = {
  anthropic: 'claude-sonnet-4-20250514',
  openai: 'gpt-4o',
  deepseek: 'deepseek-chat',
  glm: 'glm-4-flash-250414',
  qwen: 'qwen-plus',
  minimax: 'MiniMax-M2.5',
  ollama: 'llama3.2',
};

/**
 * Resolve provider-specific base URL override from user settings.
 * GLM has standard/coding endpoints; MiniMax has international/china endpoints.
 */
function resolveProviderBaseUrl(
  provider: string,
  settings: { glmEndpoint?: string; minimaxEndpoint?: string },
): string | undefined {
  const normalized = normalizeProviderName(provider);
  if (normalized === 'glm' && settings.glmEndpoint === 'coding') {
    return GLM_CODING_BASE_URL;
  }
  if (normalized === 'minimax' && settings.minimaxEndpoint === 'china') {
    return MINIMAX_CHINA_BASE_URL;
  }
  return undefined;
}

function getLocalProviderApiKey(provider: string): string | undefined {
  try {
    if (typeof localStorage === 'undefined') return undefined;
    const raw = localStorage.getItem(LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY);
    if (!raw) return undefined;
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const normalizedProvider = normalizeProviderName(provider) || provider.trim().toLowerCase();
    const value = parsed[normalizedProvider];
    if (typeof value !== 'string') return undefined;
    const trimmed = value.trim();
    return trimmed || undefined;
  } catch {
    return undefined;
  }
}

function getStandaloneContextTurnsLimit(): number {
  const rawValue = (useSettingsStore.getState() as { standaloneContextTurns?: unknown }).standaloneContextTurns;
  const value = Number(rawValue);
  if (value === STANDALONE_CONTEXT_UNLIMITED) return STANDALONE_CONTEXT_UNLIMITED;
  if (Number.isFinite(value) && value > 0) return Math.floor(value);
  return DEFAULT_STANDALONE_CONTEXT_TURNS;
}

function trimStandaloneTurns(turns: StandaloneTurn[], limit: number): StandaloneTurn[] {
  if (limit === STANDALONE_CONTEXT_UNLIMITED) return turns;
  return turns.slice(-limit);
}

function buildStandaloneConversationMessage(turns: StandaloneTurn[], userInput: string, contextTurnsLimit: number): string {
  const recent = trimStandaloneTurns(turns, contextTurnsLimit);
  const history = recent
    .map((turn) => `User: ${turn.user}\nAssistant: ${turn.assistant}`)
    .join('\n\n');

  return [
    'Continue the same conversation. Keep consistency with previous context.',
    '',
    'Conversation history:',
    history,
    '',
    `User: ${userInput}`,
  ].join('\n');
}

function createStandaloneSessionId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `simple-${crypto.randomUUID()}`;
  }
  return `simple-${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;
}

function buildHistorySessionId(taskId: string | null, standaloneSessionId: string | null): string | null {
  if (taskId && taskId.trim().length > 0) {
    return `claude:${taskId.trim()}`;
  }
  if (standaloneSessionId && standaloneSessionId.trim().length > 0) {
    return `standalone:${standaloneSessionId.trim()}`;
  }
  return null;
}

function hasAssistantTextLineSince(lines: StreamLine[], minExclusiveLineId: number): boolean {
  return lines.some(
    (line) =>
      line.id > minExclusiveLineId &&
      line.type === 'text' &&
      line.content.trim().length > 0
  );
}

function collectAssistantTextSince(lines: StreamLine[], minExclusiveLineId: number): string {
  return lines
    .filter(
      (line) =>
        line.id > minExclusiveLineId &&
        line.type === 'text' &&
        line.content.trim().length > 0
    )
    .map((line) => line.content)
    .join('')
    .trim();
}

function appendTextWithTypewriter(
  append: (content: string, type: StreamLineType) => void,
  content: string,
  chunkSize = 28,
  delayMs = 14
): Promise<void> {
  const text = content.trim();
  if (!text) return Promise.resolve();
  // Avoid very long UI replays blocking finalization; stream large payloads as a single line.
  if (text.length > 4000) {
    append(text, 'text');
    return Promise.resolve();
  }

  return new Promise((resolve) => {
    let cursor = 0;
    const pushChunk = () => {
      const next = text.slice(cursor, cursor + chunkSize);
      if (!next) {
        resolve();
        return;
      }
      append(next, 'text');
      cursor += next.length;
      if (cursor >= text.length) {
        resolve();
        return;
      }
      globalThis.setTimeout(pushChunk, delayMs);
    };
    pushChunk();
  });
}

/** Format tool arguments for display in a human-readable way */
function formatToolArgs(toolName: string, rawArgs?: string): string {
  if (!rawArgs) return '';
  try {
    const args = JSON.parse(rawArgs) as Record<string, unknown>;
    switch (toolName) {
      case 'Read':
        return String(args.file_path || '');
      case 'Write':
        return String(args.file_path || '');
      case 'Edit':
        return String(args.file_path || '');
      case 'Bash':
        return String(args.command || '').substring(0, 120);
      case 'Glob':
        return `${args.pattern || ''}${args.path ? ` in ${args.path}` : ''}`;
      case 'Grep':
        return `/${args.pattern || ''}/${args.path ? ` in ${args.path}` : ''}`;
      case 'LS':
        return String(args.path || '');
      case 'Cwd':
        return '';
      case 'Task':
        return String(args.prompt || '').substring(0, 120);
      default: {
        const compact = JSON.stringify(args);
        return compact.length > 120 ? compact.substring(0, 120) + '...' : compact;
      }
    }
  } catch {
    return rawArgs.length > 120 ? rawArgs.substring(0, 120) + '...' : rawArgs;
  }
}

function isLegacyTaskStartData(data: unknown): data is LegacyTaskStartData {
  return typeof data === 'object' && data !== null && typeof (data as { task_id?: unknown }).task_id === 'string';
}

function isBackendStandaloneExecutionResult(data: unknown): data is BackendStandaloneExecutionResult {
  if (typeof data !== 'object' || data === null) return false;
  const value = data as { iterations?: unknown; success?: unknown; usage?: unknown };
  return typeof value.iterations === 'number' && typeof value.success === 'boolean' && typeof value.usage === 'object' && value.usage !== null;
}

// Track event unlisteners
let unlisteners: UnlistenFn[] = [];
let listenerSetupVersion = 0;

const initialState = {
  status: 'idle' as ExecutionStatus,
  connectionStatus: 'disconnected' as ConnectionStatus,
  taskId: null as string | null,
  taskDescription: '',
  strategy: null as Strategy,
  stories: [] as Story[],
  batches: [] as Batch[],
  currentBatch: 0,
  currentStoryId: null as string | null,
  progress: 0,
  result: null as ExecutionResult | null,
  startedAt: null as number | null,
  logs: [] as string[],
  history: [] as ExecutionHistoryItem[],
  isSubmitting: false,
  apiError: null as string | null,
  strategyAnalysis: null as StrategyAnalysis | null,
  isAnalyzingStrategy: false,
  strategyOptions: [] as StrategyOptionInfo[],
  streamingOutput: [] as StreamLine[],
  streamLineCounter: 0,
  currentTurnStartLineId: 0,
  analysisCoverage: null as AnalysisCoverageSnapshot | null,
  qualityGateResults: [] as QualityGateResult[],
  executionErrors: [] as ExecutionError[],
  estimatedTimeRemaining: null as number | null,
  isChatSession: false,
  standaloneTurns: [] as StandaloneTurn[],
  standaloneSessionId: null as string | null,
  latestUsage: null as BackendUsageStats | null,
  sessionUsageTotals: null as BackendUsageStats | null,
  toolCallFilter: new ToolCallStreamFilter(),
  attachments: [] as FileAttachmentData[],
};

export const useExecutionStore = create<ExecutionState>()((set, get) => ({
  ...initialState,

  initialize: () => {
    // In Tauri, we're always "connected" via IPC
    set({ connectionStatus: 'connected' });
    get().addLog('Connected to Rust backend');

    // Set up Tauri event listeners for execution updates
    setupTauriEventListeners(get, set);

    // Load history
    get().loadHistory();
  },

  cleanup: () => {
    // Invalidate in-flight async listener setup to prevent duplicate registration.
    listenerSetupVersion++;

    // Clean up all event listeners
    for (const unlisten of unlisteners) {
      unlisten();
    }
    unlisteners = [];
    set({ connectionStatus: 'disconnected' });
  },

  start: async (description, mode) => {
    const settingsSnapshot = useSettingsStore.getState();
    const backendSnapshot = String((settingsSnapshot as { backend?: unknown }).backend || '');
    const isClaudeBackend = isClaudeCodeBackend(backendSnapshot);
    const existingStandaloneTurns = get().standaloneTurns;
    const existingStandaloneSessionId = get().standaloneSessionId;
    const preserveSimpleConversation =
      mode === 'simple' &&
      get().streamingOutput.length > 0 &&
      !isClaudeBackend;
    const nextStandaloneSessionId =
      mode === 'simple' && !isClaudeBackend
        ? preserveSimpleConversation && existingStandaloneSessionId
          ? existingStandaloneSessionId
          : createStandaloneSessionId()
        : null;

    set({
      isSubmitting: true,
      apiError: null,
      status: 'running',
      taskDescription: description,
      startedAt: Date.now(),
      result: null,
      taskId: isClaudeBackend ? get().taskId : null,
      isChatSession: isClaudeBackend ? get().isChatSession : false,
      logs: [],
      stories: [],
      batches: [],
      currentBatch: 0,
      currentStoryId: null,
      progress: 0,
      streamingOutput: preserveSimpleConversation ? get().streamingOutput : [],
      streamLineCounter: preserveSimpleConversation ? get().streamLineCounter : 0,
      analysisCoverage: null,
      qualityGateResults: [],
      executionErrors: [],
      estimatedTimeRemaining: null,
      standaloneSessionId: nextStandaloneSessionId,
      latestUsage: preserveSimpleConversation ? get().latestUsage : null,
      sessionUsageTotals: preserveSimpleConversation ? get().sessionUsageTotals : null,
    });

    // Reset the tool-call filter for the new execution turn
    get().toolCallFilter.reset();

    get().addLog(`Starting execution in ${mode} mode...`);
    get().addLog(`Task: ${description}`);
    if (mode === 'simple' && !isClaudeBackend) {
      // In standalone simple mode, always render the user bubble.
      get().appendStreamLine(description, 'info');
    }

    try {
      // Read user's backend/provider settings
      const settings = settingsSnapshot;
      const backendValue = String((settings as { backend?: unknown }).backend || '');
      const providerValue = String((settings as { provider?: unknown }).provider || '');
      const modelValue = String((settings as { model?: unknown }).model || '');

      if (isClaudeCodeBackend(backendValue)) {
        // Use Claude Code CLI backend via start_chat + send_message
        const projectPath = settings.workspacePath || '.';
        const startResult = await invoke<CommandResponse<{ session_id: string }>>('start_chat', {
          request: { project_path: projectPath },
        });

        if (!startResult.success || !startResult.data) {
          throw new Error(startResult.error || 'Failed to start Claude Code session');
        }

        const sessionId = startResult.data.session_id;
        set({ taskId: sessionId, isSubmitting: false, isChatSession: true });
        get().addLog(`Claude Code session started: ${sessionId}`);

        // Show user's message in the conversation
        get().appendStreamLine(description, 'info');

        // Enrich prompt with file attachments if any
        const claudeAttachments = get().attachments;
        const claudePrompt = claudeAttachments.length > 0
          ? buildPromptWithAttachments(description, claudeAttachments)
          : description;
        get().clearAttachments();

        // Send the message to the session
        await invoke('send_message', {
          request: { session_id: sessionId, prompt: claudePrompt },
        });
      } else {
        // Use standalone LLM execution
        const provider = resolveStandaloneProvider(backendValue, providerValue, modelValue);
        const model = settings.model || DEFAULT_MODEL_BY_PROVIDER[provider] || 'claude-sonnet-4-20250514';
        const providerApiKey = getLocalProviderApiKey(provider);
        const isSimpleStandalone = mode === 'simple';
        const turnStartLineId = get().streamLineCounter;
        set({ currentTurnStartLineId: turnStartLineId });
        const standaloneSessionId = get().standaloneSessionId;
        const contextTurnsLimit = getStandaloneContextTurnsLimit();
        const recentStandaloneTurns = trimStandaloneTurns(existingStandaloneTurns, contextTurnsLimit);
        const messageToSend =
          isSimpleStandalone && recentStandaloneTurns.length > 0
            ? buildStandaloneConversationMessage(existingStandaloneTurns, description, contextTurnsLimit)
            : description;
        get().addLog(
          `Resolved provider: ${provider} (backend=${backendValue || 'empty'}, setting=${providerValue || 'empty'}, model=${modelValue || 'empty'})`
        );
        if (isSimpleStandalone && recentStandaloneTurns.length > 0) {
          const contextLabel =
            contextTurnsLimit === STANDALONE_CONTEXT_UNLIMITED ? 'unlimited' : String(contextTurnsLimit);
          get().addLog(`Using standalone conversation context (${recentStandaloneTurns.length}/${contextLabel} turns)`);
        }

        // Enrich prompt with file attachments if any
        const standaloneAttachments = get().attachments;
        const enrichedMessage = standaloneAttachments.length > 0
          ? buildPromptWithAttachments(messageToSend, standaloneAttachments)
          : messageToSend;
        get().clearAttachments();

        // Resolve provider-specific base URL override (e.g. GLM Coding endpoint)
        const baseUrl = resolveProviderBaseUrl(provider, settings);

        const result = await invoke<CommandResponse<unknown>>('execute_standalone', {
          message: enrichedMessage,
          provider,
          model,
          projectPath: settings.workspacePath || '.',
          enableTools: true,
          apiKey: providerApiKey,
          baseUrl,
          analysisSessionId: standaloneSessionId,
          enableCompaction: settings.enableContextCompaction ?? true,
          enableThinking: settings.enableThinking ?? false,
          maxTotalTokens: settings.maxTotalTokens ?? undefined,
          maxIterations: settings.maxIterations ?? undefined,
        });

        if (!result.success || !result.data) {
          throw new Error(result.error || 'Failed to start execution');
        }

        // Legacy async start contract (kept for backward compatibility).
        if (isLegacyTaskStartData(result.data)) {
          set({
            taskId: result.data.task_id,
            isSubmitting: false,
          });
          get().addLog(`Task started with ID: ${result.data.task_id}`);
          return;
        }

        // Current standalone contract: command returns final execution result.
        if (isBackendStandaloneExecutionResult(result.data)) {
          const execution = result.data;
          const assistantResponse = execution.response?.trim() || '';
          const streamedAssistantText = collectAssistantTextSince(get().streamingOutput, turnStartLineId);
          const assistantTurnText = assistantResponse || streamedAssistantText;

          if (mode === 'simple' && assistantTurnText) {
            const retentionLimit = getStandaloneContextTurnsLimit();
            set((state) => ({
              standaloneTurns: trimStandaloneTurns(
                [
                  ...state.standaloneTurns,
                  {
                    user: description,
                    assistant: assistantTurnText,
                    createdAt: Date.now(),
                  },
                ],
                retentionLimit
              ),
            }));
          }

          // Mark submission complete regardless of stream event timing.
          set({ isSubmitting: false });

          let ensuredAssistantOutput = false;
          const ensureAssistantResponseVisible = async () => {
            if (ensuredAssistantOutput) return;

            const hasStreamedAssistantText = hasAssistantTextLineSince(
              get().streamingOutput,
              turnStartLineId
            );
            if (hasStreamedAssistantText) {
              ensuredAssistantOutput = true;
              return;
            }

            if (assistantTurnText) {
              ensuredAssistantOutput = true;
              await appendTextWithTypewriter(
                (chunk, type) => get().appendStreamLine(chunk, type),
                assistantTurnText
              );
              return;
            }

            // Check if thinking lines exist — don't show "no response" if the model did reason
            const hasThinkingLines = get().streamingOutput.some(
              (line) =>
                line.id > turnStartLineId &&
                line.type === 'thinking' &&
                line.content.trim().length > 0
            );
            if (hasThinkingLines) {
              ensuredAssistantOutput = true;
              // Thinking was displayed — no need for a warning
              return;
            }

            if (isSimpleStandalone) {
              ensuredAssistantOutput = true;
              get().appendStreamLine('No assistant response returned for this turn.', 'warning');
            }
          };

          // The invoke() resolves when the Rust command returns, but streaming
          // events are forwarded via a separate async task and may still be in
          // flight.  If we set status immediately, the UI shows "Completed"
          // while tool-call events are still being rendered.
          //
          // Strategy: wait briefly for the streaming `complete` event to arrive
          // and set the status.  Only fall back to setting status from the
          // invoke result if no streaming event did so.
          const finalizeFromInvoke = async () => {
            if (get().status !== 'running') {
              // Stream event already finalized status. Persist completed runs
              // once the invoke() response has been reconciled.
              if (get().status === 'completed') {
                await ensureAssistantResponseVisible();
                get().saveToHistory();
              }
              return;
            }
            const succeeded = execution.success;
            await ensureAssistantResponseVisible();

            const duration = Date.now() - (get().startedAt || Date.now());
            const durationStr = duration >= 60000
              ? `${Math.floor(duration / 60000)}m ${Math.round((duration % 60000) / 1000)}s`
              : `${Math.round(duration / 1000)}s`;

            set({
              taskId: null,
              status: succeeded ? 'completed' : 'failed',
              progress: succeeded ? 100 : get().progress,
              estimatedTimeRemaining: 0,
              apiError: succeeded ? null : (execution.error || 'Execution failed'),
              result: {
                success: succeeded,
                message: succeeded ? 'Execution completed' : 'Execution failed',
                completedStories: succeeded ? 1 : 0,
                totalStories: 1,
                duration,
                error: execution.error || undefined,
              },
            });

            // Append completion/failure banner as a stream line (always ordered correctly)
            if (succeeded) {
              get().appendStreamLine(`Completed (${durationStr})`, 'success');
            } else {
              get().appendStreamLine(
                `Execution finished with failures.${execution.error ? ` ${execution.error}` : ''}`,
                'error'
              );
            }

            if (!succeeded && execution.error) {
              get().addExecutionError({
                severity: 'error',
                title: 'Execution Failed',
                description: execution.error,
                suggestedFix: 'Check API key/model settings and retry.',
              });
            }

            get().addLog(
              succeeded
                ? `Execution completed (iterations: ${execution.iterations})`
                : `Execution failed: ${execution.error || 'Unknown error'}`
            );
            get().saveToHistory();
          };

          if (get().status === 'running') {
            if (get().streamLineCounter > turnStartLineId) {
              // Streaming events were received, so the orchestrator's complete
              // event should arrive via the listener and finalize status.
              // Use a fallback timeout in case the event is lost.
              globalThis.setTimeout(() => {
                void finalizeFromInvoke();
              }, 3000);
            } else {
              // No streaming events at all; finalize immediately.
              void finalizeFromInvoke();
            }
          } else if (get().status === 'completed') {
            void finalizeFromInvoke();
          }
          return;
        }

        throw new Error('Unexpected execute_standalone response shape');
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';

      set({
        status: 'failed',
        isSubmitting: false,
        apiError: errorMessage,
        result: {
          success: false,
          message: 'Failed to start execution',
          completedStories: 0,
          totalStories: 0,
          duration: Date.now() - (get().startedAt || Date.now()),
          error: errorMessage,
        },
      });

      get().addLog(`Error: ${errorMessage}`);
      get().saveToHistory();
    }
  },

  sendFollowUp: async (prompt) => {
    const sessionId = get().taskId;
    if (!sessionId || !get().isChatSession) {
      return;
    }

    // Add user message as a visual separator in the streaming output.
    // Using 'info' type ensures it won't concatenate with text deltas,
    // and the next text_delta will start a fresh text block.
    get().appendStreamLine(prompt, 'info');

    // Reset the tool-call filter for the new follow-up turn
    get().toolCallFilter.reset();

    // Keep existing streaming output (conversation history) and switch to running
    set({
      status: 'running',
      isSubmitting: false,
      apiError: null,
      result: null,
    });

    // Enrich prompt with file attachments if any
    const followUpAttachments = get().attachments;
    const enrichedPrompt = followUpAttachments.length > 0
      ? buildPromptWithAttachments(prompt, followUpAttachments)
      : prompt;
    get().clearAttachments();

    get().addLog(`Follow-up: ${prompt}`);

    try {
      await invoke('send_message', {
        request: { session_id: sessionId, prompt: enrichedPrompt },
      });
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({
        status: 'failed',
        apiError: errorMessage,
      });
      get().addLog(`Error: ${errorMessage}`);
    }
  },

  pause: async () => {
    try {
      // Note: Pause may not be implemented in standalone mode
      set({ status: 'paused' });
      get().addLog('Execution paused');
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({ apiError: errorMessage });
      get().addLog(`Failed to pause: ${errorMessage}`);
    }
  },

  resume: async () => {
    try {
      // Note: Resume may not be implemented in standalone mode
      set({ status: 'running' });
      get().addLog('Execution resumed');
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({ apiError: errorMessage });
      get().addLog(`Failed to resume: ${errorMessage}`);
    }
  },

  cancel: async () => {
    try {
      // Cancel current session if running
      const { taskId } = get();
      if (taskId) {
        try {
          await invoke<CommandResponse<boolean>>('cancel_execution', {
            session_id: taskId,
          });
        } catch {
          // Session might not exist in the new architecture
        }
      }

      set({
        status: 'idle',
        currentStoryId: null,
        result: {
          success: false,
          message: 'Execution cancelled by user',
          completedStories: get().stories.filter((s) => s.status === 'completed').length,
          totalStories: get().stories.length,
          duration: Date.now() - (get().startedAt || Date.now()),
        },
      });
      get().addLog('Execution cancelled');
      get().saveToHistory();
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({ apiError: errorMessage });
      get().addLog(`Failed to cancel: ${errorMessage}`);
    }
  },

  reset: () => {
    // Auto-save conversation to history before clearing
    const state = get();
    if (state.isChatSession && state.streamingOutput.length > 0) {
      get().saveToHistory();
    }

    set({
      ...initialState,
      connectionStatus: state.connectionStatus,
      history: get().history,
      toolCallFilter: new ToolCallStreamFilter(),
    });
  },

  updateStory: (storyId, updates) => {
    set((state) => ({
      stories: state.stories.map((s) =>
        s.id === storyId ? { ...s, ...updates } : s
      ),
    }));

    // Recalculate progress
    const stories = get().stories;
    if (stories.length > 0) {
      const completed = stories.filter((s) => s.status === 'completed').length;
      set({ progress: (completed / stories.length) * 100 });
    }
  },

  addLog: (message) => {
    const timestamp = new Date().toISOString().slice(11, 19);
    set((state) => ({
      logs: [...state.logs, `[${timestamp}] ${message}`],
    }));
  },

  setStories: (stories) => {
    set({ stories });
    get().addLog(`PRD loaded with ${stories.length} stories`);
  },

  setStrategy: (strategy) => {
    set({ strategy });
    get().addLog(`Strategy selected: ${strategy}`);
  },

  loadHistory: () => {
    try {
      const stored = localStorage.getItem(HISTORY_KEY);
      if (stored) {
        const history = JSON.parse(stored) as ExecutionHistoryItem[];
        set({ history });
      }
    } catch {
      // Ignore localStorage errors
    }
  },

  saveToHistory: () => {
    const state = get();
    if (!state.taskDescription) return;
    const settings = useSettingsStore.getState();
    const workspacePath = (settings.workspacePath || '').trim() || null;
    const sessionId = buildHistorySessionId(state.taskId, state.standaloneSessionId) || undefined;

    // Serialize streaming output into readable conversation content
    const TYPE_PREFIX: Record<StreamLineType, string> = {
      text: '[Assistant] ',
      info: '[User] ',
      error: '[Error] ',
      success: '[Success] ',
      warning: '[Warning] ',
      tool: '[Tool] ',
      tool_result: '[ToolResult] ',
      sub_agent: '[SubAgent] ',
      analysis: '[Analysis] ',
      thinking: '[Thinking] ',
      code: '[Code] ',
    };
    const conversationLines: HistoryConversationLine[] | undefined =
      state.streamingOutput.length > 0
        ? state.streamingOutput.map((line) => ({
            type: line.type,
            content: line.content,
          }))
        : undefined;
    const conversationContent =
      state.streamingOutput.length > 0
        ? state.streamingOutput
            .map((line) => `${TYPE_PREFIX[line.type]}${line.content}`)
            .join('\n')
        : undefined;

    const baseItem: Omit<ExecutionHistoryItem, 'id'> = {
      taskDescription: state.taskDescription,
      workspacePath,
      strategy: state.strategy,
      status: state.status,
      startedAt: state.startedAt || Date.now(),
      completedAt: Date.now(),
      duration: Date.now() - (state.startedAt || Date.now()),
      completedStories: state.stories.filter((s) => s.status === 'completed').length,
      totalStories: state.stories.length,
      success: state.status === 'completed',
      error: state.result?.error,
      conversationContent,
      conversationLines,
      sessionId,
    };

    set((prevState) => {
      let newHistory: ExecutionHistoryItem[] = prevState.history;
      if (sessionId) {
        const existingIndex = prevState.history.findIndex((item) => item.sessionId === sessionId);
        if (existingIndex >= 0) {
          const existing = prevState.history[existingIndex];
          const updated: ExecutionHistoryItem = {
            ...baseItem,
            id: existing.id,
            title: existing.title,
            taskDescription: existing.taskDescription || baseItem.taskDescription,
            startedAt: existing.startedAt,
            duration: Date.now() - existing.startedAt,
          };
          const cloned = [...prevState.history];
          cloned.splice(existingIndex, 1);
          newHistory = [updated, ...cloned].slice(0, MAX_HISTORY_ITEMS);
        } else {
          const created: ExecutionHistoryItem = {
            ...baseItem,
            id: `history_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
          };
          newHistory = [created, ...prevState.history].slice(0, MAX_HISTORY_ITEMS);
        }
      } else {
        const created: ExecutionHistoryItem = {
          ...baseItem,
          id: `history_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
        };
        newHistory = [created, ...prevState.history].slice(0, MAX_HISTORY_ITEMS);
      }

      // Save to localStorage
      try {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(newHistory));
      } catch {
        // Ignore localStorage errors
      }

      return { history: newHistory };
    });
  },

  clearHistory: () => {
    try {
      localStorage.removeItem(HISTORY_KEY);
    } catch {
      // Ignore localStorage errors
    }
    set({ history: [] });
  },

  deleteHistory: (historyId: string) => {
    set((state) => {
      const next = state.history.filter((item) => item.id !== historyId);
      try {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
      } catch {
        // Ignore localStorage errors
      }
      return { history: next };
    });
  },

  renameHistory: (historyId, title) => {
    const trimmed = title.trim();
    set((state) => {
      const next = state.history.map((item) =>
        item.id === historyId
          ? {
              ...item,
              title: trimmed.length > 0 ? trimmed : undefined,
            }
          : item
      );
      try {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
      } catch {
        // Ignore localStorage errors
      }
      return { history: next };
    });
  },

  restoreFromHistory: (historyId: string) => {
    const item = get().history.find((h) => h.id === historyId);
    if (!item) return;

    // Parse serialized content back into StreamLine entries
    const PREFIX_TO_TYPE: Record<string, StreamLineType> = {
      '[Assistant] ': 'text',
      '[User] ': 'info',
      '[Error] ': 'error',
      '[Success] ': 'success',
      '[Warning] ': 'warning',
      '[Tool] ': 'tool',
      '[ToolResult] ': 'tool_result',
      '[SubAgent] ': 'sub_agent',
      '[Analysis] ': 'analysis',
      '[Thinking] ': 'thinking',
      '[Code] ': 'code',
    };

    const lines: StreamLine[] = [];
    let counter = 0;

    if (item.conversationLines && item.conversationLines.length > 0) {
      for (const line of item.conversationLines) {
        counter++;
        lines.push({
          id: counter,
          content: line.content,
          type: line.type,
          timestamp: item.startedAt,
        });
      }
    } else if (item.conversationContent) {
      for (const raw of item.conversationContent.split('\n')) {
        let type: StreamLineType = 'text';
        let content = raw;

        for (const [prefix, lineType] of Object.entries(PREFIX_TO_TYPE)) {
          if (raw.startsWith(prefix)) {
            type = lineType;
            content = raw.slice(prefix.length);
            break;
          }
        }

        counter++;
        lines.push({
          id: counter,
          content,
          type,
          timestamp: item.startedAt,
        });
      }
    } else {
      return;
    }

    const restoredSessionId = item.sessionId || null;
    const claudePrefix = 'claude:';
    const standalonePrefix = 'standalone:';
    const isClaudeSession =
      restoredSessionId !== null && restoredSessionId.startsWith(claudePrefix);
    const isStandaloneSession =
      restoredSessionId !== null && restoredSessionId.startsWith(standalonePrefix);

    const restoredStandaloneTurns: StandaloneTurn[] = [];
    if (isStandaloneSession) {
      let pendingUser: string | null = null;
      let assistantSegments: string[] = [];
      for (const line of lines) {
        if (line.type === 'info') {
          if (pendingUser && assistantSegments.join('').trim().length > 0) {
            restoredStandaloneTurns.push({
              user: pendingUser,
              assistant: assistantSegments.join(''),
              createdAt: line.timestamp,
            });
          }
          pendingUser = line.content;
          assistantSegments = [];
        } else if (line.type === 'text' && pendingUser) {
          assistantSegments.push(line.content);
        }
      }
      if (pendingUser && assistantSegments.join('').trim().length > 0) {
        restoredStandaloneTurns.push({
          user: pendingUser,
          assistant: assistantSegments.join(''),
          createdAt: Date.now(),
        });
      }
    }

    set({
      ...initialState,
      connectionStatus: get().connectionStatus,
      history: get().history,
      streamingOutput: lines,
      streamLineCounter: counter,
      isChatSession: isClaudeSession,
      taskId: isClaudeSession ? restoredSessionId!.slice(claudePrefix.length) : null,
      standaloneSessionId: isStandaloneSession
        ? restoredSessionId!.slice(standalonePrefix.length)
        : null,
      standaloneTurns: isStandaloneSession
        ? trimStandaloneTurns(restoredStandaloneTurns, getStandaloneContextTurnsLimit())
        : [],
      taskDescription: item.title || item.taskDescription,
    });
  },

  analyzeStrategy: async (description: string) => {
    if (!description.trim()) return null;

    set({ isAnalyzingStrategy: true });
    get().addLog('Analyzing task strategy...');

    try {
      const result = await invoke<CommandResponse<StrategyAnalysis>>('analyze_task_strategy', {
        description,
        context: null,
      });

      if (result.success && result.data) {
        const analysis = result.data;
        set({
          strategyAnalysis: analysis,
          isAnalyzingStrategy: false,
          strategy: analysis.strategy as Strategy,
        });
        get().addLog(
          `Strategy recommendation: ${analysis.strategy} (confidence: ${(analysis.confidence * 100).toFixed(0)}%)`
        );
        return analysis;
      } else {
        throw new Error(result.error || 'Strategy analysis failed');
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({ isAnalyzingStrategy: false });
      get().addLog(`Strategy analysis error: ${errorMessage}`);
      return null;
    }
  },

  loadStrategyOptions: async () => {
    try {
      const result = await invoke<CommandResponse<StrategyOptionInfo[]>>('get_strategy_options');
      if (result.success && result.data) {
        set({ strategyOptions: result.data });
      }
    } catch {
      // Non-critical, options can load later
    }
  },

  clearStrategyAnalysis: () => {
    set({
      strategyAnalysis: null,
      isAnalyzingStrategy: false,
    });
  },

  appendStreamLine: (content, type) => {
    set((state) => {
      const lines = state.streamingOutput;
      const last = lines.length > 0 ? lines[lines.length - 1] : null;

      // For text and thinking deltas, concatenate to the last line of the same type
      // so streaming chunks form continuous prose instead of one-chunk-per-line
      if ((type === 'text' || type === 'thinking') && last && last.type === type) {
        const updated = { ...last, content: last.content + content };
        return {
          streamingOutput: [...lines.slice(0, -1), updated],
        };
      }

      // For other types or type transitions, append a new line
      const counter = state.streamLineCounter + 1;
      const line: StreamLine = {
        id: counter,
        content,
        type,
        timestamp: Date.now(),
      };
      // Keep buffer capped at 500 lines by trimming old entries when appending
      const trimmed = lines.length >= 500 ? lines.slice(-499) : lines;
      return {
        streamingOutput: [...trimmed, line],
        streamLineCounter: counter,
      };
    });
  },

  clearStreamingOutput: () => {
    set({ streamingOutput: [], streamLineCounter: 0 });
  },

  updateQualityGate: (result) => {
    set((state) => {
      const existing = state.qualityGateResults.findIndex(
        (r) => r.gateId === result.gateId && r.storyId === result.storyId
      );
      if (existing >= 0) {
        const updated = [...state.qualityGateResults];
        updated[existing] = result;
        return { qualityGateResults: updated };
      }
      return { qualityGateResults: [...state.qualityGateResults, result] };
    });
  },

  addExecutionError: (error) => {
    const newError: ExecutionError = {
      ...error,
      id: `err-${Date.now()}-${Math.random().toString(36).substr(2, 6)}`,
      timestamp: Date.now(),
      dismissed: false,
    };
    set((state) => ({
      executionErrors: [...state.executionErrors, newError],
    }));
    get().addLog(`[${error.severity.toUpperCase()}] ${error.title}: ${error.description}`);
  },

  dismissError: (errorId) => {
    set((state) => ({
      executionErrors: state.executionErrors.map((e) =>
        e.id === errorId ? { ...e, dismissed: true } : e
      ),
    }));
  },

  clearExecutionErrors: () => {
    set({ executionErrors: [] });
  },

  addAttachment: (file) => {
    set((state) => {
      // Avoid duplicates by id
      if (state.attachments.some((a) => a.id === file.id)) return state;
      return { attachments: [...state.attachments, file] };
    });
  },

  removeAttachment: (id) => {
    set((state) => ({
      attachments: state.attachments.filter((a) => a.id !== id),
    }));
  },

  clearAttachments: () => {
    set({ attachments: [] });
  },

  retryStory: async (storyId) => {
    const story = get().stories.find((s) => s.id === storyId);
    if (!story) return;

    // Reset story state
    get().updateStory(storyId, {
      status: 'in_progress',
      progress: 0,
      error: undefined,
      retryCount: (story.retryCount || 0) + 1,
    });

    // Clear related errors
    set((state) => ({
      executionErrors: state.executionErrors.filter((e) => e.storyId !== storyId),
    }));

    get().addLog(`Retrying story: ${story.title} (attempt ${(story.retryCount || 0) + 1})`);

    try {
      await invoke<CommandResponse<boolean>>('retry_story', {
        session_id: get().taskId,
        story_id: storyId,
      });
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Retry failed';
      get().updateStory(storyId, {
        status: 'failed',
        error: errorMessage,
      });
      get().addExecutionError({
        storyId,
        severity: 'error',
        title: `Retry failed for ${story.title}`,
        description: errorMessage,
        suggestedFix: 'Check the error output and try again, or skip this story.',
      });
    }
  },
}));

// ============================================================================
// Tauri Event Handlers
// ============================================================================

interface UnifiedEventPayload {
  type: string;
  run_id?: string;
  run_dir?: string;
  request?: string;
  session_id?: string;
  content?: string;
  phase_id?: string;
  title?: string;
  objective?: string;
  prompt?: string;
  sub_agent_id?: string;
  task_type?: string;
  role?: string;
  tool_id?: string;
  tool_name?: string;
  arguments?: string;
  file_path?: string;
  metrics?: Record<string, unknown>;
  issues?: string[];
  attempt?: number;
  max_attempts?: number;
  required_tools?: string[];
  gate_failures?: string[];
  reasons?: string[];
  worker_count?: number;
  layers?: string[];
  phase_results?: string[];
  total_metrics?: Record<string, unknown>;
  successful_phases?: number;
  partial_phases?: number;
  failed_phases?: number;
  reason?: string;
  status?: string;
  result?: string;
  usage?: Record<string, unknown>;
  error?: string;
  message?: string;
  code?: string;
  story_id?: string;
  story_title?: string;
  story_index?: number;
  total_stories?: number;
  success?: boolean;
  passed?: boolean;
  summary?: unknown;
  thinking_id?: string;
  stop_reason?: string;
  input_tokens?: number;
  output_tokens?: number;
  thinking_tokens?: number;
  messages_compacted?: number;
  messages_preserved?: number;
  compaction_tokens?: number;
  manifest_path?: string;
  report_path?: string;
}

function parseOptionalNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

function parseMetricNumber(
  source: Record<string, unknown> | undefined,
  key: string
): number | undefined {
  if (!source) return undefined;
  return parseOptionalNumber(source[key]);
}

function parsePhaseResultMetrics(phaseResults: string[]): Record<string, number> {
  const parsed: Record<string, number> = {};
  for (const item of phaseResults) {
    const idx = item.indexOf('=');
    if (idx <= 0) continue;
    const key = item.slice(0, idx).trim();
    const rawValue = item.slice(idx + 1).trim();
    const numberValue = Number(rawValue);
    if (!Number.isFinite(numberValue)) continue;
    parsed[key] = numberValue;
  }
  return parsed;
}

function formatSubAgentUsage(usage?: Record<string, unknown>): string {
  if (!usage || typeof usage !== 'object') return '';
  const inputTokens = parseOptionalNumber(usage.input_tokens);
  const outputTokens = parseOptionalNumber(usage.output_tokens);
  const iterations = parseOptionalNumber(usage.iterations);
  const fragments: string[] = [];

  if (typeof inputTokens === 'number') {
    fragments.push(`in=${inputTokens}`);
  }
  if (typeof outputTokens === 'number') {
    fragments.push(`out=${outputTokens}`);
  }
  if (typeof iterations === 'number') {
    fragments.push(`iter=${iterations}`);
  }

  return fragments.length > 0 ? ` (${fragments.join(', ')})` : '';
}

function formatAnalysisMetrics(metrics?: Record<string, unknown>): string {
  if (!metrics || typeof metrics !== 'object') return '';
  const toolCalls = parseOptionalNumber(metrics.tool_calls);
  const readCalls = parseOptionalNumber(metrics.read_calls);
  const grepCalls = parseOptionalNumber(metrics.grep_calls);
  const globCalls = parseOptionalNumber(metrics.glob_calls);
  const cwdCalls = parseOptionalNumber(metrics.cwd_calls);
  const observedPaths = parseOptionalNumber(metrics.observed_paths);
  const coverageRatio = parseOptionalNumber(metrics.coverage_ratio);
  const sampledReadRatio = parseOptionalNumber(metrics.sampled_read_ratio);
  const testCoverageRatio = parseOptionalNumber(metrics.test_coverage_ratio);
  const fragments: string[] = [];

  if (typeof toolCalls === 'number') fragments.push(`tools=${toolCalls}`);
  if (typeof readCalls === 'number') fragments.push(`read=${readCalls}`);
  if (typeof grepCalls === 'number') fragments.push(`grep=${grepCalls}`);
  if (typeof globCalls === 'number') fragments.push(`glob=${globCalls}`);
  if (typeof cwdCalls === 'number') fragments.push(`cwd=${cwdCalls}`);
  if (typeof observedPaths === 'number') fragments.push(`paths=${observedPaths}`);
  if (typeof coverageRatio === 'number') fragments.push(`coverage=${(coverageRatio * 100).toFixed(1)}%`);
  if (typeof sampledReadRatio === 'number') fragments.push(`read_depth=${(sampledReadRatio * 100).toFixed(1)}%`);
  if (typeof testCoverageRatio === 'number') fragments.push(`tests=${(testCoverageRatio * 100).toFixed(1)}%`);
  return fragments.length > 0 ? ` (${fragments.join(', ')})` : '';
}

function toShortText(value: unknown, fallback = ''): string {
  if (typeof value !== 'string') return fallback;
  return value.trim();
}

function handleUnifiedExecutionEvent(
  payload: UnifiedEventPayload,
  get: () => ExecutionState,
  set: (partial: Partial<ExecutionState>) => void
) {
  const currentMode = useModeStore.getState().mode;
  const isSimpleMode = currentMode === 'simple';
  const showSubAgent = useSettingsStore.getState().showSubAgentEvents && !isSimpleMode;
  const showAnalysisDetails = useSettingsStore.getState().showSubAgentEvents && !isSimpleMode;

  switch (payload.type) {
    case 'analysis_run_started': {
      const runId = toShortText(payload.run_id, 'run');
      const runDir = toShortText(payload.run_dir);
      const request = toShortText(payload.request);
      set({
        analysisCoverage: {
          runId,
          status: 'running',
          successfulPhases: 0,
          partialPhases: 0,
          failedPhases: 0,
          observedPaths: 0,
          inventoryTotalFiles: 0,
          sampledReadFiles: 0,
          testFilesTotal: 0,
          testFilesRead: 0,
          coverageRatio: 0,
          sampledReadRatio: 0,
          testCoverageRatio: 0,
          observedTestCoverageRatio: 0,
          validationIssues: [],
          updatedAt: Date.now(),
        },
      });
      if (showAnalysisDetails) {
        const suffix = runDir ? ` | ${runDir}` : '';
        get().appendStreamLine(`[analysis:run_start:${runId}] ${request || 'analysis started'}${suffix}`, 'analysis');
      } else {
        get().appendStreamLine(`[analysis] run started (${runId})`, 'analysis');
      }
      break;
    }

    case 'analysis_phase_planned': {
      const phaseId = toShortText(payload.phase_id, 'phase');
      const title = toShortText(payload.title, phaseId);
      const workerCount = typeof payload.worker_count === 'number' ? payload.worker_count : 0;
      const layers = Array.isArray(payload.layers) ? payload.layers.length : 0;
      if (showAnalysisDetails) {
        get().appendStreamLine(
          `[analysis:phase_plan:${phaseId}] ${title} | workers=${workerCount}, layers=${layers}`,
          'analysis'
        );
      } else {
        get().appendStreamLine(`[analysis] planning ${title}`, 'analysis');
      }
      break;
    }

    case 'analysis_sub_agent_planned': {
      if (showAnalysisDetails) {
        const phaseId = toShortText(payload.phase_id, 'phase');
        const subAgentId = toShortText(payload.sub_agent_id, 'worker');
        const role = toShortText(payload.role, 'worker');
        const objective = toShortText(payload.objective);
        const suffix = objective ? ` | ${objective}` : '';
        get().appendStreamLine(
          `[analysis:subagent_plan:${phaseId}] ${subAgentId} (${role})${suffix}`,
          'analysis'
        );
      }
      break;
    }

    case 'analysis_sub_agent_progress': {
      const phaseId = toShortText(payload.phase_id, 'phase');
      const subAgentId = toShortText(payload.sub_agent_id, 'worker');
      const status = toShortText(payload.status, 'running');
      const details = toShortText(payload.message);
      if (showAnalysisDetails) {
        get().appendStreamLine(
          `[analysis:subagent:${phaseId}:${subAgentId}] ${status}${details ? ` | ${details}` : ''}`,
          'analysis'
        );
      }
      break;
    }

    case 'text_delta':
      if (payload.content) {
        const filterResult = get().toolCallFilter.processChunk(payload.content);
        if (filterResult.output) {
          get().appendStreamLine(filterResult.output, 'text');
        }
        if (filterResult.toolIndicator) {
          get().appendStreamLine(filterResult.toolIndicator, 'tool');
        }
      }
      break;

    case 'text_replace': {
      // Replace accumulated text lines from the CURRENT TURN with a single
      // cleaned version.  During streaming with FallbackToolFormatMode, the
      // LLM often repeats its reasoning text before/between/after tool call
      // blocks, producing multiple 'text' lines separated by 'tool' indicator
      // lines.  The cleaned content from the backend has tool blocks stripped
      // and duplicates removed, so we collapse current-turn text lines into one.
      //
      // IMPORTANT: Only touch lines whose id > currentTurnStartLineId so that
      // previous turns' content is preserved in multi-turn conversations.
      const lines = get().streamingOutput;
      const turnBoundary = get().currentTurnStartLineId;
      const textIndices = lines
        .map((l, i) => (l.type === 'text' && l.id > turnBoundary ? i : -1))
        .filter((i) => i >= 0);
      if (textIndices.length > 0) {
        const cleaned = payload.content || '';
        const lastTextIdx = textIndices[textIndices.length - 1];
        const otherTextIndices = new Set(textIndices.slice(0, -1));
        if (cleaned) {
          // Keep the last current-turn text line with cleaned content,
          // remove earlier current-turn text lines
          const updated = lines.filter((_, i) => !otherTextIndices.has(i));
          const newLastIdx = lastTextIdx - otherTextIndices.size;
          updated[newLastIdx] = { ...updated[newLastIdx], content: cleaned };
          set({ streamingOutput: updated });
        } else {
          // Remove all current-turn text lines if cleaned is empty
          const allTextIndices = new Set(textIndices);
          set({ streamingOutput: lines.filter((_, i) => !allTextIndices.has(i)) });
        }
      }
      break;
    }

    case 'thinking_start':
      if (useSettingsStore.getState().showReasoningOutput) {
        get().appendStreamLine('[thinking...]', 'thinking');
      }
      break;

    case 'thinking_delta':
      if (useSettingsStore.getState().showReasoningOutput && payload.content) {
        get().appendStreamLine(payload.content, 'thinking');
      }
      break;

    case 'thinking_end':
      break;

    case 'tool_start':
      if (payload.tool_name) {
        const argsPreview = formatToolArgs(payload.tool_name, payload.arguments);
        get().appendStreamLine(`[tool:${payload.tool_name}] ${argsPreview}`, 'tool');
      }
      break;

    case 'tool_complete':
      // Tool call arguments fully accumulated; no UI action needed
      // (tool execution events tool_start/tool_result already render)
      break;

    case 'tool_result':
      if (payload.error) {
        get().appendStreamLine(`[tool_error:${payload.tool_id || ''}] ${payload.error}`, 'error');
      } else if (payload.result) {
        const preview = payload.result.length > 500
          ? payload.result.substring(0, 500) + '...'
          : payload.result;
        get().appendStreamLine(`[tool_result:${payload.tool_id || ''}] ${preview}`, 'tool_result');
      }
      break;

    case 'sub_agent_start':
      if (showSubAgent) {
        const promptPreview = (payload.prompt || '').trim().substring(0, 180);
        const label = promptPreview || payload.sub_agent_id || payload.task_type || 'sub-agent';
        get().appendStreamLine(`[sub_agent:start] ${label}`, 'sub_agent');
      }
      break;

    case 'sub_agent_end':
      if (payload.success === false || showSubAgent) {
        const usage = formatSubAgentUsage(payload.usage);
        get().appendStreamLine(
          `[sub_agent:end] ${payload.success ? 'completed' : 'failed'}${usage}`,
          'sub_agent'
        );
      }
      break;

    case 'analysis_phase_start': {
      if (showAnalysisDetails) {
        const phaseId = toShortText(payload.phase_id, 'phase');
        const title = toShortText(payload.title, phaseId);
        const objective = toShortText(payload.objective);
        const details = objective ? `${title} - ${objective}` : title;
        get().appendStreamLine(`[analysis:phase_start:${phaseId}] ${details}`, 'analysis');
      } else if (isSimpleMode) {
        const title = toShortText(payload.title, toShortText(payload.phase_id, 'phase'));
        get().appendStreamLine(`[analysis] ${title}`, 'analysis');
      }
      break;
    }

    case 'analysis_phase_attempt_start': {
      if (showAnalysisDetails) {
        const phaseId = toShortText(payload.phase_id, 'phase');
        const attempt = typeof payload.attempt === 'number' ? payload.attempt : 0;
        const maxAttempts = typeof payload.max_attempts === 'number' ? payload.max_attempts : 0;
        const requiredTools = Array.isArray(payload.required_tools)
          ? payload.required_tools.join(', ')
          : '';
        const suffix = requiredTools ? ` | required: ${requiredTools}` : '';
        get().appendStreamLine(
          `[analysis:attempt_start:${phaseId}] attempt ${attempt}/${maxAttempts}${suffix}`,
          'analysis'
        );
      }
      break;
    }

    case 'analysis_phase_progress': {
      if (showAnalysisDetails) {
        const phaseId = toShortText(payload.phase_id, 'phase');
        const details = toShortText(payload.message, 'progress update');
        get().appendStreamLine(`[analysis:phase_progress:${phaseId}] ${details}`, 'analysis');
      }
      break;
    }

    case 'analysis_evidence': {
      if (showAnalysisDetails || payload.success === false) {
        const phaseId = toShortText(payload.phase_id, 'phase');
        const toolName = toShortText(payload.tool_name, 'tool');
        const summaryValue = typeof payload.summary === 'string' ? payload.summary : payload.message;
        const summary = toShortText(summaryValue, 'evidence captured');
        const filePath = toShortText(payload.file_path);
        const suffix = filePath ? ` (${filePath})` : '';
        const state = payload.success === false ? 'error' : 'ok';
        get().appendStreamLine(
          `[analysis:evidence:${phaseId}:${state}] ${toolName}: ${summary}${suffix}`,
          'analysis'
        );
      }
      break;
    }

    case 'analysis_phase_end': {
      if (showAnalysisDetails || payload.success === false) {
        const phaseId = toShortText(payload.phase_id, 'phase');
        const usage = formatSubAgentUsage(payload.usage);
        const metrics = formatAnalysisMetrics(payload.metrics);
        get().appendStreamLine(
          `[analysis:phase_end:${phaseId}] ${payload.success ? 'completed' : 'failed'}${usage}${metrics}`,
          'analysis'
        );
      } else if (isSimpleMode) {
        const phaseId = toShortText(payload.phase_id, 'phase');
        get().appendStreamLine(
          `[analysis] ${phaseId} ${payload.success ? 'completed' : 'completed (partial)'}`,
          'analysis'
        );
      }
      break;
    }

    case 'analysis_phase_attempt_end': {
      if (showAnalysisDetails || payload.success === false) {
        const phaseId = toShortText(payload.phase_id, 'phase');
        const attempt = typeof payload.attempt === 'number' ? payload.attempt : 0;
        const metrics = formatAnalysisMetrics(payload.metrics);
        const gateFailures = Array.isArray(payload.gate_failures) ? payload.gate_failures : [];
        const failurePreview =
          gateFailures.length > 0 ? ` | ${gateFailures.slice(0, 2).join(' ; ')}` : '';
        get().appendStreamLine(
          `[analysis:attempt_end:${phaseId}] attempt ${attempt} ${payload.success ? 'passed' : 'failed'}${metrics}${failurePreview}`,
          'analysis'
        );
      }
      break;
    }

    case 'analysis_gate_failure': {
      const phaseId = toShortText(payload.phase_id, 'phase');
      const attempt = typeof payload.attempt === 'number' ? payload.attempt : 0;
      const reasons = Array.isArray(payload.reasons) ? payload.reasons : [];
      const reasonText = reasons.length > 0 ? reasons.slice(0, 3).join(' ; ') : 'unknown';
      if (showAnalysisDetails) {
        get().appendStreamLine(
          `[analysis:gate_failure:${phaseId}] attempt ${attempt} | ${reasonText}`,
          'analysis'
        );
      } else {
        get().appendStreamLine(`[analysis] ${phaseId} adjusted: ${reasonText}`, 'analysis');
      }
      break;
    }

    case 'analysis_phase_degraded': {
      const phaseId = toShortText(payload.phase_id, 'phase');
      const attempt = typeof payload.attempt === 'number' ? payload.attempt : 0;
      const reasons = Array.isArray(payload.reasons) ? payload.reasons : [];
      const reasonText = reasons.length > 0 ? reasons.slice(0, 2).join(' ; ') : 'budget gate';
      get().appendStreamLine(
        `[analysis] ${phaseId} degraded at attempt ${attempt}: ${reasonText}`,
        'analysis'
      );
      break;
    }

    case 'analysis_validation': {
      const validationStatus = toShortText(payload.status, 'unknown');
      const issues = Array.isArray(payload.issues) ? payload.issues : [];
      const issuePreview =
        issues.length > 0 ? ` | ${issues.slice(0, 3).join(' ; ')}` : '';
      get().appendStreamLine(
        `[analysis:validation:${validationStatus}] ${issues.length} issue(s)${issuePreview}`,
        'analysis'
      );

      if (validationStatus === 'warning' && issues.length > 0) {
        get().addExecutionError({
          severity: 'warning',
          title: 'Analysis validation warning',
          description: issues.slice(0, 5).join('\n'),
          suggestedFix: 'Review evidence lines and rerun analysis if needed.',
        });
      }
      const currentCoverage = get().analysisCoverage;
      if (currentCoverage) {
        set({
          analysisCoverage: {
            ...currentCoverage,
            validationIssues: issues.slice(0, 20),
            updatedAt: Date.now(),
          },
        });
      }
      break;
    }

    case 'analysis_run_summary': {
      const phaseResults = Array.isArray(payload.phase_results) ? payload.phase_results : [];
      const metrics = payload.total_metrics && typeof payload.total_metrics === 'object'
        ? JSON.stringify(payload.total_metrics)
        : '';
      const summary =
        phaseResults.length > 0 ? phaseResults.join(' | ') : 'no phase results';
      const suffix = metrics ? ` | ${metrics}` : '';
      get().appendStreamLine(
        `[analysis:run_summary:${payload.success ? 'success' : 'failed'}] ${summary}${suffix}`,
        'analysis'
      );
      const parsedPhaseMetrics = parsePhaseResultMetrics(phaseResults);
      const totalMetrics =
        payload.total_metrics && typeof payload.total_metrics === 'object'
          ? payload.total_metrics
          : undefined;
      const current = get().analysisCoverage;
      const next: AnalysisCoverageSnapshot = {
        runId: current?.runId || toShortText(payload.run_id) || undefined,
        status: payload.success === false ? 'failed' : 'completed',
        successfulPhases:
          parseMetricNumber(parsedPhaseMetrics, 'successful_phases') ??
          current?.successfulPhases ??
          0,
        partialPhases:
          parseMetricNumber(parsedPhaseMetrics, 'partial_phases') ??
          current?.partialPhases ??
          0,
        failedPhases:
          parseMetricNumber(parsedPhaseMetrics, 'failed_phases') ??
          current?.failedPhases ??
          0,
        observedPaths:
          parseMetricNumber(parsedPhaseMetrics, 'observed_paths') ??
          parseMetricNumber(totalMetrics, 'observed_paths') ??
          current?.observedPaths ??
          0,
        inventoryTotalFiles:
          parseMetricNumber(totalMetrics, 'inventory_total_files') ??
          current?.inventoryTotalFiles ??
          0,
        sampledReadFiles:
          parseMetricNumber(parsedPhaseMetrics, 'sampled_read_files') ??
          parseMetricNumber(totalMetrics, 'sampled_read_files') ??
          current?.sampledReadFiles ??
          0,
        testFilesTotal:
          parseMetricNumber(totalMetrics, 'test_files_total') ??
          current?.testFilesTotal ??
          0,
        testFilesRead:
          parseMetricNumber(totalMetrics, 'test_files_read') ??
          current?.testFilesRead ??
          0,
        coverageRatio:
          parseMetricNumber(parsedPhaseMetrics, 'coverage_ratio') ??
          parseMetricNumber(totalMetrics, 'coverage_ratio') ??
          current?.coverageRatio ??
          0,
        sampledReadRatio:
          parseMetricNumber(parsedPhaseMetrics, 'sampled_read_ratio') ??
          parseMetricNumber(totalMetrics, 'sampled_read_ratio') ??
          current?.sampledReadRatio ??
          0,
        testCoverageRatio:
          parseMetricNumber(parsedPhaseMetrics, 'test_coverage_ratio') ??
          parseMetricNumber(totalMetrics, 'test_coverage_ratio') ??
          current?.testCoverageRatio ??
          0,
        observedTestCoverageRatio:
          parseMetricNumber(totalMetrics, 'observed_test_coverage_ratio') ??
          current?.observedTestCoverageRatio ??
          0,
        coverageTargetRatio:
          parseMetricNumber(parsedPhaseMetrics, 'coverage_target_ratio') ??
          parseMetricNumber(totalMetrics, 'coverage_target_ratio') ??
          current?.coverageTargetRatio,
        sampledReadTargetRatio:
          parseMetricNumber(parsedPhaseMetrics, 'sampled_read_target_ratio') ??
          parseMetricNumber(totalMetrics, 'sampled_read_target_ratio') ??
          current?.sampledReadTargetRatio,
        testCoverageTargetRatio:
          parseMetricNumber(parsedPhaseMetrics, 'test_coverage_target_ratio') ??
          parseMetricNumber(totalMetrics, 'test_coverage_target_ratio') ??
          current?.testCoverageTargetRatio,
        validationIssues: current?.validationIssues || [],
        manifestPath: current?.manifestPath,
        reportPath: current?.reportPath,
        updatedAt: Date.now(),
      };
      set({ analysisCoverage: next });
      break;
    }

    case 'analysis_coverage_updated': {
      const metrics = payload.metrics && typeof payload.metrics === 'object'
        ? payload.metrics
        : undefined;
      const summary = metrics ? formatAnalysisMetrics(metrics) : '';
      if (metrics) {
        const current = get().analysisCoverage;
        set({
          analysisCoverage: {
            runId: current?.runId || toShortText(payload.run_id) || undefined,
            status: current?.status || 'running',
            successfulPhases:
              parseMetricNumber(metrics, 'successful_phases') ??
              current?.successfulPhases ??
              0,
            partialPhases:
              parseMetricNumber(metrics, 'partial_phases') ??
              current?.partialPhases ??
              0,
            failedPhases:
              parseMetricNumber(metrics, 'failed_phases') ??
              current?.failedPhases ??
              0,
            observedPaths:
              parseMetricNumber(metrics, 'observed_paths') ??
              current?.observedPaths ??
              0,
            inventoryTotalFiles:
              parseMetricNumber(metrics, 'inventory_total_files') ??
              current?.inventoryTotalFiles ??
              0,
            sampledReadFiles:
              parseMetricNumber(metrics, 'sampled_read_files') ??
              current?.sampledReadFiles ??
              0,
            testFilesTotal:
              parseMetricNumber(metrics, 'test_files_total') ??
              current?.testFilesTotal ??
              0,
            testFilesRead:
              parseMetricNumber(metrics, 'test_files_read') ??
              current?.testFilesRead ??
              0,
            coverageRatio:
              parseMetricNumber(metrics, 'coverage_ratio') ??
              current?.coverageRatio ??
              0,
            sampledReadRatio:
              parseMetricNumber(metrics, 'sampled_read_ratio') ??
              current?.sampledReadRatio ??
              0,
            testCoverageRatio:
              parseMetricNumber(metrics, 'test_coverage_ratio') ??
              current?.testCoverageRatio ??
              0,
            observedTestCoverageRatio:
              parseMetricNumber(metrics, 'observed_test_coverage_ratio') ??
              current?.observedTestCoverageRatio ??
              0,
            coverageTargetRatio:
              parseMetricNumber(metrics, 'coverage_target_ratio') ??
              current?.coverageTargetRatio,
            sampledReadTargetRatio:
              parseMetricNumber(metrics, 'sampled_read_target_ratio') ??
              current?.sampledReadTargetRatio,
            testCoverageTargetRatio:
              parseMetricNumber(metrics, 'test_coverage_target_ratio') ??
              current?.testCoverageTargetRatio,
            validationIssues: current?.validationIssues || [],
            manifestPath: current?.manifestPath,
            reportPath: current?.reportPath,
            updatedAt: Date.now(),
          },
        });
      }
      if (showAnalysisDetails) {
        get().appendStreamLine(
          `[analysis:coverage] updated${summary}`,
          'analysis'
        );
      }
      break;
    }

    case 'analysis_run_completed': {
      const runId = toShortText(payload.run_id, 'run');
      const manifestPath = toShortText(payload.manifest_path);
      const reportPath = toShortText(payload.report_path);
      const status = payload.success === false ? 'failed' : 'completed';
      const parts = [manifestPath, reportPath].filter(Boolean);
      const suffix = parts.length > 0 ? ` | ${parts.join(' | ')}` : '';
      get().appendStreamLine(
        `[analysis] run ${status} (${runId})${suffix}`,
        payload.success === false ? 'warning' : 'success'
      );
      const currentCoverage = get().analysisCoverage;
      if (currentCoverage) {
        set({
          analysisCoverage: {
            ...currentCoverage,
            runId,
            status: payload.success === false ? 'failed' : 'completed',
            manifestPath: manifestPath || currentCoverage.manifestPath,
            reportPath: reportPath || currentCoverage.reportPath,
            updatedAt: Date.now(),
          },
        });
      }
      break;
    }

    case 'analysis_partial': {
      const passed = typeof payload.successful_phases === 'number' ? payload.successful_phases : 0;
      const partial = typeof payload.partial_phases === 'number' ? payload.partial_phases : 0;
      const failed = typeof payload.failed_phases === 'number' ? payload.failed_phases : 0;
      const reason = toShortText(payload.reason, 'partial evidence mode');
      get().appendStreamLine(
        `[analysis:partial] passed=${passed}, partial=${partial}, failed=${failed} | ${reason}`,
        'analysis'
      );
      break;
    }

    case 'usage':
      if (typeof payload.input_tokens === 'number' && typeof payload.output_tokens === 'number') {
        const payloadRecord = payload as unknown as Record<string, unknown>;
        const usage: BackendUsageStats = {
          input_tokens: payload.input_tokens,
          output_tokens: payload.output_tokens,
          thinking_tokens:
            typeof payload.thinking_tokens === 'number' ? payload.thinking_tokens : null,
          cache_read_tokens:
            typeof payloadRecord.cache_read_tokens === 'number'
              ? payloadRecord.cache_read_tokens
              : null,
          cache_creation_tokens:
            typeof payloadRecord.cache_creation_tokens === 'number'
              ? payloadRecord.cache_creation_tokens
              : null,
        };
        const prev = get().sessionUsageTotals;
        const nextTotals: BackendUsageStats = {
          input_tokens: (prev?.input_tokens || 0) + usage.input_tokens,
          output_tokens: (prev?.output_tokens || 0) + usage.output_tokens,
          thinking_tokens: (prev?.thinking_tokens || 0) + (usage.thinking_tokens || 0),
          cache_read_tokens: (prev?.cache_read_tokens || 0) + (usage.cache_read_tokens || 0),
          cache_creation_tokens:
            (prev?.cache_creation_tokens || 0) + (usage.cache_creation_tokens || 0),
        };
        set({
          latestUsage: usage,
          sessionUsageTotals: nextTotals,
        });
        get().addLog(
          `Usage: in=${payload.input_tokens}, out=${payload.output_tokens}${typeof payload.thinking_tokens === 'number' ? `, thinking=${payload.thinking_tokens}` : ''}`
        );
      }
      break;

    case 'error':
      if (payload.message) {
        get().appendStreamLine(`[error] ${payload.message}`, 'error');
        get().addExecutionError({
          severity: 'error',
          title: 'Stream Error',
          description: payload.message,
          suggestedFix: 'Check the error details and retry if needed.',
        });
      }
      break;

    case 'complete': {
      // Flush any buffered content from the tool-call filter
      const flushedText = get().toolCallFilter.flush();
      if (flushedText) {
        get().appendStreamLine(flushedText, 'text');
      }

      // For standalone one-shot execution, this is the final completion signal.
      if (get().status === 'running' || get().status === 'paused') {
        const completedStories = get().stories.filter((s) => s.status === 'completed').length;
        const totalStories = get().stories.length || 1;
        const duration = Date.now() - (get().startedAt || Date.now());
        const durationStr = duration >= 60000
          ? `${Math.floor(duration / 60000)}m ${Math.round((duration % 60000) / 1000)}s`
          : `${Math.round(duration / 1000)}s`;

        set({
          status: 'completed',
          progress: 100,
          estimatedTimeRemaining: 0,
          result: {
            success: true,
            message: 'Execution completed',
            completedStories: completedStories > 0 ? completedStories : 1,
            totalStories,
            duration,
          },
        });
        get().appendStreamLine(`Completed (${durationStr})`, 'success');
        get().addLog('Execution completed');
      }
      break;
    }

    case 'story_start':
      if (payload.story_id && payload.story_title) {
        get().appendStreamLine(
          `Starting story ${(payload.story_index || 0) + 1}/${payload.total_stories || '?'}: ${payload.story_title}`,
          'info'
        );
        get().updateStory(payload.story_id, {
          status: 'in_progress',
          startedAt: new Date().toISOString(),
        });
        set({ currentStoryId: payload.story_id });

        // Estimate time remaining based on average story completion time
        const state = get();
        const completedStories = state.stories.filter((s) => s.status === 'completed');
        if (completedStories.length > 0 && state.startedAt) {
          const elapsed = Date.now() - state.startedAt;
          const avgTimePerStory = elapsed / completedStories.length;
          const remainingStories = (payload.total_stories || state.stories.length) - completedStories.length;
          set({ estimatedTimeRemaining: Math.round(avgTimePerStory * remainingStories) });
        }
      }
      break;

    case 'story_complete':
      if (payload.story_id) {
        const success = payload.success !== false;
        get().updateStory(payload.story_id, {
          status: success ? 'completed' : 'failed',
          progress: success ? 100 : 0,
          completedAt: new Date().toISOString(),
          error: payload.error,
        });
        get().appendStreamLine(
          `Story ${success ? 'completed' : 'failed'}: ${payload.story_id}${payload.error ? ' - ' + payload.error : ''}`,
          success ? 'success' : 'error'
        );

        if (!success && payload.error) {
          const story = get().stories.find((s) => s.id === payload.story_id);
          get().addExecutionError({
            storyId: payload.story_id,
            severity: 'error',
            title: `Story failed: ${story?.title || payload.story_id}`,
            description: payload.error,
            suggestedFix: 'Review the error output and retry this story.',
          });
        }
      }
      break;

    case 'quality_gates_result':
      if (payload.story_id && payload.summary && typeof payload.summary === 'object') {
        const summary = payload.summary as Record<string, { passed?: boolean; output?: string; duration?: number }>;
        const passed = payload.passed !== false;

        // Parse individual gate results from summary
        for (const [gateName, gateResult] of Object.entries(summary)) {
          get().updateQualityGate({
            gateId: gateName.toLowerCase().replace(/\s+/g, '_'),
            gateName,
            storyId: payload.story_id,
            status: gateResult.passed !== false ? 'passed' : 'failed',
            output: gateResult.output,
            duration: gateResult.duration,
            completedAt: Date.now(),
          });
        }

        get().appendStreamLine(
          `Quality gates ${passed ? 'passed' : 'failed'} for story: ${payload.story_id}`,
          passed ? 'success' : 'warning'
        );
      }
      break;

    case 'context_compaction': {
      const compacted = (payload as unknown as { messages_compacted?: number }).messages_compacted || 0;
      const preserved = (payload as unknown as { messages_preserved?: number }).messages_preserved || 0;
      const tokens = (payload as unknown as { compaction_tokens?: number }).compaction_tokens || 0;
      get().appendStreamLine(
        `[context compaction] ${compacted} messages summarized, ${preserved} preserved (${tokens} tokens)`,
        'info'
      );
      get().addLog(`Context compaction: ${compacted} messages compacted, ${preserved} preserved`);
      break;
    }

    case 'session_complete':
      if (payload.success !== undefined) {
        const completedStories = payload.success
          ? (payload.total_stories || get().stories.length)
          : get().stories.filter((s) => s.status === 'completed').length;
        const totalStories = payload.total_stories || get().stories.length;

        set({
          status: payload.success ? 'completed' : 'failed',
          progress: payload.success ? 100 : get().progress,
          estimatedTimeRemaining: 0,
          result: {
            success: payload.success,
            message: payload.success ? 'Execution completed' : 'Execution failed',
            completedStories,
            totalStories,
            duration: Date.now() - (get().startedAt || Date.now()),
          },
        });
        get().appendStreamLine(
          payload.success ? 'All stories completed successfully.' : 'Execution finished with failures.',
          payload.success ? 'success' : 'error'
        );
        get().saveToHistory();
      }
      break;
  }
}

async function setupTauriEventListeners(
  get: () => ExecutionState,
  set: (partial: Partial<ExecutionState>) => void
) {
  const setupVersion = ++listenerSetupVersion;

  // Clean up any existing listeners first
  for (const unlisten of unlisteners) {
    unlisten();
  }
  unlisteners = [];

  const registerListener = (unlisten: UnlistenFn): boolean => {
    if (setupVersion !== listenerSetupVersion) {
      unlisten();
      return false;
    }
    unlisteners.push(unlisten);
    return true;
  };

  try {
    // Listen for stream events from Claude Code backend
    // UnifiedStreamEvent uses serde tagged enum: { type: "text_delta", content: "..." }
    const unlistenStream = await listen<StreamEventPayload>('claude_code:stream', (event) => {
      const { event: streamEvent, session_id } = event.payload;

      // Only process events for current session
      const currentTaskId = get().taskId;
      if (currentTaskId && currentTaskId !== session_id) {
        return;
      }

      switch (streamEvent.type) {
        case 'text_delta': {
          const filterResult = get().toolCallFilter.processChunk(streamEvent.content);
          if (filterResult.output) {
            get().appendStreamLine(filterResult.output, 'text');
          }
          if (filterResult.toolIndicator) {
            get().appendStreamLine(filterResult.toolIndicator, 'tool');
          }
          break;
        }

        case 'thinking_start':
          if (useSettingsStore.getState().showReasoningOutput) {
            get().appendStreamLine('[thinking...]', 'thinking');
          }
          break;

        case 'thinking_delta':
          if (useSettingsStore.getState().showReasoningOutput) {
            get().appendStreamLine(streamEvent.content, 'thinking');
          }
          break;

        case 'tool_start':
          get().appendStreamLine(`[tool] ${streamEvent.tool_name} started`, 'tool');
          get().addLog(`Tool started: ${streamEvent.tool_name}`);
          break;

        case 'tool_result': {
          const isError = !!streamEvent.error;
          get().appendStreamLine(
            `[tool] ${streamEvent.tool_id} ${isError ? 'failed' : 'completed'}`,
            isError ? 'error' : 'success'
          );
          break;
        }

        case 'error':
          get().appendStreamLine(streamEvent.message, 'error');
          get().addExecutionError({
            severity: 'critical',
            title: 'Execution Failed',
            description: streamEvent.message,
            suggestedFix: 'Check the error details and retry the execution.',
          });
          set({
            status: 'failed',
            apiError: streamEvent.message,
            result: {
              success: false,
              message: 'Execution failed',
              completedStories: get().stories.filter((s) => s.status === 'completed').length,
              totalStories: get().stories.length,
              duration: Date.now() - (get().startedAt || Date.now()),
              error: streamEvent.message,
            },
          });
          get().addLog(`Error: ${streamEvent.message}`);
          get().saveToHistory();
          break;

        case 'complete': {
          // Flush any buffered content from the tool-call filter
          const ccFlushed = get().toolCallFilter.flush();
          if (ccFlushed) {
            get().appendStreamLine(ccFlushed, 'text');
          }

          if (get().isChatSession) {
            // Chat session: stay ready for follow-up messages
            // Keep streamingOutput visible, go back to idle
            set({
              status: 'idle',
              isSubmitting: false,
              progress: 100,
              estimatedTimeRemaining: 0,
            });
            get().addLog('Response complete — ready for follow-up');
          } else {
            // Non-chat execution: show result view
            const completedStories = get().stories.filter((s) => s.status === 'completed').length;
            const totalStories = get().stories.length;

            get().appendStreamLine('Execution completed successfully.', 'success');
            set({
              status: 'completed',
              progress: 100,
              estimatedTimeRemaining: 0,
              result: {
                success: true,
                message: 'Execution completed',
                completedStories,
                totalStories,
                duration: Date.now() - (get().startedAt || Date.now()),
              },
            });
            get().addLog('Execution completed');
            get().saveToHistory();
          }
          break;
        }
      }
    });
    if (!registerListener(unlistenStream)) return;

    // Listen for tool events
    const unlistenTool = await listen<{
      execution: { id: string; tool_name: string; success?: boolean; arguments?: string; result?: string };
      update_type: string;
      session_id: string;
    }>('claude_code:tool', (event) => {
      const { execution, update_type } = event.payload;

      if (update_type === 'started') {
        get().addLog(`Tool started: ${execution.tool_name}`);
        get().appendStreamLine(`[tool] ${execution.tool_name} started`, 'tool');
      } else if (update_type === 'completed') {
        const status = execution.success ? 'success' : 'failed';
        get().addLog(`Tool completed: ${execution.tool_name} (${status})`);
        get().appendStreamLine(`[tool] ${execution.tool_name} ${status}`, execution.success ? 'success' : 'error');
      }
    });
    if (!registerListener(unlistenTool)) return;

    // Listen for session events
    const unlistenSession = await listen<{
      session: { id: string; state: string; error_message?: string };
      update_type: string;
    }>('claude_code:session', (event) => {
      const { session, update_type } = event.payload;

      if (update_type === 'state_changed') {
        if (session.state === 'error') {
          get().appendStreamLine(`Session error: ${session.error_message || 'Unknown error'}`, 'error');
          get().addExecutionError({
            severity: 'error',
            title: 'Session Error',
            description: session.error_message || 'Unknown error',
            suggestedFix: 'The session encountered an error. Try restarting the execution.',
          });
          set({
            status: 'failed',
            apiError: session.error_message || 'Session error',
          });
          get().addLog(`Session error: ${session.error_message || 'Unknown error'}`);
        } else if (session.state === 'cancelled') {
          get().appendStreamLine('Session cancelled.', 'warning');
          set({ status: 'idle' });
          get().addLog('Session cancelled');
        }
      }
    });
    if (!registerListener(unlistenSession)) return;

    // Listen for unified stream events (from the unified streaming service)
    const unlistenUnified = await listen<UnifiedEventPayload>('execution:unified_stream', (event) => {
      handleUnifiedExecutionEvent(event.payload, get, set);
    });
    if (!registerListener(unlistenUnified)) return;

    // Standalone command streaming channel (used by execute_standalone).
    const unlistenStandalone = await listen<UnifiedEventPayload>('standalone-event', (event) => {
      handleUnifiedExecutionEvent(event.payload, get, set);
    });
    if (!registerListener(unlistenStandalone)) return;
  } catch (error) {
    console.error('Failed to set up Tauri event listeners:', error);
  }
}

export default useExecutionStore;
