/**
 * Workflow Kernel v2 Store
 *
 * Unified frontend facade for workflow_* commands.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { CommandResponse } from '../lib/tauri';
import type {
  HandoffContextBundle,
  ModeTranscriptPatch,
  ModeTranscriptState,
  ModeTranscriptPayload,
  PlanEditOperation,
  ResumeResult,
  UserInputIntent,
  WorkflowSessionCatalogItem,
  WorkflowSessionCatalogState,
  WorkflowSessionCatalogUpdatedEvent,
  WorkflowKernelUpdatedEvent,
  WorkflowMode,
  WorkflowModeTranscriptUpdatedEvent,
  WorkflowSession,
  WorkflowSessionState,
} from '../types/workflowKernel';

const WORKFLOW_KERNEL_UPDATED_CHANNEL = 'workflow-kernel-updated';
const WORKFLOW_SESSION_CATALOG_UPDATED_CHANNEL = 'workflow-session-catalog-updated';
const WORKFLOW_MODE_TRANSCRIPT_UPDATED_CHANNEL = 'workflow-mode-transcript-updated';

const DEFAULT_HANDOFF: HandoffContextBundle = {
  conversationContext: [],
  summaryItems: [],
  artifactRefs: [],
  contextSources: [],
  metadata: {},
};

type TranscriptMap = Record<string, Partial<Record<WorkflowMode, ModeTranscriptState>>>;
const EMPTY_TRANSCRIPT_STATE: ModeTranscriptState = {
  revision: 0,
  lines: [],
  loaded: false,
  unread: false,
};

function cloneTranscriptLines(lines: unknown[]): unknown[] {
  return lines.map((line) =>
    line && typeof line === 'object' ? ({ ...(line as Record<string, unknown>) } as unknown) : line,
  );
}

function upsertTranscriptState(
  current: TranscriptMap,
  sessionId: string,
  mode: WorkflowMode,
  next: Partial<ModeTranscriptState>,
): TranscriptMap {
  const previous = current[sessionId]?.[mode] ?? {
    ...EMPTY_TRANSCRIPT_STATE,
  };
  return {
    ...current,
    [sessionId]: {
      ...current[sessionId],
      [mode]: {
        ...previous,
        ...next,
      },
    },
  };
}

function applyTranscriptPatch(existingLines: unknown[], payload: WorkflowModeTranscriptUpdatedEvent): unknown[] {
  if (payload.replaceFromLineId == null) {
    return [...cloneTranscriptLines(existingLines), ...cloneTranscriptLines(payload.appendedLines)];
  }
  if (payload.replaceFromLineId === 0) {
    return cloneTranscriptLines(payload.lines ?? payload.appendedLines);
  }

  const replaceIndex = existingLines.findIndex((line) => {
    if (!line || typeof line !== 'object') return false;
    const lineId = (line as { id?: unknown }).id;
    return typeof lineId === 'number' && Number.isFinite(lineId) && lineId === payload.replaceFromLineId;
  });
  if (replaceIndex < 0) {
    return cloneTranscriptLines(payload.lines ?? payload.appendedLines);
  }

  return [
    ...cloneTranscriptLines(existingLines.slice(0, replaceIndex)),
    ...cloneTranscriptLines(payload.appendedLines),
  ];
}

function normalizeHandoff(bundle?: HandoffContextBundle): HandoffContextBundle {
  if (!bundle) return DEFAULT_HANDOFF;
  return {
    conversationContext: bundle.conversationContext ?? [],
    summaryItems: bundle.summaryItems ?? [],
    artifactRefs: bundle.artifactRefs ?? [],
    contextSources: bundle.contextSources ?? [],
    metadata: bundle.metadata ?? {},
  };
}

function normalizeSession(session: WorkflowSession): WorkflowSession {
  return {
    ...session,
    handoffContext: normalizeHandoff(session.handoffContext),
    modeSnapshots: {
      ...session.modeSnapshots,
      chat: session.modeSnapshots.chat
        ? {
            ...session.modeSnapshots.chat,
            entryHandoff: normalizeHandoff(session.modeSnapshots.chat.entryHandoff),
          }
        : null,
      plan: session.modeSnapshots.plan
        ? {
            ...session.modeSnapshots.plan,
            entryHandoff: normalizeHandoff(session.modeSnapshots.plan.entryHandoff),
          }
        : null,
      task: session.modeSnapshots.task
        ? {
            ...session.modeSnapshots.task,
            entryHandoff: normalizeHandoff(session.modeSnapshots.task.entryHandoff),
          }
        : null,
      debug: session.modeSnapshots.debug
        ? {
            ...session.modeSnapshots.debug,
            entryHandoff: normalizeHandoff(session.modeSnapshots.debug.entryHandoff),
          }
        : null,
    },
  };
}

function normalizeIntent(intent: UserInputIntent): UserInputIntent {
  return {
    ...intent,
    metadata: intent.metadata ?? {},
  };
}

function normalizePlanEdit(operation: PlanEditOperation): PlanEditOperation {
  return {
    ...operation,
    targetStepId: operation.targetStepId ?? null,
    payload: operation.payload ?? {},
  };
}

export interface WorkflowKernelStore {
  sessionId: string | null;
  activeRootSessionId: string | null;
  activeMode: WorkflowMode;
  session: WorkflowSession | null;
  events: WorkflowSessionState['events'];
  checkpoints: WorkflowSessionState['checkpoints'];
  sessionCatalog: WorkflowSessionCatalogItem[];
  modeTranscriptsBySession: TranscriptMap;
  revision: number;
  isLoading: boolean;
  error: string | null;
  _requestId: number;
  _updatesUnlisten: UnlistenFn | null;
  _catalogUpdatesUnlisten: UnlistenFn | null;
  _transcriptUpdatesUnlisten: UnlistenFn | null;

  openSession: (initialMode?: WorkflowMode, initialContext?: HandoffContextBundle) => Promise<WorkflowSession | null>;
  listSessions: () => Promise<WorkflowSessionCatalogItem[]>;
  getSessionCatalogState: () => Promise<WorkflowSessionCatalogState | null>;
  activateSession: (sessionId: string) => Promise<WorkflowSessionState | null>;
  renameSession: (sessionId: string, displayTitle: string) => Promise<WorkflowSession | null>;
  archiveSession: (sessionId: string) => Promise<WorkflowSessionCatalogState | null>;
  restoreSession: (sessionId: string) => Promise<WorkflowSessionState | null>;
  deleteSession: (sessionId: string) => Promise<WorkflowSessionCatalogState | null>;
  resumeBackgroundRuns: (sessionId?: string | null) => Promise<ResumeResult[]>;
  getModeTranscript: (sessionId: string, mode: WorkflowMode) => Promise<ModeTranscriptPayload | null>;
  getCachedModeTranscript: (sessionId: string | null, mode: WorkflowMode) => ModeTranscriptState;
  appendContextItems: (targetMode: WorkflowMode, handoff: HandoffContextBundle) => Promise<WorkflowSession | null>;
  patchModeTranscript: (
    sessionId: string,
    mode: WorkflowMode,
    patch: ModeTranscriptPatch,
  ) => Promise<ModeTranscriptPayload | null>;
  transitionMode: (targetMode: WorkflowMode, handoff?: HandoffContextBundle) => Promise<WorkflowSession | null>;
  submitInput: (intent: UserInputIntent) => Promise<WorkflowSession | null>;
  transitionAndSubmitInput: (
    targetMode: WorkflowMode,
    intent: UserInputIntent,
    handoff?: HandoffContextBundle,
  ) => Promise<WorkflowSession | null>;
  applyPlanEdit: (operation: PlanEditOperation) => Promise<WorkflowSession | null>;
  executePlan: () => Promise<WorkflowSession | null>;
  retryStep: (stepId: string) => Promise<WorkflowSession | null>;
  cancelOperation: (reason?: string) => Promise<WorkflowSession | null>;
  refreshSessionState: () => Promise<WorkflowSessionState | null>;
  recoverSession: (sessionId: string) => Promise<WorkflowSessionState | null>;
  linkModeSession: (mode: WorkflowMode, modeSessionId: string) => Promise<WorkflowSession | null>;
  markChatTurnFailed: (error: string) => Promise<WorkflowSession | null>;
  subscribeToUpdates: () => Promise<void>;
  unsubscribeFromUpdates: () => void;
  reset: () => void;
}

const DEFAULT_STATE = {
  sessionId: null,
  activeRootSessionId: null,
  activeMode: 'chat' as WorkflowMode,
  session: null as WorkflowSession | null,
  events: [] as WorkflowSessionState['events'],
  checkpoints: [] as WorkflowSessionState['checkpoints'],
  sessionCatalog: [] as WorkflowSessionCatalogItem[],
  modeTranscriptsBySession: {} as TranscriptMap,
  revision: 0,
  isLoading: false,
  error: null as string | null,
  _requestId: 0,
  _updatesUnlisten: null as UnlistenFn | null,
  _catalogUpdatesUnlisten: null as UnlistenFn | null,
  _transcriptUpdatesUnlisten: null as UnlistenFn | null,
};

function applySession(set: (partial: Partial<WorkflowKernelStore>) => void, session: WorkflowSession) {
  const normalizedSession = normalizeSession(session);
  set({
    sessionId: normalizedSession.sessionId,
    activeRootSessionId: normalizedSession.sessionId,
    activeMode: normalizedSession.activeMode,
    session: normalizedSession,
    error: null,
  });
}

function applySessionState(set: (partial: Partial<WorkflowKernelStore>) => void, sessionState: WorkflowSessionState) {
  const normalizedSession = normalizeSession(sessionState.session);
  set({
    sessionId: normalizedSession.sessionId,
    activeRootSessionId: normalizedSession.sessionId,
    activeMode: normalizedSession.activeMode,
    session: normalizedSession,
    events: sessionState.events,
    checkpoints: sessionState.checkpoints,
    revision: sessionState.events.length + sessionState.checkpoints.length,
    error: null,
  });
}

export const useWorkflowKernelStore = create<WorkflowKernelStore>((set, get) => ({
  ...DEFAULT_STATE,

  openSession: async (initialMode = 'chat', initialContext) => {
    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });

    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_open_session', {
        initialMode,
        initialContext: normalizeHandoff(initialContext),
      });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error || 'Failed to open workflow session' });
        return null;
      }

      applySession(set, result.data);
      set({ isLoading: false });
      await get().subscribeToUpdates();
      void get().getSessionCatalogState();
      void get().refreshSessionState();
      return result.data;
    } catch (error) {
      if (get()._requestId !== requestId) return null;
      set({
        isLoading: false,
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  listSessions: async () => {
    try {
      const result = await invoke<CommandResponse<WorkflowSessionCatalogItem[]>>('workflow_list_sessions');
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to list workflow sessions' });
        return [];
      }
      set({ sessionCatalog: result.data, error: null });
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return [];
    }
  },

  getSessionCatalogState: async () => {
    try {
      const result = await invoke<CommandResponse<WorkflowSessionCatalogState>>('workflow_get_session_catalog_state');
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to load workflow session catalog' });
        return null;
      }
      set({
        activeRootSessionId: result.data.activeSessionId,
        sessionCatalog: result.data.sessions,
        error: null,
      });
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  activateSession: async (sessionId) => {
    const normalizedSessionId = sessionId.trim();
    if (!normalizedSessionId) {
      set({ error: 'Session id cannot be empty' });
      return null;
    }
    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const result = await invoke<CommandResponse<WorkflowSessionState>>('workflow_activate_session', {
        sessionId: normalizedSessionId,
      });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error || 'Failed to activate workflow session' });
        return null;
      }
      applySessionState(set, result.data);
      set({ isLoading: false, activeRootSessionId: normalizedSessionId });
      await get().subscribeToUpdates();
      void get().getSessionCatalogState();
      return result.data;
    } catch (error) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  renameSession: async (sessionId, displayTitle) => {
    const normalizedSessionId = sessionId.trim();
    const normalizedTitle = displayTitle.trim();
    if (!normalizedSessionId) {
      set({ error: 'Session id cannot be empty' });
      return null;
    }
    if (!normalizedTitle) {
      set({ error: 'Display title cannot be empty' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_rename_session', {
        sessionId: normalizedSessionId,
        displayTitle: normalizedTitle,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to rename workflow session' });
        return null;
      }
      if (get().sessionId === normalizedSessionId) {
        applySession(set, result.data);
      } else {
        set((state) => ({
          sessionCatalog: mergeCatalogSession(state.sessionCatalog, result.data!),
          error: null,
        }));
      }
      void get().getSessionCatalogState();
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  archiveSession: async (sessionId) => {
    const normalizedSessionId = sessionId.trim();
    if (!normalizedSessionId) {
      set({ error: 'Session id cannot be empty' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSessionCatalogState>>('workflow_archive_session', {
        sessionId: normalizedSessionId,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to archive workflow session' });
        return null;
      }
      const catalogState = result.data;
      set({
        activeRootSessionId: catalogState.activeSessionId,
        sessionCatalog: catalogState.sessions,
        error: null,
      });

      if (!catalogState.activeSessionId) {
        set({
          sessionId: null,
          activeMode: 'chat',
          session: null,
          events: [],
          checkpoints: [],
          revision: 0,
        });
        return catalogState;
      }

      if (catalogState.activeSessionId !== get().sessionId) {
        const sessionStateResult = await invoke<CommandResponse<WorkflowSessionState>>('workflow_get_session_state', {
          sessionId: catalogState.activeSessionId,
        });
        if (sessionStateResult.success && sessionStateResult.data) {
          applySessionState(set, sessionStateResult.data);
        }
      }

      return catalogState;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  restoreSession: async (sessionId) => {
    const normalizedSessionId = sessionId.trim();
    if (!normalizedSessionId) {
      set({ error: 'Session id cannot be empty' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSessionState>>('workflow_restore_session', {
        sessionId: normalizedSessionId,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to restore workflow session' });
        return null;
      }
      applySessionState(set, result.data);
      void get().getSessionCatalogState();
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  deleteSession: async (sessionId) => {
    const normalizedSessionId = sessionId.trim();
    if (!normalizedSessionId) {
      set({ error: 'Session id cannot be empty' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSessionCatalogState>>('workflow_delete_session', {
        sessionId: normalizedSessionId,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to delete workflow session' });
        return null;
      }
      const catalogState = result.data;
      set({
        activeRootSessionId: catalogState.activeSessionId,
        sessionCatalog: catalogState.sessions,
        error: null,
      });
      if (!catalogState.activeSessionId) {
        set({
          sessionId: null,
          activeMode: 'chat',
          session: null,
          events: [],
          checkpoints: [],
          revision: 0,
        });
        return catalogState;
      }

      if (catalogState.activeSessionId !== get().sessionId) {
        const sessionStateResult = await invoke<CommandResponse<WorkflowSessionState>>('workflow_get_session_state', {
          sessionId: catalogState.activeSessionId,
        });
        if (sessionStateResult.success && sessionStateResult.data) {
          applySessionState(set, sessionStateResult.data);
        }
      }

      return catalogState;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  resumeBackgroundRuns: async (sessionId) => {
    try {
      const result = await invoke<CommandResponse<ResumeResult[]>>('workflow_resume_background_runs', {
        sessionId: sessionId || null,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to inspect resumable workflow runs' });
        return [];
      }
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return [];
    }
  },

  getModeTranscript: async (sessionId, mode) => {
    const normalizedSessionId = sessionId.trim();
    if (!normalizedSessionId) {
      set({ error: 'Session id cannot be empty' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<ModeTranscriptPayload>>('workflow_get_mode_transcript', {
        sessionId: normalizedSessionId,
        mode,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to load workflow mode transcript' });
        return null;
      }
      set((state) => ({
        modeTranscriptsBySession: upsertTranscriptState(state.modeTranscriptsBySession, normalizedSessionId, mode, {
          revision: result.data!.revision,
          lines: cloneTranscriptLines(result.data!.lines),
          loaded: true,
          unread: false,
        }),
      }));
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  getCachedModeTranscript: (sessionId, mode) => {
    if (!sessionId) {
      return EMPTY_TRANSCRIPT_STATE;
    }
    return get().modeTranscriptsBySession[sessionId]?.[mode] ?? EMPTY_TRANSCRIPT_STATE;
  },

  appendContextItems: async (targetMode, handoff) => {
    const sessionId = get().sessionId;
    if (!sessionId) {
      set({ error: 'No active workflow session' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_append_context_items', {
        sessionId,
        targetMode,
        handoff: normalizeHandoff(handoff),
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to append workflow context items' });
        return null;
      }
      applySession(set, result.data);
      void get().refreshSessionState();
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  patchModeTranscript: async (sessionId, mode, patch) => {
    const normalizedSessionId = sessionId.trim();
    if (!normalizedSessionId) {
      set({ error: 'Session id cannot be empty' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<ModeTranscriptPayload>>('workflow_patch_mode_transcript', {
        sessionId: normalizedSessionId,
        mode,
        replaceFromLineId: patch.replaceFromLineId ?? null,
        appendedLines: patch.appendedLines,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to patch workflow mode transcript' });
        return null;
      }
      set((state) => ({
        modeTranscriptsBySession: upsertTranscriptState(state.modeTranscriptsBySession, normalizedSessionId, mode, {
          revision: result.data!.revision,
          lines: cloneTranscriptLines(result.data!.lines),
          loaded: true,
          unread: false,
        }),
      }));
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  transitionMode: async (targetMode, handoff) => {
    const currentSession = get().session;
    if (currentSession && currentSession.activeMode === targetMode && !handoff) {
      return currentSession;
    }

    const sessionId = get().sessionId;
    if (!sessionId) {
      const opened = await get().openSession(targetMode, handoff);
      return opened;
    }

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_transition_mode', {
        sessionId,
        targetMode,
        handoff: normalizeHandoff(handoff),
      });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error || 'Failed to transition workflow mode' });
        return null;
      }

      applySession(set, result.data);
      set({ isLoading: false });
      return result.data;
    } catch (error) {
      if (get()._requestId !== requestId) return null;
      set({
        isLoading: false,
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  transitionAndSubmitInput: async (targetMode, intent, handoff) => {
    const currentSession = get().session;
    if (currentSession && currentSession.activeMode === targetMode && !handoff) {
      return get().submitInput(intent);
    }

    let sessionId = get().sessionId;
    if (!sessionId) {
      const opened = await get().openSession(targetMode, handoff);
      if (!opened) return null;
      sessionId = opened.sessionId;
      return get().submitInput(intent);
    }

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_transition_and_submit_input', {
        sessionId,
        targetMode,
        handoff: normalizeHandoff(handoff),
        intent: normalizeIntent(intent),
      });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error || 'Failed to transition and submit workflow input' });
        return null;
      }

      applySession(set, result.data);
      set({ isLoading: false });
      void get().refreshSessionState();
      return result.data;
    } catch (error) {
      if (get()._requestId !== requestId) return null;
      set({
        isLoading: false,
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  submitInput: async (intent) => {
    let sessionId = get().sessionId;
    if (!sessionId) {
      const opened = await get().openSession(get().activeMode);
      if (!opened) return null;
      sessionId = opened.sessionId;
    }

    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_submit_input', {
        sessionId,
        intent: normalizeIntent(intent),
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to submit workflow input' });
        return null;
      }
      applySession(set, result.data);
      return result.data;
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  applyPlanEdit: async (operation) => {
    let sessionId = get().sessionId;
    if (!sessionId) {
      const opened = await get().openSession('plan');
      if (!opened) return null;
      sessionId = opened.sessionId;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_apply_plan_edit', {
        sessionId,
        operation: normalizePlanEdit(operation),
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to apply plan edit' });
        return null;
      }
      applySession(set, result.data);
      return result.data;
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  executePlan: async () => {
    const sessionId = get().sessionId;
    if (!sessionId) {
      set({ error: 'No workflow session available' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_execute_plan', { sessionId });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to execute plan' });
        return null;
      }
      applySession(set, result.data);
      return result.data;
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  retryStep: async (stepId) => {
    const sessionId = get().sessionId;
    if (!sessionId) {
      set({ error: 'No workflow session available' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_retry_step', { sessionId, stepId });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to retry workflow step' });
        return null;
      }
      applySession(set, result.data);
      return result.data;
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  cancelOperation: async (reason) => {
    const sessionId = get().sessionId;
    if (!sessionId) {
      set({ error: 'No workflow session available' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_cancel_operation', {
        sessionId,
        reason: reason || null,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to cancel workflow operation' });
        return null;
      }
      applySession(set, result.data);
      return result.data;
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  refreshSessionState: async () => {
    const sessionId = get().sessionId;
    if (!sessionId) return null;
    try {
      const result = await invoke<CommandResponse<WorkflowSessionState>>('workflow_get_session_state', {
        sessionId,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to load workflow session state' });
        return null;
      }
      applySessionState(set, result.data);
      return result.data;
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  recoverSession: async (sessionId) => {
    if (!sessionId.trim()) {
      set({ error: 'Session id cannot be empty' });
      return null;
    }
    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const result = await invoke<CommandResponse<WorkflowSessionState>>('workflow_recover_session', { sessionId });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error || 'Failed to recover workflow session' });
        return null;
      }
      applySessionState(set, result.data);
      set({ isLoading: false });
      await get().subscribeToUpdates();
      void get().getSessionCatalogState();
      return result.data;
    } catch (error) {
      if (get()._requestId !== requestId) return null;
      set({
        isLoading: false,
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  linkModeSession: async (mode, modeSessionId) => {
    const sessionId = get().sessionId;
    const normalizedModeSessionId = modeSessionId.trim();
    if (!sessionId) {
      set({ error: 'No workflow session available' });
      return null;
    }
    if (!normalizedModeSessionId) {
      set({ error: 'Mode session id cannot be empty' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_link_mode_session', {
        sessionId,
        mode,
        modeSessionId: normalizedModeSessionId,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to link workflow mode session' });
        return null;
      }
      applySession(set, result.data);
      return result.data;
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  },

  markChatTurnFailed: async (error) => {
    const sessionId = get().sessionId;
    const normalizedError = error.trim();
    if (!sessionId) {
      set({ error: 'No workflow session available' });
      return null;
    }
    if (!normalizedError) {
      set({ error: 'Chat failure reason cannot be empty' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<WorkflowSession>>('workflow_mark_chat_turn_failed', {
        sessionId,
        error: normalizedError,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to mark chat turn failed' });
        return null;
      }
      applySession(set, result.data);
      return result.data;
    } catch (invokeError) {
      set({
        error: invokeError instanceof Error ? invokeError.message : String(invokeError),
      });
      return null;
    }
  },

  subscribeToUpdates: async () => {
    if (!get()._updatesUnlisten) {
      try {
        const unlisten = await listen<WorkflowKernelUpdatedEvent>(WORKFLOW_KERNEL_UPDATED_CHANNEL, (event) => {
          const payload = event.payload;
          const incomingSession = payload?.sessionState?.session;
          const incomingSessionId = incomingSession?.sessionId;
          if (!incomingSessionId) return;

          const currentSessionId = get().sessionId;
          if (currentSessionId && currentSessionId !== incomingSessionId) return;
          const normalizedSession = normalizeSession(payload.sessionState.session);

          set({
            sessionId: incomingSessionId,
            activeRootSessionId: incomingSessionId,
            activeMode: normalizedSession.activeMode,
            session: normalizedSession,
            events: payload.sessionState.events,
            checkpoints: payload.sessionState.checkpoints,
            revision: payload.revision,
            error: null,
          });
        });
        set({ _updatesUnlisten: unlisten });
      } catch {
        // Event subscription failure should not block workflow.
      }
    }

    if (!get()._catalogUpdatesUnlisten) {
      try {
        const unlisten = await listen<WorkflowSessionCatalogUpdatedEvent>(
          WORKFLOW_SESSION_CATALOG_UPDATED_CHANNEL,
          (event) => {
            const payload = event.payload;
            if (!payload) return;
            set({
              activeRootSessionId: payload.activeSessionId,
              sessionCatalog: payload.sessions,
            });
          },
        );
        set({ _catalogUpdatesUnlisten: unlisten });
      } catch {
        // Event subscription failure should not block workflow.
      }
    }

    if (!get()._transcriptUpdatesUnlisten) {
      try {
        const unlisten = await listen<WorkflowModeTranscriptUpdatedEvent>(
          WORKFLOW_MODE_TRANSCRIPT_UPDATED_CHANNEL,
          (event) => {
            const payload = event.payload;
            if (!payload?.sessionId) return;
            set((state) => {
              const previous = state.modeTranscriptsBySession[payload.sessionId]?.[payload.mode];
              const existingLines = previous?.lines ?? [];
              const nextLines = applyTranscriptPatch(existingLines, payload);
              return {
                modeTranscriptsBySession: upsertTranscriptState(
                  state.modeTranscriptsBySession,
                  payload.sessionId,
                  payload.mode,
                  {
                    revision: payload.revision,
                    lines: nextLines,
                    loaded: true,
                    unread:
                      !!state.activeRootSessionId &&
                      (state.activeRootSessionId !== payload.sessionId || state.activeMode !== payload.mode) &&
                      payload.appendedLines.length > 0,
                  },
                ),
              };
            });
          },
        );
        set({ _transcriptUpdatesUnlisten: unlisten });
      } catch {
        // Event subscription failure should not block workflow.
      }
    }
  },

  unsubscribeFromUpdates: () => {
    const unlisten = get()._updatesUnlisten;
    if (unlisten) {
      unlisten();
      set({ _updatesUnlisten: null });
    }
    const catalogUnlisten = get()._catalogUpdatesUnlisten;
    if (catalogUnlisten) {
      catalogUnlisten();
      set({ _catalogUpdatesUnlisten: null });
    }
    const transcriptUnlisten = get()._transcriptUpdatesUnlisten;
    if (transcriptUnlisten) {
      transcriptUnlisten();
      set({ _transcriptUpdatesUnlisten: null });
    }
  },

  reset: () => {
    get().unsubscribeFromUpdates();
    set({ ...DEFAULT_STATE });
  },
}));

function mergeCatalogSession(
  catalog: WorkflowSessionCatalogItem[],
  session: WorkflowSession,
): WorkflowSessionCatalogItem[] {
  const nextItem: WorkflowSessionCatalogItem = {
    sessionId: session.sessionId,
    sessionKind: session.sessionKind,
    displayTitle: session.displayTitle,
    workspacePath: session.workspacePath,
    activeMode: session.activeMode,
    status: session.status,
    backgroundState: session.backgroundState,
    updatedAt: session.updatedAt,
    createdAt: session.createdAt,
    lastError: session.lastError,
    contextLedger: session.contextLedger,
    modeSnapshots: session.modeSnapshots,
    modeRuntimeMeta: session.modeRuntimeMeta,
  };
  const existingIndex = catalog.findIndex((item) => item.sessionId === session.sessionId);
  if (existingIndex === -1) {
    return [nextItem, ...catalog];
  }
  const nextCatalog = [...catalog];
  nextCatalog[existingIndex] = nextItem;
  return nextCatalog.sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
}
