import { ToolCallStreamFilter } from '../../utils/toolCallFilter';
import { useWorkflowKernelStore } from '../workflowKernel';
import { useSettingsStore, type Backend } from '../settings';
import type { ExecutionRuntimeHandle, ExecutionState, ExecutionStatus } from './types';

interface RuntimeRegistryActions {
  parkForegroundRuntime: () => string | null;
  restoreForegroundChatRuntime: (params: {
    source: 'claude' | 'standalone';
    rawSessionId: string;
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
    workspaceReferences: [],
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

function cloneUsage<T extends ExecutionRuntimeHandle['latestUsage']>(usage: T): T {
  return usage ? ({ ...usage } as T) : usage;
}

function buildExecutionRuntimeHandle(params: {
  source: 'claude' | 'standalone';
  rawSessionId: string;
  rootSessionId: string | null;
  status: ExecutionStatus;
  streamingOutput: ExecutionState['streamingOutput'];
  streamLineCounter: number;
  currentTurnStartLineId: number;
  standaloneTurns: ExecutionState['standaloneTurns'];
  latestUsage: ExecutionState['latestUsage'];
  sessionUsageTotals: ExecutionState['sessionUsageTotals'];
  startedAt: number | null;
  workspacePath: string | null;
  llmBackend: string;
  llmProvider: string;
  llmModel: string;
  updatedAt?: number;
}): ExecutionRuntimeHandle {
  return {
    id: buildExecutionRuntimeHandleId(params.source, params.rawSessionId),
    source: params.source,
    rawSessionId: params.rawSessionId,
    rootSessionId: params.rootSessionId,
    mode: 'chat',
    status: params.status,
    streamingOutput: cloneLines(params.streamingOutput),
    streamLineCounter: params.streamLineCounter,
    currentTurnStartLineId: params.currentTurnStartLineId,
    standaloneTurns: [...params.standaloneTurns],
    latestUsage: cloneUsage(params.latestUsage),
    sessionUsageTotals: cloneUsage(params.sessionUsageTotals),
    startedAt: params.startedAt,
    workspacePath: params.workspacePath,
    llmBackend: params.llmBackend,
    llmProvider: params.llmProvider,
    llmModel: params.llmModel,
    updatedAt: params.updatedAt ?? Date.now(),
  };
}

export function buildActiveChatRuntimeRegistryPatch(
  state: Pick<
    ExecutionState,
    | 'runtimeRegistry'
    | 'status'
    | 'streamingOutput'
    | 'streamLineCounter'
    | 'currentTurnStartLineId'
    | 'standaloneTurns'
    | 'latestUsage'
    | 'sessionUsageTotals'
    | 'startedAt'
  >,
  params: {
    source: 'claude' | 'standalone';
    rawSessionId: string;
    rootSessionId: string | null;
    workspacePath: string | null;
    llmBackend: string;
    llmProvider: string;
    llmModel: string;
  },
): Pick<ExecutionState, 'runtimeRegistry' | 'activeRuntimeHandleId'> {
  const handle = buildExecutionRuntimeHandle({
    ...params,
    status: state.status,
    streamingOutput: state.streamingOutput,
    streamLineCounter: state.streamLineCounter,
    currentTurnStartLineId: state.currentTurnStartLineId,
    standaloneTurns: state.standaloneTurns,
    latestUsage: state.latestUsage,
    sessionUsageTotals: state.sessionUsageTotals,
    startedAt: state.startedAt,
  });
  return {
    runtimeRegistry: {
      ...state.runtimeRegistry,
      [handle.id]: handle,
    },
    activeRuntimeHandleId: handle.id,
  };
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
  return buildExecutionRuntimeHandle({
    source,
    rawSessionId,
    rootSessionId,
    status: state.status,
    streamingOutput: state.streamingOutput,
    streamLineCounter: state.streamLineCounter,
    currentTurnStartLineId: state.currentTurnStartLineId,
    standaloneTurns: state.standaloneTurns,
    latestUsage: state.latestUsage,
    sessionUsageTotals: state.sessionUsageTotals,
    startedAt: state.startedAt,
    workspacePath: settingsState.workspacePath ?? null,
    llmBackend: settingsState.backend,
    llmProvider: settingsState.provider,
    llmModel: settingsState.model,
  });
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

    restoreForegroundChatRuntime: ({ source, rawSessionId, title, phase, lastError }) => {
      const state = get();
      const handleId = buildExecutionRuntimeHandleId(source, rawSessionId);
      const matchedRuntime = state.runtimeRegistry[handleId] ?? null;
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
        streamingOutput: matchedRuntime?.streamingOutput ? cloneLines(matchedRuntime.streamingOutput) : [],
        streamLineCounter: matchedRuntime?.streamLineCounter ?? 0,
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
