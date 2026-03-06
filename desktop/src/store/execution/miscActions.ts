import { invoke } from '@tauri-apps/api/core';
import { deriveConversationTurns, rebuildStandaloneTurns } from '../../lib/conversationUtils';
import { reportNonFatal } from '../../lib/nonFatal';
import type { CardPayload } from '../../types/workflowCard';
import { ToolCallStreamFilter } from '../../utils/toolCallFilter';
import { useAgentsStore } from '../agents';
import { useSettingsStore } from '../settings';
import { useToolPermissionStore } from '../toolPermission';
import { isClaudeCodeBackend } from './providerUtils';
import type { ExecutionState, SessionSnapshot, StandaloneTurn } from './types';

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

type ExecutionSetState = (
  partial: Partial<ExecutionState> | ((state: ExecutionState) => Partial<ExecutionState>),
) => void;

interface MiscActionDeps {
  set: ExecutionSetState;
  get: () => ExecutionState;
  initialState: Partial<ExecutionState>;
  hasMeaningfulForegroundContent: (state: ExecutionState) => boolean;
  createSessionSnapshotFromForeground: (
    state: ExecutionState,
    settings: ReturnType<typeof useSettingsStore.getState>,
    id: string,
  ) => SessionSnapshot;
  getStandaloneContextTurnsLimit: () => number;
  trimStandaloneTurns: (turns: StandaloneTurn[], limit: number) => StandaloneTurn[];
}

interface MiscActions {
  pause: ExecutionState['pause'];
  resume: ExecutionState['resume'];
  reset: ExecutionState['reset'];
  analyzeStrategy: ExecutionState['analyzeStrategy'];
  loadStrategyOptions: ExecutionState['loadStrategyOptions'];
  clearStrategyAnalysis: ExecutionState['clearStrategyAnalysis'];
  appendStreamLine: ExecutionState['appendStreamLine'];
  appendCard: (payload: CardPayload, subAgentId?: string, subAgentDepth?: number) => void;
  clearStreamingOutput: ExecutionState['clearStreamingOutput'];
  updateQualityGate: ExecutionState['updateQualityGate'];
  addExecutionError: ExecutionState['addExecutionError'];
  dismissError: ExecutionState['dismissError'];
  clearExecutionErrors: ExecutionState['clearExecutionErrors'];
  addAttachment: ExecutionState['addAttachment'];
  removeAttachment: ExecutionState['removeAttachment'];
  clearAttachments: ExecutionState['clearAttachments'];
  retryStory: ExecutionState['retryStory'];
  rollbackToTurn: ExecutionState['rollbackToTurn'];
  appendStandaloneTurn: ExecutionState['appendStandaloneTurn'];
}

