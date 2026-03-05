/**
 * Task Mode Store Tests
 *
 * Story 007: Frontend Task Mode Store and UI Components
 * Tests the Zustand store logic for task mode lifecycle management.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock Tauri APIs before importing the store
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// Import after mocks
import { normalizeTaskModeSessionStatus, useTaskModeStore } from './taskMode';
import type { StrategyAnalysis, TaskModeSession, TaskPrd, TaskExecutionStatus, ExecutionReport } from './taskMode';
import { useContextSourcesStore } from './contextSources';
import { useProjectsStore } from './projects';
import { useSettingsStore } from './settings';

// Helpers
function resetStore() {
  useTaskModeStore.getState().reset();
  useSettingsStore.setState({ defaultAgent: 'claude-code' });
  useContextSourcesStore.setState({
    knowledgeEnabled: false,
    selectedCollections: [],
    selectedDocuments: [],
  });
  useProjectsStore.setState({
    selectedProject: {
      id: 'default',
      name: 'Default',
      path: '/tmp/default',
      last_activity: new Date().toISOString(),
      session_count: 0,
      message_count: 0,
    },
  });
  vi.clearAllMocks();
}

function mockAnalysis(mode: 'chat' | 'task' = 'task'): StrategyAnalysis {
  return {
    functionalAreas: ['auth', 'api'],
    estimatedStories: 5,
    hasDependencies: true,
    riskLevel: 'medium',
    parallelizationBenefit: 'significant',
    recommendedMode: mode,
    confidence: 0.85,
    reasoning: 'Complex multi-step task',
    strategyDecision: {
      strategy: 'hybrid_auto',
      confidence: 0.85,
      reasoning: 'Multi-story decomposition',
    },
  };
}

function mockSession(): TaskModeSession {
  return {
    sessionId: 'session-123',
    description: 'Build feature X',
    status: 'initialized',
    strategyAnalysis: mockAnalysis(),
    prd: null,
    progress: null,
    createdAt: '2026-02-18T00:00:00Z',
  };
}

function mockPrd(): TaskPrd {
  return {
    title: 'PRD: Build feature X',
    description: 'Build feature X',
    stories: [
      {
        id: 'story-001',
        title: 'Story 1',
        description: 'First story',
        priority: 'high',
        dependencies: [],
        acceptanceCriteria: ['Criterion 1'],
      },
      {
        id: 'story-002',
        title: 'Story 2',
        description: 'Second story',
        priority: 'medium',
        dependencies: ['story-001'],
        acceptanceCriteria: ['Criterion 2'],
      },
    ],
    batches: [
      { index: 0, storyIds: ['story-001'] },
      { index: 1, storyIds: ['story-002'] },
    ],
  };
}

// ===========================================================================
// Tests
// ===========================================================================

describe('TaskModeStore', () => {
  beforeEach(() => {
    resetStore();
  });

  // =========================================================================
  // Initial State
  // =========================================================================
  describe('initial state', () => {
    it('should start with default state and no session', () => {
      const state = useTaskModeStore.getState();
      expect(state.sessionId).toBeNull();
      expect(state.status).toBe('initialized');
      expect(state.strategyAnalysis).toBeNull();
      expect(state.suggestionDismissed).toBe(false);
      expect(state.prd).toBeNull();
      expect(state.currentBatch).toBe(0);
      expect(state.totalBatches).toBe(0);
      expect(state.storyStatuses).toEqual({});
      expect(state.qualityGateResults).toEqual({});
      expect(state.report).toBeNull();
      expect(state.isLoading).toBe(false);
      expect(state.error).toBeNull();
    });
  });

  // =========================================================================
  // analyzeForMode
  // =========================================================================
  describe('analyzeForMode', () => {
    it('should update strategyAnalysis on success', async () => {
      const analysis = mockAnalysis('task');
      mockInvoke.mockResolvedValueOnce({ success: true, data: analysis, error: null });

      await useTaskModeStore.getState().analyzeForMode('Build a complex app');

      const state = useTaskModeStore.getState();
      expect(state.strategyAnalysis).toEqual(analysis);
      expect(state.isLoading).toBe(false);
      expect(state.error).toBeNull();
      expect(state.suggestionDismissed).toBe(false);
      expect(mockInvoke).toHaveBeenCalledWith('analyze_task_for_mode', {
        description: 'Build a complex app',
      });
    });

    it('should set error on failure', async () => {
      mockInvoke.mockResolvedValueOnce({ success: false, data: null, error: 'Analysis error' });

      await useTaskModeStore.getState().analyzeForMode('bad input');

      const state = useTaskModeStore.getState();
      expect(state.strategyAnalysis).toBeNull();
      expect(state.error).toBe('Analysis error');
      expect(state.isLoading).toBe(false);
    });

    it('should handle invoke exception', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      await useTaskModeStore.getState().analyzeForMode('test');

      const state = useTaskModeStore.getState();
      expect(state.error).toContain('Network error');
      expect(state.isLoading).toBe(false);
    });
  });

  // =========================================================================
  // dismissSuggestion
  // =========================================================================
  describe('dismissSuggestion', () => {
    it('should set suggestionDismissed to true', () => {
      useTaskModeStore.getState().dismissSuggestion();
      expect(useTaskModeStore.getState().suggestionDismissed).toBe(true);
    });
  });

  // =========================================================================
  // enterTaskMode
  // =========================================================================
  describe('enterTaskMode', () => {
    it('should enter task mode and update state on success', async () => {
      const session = mockSession();
      session.status = 'exploring';
      mockInvoke.mockResolvedValueOnce({ success: true, data: session, error: null });

      await useTaskModeStore.getState().enterTaskMode('Build feature X');

      const state = useTaskModeStore.getState();
      expect(state.sessionId).toBe('session-123');
      expect(state.status).toBe('exploring');
      expect(state.strategyAnalysis).toEqual(session.strategyAnalysis);
      expect(state.isLoading).toBe(false);
      expect(useContextSourcesStore.getState().memorySessionId).toBe('session-123');
      expect(mockInvoke).toHaveBeenCalledWith('enter_task_mode', {
        description: 'Build feature X',
      });
    });

    it('should set error on failure', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Already in task mode',
      });

      await useTaskModeStore.getState().enterTaskMode('test');

      const state = useTaskModeStore.getState();
      expect(state.error).toBe('Already in task mode');
    });

    it('falls back unknown backend status to initialized', async () => {
      const session = {
        ...mockSession(),
        status: 'future_status',
      } as unknown as TaskModeSession;
      mockInvoke.mockResolvedValueOnce({ success: true, data: session, error: null });

      await useTaskModeStore.getState().enterTaskMode('Build feature X');

      const state = useTaskModeStore.getState();
      expect(state.status).toBe('initialized');
      expect(state.error).toContain("Unknown task mode session status 'future_status'");
    });
  });

  // =========================================================================
  // generatePrd
  // =========================================================================
  describe('generatePrd', () => {
    it('should generate PRD and store it', async () => {
      // First enter task mode
      useTaskModeStore.setState({ sessionId: 'session-123' });
      useContextSourcesStore.setState({ memorySessionId: null });

      const prd = mockPrd();
      mockInvoke.mockResolvedValueOnce({ success: true, data: prd, error: null });

      await useTaskModeStore.getState().generatePrd();

      const state = useTaskModeStore.getState();
      expect(state.prd).toEqual(prd);
      expect(state.isLoading).toBe(false);
      expect(useContextSourcesStore.getState().memorySessionId).toBe('session-123');
    });

    it('should set error if no active session', async () => {
      await useTaskModeStore.getState().generatePrd();

      const state = useTaskModeStore.getState();
      expect(state.error).toBe('No active session');
    });

    it('should set error on backend failure', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });
      mockInvoke.mockResolvedValueOnce({ success: false, data: null, error: 'Wrong status' });

      await useTaskModeStore.getState().generatePrd();

      expect(useTaskModeStore.getState().error).toBe('Wrong status');
    });

    it('injects knowledge contextSources into generate_task_prd payload', async () => {
      useProjectsStore.setState({
        selectedProject: {
          id: 'proj-kb',
          name: 'KB Project',
          path: '/tmp/proj-kb',
          last_activity: new Date().toISOString(),
          session_count: 0,
          message_count: 0,
        },
      });
      useContextSourcesStore.setState({
        knowledgeEnabled: true,
        selectedCollections: ['col-1'],
        selectedDocuments: [{ collection_id: 'col-1', document_uid: 'doc-1' }],
      });
      useTaskModeStore.setState({ sessionId: 'session-123' });
      mockInvoke.mockResolvedValueOnce({ success: true, data: mockPrd(), error: null });

      await useTaskModeStore.getState().generatePrd(undefined, undefined, 'openai', 'gpt-test', 'http://localhost');

      const call = mockInvoke.mock.calls.find(([command]) => command === 'generate_task_prd');
      expect(call).toBeDefined();
      const args = (
        call?.[1] as
          | {
              request?: {
                contextSources?: {
                  project_id?: string;
                  knowledge?: {
                    enabled?: boolean;
                    selected_collections?: string[];
                    selected_documents?: Array<{ collection_id: string; document_uid: string }>;
                  };
                };
              };
            }
          | undefined
      )?.request?.contextSources;
      expect(args?.project_id).toBe('proj-kb');
      expect(args?.knowledge?.enabled).toBe(true);
      expect(args?.knowledge?.selected_collections).toEqual(['col-1']);
      expect(args?.knowledge?.selected_documents).toEqual([{ collection_id: 'col-1', document_uid: 'doc-1' }]);
    });
  });

  // =========================================================================
  // approvePrd
  // =========================================================================
  describe('approvePrd', () => {
    it('should approve PRD and keep editable PRD state', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });

      const prd = mockPrd();
      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await useTaskModeStore.getState().approvePrd(prd);

      const state = useTaskModeStore.getState();
      expect(state.prd).toEqual(prd);
      expect(state.isLoading).toBe(false);
    });

    it('should set error if PRD has no stories', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'PRD must contain at least one story',
      });

      const emptyPrd: TaskPrd = {
        title: 'Empty',
        description: 'No stories',
        stories: [],
        batches: [],
      };

      await useTaskModeStore.getState().approvePrd(emptyPrd);

      expect(useTaskModeStore.getState().error).toBe('PRD must contain at least one story');
    });

    it('passes globalDefaultAgent to approve_task_prd payload', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });
      useSettingsStore.setState({ defaultAgent: 'codex' });
      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await useTaskModeStore.getState().approvePrd(mockPrd());

      const call = mockInvoke.mock.calls.find(([command]) => command === 'approve_task_prd');
      expect(call).toBeDefined();
      expect(
        (
          call?.[1] as
            | {
                request?: {
                  globalDefaultAgent?: string | null;
                };
              }
            | undefined
        )?.request?.globalDefaultAgent,
      ).toBe('codex');
    });
  });

  // =========================================================================
  // refreshStatus
  // =========================================================================
  describe('refreshStatus', () => {
    it('should update execution status from backend', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });

      const status: TaskExecutionStatus = {
        sessionId: 'session-123',
        status: 'executing',
        currentBatch: 1,
        totalBatches: 3,
        storyStatuses: { 'story-001': 'completed', 'story-002': 'running' },
        storiesCompleted: 1,
        storiesFailed: 0,
      };
      mockInvoke.mockResolvedValueOnce({ success: true, data: status, error: null });

      await useTaskModeStore.getState().refreshStatus();

      const state = useTaskModeStore.getState();
      expect(state.status).toBe('executing');
      expect(state.currentBatch).toBe(1);
      expect(state.totalBatches).toBe(3);
      expect(state.storyStatuses).toEqual({
        'story-001': 'completed',
        'story-002': 'running',
      });
    });

    it('should do nothing if no session', async () => {
      await useTaskModeStore.getState().refreshStatus();
      expect(mockInvoke).not.toHaveBeenCalled();
    });

    it('normalizes unknown status returned by refreshStatus', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });
      const unknownStatus = {
        sessionId: 'session-123',
        status: 'post_processing',
        currentBatch: 2,
        totalBatches: 4,
        storyStatuses: {},
        storiesCompleted: 2,
        storiesFailed: 0,
      } as unknown as TaskExecutionStatus;
      mockInvoke.mockResolvedValueOnce({ success: true, data: unknownStatus, error: null });

      await useTaskModeStore.getState().refreshStatus();

      const state = useTaskModeStore.getState();
      expect(state.status).toBe('initialized');
      expect(state.error).toContain("Unknown task mode session status 'post_processing'");
    });
  });

  // =========================================================================
  // cancelExecution
  // =========================================================================
  describe('cancelExecution', () => {
    it('should request cancel and wait for event confirmation', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });
      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await useTaskModeStore.getState().cancelExecution();

      expect(useTaskModeStore.getState().isCancelling).toBe(true);
      expect(useTaskModeStore.getState().isLoading).toBe(false);
    });

    it('should set error if no session', async () => {
      await useTaskModeStore.getState().cancelExecution();
      expect(useTaskModeStore.getState().error).toBe('No active session');
    });
  });

  // =========================================================================
  // fetchReport
  // =========================================================================
  describe('fetchReport', () => {
    it('should fetch and store execution report', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });

      const report: ExecutionReport = {
        sessionId: 'session-123',
        totalStories: 5,
        storiesCompleted: 4,
        storiesFailed: 1,
        totalDurationMs: 30000,
        agentAssignments: {},
        success: false,
      };
      mockInvoke.mockResolvedValueOnce({ success: true, data: report, error: null });

      await useTaskModeStore.getState().fetchReport();

      expect(useTaskModeStore.getState().report).toEqual(report);
      expect(useTaskModeStore.getState().isLoading).toBe(false);
    });

    it('should set error if no session', async () => {
      await useTaskModeStore.getState().fetchReport();
      expect(useTaskModeStore.getState().error).toBe('No active session');
    });
  });

  // =========================================================================
  // exitTaskMode
  // =========================================================================
  describe('exitTaskMode', () => {
    it('should reset state on successful exit', async () => {
      useTaskModeStore.setState({
        sessionId: 'session-123',
      });
      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await useTaskModeStore.getState().exitTaskMode();

      const state = useTaskModeStore.getState();
      expect(state.sessionId).toBeNull();
      expect(state.prd).toBeNull();
      expect(useContextSourcesStore.getState().memorySessionId).toBeNull();
    });

    it('should set error on failure', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Invalid session',
      });

      await useTaskModeStore.getState().exitTaskMode();

      expect(useTaskModeStore.getState().error).toBe('Invalid session');
    });
  });

  // =========================================================================
  // Reset
  // =========================================================================
  describe('reset', () => {
    it('should reset all state to defaults', () => {
      useTaskModeStore.setState({
        sessionId: 'session-123',
        prd: mockPrd(),
        currentBatch: 2,
        error: 'old error',
      });

      useTaskModeStore.getState().reset();

      const state = useTaskModeStore.getState();
      expect(state.sessionId).toBeNull();
      expect(state.status).toBe('initialized');
      expect(state.prd).toBeNull();
      expect(state.error).toBeNull();
      expect(useContextSourcesStore.getState().memorySessionId).toBeNull();
    });
  });

  describe('normalizeTaskModeSessionStatus', () => {
    it('accepts known exploring status', () => {
      const normalized = normalizeTaskModeSessionStatus('exploring');
      expect(normalized.status).toBe('exploring');
      expect(normalized.warning).toBeNull();
    });
  });
});
