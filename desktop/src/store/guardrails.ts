import { create } from 'zustand';
import type {
  CustomGuardrailInput,
  GuardrailEventEntry,
  GuardrailInfo,
  GuardrailMode,
  GuardrailRuntimeStatus,
} from '../lib/guardrailsApi';
import {
  clearGuardrailEvents,
  createCustomGuardrail,
  deleteGuardrail,
  listGuardrailEvents,
  listGuardrails,
  setGuardrailMode as setGuardrailModeApi,
  toggleGuardrailEnabled,
  updateGuardrail as updateGuardrailApi,
} from '../lib/guardrailsApi';

export interface GuardrailsState {
  guardrails: GuardrailInfo[];
  runtime: GuardrailRuntimeStatus | null;
  events: GuardrailEventEntry[];
  triggerLog: GuardrailEventEntry[];
  isLoading: boolean;
  isMutating: boolean;
  isLoadingEvents: boolean;
  isLoadingLog: boolean;
  error: string | null;
  fetchGuardrails: () => Promise<void>;
  setMode: (mode: GuardrailMode) => Promise<boolean>;
  toggleGuardrail: (id: string, enabled: boolean) => Promise<void>;
  createRule: (rule: CustomGuardrailInput) => Promise<boolean>;
  updateRule: (rule: CustomGuardrailInput) => Promise<boolean>;
  deleteRule: (id: string) => Promise<boolean>;
  fetchEvents: (limit?: number, offset?: number) => Promise<void>;
  clearEvents: () => Promise<void>;
  addCustomRule: (name: string, pattern: string, action: string) => Promise<boolean>;
  removeCustomRule: (id: string) => Promise<boolean>;
  fetchTriggerLog: (limit?: number, offset?: number) => Promise<void>;
  clearTriggerLog: () => Promise<void>;
  clearError: () => void;
}

export const useGuardrailsStore = create<GuardrailsState>()((set, get) => ({
  guardrails: [],
  runtime: null,
  events: [],
  triggerLog: [],
  isLoading: false,
  isMutating: false,
  isLoadingEvents: false,
  isLoadingLog: false,
  error: null,

  fetchGuardrails: async () => {
    set({ isLoading: true, error: null });
    const result = await listGuardrails();
    if (result.success && result.data) {
      set({
        guardrails: result.data.guardrails,
        runtime: result.data.runtime,
        isLoading: false,
      });
      return;
    }
    set({
      isLoading: false,
      error: result.error ?? 'Failed to fetch guardrails',
    });
  },

  setMode: async (mode) => {
    set({ isMutating: true, error: null });
    const result = await setGuardrailModeApi(mode);
    if (result.success && result.data) {
      set({ runtime: result.data, isMutating: false });
      return true;
    }
    set({
      isMutating: false,
      error: result.error ?? 'Failed to update guardrail mode',
    });
    return false;
  },

  toggleGuardrail: async (id, enabled) => {
    set({ isMutating: true, error: null });
    const result = await toggleGuardrailEnabled(id, enabled);
    if (result.success && result.data) {
      set((state) => ({
        guardrails: state.guardrails.map((guardrail) => (guardrail.id === id ? result.data! : guardrail)),
        isMutating: false,
      }));
      return;
    }
    set({
      isMutating: false,
      error: result.error ?? 'Failed to toggle guardrail',
    });
  },

  createRule: async (rule) => {
    set({ isMutating: true, error: null });
    const result = await createCustomGuardrail(rule);
    if (result.success && result.data) {
      set((state) => ({
        guardrails: [...state.guardrails, result.data!],
        isMutating: false,
      }));
      return true;
    }
    set({
      isMutating: false,
      error: result.error ?? 'Failed to create rule',
    });
    return false;
  },

  updateRule: async (rule) => {
    set({ isMutating: true, error: null });
    const result = await updateGuardrailApi(rule);
    if (result.success && result.data) {
      set((state) => ({
        guardrails: state.guardrails.map((guardrail) => (guardrail.id === result.data!.id ? result.data! : guardrail)),
        isMutating: false,
      }));
      return true;
    }
    set({
      isMutating: false,
      error: result.error ?? 'Failed to update rule',
    });
    return false;
  },

  deleteRule: async (id) => {
    set({ isMutating: true, error: null });
    const result = await deleteGuardrail(id);
    if (result.success) {
      set((state) => ({
        guardrails: state.guardrails.filter((guardrail) => guardrail.id !== id),
        isMutating: false,
      }));
      return true;
    }
    set({
      isMutating: false,
      error: result.error ?? 'Failed to delete rule',
    });
    return false;
  },

  fetchEvents: async (limit, offset) => {
    set({ isLoadingEvents: true, isLoadingLog: true, error: null });
    const result = await listGuardrailEvents(limit, offset);
    if (result.success && result.data) {
      set({ events: result.data, triggerLog: result.data, isLoadingEvents: false, isLoadingLog: false });
      return;
    }
    set({
      isLoadingEvents: false,
      isLoadingLog: false,
      error: result.error ?? 'Failed to fetch events',
    });
  },

  clearEvents: async () => {
    set({ error: null });
    const result = await clearGuardrailEvents();
    if (result.success) {
      set({ events: [], triggerLog: [] });
      return;
    }
    set({ error: result.error ?? 'Failed to clear events' });
  },

  addCustomRule: async (name, pattern, action) =>
    get().createRule({
      name,
      pattern,
      action,
      enabled: true,
      scope: ['input', 'assistant_output', 'tool_result'],
      description: '',
    }),

  removeCustomRule: async (id) => get().deleteRule(id),

  fetchTriggerLog: async (limit, offset) => {
    set({ isLoadingEvents: true, isLoadingLog: true });
    await get().fetchEvents(limit, offset);
  },

  clearTriggerLog: async () => {
    await get().clearEvents();
  },

  clearError: () => set({ error: null }),
}));

export default useGuardrailsStore;
