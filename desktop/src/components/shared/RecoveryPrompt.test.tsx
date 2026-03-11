import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { RecoveryPrompt } from './RecoveryPrompt';
import type { IncompleteTask } from '../../store/recovery';
import type { WorkflowSessionCatalogItem, WorkflowSessionState } from '../../types/workflowKernel';

const mockResumeTask = vi.fn();
const mockDiscardTask = vi.fn();
const mockDismissPrompt = vi.fn();
const mockClearError = vi.fn();
const mockRecoverSession = vi.fn(async (_sessionId: string) => null as WorkflowSessionState | null);
const mockSetWorkspacePath = vi.fn();

let mockIncompleteTasks: IncompleteTask[] = [];
let mockShowPrompt = false;
let mockWorkflowSessions: WorkflowSessionCatalogItem[] = [];

function translate(key: string, options?: Record<string, unknown>) {
  const translations: Record<string, string> = {
    'common:time.unknown': 'Unknown',
    'common:time.justNow': 'Just now',
    'common:time.minutesAgo': `${options?.count ?? 0}m ago`,
    'common:time.hoursAgo': `${options?.count ?? 0}h ago`,
    'common:time.daysAgo': `${options?.count ?? 0}d ago`,
    'common:buttons.resume': 'Resume',
    'common:status.failed': 'Failed',
    'common:recoveryPrompt.title': 'Interrupted Recovery Items Detected',
    'common:recoveryPrompt.fallbackTaskTitle': 'Untitled execution',
    'common:recoveryPrompt.fallbackDebugTitle': 'Interrupted debug case',
    'common:recoveryPrompt.checkpoints': `${options?.count ?? 0} checkpoints`,
    'common:recoveryPrompt.resuming': 'Resuming...',
    'common:recoveryPrompt.recovering': 'Recovering...',
    'common:recoveryPrompt.discard': 'Discard',
    'common:recoveryPrompt.confirmDiscard': 'Confirm Discard',
    'common:recoveryPrompt.dismiss': 'Dismiss',
    'common:recoveryPrompt.recoverDebugCase': 'Recover Debug Case',
    'common:recoveryPrompt.interrupted': 'Interrupted',
    'common:recoveryPrompt.debugDescription': 'This debug session was interrupted and can be recovered.',
    'common:recoveryPrompt.footer':
      'These tasks or debug sessions were interrupted during a previous session. You can resume them from their last checkpoint or dismiss them.',
    'simpleMode:sidebar.noWorkspace': 'No Workspace',
    'simpleMode:sidebar.debug.environment.staging': 'Staging',
    'simpleMode:sidebar.debug.severity.high': 'High',
    'simpleMode:sidebar.debug.phase.verifying': 'Verifying',
    'simpleMode:sidebar.debug.rootCauseReady': 'Root Cause',
    'simpleMode:sidebar.debug.verified': 'Verified',
    'debugMode:modeLabel': 'Debug',
  };
  return translations[key] ?? key;
}

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: translate,
  }),
}));

vi.mock('../../i18n', () => ({
  default: {
    t: translate,
  },
}));

vi.mock('../../store/recovery', () => ({
  EXECUTION_MODE_LABELS: {
    direct: 'Direct',
    hybrid_auto: 'Hybrid Auto',
    hybrid_worktree: 'Hybrid Worktree',
    mega_plan: 'Mega Plan',
  },
  useRecoveryStore: () => ({
    incompleteTasks: mockIncompleteTasks,
    isResuming: false,
    resumingTaskId: null,
    showPrompt: mockShowPrompt,
    error: null,
    resumeTask: mockResumeTask,
    discardTask: mockDiscardTask,
    dismissPrompt: mockDismissPrompt,
    clearError: mockClearError,
  }),
}));

vi.mock('../../store/workflowKernel', () => ({
  useWorkflowKernelStore: (
    selector: (state: {
      sessionCatalog: WorkflowSessionCatalogItem[];
      recoverSession: typeof mockRecoverSession;
    }) => unknown,
  ) =>
    selector({
      sessionCatalog: mockWorkflowSessions,
      recoverSession: mockRecoverSession,
    }),
}));

vi.mock('../../store/settings', () => ({
  useSettingsStore: (selector: (state: { setWorkspacePath: typeof mockSetWorkspacePath }) => unknown) =>
    selector({
      setWorkspacePath: mockSetWorkspacePath,
    }),
}));

