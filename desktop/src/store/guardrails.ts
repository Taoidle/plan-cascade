/**
 * Guardrails Store
 *
 * Zustand store for guardrail security configuration. Manages built-in
 * guardrail toggles, custom rules, and trigger log display state.
 */

import { create } from 'zustand';
import type { GuardrailInfo, TriggerLogEntry } from '../lib/guardrailsApi';
import {
  listGuardrails,
  toggleGuardrailEnabled,
  addCustomRule as addCustomRuleApi,
  removeCustomRule as removeCustomRuleApi,
  getTriggerLog,
  clearTriggerLog as clearTriggerLogApi,
} from '../lib/guardrailsApi';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface GuardrailsState {
  /** All guardrails (built-in + custom) */
  guardrails: GuardrailInfo[];

  /** Recent trigger log entries */
  triggerLog: TriggerLogEntry[];

  /** Loading states */
  isLoading: boolean;
  isTogglingGuardrail: boolean;
  isAddingRule: boolean;
  isLoadingLog: boolean;

  /** Error message */
  error: string | null;

  /** Actions */
  fetchGuardrails: () => Promise<void>;
  toggleGuardrail: (name: string, enabled: boolean) => Promise<void>;
  addCustomRule: (name: string, pattern: string, action: string) => Promise<boolean>;
  removeCustomRule: (name: string) => Promise<boolean>;
  fetchTriggerLog: (limit?: number, offset?: number) => Promise<void>;
  clearTriggerLog: () => Promise<void>;
  clearError: () => void;
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_STATE = {
  guardrails: [],
  triggerLog: [],
  isLoading: false,
  isTogglingGuardrail: false,
  isAddingRule: false,
  isLoadingLog: false,
  error: null,
};

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useGuardrailsStore = create<GuardrailsState>()((set) => ({
  ...DEFAULT_STATE,

  fetchGuardrails: async () => {
    set({ isLoading: true, error: null });
    try {
      const result = await listGuardrails();
      if (result.success && result.data) {
        set({ guardrails: result.data, isLoading: false });
      } else {
        set({
          isLoading: false,
          error: result.error ?? 'Failed to fetch guardrails',
        });
      }
    } catch (err) {
      set({
        isLoading: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  toggleGuardrail: async (name: string, enabled: boolean) => {
    set({ isTogglingGuardrail: true, error: null });
    try {
      const result = await toggleGuardrailEnabled(name, enabled);
      if (result.success) {
        // Optimistically update the local state
        set((state) => ({
          guardrails: state.guardrails.map((g) => (g.name === name ? { ...g, enabled } : g)),
          isTogglingGuardrail: false,
        }));
      } else {
        set({
          isTogglingGuardrail: false,
          error: result.error ?? 'Failed to toggle guardrail',
        });
      }
    } catch (err) {
      set({
        isTogglingGuardrail: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  addCustomRule: async (name: string, pattern: string, action: string) => {
    set({ isAddingRule: true, error: null });
    try {
      const result = await addCustomRuleApi(name, pattern, action);
      if (result.success && result.data) {
        set((state) => ({
          guardrails: [...state.guardrails, result.data!],
          isAddingRule: false,
        }));
        return true;
      } else {
        set({
          isAddingRule: false,
          error: result.error ?? 'Failed to add custom rule',
        });
        return false;
      }
    } catch (err) {
      set({
        isAddingRule: false,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  removeCustomRule: async (name: string) => {
    set({ error: null });
    try {
      const result = await removeCustomRuleApi(name);
      if (result.success) {
        set((state) => ({
          guardrails: state.guardrails.filter((g) => g.name !== name),
        }));
        return true;
      } else {
        set({ error: result.error ?? 'Failed to remove custom rule' });
        return false;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      return false;
    }
  },

  fetchTriggerLog: async (limit?: number, offset?: number) => {
    set({ isLoadingLog: true, error: null });
    try {
      const result = await getTriggerLog(limit, offset);
      if (result.success && result.data) {
        set({ triggerLog: result.data, isLoadingLog: false });
      } else {
        set({
          isLoadingLog: false,
          error: result.error ?? 'Failed to fetch trigger log',
        });
      }
    } catch (err) {
      set({
        isLoadingLog: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  clearTriggerLog: async () => {
    set({ error: null });
    try {
      const result = await clearTriggerLogApi();
      if (result.success) {
        set({ triggerLog: [] });
      } else {
        set({ error: result.error ?? 'Failed to clear trigger log' });
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
    }
  },

  clearError: () => set({ error: null }),
}));

export default useGuardrailsStore;
