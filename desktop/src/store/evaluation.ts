/**
 * Evaluation Store
 *
 * Zustand store for managing evaluation framework state.
 * Handles evaluator CRUD, evaluation runs, reports, and real-time progress.
 */

import { create } from 'zustand';
import type {
  Evaluator,
  EvaluatorInfo,
  EvaluationRun,
  EvaluationRunInfo,
  EvaluationReport,
  EvaluationCase,
  ModelConfig,
  EvaluationCriteria,
  EvaluationProgressEvent,
} from '../types/evaluation';
import { createDefaultCriteria, createDefaultEvaluator } from '../types/evaluation';
import {
  listEvaluators,
  createEvaluator,
  deleteEvaluator,
  createEvaluationRun,
  listEvaluationRuns,
  getEvaluationReports,
  deleteEvaluationRun,
} from '../lib/evaluationApi';

/** Active tab in the evaluation dashboard */
export type EvaluationTab = 'setup' | 'runs' | 'reports';

interface EvaluationState {
  /** Active dashboard tab */
  activeTab: EvaluationTab;

  /** List of evaluators (summary) */
  evaluators: EvaluatorInfo[];

  /** Currently editing evaluator */
  currentEvaluator: Evaluator | null;

  /** Whether creating a new evaluator */
  isCreatingEvaluator: boolean;

  /** List of evaluation runs (summary) */
  runs: EvaluationRunInfo[];

  /** Currently selected run ID */
  selectedRunId: string | null;

  /** Reports for the selected run */
  reports: EvaluationReport[];

  /** Models configured for the next run */
  selectedModels: ModelConfig[];

  /** Test cases configured for the next run */
  testCases: EvaluationCase[];

  /** Whether an evaluation is currently running */
  isRunning: boolean;

  /** Progress events from the current run */
  progressEvents: EvaluationProgressEvent[];

  /** Loading states */
  loading: {
    evaluators: boolean;
    runs: boolean;
    reports: boolean;
    save: boolean;
  };

  /** Error message */
  error: string | null;

  // Actions
  setActiveTab: (tab: EvaluationTab) => void;

  /** Fetch all evaluators */
  fetchEvaluators: () => Promise<void>;

  /** Start creating a new evaluator */
  startNewEvaluator: () => void;

  /** Select an evaluator for editing */
  selectEvaluator: (evaluator: EvaluatorInfo) => void;

  /** Update the current evaluator in memory */
  updateCurrentEvaluator: (updates: Partial<Evaluator>) => void;

  /** Update criteria of current evaluator */
  updateCriteria: (updates: Partial<EvaluationCriteria>) => void;

  /** Save the current evaluator */
  saveEvaluator: () => Promise<void>;

  /** Delete an evaluator */
  removeEvaluator: (id: string) => Promise<void>;

  /** Fetch all evaluation runs */
  fetchRuns: () => Promise<void>;

  /** Select a run and load its reports */
  selectRun: (runId: string) => Promise<void>;

  /** Add a model to the selection */
  addModel: (model: ModelConfig) => void;

  /** Remove a model from selection */
  removeModel: (index: number) => void;

  /** Add a test case */
  addTestCase: (testCase: EvaluationCase) => void;

  /** Update a test case */
  updateTestCase: (id: string, updates: Partial<EvaluationCase>) => void;

  /** Remove a test case */
  removeTestCase: (id: string) => void;

  /** Start an evaluation run */
  startRun: (evaluatorId: string) => Promise<void>;

  /** Delete an evaluation run */
  removeRun: (runId: string) => Promise<void>;

  /** Add a progress event */
  addProgressEvent: (event: EvaluationProgressEvent) => void;

  /** Clear progress events */
  clearProgress: () => void;

  /** Set error */
  setError: (error: string | null) => void;

  /** Clear current evaluator */
  clearCurrentEvaluator: () => void;
}

