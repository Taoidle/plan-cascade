import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { WorkflowSession } from '../../../types/workflowKernel';

const orchestratorHarness = vi.hoisted(() => ({
  state: {
    phase: 'reviewing_prd',
    approvePrd: vi.fn().mockResolvedValue({ ok: true }),
    editablePrd: null,
    updateEditableStory: vi.fn(),
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

import { PrdCard } from './PrdCard';

function createTaskKernelSession(phase: string): WorkflowSession {
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

describe('PrdCard interactive gating', () => {
  beforeEach(() => {
    orchestratorHarness.state.phase = 'reviewing_prd';
    kernelHarness.session = null;
  });

  it('enables actions when kernel phase is reviewing_prd', () => {
    kernelHarness.session = createTaskKernelSession('reviewing_prd');

    render(
      <PrdCard
        interactive
        data={{
          title: 'PRD',
          description: 'desc',
          stories: [
            {
              id: 'story-1',
              title: 'Story 1',
              description: 'desc',
              priority: 'high',
              dependencies: [],
              acceptanceCriteria: ['done'],
            },
          ],
          batches: [{ index: 0, storyIds: ['story-1'] }],
          isEditable: true,
        }}
      />,
    );

    expect(screen.getByText('workflow.prd.approveAndExecute')).toBeInTheDocument();
  });

  it('disables actions when kernel phase is not reviewing_prd even if orchestrator phase says reviewing_prd', () => {
    orchestratorHarness.state.phase = 'reviewing_prd';
    kernelHarness.session = createTaskKernelSession('executing');

    render(
      <PrdCard
        interactive
        data={{
          title: 'PRD',
          description: 'desc',
          stories: [
            {
              id: 'story-1',
              title: 'Story 1',
              description: 'desc',
              priority: 'high',
              dependencies: [],
              acceptanceCriteria: ['done'],
            },
          ],
          batches: [{ index: 0, storyIds: ['story-1'] }],
          isEditable: true,
        }}
      />,
    );

    expect(screen.queryByText('workflow.prd.approveAndExecute')).toBeNull();
  });

  it('keeps approve action retryable when approval fails', async () => {
    const user = userEvent.setup();
    kernelHarness.session = createTaskKernelSession('reviewing_prd');
    orchestratorHarness.state.approvePrd.mockResolvedValueOnce({
      ok: false,
      errorCode: 'execution_start_failed',
      message: 'cannot execute',
    });

    render(
      <PrdCard
        interactive
        data={{
          title: 'PRD',
          description: 'desc',
          stories: [
            {
              id: 'story-1',
              title: 'Story 1',
              description: 'desc',
              priority: 'high',
              dependencies: [],
              acceptanceCriteria: ['done'],
            },
          ],
          batches: [{ index: 0, storyIds: ['story-1'] }],
          isEditable: true,
        }}
      />,
    );

    const approveButton = screen.getByText('workflow.prd.approveAndExecute');
    await user.click(approveButton);

    await waitFor(() => {
      expect(screen.getByText('cannot execute')).toBeInTheDocument();
    });
    expect(approveButton).toBeEnabled();
  });
});
