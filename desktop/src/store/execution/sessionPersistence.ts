import { reportNonFatal } from '../../lib/nonFatal';
import { ToolCallStreamFilter } from '../../utils/toolCallFilter';
import { useSettingsStore } from '../settings';
import type { ExecutionState, ExecutionStatus, SessionSnapshot } from './types';

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

function toPersistedSessionSnapshot(snapshot: SessionSnapshot): PersistedSessionSnapshot {
  return {
    id: snapshot.id,
    taskDescription: snapshot.taskDescription,
    status: normalizeRestoredStatus(snapshot.status),
    streamingOutput: [...snapshot.streamingOutput],
    streamLineCounter: snapshot.streamLineCounter,
    currentTurnStartLineId: snapshot.currentTurnStartLineId,
    taskId: null,
    isChatSession: false,
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
    streamingOutput: [...snapshot.streamingOutput],
    streamLineCounter: snapshot.streamLineCounter,
    currentTurnStartLineId: snapshot.currentTurnStartLineId,
    taskId: null,
    isChatSession: false,
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
    taskId: null,
    isChatSession: false,
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

export function createSessionPersistenceController(options: SessionPersistenceOptions): SessionPersistenceController {
  let persistTimer: ReturnType<typeof setTimeout> | null = null;

  const persistNow = (state: ExecutionState): void => {
    try {
      if (typeof localStorage === 'undefined') return;
      const persisted: PersistedSessionStateV1 = {
        version: 1,
        activeSessionId: state.activeSessionId,
        backgroundSessions: Object.fromEntries(
          Object.entries(state.backgroundSessions).map(([id, snapshot]) => [id, toPersistedSessionSnapshot(snapshot)]),
        ),
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
        const parsed = JSON.parse(raw) as PersistedSessionStateV1;
        if (!parsed || parsed.version !== 1) return null;

        const restoredBackground = Object.fromEntries(
          Object.entries(parsed.backgroundSessions || {}).map(([id, snapshot]) => [
            id,
            fromPersistedSessionSnapshot(snapshot),
          ]),
        );

        if (!parsed.foreground) {
          return {
            state: {
              backgroundSessions: restoredBackground,
              activeSessionId: parsed.activeSessionId || null,
              foregroundParentSessionId: null,
              foregroundBgId: null,
              foregroundOriginHistoryId: null,
              foregroundOriginSessionId: null,
              foregroundDirty: false,
            },
          };
        }

        const fg = parsed.foreground;
        return {
          state: {
            backgroundSessions: restoredBackground,
            activeSessionId: parsed.activeSessionId || null,
            taskDescription: fg.taskDescription,
            status: normalizeRestoredStatus(fg.status),
            streamingOutput: [...fg.streamingOutput],
            streamLineCounter: fg.streamLineCounter,
            currentTurnStartLineId: fg.currentTurnStartLineId,
            taskId: null,
            isChatSession: false,
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
