/**
 * Evaluation Store Tests
 *
 * Tests for the Zustand evaluation store including state management,
 * evaluator CRUD, run management, model/case configuration, and IPC mocking.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useEvaluationStore } from './evaluation';
import type {
  EvaluatorInfo,
  EvaluationRunInfo,
  EvaluationReport,
  ModelConfig,
  EvaluationCase,
} from '../types/evaluation';

// Mock invoke is already mocked in test setup
const mockInvoke = vi.mocked(invoke);

// Helper factories
function createMockEvaluatorInfo(overrides: Partial<EvaluatorInfo> = {}): EvaluatorInfo {
  return {
    id: 'eval-1',
    name: 'Test Evaluator',
    has_tool_trajectory: true,
    has_response_similarity: false,
    has_llm_judge: false,
    ...overrides,
  };
}

function createMockRunInfo(overrides: Partial<EvaluationRunInfo> = {}): EvaluationRunInfo {
  return {
    id: 'run-1',
    evaluator_id: 'eval-1',
    model_count: 2,
    case_count: 3,
    status: 'completed',
    created_at: '2026-02-17T00:00:00Z',
    ...overrides,
  };
}

function createMockReport(overrides: Partial<EvaluationReport> = {}): EvaluationReport {
  return {
    run_id: 'run-1',
    model: 'claude-sonnet-4-20250514',
    provider: 'anthropic',
    results: [
      {
        case_id: 'case-1',
        passed: true,
        score: 0.95,
        details: 'Good response',
        tool_calls: ['read_file'],
        response: 'Hello!',
      },
    ],
    overall_score: 0.95,
    duration_ms: 1500,
    total_tokens: 500,
    estimated_cost: 0.003,
    ...overrides,
  };
}

describe('useEvaluationStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store to initial state
    useEvaluationStore.setState({
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
    });
  });

  // ========================================================================
  // Initial State Tests
  // ========================================================================

  describe('Initial State', () => {
    it('should initialize with default values', () => {
      const state = useEvaluationStore.getState();
      expect(state.activeTab).toBe('setup');
      expect(state.evaluators).toHaveLength(0);
      expect(state.currentEvaluator).toBeNull();
      expect(state.runs).toHaveLength(0);
      expect(state.selectedRunId).toBeNull();
      expect(state.reports).toHaveLength(0);
      expect(state.selectedModels).toHaveLength(0);
      expect(state.testCases).toHaveLength(0);
      expect(state.isRunning).toBe(false);
      expect(state.error).toBeNull();
    });
  });

  // ========================================================================
  // Tab Navigation Tests
  // ========================================================================

  describe('Tab Navigation', () => {
    it('should change active tab', () => {
      useEvaluationStore.getState().setActiveTab('runs');
      expect(useEvaluationStore.getState().activeTab).toBe('runs');

      useEvaluationStore.getState().setActiveTab('reports');
      expect(useEvaluationStore.getState().activeTab).toBe('reports');
    });
  });

  // ========================================================================
  // fetchEvaluators Tests
  // ========================================================================

  describe('fetchEvaluators', () => {
    it('should load evaluators successfully', async () => {
      const mockEvals = [
        createMockEvaluatorInfo({ id: 'eval-1', name: 'Eval A' }),
        createMockEvaluatorInfo({ id: 'eval-2', name: 'Eval B' }),
      ];
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: mockEvals,
        error: null,
      });

      await useEvaluationStore.getState().fetchEvaluators();

      const state = useEvaluationStore.getState();
      expect(state.evaluators).toHaveLength(2);
      expect(state.evaluators[0].name).toBe('Eval A');
      expect(state.loading.evaluators).toBe(false);
    });

    it('should handle fetch error', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      await useEvaluationStore.getState().fetchEvaluators();

      const state = useEvaluationStore.getState();
      expect(state.error).toBe('Network error');
      expect(state.loading.evaluators).toBe(false);
    });
  });

  // ========================================================================
  // Evaluator CRUD Tests
  // ========================================================================

  describe('Evaluator CRUD', () => {
    it('should start a new evaluator', () => {
      useEvaluationStore.getState().startNewEvaluator();

      const state = useEvaluationStore.getState();
      expect(state.currentEvaluator).not.toBeNull();
      expect(state.currentEvaluator?.name).toBe('New Evaluator');
      expect(state.isCreatingEvaluator).toBe(true);
    });

    it('should select an existing evaluator', () => {
      const info = createMockEvaluatorInfo({ has_tool_trajectory: true, has_llm_judge: true });
      useEvaluationStore.getState().selectEvaluator(info);

      const state = useEvaluationStore.getState();
      expect(state.currentEvaluator).not.toBeNull();
      expect(state.currentEvaluator?.id).toBe('eval-1');
      expect(state.isCreatingEvaluator).toBe(false);
    });

    it('should update current evaluator name', () => {
      useEvaluationStore.getState().startNewEvaluator();
      useEvaluationStore.getState().updateCurrentEvaluator({ name: 'Updated Name' });

      expect(useEvaluationStore.getState().currentEvaluator?.name).toBe('Updated Name');
    });

    it('should update criteria', () => {
      useEvaluationStore.getState().startNewEvaluator();
      useEvaluationStore.getState().updateCriteria({
        tool_trajectory: { expected_tools: ['read_file'], order_matters: true },
      });

      const criteria = useEvaluationStore.getState().currentEvaluator?.criteria;
      expect(criteria?.tool_trajectory).not.toBeNull();
      expect(criteria?.tool_trajectory?.expected_tools).toEqual(['read_file']);
      expect(criteria?.tool_trajectory?.order_matters).toBe(true);
    });

    it('should save evaluator', async () => {
      useEvaluationStore.getState().startNewEvaluator();
      useEvaluationStore.getState().updateCurrentEvaluator({ name: 'My Eval' });

      const savedEval = {
        id: 'eval-new',
        name: 'My Eval',
        criteria: { tool_trajectory: null, response_similarity: null, llm_judge: null },
      };
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: savedEval,
        error: null,
      });
      // For fetchEvaluators after save
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: [createMockEvaluatorInfo({ id: 'eval-new', name: 'My Eval' })],
        error: null,
      });

      await useEvaluationStore.getState().saveEvaluator();

      const state = useEvaluationStore.getState();
      expect(state.isCreatingEvaluator).toBe(false);
      expect(state.loading.save).toBe(false);
    });

    it('should handle save error', async () => {
      useEvaluationStore.getState().startNewEvaluator();
      mockInvoke.mockRejectedValueOnce(new Error('Save failed'));

      await useEvaluationStore.getState().saveEvaluator();

      expect(useEvaluationStore.getState().error).toBe('Save failed');
      expect(useEvaluationStore.getState().loading.save).toBe(false);
    });

    it('should remove evaluator and clear current if same ID', async () => {
      useEvaluationStore.setState({
        currentEvaluator: {
          id: 'eval-1',
          name: 'Test',
          criteria: { tool_trajectory: null, response_similarity: null, llm_judge: null },
        },
      });

      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: true,
        error: null,
      });
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: [],
        error: null,
      });

      await useEvaluationStore.getState().removeEvaluator('eval-1');

      expect(useEvaluationStore.getState().currentEvaluator).toBeNull();
    });

    it('should clear current evaluator', () => {
      useEvaluationStore.getState().startNewEvaluator();
      useEvaluationStore.getState().clearCurrentEvaluator();

      expect(useEvaluationStore.getState().currentEvaluator).toBeNull();
      expect(useEvaluationStore.getState().isCreatingEvaluator).toBe(false);
    });
  });

  // ========================================================================
  // Runs Tests
  // ========================================================================

  describe('Runs', () => {
    it('should fetch runs', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: [createMockRunInfo()],
        error: null,
      });

      await useEvaluationStore.getState().fetchRuns();

      expect(useEvaluationStore.getState().runs).toHaveLength(1);
      expect(useEvaluationStore.getState().loading.runs).toBe(false);
    });

    it('should select a run and load reports', async () => {
      const mockReports = [createMockReport()];
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: mockReports,
        error: null,
      });

      await useEvaluationStore.getState().selectRun('run-1');

      const state = useEvaluationStore.getState();
      expect(state.selectedRunId).toBe('run-1');
      expect(state.reports).toHaveLength(1);
      expect(state.reports[0].overall_score).toBe(0.95);
    });

    it('should remove a run', async () => {
      useEvaluationStore.setState({ selectedRunId: 'run-1', reports: [createMockReport()] });

      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: true,
        error: null,
      });
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: [],
        error: null,
      });

      await useEvaluationStore.getState().removeRun('run-1');

      expect(useEvaluationStore.getState().selectedRunId).toBeNull();
      expect(useEvaluationStore.getState().reports).toHaveLength(0);
    });
  });

  // ========================================================================
  // Model Selection Tests
  // ========================================================================

  describe('Model Selection', () => {
    it('should add and remove models', () => {
      const model: ModelConfig = { provider: 'anthropic', model: 'claude-sonnet-4-20250514', display_name: null };
      useEvaluationStore.getState().addModel(model);

      expect(useEvaluationStore.getState().selectedModels).toHaveLength(1);

      useEvaluationStore.getState().removeModel(0);
      expect(useEvaluationStore.getState().selectedModels).toHaveLength(0);
    });
  });

  // ========================================================================
  // Test Case Tests
  // ========================================================================

  describe('Test Cases', () => {
    it('should add a test case', () => {
      const tc: EvaluationCase = {
        id: 'tc-1',
        name: 'Case 1',
        input: { prompt: 'Hello' },
        expected_output: 'Hi',
        expected_tools: null,
      };
      useEvaluationStore.getState().addTestCase(tc);

      expect(useEvaluationStore.getState().testCases).toHaveLength(1);
      expect(useEvaluationStore.getState().testCases[0].name).toBe('Case 1');
    });

    it('should update a test case', () => {
      useEvaluationStore.setState({
        testCases: [
          {
            id: 'tc-1',
            name: 'Case 1',
            input: { prompt: 'Hello' },
            expected_output: null,
            expected_tools: null,
          },
        ],
      });

      useEvaluationStore.getState().updateTestCase('tc-1', { name: 'Updated Case' });

      expect(useEvaluationStore.getState().testCases[0].name).toBe('Updated Case');
    });

    it('should remove a test case', () => {
      useEvaluationStore.setState({
        testCases: [
          { id: 'tc-1', name: 'A', input: {}, expected_output: null, expected_tools: null },
          { id: 'tc-2', name: 'B', input: {}, expected_output: null, expected_tools: null },
        ],
      });

      useEvaluationStore.getState().removeTestCase('tc-1');

      expect(useEvaluationStore.getState().testCases).toHaveLength(1);
      expect(useEvaluationStore.getState().testCases[0].id).toBe('tc-2');
    });
  });

  // ========================================================================
  // Progress Events Tests
  // ========================================================================

  describe('Progress Events', () => {
    it('should add and clear progress events', () => {
      useEvaluationStore.getState().addProgressEvent({
        type: 'model_started',
        model: 'claude-sonnet-4-20250514',
        provider: 'anthropic',
      });
      useEvaluationStore.getState().addProgressEvent({
        type: 'case_completed',
        case_id: 'case-1',
        score: 0.9,
      });

      expect(useEvaluationStore.getState().progressEvents).toHaveLength(2);

      useEvaluationStore.getState().clearProgress();
      expect(useEvaluationStore.getState().progressEvents).toHaveLength(0);
      expect(useEvaluationStore.getState().isRunning).toBe(false);
    });
  });

  // ========================================================================
  // Error Handling Tests
  // ========================================================================

  describe('Error Handling', () => {
    it('should set and clear error', () => {
      useEvaluationStore.getState().setError('Something broke');
      expect(useEvaluationStore.getState().error).toBe('Something broke');

      useEvaluationStore.getState().setError(null);
      expect(useEvaluationStore.getState().error).toBeNull();
    });
  });
});
