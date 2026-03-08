/**
 * Plan Mode Store
 *
 * Command client for Plan Mode lifecycle IPC.
 * Session lifecycle truth is owned by workflow kernel snapshots.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  PlanModeSession,
  PlanCardData,
  PlanExecutionReport,
  PlanExecutionStatusResponse,
  AdapterInfo,
  StepOutputData,
  PlanClarifyAnswerCardData,
} from '../types/planModeCard';
import type { CommandResponse } from '../lib/tauri';
import type { ContextSourceConfig } from './contextSources';

let activePlanSessionId: string | null = null;

function normalizeSessionId(value: string | null | undefined): string | null {
  const normalized = value?.trim() ?? '';
  return normalized.length > 0 ? normalized : null;
}

function resolveSessionId(explicit?: string | null): string | null {
  return normalizeSessionId(explicit) ?? activePlanSessionId;
}

async function resolveRequestLlm(provider?: string, model?: string, baseUrl?: string) {
  const settingsStore = (await import('./settings')).useSettingsStore.getState();
  const finalProvider = provider || settingsStore.provider;
  const finalModel = model || settingsStore.model;
  const { resolveProviderBaseUrl } = await import('../lib/providers');
  const finalBaseUrl = baseUrl || resolveProviderBaseUrl(finalProvider, settingsStore);
  return { finalProvider, finalModel, finalBaseUrl, settingsStore };
}

// ============================================================================
// Store Interface
// ============================================================================

export interface PlanModeState {
  isLoading: boolean;
  isCancelling: boolean;
  error: string | null;
  _requestId: number;

  enterPlanMode: (
    description: string,
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    conversationContext?: string,
    locale?: string,
    kernelSessionId?: string | null,
  ) => Promise<PlanModeSession | null>;
  submitClarification: (
    answer: PlanClarifyAnswerCardData,
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    conversationContext?: string,
    locale?: string,
    sessionId?: string | null,
  ) => Promise<PlanModeSession | null>;
  skipClarification: (sessionId?: string | null) => Promise<boolean>;
  generatePlan: (
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    conversationContext?: string,
    locale?: string,
    sessionId?: string | null,
  ) => Promise<PlanCardData | null>;
  approvePlan: (
    plan: PlanCardData,
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    conversationContext?: string,
    locale?: string,
    sessionId?: string | null,
  ) => Promise<boolean>;
  retryPlanStep: (
    stepId: string,
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    conversationContext?: string,
    locale?: string,
    sessionId?: string | null,
  ) => Promise<boolean>;
  refreshStatus: (sessionId?: string | null) => Promise<PlanExecutionStatusResponse | null>;
  cancelExecution: (sessionId?: string | null) => Promise<boolean>;
  cancelOperation: (sessionId?: string | null) => Promise<boolean>;
  fetchReport: (sessionId?: string | null) => Promise<PlanExecutionReport | null>;
  fetchStepOutput: (stepId: string, sessionId?: string | null) => Promise<StepOutputData | null>;
  exitPlanMode: (sessionId?: string | null) => Promise<boolean>;
  fetchAdapters: () => Promise<AdapterInfo[]>;
  reset: () => void;
}

const DEFAULT_STATE = {
  isLoading: false,
  isCancelling: false,
  error: null as string | null,
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
    kernelSessionId,
  ) => {
    const requestId = get()._requestId + 1;
    set({ isLoading: true, isCancelling: false, error: null, _requestId: requestId });
    try {
      let finalBaseUrl = baseUrl;
      if (!finalBaseUrl && provider) {
        const settingsStore = (await import('./settings')).useSettingsStore.getState();
        const { resolveProviderBaseUrl } = await import('../lib/providers');
        finalBaseUrl = resolveProviderBaseUrl(provider, settingsStore);
      }

      const result = await invoke<CommandResponse<PlanModeSession>>('enter_plan_mode', {
        request: {
          description,
          kernelSessionId: normalizeSessionId(kernelSessionId),
          provider: provider || null,
          model: model || null,
          baseUrl: finalBaseUrl || null,
          projectPath: projectPath || null,
          contextSources: contextSources || null,
          conversationContext: conversationContext || null,
          locale: locale || null,
        },
      });
      if (get()._requestId !== requestId) return null;

      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error || 'Failed to enter plan mode' });
        return null;
      }

      activePlanSessionId = normalizeSessionId(result.data.sessionId);
      set({ isLoading: false });
      return result.data;
    } catch (e) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: String(e) });
      return null;
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
    sessionId,
  ) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active plan session' });
      return null;
    }

    const requestId = get()._requestId + 1;
    set({ _requestId: requestId, error: null });

    try {
      const { finalProvider, finalModel, finalBaseUrl } = await resolveRequestLlm(provider, model, baseUrl);
      const result = await invoke<CommandResponse<PlanModeSession>>('submit_plan_clarification', {
        request: {
          sessionId: resolvedSessionId,
          answer,
          provider: finalProvider || null,
          model: finalModel || null,
          baseUrl: finalBaseUrl || null,
          projectPath: projectPath || null,
          contextSources: contextSources || null,
          conversationContext: conversationContext || null,
          locale: locale || null,
        },
      });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to submit plan clarification' });
        return null;
      }
      activePlanSessionId = normalizeSessionId(result.data.sessionId) ?? activePlanSessionId;
      return result.data;
    } catch (e) {
      if (get()._requestId !== requestId) return null;
      set({ error: String(e) });
      return null;
    }
  },

  skipClarification: async (sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active plan session' });
      return false;
    }

    const requestId = get()._requestId + 1;
    set({ _requestId: requestId, error: null });
    try {
      const result = await invoke<CommandResponse<PlanModeSession>>('skip_plan_clarification', {
        sessionId: resolvedSessionId,
      });
      if (get()._requestId !== requestId) return false;
      if (!result.success) {
        set({ error: result.error || 'Failed to skip plan clarification' });
        return false;
      }
      return true;
    } catch (e) {
      if (get()._requestId !== requestId) return false;
      set({ error: String(e) });
      return false;
    }
  },

  generatePlan: async (
    provider,
    model,
    baseUrl,
    projectPath,
    contextSources,
    conversationContext,
    locale,
    sessionId,
  ) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active plan session' });
      return null;
    }

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const { finalProvider, finalModel, finalBaseUrl } = await resolveRequestLlm(provider, model, baseUrl);

      const result = await invoke<CommandResponse<PlanCardData>>('generate_plan', {
        request: {
          sessionId: resolvedSessionId,
          provider: finalProvider || null,
          model: finalModel || null,
          baseUrl: finalBaseUrl || null,
          projectPath: projectPath || null,
          contextSources: contextSources || null,
          conversationContext: conversationContext || null,
          locale: locale || null,
        },
      });
      if (get()._requestId !== requestId) return null;

      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error || 'Failed to generate plan' });
        return null;
      }
      set({ isLoading: false });
      return result.data;
    } catch (e) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: String(e) });
      return null;
    }
  },

  approvePlan: async (
    plan,
    provider,
    model,
    baseUrl,
    projectPath,
    contextSources,
    conversationContext,
    locale,
    sessionId,
  ) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active plan session' });
      return false;
    }

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const { finalProvider, finalModel, finalBaseUrl } = await resolveRequestLlm(provider, model, baseUrl);

      const result = await invoke<CommandResponse<boolean>>('approve_plan', {
        request: {
          sessionId: resolvedSessionId,
          plan,
          provider: finalProvider || null,
          model: finalModel || null,
          baseUrl: finalBaseUrl || null,
          projectPath: projectPath || null,
          contextSources: contextSources || null,
          conversationContext: conversationContext || null,
          locale: locale || null,
        },
      });
      if (get()._requestId !== requestId) return false;

      if (!result.success) {
        set({ isLoading: false, error: result.error || 'Failed to approve plan' });
        return false;
      }
      set({ isLoading: false });
      return true;
    } catch (e) {
      if (get()._requestId !== requestId) return false;
      set({ isLoading: false, error: String(e) });
      return false;
    }
  },

  retryPlanStep: async (
    stepId,
    provider,
    model,
    baseUrl,
    projectPath,
    contextSources,
    conversationContext,
    locale,
    sessionId,
  ) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    const normalizedStepId = stepId.trim();
    if (!resolvedSessionId) {
      set({ error: 'No active plan session' });
      return false;
    }
    if (!normalizedStepId) {
      set({ error: 'Step id is required for retry' });
      return false;
    }

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const { finalProvider, finalModel, finalBaseUrl } = await resolveRequestLlm(provider, model, baseUrl);

      const result = await invoke<CommandResponse<boolean>>('retry_plan_step', {
        request: {
          sessionId: resolvedSessionId,
          stepId: normalizedStepId,
          provider: finalProvider || null,
          model: finalModel || null,
          baseUrl: finalBaseUrl || null,
          projectPath: projectPath || null,
          contextSources: contextSources || null,
          conversationContext: conversationContext || null,
          locale: locale || null,
        },
      });
      if (get()._requestId !== requestId) return false;

      if (!result.success) {
        set({ isLoading: false, error: result.error || `Failed to retry step '${normalizedStepId}'` });
        return false;
      }
      set({ isLoading: false });
      return true;
    } catch (e) {
      if (get()._requestId !== requestId) return false;
      set({ isLoading: false, error: String(e) });
      return false;
    }
  },

  refreshStatus: async (sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) return null;

    try {
      const result = await invoke<CommandResponse<PlanExecutionStatusResponse>>('get_plan_execution_status', {
        sessionId: resolvedSessionId,
      });
      if (result.success && result.data) {
        return result.data;
      }
      return null;
    } catch {
      return null;
    }
  },

  cancelExecution: async (sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active plan session' });
      return false;
    }
    if (get().isCancelling) return false;

    const requestId = get()._requestId + 1;
    set({ _requestId: requestId, isCancelling: true, error: null });

    try {
      const result = await invoke<CommandResponse<boolean>>('cancel_plan_execution', { sessionId: resolvedSessionId });
      if (get()._requestId !== requestId) return false;
      if (!result.success) {
        set({ isCancelling: false, error: result.error || 'Failed to cancel plan execution' });
        return false;
      }
      return true;
    } catch (e) {
      if (get()._requestId !== requestId) return false;
      set({ isCancelling: false, error: String(e) });
      return false;
    }
  },

  cancelOperation: async (sessionId) => {
    const requestId = get()._requestId + 1;
    set({ _requestId: requestId, error: null });

    try {
      const result = await invoke<CommandResponse<boolean>>('cancel_plan_operation', {
        sessionId: resolveSessionId(sessionId),
      });
      if (get()._requestId !== requestId) return false;
      if (!result.success) {
        set({ error: result.error || 'Failed to cancel plan operation' });
        return false;
      }
      return true;
    } catch (e) {
      if (get()._requestId !== requestId) return false;
      set({ error: String(e) });
      return false;
    }
  },

  fetchReport: async (sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) return null;

    try {
      const result = await invoke<CommandResponse<PlanExecutionReport>>('get_plan_execution_report', {
        sessionId: resolvedSessionId,
      });
      if (result.success && result.data) {
        return result.data;
      }
      if (!result.success) {
        set({ error: result.error || 'Failed to fetch plan execution report' });
      }
      return null;
    } catch (e) {
      set({ error: String(e) });
      return null;
    }
  },

  fetchStepOutput: async (stepId, sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    const normalizedStepId = stepId.trim();
    if (!resolvedSessionId || !normalizedStepId) return null;

    try {
      const result = await invoke<CommandResponse<StepOutputData>>('get_step_output', {
        sessionId: resolvedSessionId,
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

  exitPlanMode: async (sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    const nextRequestId = get()._requestId + 1;
    set({ _requestId: nextRequestId, error: null });

    if (resolvedSessionId) {
      try {
        const result = await invoke<CommandResponse<boolean>>('exit_plan_mode', { sessionId: resolvedSessionId });
        if (!result.success) {
          set({ error: result.error || 'Failed to exit plan mode' });
          return false;
        }
      } catch (e) {
        set({ error: String(e) });
        return false;
      }
    }

    activePlanSessionId = null;
    return true;
  },

  fetchAdapters: async () => {
    try {
      const result = await invoke<CommandResponse<AdapterInfo[]>>('list_plan_adapters');
      return result.success && result.data ? result.data : [];
    } catch {
      return [];
    }
  },

  reset: () => {
    activePlanSessionId = null;
    set((state) => ({ ...DEFAULT_STATE, _requestId: state._requestId + 1 }));
  },
}));
