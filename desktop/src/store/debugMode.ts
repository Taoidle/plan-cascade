import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from '../lib/tauri';
import type {
  DebugCapabilitySnapshot,
  DebugExecutionReport,
  DebugLifecyclePhase,
  DebugModeSession,
  DebugState,
  FixProposal,
} from '../types/debugMode';
import type { ContextSourceConfig } from './contextSources';

let activeDebugSessionId: string | null = null;

function normalizeSessionId(value: string | null | undefined): string | null {
  const normalized = value?.trim() ?? '';
  return normalized.length > 0 ? normalized : null;
}

function resolveSessionId(explicit?: string | null): string | null {
  return normalizeSessionId(explicit) ?? activeDebugSessionId;
}

async function resolveRequestLlm(overrideProvider?: string, overrideModel?: string, overrideBaseUrl?: string) {
  const settingsStore = (await import('./settings')).useSettingsStore.getState();
  const finalProvider = overrideProvider || settingsStore.provider;
  const finalModel = overrideModel || settingsStore.model;
  const { resolveProviderBaseUrl } = await import('../lib/providers');
  const finalBaseUrl = overrideBaseUrl || resolveProviderBaseUrl(finalProvider, settingsStore);
  return {
    finalProvider: finalProvider || null,
    finalModel: finalModel || null,
    finalBaseUrl: finalBaseUrl || null,
  };
}

export interface DebugModeStateStore {
  isLoading: boolean;
  isCancelling: boolean;
  error: string | null;
  _requestId: number;
  capabilitySnapshot: DebugCapabilitySnapshot | null;

  enterDebugMode: (
    description: string,
    environment?: 'dev' | 'staging' | 'prod',
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    locale?: string,
    kernelSessionId?: string | null,
  ) => Promise<DebugModeSession | null>;
  submitClarification: (
    answer: string,
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    locale?: string,
    sessionId?: string | null,
  ) => Promise<DebugModeSession | null>;
  approvePatch: (
    provider?: string,
    model?: string,
    baseUrl?: string,
    projectPath?: string,
    contextSources?: ContextSourceConfig,
    locale?: string,
    sessionId?: string | null,
  ) => Promise<DebugModeSession | null>;
  rejectPatch: (reason: string, sessionId?: string | null) => Promise<DebugModeSession | null>;
  retryPhase: (phase: DebugLifecyclePhase, sessionId?: string | null) => Promise<DebugModeSession | null>;
  attachEvidence: (
    title: string,
    summary: string,
    source: string,
    sessionId?: string | null,
    metadata?: Record<string, unknown> | null,
  ) => Promise<DebugModeSession | null>;
  fetchReport: (sessionId?: string | null) => Promise<DebugExecutionReport | null>;
  getCapabilitySnapshot: (sessionId?: string | null) => Promise<DebugCapabilitySnapshot | null>;
  getSessionSnapshot: (sessionId?: string | null) => Promise<DebugModeSession | null>;
  cancelOperation: (sessionId?: string | null) => Promise<boolean>;
  exitDebugMode: (sessionId?: string | null) => Promise<boolean>;
  seedFixProposal: (proposal: FixProposal, sessionId?: string | null) => Promise<DebugModeSession | null>;
  reset: () => void;
}

const DEFAULT_STATE = {
  isLoading: false,
  isCancelling: false,
  error: null as string | null,
  _requestId: 0,
  capabilitySnapshot: null as DebugCapabilitySnapshot | null,
};

