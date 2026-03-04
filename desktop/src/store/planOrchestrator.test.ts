import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PlanCardData, PlanModeProgressPayload } from '../types/planModeCard';

const listenMock = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

import { useExecutionStore } from './execution';
import { usePlanModeStore } from './planMode';
import { usePlanOrchestratorStore } from './planOrchestrator';

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
      isPlanMode: true,
      sessionId: 'session-1',
      sessionPhase: 'reviewing_plan',
      currentBatch: 0,
      totalBatches: 0,
      stepStatuses: {},
      report: null,
      error: null,
      approvePlan: vi.fn().mockResolvedValue(undefined),
      fetchStepOutput: vi.fn().mockResolvedValue(null),
      fetchReport: vi.fn().mockResolvedValue(undefined),
    } as unknown as ReturnType<typeof usePlanModeStore.getState>);

    usePlanOrchestratorStore.setState({
      phase: 'reviewing_plan',
      sessionId: 'session-1',
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
    expect(planState.sessionPhase).toBe('cancelled');
    expect(planState.isCancelling).toBe(false);
  });
});
