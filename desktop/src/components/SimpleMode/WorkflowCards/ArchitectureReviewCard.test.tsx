import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { WorkflowSession } from '../../../types/workflowKernel';
import type { TaskLifecyclePhase } from '../../../types/workflowKernel';

const orchestratorHarness = vi.hoisted(() => ({
  state: {
    approveArchitecture: vi.fn().mockResolvedValue({
      ok: false,
      errorCode: 'architecture_apply_failed',
      message: 'cannot apply architecture changes',
    }),
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
    t: (key: string) => key,
  }),
}));

vi.mock('../../../store/workflowOrchestrator', () => ({
  useWorkflowOrchestratorStore: (selector: (state: typeof orchestratorHarness.state) => unknown) =>
    selector(orchestratorHarness.state),
}));

vi.mock('../../../store/workflowKernel', () => ({
  useWorkflowKernelStore: (selector: (state: { session: WorkflowSession | null }) => unknown) =>
    selector({ session: kernelHarness.session }),
}));

vi.mock('../../../store/simpleWorkflowCoordinator', () => ({
  submitWorkflowActionIntentViaCoordinator: vi.fn().mockResolvedValue(undefined),
}));

import { ArchitectureReviewCard } from './ArchitectureReviewCard';

function createTaskKernelSession(phase: TaskLifecyclePhase): WorkflowSession {
  const now = '2026-03-04T00:00:00Z';
  return {
    sessionId: 'kernel-task-1',
    status: 'active',
    activeMode: 'task',
    modeSnapshots: {
      chat: null,
      plan: null,
      task: {
        phase,
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

describe('ArchitectureReviewCard', () => {
  beforeEach(() => {
    orchestratorHarness.state.approveArchitecture.mockReset();
    orchestratorHarness.state.approveArchitecture.mockResolvedValue({
      ok: false,
      errorCode: 'architecture_apply_failed',
      message: 'cannot apply architecture changes',
    });
    kernelHarness.session = createTaskKernelSession('architecture_review');
  });

  it('keeps architecture actions retryable when backend action fails', async () => {
    const user = userEvent.setup();
    render(
      <ArchitectureReviewCard
        interactive
        data={{
          personaRole: 'SoftwareArchitect',
          analysis: 'analysis',
          concerns: [],
          suggestions: ['one'],
          prdModifications: [],
          approved: false,
        }}
      />,
    );

    const approveButton = screen.getByText('workflow.architectureReview.approve');
    await user.click(approveButton);

    await waitFor(() => {
      expect(screen.getByText('cannot apply architecture changes')).toBeInTheDocument();
    });
    expect(approveButton).toBeEnabled();

    await user.click(screen.getByText('workflow.architectureReview.approve'));
    expect(orchestratorHarness.state.approveArchitecture).toHaveBeenCalledTimes(2);
  });
});
