import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { WorkflowSession } from '../../../types/workflowKernel';
import type { PlanLifecyclePhase } from '../../../types/workflowKernel';

const planOrchestratorHarness = vi.hoisted(() => ({
  state: {
    phase: 'reviewing_plan',
    stepStatuses: {} as Record<string, string>,
    approvePlan: vi.fn().mockResolvedValue({ ok: true }),
    retryStep: vi.fn().mockResolvedValue(undefined),
  },
}));

const kernelHarness = vi.hoisted(() => ({
  session: null as WorkflowSession | null,
  getStateSession: null as WorkflowSession | null,
  refreshSessionState: vi.fn().mockResolvedValue({
    session: {
      modeSnapshots: {
        plan: {
          planRevision: 1,
        },
      },
    },
  }),
}));

const coordinatorHarness = vi.hoisted(() => ({
  applyPlanEditWithIntent: vi.fn().mockResolvedValue({ ok: true, errorCode: null, message: null, session: {} }),
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
  useWorkflowKernelStore: Object.assign(
    (selector: (state: { session: WorkflowSession | null; refreshSessionState: () => Promise<unknown> }) => unknown) =>
      selector({ session: kernelHarness.session, refreshSessionState: kernelHarness.refreshSessionState }),
    {
      getState: () => ({
        session: kernelHarness.getStateSession ?? kernelHarness.session,
        refreshSessionState: kernelHarness.refreshSessionState,
      }),
    },
  ),
}));

vi.mock('../../../store/simpleWorkflowCoordinator', () => ({
  applyPlanEditWithIntent: coordinatorHarness.applyPlanEditWithIntent,
  retryPlanStepViaCoordinator: vi.fn().mockResolvedValue(undefined),
  submitWorkflowActionIntentViaCoordinator: vi.fn().mockResolvedValue(undefined),
}));

import { PlanCard } from './PlanCard';

