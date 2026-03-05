import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from '../lib/tauri';
import type {
  WorkflowInteractiveActionFailureRecordRequest,
  WorkflowObservabilitySnapshot,
} from '../types/workflowObservability';

interface WorkflowObservabilityState {
  snapshot: WorkflowObservabilitySnapshot | null;
  isLoading: boolean;
  error: string | null;
  refreshSnapshot: () => Promise<WorkflowObservabilitySnapshot | null>;
  recordInteractiveActionFailure: (request: WorkflowInteractiveActionFailureRecordRequest) => Promise<boolean>;
  reset: () => void;
}

const DEFAULT_STATE = {
  snapshot: null as WorkflowObservabilitySnapshot | null,
  isLoading: false,
  error: null as string | null,
};

function normalizeStringValue(value?: string | null): string | null {
  const normalized = value?.trim() ?? '';
  return normalized.length > 0 ? normalized : null;
}

export const useWorkflowObservabilityStore = create<WorkflowObservabilityState>((set, get) => ({
  ...DEFAULT_STATE,

  refreshSnapshot: async () => {
    set({ isLoading: true, error: null });
    try {
      const response = await invoke<CommandResponse<WorkflowObservabilitySnapshot>>(
        'workflow_get_observability_snapshot',
      );
      if (!response.success || !response.data) {
        const error = response.error || 'Failed to load workflow observability snapshot';
        set({ isLoading: false, error });
        return null;
      }
      set({ snapshot: response.data, isLoading: false, error: null });
      return response.data;
    } catch (error) {
      set({ isLoading: false, error: error instanceof Error ? error.message : String(error) });
      return null;
    }
  },

  recordInteractiveActionFailure: async (request) => {
    const card = normalizeStringValue(request.card) || 'unknown_card';
    const action = normalizeStringValue(request.action) || 'unknown_action';
    const errorCode = normalizeStringValue(request.errorCode) || 'unknown_error';

    try {
      const response = await invoke<CommandResponse<boolean>>('workflow_record_interactive_action_failure', {
        request: {
          ...request,
          card,
          action,
          errorCode,
          message: normalizeStringValue(request.message),
          mode: normalizeStringValue(request.mode ?? null),
          kernelSessionId: normalizeStringValue(request.kernelSessionId),
          modeSessionId: normalizeStringValue(request.modeSessionId),
          phaseBefore: normalizeStringValue(request.phaseBefore),
          phaseAfter: normalizeStringValue(request.phaseAfter),
        },
      });

      if (!response.success || !response.data) {
        set({ error: response.error || 'Failed to record workflow action failure' });
        return false;
      }
      void get().refreshSnapshot();
      return true;
    } catch (error) {
      set({ error: error instanceof Error ? error.message : String(error) });
      return false;
    }
  },

  reset: () => set({ ...DEFAULT_STATE }),
}));
