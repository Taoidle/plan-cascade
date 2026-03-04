import { useSettingsStore } from '../settings';
import { normalizeTurnBoundaries, rebuildStandaloneTurns } from '../../lib/conversationUtils';
import type {
  ExecutionState,
  ExecutionHistoryItem,
  HistoryConversationLine,
  SessionSnapshot,
  StandaloneTurn,
  StreamLine,
  StreamLineType,
} from './types';

interface HistoryActions {
  loadHistory: () => void;
  saveToHistory: () => void;
  clearHistory: () => void;
  deleteHistory: (historyId: string) => void;
  renameHistory: (historyId: string, title: string) => void;
  restoreFromHistory: (historyId: string) => void;
}

type ExecutionSetState = (
  partial: Partial<ExecutionState> | ((state: ExecutionState) => Partial<ExecutionState>),
) => void;

interface HistoryActionDeps {
  set: ExecutionSetState;
  get: () => ExecutionState;
  initialState: Partial<ExecutionState>;
  maxHistoryItems: number;
  listHistoryFromSQLite: (limit?: number) => Promise<ExecutionHistoryItem[] | null>;
  upsertHistoryToSQLite: (item: ExecutionHistoryItem) => Promise<void>;
  importHistoryToSQLite: (items: ExecutionHistoryItem[]) => Promise<boolean>;
  clearHistoryInSQLite: () => Promise<void>;
  deleteHistoryFromSQLite: (historyId: string) => Promise<void>;
  renameHistoryInSQLite: (historyId: string, title?: string) => Promise<void>;
  isHistoryMigrationDone: () => boolean;
  loadLegacyHistoryFromLocalStorage: () => ExecutionHistoryItem[];
  markHistoryMigrationDone: () => void;
  clearSessionScopedMemory: (sessionId: string | null | undefined) => void;
  buildHistorySessionId: (taskId: string | null, standaloneSessionId: string | null) => string | null;
  createSessionSnapshotFromForeground: (
    state: ExecutionState,
    settings: ReturnType<typeof useSettingsStore.getState>,
    id: string,
  ) => SessionSnapshot;
  shouldPersistForegroundBeforeSwitch: (state: ExecutionState) => boolean;
  restoreSessionLlmSettings: (settings: { llmBackend?: string; llmProvider?: string; llmModel?: string }) => void;
  trimStandaloneTurns: (turns: StandaloneTurn[], limit: number) => StandaloneTurn[];
  getStandaloneContextTurnsLimit: () => number;
}

