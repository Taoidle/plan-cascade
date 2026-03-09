import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { StreamEventPayload } from '../../lib/claudeCodeClient';
import { reportNonFatal } from '../../lib/nonFatal';
import { ToolCallStreamFilter } from '../../utils/toolCallFilter';
import { useModeStore } from '../mode';
import { useSettingsStore } from '../settings';
import { useToolPermissionStore } from '../toolPermission';
import { useExecutionStore } from '../execution';
import { useWorkflowKernelStore } from '../workflowKernel';
import { formatToolArgs } from './messageDispatch';
import { clearPendingDeltas, flushPendingDeltas, getPending, scheduleFlush } from './streamDeltas';
import {
  appendToBackgroundSession,
  findBackgroundSessionByTaskId,
  isForegroundSession,
  updateBackgroundSessionByTaskId,
} from './sessionRouting';
import { buildExecutionRuntimeHandleId } from './runtimeRegistryActions';
import type {
  AnalysisCoverageSnapshot,
  BackendUsageStats,
  ExecutionRuntimeHandle,
  ExecutionState,
  ExecutionStatus,
  StreamLine,
} from './types';
import { selectStableConversationLines } from '../../lib/conversationUtils';

let unlisteners: UnlistenFn[] = [];
let listenerSetupVersion = 0;
let lateEventDroppedCount = 0;
type ChatRuntimeSource = 'claude' | 'standalone';

export function resetExecutionEventListenerState(): void {
  lateEventDroppedCount = 0;
}

export function cleanupExecutionEventListeners(): void {
  listenerSetupVersion += 1;
  for (const unlisten of unlisteners) {
    unlisten();
  }
  unlisteners = [];
  lateEventDroppedCount = 0;
}

