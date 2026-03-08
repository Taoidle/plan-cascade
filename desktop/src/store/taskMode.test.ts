import { describe, it, expect, beforeEach, vi } from 'vitest';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { normalizeTaskModeSessionStatus, useTaskModeStore } from './taskMode';
import type { StrategyAnalysis, TaskModeSession, TaskPrd, TaskExecutionStatus, ExecutionReport } from './taskMode';
import { useContextSourcesStore } from './contextSources';
import { useProjectsStore } from './projects';
import { useSettingsStore } from './settings';

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
    strategyRecommendation: null,
    configConfirmationState: 'pending',
    confirmedConfig: null,
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

async function enterSession(session: TaskModeSession = mockSession()) {
  mockInvoke.mockResolvedValueOnce({ success: true, data: session, error: null });
  const entered = await useTaskModeStore.getState().enterTaskMode(session.description);
  expect(entered?.sessionId).toBe(session.sessionId);
}

describe('TaskModeStore', () => {
  beforeEach(() => {
    resetStore();
  });

  describe('initial state', () => {
    it('starts with command-client state only', () => {
      const state = useTaskModeStore.getState();
      expect(state.isLoading).toBe(false);
      expect(state.isCancelling).toBe(false);
      expect(state.error).toBeNull();
      expect(state._requestId).toBeGreaterThanOrEqual(0);
    });
  });

  describe('analyzeForMode', () => {
    it('returns strategy analysis on success', async () => {
      const analysis = mockAnalysis('task');
      mockInvoke.mockResolvedValueOnce({ success: true, data: analysis, error: null });

      const result = await useTaskModeStore.getState().analyzeForMode('Build a complex app');

      expect(result).toEqual(analysis);
      expect(useTaskModeStore.getState().isLoading).toBe(false);
      expect(useTaskModeStore.getState().error).toBeNull();
      expect(mockInvoke).toHaveBeenCalledWith('analyze_task_for_mode', {
        description: 'Build a complex app',
      });
    });

    it('sets error on backend failure', async () => {
      mockInvoke.mockResolvedValueOnce({ success: false, data: null, error: 'Analysis error' });

      const result = await useTaskModeStore.getState().analyzeForMode('bad input');

      expect(result).toBeNull();
      expect(useTaskModeStore.getState().error).toBe('Analysis error');
      expect(useTaskModeStore.getState().isLoading).toBe(false);
    });

    it('handles invoke exception', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      const result = await useTaskModeStore.getState().analyzeForMode('test');

      expect(result).toBeNull();
      expect(useTaskModeStore.getState().error).toContain('Network error');
      expect(useTaskModeStore.getState().isLoading).toBe(false);
    });
  });

  describe('enterTaskMode', () => {
    it('returns session and sets memory session id on success', async () => {
      const session = mockSession();
      session.status = 'exploring';
      mockInvoke.mockResolvedValueOnce({ success: true, data: session, error: null });

      const result = await useTaskModeStore.getState().enterTaskMode('Build feature X');

      expect(result?.sessionId).toBe('session-123');
      expect(result?.status).toBe('exploring');
      expect(useContextSourcesStore.getState().memorySessionId).toBe('session-123');
      expect(useTaskModeStore.getState().isLoading).toBe(false);
      expect(mockInvoke).toHaveBeenCalledWith('enter_task_mode', {
        request: {
          description: 'Build feature X',
          kernelSessionId: null,
          locale: expect.any(String),
        },
      });
    });

    it('sets error on failure', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Already in task mode',
      });

      const result = await useTaskModeStore.getState().enterTaskMode('test');

      expect(result).toBeNull();
      expect(useTaskModeStore.getState().error).toBe('Already in task mode');
    });

    it('normalizes unknown backend status to initialized', async () => {
      const session = {
        ...mockSession(),
        status: 'future_status',
      } as unknown as TaskModeSession;
      mockInvoke.mockResolvedValueOnce({ success: true, data: session, error: null });

      const result = await useTaskModeStore.getState().enterTaskMode('Build feature X');

      expect(result?.status).toBe('initialized');
      expect(useTaskModeStore.getState().error).toContain("Unknown task mode session status 'future_status'");
    });
  });

  describe('generatePrd', () => {
    it('returns PRD on success', async () => {
      await enterSession();
      const prd = mockPrd();
      mockInvoke.mockResolvedValueOnce({ success: true, data: prd, error: null });

      const result = await useTaskModeStore.getState().generatePrd();

      expect(result).toEqual(prd);
      expect(useTaskModeStore.getState().isLoading).toBe(false);
      expect(useContextSourcesStore.getState().memorySessionId).toBe('session-123');
    });

    it('sets error if no active session', async () => {
      const result = await useTaskModeStore.getState().generatePrd();

      expect(result).toBeNull();
      expect(useTaskModeStore.getState().error).toBe('No active session');
    });

    it('sets error on backend failure', async () => {
      await enterSession();
      mockInvoke.mockResolvedValueOnce({ success: false, data: null, error: 'Wrong status' });

      const result = await useTaskModeStore.getState().generatePrd();

      expect(result).toBeNull();
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
      await enterSession();
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

  describe('approvePrd', () => {
    it('approves PRD successfully', async () => {
      await enterSession();
      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      const ok = await useTaskModeStore.getState().approvePrd(mockPrd());

      expect(ok).toBe(true);
      expect(useTaskModeStore.getState().isLoading).toBe(false);
    });

    it('sets error when no active session', async () => {
      const ok = await useTaskModeStore.getState().approvePrd(mockPrd());
      expect(ok).toBe(false);
      expect(useTaskModeStore.getState().error).toBe('No active session');
    });

    it('passes globalDefaultAgent to approve_task_prd payload', async () => {
      await enterSession();
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

  describe('applyPrdFeedback', () => {
    it('applies PRD feedback successfully', async () => {
      await enterSession();
      const updatedPrd = mockPrd();
      updatedPrd.title = 'PRD: Updated';
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: {
          prd: updatedPrd,
          summary: {
            addedStoryIds: ['story-003'],
            removedStoryIds: [],
            updatedStoryIds: ['story-002'],
            batchChanges: ['batch_count:2->3'],
            warnings: [],
          },
        },
        error: null,
      });

      const result = await useTaskModeStore.getState().applyPrdFeedback('Split story-002');

      expect(result?.prd.title).toBe('PRD: Updated');
      expect(result?.summary.updatedStoryIds).toEqual(['story-002']);
    });

    it('fails when no active session exists', async () => {
      const result = await useTaskModeStore.getState().applyPrdFeedback('Need update');
      expect(result).toBeNull();
      expect(useTaskModeStore.getState().error).toBe('No active session');
    });
  });

  describe('refreshStatus', () => {
    it('returns latest execution status', async () => {
      await enterSession();
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

      const result = await useTaskModeStore.getState().refreshStatus();

      expect(result?.status).toBe('executing');
      expect(result?.currentBatch).toBe(1);
      expect(result?.totalBatches).toBe(3);
    });

    it('returns null if no session', async () => {
      const result = await useTaskModeStore.getState().refreshStatus();
      expect(result).toBeNull();
      expect(mockInvoke).not.toHaveBeenCalled();
    });

    it('normalizes unknown status returned by refreshStatus', async () => {
      await enterSession();
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

      const result = await useTaskModeStore.getState().refreshStatus();

      expect(result?.status).toBe('initialized');
      expect(useTaskModeStore.getState().error).toContain("Unknown task mode session status 'post_processing'");
    });
  });

  describe('cancelExecution', () => {
    it('requests cancellation and keeps isCancelling=true until progress event confirms', async () => {
      await enterSession();
      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      const ok = await useTaskModeStore.getState().cancelExecution();

      expect(ok).toBe(true);
      expect(useTaskModeStore.getState().isCancelling).toBe(true);
    });

    it('sets error if no session', async () => {
      const ok = await useTaskModeStore.getState().cancelExecution();
      expect(ok).toBe(false);
      expect(useTaskModeStore.getState().error).toBe('No active session');
    });
  });

  describe('fetchReport', () => {
    it('returns execution report', async () => {
      await enterSession();
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

      const result = await useTaskModeStore.getState().fetchReport();

      expect(result).toEqual(report);
    });

    it('sets error if no session', async () => {
      const result = await useTaskModeStore.getState().fetchReport();
      expect(result).toBeNull();
      expect(useTaskModeStore.getState().error).toBe('No active session');
    });
  });

  describe('exitTaskMode', () => {
    it('clears active session and memory scope on successful exit', async () => {
      await enterSession();
      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      const ok = await useTaskModeStore.getState().exitTaskMode();

      expect(ok).toBe(true);
      expect(useContextSourcesStore.getState().memorySessionId).toBeNull();
    });

    it('sets error on failure', async () => {
      await enterSession();
      mockInvoke.mockResolvedValueOnce({ success: false, data: null, error: 'Invalid session' });

      const ok = await useTaskModeStore.getState().exitTaskMode();

      expect(ok).toBe(false);
      expect(useTaskModeStore.getState().error).toBe('Invalid session');
    });
  });

  describe('reset', () => {
    it('resets command-client state and clears memory scope', () => {
      useTaskModeStore.setState({ isLoading: true, error: 'old error' });
      useContextSourcesStore.getState().setMemorySessionId('session-123');

      useTaskModeStore.getState().reset();

      const state = useTaskModeStore.getState();
      expect(state.isLoading).toBe(false);
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
