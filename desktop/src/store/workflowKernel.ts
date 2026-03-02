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
  PlanEditOperation,
  UserInputIntent,
  WorkflowKernelUpdatedEvent,
  WorkflowMode,
  WorkflowSession,
  WorkflowSessionState,
} from '../types/workflowKernel';

const WORKFLOW_KERNEL_UPDATED_CHANNEL = 'workflow-kernel-updated';

const DEFAULT_HANDOFF: HandoffContextBundle = {
  conversationContext: [],
  artifactRefs: [],
  contextSources: [],
  metadata: {},
};

function normalizeHandoff(bundle?: HandoffContextBundle): HandoffContextBundle {
  if (!bundle) return DEFAULT_HANDOFF;
  return {
    conversationContext: bundle.conversationContext ?? [],
    artifactRefs: bundle.artifactRefs ?? [],
    contextSources: bundle.contextSources ?? [],
    metadata: bundle.metadata ?? {},
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
  activeMode: WorkflowMode;
  session: WorkflowSession | null;
  events: WorkflowSessionState['events'];
  checkpoints: WorkflowSessionState['checkpoints'];
  revision: number;
  isLoading: boolean;
  error: string | null;
  _requestId: number;
  _updatesUnlisten: UnlistenFn | null;

  openSession: (initialMode?: WorkflowMode, initialContext?: HandoffContextBundle) => Promise<WorkflowSession | null>;
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
  subscribeToUpdates: () => Promise<void>;
  unsubscribeFromUpdates: () => void;
  reset: () => void;
}

const DEFAULT_STATE = {
  sessionId: null,
  activeMode: 'chat' as WorkflowMode,
  session: null as WorkflowSession | null,
  events: [] as WorkflowSessionState['events'],
  checkpoints: [] as WorkflowSessionState['checkpoints'],
  revision: 0,
  isLoading: false,
  error: null as string | null,
  _requestId: 0,
  _updatesUnlisten: null as UnlistenFn | null,
};

function applySession(set: (partial: Partial<WorkflowKernelStore>) => void, session: WorkflowSession) {
  set({
    sessionId: session.sessionId,
    activeMode: session.activeMode,
    session,
    error: null,
  });
}

function applySessionState(set: (partial: Partial<WorkflowKernelStore>) => void, sessionState: WorkflowSessionState) {
  set({
    sessionId: sessionState.session.sessionId,
    activeMode: sessionState.session.activeMode,
    session: sessionState.session,
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

  subscribeToUpdates: async () => {
    if (get()._updatesUnlisten) return;
    try {
      const unlisten = await listen<WorkflowKernelUpdatedEvent>(WORKFLOW_KERNEL_UPDATED_CHANNEL, (event) => {
        const payload = event.payload;
        const incomingSessionId = payload?.sessionState?.session?.sessionId;
        if (!incomingSessionId) return;
        const currentSessionId = get().sessionId;
        if (currentSessionId && currentSessionId !== incomingSessionId) return;

        set({
          sessionId: incomingSessionId,
          activeMode: payload.sessionState.session.activeMode,
          session: payload.sessionState.session,
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
  },

  unsubscribeFromUpdates: () => {
    const unlisten = get()._updatesUnlisten;
    if (unlisten) {
      unlisten();
      set({ _updatesUnlisten: null });
    }
  },

  reset: () => {
    get().unsubscribeFromUpdates();
    set({ ...DEFAULT_STATE });
  },
}));