export function createMiscActions(deps: MiscActionDeps): MiscActions {
  const {
    set,
    get,
    initialState,
    hasMeaningfulForegroundContent,
    createSessionSnapshotFromForeground,
    getStandaloneContextTurnsLimit,
    trimStandaloneTurns,
  } = deps;

  return {
    pause: async () => {
      try {
        const { standaloneSessionId } = get();
        if (standaloneSessionId) {
          await invoke<CommandResponse<boolean>>('pause_standalone_execution', {
            sessionId: standaloneSessionId,
          });
        }
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
        const { standaloneSessionId } = get();
        if (standaloneSessionId) {
          await invoke<CommandResponse<boolean>>('unpause_standalone_execution', {
            sessionId: standaloneSessionId,
          });
        }
        set({ status: 'running' });
        get().addLog('Execution resumed');
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ apiError: errorMessage });
        get().addLog(`Failed to resume: ${errorMessage}`);
      }
    },

    reset: () => {
      const state = get();

      if ((state.status === 'running' || state.status === 'paused') && hasMeaningfulForegroundContent(state)) {
        if (state.isChatSession) {
          get().parkForegroundRuntime();
        } else {
          get().backgroundCurrentSession();
        }
        const postBgState = get();
        set({
          ...initialState,
          connectionStatus: postBgState.connectionStatus,
          history: postBgState.history,
          backgroundSessions: postBgState.backgroundSessions,
          runtimeRegistry: postBgState.runtimeRegistry,
          activeRuntimeHandleId: postBgState.activeRuntimeHandleId,
          activeSessionId: postBgState.activeSessionId,
          foregroundParentSessionId: null,
          foregroundBgId: null,
          foregroundOriginHistoryId: null,
          foregroundOriginSessionId: null,
          foregroundDirty: false,
          toolCallFilter: new ToolCallStreamFilter(),
        });
        return;
      }

      let backgroundSessions = state.backgroundSessions;
      if (state.foregroundBgId && backgroundSessions[state.foregroundBgId]) {
        const settingsState = useSettingsStore.getState();
        const updatedGhost = createSessionSnapshotFromForeground(state, settingsState, state.foregroundBgId);
        backgroundSessions = { ...backgroundSessions, [state.foregroundBgId]: updatedGhost };
      }

      if (state.isChatSession && state.streamingOutput.length > 0) {
        get().saveToHistory();
      }

      set({
        ...initialState,
        connectionStatus: state.connectionStatus,
        history: get().history,
        backgroundSessions,
        runtimeRegistry: state.runtimeRegistry,
        activeRuntimeHandleId: state.activeRuntimeHandleId,
        activeSessionId: state.activeSessionId,
        foregroundParentSessionId: null,
        foregroundBgId: null,
        foregroundOriginHistoryId: null,
        foregroundOriginSessionId: null,
        foregroundDirty: false,
        toolCallFilter: new ToolCallStreamFilter(),
      });

      useToolPermissionStore.getState().reset();
      useAgentsStore.getState().clearActiveAgent();
    },

    analyzeStrategy: async (description: string) => {
      if (!description.trim()) return null;

      set({ isAnalyzingStrategy: true });
      get().addLog('Analyzing task strategy...');

      try {
        const result = await invoke<CommandResponse<NonNullable<ExecutionState['strategyAnalysis']>>>(
          'analyze_task_strategy',
          {
            description,
            context: null,
          },
        );

        if (result.success && result.data) {
          const analysis = result.data;
          set({
            strategyAnalysis: analysis,
            isAnalyzingStrategy: false,
            strategy: analysis.strategy as ExecutionState['strategy'],
          });
          get().addLog(
            `Strategy recommendation: ${analysis.strategy} (confidence: ${(analysis.confidence * 100).toFixed(0)}%)`,
          );
          return analysis;
        }
        throw new Error(result.error || 'Strategy analysis failed');
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({ isAnalyzingStrategy: false });
        get().addLog(`Strategy analysis error: ${errorMessage}`);
        return null;
      }
    },

    loadStrategyOptions: async () => {
      try {
        const result = await invoke<CommandResponse<ExecutionState['strategyOptions']>>('get_strategy_options');
        if (result.success && result.data) {
          set({ strategyOptions: result.data });
        }
      } catch (error) {
        reportNonFatal('execution.loadStrategyOptions', error);
      }
    },

    clearStrategyAnalysis: () => {
      set({
        strategyAnalysis: null,
        isAnalyzingStrategy: false,
      });
    },

    appendStreamLine: (content, type, subAgentId, subAgentDepth, options) => {
      set((state) => {
        const lines = state.streamingOutput;
        const last = lines.length > 0 ? lines[lines.length - 1] : null;
        const targetTurnId = options?.turnId;

        if (
          (type === 'text' || type === 'thinking') &&
          last &&
          last.type === type &&
          last.subAgentId === subAgentId &&
          (targetTurnId === undefined || last.turnId === targetTurnId)
        ) {
          const updated = { ...last, content: last.content + content };
          const newLines = lines.slice();
          newLines[newLines.length - 1] = updated;
          return {
            streamingOutput: newLines,
            foregroundDirty: true,
          };
        }

        const counter = state.streamLineCounter + 1;
        return {
          streamingOutput: [
            ...lines,
            {
              id: counter,
              content,
              type,
              timestamp: Date.now(),
              subAgentId,
              subAgentDepth,
              turnId: targetTurnId,
              turnBoundary: options?.turnBoundary,
            },
          ],
          streamLineCounter: counter,
          foregroundDirty: true,
        };
      });
    },

    appendCard: (payload, subAgentId, subAgentDepth) => {
      set((state) => {
        const counter = state.streamLineCounter + 1;
        return {
          streamingOutput: [
            ...state.streamingOutput,
            {
              id: counter,
              content: JSON.stringify(payload),
              type: 'card' as const,
              timestamp: Date.now(),
              cardPayload: payload,
              subAgentId,
              subAgentDepth,
            },
          ],
          streamLineCounter: counter,
          foregroundDirty: true,
        };
      });
    },

    clearStreamingOutput: () => {
      set({ streamingOutput: [], streamLineCounter: 0, foregroundDirty: true });
    },

    updateQualityGate: (result) => {
      set((state) => {
        const existing = state.qualityGateResults.findIndex(
          (r) => r.gateId === result.gateId && r.storyId === result.storyId,
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
      const newError = {
        ...error,
        id: `err-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
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
        executionErrors: state.executionErrors.map((entry) =>
          entry.id === errorId ? { ...entry, dismissed: true } : entry,
        ),
      }));
    },

    clearExecutionErrors: () => {
      set({ executionErrors: [] });
    },

    addAttachment: (file) => {
      set((state) => {
        if (state.attachments.some((attachment) => attachment.id === file.id)) return state;
        return { attachments: [...state.attachments, file] };
      });
    },

    removeAttachment: (id) => {
      set((state) => ({
        attachments: state.attachments.filter((attachment) => attachment.id !== id),
      }));
    },

    clearAttachments: () => {
      set({ attachments: [] });
    },

    retryStory: async (storyId) => {
      const story = get().stories.find((item) => item.id === storyId);
      if (!story) return;

      get().updateStory(storyId, {
        status: 'in_progress',
        progress: 0,
        error: undefined,
        retryCount: (story.retryCount || 0) + 1,
      });

      set((state) => ({
        executionErrors: state.executionErrors.filter((error) => error.storyId !== storyId),
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

    rollbackToTurn: (userLineId: number) => {
      const lines = get().streamingOutput;
      const turns = deriveConversationTurns(lines);
      const targetTurn = turns.find((turn) => turn.userLineId === userLineId);
      if (!targetTurn) return;

      const taskId = get().taskId;
      const standaloneSessionId = get().standaloneSessionId;
      const fileChangeSessionId = taskId || standaloneSessionId;
      const settingsSnapshot = useSettingsStore.getState();
      const workspacePath = (settingsSnapshot as { workspacePath?: string }).workspacePath;
      if (fileChangeSessionId && workspacePath) {
        void invoke('restore_files_to_turn_v2', {
          sessionId: fileChangeSessionId,
          projectRoot: workspacePath,
          turnIndex: targetTurn.turnIndex,
          createSnapshot: true,
        })
          .then(() =>
            invoke('truncate_changes_from_turn', {
              sessionId: fileChangeSessionId,
              projectRoot: workspacePath,
              turnIndex: targetTurn.turnIndex,
            }),
          )
          .catch((error: unknown) => {
            reportNonFatal('execution.rollback.restoreFiles', error, {
              sessionId: fileChangeSessionId,
              turnIndex: targetTurn.turnIndex,
            });
          });
      }

      const truncatedLines = lines.slice(0, targetTurn.assistantEndIndex + 1);
      const rebuiltTurns = rebuildStandaloneTurns(truncatedLines);

      const backendValue = String((settingsSnapshot as { backend?: unknown }).backend || '');
      if (isClaudeCodeBackend(backendValue) && taskId) {
        void invoke('cancel_execution', { session_id: taskId }).catch((error: unknown) => {
          reportNonFatal('execution.rollback.cancelExecution', error, { taskId });
        });
      }

      set({
        streamingOutput: truncatedLines,
        streamLineCounter: truncatedLines.length > 0 ? truncatedLines[truncatedLines.length - 1].id : 0,
        standaloneTurns: rebuiltTurns,
        status: 'idle',
        isCancelling: false,
        pendingCancelBeforeSessionReady: false,
        activeExecutionId: null,
        isSubmitting: false,
        apiError: null,
        result: null,
        foregroundDirty: true,
      });

      get().addLog(`Rolled back to turn with userLineId=${userLineId}`);
    },

    appendStandaloneTurn: (turn) => {
      const limit = getStandaloneContextTurnsLimit();
      set({
        standaloneTurns: trimStandaloneTurns([...get().standaloneTurns, turn], limit),
      });
    },
  };
}
