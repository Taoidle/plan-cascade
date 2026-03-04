import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TaskPrd } from './taskMode';

const listenMock = vi.fn();
const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
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
});
