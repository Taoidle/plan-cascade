import { reportNonFatal } from '../../lib/nonFatal';
import { normalizeTurnBoundaries } from '../../lib/conversationUtils';
import { ToolCallStreamFilter } from '../../utils/toolCallFilter';
import { useSettingsStore } from '../settings';
import { buildExecutionRuntimeHandleId } from './runtimeRegistryActions';
import type { ExecutionRuntimeHandle, ExecutionState, ExecutionStatus, SessionSnapshot } from './types';

interface PersistedSessionSnapshot {
  id: string;
  taskDescription: string;
  status: ExecutionStatus;
  streamingOutput: ExecutionState['streamingOutput'];
  streamLineCounter: number;
  currentTurnStartLineId: number;
  taskId: string | null;
  isChatSession: boolean;
  standaloneTurns: ExecutionState['standaloneTurns'];
  standaloneSessionId: string | null;
  latestUsage: ExecutionState['latestUsage'];
  sessionUsageTotals: ExecutionState['sessionUsageTotals'];
  startedAt: number | null;
  llmBackend: string;
  llmProvider: string;
  llmModel: string;
  parentSessionId?: string;
  workspacePath?: string;
  originHistoryId?: string;
  originSessionId?: string;
  updatedAt?: number;
}

export interface PersistedRuntimeRegistryHandle {
  id: string;
  source: 'claude' | 'standalone';
  rawSessionId: string;
  rootSessionId: string | null;
  mode: 'chat';
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
  updatedAt: number;
}

export interface PersistedLegacySessionTreeState {
  backgroundSessions: Record<string, PersistedSessionSnapshot>;
  activeSessionId: string | null;
  foregroundParentSessionId: string | null;
  foregroundBgId: string | null;
  foregroundOriginHistoryId: string | null;
  foregroundOriginSessionId: string | null;
}

interface PersistedForegroundSnapshot {
  taskDescription: string;
  status: ExecutionStatus;
  streamingOutput: ExecutionState['streamingOutput'];
  streamLineCounter: number;
  currentTurnStartLineId: number;
  taskId: string | null;
  isChatSession: boolean;
  standaloneTurns: ExecutionState['standaloneTurns'];
  standaloneSessionId: string | null;
  latestUsage: ExecutionState['latestUsage'];
  sessionUsageTotals: ExecutionState['sessionUsageTotals'];
  turnUsageTotals: ExecutionState['turnUsageTotals'];
  startedAt: number | null;
  llmBackend: string;
  llmProvider: string;
  llmModel: string;
  foregroundParentSessionId: string | null;
  foregroundBgId: string | null;
  foregroundOriginHistoryId: string | null;
  foregroundOriginSessionId: string | null;
  foregroundDirty: boolean;
  workspacePath?: string;
}

interface PersistedSessionStateV1 {
  version: 1;
  activeSessionId: string | null;
  backgroundSessions: Record<string, PersistedSessionSnapshot>;
  foreground: PersistedForegroundSnapshot | null;
}

export interface PersistedSessionStateV2 {
  version: 2;
  runtimeRegistry: Record<string, PersistedRuntimeRegistryHandle>;
  activeRuntimeHandleId: string | null;
  legacySessionTree: PersistedLegacySessionTreeState;
  foreground: PersistedForegroundSnapshot | null;
}

export interface PersistedSessionRestoreResult {
  state: Partial<ExecutionState>;
  foregroundWorkspacePath?: string;
}

interface SessionPersistenceOptions {
  sessionStateKey: string;
  hasMeaningfulForegroundContent: (state: ExecutionState) => boolean;
  buildHistorySessionId: (taskId: string | null, standaloneSessionId: string | null) => string | null;
}

interface SessionPersistenceController {
  load: () => PersistedSessionRestoreResult | null;
  persistNow: (state: ExecutionState) => void;
  schedule: (state: ExecutionState) => void;
  cancelScheduled: () => void;
}

function normalizeRestoredStatus(status: ExecutionStatus): ExecutionStatus {
  if (status === 'running' || status === 'paused') return 'idle';
  return status;
}

function cloneLines(lines: ExecutionState['streamingOutput']): ExecutionState['streamingOutput'] {
  return normalizeTurnBoundaries(lines.map((line) => ({ ...line })));
}

