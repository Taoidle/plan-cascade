import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PlanCardData, PlanModeProgressPayload } from '../types/planModeCard';

const listenMock = vi.fn();
const resolveRootSessionForModeMock = vi.fn();
const routedCards: Array<{ mode: string; payload: { cardType?: string; data?: Record<string, unknown> } }> = [];

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

vi.mock('./modeTranscriptRouting', () => ({
  routeModeCard: vi.fn(async (mode: string, payload: { cardType?: string; data?: Record<string, unknown> }) => {
    routedCards.push({ mode, payload });
  }),
  routeModeStreamLine: vi.fn(async () => undefined),
  resolveRootSessionForMode: (...args: unknown[]) => resolveRootSessionForModeMock(...args),
}));

import { useExecutionStore } from './execution';
import { usePlanModeStore } from './planMode';
import { usePlanOrchestratorStore } from './planOrchestrator';
import { useWorkflowKernelStore } from './workflowKernel';

const TEST_PLAN: PlanCardData = {
  title: 'Test Plan',
  description: 'desc',
  domain: 'general',
  adapterName: 'default',
  editable: true,
  steps: [
    {
      id: 'step-1',
      title: 'Step 1',
      description: 'do thing',
      priority: 'medium',
      dependencies: [],
      completionCriteria: ['done'],
      expectedOutput: 'result',
    },
  ],
  batches: [{ index: 0, stepIds: ['step-1'] }],
};

function payload(base: Partial<PlanModeProgressPayload>): PlanModeProgressPayload {
  return {
    sessionId: 'session-1',
    eventType: 'step_started',
    currentBatch: 0,
    totalBatches: 1,
    progressPct: 10,
    ...base,
  };
}

function extractPlanCompletionCards() {
  return routedCards
    .filter((entry) => entry.mode === 'plan')
    .map((entry) => entry.payload)
    .filter((card) => card.cardType === 'plan_completion_card');
}

function extractPlanStepUpdateCards() {
  return routedCards
    .filter((entry) => entry.mode === 'plan')
    .map((entry) => entry.payload)
    .filter((card) => card.cardType === 'plan_step_update');
}

function extractPlanStepOutputCards() {
  return routedCards
    .filter((entry) => entry.mode === 'plan')
    .map((entry) => entry.payload)
    .filter((card) => card.cardType === 'plan_step_output');
}

