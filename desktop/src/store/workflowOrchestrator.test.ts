import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TaskPrd } from './taskMode';

const listenMock = vi.fn();
const invokeMock = vi.fn();
const synthesizeExecutionTurnMock = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock('../lib/contextBridge', () => ({
  buildConversationHistory: () => [],
  synthesizePlanningTurn: vi.fn(),
  synthesizeExecutionTurn: (...args: unknown[]) => synthesizeExecutionTurnMock(...args),
}));

import { useExecutionStore } from './execution';
import { useTaskModeStore } from './taskMode';
import { useWorkflowOrchestratorStore } from './workflowOrchestrator';

const TEST_PRD: TaskPrd = {
  title: 'Test PRD',
  description: 'desc',
  stories: [
    {
      id: 'story-1',
      title: 'Story 1',
      description: 'do thing',
      priority: 'high',
      dependencies: [],
      acceptanceCriteria: ['done'],
    },
  ],
  batches: [{ index: 0, storyIds: ['story-1'] }],
};

function progressPayload(
  overrides: Partial<{
    sessionId: string;
    eventType: string;
    currentBatch: number;
    totalBatches: number;
    storyId: string | null;
    storyStatus: string | null;
    agentName: string | null;
    gateResults: null;
    error: string | null;
    progressPct: number;
  }>,
) {
  return {
    sessionId: 'task-session',
    eventType: 'story_started',
    currentBatch: 0,
    totalBatches: 1,
    storyId: 'story-1',
    storyStatus: 'running',
    agentName: null,
    gateResults: null,
    error: null,
    progressPct: 10,
    ...overrides,
  };
}

function extractCompletionReportCards() {
  return useExecutionStore
    .getState()
    .streamingOutput.filter((line) => line.type === 'card')
    .map((line) => JSON.parse(line.content) as { cardType?: string; data?: Record<string, unknown> })
    .filter((card) => card.cardType === 'completion_report');
}

