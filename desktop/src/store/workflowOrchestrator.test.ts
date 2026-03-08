import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TaskPrd } from './taskMode';

const listenMock = vi.fn();
const invokeMock = vi.fn();
const synthesizeExecutionTurnMock = vi.fn();
const routedCards: Array<{ mode: string; payload: { cardType?: string; data?: Record<string, unknown> } }> = [];

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock('./modeTranscriptRouting', () => ({
  routeModeCard: vi.fn(async (mode: string, payload: { cardType?: string; data?: Record<string, unknown> }) => {
    routedCards.push({ mode, payload });
  }),
  routeModeStreamLine: vi.fn(async () => undefined),
}));

vi.mock('../lib/contextBridge', () => ({
  buildConversationHistory: () => [],
  buildRootConversationHistory: () => [],
  buildRootConversationContextString: () => undefined,
  synthesizePlanningTurn: vi.fn(),
  synthesizeExecutionTurn: (...args: unknown[]) => synthesizeExecutionTurnMock(...args),
}));

import { useExecutionStore } from './execution';
import { useTaskModeStore } from './taskMode';
import { useWorkflowOrchestratorStore } from './workflowOrchestrator';
import { useWorkflowKernelStore } from './workflowKernel';

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
  return routedCards
    .filter((entry) => entry.mode === 'task')
    .map((entry) => entry.payload)
    .filter((card) => card.cardType === 'completion_report');
}

