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

type EventCallback = (event: { payload: unknown }) => void;
const eventHandlers: Record<string, EventCallback> = {};

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation((eventName: string, handler: EventCallback) => {
    eventHandlers[eventName] = handler;
    return Promise.resolve(() => {
      delete eventHandlers[eventName];
    });
  }),
}));

// Import after mocks
import { useTaskModeStore } from './taskMode';
import type {
  StrategyAnalysis,
  TaskModeSession,
  TaskPrd,
  TaskExecutionStatus,
  ExecutionReport,
} from './taskMode';

// Helpers
function resetStore() {
  useTaskModeStore.getState().reset();
  vi.clearAllMocks();
  // Clear event handlers
  Object.keys(eventHandlers).forEach((k) => delete eventHandlers[k]);
}

function emitEvent(eventName: string, payload: unknown) {
  const handler = eventHandlers[eventName];
  if (!handler) {
    throw new Error(`No listener for "${eventName}"`);
  }
  handler({ payload });
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
    it('should start with idle status and no session', () => {
      const state = useTaskModeStore.getState();
      expect(state.isTaskMode).toBe(false);
      expect(state.sessionId).toBeNull();
      expect(state.strategyAnalysis).toBeNull();
      expect(state.suggestionDismissed).toBe(false);
      expect(state.sessionStatus).toBe('idle');
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
      mockInvoke.mockResolvedValueOnce({ success: true, data: session, error: null });

      await useTaskModeStore.getState().enterTaskMode('Build feature X');

      const state = useTaskModeStore.getState();
      expect(state.isTaskMode).toBe(true);
      expect(state.sessionId).toBe('session-123');
      expect(state.sessionStatus).toBe('initialized');
      expect(state.strategyAnalysis).toEqual(session.strategyAnalysis);
      expect(state.isLoading).toBe(false);
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
      expect(state.isTaskMode).toBe(false);
      expect(state.error).toBe('Already in task mode');
    });
  });

  // =========================================================================
  // generatePrd
  // =========================================================================
  describe('generatePrd', () => {
    it('should generate PRD and transition to reviewing_prd', async () => {
      // First enter task mode
      useTaskModeStore.setState({ sessionId: 'session-123', isTaskMode: true });

      const prd = mockPrd();
      mockInvoke.mockResolvedValueOnce({ success: true, data: prd, error: null });

      await useTaskModeStore.getState().generatePrd();

      const state = useTaskModeStore.getState();
      expect(state.prd).toEqual(prd);
      expect(state.sessionStatus).toBe('reviewing_prd');
      expect(state.isLoading).toBe(false);
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
  });

  // =========================================================================
  // approvePrd
  // =========================================================================
  describe('approvePrd', () => {
    it('should approve PRD and transition to executing', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123', isTaskMode: true });

      const prd = mockPrd();
      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await useTaskModeStore.getState().approvePrd(prd);

      const state = useTaskModeStore.getState();
      expect(state.prd).toEqual(prd);
      expect(state.sessionStatus).toBe('executing');
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

      expect(useTaskModeStore.getState().error).toBe(
        'PRD must contain at least one story'
      );
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
      expect(state.sessionStatus).toBe('executing');
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
  });

  // =========================================================================
  // cancelExecution
  // =========================================================================
  describe('cancelExecution', () => {
    it('should cancel execution and update status', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123', sessionStatus: 'executing' });
      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await useTaskModeStore.getState().cancelExecution();

      expect(useTaskModeStore.getState().sessionStatus).toBe('cancelled');
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
      useTaskModeStore.setState({ sessionId: 'session-123', sessionStatus: 'completed' });

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
        isTaskMode: true,
        sessionId: 'session-123',
        sessionStatus: 'completed',
      });
      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await useTaskModeStore.getState().exitTaskMode();

      const state = useTaskModeStore.getState();
      expect(state.isTaskMode).toBe(false);
      expect(state.sessionId).toBeNull();
      expect(state.sessionStatus).toBe('idle');
      expect(state.prd).toBeNull();
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
  // Event Subscription
  // =========================================================================
  describe('event subscription', () => {
    it('should subscribe to task-mode-progress events', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });

      await useTaskModeStore.getState().subscribeToEvents();

      expect(eventHandlers['task-mode-progress']).toBeDefined();
    });

    it('should update store on event payload', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });

      await useTaskModeStore.getState().subscribeToEvents();

      emitEvent('task-mode-progress', {
        sessionId: 'session-123',
        currentBatch: 2,
        totalBatches: 3,
        storyStatuses: { 'story-001': 'completed', 'story-002': 'running' },
        storiesCompleted: 1,
        storiesFailed: 0,
      });

      const state = useTaskModeStore.getState();
      expect(state.currentBatch).toBe(2);
      expect(state.totalBatches).toBe(3);
      expect(state.storyStatuses['story-001']).toBe('completed');
    });

    it('should ignore events for different session', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123', currentBatch: 0 });

      await useTaskModeStore.getState().subscribeToEvents();

      emitEvent('task-mode-progress', {
        sessionId: 'other-session',
        currentBatch: 5,
        totalBatches: 10,
        storyStatuses: {},
        storiesCompleted: 0,
        storiesFailed: 0,
      });

      expect(useTaskModeStore.getState().currentBatch).toBe(0);
    });

    it('should unsubscribe from events', async () => {
      useTaskModeStore.setState({ sessionId: 'session-123' });

      await useTaskModeStore.getState().subscribeToEvents();
      expect(eventHandlers['task-mode-progress']).toBeDefined();

      useTaskModeStore.getState().unsubscribeFromEvents();
      expect(eventHandlers['task-mode-progress']).toBeUndefined();
    });
  });

  // =========================================================================
  // Reset
  // =========================================================================
  describe('reset', () => {
    it('should reset all state to defaults', () => {
      useTaskModeStore.setState({
        isTaskMode: true,
        sessionId: 'session-123',
        sessionStatus: 'executing',
        prd: mockPrd(),
        currentBatch: 2,
        error: 'old error',
      });

      useTaskModeStore.getState().reset();

      const state = useTaskModeStore.getState();
      expect(state.isTaskMode).toBe(false);
      expect(state.sessionId).toBeNull();
      expect(state.sessionStatus).toBe('idle');
      expect(state.prd).toBeNull();
      expect(state.error).toBeNull();
    });
  });
});