function toPersistedSessionSnapshot(snapshot: SessionSnapshot): PersistedSessionSnapshot {
  return {
    id: snapshot.id,
    taskDescription: snapshot.taskDescription,
    status: normalizeRestoredStatus(snapshot.status),
    streamingOutput: [...snapshot.streamingOutput],
    streamLineCounter: snapshot.streamLineCounter,
    currentTurnStartLineId: snapshot.currentTurnStartLineId,
    taskId: snapshot.taskId,
    isChatSession: snapshot.isChatSession,
    standaloneTurns: [...snapshot.standaloneTurns],
    standaloneSessionId: snapshot.standaloneSessionId,
    latestUsage: snapshot.latestUsage ? { ...snapshot.latestUsage } : null,
    sessionUsageTotals: snapshot.sessionUsageTotals ? { ...snapshot.sessionUsageTotals } : null,
    startedAt: snapshot.startedAt,
    llmBackend: snapshot.llmBackend,
    llmProvider: snapshot.llmProvider,
    llmModel: snapshot.llmModel,
    parentSessionId: snapshot.parentSessionId,
    workspacePath: snapshot.workspacePath,
    originHistoryId: snapshot.originHistoryId,
    originSessionId: snapshot.originSessionId,
    updatedAt: snapshot.updatedAt || Date.now(),
  };
}

function fromPersistedSessionSnapshot(snapshot: PersistedSessionSnapshot): SessionSnapshot {
  return {
    id: snapshot.id,
    taskDescription: snapshot.taskDescription,
    status: normalizeRestoredStatus(snapshot.status),
    streamingOutput: cloneLines(snapshot.streamingOutput),
    streamLineCounter: snapshot.streamLineCounter,
    currentTurnStartLineId: snapshot.currentTurnStartLineId,
    taskId: snapshot.taskId,
    isChatSession: snapshot.isChatSession,
    standaloneTurns: [...snapshot.standaloneTurns],
    standaloneSessionId: snapshot.standaloneSessionId,
    latestUsage: snapshot.latestUsage ? { ...snapshot.latestUsage } : null,
    sessionUsageTotals: snapshot.sessionUsageTotals ? { ...snapshot.sessionUsageTotals } : null,
    startedAt: snapshot.startedAt,
    toolCallFilter: new ToolCallStreamFilter(),
    llmBackend: snapshot.llmBackend,
    llmProvider: snapshot.llmProvider,
    llmModel: snapshot.llmModel,
    parentSessionId: snapshot.parentSessionId,
    workspacePath: snapshot.workspacePath,
    originHistoryId: snapshot.originHistoryId,
    originSessionId: snapshot.originSessionId,
    updatedAt: snapshot.updatedAt || Date.now(),
  };
}

function toPersistedRuntimeRegistryHandle(handle: ExecutionRuntimeHandle): PersistedRuntimeRegistryHandle {
  return {
    id: handle.id,
    source: handle.source,
    rawSessionId: handle.rawSessionId,
    rootSessionId: handle.rootSessionId,
    mode: 'chat',
    status: normalizeRestoredStatus(handle.status),
    streamingOutput: [...handle.streamingOutput],
    streamLineCounter: handle.streamLineCounter,
    currentTurnStartLineId: handle.currentTurnStartLineId,
    standaloneTurns: [...handle.standaloneTurns],
    latestUsage: handle.latestUsage ? { ...handle.latestUsage } : null,
    sessionUsageTotals: handle.sessionUsageTotals ? { ...handle.sessionUsageTotals } : null,
    startedAt: handle.startedAt,
    workspacePath: handle.workspacePath,
    llmBackend: handle.llmBackend,
    llmProvider: handle.llmProvider,
    llmModel: handle.llmModel,
    updatedAt: handle.updatedAt,
  };
}

function fromPersistedRuntimeRegistryHandle(handle: PersistedRuntimeRegistryHandle): ExecutionRuntimeHandle {
  return {
    id: handle.id,
    source: handle.source,
    rawSessionId: handle.rawSessionId,
    rootSessionId: handle.rootSessionId,
    mode: 'chat',
    status: normalizeRestoredStatus(handle.status),
    streamingOutput: cloneLines(handle.streamingOutput),
    streamLineCounter: handle.streamLineCounter,
    currentTurnStartLineId: handle.currentTurnStartLineId,
    standaloneTurns: [...handle.standaloneTurns],
    latestUsage: handle.latestUsage ? { ...handle.latestUsage } : null,
    sessionUsageTotals: handle.sessionUsageTotals ? { ...handle.sessionUsageTotals } : null,
    startedAt: handle.startedAt,
    workspacePath: handle.workspacePath,
    llmBackend: handle.llmBackend,
    llmProvider: handle.llmProvider,
    llmModel: handle.llmModel,
    updatedAt: handle.updatedAt || Date.now(),
  };
}