function extractCards(cardType: string) {
  return routedCards
    .filter((entry) => entry.mode === 'task')
    .map((entry) => entry.payload)
    .filter((card) => card.cardType === cardType);
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
    routedCards.length = 0;

    useExecutionStore.getState().reset();
    useTaskModeStore.getState().reset();
    useWorkflowOrchestratorStore.getState().resetWorkflow();
    useWorkflowKernelStore.getState().reset();

    useTaskModeStore.setState({
      sessionId: 'task-session',
      currentBatch: 0,
      totalBatches: 0,
      storyStatuses: {},
      qualityGateResults: {},
      error: null,
      isCancelling: false,
      approvePrd: vi.fn().mockResolvedValue(true),
      cancelExecution: vi.fn().mockResolvedValue(true),
      fetchReport: vi.fn().mockResolvedValue(null),
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

    expect(useWorkflowOrchestratorStore.getState().storyStatuses).toEqual({});
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
    expect(workflowState.phase).toBe('cancelled');
    expect(workflowState.isCancelling).toBe(false);
  });

  it('injects one completion card with report data without frontend summary synthesis', async () => {
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
      fetchReport: vi.fn().mockResolvedValue(report),
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
    expect(synthesizeExecutionTurnMock).not.toHaveBeenCalled();
  });

  it('injects one fallback completion card on report timeout and does not append late report card', async () => {
    vi.useFakeTimers();
    useTaskModeStore.setState({
      fetchReport: vi.fn().mockImplementation(
        () =>
          new Promise<{
            sessionId: string;
            totalStories: number;
            storiesCompleted: number;
            storiesFailed: number;
            totalDurationMs: number;
            agentAssignments: Record<string, string>;
            success: boolean;
          }>((resolve) => {
            setTimeout(() => {
              resolve({
                sessionId: 'task-session',
                totalStories: 1,
                storiesCompleted: 1,
                storiesFailed: 0,
                totalDurationMs: 9999,
                agentAssignments: { 'story-1': 'late-agent' },
                success: true,
              });
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
    expect(synthesizeExecutionTurnMock).not.toHaveBeenCalled();
  });

  it('uses kernel linked task session id when local session id is missing', async () => {
    const approvePrd = vi.fn().mockResolvedValue(true);
    useTaskModeStore.setState({
      approvePrd,
    } as unknown as ReturnType<typeof useTaskModeStore.getState>);
    useWorkflowOrchestratorStore.setState({
      sessionId: null,
      phase: 'reviewing_prd',
      editablePrd: TEST_PRD,
      _runToken: 5,
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
    useWorkflowKernelStore.setState({
      sessionId: 'kernel-task-1',
      session: {
        sessionId: 'kernel-task-1',
        status: 'active',
        activeMode: 'task',
        modeSnapshots: {
          chat: null,
          plan: null,
          task: {
            phase: 'reviewing_prd',
            prdId: null,
            currentStoryId: null,
            interviewSessionId: null,
            pendingInterview: null,
            completedStories: 0,
            failedStories: 0,
          },
        },
        handoffContext: {
          conversationContext: [],
          artifactRefs: [],
          contextSources: ['simple_mode'],
          metadata: {},
        },
        linkedModeSessions: {
          task: 'linked-task-session',
        },
        lastError: null,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        lastCheckpointId: null,
      },
    } as unknown as ReturnType<typeof useWorkflowKernelStore.getState>);

    await useWorkflowOrchestratorStore.getState().approvePrd(TEST_PRD);

    expect(approvePrd).toHaveBeenCalledWith(TEST_PRD, 'linked-task-session', {
      flowLevel: 'quick',
      tddMode: 'off',
      enableInterview: false,
      qualityGatesEnabled: true,
      maxParallel: 4,
      skipVerification: false,
      skipReview: false,
      globalAgentOverride: null,
      implAgentOverride: null,
    });
    expect(useWorkflowOrchestratorStore.getState().sessionId).toBe('linked-task-session');
  });

  it('submits standard-flow PRD to architecture review before execution', async () => {
    const approvePrd = vi.fn().mockResolvedValue(true);
    const architectureReview = {
      personaRole: 'SoftwareArchitect',
      analysis: 'Architecture review completed.',
      concerns: [],
      suggestions: ['Consider splitting the implementation flow.'],
      prdModifications: [
        {
          operationId: 'op-1',
          type: 'update_story',
          targetStoryId: 'story-1',
          payload: {
            description: 'Updated by architecture review',
          },
          preview: 'Update Story 1',
          reason: 'Reduce ambiguity',
          confidence: 0.91,
        },
      ],
      approved: false,
    };

    useTaskModeStore.setState({
      approvePrd,
    } as unknown as ReturnType<typeof useTaskModeStore.getState>);
    useWorkflowOrchestratorStore.setState({
      phase: 'reviewing_prd',
      sessionId: 'task-session',
      editablePrd: TEST_PRD,
      _runToken: 9,
      prdReviewStage: 'pre_architecture_review',
      config: {
        flowLevel: 'standard',
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
    invokeMock.mockResolvedValueOnce({
      success: true,
      data: architectureReview,
      error: null,
    });

    const result = await useWorkflowOrchestratorStore.getState().approvePrd(TEST_PRD);

    expect(result.ok).toBe(true);
    expect(approvePrd).not.toHaveBeenCalled();
    expect(useWorkflowOrchestratorStore.getState().phase).toBe('architecture_review');
    expect(useWorkflowOrchestratorStore.getState().architectureReview).toEqual(architectureReview);
    expect(extractCards('architecture_review_card')).toHaveLength(1);
  });

  it('applies architecture modifications and reinjects an executable PRD card', async () => {
    const selectedModifications = [
      {
        operationId: 'op-1',
        type: 'update_story' as const,
        targetStoryId: 'story-1',
        payload: {
          title: 'Story 1 (Revised)',
          description: 'Architecture-adjusted implementation plan',
        },
        preview: 'Update Story 1',
        reason: 'Clarify scope',
        confidence: 0.88,
      },
    ];

    useWorkflowOrchestratorStore.setState({
      phase: 'architecture_review',
      editablePrd: TEST_PRD,
      architectureReview: {
        personaRole: 'SoftwareArchitect',
        analysis: 'Needs a small PRD revision.',
        concerns: [],
        suggestions: ['Tighten the story scope.'],
        prdModifications: selectedModifications,
        approved: false,
      },
      prdReviewStage: 'pre_architecture_review',
      config: {
        flowLevel: 'standard',
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

    const result = await useWorkflowOrchestratorStore.getState().approveArchitecture(false, selectedModifications);

    expect(result.ok).toBe(true);
    expect(useWorkflowOrchestratorStore.getState().phase).toBe('reviewing_prd');
    expect(useWorkflowOrchestratorStore.getState().prdReviewStage).toBe('ready_for_execution');
    expect(useWorkflowOrchestratorStore.getState().editablePrd?.stories[0]).toEqual(
      expect.objectContaining({
        id: 'story-1',
        title: 'Story 1 (Revised)',
        description: 'Architecture-adjusted implementation plan',
      }),
    );

    const prdCards = extractCards('prd_card');
    expect(prdCards).toHaveLength(1);
    expect(prdCards[0]?.data).toEqual(
      expect.objectContaining({
        primaryAction: 'approve_and_execute',
        revisionSource: 'architecture_updated',
      }),
    );
  });
});
