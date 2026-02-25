/**
 * Agent Composer Store
 *
 * Zustand store for managing agent pipeline state in the Agent Composer UI.
 * Handles CRUD operations, selection, and execution state.
 */

import { create } from 'zustand';
import type { AgentPipeline, AgentPipelineInfo, AgentStep, AgentEvent } from '../types/agentComposer';
import { createEmptyPipeline } from '../types/agentComposer';
import {
  listAgentPipelines,
  getAgentPipeline,
  createAgentPipeline,
  updateAgentPipeline,
  deleteAgentPipeline,
} from '../lib/agentComposerApi';

interface AgentComposerState {
  /** List of all saved pipelines (summary info) */
  pipelines: AgentPipelineInfo[];

  /** Currently selected/editing pipeline (full definition) */
  currentPipeline: AgentPipeline | null;

  /** Whether we're in "create new" mode vs editing existing */
  isCreating: boolean;

  /** Events from the currently running pipeline */
  executionEvents: AgentEvent[];

  /** Whether a pipeline is currently executing */
  isExecuting: boolean;

  /** Loading states */
  loading: {
    list: boolean;
    detail: boolean;
    save: boolean;
  };

  /** Error message */
  error: string | null;

  // Actions

  /** Fetch the list of all pipelines */
  fetchPipelines: () => Promise<void>;

  /** Select a pipeline to view/edit */
  selectPipeline: (id: string) => Promise<void>;

  /** Start creating a new pipeline */
  startNewPipeline: () => void;

  /** Update the current pipeline in memory (not persisted) */
  updateCurrentPipeline: (updates: Partial<AgentPipeline>) => void;

  /** Add a step to the current pipeline */
  addStep: (step: AgentStep) => void;

  /** Remove a step from the current pipeline by index */
  removeStep: (index: number) => void;

  /** Update a step in the current pipeline by index */
  updateStep: (index: number, step: AgentStep) => void;

  /** Save the current pipeline (create or update) */
  savePipeline: () => Promise<void>;

  /** Delete a pipeline by ID */
  deletePipeline: (id: string) => Promise<void>;

  /** Clear the current selection */
  clearSelection: () => void;

  /** Add an execution event */
  addExecutionEvent: (event: AgentEvent) => void;

  /** Clear execution events */
  clearExecutionEvents: () => void;

  /** Set error message */
  setError: (error: string | null) => void;
}

export const useAgentComposerStore = create<AgentComposerState>((set, get) => ({
  pipelines: [],
  currentPipeline: null,
  isCreating: false,
  executionEvents: [],
  isExecuting: false,
  loading: { list: false, detail: false, save: false },
  error: null,

  fetchPipelines: async () => {
    set((s) => ({ loading: { ...s.loading, list: true }, error: null }));
    try {
      const pipelines = await listAgentPipelines();
      set({ pipelines, loading: { ...get().loading, list: false } });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set((s) => ({ error: msg, loading: { ...s.loading, list: false } }));
    }
  },

  selectPipeline: async (id: string) => {
    set((s) => ({
      loading: { ...s.loading, detail: true },
      error: null,
      isCreating: false,
    }));
    try {
      const pipeline = await getAgentPipeline(id);
      set({
        currentPipeline: pipeline,
        loading: { ...get().loading, detail: false },
      });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set((s) => ({ error: msg, loading: { ...s.loading, detail: false } }));
    }
  },

  startNewPipeline: () => {
    set({
      currentPipeline: createEmptyPipeline(),
      isCreating: true,
      error: null,
    });
  },

  updateCurrentPipeline: (updates) => {
    const current = get().currentPipeline;
    if (!current) return;
    set({ currentPipeline: { ...current, ...updates } });
  },

  addStep: (step) => {
    const current = get().currentPipeline;
    if (!current) return;
    set({
      currentPipeline: {
        ...current,
        steps: [...current.steps, step],
      },
    });
  },

  removeStep: (index) => {
    const current = get().currentPipeline;
    if (!current) return;
    const steps = current.steps.filter((_, i) => i !== index);
    set({ currentPipeline: { ...current, steps } });
  },

  updateStep: (index, step) => {
    const current = get().currentPipeline;
    if (!current) return;
    const steps = [...current.steps];
    steps[index] = step;
    set({ currentPipeline: { ...current, steps } });
  },

  savePipeline: async () => {
    const { currentPipeline, isCreating } = get();
    if (!currentPipeline) return;

    set((s) => ({ loading: { ...s.loading, save: true }, error: null }));
    try {
      if (isCreating) {
        const saved = await createAgentPipeline(currentPipeline);
        set({
          currentPipeline: saved,
          isCreating: false,
          loading: { ...get().loading, save: false },
        });
      } else {
        const saved = await updateAgentPipeline(currentPipeline.pipeline_id, currentPipeline);
        set({
          currentPipeline: saved,
          loading: { ...get().loading, save: false },
        });
      }
      // Refresh the list
      await get().fetchPipelines();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set((s) => ({ error: msg, loading: { ...s.loading, save: false } }));
    }
  },

  deletePipeline: async (id) => {
    set({ error: null });
    try {
      await deleteAgentPipeline(id);
      // Clear selection if we deleted the current pipeline
      if (get().currentPipeline?.pipeline_id === id) {
        set({ currentPipeline: null, isCreating: false });
      }
      await get().fetchPipelines();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ error: msg });
    }
  },

  clearSelection: () => {
    set({ currentPipeline: null, isCreating: false, error: null });
  },

  addExecutionEvent: (event) => {
    set((s) => ({
      executionEvents: [...s.executionEvents, event],
    }));
  },

  clearExecutionEvents: () => {
    set({ executionEvents: [], isExecuting: false });
  },

  setError: (error) => {
    set({ error });
  },
}));
