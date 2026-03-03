/**
 * Execution Store (v5.0 Pure Rust Backend)
 *
 * Manages task execution state with real-time updates from Tauri events.
 * Replaces the legacy WebSocket-based approach with Tauri IPC.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore, type Backend } from './settings';
import { ToolCallStreamFilter } from '../utils/toolCallFilter';
import { buildPromptWithAttachments } from '../lib/conversationUtils';
import type { FileAttachmentData } from '../types/attachment';
import type { ContextSourceConfig } from '../types/contextSources';
import { assembleTurnContext, type ContextEnvelope } from '../lib/contextApi';
import { useContextOpsStore } from './contextOps';
import { DEFAULT_PROMPT_TOKEN_BUDGET, resolvePromptTokenBudget } from '../lib/promptTokenBudget';
import { reportNonFatal } from '../lib/nonFatal';
import {
  buildChatConversationTurns as buildChatConversationTurnsFromAssembly,
  buildHandoffManualBlock,
  buildStandaloneContextConversationTurns as buildStandaloneContextConversationTurnsFromAssembly,
  buildStandaloneConversationMessage as buildStandaloneConversationMessageFromAssembly,
  inferInjectedSourceKinds,
  trimStandaloneTurns as trimStandaloneTurnsFromAssembly,
} from './execution/contextAssembly';
import { buildHistorySessionId } from './execution/sessionLifecycle';
import { clearSessionScopedMemory } from './execution/memoryPostProcess';
import {
  cleanupExecutionEventListeners,
  resetExecutionEventListenerState,
  setupExecutionEventListeners,
} from './execution/eventListeners';
import { createConversationActions } from './execution/conversationActions';
import { createStartAction } from './execution/startAction';
import { createHistoryActions } from './execution/historyActions';
import { createSessionTreeActions } from './execution/sessionTreeActions';
import { createMiscActions } from './execution/miscActions';
import { createSessionPersistenceController } from './execution/sessionPersistence';

import type {
  AnalysisCoverageSnapshot,
  BackendUsageStats,
  Batch,
  ConnectionStatus,
  ExecutionError,
  ExecutionHistoryItem,
  ExecutionResult,
  ExecutionState,
  ExecutionStatus,
  QualityGateResult,
  SessionSnapshot,
  StandaloneTurn,
  Strategy,
  StrategyAnalysis,
  StrategyOptionInfo,
  Story,
  StreamLine,
  StreamLineType,
} from './execution/types';

export * from './execution/types';

const HISTORY_KEY = 'plan-cascade-execution-history';
const HISTORY_MIGRATION_KEY = 'plan-cascade-execution-history-migrated-v2';
const SESSION_STATE_KEY = 'plan-cascade-execution-sessions-v1';
const MAX_HISTORY_ITEMS = 200;
const DEFAULT_STANDALONE_CONTEXT_TURNS = -1;
const STANDALONE_CONTEXT_UNLIMITED = -1;

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

function isCommandResponse<T>(value: unknown): value is CommandResponse<T> {
  if (typeof value !== 'object' || value === null) return false;
  return 'success' in value && 'data' in value && 'error' in value;
}

interface AttachmentContextInput {
  name: string;
  path: string;
  size: number;
  type: FileAttachmentData['type'];
  content?: string;
  preview?: string;
}

interface AttachmentContextPrepareResult {
  prepared_prompt: string;
  included_files: string[];
  skipped_files: { name: string; path: string; reason: string }[];
  prompt_tokens: number;
  attachment_tokens: number;
  total_tokens: number;
  budget_tokens: number;
  exceeds_budget: boolean;
  truncated: boolean;
}

type ContextEnvelopeV2 = ContextEnvelope;

interface ContextConversationTurnInput {
  role: 'user' | 'assistant';
  content: string;
}

interface BackendStandaloneExecutionResult {
  response?: string | null;
  usage: BackendUsageStats;
  iterations: number;
  success: boolean;
  error?: string | null;
}

interface SessionLlmSettings {
  llmBackend?: string;
  llmProvider?: string;
  llmModel?: string;
}

async function listHistoryFromSQLite(limit = MAX_HISTORY_ITEMS): Promise<ExecutionHistoryItem[] | null> {
  try {
    const response = await invoke<CommandResponse<ExecutionHistoryItem[]>>('list_execution_history', { limit });
    if (!isCommandResponse<ExecutionHistoryItem[]>(response)) return null;
    const result = response;
    if (!result.success || !result.data) return null;
    return result.data;
  } catch (error) {
    reportNonFatal('execution.listHistoryFromSQLite', error, { limit });
    return null;
  }
}

async function upsertHistoryToSQLite(item: ExecutionHistoryItem): Promise<void> {
  try {
    await invoke<CommandResponse<ExecutionHistoryItem>>('upsert_execution_history', { item });
  } catch (error) {
    reportNonFatal('execution.upsertHistoryToSQLite', error, { historyId: item.id });
  }
}

async function importHistoryToSQLite(items: ExecutionHistoryItem[]): Promise<boolean> {
  if (items.length === 0) return true;
  try {
    const result = await invoke<CommandResponse<number>>('import_execution_history', { items });
    return Boolean(result.success);
  } catch (error) {
    reportNonFatal('execution.importHistoryToSQLite', error, { count: items.length });
    return false;
  }
}

async function deleteHistoryFromSQLite(historyId: string): Promise<void> {
  try {
    await invoke<CommandResponse<boolean>>('delete_execution_history', {
      historyId,
      history_id: historyId,
    });
  } catch (error) {
    reportNonFatal('execution.deleteHistoryFromSQLite', error, { historyId });
  }
}

async function renameHistoryInSQLite(historyId: string, title?: string): Promise<void> {
  try {
    await invoke<CommandResponse<boolean>>('rename_execution_history', {
      historyId,
      history_id: historyId,
      title: title?.trim().length ? title.trim() : null,
    });
  } catch (error) {
    reportNonFatal('execution.renameHistoryInSQLite', error, { historyId });
  }
}

async function clearHistoryInSQLite(): Promise<void> {
  try {
    await invoke<CommandResponse<boolean>>('clear_execution_history');
  } catch (error) {
    reportNonFatal('execution.clearHistoryInSQLite', error);
  }
}

function loadLegacyHistoryFromLocalStorage(): ExecutionHistoryItem[] {
  try {
    const stored = localStorage.getItem(HISTORY_KEY);
    if (!stored) return [];
    const parsed = JSON.parse(stored) as ExecutionHistoryItem[];
    return Array.isArray(parsed) ? parsed : [];
  } catch (error) {
    reportNonFatal('execution.loadLegacyHistoryFromLocalStorage', error);
    return [];
  }
}

function markHistoryMigrationDone(): void {
  try {
    localStorage.setItem(HISTORY_MIGRATION_KEY, '1');
    localStorage.removeItem(HISTORY_KEY);
  } catch (error) {
    reportNonFatal('execution.markHistoryMigrationDone', error);
  }
}

function isHistoryMigrationDone(): boolean {
  try {
    return localStorage.getItem(HISTORY_MIGRATION_KEY) === '1';
  } catch (error) {
    reportNonFatal('execution.isHistoryMigrationDone', error);
    return false;
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
  return trimStandaloneTurnsFromAssembly(turns, limit, STANDALONE_CONTEXT_UNLIMITED);
}

function buildStandaloneConversationMessage(
  turns: StandaloneTurn[],
  userInput: string,
  contextTurnsLimit: number,
): string {
  return buildStandaloneConversationMessageFromAssembly(
    turns,
    userInput,
    contextTurnsLimit,
    STANDALONE_CONTEXT_UNLIMITED,
  );
}

function buildContextConversationTurns(
  turns: StandaloneTurn[],
  contextTurnsLimit: number,
): ContextConversationTurnInput[] {
  return buildStandaloneContextConversationTurnsFromAssembly(turns, contextTurnsLimit, STANDALONE_CONTEXT_UNLIMITED);
}

async function buildStandaloneMessageWithContextEnvelope(params: {
  query: string;
  turns: StandaloneTurn[];
  contextTurnsLimit: number;
  projectPath: string;
  sessionId: string | null;
  contextSources: ContextSourceConfig | null;
  addLog: (message: string) => void;
}): Promise<{ message: string; injectedSourceKinds: string[]; externalContextInjected: boolean }> {
  const fallbackMessage =
    trimStandaloneTurns(params.turns, params.contextTurnsLimit).length > 0
      ? buildStandaloneConversationMessage(params.turns, params.query, params.contextTurnsLimit)
      : params.query;
  const fallbackInjectedSourceKinds = inferInjectedSourceKinds({
    hasHistory: trimStandaloneTurns(params.turns, params.contextTurnsLimit).length > 0,
    contextSources: params.contextSources,
  });
  const conversationHistory = buildContextConversationTurns(params.turns, params.contextTurnsLimit);
  const handoffCapsule = buildHandoffManualBlock(conversationHistory);
  const settings = useSettingsStore.getState();
  const hardLimit = await resolvePromptTokenBudget({
    backend: settings.backend,
    provider: settings.provider,
    model: settings.model,
    fallbackBudget: DEFAULT_PROMPT_TOKEN_BUDGET,
  });
  const reservedOutputTokens = Math.max(2_048, Math.round(hardLimit * 0.2));
  const inputTokenBudget = Math.max(256, hardLimit - reservedOutputTokens);

  const request = {
    project_path: params.projectPath,
    query: params.query,
    session_id: params.sessionId ? `standalone:${params.sessionId}` : undefined,
    mode: 'standalone',
    conversation_history: conversationHistory,
    context_sources: params.contextSources ?? undefined,
    manual_blocks: handoffCapsule ? [handoffCapsule] : undefined,
    input_token_budget: inputTokenBudget,
    reserved_output_tokens: reservedOutputTokens,
    hard_limit: hardLimit,
  };

  try {
    const result = await assembleTurnContext(request);
    if (result.success && result.data?.assembled_prompt) {
      const envelope: ContextEnvelopeV2 = {
        request_meta: result.data.request_meta,
        budget: result.data.budget,
        sources: result.data.sources,
        blocks: result.data.blocks,
        compaction: result.data.compaction,
        trace_id: result.data.trace_id,
        assembled_prompt: result.data.assembled_prompt,
        diagnostics: result.data.diagnostics,
      };
      useContextOpsStore.getState().setLatestEnvelope(envelope);
      const used = result.data.budget?.used_input_tokens;
      const total = result.data.budget?.input_token_budget;
      if (typeof used === 'number' && typeof total === 'number') {
        const fallbackTag = result.data.fallback_used ? ', fallback' : '';
        const handoffTag = handoffCapsule ? ', handoff_capsule' : '';
        params.addLog(
          `Context assembled (${used}/${total} tokens, trace=${result.data.trace_id}${fallbackTag}${handoffTag})`,
        );
      } else {
        params.addLog(`Context assembled (trace=${result.data.trace_id}${handoffCapsule ? ', handoff_capsule' : ''})`);
      }
      return {
        message: result.data.assembled_prompt,
        injectedSourceKinds: result.data.injected_source_kinds ?? fallbackInjectedSourceKinds,
        externalContextInjected: true,
      };
    }

    if (result.error) {
      params.addLog(`Context assemble fallback to legacy path: ${result.error}`);
    }
    return {
      message: fallbackMessage,
      injectedSourceKinds: fallbackInjectedSourceKinds,
      externalContextInjected: fallbackInjectedSourceKinds.length > 0,
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    params.addLog(`Context assemble invocation failed, fallback to legacy path: ${message}`);
    return {
      message: fallbackMessage,
      injectedSourceKinds: fallbackInjectedSourceKinds,
      externalContextInjected: fallbackInjectedSourceKinds.length > 0,
    };
  }
}

async function buildClaudePromptWithContextEnvelope(params: {
  query: string;
  lines: StreamLine[];
  projectPath: string;
  sessionId: string | null;
  contextSources: ContextSourceConfig | null;
  addLog: (message: string) => void;
}): Promise<string> {
  const conversationHistory = buildChatConversationTurnsFromAssembly(params.lines);
  const handoffCapsule = buildHandoffManualBlock(conversationHistory);
  const settings = useSettingsStore.getState();
  const hardLimit = await resolvePromptTokenBudget({
    backend: settings.backend,
    provider: settings.provider,
    model: settings.model,
    fallbackBudget: DEFAULT_PROMPT_TOKEN_BUDGET,
  });
  const reservedOutputTokens = Math.max(2_048, Math.round(hardLimit * 0.2));
  const inputTokenBudget = Math.max(256, hardLimit - reservedOutputTokens);
  const request = {
    project_path: params.projectPath,
    query: params.query,
    session_id: params.sessionId ? `claude:${params.sessionId}` : undefined,
    mode: 'chat',
    conversation_history: conversationHistory,
    context_sources: params.contextSources ?? undefined,
    manual_blocks: handoffCapsule ? [handoffCapsule] : undefined,
    input_token_budget: inputTokenBudget,
    reserved_output_tokens: reservedOutputTokens,
    hard_limit: hardLimit,
  };

  try {
    const result = await assembleTurnContext(request);
    if (result.success && result.data?.assembled_prompt) {
      const envelope: ContextEnvelopeV2 = {
        request_meta: result.data.request_meta,
        budget: result.data.budget,
        sources: result.data.sources,
        blocks: result.data.blocks,
        compaction: result.data.compaction,
        trace_id: result.data.trace_id,
        assembled_prompt: result.data.assembled_prompt,
        diagnostics: result.data.diagnostics,
      };
      useContextOpsStore.getState().setLatestEnvelope(envelope);
      const fallbackTag = result.data.fallback_used ? ', fallback' : '';
      const handoffTag = handoffCapsule ? ', handoff_capsule' : '';
      params.addLog(`Context assembled (trace=${result.data.trace_id}${fallbackTag}${handoffTag})`);
      return result.data.assembled_prompt;
    }
  } catch (error) {
    reportNonFatal('execution.buildClaudePromptWithContextEnvelope', error, {
      projectPath: params.projectPath,
      sessionId: params.sessionId,
    });
  }

  return params.query;
}

function hasMeaningfulForegroundContent(state: ExecutionState): boolean {
  return (
    state.streamingOutput.length > 0 ||
    state.taskId != null ||
    state.standaloneSessionId != null ||
    state.taskDescription.trim().length > 0
  );
}

function createSessionSnapshotFromForeground(
  state: ExecutionState,
  settings: ReturnType<typeof useSettingsStore.getState>,
  id: string,
): SessionSnapshot {
  return {
    id,
    taskDescription: state.taskDescription,
    status: state.status,
    streamingOutput: [...state.streamingOutput],
    streamLineCounter: state.streamLineCounter,
    currentTurnStartLineId: state.currentTurnStartLineId,
    taskId: state.taskId,
    isChatSession: state.isChatSession,
    standaloneTurns: [...state.standaloneTurns],
    standaloneSessionId: state.standaloneSessionId,
    latestUsage: state.latestUsage ? { ...state.latestUsage } : null,
    sessionUsageTotals: state.sessionUsageTotals ? { ...state.sessionUsageTotals } : null,
    startedAt: state.startedAt,
    toolCallFilter: state.toolCallFilter,
    llmBackend: settings.backend,
    llmProvider: settings.provider,
    llmModel: settings.model,
    parentSessionId: state.foregroundParentSessionId || undefined,
    workspacePath: settings.workspacePath || undefined,
    originHistoryId: state.foregroundOriginHistoryId || undefined,
    originSessionId:
      state.foregroundOriginSessionId || buildHistorySessionId(state.taskId, state.standaloneSessionId) || undefined,
    updatedAt: Date.now(),
  };
}

function shouldPersistForegroundBeforeSwitch(state: ExecutionState): boolean {
  if (!hasMeaningfulForegroundContent(state)) return false;
  if (state.status === 'running' || state.status === 'paused') return true;
  if (state.taskId || state.standaloneSessionId) return true;
  if (state.foregroundDirty) return true;
  // Restored history sessions should still be represented once in active tree.
  if (state.foregroundOriginHistoryId || state.foregroundOriginSessionId) return true;
  return false;
}

function hasAssistantTextLineSince(lines: StreamLine[], minExclusiveLineId: number): boolean {
  return lines.some((line) => line.id > minExclusiveLineId && line.type === 'text' && line.content.trim().length > 0);
}

function collectAssistantTextSince(lines: StreamLine[], minExclusiveLineId: number): string {
  return lines
    .filter((line) => line.id > minExclusiveLineId && line.type === 'text' && line.content.trim().length > 0)
    .map((line) => line.content)
    .join('')
    .trim();
}

function appendTextWithTypewriter(
  append: (content: string, type: StreamLineType) => void,
  content: string,
  chunkSize = 28,
  delayMs = 14,
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

function toAttachmentContextInputs(attachments: FileAttachmentData[]): AttachmentContextInput[] {
  return attachments.map((attachment) => ({
    name: attachment.name,
    path: attachment.path,
    size: attachment.size,
    type: attachment.type,
    content: attachment.content,
    preview: attachment.preview,
  }));
}

async function preparePromptWithAttachmentContext(
  prompt: string,
  attachments: FileAttachmentData[],
  addLog?: (message: string) => void,
): Promise<string> {
  if (attachments.length === 0) return prompt;

  try {
    const settings = useSettingsStore.getState();
    const budgetTokens = await resolvePromptTokenBudget({
      backend: settings.backend,
      provider: settings.provider,
      model: settings.model,
      fallbackBudget: DEFAULT_PROMPT_TOKEN_BUDGET,
    });
    const maxAttachmentTokens = Math.max(4_000, Math.min(64_000, Math.floor(budgetTokens * 0.4)));
    const maxTokensPerFile = Math.min(12_000, maxAttachmentTokens);

    const result = await invoke<CommandResponse<AttachmentContextPrepareResult>>('prepare_attachment_context', {
      prompt,
      attachments: toAttachmentContextInputs(attachments),
      budgetTokens,
      maxAttachmentTokens,
      maxTokensPerFile,
    });

    if (result.success && result.data) {
      const prepared = result.data;
      if (prepared.truncated) {
        addLog?.('Attachment context was truncated to fit budget');
      }
      if (prepared.skipped_files.length > 0) {
        addLog?.(`Skipped ${prepared.skipped_files.length} attachment(s) due to context budget`);
      }
      if (prepared.exceeds_budget) {
        addLog?.(
          `Prepared prompt exceeds estimated budget (${prepared.total_tokens}/${prepared.budget_tokens} tokens)`,
        );
      }
      return prepared.prepared_prompt;
    }
  } catch (error) {
    reportNonFatal('execution.preparePromptWithAttachmentContext', error, { attachments: attachments.length });
  }

  addLog?.('Falling back to legacy attachment prompt builder');
  return buildPromptWithAttachments(prompt, attachments);
}

function isBackendStandaloneExecutionResult(data: unknown): data is BackendStandaloneExecutionResult {
  if (typeof data !== 'object' || data === null) return false;
  const value = data as { iterations?: unknown; success?: unknown; usage?: unknown };
  return (
    typeof value.iterations === 'number' &&
    typeof value.success === 'boolean' &&
    typeof value.usage === 'object' &&
    value.usage !== null
  );
}

function restoreSessionLlmSettings(settings: SessionLlmSettings): void {
  if (!settings.llmBackend) return;
  useSettingsStore.setState({
    backend: settings.llmBackend as Backend,
    provider: settings.llmProvider || '',
    model: settings.llmModel || '',
  });
}

const sessionPersistence = createSessionPersistenceController({
  sessionStateKey: SESSION_STATE_KEY,
  hasMeaningfulForegroundContent,
  buildHistorySessionId,
});

const initialState = {
  status: 'idle' as ExecutionStatus,
  connectionStatus: 'disconnected' as ConnectionStatus,
  taskId: null as string | null,
  activeExecutionId: null as string | null,
  isCancelling: false,
  pendingCancelBeforeSessionReady: false,
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
  turnUsageTotals: null as BackendUsageStats | null,
  toolCallFilter: new ToolCallStreamFilter(),
  attachments: [] as FileAttachmentData[],
  backgroundSessions: {} as Record<string, SessionSnapshot>,
  activeSessionId: null as string | null,
  foregroundParentSessionId: null as string | null,
  foregroundBgId: null as string | null,
  foregroundOriginHistoryId: null as string | null,
  foregroundOriginSessionId: null as string | null,
  foregroundDirty: false,
  activeAgentId: null as string | null,
  activeAgentName: null as string | null,
  _pendingTaskContext: null as string | null,
};

export const useExecutionStore = create<ExecutionState>()((set, get) => {
  const conversationActions = createConversationActions({
    set,
    get,
    buildClaudePromptWithContextEnvelope,
    buildStandaloneMessageWithContextEnvelope,
    preparePromptWithAttachmentContext,
    getStandaloneContextTurnsLimit,
    trimStandaloneTurns,
    collectAssistantTextSince,
    hasAssistantTextLineSince,
    appendTextWithTypewriter,
    isBackendStandaloneExecutionResult,
  });
  const startAction = createStartAction({
    set,
    get,
    buildClaudePromptWithContextEnvelope,
    buildStandaloneMessageWithContextEnvelope,
    preparePromptWithAttachmentContext,
    getStandaloneContextTurnsLimit,
    trimStandaloneTurns,
    collectAssistantTextSince,
    hasAssistantTextLineSince,
    appendTextWithTypewriter,
    isBackendStandaloneExecutionResult,
    standaloneContextUnlimited: STANDALONE_CONTEXT_UNLIMITED,
  });
  const historyActions = createHistoryActions({
    set,
    get,
    initialState,
    maxHistoryItems: MAX_HISTORY_ITEMS,
    listHistoryFromSQLite,
    upsertHistoryToSQLite,
    importHistoryToSQLite,
    clearHistoryInSQLite,
    deleteHistoryFromSQLite,
    renameHistoryInSQLite,
    isHistoryMigrationDone,
    loadLegacyHistoryFromLocalStorage,
    markHistoryMigrationDone,
    clearSessionScopedMemory,
    buildHistorySessionId,
    createSessionSnapshotFromForeground,
    shouldPersistForegroundBeforeSwitch,
    restoreSessionLlmSettings,
    trimStandaloneTurns,
    getStandaloneContextTurnsLimit,
  });
  const sessionTreeActions = createSessionTreeActions({
    set,
    get,
    hasMeaningfulForegroundContent,
    createSessionSnapshotFromForeground,
    shouldPersistForegroundBeforeSwitch,
  });
  const miscActions = createMiscActions({
    set,
    get,
    initialState,
    hasMeaningfulForegroundContent,
    createSessionSnapshotFromForeground,
    getStandaloneContextTurnsLimit,
    trimStandaloneTurns,
  });

  return {
    ...initialState,

    initialize: () => {
      resetExecutionEventListenerState();
      const persisted = sessionPersistence.load();
      if (persisted) {
        set(persisted.state);
        // Restore foreground workspace path into settings store
        if (persisted.foregroundWorkspacePath) {
          useSettingsStore.setState({ workspacePath: persisted.foregroundWorkspacePath });
        }
        get().addLog('Restored session tree from local cache');
      }

      // In Tauri, we're always "connected" via IPC
      set({ connectionStatus: 'connected' });
      get().addLog('Connected to Rust backend');

      // Set up Tauri event listeners for execution updates
      void setupExecutionEventListeners(get, set);

      // Load history
      get().loadHistory();
    },

    cleanup: () => {
      cleanupExecutionEventListeners();
      sessionPersistence.cancelScheduled();
      set({ connectionStatus: 'disconnected' });
    },

    start: startAction,
    sendFollowUp: conversationActions.sendFollowUp,
    pause: miscActions.pause,
    resume: miscActions.resume,
    cancel: conversationActions.cancel,
    reset: miscActions.reset,

    updateStory: (storyId, updates) => {
      set((state) => ({
        stories: state.stories.map((s) => (s.id === storyId ? { ...s, ...updates } : s)),
      }));

      // Recalculate progress
      const stories = get().stories;
      if (stories.length > 0) {
        const completed = stories.filter((s) => s.status === 'completed').length;
        set({ progress: (completed / stories.length) * 100 });
      }
    },

    addLog: (message) => {
      const timestamp = new Date().toLocaleTimeString('en-GB', { hour12: false });
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
    loadHistory: historyActions.loadHistory,
    saveToHistory: historyActions.saveToHistory,
    clearHistory: historyActions.clearHistory,
    deleteHistory: historyActions.deleteHistory,
    renameHistory: historyActions.renameHistory,
    restoreFromHistory: historyActions.restoreFromHistory,
    analyzeStrategy: miscActions.analyzeStrategy,
    loadStrategyOptions: miscActions.loadStrategyOptions,
    clearStrategyAnalysis: miscActions.clearStrategyAnalysis,
    appendStreamLine: miscActions.appendStreamLine,
    appendCard: miscActions.appendCard,
    clearStreamingOutput: miscActions.clearStreamingOutput,
    updateQualityGate: miscActions.updateQualityGate,
    addExecutionError: miscActions.addExecutionError,
    dismissError: miscActions.dismissError,
    clearExecutionErrors: miscActions.clearExecutionErrors,
    addAttachment: miscActions.addAttachment,
    removeAttachment: miscActions.removeAttachment,
    clearAttachments: miscActions.clearAttachments,

    backgroundCurrentSession: sessionTreeActions.backgroundCurrentSession,
    switchToSession: sessionTreeActions.switchToSession,
    removeBackgroundSession: sessionTreeActions.removeBackgroundSession,

    retryStory: miscActions.retryStory,
    rollbackToTurn: miscActions.rollbackToTurn,

    forkSessionAtTurn: sessionTreeActions.forkSessionAtTurn,
    regenerateResponse: conversationActions.regenerateResponse,
    editAndResend: conversationActions.editAndResend,

    appendStandaloneTurn: miscActions.appendStandaloneTurn,
    setPendingTaskContext: miscActions.setPendingTaskContext,
    clearPendingTaskContext: miscActions.clearPendingTaskContext,
  };
});

if (typeof window !== 'undefined') {
  useExecutionStore.subscribe((state) => {
    sessionPersistence.schedule(state);
  });
}

export default useExecutionStore;