function buildPersistedForegroundSnapshot(
  state: ExecutionState,
  options: Pick<SessionPersistenceOptions, 'hasMeaningfulForegroundContent' | 'buildHistorySessionId'>,
): PersistedForegroundSnapshot | null {
  if (!options.hasMeaningfulForegroundContent(state)) return null;
  const settings = useSettingsStore.getState();
  return {
    taskDescription: state.taskDescription,
    status: normalizeRestoredStatus(state.status),
    streamingOutput: [...state.streamingOutput],
    streamLineCounter: state.streamLineCounter,
    currentTurnStartLineId: state.currentTurnStartLineId,
    taskId: state.taskId,
    isChatSession: state.isChatSession,
    standaloneTurns: [...state.standaloneTurns],
    standaloneSessionId: state.standaloneSessionId,
    latestUsage: state.latestUsage ? { ...state.latestUsage } : null,
    sessionUsageTotals: state.sessionUsageTotals ? { ...state.sessionUsageTotals } : null,
    turnUsageTotals: state.turnUsageTotals ? { ...state.turnUsageTotals } : null,
    startedAt: state.startedAt,
    llmBackend: settings.backend,
    llmProvider: settings.provider,
    llmModel: settings.model,
    foregroundParentSessionId: state.foregroundParentSessionId,
    foregroundBgId: state.foregroundBgId,
    foregroundOriginHistoryId: state.foregroundOriginHistoryId,
    foregroundOriginSessionId:
      state.foregroundOriginSessionId || options.buildHistorySessionId(state.taskId, state.standaloneSessionId),
    foregroundDirty: state.foregroundDirty,
    workspacePath: settings.workspacePath || undefined,
  };
}

function restoreLegacySessionTree(
  tree: PersistedLegacySessionTreeState | null | undefined,
): Pick<
  ExecutionState,
  | 'backgroundSessions'
  | 'activeSessionId'
  | 'foregroundParentSessionId'
  | 'foregroundBgId'
  | 'foregroundOriginHistoryId'
  | 'foregroundOriginSessionId'
> {
  const backgroundSessions = Object.fromEntries(
    Object.entries(tree?.backgroundSessions || {}).map(([id, snapshot]) => [
      id,
      fromPersistedSessionSnapshot(snapshot),
    ]),
  );
  return {
    backgroundSessions,
    activeSessionId: tree?.activeSessionId || null,
    foregroundParentSessionId: tree?.foregroundParentSessionId || null,
    foregroundBgId: tree?.foregroundBgId || null,
    foregroundOriginHistoryId: tree?.foregroundOriginHistoryId || null,
    foregroundOriginSessionId: tree?.foregroundOriginSessionId || null,
  };
}

function buildRuntimeHandleFromLegacySnapshot(
  snapshot: PersistedSessionSnapshot,
  rootSessionId: string | null = null,
): PersistedRuntimeRegistryHandle | null {
  const source = snapshot.taskId ? 'claude' : snapshot.standaloneSessionId ? 'standalone' : null;
  const rawSessionId = snapshot.taskId ?? snapshot.standaloneSessionId;
  if (!source || !rawSessionId || !snapshot.isChatSession) return null;
  return {
    id: buildExecutionRuntimeHandleId(source, rawSessionId),
    source,
    rawSessionId,
    rootSessionId,
    mode: 'chat',
    status: normalizeRestoredStatus(snapshot.status),
    streamingOutput: [...snapshot.streamingOutput],
    streamLineCounter: snapshot.streamLineCounter,
    currentTurnStartLineId: snapshot.currentTurnStartLineId,
    standaloneTurns: [...snapshot.standaloneTurns],
    latestUsage: snapshot.latestUsage ? { ...snapshot.latestUsage } : null,
    sessionUsageTotals: snapshot.sessionUsageTotals ? { ...snapshot.sessionUsageTotals } : null,
    startedAt: snapshot.startedAt,
    workspacePath: snapshot.workspacePath ?? null,
    llmBackend: snapshot.llmBackend,
    llmProvider: snapshot.llmProvider,
    llmModel: snapshot.llmModel,
    updatedAt: snapshot.updatedAt || Date.now(),
  };
}

function shouldPersistLegacySnapshot(
  snapshot: SessionSnapshot,
  runtimeRegistry: Record<string, ExecutionRuntimeHandle>,
): boolean {
  const source = snapshot.taskId ? 'claude' : snapshot.standaloneSessionId ? 'standalone' : null;
  const rawSessionId = snapshot.taskId ?? snapshot.standaloneSessionId;
  if (!snapshot.isChatSession || !source || !rawSessionId) return true;
  return !runtimeRegistry[buildExecutionRuntimeHandleId(source, rawSessionId)];
}

