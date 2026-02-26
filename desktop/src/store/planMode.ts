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
  PlanClarifyAnswerCardData,
  PlanClarifyQuestionCardData,
} from '../types/planModeCard';
import type { CommandResponse } from '../lib/tauri';

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
  error: string | null;
  _unlistenFn: UnlistenFn | null;

  // Actions
  enterPlanMode: (
    description: string,
    provider?: string,
    model?: string,
    baseUrl?: string,
    conversationContext?: string,
    locale?: string,
  ) => Promise<void>;
  submitClarification: (
    answer: PlanClarifyAnswerCardData,
    provider?: string,
    model?: string,
    baseUrl?: string,
    locale?: string,
  ) => Promise<PlanModeSession | null>;
  skipClarification: () => Promise<void>;
  generatePlan: (
    provider?: string,
    model?: string,
    baseUrl?: string,
    conversationContext?: string,
    locale?: string,
  ) => Promise<void>;
  approvePlan: (
    plan: PlanCardData,
    provider?: string,
    model?: string,
    baseUrl?: string,
    locale?: string,
  ) => Promise<void>;
  refreshStatus: () => Promise<void>;
  cancelExecution: () => Promise<void>;
  fetchReport: () => Promise<void>;
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
  error: null,
  _unlistenFn: null,
};

// ============================================================================
// Store
// ============================================================================

export const usePlanModeStore = create<PlanModeState>((set, get) => ({
  ...DEFAULT_STATE,

  enterPlanMode: async (description, provider, model, baseUrl, conversationContext, locale) => {
    set({ isLoading: true, error: null });
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
        conversationContext: conversationContext || null,
        locale: locale || null,
      });

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
      set({ isLoading: false, error: String(e) });
    }
  },

  submitClarification: async (answer, provider, model, baseUrl, locale) => {
    const { sessionId } = get();
    if (!sessionId) return null;

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
        locale: locale || null,
      });
      if (result.success && result.data) {
        set({
          sessionPhase: result.data.phase,
          currentQuestion: result.data.currentQuestion ?? null,
        });
        return result.data;
      }
      return null;
    } catch (e) {
      set({ error: String(e) });
      return null;
    }
  },

  skipClarification: async () => {
    const { sessionId } = get();
    if (!sessionId) return;

    try {
      const result = await invoke<CommandResponse<PlanModeSession>>('skip_plan_clarification', {
        sessionId,
      });
      if (result.success && result.data) {
        set({ sessionPhase: result.data.phase });
      }
    } catch (e) {
      set({ error: String(e) });
    }
  },

  generatePlan: async (provider, model, baseUrl, conversationContext, locale) => {
    const { sessionId } = get();
    if (!sessionId) return;

    set({ isLoading: true, error: null });
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
        conversationContext: conversationContext || null,
        locale: locale || null,
      });

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
      set({ isLoading: false, error: String(e) });
    }
  },

  approvePlan: async (plan, provider, model, baseUrl, locale) => {
    const { sessionId } = get();
    if (!sessionId) return;

    set({ isLoading: true, error: null });
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
        locale: locale || null,
      });

      if (result.success) {
        set({ sessionPhase: 'executing', isLoading: false });
      } else {
        set({ isLoading: false, error: result.error || 'Failed to approve plan' });
      }
    } catch (e) {
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

    try {
      await invoke<CommandResponse<boolean>>('cancel_plan_execution', { sessionId });
      set({ sessionPhase: 'cancelled' });
    } catch (e) {
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

  exitPlanMode: async () => {
    const { sessionId, _unlistenFn } = get();

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

    set({ ...DEFAULT_STATE });
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
      } else if (payload.eventType === 'execution_cancelled') {
        updates.sessionPhase = 'cancelled';
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
    set({ ...DEFAULT_STATE });
  },
}));