function createPlanKernelSession(phase: PlanLifecyclePhase): WorkflowSession {
  const now = '2026-03-04T00:00:00Z';
  return {
    sessionId: 'kernel-plan-1',
    sessionKind: 'simple_root',
    displayTitle: 'Kernel plan',
    workspacePath: '/tmp/project',
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
        runId: null,
        backgroundStatus: null,
        resumableFromCheckpoint: false,
        lastCheckpointId: null,
      },
    },
    handoffContext: {
      conversationContext: [],
      artifactRefs: [],
      contextSources: [],
      metadata: {},
    },
    linkedModeSessions: {},
    backgroundState: 'foreground',
    contextLedger: {
      conversationTurnCount: 0,
      artifactRefCount: 0,
      contextSourceKinds: [],
      lastCompactionAt: null,
      ledgerVersion: 1,
    },
    modeRuntimeMeta: {},
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
    kernelHarness.getStateSession = null;
    kernelHarness.refreshSessionState.mockClear();
    kernelHarness.refreshSessionState.mockResolvedValue({
      session: {
        modeSnapshots: {
          plan: {
            planRevision: 1,
          },
        },
      },
    });
    coordinatorHarness.applyPlanEditWithIntent.mockClear();
    coordinatorHarness.applyPlanEditWithIntent.mockResolvedValue({
      ok: true,
      errorCode: null,
      message: null,
      session: {},
    });
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

    const stepToggle = screen.getByRole('button', { name: /Step 1/i });
    await user.click(stepToggle);
    if (!screen.queryByText('plan.editStep')) {
      await user.click(stepToggle);
    }
    await waitFor(() => {
      expect(screen.getByText('plan.editStep')).toBeInTheDocument();
    });
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

  it('shows failed edit status when plan edit intent/apply fails', async () => {
    const user = userEvent.setup();
    kernelHarness.session = createPlanKernelSession('reviewing_plan');
    coordinatorHarness.applyPlanEditWithIntent.mockResolvedValueOnce({
      ok: false,
      errorCode: 'intent_submit_failed',
      message: 'intent failed',
      session: null,
    });

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

    await user.click(screen.getByRole('button', { name: /Step 1/i }));
    await waitFor(() => {
      expect(screen.getByText('plan.editStep')).toBeInTheDocument();
    });
    await user.click(screen.getByText('plan.editStep'));
    await user.clear(screen.getByPlaceholderText('plan.stepTitle'));
    await user.type(screen.getByPlaceholderText('plan.stepTitle'), 'Step 1 updated');
    await user.click(screen.getByText('common:save'));

    await waitFor(() => {
      expect(screen.getAllByText('intent failed').length).toBeGreaterThan(0);
      expect(screen.getByText('plan.editSummary.status.failed')).toBeInTheDocument();
    });
  });

  it('rolls back local optimistic plan edit after apply failure', async () => {
    const user = userEvent.setup();
    kernelHarness.session = createPlanKernelSession('reviewing_plan');
    coordinatorHarness.applyPlanEditWithIntent.mockResolvedValueOnce({
      ok: false,
      errorCode: 'apply_edit_failed',
      message: 'apply failed',
      session: null,
    });

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

    await user.click(screen.getByRole('button', { name: /Step 1/i }));
    await waitFor(() => {
      expect(screen.getByText('plan.editStep')).toBeInTheDocument();
    });
    await user.click(screen.getByText('plan.editStep'));
    await user.clear(screen.getByPlaceholderText('plan.stepTitle'));
    await user.type(screen.getByPlaceholderText('plan.stepTitle'), 'Updated title');
    await user.click(screen.getByText('common:save'));

    await waitFor(() => {
      expect(screen.getByText('Step 1')).toBeInTheDocument();
    });
    expect(screen.queryByText('Updated title')).toBeNull();
  });

  it('retries a failed edit operation from edit summary', async () => {
    const user = userEvent.setup();
    kernelHarness.session = createPlanKernelSession('reviewing_plan');
    coordinatorHarness.applyPlanEditWithIntent
      .mockResolvedValueOnce({
        ok: false,
        errorCode: 'apply_edit_failed',
        message: 'first failure',
        session: null,
      })
      .mockResolvedValueOnce({
        ok: true,
        errorCode: null,
        message: null,
        session: {},
      });

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

    await user.click(screen.getByRole('button', { name: /Step 1/i }));
    await waitFor(() => {
      expect(screen.getByText('plan.editStep')).toBeInTheDocument();
    });
    await user.click(screen.getByText('plan.editStep'));
    await user.clear(screen.getByPlaceholderText('plan.stepTitle'));
    await user.type(screen.getByPlaceholderText('plan.stepTitle'), 'Retry me');
    await user.click(screen.getByText('common:save'));

    await waitFor(() => {
      expect(screen.getByText('plan.editSummary.retry')).toBeInTheDocument();
    });
    await user.click(screen.getByText('plan.editSummary.retry'));

    await waitFor(() => {
      expect(coordinatorHarness.applyPlanEditWithIntent).toHaveBeenCalledTimes(2);
      expect(screen.getByText('plan.editSummary.status.success')).toBeInTheDocument();
    });
  });

  it('detects revision conflict before applying edit and asks user to retry', async () => {
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

    kernelHarness.getStateSession = {
      ...createPlanKernelSession('reviewing_plan'),
      modeSnapshots: {
        chat: null,
        task: null,
        plan: {
          ...createPlanKernelSession('reviewing_plan').modeSnapshots.plan!,
          planRevision: 3,
        },
      },
    };

    const parallelismInput = screen.getByRole('spinbutton');
    await user.clear(parallelismInput);
    await user.type(parallelismInput, '5');
    await user.click(screen.getByText('plan.parallelism.apply'));

    await waitFor(() => {
      expect(screen.getAllByText('plan.editConflictDetected').length).toBeGreaterThan(0);
      expect(coordinatorHarness.applyPlanEditWithIntent).not.toHaveBeenCalled();
    });
  });

  it('keeps successful and failed edit summaries independently for consecutive edits', async () => {
    const user = userEvent.setup();
    kernelHarness.session = createPlanKernelSession('reviewing_plan');
    coordinatorHarness.applyPlanEditWithIntent
      .mockResolvedValueOnce({
        ok: true,
        errorCode: null,
        message: null,
        session: {},
      })
      .mockResolvedValueOnce({
        ok: false,
        errorCode: 'apply_edit_failed',
        message: 'second failed',
        session: null,
      });

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

    const parallelInput = screen.getByDisplayValue('4');
    await user.clear(parallelInput);
    await user.type(parallelInput, '5');
    await user.click(screen.getByText('plan.parallelism.apply'));
    await waitFor(() => {
      expect(screen.getByText('plan.editSummary.status.success')).toBeInTheDocument();
    });

    await user.click(screen.getByRole('button', { name: /Step 1/i }));
    await user.click(screen.getByText('plan.editStep'));
    await user.clear(screen.getByPlaceholderText('plan.stepTitle'));
    await user.type(screen.getByPlaceholderText('plan.stepTitle'), 'second edit');
    await user.click(screen.getByText('common:save'));

    await waitFor(() => {
      expect(screen.getAllByText('second failed').length).toBeGreaterThan(0);
      expect(screen.getByText('plan.editSummary.status.failed')).toBeInTheDocument();
    });
  });

  it('keeps approve button available when approve action fails', async () => {
    const user = userEvent.setup();
    kernelHarness.session = createPlanKernelSession('reviewing_plan');
    planOrchestratorHarness.state.approvePlan.mockResolvedValueOnce({
      ok: false,
      errorCode: 'plan_approval_failed',
      message: 'backend rejected',
    });

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

    const approveButton = screen.getByText('plan.approveAndExecute');
    await user.click(approveButton);

    await waitFor(() => {
      expect(screen.getByText('backend rejected')).toBeInTheDocument();
    });
    expect(approveButton).toBeEnabled();
  });
});