describe('planOrchestrator event handling', () => {
  let progressListener: ((event: { payload: PlanModeProgressPayload }) => void) | null = null;
  const unlisten = vi.fn();

  beforeEach(() => {
    progressListener = null;
    listenMock.mockReset();
    unlisten.mockReset();
    listenMock.mockImplementation(
      async (_eventName: string, callback: (event: { payload: PlanModeProgressPayload }) => void) => {
        progressListener = callback;
        return unlisten;
      },
    );
    resolveRootSessionForModeMock.mockReset();
    resolveRootSessionForModeMock.mockReturnValue(null);
    routedCards.length = 0;

    useExecutionStore.getState().reset();
    usePlanModeStore.getState().reset();
    usePlanOrchestratorStore.getState().resetWorkflow();
    useWorkflowKernelStore.getState().reset();

    usePlanModeStore.setState({
      error: null,
      approvePlan: vi.fn().mockResolvedValue(true),
      retryPlanStep: vi.fn().mockResolvedValue(true),
      fetchStepOutput: vi.fn().mockResolvedValue(null),
      fetchReport: vi.fn().mockResolvedValue(null),
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);

    usePlanOrchestratorStore.setState({
      phase: 'reviewing_plan',
      sessionId: 'session-1',
      editablePlan: TEST_PLAN,
      _runToken: 1,
      isBusy: false,
      isCancelling: false,
    } as unknown as ReturnType<typeof usePlanOrchestratorStore.getState>);
  });

  it('ignores progress events from a different session', async () => {
    await usePlanOrchestratorStore.getState().approvePlan(TEST_PLAN);
    expect(progressListener).not.toBeNull();

    progressListener!({
      payload: payload({
        sessionId: 'other-session',
        eventType: 'step_started',
        stepId: 'step-1',
        stepStatus: 'running',
      }),
    });

    expect(usePlanOrchestratorStore.getState().stepStatuses).toEqual({});
  });

  it('does not re-inject progress or completion cards when kernel transcript is authoritative', async () => {
    resolveRootSessionForModeMock.mockImplementation((mode: string, modeSessionId?: string | null) =>
      mode === 'plan' && modeSessionId === 'session-1' ? 'kernel-plan-1' : null,
    );
    useWorkflowKernelStore.setState({
      sessionId: 'kernel-plan-1',
      session: {
        ...createKernelPlanSession(),
        activeMode: 'plan',
        linkedModeSessions: { plan: 'session-1' },
      },
    } as unknown as ReturnType<typeof useWorkflowKernelStore.getState>);

    await usePlanOrchestratorStore.getState().approvePlan(TEST_PLAN);
    expect(progressListener).not.toBeNull();

    progressListener!({
      payload: payload({
        eventType: 'step_started',
        stepId: 'step-1',
        stepStatus: 'running',
      }),
    });

    progressListener!({
      payload: payload({
        eventType: 'step_completed',
        stepId: 'step-1',
        stepStatus: 'completed',
        progressPct: 100,
        stepOutput: {
          stepId: 'step-1',
          summary: 'done',
          content: 'done',
          fullContent: 'done',
          format: 'markdown',
          criteriaMet: [],
          artifacts: [],
          truncated: false,
          originalLength: 4,
          shownLength: 4,
          toolEvidence: [],
        },
      }),
    });

    progressListener!({
      payload: payload({
        eventType: 'execution_completed',
        progressPct: 100,
        terminalReport: {
          sessionId: 'session-1',
          planTitle: 'Test Plan',
          success: true,
          terminalState: 'completed',
          totalSteps: 1,
          stepsCompleted: 1,
          stepsFailed: 0,
          totalDurationMs: 1000,
          stepSummaries: { 'step-1': 'done' },
          failureReasons: {},
          cancelledBy: null,
          runId: 'run-1',
          finalConclusionMarkdown: 'done',
          highlights: [],
          nextActions: ['ship'],
          retryStats: { totalRetries: 0, stepsRetried: 0, exhaustedFailures: 0 },
        },
      }),
    });

    expect(extractPlanStepUpdateCards()).toHaveLength(0);
    expect(extractPlanStepOutputCards()).toHaveLength(0);
    expect(extractPlanCompletionCards()).toHaveLength(0);
    expect(usePlanOrchestratorStore.getState().stepStatuses['step-1']).toBe('completed');
    expect(usePlanOrchestratorStore.getState().report?.runId).toBe('run-1');
    expect(usePlanOrchestratorStore.getState()._completionCardInjectedRunToken).toBe(1);
  });

  it('converges to cancelled state on execution_cancelled event', async () => {
    await usePlanOrchestratorStore.getState().approvePlan(TEST_PLAN);
    expect(progressListener).not.toBeNull();

    usePlanOrchestratorStore.setState({ isBusy: true, isCancelling: true });

    progressListener!({
      payload: payload({
        eventType: 'execution_cancelled',
        progressPct: 100,
      }),
    });

    const orchestratorState = usePlanOrchestratorStore.getState();
    const planState = usePlanModeStore.getState();

    expect(orchestratorState.phase).toBe('executing');
    expect(orchestratorState.isBusy).toBe(false);
    expect(orchestratorState.isCancelling).toBe(false);
    expect(planState.isCancelling).toBe(false);
    const completionCards = extractPlanCompletionCards();
    expect(completionCards).toHaveLength(1);
    expect(completionCards[0]?.data?.terminalState).toBe('cancelled');
  });

  it('uses terminal cancelled report stats when execution_cancelled includes terminalReport', async () => {
    await usePlanOrchestratorStore.getState().approvePlan(TEST_PLAN);
    expect(progressListener).not.toBeNull();

    progressListener!({
      payload: payload({
        eventType: 'execution_cancelled',
        progressPct: 100,
        terminalReport: {
          sessionId: 'session-1',
          planTitle: 'Test Plan',
          success: false,
          terminalState: 'cancelled',
          totalSteps: 7,
          stepsCompleted: 2,
          stepsFailed: 1,
          stepsCancelled: 3,
          stepsAttempted: 6,
          stepsFailedBeforeCancel: 1,
          totalDurationMs: 4200,
          stepSummaries: {},
          failureReasons: { 'step-4': 'blocked' },
          cancelledBy: 'user',
          runId: 'run-cancelled-1',
          finalConclusionMarkdown: 'cancelled summary',
          highlights: [],
          nextActions: ['resume'],
          retryStats: { totalRetries: 2, stepsRetried: 1, exhaustedFailures: 0 },
        },
      }),
    });

    const completionCards = extractPlanCompletionCards();
    expect(completionCards).toHaveLength(1);
    expect(completionCards[0]?.data?.terminalState).toBe('cancelled');
    expect(completionCards[0]?.data?.stepsCancelled).toBe(3);
    expect(completionCards[0]?.data?.stepsAttempted).toBe(6);
    expect(completionCards[0]?.data?.stepsFailedBeforeCancel).toBe(1);
  });

  it('keeps completed terminal phase when report fetch fails', async () => {
    usePlanModeStore.setState({
      fetchReport: vi.fn().mockRejectedValue(new Error('report fetch failed')),
      report: null,
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);

    await usePlanOrchestratorStore.getState().approvePlan(TEST_PLAN);
    expect(progressListener).not.toBeNull();

    progressListener!({
      payload: payload({
        eventType: 'step_completed',
        stepId: 'step-1',
        stepStatus: 'completed',
      }),
    });

    progressListener!({
      payload: payload({
        eventType: 'execution_completed',
        progressPct: 100,
      }),
    });

    await Promise.resolve();
    await Promise.resolve();

    const orchestratorState = usePlanOrchestratorStore.getState();
    const completionCards = extractPlanCompletionCards();

    expect(orchestratorState.phase).toBe('executing');
    expect(orchestratorState.stepStatuses['step-1']).toBe('completed');
    expect(completionCards).toHaveLength(1);
    expect(completionCards[0]?.data?.success).toBe(true);
  });

  it('marks terminal phase as failed when any step failed even without report', async () => {
    usePlanModeStore.setState({
      fetchReport: vi.fn().mockResolvedValue(undefined),
      report: null,
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);

    await usePlanOrchestratorStore.getState().approvePlan(TEST_PLAN);
    expect(progressListener).not.toBeNull();

    progressListener!({
      payload: payload({
        eventType: 'step_failed',
        stepId: 'step-1',
        stepStatus: 'failed',
        error: 'boom',
      }),
    });

    progressListener!({
      payload: payload({
        eventType: 'execution_completed',
        progressPct: 100,
      }),
    });

    await Promise.resolve();
    await Promise.resolve();

    const orchestratorState = usePlanOrchestratorStore.getState();
    const completionCards = extractPlanCompletionCards();

    expect(orchestratorState.phase).toBe('executing');
    expect(orchestratorState.stepStatuses['step-1']).toBe('failed');
    expect(completionCards).toHaveLength(1);
    expect(completionCards[0]?.data?.success).toBe(false);
  });

  it('records failed step diagnostics with fullContent in update card payload', async () => {
    await usePlanOrchestratorStore.getState().approvePlan(TEST_PLAN);
    expect(progressListener).not.toBeNull();

    progressListener!({
      payload: payload({
        eventType: 'step_failed',
        stepId: 'step-1',
        stepStatus: 'failed',
        error: 'Step output incomplete',
        attemptCount: 2,
        errorCode: 'incomplete_narration',
        stepOutput: {
          stepId: 'step-1',
          summary: 'short summary',
          content: '让我先实现这个模块。',
          fullContent: '让我先实现这个模块。\n这里是失败步骤的完整输出诊断内容。',
          format: 'markdown',
          criteriaMet: [],
          artifacts: [],
          truncated: false,
          originalLength: 29,
          shownLength: 29,
          qualityState: 'incomplete',
          incompleteReason: 'Output is an execution narration rather than a completed result',
          attemptCount: 2,
          iterations: 24,
          stopReason: 'iteration_stalled',
          errorCode: 'iteration_stalled',
          toolEvidence: [],
        },
      }),
    });

    const updateCards = extractPlanStepUpdateCards();
    expect(updateCards).toHaveLength(1);
    expect(updateCards[0]?.data?.eventType).toBe('step_failed');

    const diagnostics = (updateCards[0]?.data?.diagnostics ?? null) as {
      fullContent?: string;
      qualityState?: string;
      attemptCount?: number;
      iterations?: number;
      stopReason?: string;
      errorCode?: string;
    } | null;
    expect(diagnostics?.fullContent).toBe('让我先实现这个模块。\n这里是失败步骤的完整输出诊断内容。');
    expect(diagnostics?.qualityState).toBe('incomplete');
    expect(diagnostics?.attemptCount).toBe(2);
    expect(updateCards[0]?.data?.attemptCount).toBe(2);
    expect(updateCards[0]?.data?.errorCode).toBe('incomplete_narration');
    expect(diagnostics?.iterations).toBe(24);
    expect(diagnostics?.stopReason).toBe('iteration_stalled');
    expect(diagnostics?.errorCode).toBe('iteration_stalled');
  });

  it('injects at most one plan completion card for the same run token', async () => {
    usePlanModeStore.setState({
      fetchReport: vi.fn().mockResolvedValue(undefined),
      report: {
        sessionId: 'session-1',
        planTitle: 'Test Plan',
        success: true,
        terminalState: 'completed',
        totalSteps: 1,
        stepsCompleted: 1,
        stepsFailed: 0,
        totalDurationMs: 1000,
        stepSummaries: { 'step-1': 'done' },
        failureReasons: {},
        cancelledBy: null,
        runId: 'run-1',
        finalConclusionMarkdown: 'all done',
        highlights: ['done'],
        nextActions: ['ship'],
        retryStats: { totalRetries: 0, stepsRetried: 0, exhaustedFailures: 0 },
      },
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);

    await usePlanOrchestratorStore.getState().approvePlan(TEST_PLAN);
    expect(progressListener).not.toBeNull();

    const completionEvent = {
      payload: payload({
        eventType: 'execution_completed',
        progressPct: 100,
      }),
    };

    progressListener!(completionEvent);
    progressListener!(completionEvent);

    await Promise.resolve();
    await Promise.resolve();

    expect(extractPlanCompletionCards()).toHaveLength(1);
  });

  it('injects completion card from kernel terminal fallback when progress terminal report is missing', async () => {
    useWorkflowKernelStore.setState({
      sessionId: 'kernel-plan-1',
      session: {
        ...createKernelPlanSession(),
        activeMode: 'plan',
        modeSnapshots: {
          ...createKernelPlanSession().modeSnapshots,
          plan: {
            ...createKernelPlanSession().modeSnapshots.plan,
            phase: 'completed',
          },
        },
        linkedModeSessions: { plan: 'session-1' },
      },
    } as unknown as ReturnType<typeof useWorkflowKernelStore.getState>);
    usePlanModeStore.setState({
      fetchReport: vi.fn().mockResolvedValue({
        sessionId: 'session-1',
        planTitle: 'Test Plan',
        success: true,
        terminalState: 'completed',
        totalSteps: 1,
        stepsCompleted: 1,
        stepsFailed: 0,
        totalDurationMs: 100,
        stepSummaries: { 'step-1': 'done' },
        failureReasons: {},
        cancelledBy: null,
        runId: 'run-fallback',
        finalConclusionMarkdown: 'final',
        highlights: ['done'],
        nextActions: ['ship'],
        retryStats: { totalRetries: 0, stepsRetried: 0, exhaustedFailures: 0 },
      }),
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);
    usePlanOrchestratorStore.setState({
      phase: 'executing',
      sessionId: 'session-1',
      editablePlan: TEST_PLAN,
      _runToken: 22,
      _completionCardInjectedRunToken: null,
    } as unknown as ReturnType<typeof usePlanOrchestratorStore.getState>);

    await usePlanOrchestratorStore.getState().ensureTerminalCompletionCardFromKernel();

    const completionCards = extractPlanCompletionCards();
    expect(completionCards).toHaveLength(1);
    expect(completionCards[0]?.data?.planTitle).toBe('Test Plan');
  });

  it('retries a single step and converges to completed after execution_completed', async () => {
    const retryPlanStep = vi.fn().mockResolvedValue(true);
    usePlanModeStore.setState({
      retryPlanStep,
      fetchReport: vi.fn().mockResolvedValue(null),
      report: null,
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);
    usePlanOrchestratorStore.setState({
      phase: 'failed',
      editablePlan: TEST_PLAN,
      sessionId: 'session-1',
      _runToken: 5,
    } as unknown as ReturnType<typeof usePlanOrchestratorStore.getState>);

    await usePlanOrchestratorStore.getState().retryStep('step-1');
    expect(progressListener).not.toBeNull();
    expect(retryPlanStep).toHaveBeenCalledWith(
      'step-1',
      expect.any(String),
      expect.any(String),
      undefined,
      undefined,
      expect.objectContaining({
        project_id: 'default',
      }),
      undefined,
      'en-US',
      'session-1',
      null,
      'global',
    );

    progressListener!({
      payload: payload({
        eventType: 'step_started',
        stepId: 'step-1',
        stepStatus: 'running',
      }),
    });
    progressListener!({
      payload: payload({
        eventType: 'step_completed',
        stepId: 'step-1',
        stepStatus: 'completed',
      }),
    });
    progressListener!({
      payload: payload({
        eventType: 'execution_completed',
        progressPct: 100,
      }),
    });

    await Promise.resolve();
    await Promise.resolve();

    expect(usePlanOrchestratorStore.getState().stepStatuses['step-1']).toBe('completed');
    expect(usePlanOrchestratorStore.getState().phase).toBe('executing');
  });

  it('retries a single step and converges to failed when retry step fails', async () => {
    usePlanOrchestratorStore.setState({
      phase: 'failed',
      editablePlan: TEST_PLAN,
      sessionId: 'session-1',
      _runToken: 9,
    } as unknown as ReturnType<typeof usePlanOrchestratorStore.getState>);

    await usePlanOrchestratorStore.getState().retryStep('step-1');
    expect(progressListener).not.toBeNull();

    progressListener!({
      payload: payload({
        eventType: 'step_failed',
        stepId: 'step-1',
        stepStatus: 'failed',
        error: 'retry failed',
      }),
    });
    progressListener!({
      payload: payload({
        eventType: 'execution_completed',
        progressPct: 100,
      }),
    });

    await Promise.resolve();
    await Promise.resolve();

    expect(usePlanOrchestratorStore.getState().stepStatuses['step-1']).toBe('failed');
    expect(usePlanOrchestratorStore.getState().phase).toBe('executing');
  });

  it('falls back to kernel linked session and rolls back to reviewing_plan when approve fails', async () => {
    const approvePlan = vi.fn().mockResolvedValue(false);
    usePlanModeStore.setState({
      approvePlan,
      error: 'backend approve failed',
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);
    usePlanOrchestratorStore.setState({
      sessionId: null,
      phase: 'reviewing_plan',
      editablePlan: TEST_PLAN,
      _runToken: 12,
    } as unknown as ReturnType<typeof usePlanOrchestratorStore.getState>);
    useWorkflowKernelStore.setState({
      sessionId: 'kernel-plan-1',
      session: {
        ...createKernelPlanSession(),
        modeSnapshots: {
          ...createKernelPlanSession().modeSnapshots,
          plan: {
            ...createKernelPlanSession().modeSnapshots.plan,
            phase: 'reviewing_plan',
          },
        },
        linkedModeSessions: { plan: 'linked-plan-1' },
      },
    } as unknown as ReturnType<typeof useWorkflowKernelStore.getState>);

    await usePlanOrchestratorStore.getState().approvePlan(TEST_PLAN);

    expect(approvePlan).toHaveBeenCalledWith(
      TEST_PLAN,
      expect.any(String),
      expect.any(String),
      undefined,
      undefined,
      expect.objectContaining({
        project_id: 'default',
      }),
      undefined,
      'en-US',
      'linked-plan-1',
      null,
      'global',
    );
    expect(usePlanOrchestratorStore.getState().sessionId).toBe('linked-plan-1');
    expect(usePlanOrchestratorStore.getState().phase).toBe('reviewing_plan');
  });
});

describe('planOrchestrator clarification recovery', () => {
  beforeEach(() => {
    useExecutionStore.getState().reset();
    usePlanModeStore.getState().reset();
    usePlanOrchestratorStore.getState().resetWorkflow();
    useWorkflowKernelStore.getState().reset();
  });

  it('enters clarification_error when clarifying has no question', async () => {
    useWorkflowKernelStore.setState({
      sessionId: 'kernel-plan-1',
      session: createKernelPlanSession(),
    } as unknown as ReturnType<typeof useWorkflowKernelStore.getState>);

    const enterPlanMode = vi.fn().mockResolvedValue({
      sessionId: 'plan-session-1',
      phase: 'clarifying',
      analysis: {
        domain: 'general',
        complexity: 2,
        estimatedSteps: 2,
        needsClarification: true,
        reasoning: 'Need more detail.',
        adapterName: 'default',
        suggestedApproach: 'Ask one more question',
      },
      currentQuestion: null,
    });

    usePlanModeStore.setState({
      enterPlanMode,
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);

    await usePlanOrchestratorStore.getState().startPlanWorkflow('draft release plan');

    expect(usePlanOrchestratorStore.getState().phase).toBe('clarification_error');
    expect(usePlanOrchestratorStore.getState().pendingClarifyQuestion).toBeNull();

    const cards = routedCards.map((entry) => entry.payload);
    expect(cards.some((card) => card.cardType === 'plan_clarification_resolution')).toBe(true);
  });

  it('keeps clarification_error after submit failure and only skip proceeds to planning', async () => {
    const submitClarification = vi.fn().mockResolvedValue(null);
    const skipClarification = vi.fn().mockResolvedValue(true);
    const generatePlan = vi.fn().mockResolvedValue(TEST_PLAN);

    usePlanModeStore.setState({
      submitClarification,
      skipClarification,
      generatePlan,
      error: null,
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);

    usePlanOrchestratorStore.setState({
      phase: 'clarifying',
      sessionId: 'plan-session-2',
      pendingClarifyQuestion: {
        questionId: 'q-1',
        question: 'Need scope?',
        hint: null,
        inputType: 'text',
        options: [],
      },
      _runToken: 3,
    } as unknown as ReturnType<typeof usePlanOrchestratorStore.getState>);

    await usePlanOrchestratorStore.getState().submitClarification({
      questionId: 'q-1',
      answer: 'desc',
      skipped: false,
    });

    expect(usePlanOrchestratorStore.getState().phase).toBe('clarification_error');
    expect(generatePlan).not.toHaveBeenCalled();

    await usePlanOrchestratorStore.getState().skipClarification();

    expect(skipClarification).toHaveBeenCalledTimes(1);
    expect(generatePlan).toHaveBeenCalledTimes(1);
    expect(usePlanOrchestratorStore.getState().phase).toBe('reviewing_plan');
  });
});

function createKernelPlanSession() {
  const now = new Date().toISOString();
  return {
    sessionId: 'kernel-plan-1',
    status: 'active' as const,
    activeMode: 'plan' as const,
    modeSnapshots: {
      chat: null,
      task: null,
      plan: {
        phase: 'clarifying',
        planId: null,
        runningStepId: null,
        pendingClarification: null,
        retryableSteps: [],
        planRevision: 0,
        lastEditOperation: null,
      },
    },
    handoffContext: {
      conversationContext: [],
      artifactRefs: [],
      contextSources: ['simple_mode'],
      metadata: {},
    },
    linkedModeSessions: {},
    lastError: null,
    createdAt: now,
    updatedAt: now,
    lastCheckpointId: null,
  };
}
