/**
 * Plan Mode Store
 *
 * Zustand store for Plan Mode lifecycle management.
 * Manages mode switching, plan generation/review, execution monitoring,
 * and step outputs via Tauri IPC commands and events.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  PlanModePhase,
  PlanModeSession,
  PlanAnalysisCardData,
  PlanCardData,
  PlanExecutionReport,
  PlanModeProgressPayload,
  PlanExecutionStatusResponse,
  AdapterInfo,
  StepOutputData,
  PlanClarifyAnswerCardData,
  PlanClarifyQuestionCardData,
} from '../types/planModeCard';
import type { CommandResponse } from '../lib/tauri';
import type { ContextSourceConfig } from './contextSources';

// ============================================================================
// Store Interface
// ============================================================================

export interface PlanModeState {
  // State
  isPlanMode: boolean;
  sessionId: string | null;
  sessionPhase: PlanModePhase;
  analysis: PlanAnalysisCardData | null;
  currentQuestion: PlanClarifyQuestionCardData | null;
  plan: PlanCardData | null;
  currentBatch: number;
  totalBatches: number;
  stepStatuses: Record<string, string>;
  report: PlanExecutionReport | null;
  adapters: AdapterInfo[];
  isLoading: boolean;
  isCancelling: boolean;
  error: string | null;
  _unlistenFn: UnlistenFn | null;
  _requestId: number;

  // Actions
  enterPlanMode: (
    description: string,
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    conversationContext?: string,
    locale?: string,
  ) => Promise<void>;
  submitClarification: (
    answer: PlanClarifyAnswerCardData,
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    conversationContext?: string,
    locale?: string,
  ) => Promise<PlanModeSession | null>;
  skipClarification: () => Promise<void>;
  generatePlan: (
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    conversationContext?: string,
    locale?: string,
  ) => Promise<void>;
  approvePlan: (
    plan: PlanCardData,
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    conversationContext?: string,
    locale?: string,
  ) => Promise<void>;
  refreshStatus: () => Promise<void>;
  cancelExecution: () => Promise<void>;
  cancelOperation: () => Promise<void>;
  fetchReport: () => Promise<void>;
  fetchStepOutput: (stepId: string) => Promise<StepOutputData | null>;
  exitPlanMode: () => Promise<void>;
  fetchAdapters: () => Promise<void>;
  subscribeToEvents: () => Promise<void>;
  unsubscribeFromEvents: () => void;
  reset: () => void;
}

// ============================================================================
// Default State
// ============================================================================

const DEFAULT_STATE = {
  isPlanMode: false,
  sessionId: null,
  sessionPhase: 'idle' as PlanModePhase,
  analysis: null,
  currentQuestion: null as PlanClarifyQuestionCardData | null,
  plan: null,
  currentBatch: 0,
  totalBatches: 0,
  stepStatuses: {} as Record<string, string>,
  report: null,
  adapters: [] as AdapterInfo[],
  isLoading: false,
  isCancelling: false,
  error: null,
  _unlistenFn: null,
  _requestId: 0,
};

// ============================================================================
// Store
// ============================================================================

export const usePlanModeStore = create<PlanModeState>((set, get) => ({
  ...DEFAULT_STATE,

  enterPlanMode: async (
    description,
    provider,
    model,
    baseUrl,
    projectPath,
    contextSources,
    conversationContext,
    locale,
  ) => {
    const requestId = get()._requestId + 1;
    set({ isLoading: true, isCancelling: false, error: null, _requestId: requestId });
    try {
      // Resolve base URL for multi-endpoint providers (Qwen, GLM, MiniMax)
      let finalBaseUrl = baseUrl;
      if (!finalBaseUrl && provider) {
        const settingsStore = (await import('./settings')).useSettingsStore.getState();
        const { resolveProviderBaseUrl } = await import('../lib/providers');
        finalBaseUrl = resolveProviderBaseUrl(provider, settingsStore);
      }

      const result = await invoke<CommandResponse<PlanModeSession>>('enter_plan_mode', {
        description,
        provider: provider || null,
        model: model || null,
        baseUrl: finalBaseUrl || null,
        projectPath: projectPath || null,
        contextSources: contextSources || null,
        conversationContext: conversationContext || null,
        locale: locale || null,
      });
      if (get()._requestId !== requestId) return;

      if (result.success && result.data) {
        set({
          isPlanMode: true,
          sessionId: result.data.sessionId,
          sessionPhase: result.data.phase,
          analysis: result.data.analysis,
          currentQuestion: result.data.currentQuestion ?? null,
          isLoading: false,
        });
        // Subscribe to events
        await get().subscribeToEvents();
      } else {
        set({ isLoading: false, error: result.error || 'Failed to enter plan mode' });
      }
    } catch (e) {
      if (get()._requestId !== requestId) return;
      set({ isLoading: false, error: String(e) });
    }
  },

  submitClarification: async (
    answer,
    provider,
    model,
    baseUrl,
    projectPath,
    contextSources,
    conversationContext,
    locale,
  ) => {
    const { sessionId } = get();
    if (!sessionId) return null;
    const requestId = get()._requestId + 1;
    set({ _requestId: requestId });

    try {
      const settingsStore = (await import('./settings')).useSettingsStore.getState();
      const finalProvider = provider || settingsStore.provider;
      const finalModel = model || settingsStore.model;
      const { resolveProviderBaseUrl } = await import('../lib/providers');
      const finalBaseUrl = baseUrl || resolveProviderBaseUrl(finalProvider, settingsStore);

      const result = await invoke<CommandResponse<PlanModeSession>>('submit_plan_clarification', {
        sessionId,
        answer,
        provider: finalProvider || null,
        model: finalModel || null,
        baseUrl: finalBaseUrl || null,
        projectPath: projectPath || null,
        contextSources: contextSources || null,
        conversationContext: conversationContext || null,
        locale: locale || null,
      });
      if (get()._requestId !== requestId) return null;
      if (result.success && result.data) {
        set({
          sessionPhase: result.data.phase,
          currentQuestion: result.data.currentQuestion ?? null,
        });
        return result.data;
      }
      return null;
    } catch (e) {
      if (get()._requestId !== requestId) return null;
      set({ error: String(e) });
      return null;
    }
  },

  skipClarification: async () => {
    const { sessionId } = get();
    if (!sessionId) return;
    const requestId = get()._requestId + 1;
    set({ _requestId: requestId });

    try {
      const result = await invoke<CommandResponse<PlanModeSession>>('skip_plan_clarification', {
        sessionId,
      });
      if (get()._requestId !== requestId) return;
      if (result.success && result.data) {
        set({ sessionPhase: result.data.phase });
      }
    } catch (e) {
      if (get()._requestId !== requestId) return;
      set({ error: String(e) });
    }
  },

  generatePlan: async (provider, model, baseUrl, projectPath, contextSources, conversationContext, locale) => {
    const { sessionId } = get();
    if (!sessionId) return;

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const settingsStore = (await import('./settings')).useSettingsStore.getState();
      const finalProvider = provider || settingsStore.provider;
      const finalModel = model || settingsStore.model;
      const { resolveProviderBaseUrl } = await import('../lib/providers');
      const finalBaseUrl = baseUrl || resolveProviderBaseUrl(finalProvider, settingsStore);

      const result = await invoke<CommandResponse<PlanCardData>>('generate_plan', {
        sessionId,
        provider: finalProvider || null,
        model: finalModel || null,
        baseUrl: finalBaseUrl || null,
        projectPath: projectPath || null,
        contextSources: contextSources || null,
        conversationContext: conversationContext || null,
        locale: locale || null,
      });
      if (get()._requestId !== requestId) return;

      if (result.success && result.data) {
        set({
          plan: result.data,
          sessionPhase: 'reviewing_plan',
          isLoading: false,
        });
      } else {
        set({ isLoading: false, error: result.error || 'Failed to generate plan' });
      }
    } catch (e) {
      if (get()._requestId !== requestId) return;
      set({ isLoading: false, error: String(e) });
    }
  },

  approvePlan: async (plan, provider, model, baseUrl, projectPath, contextSources, conversationContext, locale) => {
    const { sessionId } = get();
    if (!sessionId) return;

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const settingsStore = (await import('./settings')).useSettingsStore.getState();
      const finalProvider = provider || settingsStore.provider;
      const finalModel = model || settingsStore.model;
      const { resolveProviderBaseUrl } = await import('../lib/providers');
      const finalBaseUrl = baseUrl || resolveProviderBaseUrl(finalProvider, settingsStore);

      const result = await invoke<CommandResponse<boolean>>('approve_plan', {
        sessionId,
        plan,
        provider: finalProvider || null,
        model: finalModel || null,
        baseUrl: finalBaseUrl || null,
        projectPath: projectPath || null,
        contextSources: contextSources || null,
        conversationContext: conversationContext || null,
        locale: locale || null,
      });
      if (get()._requestId !== requestId) return;

      if (result.success) {
        set({ sessionPhase: 'executing', isLoading: false });
      } else {
        set({ isLoading: false, error: result.error || 'Failed to approve plan' });
      }
    } catch (e) {
      if (get()._requestId !== requestId) return;
      set({ isLoading: false, error: String(e) });
    }
  },

  refreshStatus: async () => {
    const { sessionId } = get();
    if (!sessionId) return;

    try {
      const result = await invoke<CommandResponse<PlanExecutionStatusResponse>>('get_plan_execution_status', {
        sessionId,
      });
      if (result.success && result.data) {
        set({
          sessionPhase: result.data.phase,
          totalBatches: result.data.totalBatches,
        });
      }
    } catch {
      // Silently ignore polling errors
    }
  },

  cancelExecution: async () => {
    const { sessionId } = get();
    if (!sessionId) return;
    if (get().isCancelling) return;
    const requestId = get()._requestId + 1;
    set({ _requestId: requestId, isCancelling: true });

    try {
      const result = await invoke<CommandResponse<boolean>>('cancel_plan_execution', { sessionId });
      if (get()._requestId !== requestId) return;
      if (!result.success) {
        set({ isCancelling: false, error: result.error || 'Failed to cancel plan execution' });
      }
    } catch (e) {
      if (get()._requestId !== requestId) return;
      set({ isCancelling: false, error: String(e) });
    }
  },

  cancelOperation: async () => {
    const { sessionId } = get();
    const requestId = get()._requestId + 1;
    set({ _requestId: requestId });

    try {
      const result = await invoke<CommandResponse<boolean>>('cancel_plan_operation', {
        sessionId: sessionId || null,
      });
      if (get()._requestId !== requestId) return;
      if (!result.success) {
        set({ error: result.error || 'Failed to cancel plan operation' });
      }
    } catch (e) {
      if (get()._requestId !== requestId) return;
      set({ error: String(e) });
    }
  },

  fetchReport: async () => {
    const { sessionId } = get();
    if (!sessionId) return;

    try {
      const result = await invoke<CommandResponse<PlanExecutionReport>>('get_plan_execution_report', { sessionId });
      if (result.success && result.data) {
        set({ report: result.data });
      }
    } catch (e) {
      set({ error: String(e) });
    }
  },

  fetchStepOutput: async (stepId) => {
    const { sessionId } = get();
    const normalizedStepId = stepId.trim();
    if (!sessionId || !normalizedStepId) return null;

    try {
      const result = await invoke<CommandResponse<StepOutputData>>('get_step_output', {
        sessionId,
        stepId: normalizedStepId,
      });
      if (result.success && result.data) {
        return result.data;
      }
      set({ error: result.error || `Failed to fetch output for step '${normalizedStepId}'` });
      return null;
    } catch (e) {
      set({ error: String(e) });
      return null;
    }
  },

  exitPlanMode: async () => {
    const { sessionId, _unlistenFn } = get();
    const nextRequestId = get()._requestId + 1;
    set({ _requestId: nextRequestId });

    if (_unlistenFn) {
      _unlistenFn();
    }

    if (sessionId) {
      try {
        await invoke<CommandResponse<boolean>>('exit_plan_mode', { sessionId });
      } catch {
        // Ignore cleanup errors
      }
    }

    set({ ...DEFAULT_STATE, _requestId: nextRequestId });
  },

  fetchAdapters: async () => {
    try {
      const result = await invoke<CommandResponse<AdapterInfo[]>>('list_plan_adapters');
      if (result.success && result.data) {
        set({ adapters: result.data });
      }
    } catch {
      // Silently ignore
    }
  },

  subscribeToEvents: async () => {
    const unlisten = await listen<PlanModeProgressPayload>('plan-mode-progress', (event) => {
      const payload = event.payload;
      const { sessionId } = get();

      // Only process events for our session
      if (payload.sessionId !== sessionId) return;

      const updates: Partial<PlanModeState> = {
        currentBatch: payload.currentBatch,
        totalBatches: payload.totalBatches,
      };

      // Accumulate step statuses
      if (payload.stepId && payload.stepStatus) {
        const prevStatuses = get().stepStatuses;
        updates.stepStatuses = { ...prevStatuses, [payload.stepId]: payload.stepStatus };
      }

      // Determine session phase from event type
      if (payload.eventType === 'execution_completed') {
        const allStatuses = { ...get().stepStatuses, ...updates.stepStatuses };
        const failedCount = Object.values(allStatuses).filter((s) => s === 'failed').length;
        updates.sessionPhase = failedCount > 0 ? 'failed' : 'completed';
        updates.isCancelling = false;
      } else if (payload.eventType === 'execution_cancelled') {
        updates.sessionPhase = 'cancelled';
        updates.isCancelling = false;
      } else if (payload.eventType === 'step_failed' && payload.error) {
        updates.error = payload.error;
      }

      set(updates);
    });

    set({ _unlistenFn: unlisten });
  },

  unsubscribeFromEvents: () => {
    const { _unlistenFn } = get();
    if (_unlistenFn) {
      _unlistenFn();
      set({ _unlistenFn: null });
    }
  },

  reset: () => {
    const { _unlistenFn } = get();
    if (_unlistenFn) _unlistenFn();
    set((state) => ({ ...DEFAULT_STATE, _requestId: state._requestId + 1 }));
  },
}));