export const useDebugModeStore = create<DebugModeStateStore>((set, get) => ({
  ...DEFAULT_STATE,

  enterDebugMode: async (
    description,
    environment,
    provider,
    model,
    baseUrl,
    projectPath,
    contextSources,
    locale,
    kernelSessionId,
  ) => {
    const requestId = get()._requestId + 1;
    set({ isLoading: true, isCancelling: false, error: null, _requestId: requestId });
    try {
      const requestLlm = await resolveRequestLlm(provider, model, baseUrl);
      const result = await invoke<CommandResponse<DebugModeSession>>('enter_debug_mode', {
        request: {
          description,
          environment: environment || null,
          kernelSessionId: normalizeSessionId(kernelSessionId),
          provider: requestLlm.finalProvider,
          model: requestLlm.finalModel,
          baseUrl: requestLlm.finalBaseUrl,
          projectPath: projectPath || null,
          contextSources: contextSources || null,
          locale: locale || null,
        },
      });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error || 'Failed to enter debug mode' });
        return null;
      }
      activeDebugSessionId = normalizeSessionId(result.data.sessionId);
      set({ isLoading: false, error: null, capabilitySnapshot: null });
      return result.data;
    } catch (error) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  submitClarification: async (answer, provider, model, baseUrl, projectPath, contextSources, locale, sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active debug session' });
      return null;
    }
    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const requestLlm = await resolveRequestLlm(provider, model, baseUrl);
      const result = await invoke<CommandResponse<DebugModeSession>>('submit_debug_clarification', {
        request: {
          sessionId: resolvedSessionId,
          answer,
          provider: requestLlm.finalProvider,
          model: requestLlm.finalModel,
          baseUrl: requestLlm.finalBaseUrl,
          projectPath: projectPath || null,
          contextSources: contextSources || null,
          locale: locale || null,
        },
      });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error || 'Failed to submit clarification' });
        return null;
      }
      set({ isLoading: false, error: null });
      return result.data;
    } catch (error) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  approvePatch: async (provider, model, baseUrl, projectPath, contextSources, locale, sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active debug session' });
      return null;
    }
    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const requestLlm = await resolveRequestLlm(provider, model, baseUrl);
      const result = await invoke<CommandResponse<DebugModeSession>>('approve_debug_patch', {
        request: {
          sessionId: resolvedSessionId,
          provider: requestLlm.finalProvider,
          model: requestLlm.finalModel,
          baseUrl: requestLlm.finalBaseUrl,
          projectPath: projectPath || null,
          contextSources: contextSources || null,
          locale: locale || null,
        },
      });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error || 'Failed to approve patch' });
        return null;
      }
      set({ isLoading: false, error: null });
      return result.data;
    } catch (error) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  rejectPatch: async (reason, sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active debug session' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<DebugModeSession>>('reject_debug_patch', {
        request: { sessionId: resolvedSessionId, reason },
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to reject patch' });
        return null;
      }
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  retryPhase: async (phase, sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active debug session' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<DebugModeSession>>('retry_debug_phase', {
        request: { sessionId: resolvedSessionId, phase },
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to retry debug phase' });
        return null;
      }
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  attachEvidence: async (title, summary, source, sessionId, metadata) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active debug session' });
      return null;
    }
    try {
      const result = await invoke<CommandResponse<DebugModeSession>>('attach_debug_evidence', {
        request: { sessionId: resolvedSessionId, title, summary, source, metadata: metadata || null },
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to attach evidence' });
        return null;
      }
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  fetchReport: async (sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) return null;
    try {
      const result = await invoke<CommandResponse<DebugExecutionReport>>('fetch_debug_report', {
        sessionId: resolvedSessionId,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to fetch debug report' });
        return null;
      }
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  getCapabilitySnapshot: async (sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) return null;
    try {
      const result = await invoke<CommandResponse<DebugCapabilitySnapshot>>('get_debug_capability_snapshot', {
        sessionId: resolvedSessionId,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to fetch debug capability snapshot' });
        return null;
      }
      set({ capabilitySnapshot: result.data });
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  getSessionSnapshot: async (sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) return null;
    try {
      const result = await invoke<CommandResponse<DebugModeSession>>('get_debug_session_snapshot', {
        sessionId: resolvedSessionId,
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to load debug session snapshot' });
        return null;
      }
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  cancelOperation: async (sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) return false;
    set({ isCancelling: true, error: null });
    try {
      const result = await invoke<CommandResponse<boolean>>('cancel_debug_operation', {
        sessionId: resolvedSessionId,
      });
      set({ isCancelling: false });
      if (!result.success || result.data !== true) {
        set({ error: result.error || 'Failed to cancel debug operation' });
        return false;
      }
      return true;
    } catch (error) {
      set({ isCancelling: false, error: error instanceof Error ? error.message : String(error) });
      return false;
    }
  },

  exitDebugMode: async (sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) return true;
    try {
      const result = await invoke<CommandResponse<boolean>>('exit_debug_mode', {
        sessionId: resolvedSessionId,
      });
      if (!result.success || result.data !== true) {
        set({ error: result.error || 'Failed to exit debug mode' });
        return false;
      }
      if (activeDebugSessionId === resolvedSessionId) {
        activeDebugSessionId = null;
      }
      set({ capabilitySnapshot: null });
      return true;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return false;
    }
  },

  seedFixProposal: async (proposal, sessionId) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) return null;
    try {
      const result = await invoke<CommandResponse<DebugModeSession>>('seed_debug_fix_proposal', {
        request: { sessionId: resolvedSessionId, proposal },
      });
      if (!result.success || !result.data) {
        set({ error: result.error || 'Failed to seed debug fix proposal' });
        return null;
      }
      return result.data;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  reset: () => {
    activeDebugSessionId = null;
    set({ ...DEFAULT_STATE });
  },
}));

export function selectDebugStateSnapshot(session: DebugModeSession | null | undefined): DebugState | null {
  return session?.state ?? null;
}