function buildPersistedLegacySessionTree(state: ExecutionState): PersistedLegacySessionTreeState {
  const backgroundSessions = Object.fromEntries(
    Object.entries(state.backgroundSessions)
      .filter(([, snapshot]) => shouldPersistLegacySnapshot(snapshot, state.runtimeRegistry))
      .map(([id, snapshot]) => [id, toPersistedSessionSnapshot(snapshot)]),
  );
  const activeSessionId =
    state.activeSessionId && backgroundSessions[state.activeSessionId] ? state.activeSessionId : null;
  const foregroundBgId = state.foregroundBgId && backgroundSessions[state.foregroundBgId] ? state.foregroundBgId : null;
  return {
    backgroundSessions,
    activeSessionId,
    foregroundParentSessionId: state.foregroundParentSessionId,
    foregroundBgId,
    foregroundOriginHistoryId: foregroundBgId ? state.foregroundOriginHistoryId : null,
    foregroundOriginSessionId: foregroundBgId ? state.foregroundOriginSessionId : null,
  };
}

function restoreV2(parsed: PersistedSessionStateV2): PersistedSessionRestoreResult {
  const restoredRuntimeRegistry = Object.fromEntries(
    Object.entries(parsed.runtimeRegistry || {}).map(([id, handle]) => [
      id,
      fromPersistedRuntimeRegistryHandle(handle),
    ]),
  );
  const restoredLegacyTree = restoreLegacySessionTree(parsed.legacySessionTree);

  if (!parsed.foreground) {
    return {
      state: {
        ...restoredLegacyTree,
        runtimeRegistry: restoredRuntimeRegistry,
        activeRuntimeHandleId: parsed.activeRuntimeHandleId || null,
        foregroundDirty: false,
      },
    };
  }

  const fg = parsed.foreground;
  return {
    state: {
      ...restoredLegacyTree,
      runtimeRegistry: restoredRuntimeRegistry,
      activeRuntimeHandleId: parsed.activeRuntimeHandleId || null,
      taskDescription: fg.taskDescription,
      status: normalizeRestoredStatus(fg.status),
      streamingOutput: cloneLines(fg.streamingOutput),
      streamLineCounter: fg.streamLineCounter,
      currentTurnStartLineId: fg.currentTurnStartLineId,
      taskId: fg.taskId,
      isChatSession: fg.isChatSession,
      standaloneTurns: [...fg.standaloneTurns],
      standaloneSessionId: fg.standaloneSessionId,
      latestUsage: fg.latestUsage ? { ...fg.latestUsage } : null,
      sessionUsageTotals: fg.sessionUsageTotals ? { ...fg.sessionUsageTotals } : null,
      turnUsageTotals: fg.turnUsageTotals ? { ...fg.turnUsageTotals } : null,
      startedAt: fg.startedAt,
      foregroundParentSessionId: fg.foregroundParentSessionId,
      foregroundBgId: fg.foregroundBgId,
      foregroundOriginHistoryId: fg.foregroundOriginHistoryId,
      foregroundOriginSessionId: fg.foregroundOriginSessionId,
      foregroundDirty: fg.foregroundDirty,
      toolCallFilter: new ToolCallStreamFilter(),
    },
    foregroundWorkspacePath: fg.workspacePath,
  };
}