describe('workflowOrchestrator task progress events', () => {
  let progressListener: ((event: { payload: ReturnType<typeof progressPayload> }) => void) | null = null;
  const unlisten = vi.fn();

  beforeEach(() => {
    progressListener = null;
    listenMock.mockReset();
    invokeMock.mockReset();
    unlisten.mockReset();

    listenMock.mockImplementation(
      async (_eventName: string, callback: (event: { payload: ReturnType<typeof progressPayload> }) => void) => {
        progressListener = callback;
        return unlisten;
      },
    );

    // Design doc pre-step is best-effort; keep it cheap in tests.
    invokeMock.mockResolvedValue({ success: false, data: null, error: 'skip design doc' });

    useExecutionStore.getState().reset();
    useTaskModeStore.getState().reset();
    useWorkflowOrchestratorStore.getState().resetWorkflow();

    useTaskModeStore.setState({
      isTaskMode: true,
      sessionId: 'task-session',
      sessionStatus: 'reviewing_prd',
      currentBatch: 0,
      totalBatches: 0,
      storyStatuses: {},
      qualityGateResults: {},
      error: null,
      isCancelling: false,
      approvePrd: vi.fn().mockResolvedValue(undefined),
      cancelExecution: vi.fn().mockResolvedValue(undefined),
      fetchReport: vi.fn().mockResolvedValue(undefined),
    } as unknown as ReturnType<typeof useTaskModeStore.getState>);

    useWorkflowOrchestratorStore.setState({
      phase: 'reviewing_prd',
      sessionId: 'task-session',
      editablePrd: TEST_PRD,
      _runToken: 1,
      isCancelling: false,
      config: {
        flowLevel: 'quick',
        tddMode: 'off',
        maxParallel: 4,
        qualityGatesEnabled: true,
        specInterviewEnabled: false,
        skipVerification: false,
        skipReview: false,
        globalAgentOverride: null,
        implAgentOverride: null,
      },
    } as unknown as ReturnType<typeof useWorkflowOrchestratorStore.getState>);

    synthesizeExecutionTurnMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('ignores task progress events from a different session', async () => {
    await useWorkflowOrchestratorStore.getState().approvePrd(TEST_PRD);
    expect(progressListener).not.toBeNull();

    progressListener!({
      payload: progressPayload({
        sessionId: 'other-session',
      }),
    });

    expect(useTaskModeStore.getState().storyStatuses).toEqual({});
  });

  it('converges to cancelled state on execution_cancelled', async () => {
    await useWorkflowOrchestratorStore.getState().approvePrd(TEST_PRD);
    expect(progressListener).not.toBeNull();

    useWorkflowOrchestratorStore.setState({ isCancelling: true });

    progressListener!({
      payload: progressPayload({
        eventType: 'execution_cancelled',
        storyId: null,
        storyStatus: null,
        progressPct: 100,
      }),
    });

    const workflowState = useWorkflowOrchestratorStore.getState();
    const taskState = useTaskModeStore.getState();

    expect(workflowState.phase).toBe('cancelled');
    expect(workflowState.isCancelling).toBe(false);
    expect(taskState.sessionStatus).toBe('cancelled');
    expect(taskState.isCancelling).toBe(false);
  });

  it('syncRuntimeFromKernel hydrates runtime IDs/questions without overwriting orchestrator phase', () => {
    useWorkflowOrchestratorStore.setState({
      phase: 'reviewing_prd',
      sessionId: null,
      pendingQuestion: null,
    } as unknown as ReturnType<typeof useWorkflowOrchestratorStore.getState>);

    useWorkflowOrchestratorStore.getState().syncRuntimeFromKernel({
      sessionId: 'task-session',
      phase: 'executing',
      pendingQuestion: {
        questionId: 'q-1',
        question: 'Need auth?',
        hint: null,
        required: true,
        inputType: 'text',
        options: [],
        allowCustom: true,
        questionNumber: 1,
        totalQuestions: 2,
      },
    });

    const state = useWorkflowOrchestratorStore.getState();
    expect(state.phase).toBe('reviewing_prd');
    expect(state.sessionId).toBe('task-session');
    expect(state.pendingQuestion?.questionId).toBe('q-1');
  });

  it('injects one completion card with report data and synthesizes matching summary', async () => {
    const report = {
      sessionId: 'task-session',
      totalStories: 1,
      storiesCompleted: 1,
      storiesFailed: 0,
      totalDurationMs: 1234,
      agentAssignments: { 'story-1': 'impl-agent' },
      success: true,
    };
    useTaskModeStore.setState({
      report,
      fetchReport: vi.fn().mockResolvedValue(undefined),
    } as unknown as ReturnType<typeof useTaskModeStore.getState>);

    await useWorkflowOrchestratorStore.getState().approvePrd(TEST_PRD);
    expect(progressListener).not.toBeNull();

    progressListener!({
      payload: progressPayload({
        eventType: 'story_completed',
        storyId: 'story-1',
        storyStatus: 'completed',
        progressPct: 80,
      }),
    });
    progressListener!({
      payload: progressPayload({
        eventType: 'execution_completed',
        storyId: null,
        storyStatus: null,
        progressPct: 100,
      }),
    });

    await new Promise((resolve) => setTimeout(resolve, 0));

    const completionCards = extractCompletionReportCards();
    expect(completionCards).toHaveLength(1);
    expect(completionCards[0]?.data).toEqual(
      expect.objectContaining({
        success: true,
        totalStories: 1,
        completed: 1,
        failed: 0,
        duration: 1234,
      }),
    );
    expect(synthesizeExecutionTurnMock).toHaveBeenCalledWith(1, 1, true);
  });

  it('injects one fallback completion card on report timeout and does not append late report card', async () => {
    vi.useFakeTimers();
    useTaskModeStore.setState({
      report: null,
      fetchReport: vi.fn().mockImplementation(
        () =>
          new Promise<void>((resolve) => {
            setTimeout(() => {
              useTaskModeStore.setState({
                report: {
                  sessionId: 'task-session',
                  totalStories: 1,
                  storiesCompleted: 1,
                  storiesFailed: 0,
                  totalDurationMs: 9999,
                  agentAssignments: { 'story-1': 'late-agent' },
                  success: true,
                },
              } as unknown as ReturnType<typeof useTaskModeStore.getState>);
              resolve();
            }, 2000);
          }),
      ),
    } as unknown as ReturnType<typeof useTaskModeStore.getState>);

    await useWorkflowOrchestratorStore.getState().approvePrd(TEST_PRD);
    expect(progressListener).not.toBeNull();

    progressListener!({
      payload: progressPayload({
        eventType: 'story_completed',
        storyId: 'story-1',
        storyStatus: 'completed',
        progressPct: 80,
      }),
    });
    progressListener!({
      payload: progressPayload({
        eventType: 'execution_completed',
        storyId: null,
        storyStatus: null,
        progressPct: 100,
      }),
    });

    await vi.advanceTimersByTimeAsync(1500);
    await Promise.resolve();

    let completionCards = extractCompletionReportCards();
    expect(completionCards).toHaveLength(1);
    expect(completionCards[0]?.data).toEqual(
      expect.objectContaining({
        success: true,
        totalStories: 1,
        completed: 1,
        failed: 0,
        duration: 0,
      }),
    );

    await vi.advanceTimersByTimeAsync(1000);
    await Promise.resolve();

    completionCards = extractCompletionReportCards();
    expect(completionCards).toHaveLength(1);
    expect(synthesizeExecutionTurnMock).toHaveBeenCalledTimes(1);
    expect(synthesizeExecutionTurnMock).toHaveBeenCalledWith(1, 1, true);
  });
});