interface UnifiedEventPayload {
  type: string;
  execution_id?: string;
  run_id?: string;
  run_dir?: string;
  request?: string;
  request_id?: string;
  session_id?: string;
  content?: string;
  phase_id?: string;
  title?: string;
  objective?: string;
  prompt?: string;
  sub_agent_id?: string;
  subagent_type?: string;
  depth?: number;
  event_type?: string;
  event_data?: Record<string, unknown>;
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
  risk?: string;
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

function parseMetricNumber(source: Record<string, unknown> | undefined, key: string): number | undefined {
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

function cloneStreamLines(lines: StreamLine[]): StreamLine[] {
  return lines.map((line) => ({ ...line }));
}

function getCachedChatTranscript(rootSessionId: string | null): StreamLine[] {
  if (!rootSessionId) return [];
  return useWorkflowKernelStore.getState().getCachedModeTranscript(rootSessionId, 'chat').lines as StreamLine[];
}

function resolveChatRuntimeSource(state: ExecutionState, rawSessionId: string): ChatRuntimeSource | null {
  const normalizedRawSessionId = rawSessionId.trim();
  if (!normalizedRawSessionId) return null;

  if (state.taskId === normalizedRawSessionId) return 'claude';
  if (state.standaloneSessionId === normalizedRawSessionId) return 'standalone';

  const backgroundMatch = findBackgroundSessionByTaskId(state, normalizedRawSessionId);
  if (backgroundMatch?.snapshot.taskId === normalizedRawSessionId) return 'claude';
  if (backgroundMatch?.snapshot.standaloneSessionId === normalizedRawSessionId) return 'standalone';

  if (state.runtimeRegistry[buildExecutionRuntimeHandleId('claude', normalizedRawSessionId)]) return 'claude';
  if (state.runtimeRegistry[buildExecutionRuntimeHandleId('standalone', normalizedRawSessionId)]) return 'standalone';

  const kernel = useWorkflowKernelStore.getState();
  const linkedModeSessionId = kernel.session?.linkedModeSessions?.chat ?? null;
  if (linkedModeSessionId === `claude:${normalizedRawSessionId}`) return 'claude';
  if (linkedModeSessionId === `standalone:${normalizedRawSessionId}`) return 'standalone';

  for (const item of kernel.sessionCatalog) {
    const bindingSessionId = item.modeRuntimeMeta?.chat?.bindingSessionId ?? null;
    if (bindingSessionId === `claude:${normalizedRawSessionId}`) return 'claude';
    if (bindingSessionId === `standalone:${normalizedRawSessionId}`) return 'standalone';
  }

  return null;
}

function upsertBackgroundChatRuntime(
  state: ExecutionState,
  rawSessionId: string,
  updater: (runtime: ExecutionRuntimeHandle | null) => Partial<ExecutionRuntimeHandle>,
  source: ChatRuntimeSource = 'claude',
): Partial<ExecutionState> {
  const handleId = buildExecutionRuntimeHandleId(source, rawSessionId);
  const existing = state.runtimeRegistry[handleId] ?? null;
  const rootSessionId = existing?.rootSessionId ?? resolveChatRootSessionId(rawSessionId, source);
  if (!existing && !rootSessionId) return {};
  const baseLines = selectStableConversationLines(
    existing?.streamingOutput ?? [],
    getCachedChatTranscript(rootSessionId),
  );

  const settings = useSettingsStore.getState();
  const patch = updater(existing);
  const base: ExecutionRuntimeHandle = existing ?? {
    id: handleId,
    source,
    rawSessionId,
    rootSessionId,
    mode: 'chat',
    status: 'idle',
    streamingOutput: cloneStreamLines(baseLines),
    streamLineCounter: baseLines.reduce((max, line) => Math.max(max, line.id), 0),
    currentTurnStartLineId: 0,
    standaloneTurns: [],
    latestUsage: null,
    sessionUsageTotals: null,
    startedAt: Date.now(),
    workspacePath: settings.workspacePath ?? null,
    llmBackend: settings.backend,
    llmProvider: settings.provider,
    llmModel: settings.model,
    updatedAt: Date.now(),
  };
  const next: ExecutionRuntimeHandle = {
    ...base,
    ...patch,
    rootSessionId: patch.rootSessionId ?? base.rootSessionId ?? null,
    updatedAt: Date.now(),
  };
  return {
    runtimeRegistry: {
      ...state.runtimeRegistry,
      [handleId]: next,
    },
  };
}

function appendToBackgroundChatRuntime(
  state: ExecutionState,
  rawSessionId: string,
  content: string,
  type: 'text' | 'tool' | 'error' | 'success',
  source: ChatRuntimeSource = 'claude',
): Partial<ExecutionState> {
  return upsertBackgroundChatRuntime(
    state,
    rawSessionId,
    (runtime) => {
      const lines = runtime?.streamingOutput ?? [];
      const last = lines.length > 0 ? lines[lines.length - 1] : null;
      if ((type === 'text' || type === 'tool') && last && last.type === type) {
        const updated = { ...last, content: last.content + content };
        const nextLines = lines.slice();
        nextLines[nextLines.length - 1] = updated;
        return {
          streamingOutput: nextLines,
          streamLineCounter: runtime?.streamLineCounter ?? nextLines.length,
        };
      }
      const nextId = (runtime?.streamLineCounter ?? 0) + 1;
      return {
        streamingOutput: [
          ...lines,
          {
            id: nextId,
            content,
            type,
            timestamp: Date.now(),
          },
        ],
        streamLineCounter: nextId,
      };
    },
    source,
  );
}

function upsertForegroundChatRuntimeFromState(state: ExecutionState): Partial<ExecutionState> {
  const source = state.taskId ? 'claude' : state.standaloneSessionId ? 'standalone' : null;
  const rawSessionId = state.taskId ?? state.standaloneSessionId;
  if (!source || !rawSessionId) return {};
  const settings = useSettingsStore.getState();
  const handleId = buildExecutionRuntimeHandleId(source, rawSessionId);
  const rootSessionId =
    state.runtimeRegistry[handleId]?.rootSessionId ?? resolveChatRootSessionId(rawSessionId, source);
  const stableLines = selectStableConversationLines(
    state.streamingOutput,
    getCachedChatTranscript(rootSessionId ?? null),
  );
  return {
    runtimeRegistry: {
      ...state.runtimeRegistry,
      [handleId]: {
        id: handleId,
        source,
        rawSessionId,
        rootSessionId: rootSessionId ?? null,
        mode: 'chat',
        status: state.status,
        streamingOutput: cloneStreamLines(stableLines),
        streamLineCounter: stableLines.reduce((max, line) => Math.max(max, line.id), 0),
        currentTurnStartLineId: state.currentTurnStartLineId,
        standaloneTurns: [...state.standaloneTurns],
        latestUsage: state.latestUsage ? { ...state.latestUsage } : null,
        sessionUsageTotals: state.sessionUsageTotals ? { ...state.sessionUsageTotals } : null,
        startedAt: state.startedAt,
        workspacePath: settings.workspacePath ?? null,
        llmBackend: settings.backend,
        llmProvider: settings.provider,
        llmModel: settings.model,
        updatedAt: Date.now(),
      },
    },
    activeRuntimeHandleId: handleId,
  };
}

function resolveChatRootSessionId(rawSessionId: string, sourceHint?: ChatRuntimeSource | null): string | null {
  const candidateSources: ChatRuntimeSource[] = sourceHint ? [sourceHint] : ['claude', 'standalone'];
  const executionState = useExecutionStore.getState();
  for (const source of candidateSources) {
    const runtimeRootSessionId =
      executionState.runtimeRegistry[buildExecutionRuntimeHandleId(source, rawSessionId)]?.rootSessionId ?? null;
    if (runtimeRootSessionId) {
      return runtimeRootSessionId;
    }
  }

  const kernel = useWorkflowKernelStore.getState();
  for (const source of candidateSources) {
    const modeSessionId = `${source}:${rawSessionId}`;
    if (kernel.session?.linkedModeSessions?.chat === modeSessionId) {
      return kernel.session.sessionId;
    }
    for (const item of kernel.sessionCatalog) {
      const bindingSessionId = item.modeRuntimeMeta?.chat?.bindingSessionId ?? null;
      if (bindingSessionId === modeSessionId) {
        return item.sessionId;
      }
    }
  }

  const modeSessionIds = candidateSources.map((source) => `${source}:${rawSessionId}`);
  reportNonFatal('execution.resolveChatRootSessionId.unresolved', new Error('Unresolved chat runtime root session'), {
    rawSessionId,
    modeSessionIds,
  });
  return null;
}

function scheduleForegroundChatTranscriptSync(rawSessionId: string, get: () => ExecutionState): void {
  const state = get();
  const source = resolveChatRuntimeSource(state, rawSessionId);
  if (!source) return;
  const runtimePatch = upsertForegroundChatRuntimeFromState(state);
  if (Object.keys(runtimePatch).length > 0) {
    useExecutionStore.setState(runtimePatch);
  }
}

function clearPermissionRequestsForSession(rawSessionId?: string | null): void {
  const normalizedRawSessionId = rawSessionId?.trim() ?? '';
  if (!normalizedRawSessionId) return;
  useToolPermissionStore.getState().clearSessionRequests(normalizedRawSessionId);
}

function scheduleBackgroundChatTranscriptSync(
  rawSessionId: string,
  get: () => ExecutionState,
  sourceHint?: ChatRuntimeSource | null,
): void {
  const source = sourceHint ?? resolveChatRuntimeSource(get(), rawSessionId);
  const handleId = source ? buildExecutionRuntimeHandleId(source, rawSessionId) : null;
  const runtime = handleId ? get().runtimeRegistry[handleId] : null;
  const found = findBackgroundSessionByTaskId(get(), rawSessionId);
  if (!runtime && !found?.snapshot.isChatSession) return;
}

function handleUnifiedExecutionEvent(
  payload: UnifiedEventPayload,
  get: () => ExecutionState,
  set: (partial: Partial<ExecutionState>) => void,
) {
  const state = get();
  const isTerminalWhileCancelling =
    payload.type === 'complete' || payload.type === 'error' || payload.type === 'session_complete';
  if (state.isCancelling && isForegroundSession(state, payload.session_id) && !isTerminalWhileCancelling) {
    return;
  }
  if (isForegroundSession(state, payload.session_id) && state.activeExecutionId) {
    if (!payload.execution_id || payload.execution_id !== state.activeExecutionId) {
      lateEventDroppedCount += 1;
      if (lateEventDroppedCount === 1 || lateEventDroppedCount % 20 === 0) {
        console.debug(
          `[execution] dropped late unified event #${lateEventDroppedCount} for session=${payload.session_id}, execution=${payload.execution_id || 'none'}, active=${state.activeExecutionId}`,
        );
      }
      return;
    }
  }

  // Session isolation: route events from non-foreground sessions to
  // their background snapshot instead of the foreground UI.
  if (payload.session_id && !isForegroundSession(state, payload.session_id)) {
    const bgSessionId = payload.session_id;
    const runtimeSource = resolveChatRuntimeSource(get(), bgSessionId);
    const handleId = runtimeSource ? buildExecutionRuntimeHandleId(runtimeSource, bgSessionId) : null;
    const hasRuntimeHandle =
      Boolean(handleId && get().runtimeRegistry[handleId]) ||
      Boolean(resolveChatRootSessionId(bgSessionId, runtimeSource));
    switch (payload.type) {
      case 'text_delta': {
        const found = findBackgroundSessionByTaskId(get(), bgSessionId);
        if (!found && !hasRuntimeHandle) return;
        const filter = found?.snapshot.toolCallFilter ?? new ToolCallStreamFilter();
        const filterResult = filter.processChunk(payload.content || '');
        if (filterResult.output) {
          if (hasRuntimeHandle) {
            const runtimeUpd = appendToBackgroundChatRuntime(
              get(),
              bgSessionId,
              filterResult.output,
              'text',
              runtimeSource ?? 'claude',
            );
            if (Object.keys(runtimeUpd).length > 0) set(runtimeUpd);
          }
          if (found) {
            const legacyUpd = appendToBackgroundSession(get(), bgSessionId, filterResult.output, 'text');
            if (Object.keys(legacyUpd).length > 0) set(legacyUpd);
          }
        }
        if (filterResult.toolIndicator) {
          if (hasRuntimeHandle) {
            const runtimeUpd = appendToBackgroundChatRuntime(
              get(),
              bgSessionId,
              filterResult.toolIndicator,
              'tool',
              runtimeSource ?? 'claude',
            );
            if (Object.keys(runtimeUpd).length > 0) set(runtimeUpd);
          }
          if (found) {
            const legacyUpd = appendToBackgroundSession(get(), bgSessionId, filterResult.toolIndicator, 'tool');
            if (Object.keys(legacyUpd).length > 0) set(legacyUpd);
          }
        }
        scheduleBackgroundChatTranscriptSync(bgSessionId, get, runtimeSource);
        break;
      }
      case 'tool_start':
        if (payload.tool_name) {
          if (hasRuntimeHandle) {
            set(
              appendToBackgroundChatRuntime(
                get(),
                bgSessionId,
                `[tool] ${payload.tool_name} started`,
                'tool',
                runtimeSource ?? 'claude',
              ),
            );
          }
          const legacyUpd = appendToBackgroundSession(
            get(),
            bgSessionId,
            `[tool] ${payload.tool_name} started`,
            'tool',
          );
          if (Object.keys(legacyUpd).length > 0) set(legacyUpd);
          scheduleBackgroundChatTranscriptSync(bgSessionId, get, runtimeSource);
        }
        break;
      case 'tool_result': {
        const isErr = !!payload.error;
        if (hasRuntimeHandle) {
          set(
            appendToBackgroundChatRuntime(
              get(),
              bgSessionId,
              `[tool] ${payload.tool_id || ''} ${isErr ? 'failed' : 'completed'}`,
              isErr ? 'error' : 'success',
              runtimeSource ?? 'claude',
            ),
          );
        }
        const legacyUpd = appendToBackgroundSession(
          get(),
          bgSessionId,
          `[tool] ${payload.tool_id || ''} ${isErr ? 'failed' : 'completed'}`,
          isErr ? 'error' : 'success',
        );
        if (Object.keys(legacyUpd).length > 0) set(legacyUpd);
        scheduleBackgroundChatTranscriptSync(bgSessionId, get, runtimeSource);
        break;
      }
      case 'error':
        if (payload.message) {
          if (hasRuntimeHandle) {
            set(appendToBackgroundChatRuntime(get(), bgSessionId, payload.message, 'error', runtimeSource ?? 'claude'));
            set(
              upsertBackgroundChatRuntime(
                get(),
                bgSessionId,
                () => ({
                  status: 'failed' as ExecutionStatus,
                }),
                runtimeSource ?? 'claude',
              ),
            );
          }
          set(appendToBackgroundSession(get(), bgSessionId, payload.message, 'error'));
          set(updateBackgroundSessionByTaskId(get(), bgSessionId, () => ({ status: 'failed' as ExecutionStatus })));
          scheduleBackgroundChatTranscriptSync(bgSessionId, get, runtimeSource);
        }
        break;
      case 'complete': {
        // Flush the per-session tool-call filter
        const bgFound = findBackgroundSessionByTaskId(get(), bgSessionId);
        if (bgFound) {
          const bgFlushed = bgFound.snapshot.toolCallFilter.flush();
          if (bgFlushed) {
            set(appendToBackgroundSession(get(), bgSessionId, bgFlushed, 'text'));
          }
        }
        // Mark background session as completed
        const bgFoundAfter = findBackgroundSessionByTaskId(get(), bgSessionId);
        if (hasRuntimeHandle) {
          set(
            upsertBackgroundChatRuntime(
              get(),
              bgSessionId,
              (runtime) => ({
                status: runtime ? 'idle' : ('idle' as ExecutionStatus),
              }),
              runtimeSource ?? 'claude',
            ),
          );
        }
        if (bgFoundAfter) {
          const isBoundChatRuntime = Boolean(runtimeSource && resolveChatRootSessionId(bgSessionId, runtimeSource));
          const nextStatus: ExecutionStatus =
            bgFoundAfter.snapshot.isChatSession || isBoundChatRuntime ? 'idle' : 'completed';
          set(
            updateBackgroundSessionByTaskId(get(), bgSessionId, () => ({
              status: nextStatus,
            })),
          );
        }
        scheduleBackgroundChatTranscriptSync(bgSessionId, get, runtimeSource);
        break;
      }
      case 'tool_permission_request':
        if (payload.request_id && payload.tool_name && payload.session_id) {
          import('../toolPermission').then(({ useToolPermissionStore }) => {
            useToolPermissionStore.getState().enqueueRequest({
              requestId: payload.request_id!,
              sessionId: payload.session_id!,
              toolName: payload.tool_name!,
              arguments: payload.arguments || '{}',
              risk: (payload.risk || 'Dangerous') as 'ReadOnly' | 'SafeWrite' | 'Dangerous',
            });
          });
        }
        break;
      default:
        break;
    }
    return;
  }

  const currentMode = useModeStore.getState().mode;
  const isSimpleMode = currentMode === 'simple';
  const showSubAgent = useSettingsStore.getState().showSubAgentEvents && !isSimpleMode;
  const showAnalysisDetails = useSettingsStore.getState().showSubAgentEvents && !isSimpleMode;

  switch (payload.type) {
    case 'tool_permission_request':
      if (payload.request_id && payload.tool_name && payload.session_id) {
        import('../toolPermission').then(({ useToolPermissionStore }) => {
          useToolPermissionStore.getState().enqueueRequest({
            requestId: payload.request_id!,
            sessionId: payload.session_id!,
            toolName: payload.tool_name!,
            arguments: payload.arguments || '{}',
            risk: (payload.risk || 'Dangerous') as 'ReadOnly' | 'SafeWrite' | 'Dangerous',
          });
        });
      }
      return;
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
          'analysis',
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
        get().appendStreamLine(`[analysis:subagent_plan:${phaseId}] ${subAgentId} (${role})${suffix}`, 'analysis');
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
          'analysis',
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
        if (payload.session_id) {
          scheduleForegroundChatTranscriptSync(payload.session_id, get);
        }
      }
      break;

    case 'text_replace': {
      // Replace root assistant text lines from the CURRENT TURN with a single
      // cleaned trailing answer. This preserves the relative order:
      // thinking -> tool/task activity -> final answer.
      const lines = get().streamingOutput;
      const turnBoundary = get().currentTurnStartLineId;
      const textIndices = lines
        .map((l, i) => (l.type === 'text' && !l.subAgentId && l.id > turnBoundary ? i : -1))
        .filter((i) => i >= 0);
      const currentTurnId =
        [...lines]
          .reverse()
          .find((line) => line.id > turnBoundary && !line.subAgentId && typeof line.turnId === 'number')?.turnId ??
        undefined;
      const cleaned = payload.content || '';
      if (textIndices.length > 0) {
        const textIndexSet = new Set(textIndices);
        const updated = lines.filter((_, i) => !textIndexSet.has(i));
        if (cleaned) {
          const nextLineId = Math.max(get().streamLineCounter, ...updated.map((line) => line.id), 0) + 1;
          updated.push({
            id: nextLineId,
            content: cleaned,
            type: 'text',
            timestamp: Date.now(),
            turnId: currentTurnId,
            turnBoundary: 'assistant',
          });
          set({
            streamingOutput: updated,
            streamLineCounter: Math.max(get().streamLineCounter, nextLineId),
            foregroundDirty: true,
          });
        } else {
          set({ streamingOutput: updated, foregroundDirty: true });
        }
        if (payload.session_id) {
          scheduleForegroundChatTranscriptSync(payload.session_id, get);
        }
      } else if (cleaned) {
        // Some fallback-tool-call turns stream only raw tool-call syntax, which
        // the frontend filter suppresses completely. When the backend later
        // sends a cleaned TextReplace payload, treat it as a fresh assistant
        // text line instead of dropping it.
        get().appendStreamLine(cleaned, 'text', undefined, undefined, {
          turnId: currentTurnId,
          turnBoundary: 'assistant',
        });
        if (payload.session_id) {
          scheduleForegroundChatTranscriptSync(payload.session_id, get);
        }
      }
      break;
    }

    case 'thinking_start':
      if (useSettingsStore.getState().showReasoningOutput) {
        get().appendStreamLine('[thinking...]', 'thinking');
        if (payload.session_id) {
          scheduleForegroundChatTranscriptSync(payload.session_id, get);
        }
      }
      break;

    case 'thinking_delta':
      if (useSettingsStore.getState().showReasoningOutput && payload.content) {
        get().appendStreamLine(payload.content, 'thinking');
        if (payload.session_id) {
          scheduleForegroundChatTranscriptSync(payload.session_id, get);
        }
      }
      break;

    case 'thinking_end':
      break;

    case 'tool_start':
      if (payload.tool_name) {
        const argsPreview = formatToolArgs(payload.tool_name, payload.arguments);
        get().appendStreamLine(`[tool:${payload.tool_name}] ${argsPreview}`, 'tool');
        if (payload.session_id) {
          scheduleForegroundChatTranscriptSync(payload.session_id, get);
        }
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
        const preview = payload.result.length > 500 ? payload.result.substring(0, 500) + '...' : payload.result;
        get().appendStreamLine(`[tool_result:${payload.tool_id || ''}] ${preview}`, 'tool_result');
      }
      if (payload.session_id && (payload.error || payload.result)) {
        scheduleForegroundChatTranscriptSync(payload.session_id, get);
      }
      break;

    case 'sub_agent_event': {
      const innerType = payload.event_type as string;
      const innerData = (payload.event_data || {}) as Record<string, unknown>;
      const subAgentId = payload.sub_agent_id as string;
      const depth = (payload.depth as number) || 0;

      switch (innerType) {
        case 'text_delta': {
          const content = innerData.content as string;
          if (content) {
            const pending = getPending(subAgentId, depth);
            const filterResult = get().toolCallFilter.processChunk(content);
            if (filterResult.output) pending.text += filterResult.output;
            if (filterResult.toolIndicator) {
              // Flush text first, then add tool indicator
              if (pending.text) {
                get().appendStreamLine(pending.text, 'text', subAgentId, depth);
                pending.text = '';
              }
              get().appendStreamLine(filterResult.toolIndicator, 'tool', subAgentId, depth);
              if (payload.session_id) {
                scheduleForegroundChatTranscriptSync(payload.session_id, get);
              }
            }
            scheduleFlush(
              get,
              payload.session_id ? () => scheduleForegroundChatTranscriptSync(payload.session_id!, get) : null,
            );
          }
          break;
        }
        case 'thinking_delta': {
          if (useSettingsStore.getState().showReasoningOutput && innerData.content) {
            const pending = getPending(subAgentId, depth);
            pending.thinking += innerData.content as string;
            scheduleFlush(
              get,
              payload.session_id ? () => scheduleForegroundChatTranscriptSync(payload.session_id!, get) : null,
            );
          }
          break;
        }
        case 'tool_start': {
          flushPendingDeltas(get);
          const argsPreview = formatToolArgs(
            (innerData.tool_name as string) || '',
            (innerData.arguments as string) || '',
          );
          get().appendStreamLine(`[tool:${innerData.tool_name}] ${argsPreview}`, 'tool', subAgentId, depth);
          break;
        }
        case 'tool_result': {
          if (innerData.error) {
            get().appendStreamLine(
              `[tool_error:${innerData.tool_id || ''}] ${innerData.error}`,
              'error',
              subAgentId,
              depth,
            );
          } else if (innerData.result) {
            const result = innerData.result as string;
            const preview = result.length > 500 ? result.substring(0, 500) + '...' : result;
            get().appendStreamLine(
              `[tool_result:${innerData.tool_id || ''}] ${preview}`,
              'tool_result',
              subAgentId,
              depth,
            );
          }
          break;
        }
        case 'error': {
          if (innerData.message) {
            get().appendStreamLine(innerData.message as string, 'error', subAgentId, depth);
          }
          break;
        }
        default:
          break;
      }
      break;
    }

    case 'sub_agent_start':
      if (showSubAgent) {
        const promptPreview = (payload.prompt || '').trim().substring(0, 180);
        const label = promptPreview || payload.sub_agent_id || payload.task_type || 'sub-agent';
        get().appendStreamLine(`[sub_agent:start] ${label}`, 'sub_agent', payload.sub_agent_id, payload.depth);
      }
      break;

    case 'sub_agent_end':
      if (payload.success === false || showSubAgent) {
        const usage = formatSubAgentUsage(payload.usage);
        get().appendStreamLine(
          `[sub_agent:end] ${payload.success ? 'completed' : 'failed'}${usage}`,
          'sub_agent',
          payload.sub_agent_id,
          payload.depth,
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
        const requiredTools = Array.isArray(payload.required_tools) ? payload.required_tools.join(', ') : '';
        const suffix = requiredTools ? ` | required: ${requiredTools}` : '';
        get().appendStreamLine(
          `[analysis:attempt_start:${phaseId}] attempt ${attempt}/${maxAttempts}${suffix}`,
          'analysis',
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
        get().appendStreamLine(`[analysis:evidence:${phaseId}:${state}] ${toolName}: ${summary}${suffix}`, 'analysis');
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
          'analysis',
        );
      } else if (isSimpleMode) {
        const phaseId = toShortText(payload.phase_id, 'phase');
        get().appendStreamLine(
          `[analysis] ${phaseId} ${payload.success ? 'completed' : 'completed (partial)'}`,
          'analysis',
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
        const failurePreview = gateFailures.length > 0 ? ` | ${gateFailures.slice(0, 2).join(' ; ')}` : '';
        get().appendStreamLine(
          `[analysis:attempt_end:${phaseId}] attempt ${attempt} ${payload.success ? 'passed' : 'failed'}${metrics}${failurePreview}`,
          'analysis',
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
        get().appendStreamLine(`[analysis:gate_failure:${phaseId}] attempt ${attempt} | ${reasonText}`, 'analysis');
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
      get().appendStreamLine(`[analysis] ${phaseId} degraded at attempt ${attempt}: ${reasonText}`, 'analysis');
      break;
    }

    case 'analysis_validation': {
      const validationStatus = toShortText(payload.status, 'unknown');
      const issues = Array.isArray(payload.issues) ? payload.issues : [];
      const issuePreview = issues.length > 0 ? ` | ${issues.slice(0, 3).join(' ; ')}` : '';
      get().appendStreamLine(
        `[analysis:validation:${validationStatus}] ${issues.length} issue(s)${issuePreview}`,
        'analysis',
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
      const metrics =
        payload.total_metrics && typeof payload.total_metrics === 'object' ? JSON.stringify(payload.total_metrics) : '';
      const summary = phaseResults.length > 0 ? phaseResults.join(' | ') : 'no phase results';
      const suffix = metrics ? ` | ${metrics}` : '';
      get().appendStreamLine(
        `[analysis:run_summary:${payload.success ? 'success' : 'failed'}] ${summary}${suffix}`,
        'analysis',
      );
      const parsedPhaseMetrics = parsePhaseResultMetrics(phaseResults);
      const totalMetrics =
        payload.total_metrics && typeof payload.total_metrics === 'object' ? payload.total_metrics : undefined;
      const current = get().analysisCoverage;
      const next: AnalysisCoverageSnapshot = {
        runId: current?.runId || toShortText(payload.run_id) || undefined,
        status: payload.success === false ? 'failed' : 'completed',
        successfulPhases: parseMetricNumber(parsedPhaseMetrics, 'successful_phases') ?? current?.successfulPhases ?? 0,
        partialPhases: parseMetricNumber(parsedPhaseMetrics, 'partial_phases') ?? current?.partialPhases ?? 0,
        failedPhases: parseMetricNumber(parsedPhaseMetrics, 'failed_phases') ?? current?.failedPhases ?? 0,
        observedPaths:
          parseMetricNumber(parsedPhaseMetrics, 'observed_paths') ??
          parseMetricNumber(totalMetrics, 'observed_paths') ??
          current?.observedPaths ??
          0,
        inventoryTotalFiles:
          parseMetricNumber(totalMetrics, 'inventory_total_files') ?? current?.inventoryTotalFiles ?? 0,
        sampledReadFiles:
          parseMetricNumber(parsedPhaseMetrics, 'sampled_read_files') ??
          parseMetricNumber(totalMetrics, 'sampled_read_files') ??
          current?.sampledReadFiles ??
          0,
        testFilesTotal: parseMetricNumber(totalMetrics, 'test_files_total') ?? current?.testFilesTotal ?? 0,
        testFilesRead: parseMetricNumber(totalMetrics, 'test_files_read') ?? current?.testFilesRead ?? 0,
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
          parseMetricNumber(totalMetrics, 'observed_test_coverage_ratio') ?? current?.observedTestCoverageRatio ?? 0,
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
      const metrics = payload.metrics && typeof payload.metrics === 'object' ? payload.metrics : undefined;
      const summary = metrics ? formatAnalysisMetrics(metrics) : '';
      if (metrics) {
        const current = get().analysisCoverage;
        set({
          analysisCoverage: {
            runId: current?.runId || toShortText(payload.run_id) || undefined,
            status: current?.status || 'running',
            successfulPhases: parseMetricNumber(metrics, 'successful_phases') ?? current?.successfulPhases ?? 0,
            partialPhases: parseMetricNumber(metrics, 'partial_phases') ?? current?.partialPhases ?? 0,
            failedPhases: parseMetricNumber(metrics, 'failed_phases') ?? current?.failedPhases ?? 0,
            observedPaths: parseMetricNumber(metrics, 'observed_paths') ?? current?.observedPaths ?? 0,
            inventoryTotalFiles:
              parseMetricNumber(metrics, 'inventory_total_files') ?? current?.inventoryTotalFiles ?? 0,
            sampledReadFiles: parseMetricNumber(metrics, 'sampled_read_files') ?? current?.sampledReadFiles ?? 0,
            testFilesTotal: parseMetricNumber(metrics, 'test_files_total') ?? current?.testFilesTotal ?? 0,
            testFilesRead: parseMetricNumber(metrics, 'test_files_read') ?? current?.testFilesRead ?? 0,
            coverageRatio: parseMetricNumber(metrics, 'coverage_ratio') ?? current?.coverageRatio ?? 0,
            sampledReadRatio: parseMetricNumber(metrics, 'sampled_read_ratio') ?? current?.sampledReadRatio ?? 0,
            testCoverageRatio: parseMetricNumber(metrics, 'test_coverage_ratio') ?? current?.testCoverageRatio ?? 0,
            observedTestCoverageRatio:
              parseMetricNumber(metrics, 'observed_test_coverage_ratio') ?? current?.observedTestCoverageRatio ?? 0,
            coverageTargetRatio: parseMetricNumber(metrics, 'coverage_target_ratio') ?? current?.coverageTargetRatio,
            sampledReadTargetRatio:
              parseMetricNumber(metrics, 'sampled_read_target_ratio') ?? current?.sampledReadTargetRatio,
            testCoverageTargetRatio:
              parseMetricNumber(metrics, 'test_coverage_target_ratio') ?? current?.testCoverageTargetRatio,
            validationIssues: current?.validationIssues || [],
            manifestPath: current?.manifestPath,
            reportPath: current?.reportPath,
            updatedAt: Date.now(),
          },
        });
      }
      if (showAnalysisDetails) {
        get().appendStreamLine(`[analysis:coverage] updated${summary}`, 'analysis');
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
        payload.success === false ? 'warning' : 'success',
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
        'analysis',
      );
      break;
    }

    case 'usage':
      if (typeof payload.input_tokens === 'number' && typeof payload.output_tokens === 'number') {
        const payloadRecord = payload as unknown as Record<string, unknown>;
        const usage: BackendUsageStats = {
          input_tokens: payload.input_tokens,
          output_tokens: payload.output_tokens,
          thinking_tokens: typeof payload.thinking_tokens === 'number' ? payload.thinking_tokens : null,
          cache_read_tokens:
            typeof payloadRecord.cache_read_tokens === 'number' ? payloadRecord.cache_read_tokens : null,
          cache_creation_tokens:
            typeof payloadRecord.cache_creation_tokens === 'number' ? payloadRecord.cache_creation_tokens : null,
        };
        const prev = get().sessionUsageTotals;
        const nextTotals: BackendUsageStats = {
          input_tokens: (prev?.input_tokens || 0) + usage.input_tokens,
          output_tokens: (prev?.output_tokens || 0) + usage.output_tokens,
          thinking_tokens: (prev?.thinking_tokens || 0) + (usage.thinking_tokens || 0),
          cache_read_tokens: (prev?.cache_read_tokens || 0) + (usage.cache_read_tokens || 0),
          cache_creation_tokens: (prev?.cache_creation_tokens || 0) + (usage.cache_creation_tokens || 0),
        };
        const prevTurn = get().turnUsageTotals;
        const nextTurnTotals: BackendUsageStats = {
          input_tokens: (prevTurn?.input_tokens || 0) + usage.input_tokens,
          output_tokens: (prevTurn?.output_tokens || 0) + usage.output_tokens,
          thinking_tokens: (prevTurn?.thinking_tokens || 0) + (usage.thinking_tokens || 0),
          cache_read_tokens: (prevTurn?.cache_read_tokens || 0) + (usage.cache_read_tokens || 0),
          cache_creation_tokens: (prevTurn?.cache_creation_tokens || 0) + (usage.cache_creation_tokens || 0),
        };
        set({
          latestUsage: usage,
          sessionUsageTotals: nextTotals,
          turnUsageTotals: nextTurnTotals,
        });
        get().addLog(
          `Usage: in=${payload.input_tokens}, out=${payload.output_tokens}${typeof payload.thinking_tokens === 'number' ? `, thinking=${payload.thinking_tokens}` : ''}`,
        );
      }
      break;

    case 'error':
      if (payload.message) {
        clearPermissionRequestsForSession(payload.session_id);
        get().appendStreamLine(`[error] ${payload.message}`, 'error');
        get().addExecutionError({
          severity: 'error',
          title: 'Stream Error',
          description: payload.message,
          suggestedFix: 'Check the error details and retry if needed.',
        });
        if (payload.session_id) {
          scheduleForegroundChatTranscriptSync(payload.session_id, get);
        }
      }
      break;

    case 'warning':
      if (payload.message) {
        get().appendStreamLine(`[warning] ${payload.message}`, 'warning');
        get().addExecutionError({
          severity: 'warning',
          title: 'Stream Warning',
          description: payload.message,
        });
        if (payload.session_id) {
          scheduleForegroundChatTranscriptSync(payload.session_id, get);
        }
      }
      break;

    case 'complete': {
      clearPermissionRequestsForSession(payload.session_id);
      // Flush any buffered content from the tool-call filter
      const flushedText = get().toolCallFilter.flush();
      if (flushedText) {
        get().appendStreamLine(flushedText, 'text');
      }

      const isForegroundStandaloneTurn =
        !!payload.session_id && state.standaloneSessionId === payload.session_id && !state.isChatSession;

      if (isForegroundStandaloneTurn) {
        scheduleForegroundChatTranscriptSync(payload.session_id!, get);
        break;
      }

      // For standalone one-shot execution, this is the final completion signal.
      if (get().status === 'running' || get().status === 'paused') {
        const completedStories = get().stories.filter((s) => s.status === 'completed').length;
        const totalStories = get().stories.length || 1;
        const duration = Date.now() - (get().startedAt || Date.now());
        const durationStr =
          duration >= 60000
            ? `${Math.floor(duration / 60000)}m ${Math.round((duration % 60000) / 1000)}s`
            : `${Math.round(duration / 1000)}s`;

        set({
          status: 'completed',
          isSubmitting: false,
          isCancelling: false,
          pendingCancelBeforeSessionReady: false,
          activeExecutionId: null,
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
      if (payload.session_id) {
        scheduleForegroundChatTranscriptSync(payload.session_id, get);
      }
      break;
    }

    case 'story_start':
      if (payload.story_id && payload.story_title) {
        get().appendStreamLine(
          `Starting story ${(payload.story_index || 0) + 1}/${payload.total_stories || '?'}: ${payload.story_title}`,
          'info',
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
          success ? 'success' : 'error',
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
          passed ? 'success' : 'warning',
        );
      }
      break;

    case 'context_compaction': {
      const compacted = (payload as unknown as { messages_compacted?: number }).messages_compacted || 0;
      const preserved = (payload as unknown as { messages_preserved?: number }).messages_preserved || 0;
      get().addLog(`Context compaction: ${compacted} messages compacted, ${preserved} preserved`);
      break;
    }

    case 'session_complete':
      if (payload.success !== undefined) {
        clearPermissionRequestsForSession(payload.session_id);
        const isForegroundStandaloneTurn =
          !!payload.session_id && state.standaloneSessionId === payload.session_id && !state.isChatSession;
        if (isForegroundStandaloneTurn) {
          scheduleForegroundChatTranscriptSync(payload.session_id!, get);
          break;
        }
        const completedStories = payload.success
          ? payload.total_stories || get().stories.length
          : get().stories.filter((s) => s.status === 'completed').length;
        const totalStories = payload.total_stories || get().stories.length;

        set({
          status: payload.success ? 'completed' : 'failed',
          isSubmitting: false,
          isCancelling: false,
          pendingCancelBeforeSessionReady: false,
          activeExecutionId: null,
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
          payload.success ? 'success' : 'error',
        );
        get().saveToHistory();
      }
      break;
  }
}

export async function setupExecutionEventListeners(
  get: () => ExecutionState,
  set: (partial: Partial<ExecutionState>) => void,
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
      const { event: streamEvent, session_id, execution_id } = event.payload;
      const state = get();

      const isTerminalWhileCancelling = streamEvent.type === 'complete' || streamEvent.type === 'error';
      if (state.isCancelling && isForegroundSession(state, session_id) && !isTerminalWhileCancelling) {
        return;
      }

      if (execution_id && isForegroundSession(state, session_id)) {
        // Only consume events for the currently ACKed execution_id. This isolates
        // late stream events from cancelled/previous turns.
        if (!state.activeExecutionId || state.activeExecutionId !== execution_id) {
          lateEventDroppedCount += 1;
          if (lateEventDroppedCount === 1 || lateEventDroppedCount % 20 === 0) {
            console.debug(
              `[execution] dropped late stream event #${lateEventDroppedCount} for session=${session_id}, execution=${execution_id}, active=${state.activeExecutionId}`,
            );
          }
          return;
        }
      }

      // ---- Route to background session if not foreground ----
      if (!isForegroundSession(state, session_id)) {
        const hasRuntimeHandle =
          Boolean(get().runtimeRegistry[buildExecutionRuntimeHandleId('claude', session_id)]) ||
          Boolean(resolveChatRootSessionId(session_id));
        switch (streamEvent.type) {
          case 'text_delta': {
            // Use the background snapshot's own ToolCallStreamFilter to keep
            // filtering isolated per session.
            const found = findBackgroundSessionByTaskId(state, session_id);
            if (!found && !hasRuntimeHandle) return;
            const filterResult = (found?.snapshot.toolCallFilter ?? new ToolCallStreamFilter()).processChunk(
              streamEvent.content,
            );
            if (filterResult.output) {
              if (hasRuntimeHandle) {
                const runtimeUpdate = appendToBackgroundChatRuntime(get(), session_id, filterResult.output, 'text');
                if (Object.keys(runtimeUpdate).length > 0) set(runtimeUpdate);
              }
              if (found) {
                const bgUpdate = appendToBackgroundSession(get(), session_id, filterResult.output, 'text');
                if (Object.keys(bgUpdate).length > 0) set(bgUpdate);
              }
            }
            if (filterResult.toolIndicator) {
              if (hasRuntimeHandle) {
                const runtimeUpdate = appendToBackgroundChatRuntime(
                  get(),
                  session_id,
                  filterResult.toolIndicator,
                  'tool',
                );
                if (Object.keys(runtimeUpdate).length > 0) set(runtimeUpdate);
              }
              if (found) {
                const bgUpdate = appendToBackgroundSession(get(), session_id, filterResult.toolIndicator, 'tool');
                if (Object.keys(bgUpdate).length > 0) set(bgUpdate);
              }
            }
            scheduleBackgroundChatTranscriptSync(session_id, get);
            break;
          }
          case 'tool_start':
            if (hasRuntimeHandle) {
              set(appendToBackgroundChatRuntime(get(), session_id, `[tool] ${streamEvent.tool_name} started`, 'tool'));
            }
            set(appendToBackgroundSession(state, session_id, `[tool] ${streamEvent.tool_name} started`, 'tool'));
            scheduleBackgroundChatTranscriptSync(session_id, get);
            break;
          case 'tool_result': {
            const bgIsError = !!streamEvent.error;
            if (hasRuntimeHandle) {
              set(
                appendToBackgroundChatRuntime(
                  get(),
                  session_id,
                  `[tool] ${streamEvent.tool_id} ${bgIsError ? 'failed' : 'completed'}`,
                  bgIsError ? 'error' : 'success',
                ),
              );
            }
            set(
              appendToBackgroundSession(
                get(),
                session_id,
                `[tool] ${streamEvent.tool_id} ${bgIsError ? 'failed' : 'completed'}`,
                bgIsError ? 'error' : 'success',
              ),
            );
            scheduleBackgroundChatTranscriptSync(session_id, get);
            break;
          }
          case 'error':
            clearPermissionRequestsForSession(session_id);
            if (hasRuntimeHandle) {
              set(appendToBackgroundChatRuntime(get(), session_id, streamEvent.message, 'error'));
              set(upsertBackgroundChatRuntime(get(), session_id, () => ({ status: 'failed' as ExecutionStatus })));
            }
            set(appendToBackgroundSession(get(), session_id, streamEvent.message, 'error'));
            set(
              updateBackgroundSessionByTaskId(get(), session_id, () => ({
                status: 'failed' as ExecutionStatus,
              })),
            );
            scheduleBackgroundChatTranscriptSync(session_id, get);
            break;
          case 'complete': {
            clearPermissionRequestsForSession(session_id);
            // Flush the per-session tool-call filter
            const bgFound = findBackgroundSessionByTaskId(get(), session_id);
            if (bgFound) {
              const bgFlushed = bgFound.snapshot.toolCallFilter.flush();
              if (bgFlushed) {
                set(appendToBackgroundSession(get(), session_id, bgFlushed, 'text'));
              }
            }
            // Mark session as idle (chat) or completed
            const bgFoundAfter = findBackgroundSessionByTaskId(get(), session_id);
            if (hasRuntimeHandle) {
              set(upsertBackgroundChatRuntime(get(), session_id, () => ({ status: 'idle' as ExecutionStatus })));
            }
            if (bgFoundAfter) {
              const nextStatus: ExecutionStatus = bgFoundAfter.snapshot.isChatSession ? 'idle' : 'completed';
              set(
                updateBackgroundSessionByTaskId(get(), session_id, () => ({
                  status: nextStatus,
                })),
              );
            }
            scheduleBackgroundChatTranscriptSync(session_id, get);
            break;
          }
          case 'tool_permission_request':
            import('../toolPermission').then(({ useToolPermissionStore }) => {
              useToolPermissionStore.getState().enqueueRequest({
                requestId: streamEvent.request_id,
                sessionId: streamEvent.session_id,
                toolName: streamEvent.tool_name,
                arguments: streamEvent.arguments,
                risk: streamEvent.risk,
              });
            });
            break;
          // thinking_start, thinking_delta: skip for background sessions
          // (not needed for background display)
          default:
            break;
        }
        return;
      }

      // ---- Foreground session processing (existing logic, unchanged) ----
      switch (streamEvent.type) {
        case 'tool_permission_request':
          import('../toolPermission').then(({ useToolPermissionStore }) => {
            useToolPermissionStore.getState().enqueueRequest({
              requestId: streamEvent.request_id,
              sessionId: streamEvent.session_id,
              toolName: streamEvent.tool_name,
              arguments: streamEvent.arguments,
              risk: streamEvent.risk,
            });
          });
          return;
        case 'text_delta': {
          const filterResult = get().toolCallFilter.processChunk(streamEvent.content);
          if (filterResult.toolIndicator) {
            // Flush accumulated text before appending tool indicator
            flushPendingDeltas(get);
            get().appendStreamLine(filterResult.toolIndicator, 'tool');
            scheduleForegroundChatTranscriptSync(session_id, get);
          }
          if (filterResult.output) {
            getPending().text += filterResult.output;
            scheduleFlush(get, () => {
              scheduleForegroundChatTranscriptSync(session_id, get);
            });
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
            getPending().thinking += streamEvent.content;
            scheduleFlush(get, () => {
              scheduleForegroundChatTranscriptSync(session_id, get);
            });
          }
          break;

        case 'tool_start':
          flushPendingDeltas(get);
          get().appendStreamLine(`[tool] ${streamEvent.tool_name} started`, 'tool');
          get().addLog(`Tool started: ${streamEvent.tool_name}`);
          scheduleForegroundChatTranscriptSync(session_id, get);
          break;

        case 'tool_result': {
          const isError = !!streamEvent.error;
          get().appendStreamLine(
            `[tool] ${streamEvent.tool_id} ${isError ? 'failed' : 'completed'}`,
            isError ? 'error' : 'success',
          );
          scheduleForegroundChatTranscriptSync(session_id, get);
          break;
        }

        case 'error':
          clearPermissionRequestsForSession(session_id);
          flushPendingDeltas(get);
          get().appendStreamLine(streamEvent.message, 'error');
          get().addExecutionError({
            severity: 'critical',
            title: 'Execution Failed',
            description: streamEvent.message,
            suggestedFix: 'Check the error details and retry the execution.',
          });
          set({
            status: 'failed',
            isCancelling: false,
            pendingCancelBeforeSessionReady: false,
            activeExecutionId: null,
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
          scheduleForegroundChatTranscriptSync(session_id, get);
          break;

        case 'complete': {
          clearPermissionRequestsForSession(session_id);
          flushPendingDeltas(get);
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
              isCancelling: false,
              pendingCancelBeforeSessionReady: false,
              activeExecutionId: null,
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
              isCancelling: false,
              pendingCancelBeforeSessionReady: false,
              activeExecutionId: null,
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
          scheduleForegroundChatTranscriptSync(session_id, get);
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
      const { execution, update_type, session_id } = event.payload;
      const state = get();

      if (state.isCancelling && isForegroundSession(state, session_id)) {
        return;
      }

      // ---- Route to background session if not foreground ----
      if (!isForegroundSession(state, session_id)) {
        const hasRuntimeHandle =
          Boolean(get().runtimeRegistry[buildExecutionRuntimeHandleId('claude', session_id)]) ||
          Boolean(resolveChatRootSessionId(session_id));
        if (update_type === 'started') {
          if (hasRuntimeHandle) {
            set(appendToBackgroundChatRuntime(get(), session_id, `[tool] ${execution.tool_name} started`, 'tool'));
          }
          set(appendToBackgroundSession(get(), session_id, `[tool] ${execution.tool_name} started`, 'tool'));
          scheduleBackgroundChatTranscriptSync(session_id, get);
        } else if (update_type === 'completed') {
          const bgStatus = execution.success ? 'success' : 'failed';
          if (hasRuntimeHandle) {
            set(
              appendToBackgroundChatRuntime(
                get(),
                session_id,
                `[tool] ${execution.tool_name} ${bgStatus}`,
                execution.success ? 'success' : 'error',
              ),
            );
          }
          set(
            appendToBackgroundSession(
              get(),
              session_id,
              `[tool] ${execution.tool_name} ${bgStatus}`,
              execution.success ? 'success' : 'error',
            ),
          );
          scheduleBackgroundChatTranscriptSync(session_id, get);
        }
        return;
      }

      // ---- Foreground processing (existing logic) ----
      if (update_type === 'started') {
        get().addLog(`Tool started: ${execution.tool_name}`);
        get().appendStreamLine(`[tool] ${execution.tool_name} started`, 'tool');
        scheduleForegroundChatTranscriptSync(session_id, get);
      } else if (update_type === 'completed') {
        const status = execution.success ? 'success' : 'failed';
        get().addLog(`Tool completed: ${execution.tool_name} (${status})`);
        get().appendStreamLine(`[tool] ${execution.tool_name} ${status}`, execution.success ? 'success' : 'error');
        scheduleForegroundChatTranscriptSync(session_id, get);
      }
    });
    if (!registerListener(unlistenTool)) return;

    // Listen for session events
    const unlistenSession = await listen<{
      session: { id: string; state: string; error_message?: string };
      update_type: string;
    }>('claude_code:session', (event) => {
      const { session, update_type } = event.payload;
      const state = get();

      // ---- Route to background session if not foreground ----
      if (!isForegroundSession(state, session.id)) {
        const hasRuntimeHandle =
          Boolean(get().runtimeRegistry[buildExecutionRuntimeHandleId('claude', session.id)]) ||
          Boolean(resolveChatRootSessionId(session.id));
        if (update_type === 'state_changed') {
          if (session.state === 'error') {
            clearPermissionRequestsForSession(session.id);
            const errorMsg = session.error_message || 'Unknown error';
            if (hasRuntimeHandle) {
              set(appendToBackgroundChatRuntime(get(), session.id, `Session error: ${errorMsg}`, 'error'));
              set(upsertBackgroundChatRuntime(get(), session.id, () => ({ status: 'failed' as ExecutionStatus })));
            }
            set(appendToBackgroundSession(get(), session.id, `Session error: ${errorMsg}`, 'error'));
            set(
              updateBackgroundSessionByTaskId(get(), session.id, () => ({
                status: 'failed' as ExecutionStatus,
              })),
            );
            scheduleBackgroundChatTranscriptSync(session.id, get);
          } else if (session.state === 'cancelled') {
            clearPermissionRequestsForSession(session.id);
            if (hasRuntimeHandle) {
              set(appendToBackgroundChatRuntime(get(), session.id, 'Session cancelled.', 'error'));
              set(upsertBackgroundChatRuntime(get(), session.id, () => ({ status: 'idle' as ExecutionStatus })));
            }
            set(appendToBackgroundSession(get(), session.id, 'Session cancelled.', 'warning'));
            set(
              updateBackgroundSessionByTaskId(get(), session.id, () => ({
                status: 'idle' as ExecutionStatus,
              })),
            );
            scheduleBackgroundChatTranscriptSync(session.id, get);
          }
        }
        return;
      }

      // ---- Foreground processing (existing logic) ----
      if (update_type === 'state_changed') {
        if (session.state === 'error') {
          clearPermissionRequestsForSession(session.id);
          get().appendStreamLine(`Session error: ${session.error_message || 'Unknown error'}`, 'error');
          get().addExecutionError({
            severity: 'error',
            title: 'Session Error',
            description: session.error_message || 'Unknown error',
            suggestedFix: 'The session encountered an error. Try restarting the execution.',
          });
          set({
            status: 'failed',
            isCancelling: false,
            pendingCancelBeforeSessionReady: false,
            activeExecutionId: null,
            apiError: session.error_message || 'Session error',
          });
          get().addLog(`Session error: ${session.error_message || 'Unknown error'}`);
          scheduleForegroundChatTranscriptSync(session.id, get);
        } else if (session.state === 'cancelled') {
          clearPermissionRequestsForSession(session.id);
          get().appendStreamLine('Session cancelled.', 'warning');
          clearPendingDeltas();
          set({
            status: 'idle',
            isCancelling: false,
            pendingCancelBeforeSessionReady: false,
            activeExecutionId: null,
          });
          get().addLog('Session cancelled');
          scheduleForegroundChatTranscriptSync(session.id, get);
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
    reportNonFatal('execution.setupExecutionEventListeners', error);
  }
}
