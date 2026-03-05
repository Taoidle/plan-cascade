import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { useWorkflowObservabilityStore } from './workflowObservability';

describe('workflowObservability store', () => {
  beforeEach(() => {
    useWorkflowObservabilityStore.getState().reset();
    vi.clearAllMocks();
  });

  it('refreshes snapshot from backend command', async () => {
    mockInvoke.mockResolvedValueOnce({
      success: true,
      error: null,
      data: {
        metrics: {
          workflowLinkRehydrateTotal: 3,
          workflowLinkRehydrateSuccess: 2,
          workflowLinkRehydrateFailure: 1,
          interactiveActionFailTotal: 4,
          prdFeedbackApplyTotal: 5,
          prdFeedbackApplySuccess: 4,
          prdFeedbackApplyFailure: 1,
        },
        interactiveActionFailBreakdown: [],
        latestFailure: null,
      },
    });

    const result = await useWorkflowObservabilityStore.getState().refreshSnapshot();

    expect(result?.metrics.workflowLinkRehydrateTotal).toBe(3);
    expect(mockInvoke).toHaveBeenCalledWith('workflow_get_observability_snapshot');
  });

  it('records interactive action failure and triggers refresh', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        success: true,
        error: null,
        data: true,
      })
      .mockResolvedValueOnce({
        success: true,
        error: null,
        data: {
          metrics: {
            workflowLinkRehydrateTotal: 1,
            workflowLinkRehydrateSuccess: 1,
            workflowLinkRehydrateFailure: 0,
            interactiveActionFailTotal: 1,
            prdFeedbackApplyTotal: 0,
            prdFeedbackApplySuccess: 0,
            prdFeedbackApplyFailure: 0,
          },
          interactiveActionFailBreakdown: [],
          latestFailure: {
            timestamp: '2026-03-05T00:00:00Z',
            action: 'approve_prd',
            card: 'prd_card',
            mode: 'task',
            kernelSessionId: 'kernel-1',
            modeSessionId: 'task-1',
            phaseBefore: 'reviewing_prd',
            phaseAfter: 'reviewing_prd',
            errorCode: 'prd_feedback_apply_failed',
            message: 'failed',
          },
        },
      });

    const ok = await useWorkflowObservabilityStore.getState().recordInteractiveActionFailure({
      card: 'prd_card',
      action: 'approve_prd',
      errorCode: 'prd_feedback_apply_failed',
      mode: 'task',
    });

    expect(ok).toBe(true);
    expect(mockInvoke).toHaveBeenNthCalledWith(1, 'workflow_record_interactive_action_failure', {
      request: {
        card: 'prd_card',
        action: 'approve_prd',
        errorCode: 'prd_feedback_apply_failed',
        message: null,
        mode: 'task',
        kernelSessionId: null,
        modeSessionId: null,
        phaseBefore: null,
        phaseAfter: null,
      },
    });
    expect(mockInvoke).toHaveBeenNthCalledWith(2, 'workflow_get_observability_snapshot');
  });
});
