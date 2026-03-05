import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PlanCardData, PlanModeProgressPayload } from '../types/planModeCard';

const listenMock = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
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
  return useExecutionStore
    .getState()
    .streamingOutput.filter((line) => line.type === 'card')
    .map((line) => JSON.parse(line.content) as { cardType?: string; data?: Record<string, unknown> })
    .filter((card) => card.cardType === 'plan_completion_card');
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

    useExecutionStore.getState().reset();
    usePlanModeStore.getState().reset();
    usePlanOrchestratorStore.getState().resetWorkflow();

    usePlanModeStore.setState({
      sessionId: 'session-1',
      currentBatch: 0,
      totalBatches: 0,
      stepStatuses: {},
      report: null,
      error: null,
      approvePlan: vi.fn().mockResolvedValue(undefined),
      retryPlanStep: vi.fn().mockResolvedValue(undefined),
      fetchStepOutput: vi.fn().mockResolvedValue(null),
      fetchReport: vi.fn().mockResolvedValue(undefined),
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

    expect(usePlanModeStore.getState().stepStatuses).toEqual({});
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

    expect(orchestratorState.phase).toBe('cancelled');
    expect(orchestratorState.isBusy).toBe(false);
    expect(orchestratorState.isCancelling).toBe(false);
    expect(planState.isCancelling).toBe(false);
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
    const planState = usePlanModeStore.getState();
    const completionCards = extractPlanCompletionCards();

    expect(orchestratorState.phase).toBe('completed');
    expect(planState.stepStatuses['step-1']).toBe('completed');
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
    const planState = usePlanModeStore.getState();
    const completionCards = extractPlanCompletionCards();

    expect(orchestratorState.phase).toBe('failed');
    expect(planState.stepStatuses['step-1']).toBe('failed');
    expect(completionCards).toHaveLength(1);
    expect(completionCards[0]?.data?.success).toBe(false);
  });

  it('injects at most one plan completion card for the same run token', async () => {
    usePlanModeStore.setState({
      fetchReport: vi.fn().mockResolvedValue(undefined),
      report: {
        sessionId: 'session-1',
        planTitle: 'Test Plan',
        success: true,
        totalSteps: 1,
        stepsCompleted: 1,
        stepsFailed: 0,
        totalDurationMs: 1000,
        stepSummaries: { 'step-1': 'done' },
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

  it('retries a single step and converges to completed after execution_completed', async () => {
    const retryPlanStep = vi.fn().mockResolvedValue(undefined);
    usePlanModeStore.setState({
      retryPlanStep,
      fetchReport: vi.fn().mockResolvedValue(undefined),
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
      undefined,
      undefined,
      undefined,
      undefined,
      expect.anything(),
      undefined,
      expect.any(String),
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

    expect(usePlanModeStore.getState().stepStatuses['step-1']).toBe('completed');
    expect(usePlanOrchestratorStore.getState().phase).toBe('completed');
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

    expect(usePlanModeStore.getState().stepStatuses['step-1']).toBe('failed');
    expect(usePlanOrchestratorStore.getState().phase).toBe('failed');
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
    const transitionAndSubmitInput = vi.fn().mockResolvedValue(createKernelPlanSession());
    useWorkflowKernelStore.setState({
      sessionId: 'kernel-plan-1',
      session: createKernelPlanSession(),
      transitionAndSubmitInput,
    } as unknown as ReturnType<typeof useWorkflowKernelStore.getState>);

    const enterPlanMode = vi.fn().mockImplementation(async () => {
      usePlanModeStore.setState({
        sessionId: 'plan-session-1',
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
        error: null,
      } as unknown as ReturnType<typeof usePlanModeStore.getState>);
    });

    usePlanModeStore.setState({
      enterPlanMode,
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);

    await usePlanOrchestratorStore.getState().startPlanWorkflow('draft release plan');

    expect(usePlanOrchestratorStore.getState().phase).toBe('clarification_error');
    expect(usePlanOrchestratorStore.getState().pendingClarifyQuestion).toBeNull();
    expect(transitionAndSubmitInput).toHaveBeenCalledWith(
      'plan',
      expect.objectContaining({
        type: 'system_phase_update',
        metadata: expect.objectContaining({
          phase: 'clarification_error',
          reasonCode: 'clarification_question_missing',
        }),
      }),
    );

    const cards = useExecutionStore
      .getState()
      .streamingOutput.filter((line) => line.type === 'card')
      .map((line) => JSON.parse(line.content) as { cardType?: string });
    expect(cards.some((card) => card.cardType === 'plan_clarification_resolution')).toBe(true);
  });

  it('keeps clarification_error after submit failure and only skip proceeds to planning', async () => {
    const submitClarification = vi.fn().mockResolvedValue(null);
    const skipClarification = vi.fn().mockResolvedValue(undefined);
    const generatePlan = vi.fn().mockImplementation(async () => {
      usePlanModeStore.setState({
        plan: TEST_PLAN,
        error: null,
      } as unknown as ReturnType<typeof usePlanModeStore.getState>);
    });

    usePlanModeStore.setState({
      submitClarification,
      skipClarification,
      generatePlan,
      sessionId: 'plan-session-2',
      plan: null,
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
        pendingQuestion: null,
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
