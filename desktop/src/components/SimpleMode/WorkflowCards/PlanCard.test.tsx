import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { WorkflowSession } from '../../../types/workflowKernel';

const planOrchestratorHarness = vi.hoisted(() => ({
  state: {
    phase: 'reviewing_plan',
    approvePlan: vi.fn().mockResolvedValue(undefined),
  },
}));

const planModeHarness = vi.hoisted(() => ({
  state: {
    stepStatuses: {} as Record<string, string>,
  },
}));

const kernelHarness = vi.hoisted(() => ({
  session: null as WorkflowSession | null,
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue || key,
  }),
}));

vi.mock('../../../store/planOrchestrator', () => ({
  usePlanOrchestratorStore: (selector: (state: typeof planOrchestratorHarness.state) => unknown) =>
    selector(planOrchestratorHarness.state),
}));

vi.mock('../../../store/planMode', () => ({
  usePlanModeStore: (selector: (state: typeof planModeHarness.state) => unknown) => selector(planModeHarness.state),
}));

vi.mock('../../../store/workflowKernel', () => ({
  useWorkflowKernelStore: (selector: (state: { session: WorkflowSession | null }) => unknown) =>
    selector({ session: kernelHarness.session }),
}));

vi.mock('../../../store/simpleWorkflowCoordinator', () => ({
  applyPlanEditViaCoordinator: vi.fn().mockResolvedValue(undefined),
  executePlanViaCoordinator: vi.fn().mockResolvedValue(undefined),
  retryPlanStepViaCoordinator: vi.fn().mockResolvedValue(undefined),
  submitWorkflowActionIntentViaCoordinator: vi.fn().mockResolvedValue(undefined),
}));

import { PlanCard } from './PlanCard';

function createPlanKernelSession(phase: string): WorkflowSession {
  const now = '2026-03-04T00:00:00Z';
  return {
    sessionId: 'kernel-plan-1',
    status: 'active',
    activeMode: 'plan',
    modeSnapshots: {
      chat: null,
      task: null,
      plan: {
        phase,
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
      contextSources: [],
      metadata: {},
    },
    linkedModeSessions: {},
    lastError: null,
    createdAt: now,
    updatedAt: now,
    lastCheckpointId: null,
  };
}

describe('PlanCard interactive gating', () => {
  beforeEach(() => {
    planOrchestratorHarness.state.phase = 'reviewing_plan';
    planModeHarness.state.stepStatuses = {};
    kernelHarness.session = null;
  });

  it('enables actions when kernel phase is reviewing_plan', () => {
    kernelHarness.session = createPlanKernelSession('reviewing_plan');

    render(
      <PlanCard
        interactive
        data={{
          title: 'Plan',
          description: 'desc',
          domain: 'general',
          adapterName: 'default',
          editable: true,
          steps: [
            {
              id: 'step-1',
              title: 'Step 1',
              description: 'desc',
              priority: 'medium',
              dependencies: [],
              completionCriteria: ['done'],
              expectedOutput: 'result',
            },
          ],
          batches: [{ index: 0, stepIds: ['step-1'] }],
        }}
      />,
    );

    expect(screen.getByText('plan.approveAndExecute')).toBeInTheDocument();
  });

  it('disables actions when kernel phase is not reviewing_plan even if orchestrator phase says reviewing_plan', () => {
    planOrchestratorHarness.state.phase = 'reviewing_plan';
    kernelHarness.session = createPlanKernelSession('executing');

    render(
      <PlanCard
        interactive
        data={{
          title: 'Plan',
          description: 'desc',
          domain: 'general',
          adapterName: 'default',
          editable: true,
          steps: [
            {
              id: 'step-1',
              title: 'Step 1',
              description: 'desc',
              priority: 'medium',
              dependencies: [],
              completionCriteria: ['done'],
              expectedOutput: 'result',
            },
          ],
          batches: [{ index: 0, stepIds: ['step-1'] }],
        }}
      />,
    );

    expect(screen.queryByText('plan.approveAndExecute')).toBeNull();
  });
});
