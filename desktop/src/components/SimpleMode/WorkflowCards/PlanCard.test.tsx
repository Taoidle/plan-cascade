import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { WorkflowSession } from '../../../types/workflowKernel';

const planOrchestratorHarness = vi.hoisted(() => ({
  state: {
    phase: 'reviewing_plan',
    stepStatuses: {} as Record<string, string>,
    approvePlan: vi.fn().mockResolvedValue(undefined),
    retryStep: vi.fn().mockResolvedValue(undefined),
  },
}));

const kernelHarness = vi.hoisted(() => ({
  session: null as WorkflowSession | null,
}));

const i18nHarness = vi.hoisted(() => ({
  t: vi.fn((key: string) => key),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: i18nHarness.t,
  }),
}));

vi.mock('../../../store/planOrchestrator', () => ({
  usePlanOrchestratorStore: (selector: (state: typeof planOrchestratorHarness.state) => unknown) =>
    selector(planOrchestratorHarness.state),
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
    planOrchestratorHarness.state.stepStatuses = {};
    planOrchestratorHarness.state.approvePlan.mockClear();
    planOrchestratorHarness.state.retryStep.mockClear();
    kernelHarness.session = null;
    i18nHarness.t.mockClear();
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

  it('renders plan operation controls and validation gate from i18n keys', () => {
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
          executionConfig: { maxParallel: 4 },
          steps: [
            {
              id: 'step-1',
              title: 'Step 1',
              description: 'desc',
              priority: 'medium',
              dependencies: ['step-2'],
              completionCriteria: ['done'],
              expectedOutput: 'result',
            },
            {
              id: 'step-2',
              title: 'Step 2',
              description: 'desc',
              priority: 'medium',
              dependencies: ['step-1'],
              completionCriteria: ['done'],
              expectedOutput: 'result',
            },
          ],
          batches: [{ index: 0, stepIds: ['step-1', 'step-2'] }],
        }}
      />,
    );

    expect(screen.getByText('plan.addStep.action')).toBeInTheDocument();
    expect(screen.getByText('plan.validation.blockTitle')).toBeInTheDocument();
    expect(i18nHarness.t).toHaveBeenCalledWith(
      'plan.validation.blockTitle',
      expect.objectContaining({ defaultValue: 'Execution blocked by plan validation' }),
    );
  });

  it('uses i18n keys for edit-summary details when editing and reordering steps', async () => {
    const user = userEvent.setup();
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
          executionConfig: { maxParallel: 4 },
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
            {
              id: 'step-2',
              title: 'Step 2',
              description: 'desc 2',
              priority: 'medium',
              dependencies: [],
              completionCriteria: ['done'],
              expectedOutput: 'result',
            },
          ],
          batches: [{ index: 0, stepIds: ['step-1', 'step-2'] }],
        }}
      />,
    );

    await user.click(screen.getByRole('button', { name: /Step 1/i }));
    await user.click(screen.getByText('plan.editStep'));
    await user.clear(screen.getByPlaceholderText('plan.stepTitle'));
    await user.type(screen.getByPlaceholderText('plan.stepTitle'), 'Step 1 updated');
    await user.click(screen.getByText('common:save'));

    await waitFor(() => {
      expect(screen.getByText(/plan\.editSummary\.field\.title/)).toBeInTheDocument();
    });

    await user.click(screen.getByRole('button', { name: /Step 2/i }));
    const moveUpButtons = screen.getAllByRole('button', { name: 'plan.reorder.up' });
    const enabledMoveUpButton = moveUpButtons.find((button) => !button.hasAttribute('disabled')) ?? moveUpButtons[0];
    expect(enabledMoveUpButton).toBeTruthy();
    await user.click(enabledMoveUpButton as HTMLButtonElement);

    await waitFor(() => {
      expect(screen.getAllByText(/plan\.reorder\.up/).length).toBeGreaterThan(0);
    });
  });

  it('routes failed-step retry button to planOrchestrator.retryStep', async () => {
    const user = userEvent.setup();
    planOrchestratorHarness.state.stepStatuses = { 'step-1': 'failed' };
    kernelHarness.session = createPlanKernelSession('failed');

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

    await user.click(screen.getByRole('button', { name: 'plan.retryStep' }));

    await waitFor(() => {
      expect(planOrchestratorHarness.state.retryStep).toHaveBeenCalledWith('step-1');
    });
  });
});
