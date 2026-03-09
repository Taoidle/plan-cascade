import { deriveConversationTurns, rebuildStandaloneTurns } from '../../lib/conversationUtils';
import { ToolCallStreamFilter } from '../../utils/toolCallFilter';
import { useSettingsStore, type Backend } from '../settings';
import { clearSessionScopedMemory } from './memoryPostProcess';
import { buildHistorySessionId, createStandaloneSessionId } from './sessionLifecycle';
import type { ExecutionState, ExecutionStatus, SessionSnapshot } from './types';

interface SessionTreeActions {
  /** Legacy session-tree only. Simple chat should use runtimeRegistryActions instead. */
  backgroundCurrentSession: () => void;
  /** Legacy session-tree only. Simple chat should restore via runtimeRegistryActions instead. */
  switchToSession: (id: string) => void;
  /** Legacy session-tree only. */
  removeBackgroundSession: (id: string) => void;
  /** Legacy session-tree only. */
  forkSessionAtTurn: (userLineId: number) => void;
}

type ExecutionSetState = (
  partial: Partial<ExecutionState> | ((state: ExecutionState) => Partial<ExecutionState>),
) => void;

interface SessionTreeActionDeps {
  set: ExecutionSetState;
  get: () => ExecutionState;
  hasMeaningfulForegroundContent: (state: ExecutionState) => boolean;
  createSessionSnapshotFromForeground: (
    state: ExecutionState,
    settings: ReturnType<typeof useSettingsStore.getState>,
    id: string,
  ) => SessionSnapshot;
  shouldPersistForegroundBeforeSwitch: (state: ExecutionState) => boolean;
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

export function createSessionTreeActions(deps: SessionTreeActionDeps): SessionTreeActions {
  const {
    set,
    get,
    hasMeaningfulForegroundContent,
    createSessionSnapshotFromForeground,
    shouldPersistForegroundBeforeSwitch,
  } = deps;

  return {
    // Legacy session-tree path used by history/fork flows outside Simple.
    backgroundCurrentSession: () => {
      const state = get();
      const settingsState = useSettingsStore.getState();

      if (state.foregroundBgId && state.backgroundSessions[state.foregroundBgId]) {
        const updatedGhost = createSessionSnapshotFromForeground(state, settingsState, state.foregroundBgId);

        set({
          backgroundSessions: { ...state.backgroundSessions, [state.foregroundBgId]: updatedGhost },
          activeSessionId: state.foregroundBgId,
          ...buildResetForegroundPatch(),
        });
        return;
      }

      const hasForegroundContent = hasMeaningfulForegroundContent(state);
      if (hasForegroundContent) {
        const sessionId =
          typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
            ? `bg-${crypto.randomUUID()}`
            : `bg-${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;

        const snapshot = createSessionSnapshotFromForeground(state, settingsState, sessionId);
        set({
          backgroundSessions: { ...state.backgroundSessions, [sessionId]: snapshot },
          activeSessionId: sessionId,
          ...buildResetForegroundPatch(),
        });
        return;
      }

      set(buildResetForegroundPatch());
    },

    switchToSession: (id: string) => {
      const state = get();
      if (id === state.foregroundBgId) return;

      const targetSnapshot = state.backgroundSessions[id];
      if (!targetSnapshot) return;

      const currentSettings = useSettingsStore.getState();
      const newBackgroundSessions = { ...state.backgroundSessions };

      if (state.foregroundBgId && newBackgroundSessions[state.foregroundBgId]) {
        newBackgroundSessions[state.foregroundBgId] = createSessionSnapshotFromForeground(
          state,
          currentSettings,
          state.foregroundBgId,
        );
      } else if (shouldPersistForegroundBeforeSwitch(state)) {
        const canMergeByOrigin = !state.foregroundParentSessionId;
        const existingOriginMatch = canMergeByOrigin
          ? Object.values(newBackgroundSessions).find(
              (snap) =>
                (!!state.foregroundOriginHistoryId && snap.originHistoryId === state.foregroundOriginHistoryId) ||
                (!!state.foregroundOriginSessionId &&
                  (snap.originSessionId || buildHistorySessionId(snap.taskId, snap.standaloneSessionId)) ===
                    state.foregroundOriginSessionId),
            )
          : undefined;
        const pristineRestoredHistory =
          !state.foregroundDirty &&
          !state.taskId &&
          state.status !== 'running' &&
          state.status !== 'paused' &&
          !!(state.foregroundOriginHistoryId || state.foregroundOriginSessionId);

        if (existingOriginMatch) {
          if (state.foregroundDirty || state.status === 'running' || state.status === 'paused') {
            newBackgroundSessions[existingOriginMatch.id] = createSessionSnapshotFromForeground(
              state,
              currentSettings,
              existingOriginMatch.id,
            );
          }
        } else if (!pristineRestoredHistory) {
          const newBgId =
            typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
              ? `bg-${crypto.randomUUID()}`
              : `bg-${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;
          newBackgroundSessions[newBgId] = createSessionSnapshotFromForeground(state, currentSettings, newBgId);
        }
      }

      set({
        backgroundSessions: newBackgroundSessions,
        activeSessionId:
          state.foregroundBgId ||
          Object.keys(newBackgroundSessions).find(
            (k) => k !== id && newBackgroundSessions[k].taskId === state.taskId,
          ) ||
          state.activeSessionId,
        foregroundParentSessionId: targetSnapshot.parentSessionId || null,
        foregroundBgId: id,
        foregroundOriginHistoryId: targetSnapshot.originHistoryId || null,
        foregroundOriginSessionId:
          targetSnapshot.originSessionId ||
          buildHistorySessionId(targetSnapshot.taskId, targetSnapshot.standaloneSessionId),
        foregroundDirty: false,
        taskDescription: targetSnapshot.taskDescription,
        status: targetSnapshot.status,
        isCancelling: false,
        pendingCancelBeforeSessionReady: false,
        activeExecutionId: null,
        streamingOutput: [...targetSnapshot.streamingOutput],
        streamLineCounter: targetSnapshot.streamLineCounter,
        currentTurnStartLineId: targetSnapshot.currentTurnStartLineId,
        taskId: targetSnapshot.taskId,
        isChatSession: targetSnapshot.isChatSession,
        standaloneTurns: [...targetSnapshot.standaloneTurns],
        standaloneSessionId: targetSnapshot.standaloneSessionId,
        latestUsage: targetSnapshot.latestUsage ? { ...targetSnapshot.latestUsage } : null,
        sessionUsageTotals: targetSnapshot.sessionUsageTotals ? { ...targetSnapshot.sessionUsageTotals } : null,
        startedAt: targetSnapshot.startedAt,
        toolCallFilter: targetSnapshot.toolCallFilter,
      });

      restoreSessionLlmSettings({
        llmBackend: targetSnapshot.llmBackend,
        llmProvider: targetSnapshot.llmProvider,
        llmModel: targetSnapshot.llmModel,
      });

      if (targetSnapshot.workspacePath) {
        useSettingsStore.setState({ workspacePath: targetSnapshot.workspacePath });
      }
    },

    removeBackgroundSession: (id: string) => {
      const snapshot = get().backgroundSessions[id];
      const removedSessionId =
        snapshot?.originSessionId ||
        buildHistorySessionId(snapshot?.taskId || null, snapshot?.standaloneSessionId || null);

      set((state) => {
        if (!state.backgroundSessions[id]) return {};
        const removed = state.backgroundSessions[id];
        const remaining = { ...state.backgroundSessions };
        delete remaining[id];

        const reparented: Record<string, SessionSnapshot> = {};
        for (const [sid, snap] of Object.entries(remaining)) {
          if (snap.parentSessionId === id) {
            reparented[sid] = { ...snap, parentSessionId: removed.parentSessionId };
          } else {
            reparented[sid] = snap;
          }
        }

        return {
          backgroundSessions: reparented,
          foregroundParentSessionId:
            state.foregroundParentSessionId === id ? removed.parentSessionId || null : state.foregroundParentSessionId,
          foregroundBgId: state.foregroundBgId === id ? null : state.foregroundBgId,
          foregroundOriginHistoryId: state.foregroundBgId === id ? null : state.foregroundOriginHistoryId,
          foregroundOriginSessionId: state.foregroundBgId === id ? null : state.foregroundOriginSessionId,
        };
      });

      clearSessionScopedMemory(removedSessionId);
    },

    forkSessionAtTurn: (userLineId: number) => {
      const state = get();
      const lines = state.streamingOutput;
      const turns = deriveConversationTurns(lines);
      const targetTurn = turns.find((t) => t.userLineId === userLineId);
      if (!targetTurn) return;

      const settingsState = useSettingsStore.getState();
      const newBackgroundSessions = { ...state.backgroundSessions };
      let parentId: string;

      if (state.foregroundBgId && newBackgroundSessions[state.foregroundBgId]) {
        newBackgroundSessions[state.foregroundBgId] = createSessionSnapshotFromForeground(
          state,
          settingsState,
          state.foregroundBgId,
        );
        parentId = state.foregroundBgId;
      } else {
        const sessionId =
          typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
            ? `bg-${crypto.randomUUID()}`
            : `bg-${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;

        newBackgroundSessions[sessionId] = createSessionSnapshotFromForeground(state, settingsState, sessionId);
        parentId = sessionId;
      }

      const truncatedLines = lines.slice(0, targetTurn.assistantEndIndex + 1);
      const rebuiltTurns = rebuildStandaloneTurns(truncatedLines);

      set({
        backgroundSessions: newBackgroundSessions,
        activeSessionId: parentId,
        foregroundParentSessionId: parentId,
        foregroundBgId: null,
        foregroundOriginHistoryId: state.foregroundOriginHistoryId,
        foregroundOriginSessionId: state.foregroundOriginSessionId,
        foregroundDirty: true,
        streamingOutput: truncatedLines,
        streamLineCounter: truncatedLines.length > 0 ? truncatedLines[truncatedLines.length - 1].id : 0,
        standaloneTurns: rebuiltTurns,
        standaloneSessionId: createStandaloneSessionId(),
        status: 'idle' as ExecutionStatus,
        isCancelling: false,
        pendingCancelBeforeSessionReady: false,
        isSubmitting: false,
        activeExecutionId: null,
        taskId: null,
        isChatSession: false,
        latestUsage: null,
        sessionUsageTotals: null,
        turnUsageTotals: null,
        toolCallFilter: new ToolCallStreamFilter(),
        attachments: [],
        apiError: null,
        result: null,
      });

      get().addLog(`Forked conversation at turn with userLineId=${userLineId}`);
    },
  };
}
