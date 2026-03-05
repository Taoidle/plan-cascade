import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PlanClarifyQuestionCardData } from '../../types/planModeCard';
import type { InterviewQuestionCardData } from '../../types/workflowCard';
import type { HandoffContextBundle, UserInputIntent, WorkflowSession } from '../../types/workflowKernel';
import { useSimpleInputRouting } from './useSimpleInputRouting';

const coordinatorMocks = vi.hoisted(() => ({
  startModeWithCompensation: vi.fn(),
  submitWorkflowInputWithTracking: vi.fn(),
}));

vi.mock('../../lib/contextBridge', () => ({
  buildConversationHistory: vi.fn(() => []),
}));

vi.mock('../../store/simpleWorkflowCoordinator', () => ({
  startModeWithCompensation: (...args: unknown[]) => coordinatorMocks.startModeWithCompensation(...args),
  submitWorkflowInputWithTracking: (...args: unknown[]) => coordinatorMocks.submitWorkflowInputWithTracking(...args),
}));

function createWorkflowSession(mode: 'chat' | 'task' | 'plan'): WorkflowSession {
  return {
    sessionId: `session-${mode}`,
    status: 'active',
    activeMode: mode,
    modeSnapshots: {
      chat: null,
      task: null,
      plan: null,
    },
    handoffContext: {
      conversationContext: [],
      artifactRefs: [],
      contextSources: [],
      metadata: {},
    },
    linkedModeSessions: {},
    lastError: null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    lastCheckpointId: null,
  };
}

function createBaseParams() {
  return {
    description: 'build task',
    setDescription: vi.fn(),
    workflowMode: 'task' as 'chat' | 'task' | 'plan',
    workflowPhase: 'configuring',
    planPhase: 'idle',
    isSubmitting: false,
    isAnalyzingStrategy: false,
    start: vi.fn().mockResolvedValue(undefined),
    sendFollowUp: vi.fn().mockResolvedValue(undefined),
    startWorkflow: vi.fn().mockResolvedValue({ modeSessionId: 'task-session-1' }),
    startPlanWorkflow: vi.fn().mockResolvedValue({ modeSessionId: 'plan-session-1' }),
    overrideConfigNatural: vi.fn(),
    addPrdFeedback: vi.fn(),
    submitPlanClarification: vi.fn().mockResolvedValue({ ok: true }),
    submitInterviewAnswer: vi.fn().mockResolvedValue(undefined),
    skipInterviewQuestion: vi.fn().mockResolvedValue(undefined),
    skipPlanClarification: vi.fn().mockResolvedValue(undefined),
    taskInterviewingPhase: false,
    taskPendingQuestion: null as InterviewQuestionCardData | null,
    planClarifyingPhase: false,
    planPendingQuestion: null as PlanClarifyQuestionCardData | null,
    hasStructuredInterviewQuestion: false,
    hasStructuredPlanClarifyQuestion: false,
    linkWorkflowKernelModeSession: vi.fn().mockResolvedValue(createWorkflowSession('task')),
    cancelWorkflowKernelOperation: vi.fn().mockResolvedValue(createWorkflowSession('task')),
    transitionAndSubmitWorkflowKernelInput: vi.fn().mockResolvedValue(createWorkflowSession('task')) as unknown as (
      targetMode: 'chat' | 'task' | 'plan',
      intent: UserInputIntent,
      handoff?: HandoffContextBundle,
    ) => Promise<WorkflowSession | null>,
  };
}

describe('useSimpleInputRouting', () => {
  beforeEach(() => {
    coordinatorMocks.startModeWithCompensation.mockReset();
    coordinatorMocks.submitWorkflowInputWithTracking.mockReset();
    coordinatorMocks.startModeWithCompensation.mockResolvedValue({
      ok: true,
      errorCode: null,
      session: createWorkflowSession('task'),
    });
    coordinatorMocks.submitWorkflowInputWithTracking.mockResolvedValue(createWorkflowSession('task'));
  });

  it('aborts task config follow-up when kernel submission fails', async () => {
    const params = createBaseParams();
    coordinatorMocks.submitWorkflowInputWithTracking.mockResolvedValueOnce(null);
    const { result } = renderHook(() => useSimpleInputRouting(params));

    await act(async () => {
      await result.current.handleFollowUp('使用 6 个并行代理并启用 TDD');
    });

    expect(params.overrideConfigNatural).not.toHaveBeenCalled();
    expect(params.sendFollowUp).not.toHaveBeenCalled();
  });

  it('submits plan clarification only after kernel submission succeeds', async () => {
    const params = createBaseParams();
    params.workflowMode = 'plan';
    params.planPhase = 'clarifying';
    params.planClarifyingPhase = true;
    params.planPendingQuestion = {
      questionId: 'clarify-1',
      question: 'Need details',
      hint: null,
      inputType: 'text',
      options: [],
    };

    const { result } = renderHook(() => useSimpleInputRouting(params));

    await act(async () => {
      await result.current.handleFollowUp('补充细节');
    });

    expect(params.submitPlanClarification).toHaveBeenCalledWith({
      questionId: 'clarify-1',
      answer: '补充细节',
      skipped: false,
    });
  });

  it('submits structured plan clarification via dedicated handler', async () => {
    const params = createBaseParams();
    params.workflowMode = 'plan';
    params.planPhase = 'clarifying';
    params.planClarifyingPhase = true;
    params.hasStructuredPlanClarifyQuestion = true;
    params.planPendingQuestion = {
      questionId: 'clarify-structured-1',
      question: 'Choose execution strategy',
      hint: null,
      inputType: 'single_select',
      options: ['strict', 'fast'],
    };

    const { result } = renderHook(() => useSimpleInputRouting(params));

    await act(async () => {
      await result.current.handleStructuredPlanClarifySubmit('strict');
    });

    expect(params.submitPlanClarification).toHaveBeenCalledWith({
      questionId: 'clarify-structured-1',
      answer: 'strict',
      skipped: false,
    });
  });

  it('delegates start action to transactional coordinator with composer source', async () => {
    const params = createBaseParams();
    params.workflowMode = 'task';
    params.description = 'ship workflow';

    const { result } = renderHook(() => useSimpleInputRouting(params));

    await act(async () => {
      await result.current.handleStart();
    });

    expect(params.setDescription).toHaveBeenCalledWith('');
    expect(coordinatorMocks.startModeWithCompensation).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: 'task',
        prompt: 'ship workflow',
        source: 'composer',
        transitionAndSubmitInput: params.transitionAndSubmitWorkflowKernelInput,
        cancelKernelOperation: params.cancelWorkflowKernelOperation,
        startTaskWorkflow: params.startWorkflow,
      }),
    );
  });
});