function restoreV1(parsed: PersistedSessionStateV1): PersistedSessionRestoreResult {
  const backgroundEntries = Object.entries(parsed.backgroundSessions || {});
  const restoredLegacyTree = restoreLegacySessionTree({
    backgroundSessions: parsed.backgroundSessions || {},
    activeSessionId: parsed.activeSessionId || null,
    foregroundParentSessionId: null,
    foregroundBgId: null,
    foregroundOriginHistoryId: null,
    foregroundOriginSessionId: null,
  });

  const migratedRuntimeRegistry = Object.fromEntries(
    backgroundEntries
      .map(([, snapshot]) => buildRuntimeHandleFromLegacySnapshot(snapshot))
      .filter((handle): handle is PersistedRuntimeRegistryHandle => Boolean(handle))
      .map((handle) => [handle.id, fromPersistedRuntimeRegistryHandle(handle)]),
  );

  if (!parsed.foreground) {
    return {
      state: {
        ...restoredLegacyTree,
        runtimeRegistry: migratedRuntimeRegistry,
        activeRuntimeHandleId: null,
        foregroundDirty: false,
      },
    };
  }

  const fg = parsed.foreground;
  const foregroundHandle = fg.isChatSession
    ? buildRuntimeHandleFromLegacySnapshot(
        {
          id: fg.taskId ?? fg.standaloneSessionId ?? 'foreground',
          taskDescription: fg.taskDescription,
          status: fg.status,
          streamingOutput: fg.streamingOutput,
          streamLineCounter: fg.streamLineCounter,
          currentTurnStartLineId: fg.currentTurnStartLineId,
          taskId: fg.taskId,
          isChatSession: fg.isChatSession,
          standaloneTurns: fg.standaloneTurns,
          standaloneSessionId: fg.standaloneSessionId,
          latestUsage: fg.latestUsage,
          sessionUsageTotals: fg.sessionUsageTotals,
          startedAt: fg.startedAt,
          llmBackend: fg.llmBackend,
          llmProvider: fg.llmProvider,
          llmModel: fg.llmModel,
          workspacePath: fg.workspacePath,
        },
        null,
      )
    : null;
  if (foregroundHandle) {
    migratedRuntimeRegistry[foregroundHandle.id] = fromPersistedRuntimeRegistryHandle(foregroundHandle);
  }

  return {
    state: {
      ...restoredLegacyTree,
      runtimeRegistry: migratedRuntimeRegistry,
      activeRuntimeHandleId: foregroundHandle?.id ?? null,
      taskDescription: fg.taskDescription,
      status: normalizeRestoredStatus(fg.status),
      streamingOutput: cloneLines(fg.streamingOutput),
      streamLineCounter: fg.streamLineCounter,
      currentTurnStartLineId: fg.currentTurnStartLineId,
      taskId: fg.taskId,
      isChatSession: fg.isChatSession,
      standaloneTurns: [...fg.standaloneTurns],
      standaloneSessionId: fg.standaloneSessionId,
      latestUsage: fg.latestUsage ? { ...fg.latestUsage } : null,
      sessionUsageTotals: fg.sessionUsageTotals ? { ...fg.sessionUsageTotals } : null,
      turnUsageTotals: fg.turnUsageTotals ? { ...fg.turnUsageTotals } : null,
      startedAt: fg.startedAt,
      foregroundParentSessionId: fg.foregroundParentSessionId,
      foregroundBgId: fg.foregroundBgId,
      foregroundOriginHistoryId: fg.foregroundOriginHistoryId,
      foregroundOriginSessionId: fg.foregroundOriginSessionId,
      foregroundDirty: fg.foregroundDirty,
      toolCallFilter: new ToolCallStreamFilter(),
    },
    foregroundWorkspacePath: fg.workspacePath,
  };
}

export function createSessionPersistenceController(options: SessionPersistenceOptions): SessionPersistenceController {
  let persistTimer: ReturnType<typeof setTimeout> | null = null;

  const persistNow = (state: ExecutionState): void => {
    try {
      if (typeof localStorage === 'undefined') return;
      const persisted: PersistedSessionStateV2 = {
        version: 2,
        runtimeRegistry: Object.fromEntries(
          Object.entries(state.runtimeRegistry).map(([id, handle]) => [id, toPersistedRuntimeRegistryHandle(handle)]),
        ),
        activeRuntimeHandleId: state.activeRuntimeHandleId,
        legacySessionTree: buildPersistedLegacySessionTree(state),
        foreground: buildPersistedForegroundSnapshot(state, options),
      };
      localStorage.setItem(options.sessionStateKey, JSON.stringify(persisted));
    } catch (error) {
      reportNonFatal('execution.persistSessionStateSnapshot', error);
    }
  };

  return {
    load: (): PersistedSessionRestoreResult | null => {
      try {
        if (typeof localStorage === 'undefined') return null;
        const raw = localStorage.getItem(options.sessionStateKey);
        if (!raw) return null;
        const parsed = JSON.parse(raw) as PersistedSessionStateV1 | PersistedSessionStateV2;
        if (!parsed || typeof parsed !== 'object' || !('version' in parsed)) return null;
        if (parsed.version === 2) return restoreV2(parsed);
        if (parsed.version === 1) return restoreV1(parsed);
        return null;
      } catch (error) {
        reportNonFatal('execution.loadPersistedSessionState', error);
        return null;
      }
    },

    persistNow,

    schedule: (state: ExecutionState) => {
      if (persistTimer) {
        clearTimeout(persistTimer);
      }
      persistTimer = setTimeout(() => {
        persistTimer = null;
        persistNow(state);
      }, 120);
    },

    cancelScheduled: () => {
      if (persistTimer) {
        clearTimeout(persistTimer);
        persistTimer = null;
      }
    },
  };
}
