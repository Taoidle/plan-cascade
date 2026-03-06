import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { WorkflowSession } from '../../../types/workflowKernel';
import type { TaskLifecyclePhase } from '../../../types/workflowKernel';

const orchestratorHarness = vi.hoisted(() => ({
  state: {
    updateConfig: vi.fn(),
    confirmConfig: vi.fn().mockResolvedValue({
      ok: false,
      errorCode: 'config_confirm_failed',
      message: 'config failed',
    }),
    overrideConfigNatural: vi.fn(),
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

import { ConfigCard } from './ConfigCard';

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

describe('ConfigCard', () => {
  beforeEach(() => {
    orchestratorHarness.state.updateConfig.mockReset();
    orchestratorHarness.state.confirmConfig.mockReset();
    orchestratorHarness.state.confirmConfig.mockResolvedValue({
      ok: false,
      errorCode: 'config_confirm_failed',
      message: 'config failed',
    });
    orchestratorHarness.state.overrideConfigNatural.mockReset();
    kernelHarness.session = createTaskKernelSession('configuring');
  });

  it('keeps continue action retryable when config confirm fails', async () => {
    const user = userEvent.setup();

    render(
      <ConfigCard
        interactive
        data={{
          flowLevel: 'standard',
          tddMode: 'off',
          maxParallel: 4,
          qualityGatesEnabled: true,
          specInterviewEnabled: false,
          isOverridden: false,
        }}
      />,
    );

    const continueButton = screen.getByText('workflow.config.continue');
    await user.click(continueButton);

    await waitFor(() => {
      expect(screen.getByText('config failed')).toBeInTheDocument();
    });
    expect(continueButton).toBeEnabled();

    await user.click(continueButton);
    expect(orchestratorHarness.state.confirmConfig).toHaveBeenCalledTimes(2);
  });
});