function createDebugWorkflowSession(): WorkflowSessionCatalogItem {
  return {
    sessionId: 'debug-session-1',
    sessionKind: 'simple_root',
    displayTitle: 'Checkout page failing',
    workspacePath: '/repo/app',
    activeMode: 'debug',
    status: 'failed',
    backgroundState: 'interrupted',
    updatedAt: new Date('2026-03-11T10:00:00Z').toISOString(),
    createdAt: new Date('2026-03-11T09:00:00Z').toISOString(),
    lastError: '500 from checkout API',
    contextLedger: {
      conversationTurnCount: 0,
      artifactRefCount: 0,
      contextSourceKinds: [],
      lastCompactionAt: null,
      ledgerVersion: 1,
    },
    modeSnapshots: {
      chat: null,
      plan: null,
      task: null,
      debug: {
        caseId: 'case-1',
        phase: 'verifying',
        severity: 'high',
        environment: 'staging',
        symptomSummary: 'Checkout fails after clicking pay.',
        title: 'Checkout page failing',
        expectedBehavior: null,
        actualBehavior: null,
        reproSteps: [],
        affectedSurface: [],
        recentChanges: null,
        targetUrlOrEntry: 'https://example.test/checkout',
        evidenceRefs: [],
        activeHypotheses: [],
        selectedRootCause: {
          conclusion: 'Checkout requests use a stale API route.',
          supportingEvidenceIds: [],
          contradictions: [],
          confidence: 0.92,
          impactScope: ['checkout'],
          recommendedDirection: 'Update frontend API config',
        },
        fixProposal: {
          summary: 'Point checkout client to the new staging endpoint.',
          changeScope: ['frontend'],
          riskLevel: 'medium',
          filesOrSystemsTouched: ['src/api/checkout.ts'],
          manualApprovalsRequired: [],
          verificationPlan: ['Retry checkout'],
          patchPreviewRef: 'patch:case-1',
        },
        pendingApproval: null,
        verificationReport: {
          summary: 'Checkout flow passed after updating the endpoint.',
          checks: [],
          residualRisks: [],
          artifacts: ['patch:case-1'],
        },
        pendingPrompt: null,
        capabilityProfile: 'staging_limited',
        toolBlockReason: null,
      },
    },
    modeRuntimeMeta: {
      debug: {
        mode: 'debug',
        runId: 'run-1',
        bindingSessionId: null,
        isForeground: true,
        isBackgroundRunning: false,
        isInterrupted: true,
        resumePolicy: 'checkpoint',
        lastHeartbeatAt: null,
        lastCheckpointId: 'checkpoint-1',
        lastError: '500 from checkout API',
      },
    },
  };
}

describe('RecoveryPrompt', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockIncompleteTasks = [];
    mockShowPrompt = false;
    mockWorkflowSessions = [];
    mockRecoverSession.mockResolvedValue(null);
  });

  it('renders localized debug recovery metadata', () => {
    mockWorkflowSessions = [createDebugWorkflowSession()];

    render(<RecoveryPrompt />);

    expect(screen.getByText('Interrupted Recovery Items Detected')).toBeInTheDocument();
    expect(screen.getByText('Checkout page failing')).toBeInTheDocument();
    expect(screen.getByText('Verifying')).toBeInTheDocument();
    expect(screen.getByText('Staging')).toBeInTheDocument();
    expect(screen.getByText('High')).toBeInTheDocument();
    expect(screen.getByText('Root Cause')).toBeInTheDocument();
    expect(screen.getByText('Verified')).toBeInTheDocument();
    expect(screen.getByText('Checkout flow passed after updating the endpoint.')).toBeInTheDocument();
  });

  it('recovers a debug workflow session and updates workspace path', async () => {
    mockWorkflowSessions = [createDebugWorkflowSession()];
    mockRecoverSession.mockResolvedValue({
      session: {
        sessionId: 'debug-session-1',
      },
      events: [],
      checkpoints: [],
    } as unknown as WorkflowSessionState);

    render(<RecoveryPrompt />);

    fireEvent.click(screen.getByText('Recover Debug Case'));

    await waitFor(() => {
      expect(mockRecoverSession).toHaveBeenCalledWith('debug-session-1');
    });
    expect(mockSetWorkspacePath).toHaveBeenCalledWith('/repo/app');
    expect(mockClearError).toHaveBeenCalled();
  });
});
