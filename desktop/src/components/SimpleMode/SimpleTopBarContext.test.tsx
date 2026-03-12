import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SessionRuntimeCapsule, SimpleTopBarContext, buildDefaultWorktreeBranchName } from './SimpleTopBarContext';
import type { WorkflowSession } from '../../types/workflowKernel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue || key,
  }),
}));

const mockShowToast = vi.fn();
vi.mock('../shared/Toast', () => ({
  useToast: () => ({ showToast: mockShowToast }),
}));

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/plugin-shell', () => ({
  Command: {
    create: vi.fn(() => ({
      execute: vi.fn().mockResolvedValue(undefined),
    })),
  },
}));

const mockWorkflowStore = {
  session: null as WorkflowSession | null,
  renameSession: vi.fn(),
  moveSessionToManagedWorktree: vi.fn(),
  detachSessionWorktree: vi.fn(),
  cleanupSessionWorktree: vi.fn(),
  prepareSessionPr: vi.fn(),
  createSessionPr: vi.fn(),
  listRepoWorktrees: vi.fn().mockResolvedValue([]),
};

vi.mock('../../store/workflowKernel', () => ({
  useWorkflowKernelStore: (selector?: (state: typeof mockWorkflowStore) => unknown) =>
    typeof selector === 'function' ? selector(mockWorkflowStore) : mockWorkflowStore,
}));

const mockSettingsStore = {
  workspacePath: '/repo/app',
};

vi.mock('../../store/settings', () => ({
  useSettingsStore: (selector?: (state: typeof mockSettingsStore) => unknown) =>
    typeof selector === 'function' ? selector(mockSettingsStore) : mockSettingsStore,
}));

function createSession(overrides: Partial<WorkflowSession> = {}): WorkflowSession {
  return {
    sessionId: overrides.sessionId ?? 'session-12345678',
    sessionKind: 'simple_root',
    displayTitle: overrides.displayTitle ?? 'Desktop Desktop',
    runtime: overrides.runtime ?? {
      rootPath: '/repo/app',
      runtimePath: '/repo/app',
      runtimeKind: 'main',
      displayLabel: null,
      branch: 'main',
      targetBranch: 'main',
      managedWorktreeId: null,
      legacy: false,
      runtimeStatus: null,
      prStatus: null,
    },
    workspacePath: overrides.workspacePath ?? '/repo/app',
    status: overrides.status ?? 'active',
    activeMode: overrides.activeMode ?? 'chat',
    modeSnapshots: overrides.modeSnapshots ?? {
      chat: {
        phase: 'ready',
        pendingInput: '',
        activeTurnId: null,
        turnCount: 0,
        lastUserMessage: null,
        lastAssistantMessage: null,
      },
      plan: {
        phase: 'idle',
        planId: null,
        runningStepId: null,
        pendingClarification: null,
        retryableSteps: [],
        planRevision: 0,
        lastEditOperation: null,
      },
      task: {
        phase: 'idle',
        prdId: null,
        currentStoryId: null,
        interviewSessionId: null,
        pendingInterview: null,
        completedStories: 0,
        failedStories: 0,
      },
      debug: null,
    },
    handoffContext: overrides.handoffContext ?? {
      conversationContext: [],
      summaryItems: [],
      artifactRefs: [],
      contextSources: [],
      metadata: {},
    },
    linkedModeSessions: overrides.linkedModeSessions ?? {},
    backgroundState: overrides.backgroundState ?? 'foreground',
    contextLedger: overrides.contextLedger ?? {
      conversationTurnCount: 0,
      artifactRefCount: 0,
      contextSourceKinds: [],
      lastCompactionAt: null,
      ledgerVersion: 1,
    },
    modeRuntimeMeta: overrides.modeRuntimeMeta ?? {},
    lastError: overrides.lastError ?? null,
    createdAt: overrides.createdAt ?? new Date().toISOString(),
    updatedAt: overrides.updatedAt ?? new Date().toISOString(),
    lastCheckpointId: overrides.lastCheckpointId ?? null,
  };
}

describe('SimpleTopBarContext', () => {
  beforeEach(() => {
    mockShowToast.mockReset();
    mockWorkflowStore.session = null;
    mockWorkflowStore.renameSession = vi.fn().mockResolvedValue(null);
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue({
      success: true,
      data: [
        { name: 'master', is_head: true },
        { name: 'develop', is_head: false },
      ],
    });
  });

  it('renders empty state when there is no active session', () => {
    render(<SimpleTopBarContext />);
    expect(screen.getByText('No active session')).toBeInTheDocument();
  });

  it('renders current session context details', async () => {
    mockWorkflowStore.session = createSession();
    render(<SimpleTopBarContext />);
    expect(screen.getByText('Desktop Desktop')).toBeInTheDocument();
    expect(screen.getByText('Main')).toBeInTheDocument();
    expect(await screen.findByText('master')).toBeInTheDocument();
  });

  it('allows renaming the session title by double-clicking it', async () => {
    mockWorkflowStore.session = createSession();
    const renameSession = vi.fn().mockResolvedValue({
      ...mockWorkflowStore.session,
      displayTitle: 'Renamed session',
    });
    mockWorkflowStore.renameSession = renameSession;

    render(<SimpleTopBarContext />);

    fireEvent.doubleClick(screen.getByRole('button', { name: /Session title\. Double-click to rename\./i }));

    const input = screen.getByDisplayValue('Desktop Desktop');
    fireEvent.change(input, { target: { value: 'Renamed session' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => {
      expect(renameSession).toHaveBeenCalledWith('session-12345678', 'Renamed session');
    });
  });
});

describe('SessionRuntimeCapsule', () => {
  beforeEach(() => {
    mockWorkflowStore.session = createSession();
  });

  it('shows move to worktree button for main runtime', () => {
    render(<SessionRuntimeCapsule />);
    expect(screen.getByRole('button', { name: /Move to Worktree/i })).toBeInTheDocument();
  });

  it('shows worktree capsule for managed runtime', () => {
    mockWorkflowStore.session = createSession({
      runtime: {
        rootPath: '/repo/app',
        runtimePath: '/Users/test/.plan-cascade/worktrees/repo-id/session-123',
        runtimeKind: 'managed_worktree',
        displayLabel: 'Desktop runtime',
        branch: 'pc/desktop-desktop-12345678',
        targetBranch: 'main',
        managedWorktreeId: 'managed-1',
        legacy: false,
        runtimeStatus: 'active',
        prStatus: null,
      },
    });
    render(<SessionRuntimeCapsule />);
    expect(screen.getByRole('button', { name: /Worktree: pc\/desktop-desktop-12345678/i })).toBeInTheDocument();
  });
});

describe('buildDefaultWorktreeBranchName', () => {
  it('uses session title and short id', () => {
    const branch = buildDefaultWorktreeBranchName(
      {
        sessionId: '12345678-abcdef',
        displayTitle: 'Desktop Desktop',
      },
      '/repo/app',
    );
    expect(branch).toBe('pc/desktop-desktop-12345678');
  });
});