export function createHistoryActions(deps: HistoryActionDeps): HistoryActions {
  const {
    set,
    get,
    initialState,
    maxHistoryItems,
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
  } = deps;

  return {
    loadHistory: () => {
      void (async () => {
        const dbHistory = await listHistoryFromSQLite(maxHistoryItems);
        const migrated = isHistoryMigrationDone();

        if (!migrated) {
          const legacy = loadLegacyHistoryFromLocalStorage();
          if (legacy.length > 0) {
            const imported = await importHistoryToSQLite(legacy);
            if (imported) {
              markHistoryMigrationDone();
            } else {
              set({ history: legacy });
              return;
            }
          } else {
            markHistoryMigrationDone();
          }
        }

        const finalHistory = (await listHistoryFromSQLite(maxHistoryItems)) ?? dbHistory;
        if (finalHistory) {
          set({ history: finalHistory });
          return;
        }

        const legacyFallback = loadLegacyHistoryFromLocalStorage();
        if (legacyFallback.length > 0) {
          set({ history: legacyFallback });
        }
      })();
    },

    saveToHistory: () => {
      const state = get();
      if (!state.taskDescription) return;
      const settings = useSettingsStore.getState();
      const workspacePath = (settings.workspacePath || '').trim() || null;
      const sessionId = buildHistorySessionId(state.taskId, state.standaloneSessionId) || undefined;

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
        card: '[Card] ',
      };
      const conversationLines: HistoryConversationLine[] | undefined =
        state.streamingOutput.length > 0
          ? state.streamingOutput.map((line) => ({
              type: line.type,
              content: line.content,
              ...(line.subAgentId ? { subAgentId: line.subAgentId } : {}),
              ...(line.subAgentDepth != null ? { subAgentDepth: line.subAgentDepth } : {}),
              ...(line.turnId != null ? { turnId: line.turnId } : {}),
              ...(line.turnBoundary ? { turnBoundary: line.turnBoundary } : {}),
            }))
          : undefined;
      const conversationContent =
        state.streamingOutput.length > 0
          ? state.streamingOutput.map((line) => `${TYPE_PREFIX[line.type]}${line.content}`).join('\n')
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
        llmBackend: settings.backend,
        llmProvider: settings.provider,
        llmModel: settings.model,
      };

      let itemToPersist: ExecutionHistoryItem | null = null;
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
            newHistory = [updated, ...cloned].slice(0, maxHistoryItems);
            itemToPersist = updated;
          } else {
            const created: ExecutionHistoryItem = {
              ...baseItem,
              id: `history_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
            };
            newHistory = [created, ...prevState.history].slice(0, maxHistoryItems);
            itemToPersist = created;
          }
        } else {
          const created: ExecutionHistoryItem = {
            ...baseItem,
            id: `history_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
          };
          newHistory = [created, ...prevState.history].slice(0, maxHistoryItems);
          itemToPersist = created;
        }

        return { history: newHistory };
      });

      if (itemToPersist) {
        void upsertHistoryToSQLite(itemToPersist);
      }
    },

    clearHistory: () => {
      const sessionIds = get()
        .history.map((item) => item.sessionId || null)
        .filter((sid): sid is string => Boolean(sid && sid.trim()));
      set({ history: [] });
      void clearHistoryInSQLite();
      for (const sid of sessionIds) {
        clearSessionScopedMemory(sid);
      }
    },

    deleteHistory: (historyId: string) => {
      const removedSessionId = get().history.find((item) => item.id === historyId)?.sessionId;
      set((state) => {
        const next = state.history.filter((item) => item.id !== historyId);
        return { history: next };
      });
      void deleteHistoryFromSQLite(historyId);
      clearSessionScopedMemory(removedSessionId);
    },

    renameHistory: (historyId: string, title: string) => {
      const trimmed = title.trim();
      set((state) => {
        const next = state.history.map((item) =>
          item.id === historyId
            ? {
                ...item,
                title: trimmed.length > 0 ? trimmed : undefined,
              }
            : item,
        );
        return { history: next };
      });
      void renameHistoryInSQLite(historyId, trimmed.length > 0 ? trimmed : undefined);
    },

    restoreFromHistory: (historyId: string) => {
      const item = get().history.find((h) => h.id === historyId);
      if (!item) return;

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
            ...(line.subAgentId ? { subAgentId: line.subAgentId } : {}),
            ...(line.subAgentDepth != null ? { subAgentDepth: line.subAgentDepth } : {}),
            ...(line.turnId != null ? { turnId: line.turnId } : {}),
            ...(line.turnBoundary ? { turnBoundary: line.turnBoundary } : {}),
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

      const normalizedLines = normalizeTurnBoundaries(lines);

      const restoredSessionId = item.sessionId || null;
      const claudePrefix = 'claude:';
      const standalonePrefix = 'standalone:';
      const isClaudeSession = restoredSessionId !== null && restoredSessionId.startsWith(claudePrefix);
      const isStandaloneSession = restoredSessionId !== null && restoredSessionId.startsWith(standalonePrefix);
      const restoredStandaloneTurns: StandaloneTurn[] = isStandaloneSession
        ? rebuildStandaloneTurns(normalizedLines)
        : [];

      const currentState = get();
      let bgSessions = currentState.backgroundSessions;
      if (currentState.foregroundBgId && bgSessions[currentState.foregroundBgId]) {
        const curSettings = useSettingsStore.getState();
        const originalWorkspacePath = bgSessions[currentState.foregroundBgId].workspacePath;
        const updatedGhost = createSessionSnapshotFromForeground(
          currentState,
          curSettings,
          currentState.foregroundBgId,
        );
        updatedGhost.workspacePath = originalWorkspacePath;
        bgSessions = { ...bgSessions, [currentState.foregroundBgId]: updatedGhost };
      } else if (shouldPersistForegroundBeforeSwitch(currentState)) {
        const curSettings = useSettingsStore.getState();
        const newBgId =
          typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
            ? `bg-${crypto.randomUUID()}`
            : `bg-${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;
        bgSessions = {
          ...bgSessions,
          [newBgId]: createSessionSnapshotFromForeground(currentState, curSettings, newBgId),
        };
      }

      set({
        ...(initialState as ExecutionState),
        connectionStatus: currentState.connectionStatus,
        history: currentState.history,
        backgroundSessions: bgSessions,
        activeSessionId: currentState.activeSessionId,
        foregroundParentSessionId: null,
        foregroundBgId: null,
        foregroundOriginHistoryId: historyId,
        foregroundOriginSessionId: item.sessionId || null,
        foregroundDirty: false,
        streamingOutput: normalizedLines,
        streamLineCounter: counter,
        isChatSession: isClaudeSession,
        taskId: isClaudeSession ? restoredSessionId!.slice(claudePrefix.length) : null,
        standaloneSessionId: isStandaloneSession ? restoredSessionId!.slice(standalonePrefix.length) : null,
        standaloneTurns: isStandaloneSession
          ? trimStandaloneTurns(restoredStandaloneTurns, getStandaloneContextTurnsLimit())
          : [],
        taskDescription: item.title || item.taskDescription,
      });

      restoreSessionLlmSettings({
        llmBackend: item.llmBackend,
        llmProvider: item.llmProvider,
        llmModel: item.llmModel,
      });
    },
  };
}