export const useEvaluationStore = create<EvaluationState>((set, get) => ({
  activeTab: 'setup',
  evaluators: [],
  currentEvaluator: null,
  isCreatingEvaluator: false,
  runs: [],
  selectedRunId: null,
  reports: [],
  selectedModels: [],
  testCases: [],
  isRunning: false,
  progressEvents: [],
  loading: { evaluators: false, runs: false, reports: false, save: false },
  error: null,

  setActiveTab: (tab) => set({ activeTab: tab }),

  fetchEvaluators: async () => {
    set((s) => ({ loading: { ...s.loading, evaluators: true }, error: null }));
    try {
      const evaluators = await listEvaluators();
      set({ evaluators, loading: { ...get().loading, evaluators: false } });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set((s) => ({ error: msg, loading: { ...s.loading, evaluators: false } }));
    }
  },

  startNewEvaluator: () => {
    set({
      currentEvaluator: createDefaultEvaluator(),
      isCreatingEvaluator: true,
      error: null,
    });
  },

  selectEvaluator: (info) => {
    // For now, create an evaluator with the info we have.
    // Full evaluator details would require a get_evaluator command.
    set({
      currentEvaluator: {
        id: info.id,
        name: info.name,
        criteria: {
          tool_trajectory: info.has_tool_trajectory ? { expected_tools: [], order_matters: false } : null,
          response_similarity: info.has_response_similarity ? { reference_response: '', threshold: 0.8 } : null,
          llm_judge: info.has_llm_judge ? { judge_model: '', judge_provider: '', rubric: '' } : null,
        },
      },
      isCreatingEvaluator: false,
      error: null,
    });
  },

  updateCurrentEvaluator: (updates) => {
    const current = get().currentEvaluator;
    if (!current) return;
    set({ currentEvaluator: { ...current, ...updates } });
  },

  updateCriteria: (updates) => {
    const current = get().currentEvaluator;
    if (!current) return;
    set({
      currentEvaluator: {
        ...current,
        criteria: { ...current.criteria, ...updates },
      },
    });
  },

  saveEvaluator: async () => {
    const { currentEvaluator, isCreatingEvaluator } = get();
    if (!currentEvaluator) return;

    set((s) => ({ loading: { ...s.loading, save: true }, error: null }));
    try {
      const saved = await createEvaluator(currentEvaluator);
      set({
        currentEvaluator: saved,
        isCreatingEvaluator: false,
        loading: { ...get().loading, save: false },
      });
      await get().fetchEvaluators();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set((s) => ({ error: msg, loading: { ...s.loading, save: false } }));
    }
  },

  removeEvaluator: async (id) => {
    set({ error: null });
    try {
      await deleteEvaluator(id);
      if (get().currentEvaluator?.id === id) {
        set({ currentEvaluator: null, isCreatingEvaluator: false });
      }
      await get().fetchEvaluators();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ error: msg });
    }
  },

  fetchRuns: async () => {
    set((s) => ({ loading: { ...s.loading, runs: true }, error: null }));
    try {
      const runs = await listEvaluationRuns();
      set({ runs, loading: { ...get().loading, runs: false } });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set((s) => ({ error: msg, loading: { ...s.loading, runs: false } }));
    }
  },

  selectRun: async (runId) => {
    set((s) => ({
      selectedRunId: runId,
      loading: { ...s.loading, reports: true },
      error: null,
    }));
    try {
      const reports = await getEvaluationReports(runId);
      set({ reports, loading: { ...get().loading, reports: false } });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set((s) => ({ error: msg, loading: { ...s.loading, reports: false } }));
    }
  },

  addModel: (model) => {
    set((s) => ({ selectedModels: [...s.selectedModels, model] }));
  },

  removeModel: (index) => {
    set((s) => ({
      selectedModels: s.selectedModels.filter((_, i) => i !== index),
    }));
  },

  addTestCase: (testCase) => {
    set((s) => ({ testCases: [...s.testCases, testCase] }));
  },

  updateTestCase: (id, updates) => {
    set((s) => ({
      testCases: s.testCases.map((tc) =>
        tc.id === id ? { ...tc, ...updates } : tc
      ),
    }));
  },

  removeTestCase: (id) => {
    set((s) => ({
      testCases: s.testCases.filter((tc) => tc.id !== id),
    }));
  },

  startRun: async (evaluatorId) => {
    const { selectedModels, testCases } = get();
    if (selectedModels.length === 0 || testCases.length === 0) return;

    set({ isRunning: true, progressEvents: [], error: null });
    try {
      const run: EvaluationRun = {
        id: '',
        evaluator_id: evaluatorId,
        models: selectedModels,
        cases: testCases,
        status: 'pending',
        created_at: new Date().toISOString(),
      };
      await createEvaluationRun(run);
      await get().fetchRuns();
      set({ isRunning: false });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ error: msg, isRunning: false });
    }
  },

  removeRun: async (runId) => {
    set({ error: null });
    try {
      await deleteEvaluationRun(runId);
      if (get().selectedRunId === runId) {
        set({ selectedRunId: null, reports: [] });
      }
      await get().fetchRuns();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ error: msg });
    }
  },

  addProgressEvent: (event) => {
    set((s) => ({
      progressEvents: [...s.progressEvents, event],
    }));
  },

  clearProgress: () => {
    set({ progressEvents: [], isRunning: false });
  },

  setError: (error) => set({ error }),

  clearCurrentEvaluator: () => {
    set({ currentEvaluator: null, isCreatingEvaluator: false });
  },
}));
