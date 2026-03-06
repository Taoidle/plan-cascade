import { ToolCallStreamFilter } from '../../utils/toolCallFilter';
import { selectStableConversationLines } from '../../lib/conversationUtils';
import { useWorkflowKernelStore } from '../workflowKernel';
import { useSimpleSessionStore } from '../simpleSessionStore';
import { useSettingsStore, type Backend } from '../settings';
import type { ExecutionRuntimeHandle, ExecutionState, ExecutionStatus } from './types';

interface RuntimeRegistryActions {
  parkForegroundRuntime: () => string | null;
  restoreForegroundChatRuntime: (params: {
    source: 'claude' | 'standalone';
    rawSessionId: string;
    fallbackLines: ExecutionState['streamingOutput'];
    title?: string | null;
    phase?: string | null;
    lastError?: string | null;
  }) => void;
}

export function buildExecutionRuntimeHandleId(source: 'claude' | 'standalone', rawSessionId: string): string {
  return `${source}:${rawSessionId}`;
}

type ExecutionSetState = (
  partial: Partial<ExecutionState> | ((state: ExecutionState) => Partial<ExecutionState>),
) => void;

interface RuntimeRegistryActionDeps {
  set: ExecutionSetState;
  get: () => ExecutionState;
  hasMeaningfulForegroundContent: (state: ExecutionState) => boolean;
}

function buildResetForegroundPatch(): Partial<ExecutionState> {
  return {
    foregroundParentSessionId: null,
    foregroundBgId: null,
    foregroundOriginHistoryId: null,
    foregroundOriginSessionId: null,
    foregroundDirty: false,
    taskDescription: '',
    status: 'idle' as ExecutionStatus,
    isCancelling: false,
    pendingCancelBeforeSessionReady: false,
    activeExecutionId: null,
    streamingOutput: [],
    streamLineCounter: 0,
    currentTurnStartLineId: 0,
    taskId: null,
    isChatSession: false,
    standaloneTurns: [],
    standaloneSessionId: null,
    latestUsage: null,
    sessionUsageTotals: null,
    turnUsageTotals: null,
    startedAt: null,
    result: null,
    apiError: null,
    isSubmitting: false,
    toolCallFilter: new ToolCallStreamFilter(),
    attachments: [],
  };
}

function restoreSessionLlmSettings(settings: { llmBackend?: string; llmProvider?: string; llmModel?: string }): void {
  if (!settings.llmBackend) return;
  useSettingsStore.setState({
    backend: settings.llmBackend as Backend,
    provider: settings.llmProvider || '',
    model: settings.llmModel || '',
  });
}

function cloneLines(lines: ExecutionState['streamingOutput']): ExecutionState['streamingOutput'] {
  return lines.map((line) => ({ ...line }));
}

function resolveRuntimeRootSessionId(): string | null {
  const kernel = useWorkflowKernelStore.getState();
  return kernel.activeRootSessionId ?? kernel.session?.sessionId ?? null;
}

function buildRuntimeHandleFromForeground(state: ExecutionState): ExecutionRuntimeHandle | null {
  const source = state.taskId ? 'claude' : state.standaloneSessionId ? 'standalone' : null;
  const rawSessionId = state.taskId ?? state.standaloneSessionId;
  if (!source || !rawSessionId) return null;

  const settingsState = useSettingsStore.getState();
  const rootSessionId = resolveRuntimeRootSessionId();
  const fallbackLines = rootSessionId
    ? (useSimpleSessionStore.getState().getModeLines(rootSessionId, 'chat') as ExecutionState['streamingOutput'])
    : [];
  const stableLines = selectStableConversationLines(state.streamingOutput, fallbackLines);
  return {
    id: buildExecutionRuntimeHandleId(source, rawSessionId),
    source,
    rawSessionId,
    rootSessionId,
    mode: 'chat',
    status: state.status,
    streamingOutput: cloneLines(stableLines),
    streamLineCounter: stableLines.reduce((max, line) => Math.max(max, line.id), 0),
    currentTurnStartLineId: state.currentTurnStartLineId,
    standaloneTurns: [...state.standaloneTurns],
    latestUsage: state.latestUsage ? { ...state.latestUsage } : null,
    sessionUsageTotals: state.sessionUsageTotals ? { ...state.sessionUsageTotals } : null,
    startedAt: state.startedAt,
    workspacePath: settingsState.workspacePath ?? null,
    llmBackend: settingsState.backend,
    llmProvider: settingsState.provider,
    llmModel: settingsState.model,
    updatedAt: Date.now(),
  };
}

export function createRuntimeRegistryActions(deps: RuntimeRegistryActionDeps): RuntimeRegistryActions {
  const { set, get, hasMeaningfulForegroundContent } = deps;

  return {
    parkForegroundRuntime: () => {
      const state = get();
      const hasForegroundContent = hasMeaningfulForegroundContent(state);
      if (!hasForegroundContent) {
        set(buildResetForegroundPatch());
        return null;
      }

      const handle = buildRuntimeHandleFromForeground(state);
      if (!handle) {
        set(buildResetForegroundPatch());
        return null;
      }

      set({
        runtimeRegistry: {
          ...state.runtimeRegistry,
          [handle.id]: handle,
        },
        activeRuntimeHandleId: handle.id,
        ...buildResetForegroundPatch(),
      });
      return handle.id;
    },

    restoreForegroundChatRuntime: ({ source, rawSessionId, fallbackLines, title, phase, lastError }) => {
      const state = get();
      const handleId = buildExecutionRuntimeHandleId(source, rawSessionId);
      const matchedRuntime = state.runtimeRegistry[handleId] ?? null;
      const lines = selectStableConversationLines(matchedRuntime?.streamingOutput ?? [], fallbackLines);
      const nextStreamLineCounter = lines.reduce((max, line) => Math.max(max, line.id), 0);
      const nextStatus =
        phase === 'streaming' || phase === 'submitting'
          ? ('running' as ExecutionStatus)
          : phase === 'paused'
            ? ('paused' as ExecutionStatus)
            : phase === 'failed'
              ? ('failed' as ExecutionStatus)
              : ('idle' as ExecutionStatus);

      set({
        ...buildResetForegroundPatch(),
        taskDescription: title || '',
        status: nextStatus,
        streamingOutput: cloneLines(lines),
        streamLineCounter: nextStreamLineCounter,
        currentTurnStartLineId: matchedRuntime?.currentTurnStartLineId ?? 0,
        taskId: source === 'claude' ? rawSessionId : null,
        isChatSession: source === 'claude',
        standaloneTurns: matchedRuntime?.standaloneTurns ? [...matchedRuntime.standaloneTurns] : [],
        standaloneSessionId: source === 'standalone' ? rawSessionId : null,
        latestUsage: matchedRuntime?.latestUsage ? { ...matchedRuntime.latestUsage } : null,
        sessionUsageTotals: matchedRuntime?.sessionUsageTotals ? { ...matchedRuntime.sessionUsageTotals } : null,
        startedAt: matchedRuntime?.startedAt ?? null,
        apiError: phase === 'failed' ? (lastError ?? null) : null,
        toolCallFilter: new ToolCallStreamFilter(),
        activeRuntimeHandleId: matchedRuntime?.id ?? handleId,
      });

      restoreSessionLlmSettings({
        llmBackend: matchedRuntime?.llmBackend,
        llmProvider: matchedRuntime?.llmProvider,
        llmModel: matchedRuntime?.llmModel,
      });

      if (matchedRuntime?.workspacePath) {
        useSettingsStore.setState({ workspacePath: matchedRuntime.workspacePath });
      }
    },
  };
}
