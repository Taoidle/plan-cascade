import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { WorkflowSession } from '../../../types/workflowKernel';
import type { TaskLifecyclePhase } from '../../../types/workflowKernel';

const orchestratorHarness = vi.hoisted(() => ({
  state: {
    phase: 'architecture_review',
    approveArchitecture: vi.fn().mockResolvedValue({
      ok: false,
      errorCode: 'architecture_apply_failed',
      message: 'cannot apply architecture changes',
    }),
  },
}));

const kernelHarness = vi.hoisted(() => ({
  session: null as WorkflowSession | null,
  taskTranscriptLines: [] as Array<{
    type: string;
    cardPayload?: { interactive?: boolean; cardType?: string; cardId?: string };
  }>,
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
  useWorkflowKernelStore: (
    selector: (state: {
      session: WorkflowSession | null;
      getCachedModeTranscript: (sessionId: string, mode: 'task') => { lines: typeof kernelHarness.taskTranscriptLines };
    }) => unknown,
  ) =>
    selector({
      session: kernelHarness.session,
      getCachedModeTranscript: () => ({ lines: kernelHarness.taskTranscriptLines }),
    }),
}));

vi.mock('../../../store/simpleWorkflowCoordinator', () => ({
  submitWorkflowActionIntentViaCoordinator: vi.fn().mockResolvedValue(undefined),
}));

import { ArchitectureReviewCard } from './ArchitectureReviewCard';

function createTaskKernelSession(phase: TaskLifecyclePhase): WorkflowSession {
  const now = '2026-03-04T00:00:00Z';
  return {
    sessionId: 'kernel-task-1',
    sessionKind: 'simple_root',
    displayTitle: 'Kernel task',
    workspacePath: '/tmp/project',
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

describe('ArchitectureReviewCard', () => {
  beforeEach(() => {
    orchestratorHarness.state.phase = 'architecture_review';
    orchestratorHarness.state.approveArchitecture.mockReset();
    orchestratorHarness.state.approveArchitecture.mockResolvedValue({
      ok: false,
      errorCode: 'architecture_apply_failed',
      message: 'cannot apply architecture changes',
    });
    kernelHarness.session = createTaskKernelSession('architecture_review');
    kernelHarness.taskTranscriptLines = [];
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

    const continueButton = screen.getByText('workflow.architectureReview.continueExecution');
    await user.click(continueButton);

    await waitFor(() => {
      expect(screen.getByText('cannot apply architecture changes')).toBeInTheDocument();
    });
    expect(continueButton).toBeEnabled();

    await user.click(screen.getByText('workflow.architectureReview.continueExecution'));
    expect(orchestratorHarness.state.approveArchitecture).toHaveBeenCalledTimes(2);
  });

  it('shows apply and bypass actions when architecture review has PRD modifications', () => {
    render(
      <ArchitectureReviewCard
        interactive
        data={{
          personaRole: 'SoftwareArchitect',
          analysis: 'analysis',
          concerns: [],
          suggestions: ['one'],
          prdModifications: [
            {
              operationId: 'mod-1',
              type: 'update_story',
              targetStoryId: 'story-1',
              payload: { title: 'Updated story' },
              preview: 'Update story title',
              reason: 'Clarify scope',
              confidence: 0.9,
            },
          ],
          approved: false,
        }}
      />,
    );

    expect(screen.getByText('workflow.architectureReview.applyModifications')).toBeInTheDocument();
    expect(screen.getByText('workflow.architectureReview.bypassAndExecute')).toBeInTheDocument();
    expect(screen.queryByText('workflow.architectureReview.continueExecution')).toBeNull();
  });

  it('still shows architecture actions when kernel session is unavailable but orchestrator is reviewing architecture', () => {
    kernelHarness.session = null;
    orchestratorHarness.state.phase = 'architecture_review';

    render(
      <ArchitectureReviewCard
        interactive
        data={{
          personaRole: 'SoftwareArchitect',
          analysis: 'analysis',
          concerns: [],
          suggestions: ['one'],
          prdModifications: [
            {
              operationId: 'mod-1',
              type: 'update_story',
              targetStoryId: 'story-1',
              payload: { title: 'Updated story' },
              preview: 'Update story title',
              reason: 'Clarify scope',
              confidence: 0.9,
            },
          ],
          approved: false,
        }}
      />,
    );

    expect(screen.getByText('workflow.architectureReview.applyModifications')).toBeInTheDocument();
    expect(screen.getByText('workflow.architectureReview.bypassAndExecute')).toBeInTheDocument();
  });
});
