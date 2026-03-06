import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import type { WorkflowSession } from '../../types/workflowKernel';
import {
  blobToBase64,
  captureElementToBlob,
  localTimestampForFilename,
  saveBinaryWithDialog,
} from '../../lib/exportUtils';

const storeHarness = vi.hoisted(() => {
  let executionState: Record<string, unknown> = {};
  let settingsState: Record<string, unknown> = {};
  let workflowKernelState: Record<string, unknown> = {};
  let workflowOrchestratorState: Record<string, unknown> = {};
  let planOrchestratorState: Record<string, unknown> = {};
  let taskModeState: Record<string, unknown> = {};
  let planModeState: Record<string, unknown> = {};
  let toolPermissionState: Record<string, unknown> = {};
  let contextSourcesState: Record<string, unknown> = {};

  const useExecutionStore = ((selector?: (state: Record<string, unknown>) => unknown) =>
    selector ? selector(executionState) : executionState) as {
    (selector?: (state: Record<string, unknown>) => unknown): unknown;
    getState: () => Record<string, unknown>;
    setState: (partial: Record<string, unknown>) => void;
    subscribe: ReturnType<typeof vi.fn>;
  };
  useExecutionStore.getState = () => executionState;
  useExecutionStore.setState = (partial) => {
    executionState = { ...executionState, ...partial };
  };
  useExecutionStore.subscribe = vi.fn(() => () => {});

  const useSettingsStore = ((selector?: (state: Record<string, unknown>) => unknown) =>
    selector ? selector(settingsState) : settingsState) as {
    (selector?: (state: Record<string, unknown>) => unknown): unknown;
    getState: () => Record<string, unknown>;
    setState: (partial: Record<string, unknown>) => void;
  };
  useSettingsStore.getState = () => settingsState;
  useSettingsStore.setState = (partial) => {
    settingsState = { ...settingsState, ...partial };
  };

  const useWorkflowKernelStore = ((selector?: (state: Record<string, unknown>) => unknown) =>
    selector ? selector(workflowKernelState) : workflowKernelState) as {
    (selector?: (state: Record<string, unknown>) => unknown): unknown;
    getState: () => Record<string, unknown>;
    setState: (partial: Record<string, unknown>) => void;
  };
  useWorkflowKernelStore.getState = () => workflowKernelState;
  useWorkflowKernelStore.setState = (partial) => {
    workflowKernelState = { ...workflowKernelState, ...partial };
  };

  const useWorkflowOrchestratorStore = ((selector?: (state: Record<string, unknown>) => unknown) =>
    selector ? selector(workflowOrchestratorState) : workflowOrchestratorState) as {
    (selector?: (state: Record<string, unknown>) => unknown): unknown;
    getState: () => Record<string, unknown>;
  };
  useWorkflowOrchestratorStore.getState = () => workflowOrchestratorState;

  const usePlanOrchestratorStore = ((selector?: (state: Record<string, unknown>) => unknown) =>
    selector ? selector(planOrchestratorState) : planOrchestratorState) as {
    (selector?: (state: Record<string, unknown>) => unknown): unknown;
    getState: () => Record<string, unknown>;
  };
  usePlanOrchestratorStore.getState = () => planOrchestratorState;

  const useTaskModeStore = ((selector?: (state: Record<string, unknown>) => unknown) =>
    selector ? selector(taskModeState) : taskModeState) as {
    (selector?: (state: Record<string, unknown>) => unknown): unknown;
    getState: () => Record<string, unknown>;
    setState: (partial: Record<string, unknown>) => void;
  };
  useTaskModeStore.getState = () => taskModeState;
  useTaskModeStore.setState = (partial) => {
    taskModeState = { ...taskModeState, ...partial };
  };

  const usePlanModeStore = ((selector?: (state: Record<string, unknown>) => unknown) =>
    selector ? selector(planModeState) : planModeState) as {
    (selector?: (state: Record<string, unknown>) => unknown): unknown;
    getState: () => Record<string, unknown>;
    setState: (partial: Record<string, unknown>) => void;
  };
  usePlanModeStore.getState = () => planModeState;
  usePlanModeStore.setState = (partial) => {
    planModeState = { ...planModeState, ...partial };
  };

  const useToolPermissionStore = ((selector?: (state: Record<string, unknown>) => unknown) =>
    selector ? selector(toolPermissionState) : toolPermissionState) as {
    (selector?: (state: Record<string, unknown>) => unknown): unknown;
  };

  const useContextSourcesStore = ((selector?: (state: Record<string, unknown>) => unknown) =>
    selector ? selector(contextSourcesState) : contextSourcesState) as {
    (selector?: (state: Record<string, unknown>) => unknown): unknown;
    getState: () => Record<string, unknown>;
  };
  useContextSourcesStore.getState = () => contextSourcesState;

  const mockSetGitSelectedTab = vi.fn();
  const useGitStore = {
    getState: () => ({
      setSelectedTab: mockSetGitSelectedTab,
    }),
  };

  const mockSelectTurn = vi.fn();
  const useFileChangesStore = {
    getState: () => ({
      selectTurn: mockSelectTurn,
    }),
  };

  const useAgentsStore = {
    getState: () => ({
      clearActiveAgent: vi.fn(),
    }),
  };

  return {
    useExecutionStore,
    useSettingsStore,
    useWorkflowKernelStore,
    useWorkflowOrchestratorStore,
    usePlanOrchestratorStore,
    useTaskModeStore,
    usePlanModeStore,
    useToolPermissionStore,
    useContextSourcesStore,
    useGitStore,
    useFileChangesStore,
    useAgentsStore,
    getExecutionState: () => executionState,
    setExecutionState: (state: Record<string, unknown>) => {
      executionState = state;
    },
    getSettingsState: () => settingsState,
    setSettingsState: (state: Record<string, unknown>) => {
      settingsState = state;
    },
    getWorkflowKernelState: () => workflowKernelState,
    setWorkflowKernelState: (state: Record<string, unknown>) => {
      workflowKernelState = state;
    },
    getWorkflowOrchestratorState: () => workflowOrchestratorState,
    setWorkflowOrchestratorState: (state: Record<string, unknown>) => {
      workflowOrchestratorState = state;
    },
    getPlanOrchestratorState: () => planOrchestratorState,
    setPlanOrchestratorState: (state: Record<string, unknown>) => {
      planOrchestratorState = state;
    },
    getTaskModeState: () => taskModeState,
    setTaskModeState: (state: Record<string, unknown>) => {
      taskModeState = state;
    },
    getPlanModeState: () => planModeState,
    setPlanModeState: (state: Record<string, unknown>) => {
      planModeState = state;
    },
    setToolPermissionState: (state: Record<string, unknown>) => {
      toolPermissionState = state;
    },
    setContextSourcesState: (state: Record<string, unknown>) => {
      contextSourcesState = state;
    },
  };
});

const mockInvoke = vi.fn();
const mockShowToast = vi.fn();

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue || key,
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('../shared/Toast', () => ({
  useToast: () => ({ showToast: mockShowToast }),
}));

vi.mock('../shared/EffectiveContextSummary', () => ({
  EffectiveContextSummary: () => <div data-testid="effective-context-summary" />,
}));

vi.mock('../SimpleMode/ChatTranscript', () => ({
  ChatTranscript: ({
    lines,
    forceFullRender,
    scrollRef,
  }: {
    lines: Array<unknown>;
    forceFullRender?: boolean;
    scrollRef?: { current: HTMLDivElement | null };
  }) => (
    <div
      ref={(node) => {
        if (scrollRef) scrollRef.current = node;
      }}
      data-testid="chat-transcript"
      data-force-full-render={forceFullRender ? 'true' : 'false'}
    >
      lines:{lines.length}
    </div>
  ),
}));

vi.mock('../SimpleMode/WorkspaceTreeSidebar', () => ({
  WorkspaceTreeSidebar: () => <div data-testid="workspace-tree-sidebar" />,
}));

vi.mock('../SimpleMode/EdgeCollapseButton', () => ({
  EdgeCollapseButton: ({ onToggle }: { onToggle: () => void }) => (
    <button data-testid="edge-toggle" onClick={onToggle}>
      edge-toggle
    </button>
  ),
}));

vi.mock('../SimpleMode/BottomStatusBar', () => ({
  BottomStatusBar: () => <div data-testid="bottom-status-bar" />,
}));

vi.mock('../SimpleMode/InputBox', async () => {
  const React = await import('react');
  const InputBox = React.forwardRef(function MockInputBox(
    props: {
      value: string;
      onChange: (value: string) => void;
      onSubmit: () => void;
      disabled?: boolean;
      placeholder?: string;
    },
    ref: React.Ref<{ pickFile: () => void }>,
  ) {
    React.useImperativeHandle(ref, () => ({
      pickFile: vi.fn(),
    }));
    return (
      <div data-testid="input-box">
        <input
          data-testid="composer-input"
          value={props.value}
          onChange={(event) => props.onChange(event.target.value)}
          placeholder={props.placeholder}
          disabled={props.disabled}
        />
        <button data-testid="composer-submit" onClick={props.onSubmit} disabled={props.disabled}>
          submit
        </button>
      </div>
    );
  });
  return { InputBox };
});

vi.mock('../SimpleMode/InterviewInputPanel', () => ({
  InterviewInputPanel: () => <div data-testid="interview-input-panel" />,
}));

vi.mock('../SimpleMode/ToolPermissionOverlay', () => ({
  ToolPermissionOverlay: () => <div data-testid="tool-permission-overlay" />,
}));

vi.mock('../SimpleMode/TabbedRightPanel', () => ({
  TabbedRightPanel: ({
    workflowMode,
    workflowPhase,
    activeTab,
  }: {
    workflowMode: string;
    workflowPhase: string;
    activeTab: string;
  }) => (
    <div
      data-testid="tabbed-right-panel"
      data-workflow-mode={workflowMode}
      data-workflow-phase={workflowPhase}
      data-active-tab={activeTab}
    />
  ),
}));

vi.mock('../SimpleMode/ChatToolbar', () => ({
  ChatToolbar: ({
    workflowMode,
    onWorkflowModeChange,
    onToggleOutput,
    onExportImage,
  }: {
    workflowMode: string;
    onWorkflowModeChange: (mode: 'chat' | 'plan' | 'task') => void;
    onToggleOutput: () => void;
    onExportImage: () => void;
  }) => (
    <div data-testid="chat-toolbar">
      <div data-testid="toolbar-workflow-mode">{workflowMode}</div>
      <button data-testid="mode-chat" onClick={() => onWorkflowModeChange('chat')}>
        mode-chat
      </button>
      <button data-testid="mode-plan" onClick={() => onWorkflowModeChange('plan')}>
        mode-plan
      </button>
      <button data-testid="mode-task" onClick={() => onWorkflowModeChange('task')}>
        mode-task
      </button>
      <button data-testid="toggle-output" onClick={onToggleOutput}>
        toggle-output
      </button>
      <button data-testid="export-image" onClick={onExportImage}>
        export-image
      </button>
    </div>
  ),
}));

vi.mock('../SimpleMode/WorkflowModeSwitchDialog', () => ({
  WorkflowModeSwitchDialog: ({
    open,
    onOpenChange,
    onConfirm,
  }: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onConfirm: () => void;
  }) =>
    open ? (
      <div data-testid="mode-switch-dialog">
        <button data-testid="mode-switch-confirm" onClick={onConfirm}>
          confirm-mode-switch
        </button>
        <button data-testid="mode-switch-cancel" onClick={() => onOpenChange(false)}>
          cancel-mode-switch
        </button>
      </div>
    ) : null,
}));

vi.mock('../../lib/fileChangeCardBridge', () => ({
  createFileChangeCardBridge: () => ({
    startListening: async () => () => {},
    onTurnEnd: vi.fn(),
    reset: vi.fn(),
  }),
}));

vi.mock('../../lib/simpleModeNavigation', () => ({
  listenOpenAIChanges: () => () => {},
}));

vi.mock('../../lib/exportUtils', () => ({
  captureElementToBlob: vi.fn(),
  blobToBase64: vi.fn(),
  saveBinaryWithDialog: vi.fn(),
  localTimestampForFilename: vi.fn(),
}));

vi.mock('../../lib/contextBridge', () => ({
  buildConversationHistory: () => [],
  buildRootConversationHistory: () => [],
  buildRootConversationContextString: () => undefined,
}));

vi.mock('../SimpleMode/queuePersistence', () => ({
  clearPersistedSimpleChatQueue: vi.fn(),
  loadPersistedSimpleChatQueue: vi.fn(() => []),
  loadPersistedSimpleChatQueueWithMeta: vi.fn(() => ({
    queue: [],
    sourceVersion: null,
    sourceKey: null,
    migratedFromVersion: null,
    crossSessionCount: 0,
  })),
  persistSimpleChatQueue: vi.fn(),
}));

vi.mock('../SimpleMode/tokenBudget', () => ({
  DEFAULT_PROMPT_TOKEN_BUDGET: 8000,
  estimatePromptTokensFallback: vi.fn(() => ({
    estimatedTokens: 10,
    exceedsBudget: false,
  })),
  toAttachmentTokenEstimateInput: vi.fn(() => []),
}));

vi.mock('../../lib/promptTokenBudget', () => ({
  resolvePromptTokenBudget: vi.fn(async () => 8000),
}));

vi.mock('../../store/execution', () => ({
  useExecutionStore: storeHarness.useExecutionStore,
}));

vi.mock('../../store/settings', () => ({
  useSettingsStore: storeHarness.useSettingsStore,
}));

vi.mock('../../store/workflowKernel', () => ({
  useWorkflowKernelStore: storeHarness.useWorkflowKernelStore,
}));

vi.mock('../../store/workflowOrchestrator', () => ({
  useWorkflowOrchestratorStore: storeHarness.useWorkflowOrchestratorStore,
}));

vi.mock('../../store/planOrchestrator', () => ({
  usePlanOrchestratorStore: storeHarness.usePlanOrchestratorStore,
}));

vi.mock('../../store/taskMode', () => ({
  useTaskModeStore: storeHarness.useTaskModeStore,
}));

vi.mock('../../store/planMode', () => ({
  usePlanModeStore: storeHarness.usePlanModeStore,
}));

vi.mock('../../store/toolPermission', () => ({
  useToolPermissionStore: storeHarness.useToolPermissionStore,
}));

vi.mock('../../store/contextSources', () => ({
  useContextSourcesStore: storeHarness.useContextSourcesStore,
}));

vi.mock('../../store/git', () => ({
  useGitStore: storeHarness.useGitStore,
}));

vi.mock('../../store/fileChanges', () => ({
  useFileChangesStore: storeHarness.useFileChangesStore,
}));

vi.mock('../../store/agents', () => ({
  useAgentsStore: storeHarness.useAgentsStore,
}));

import { SimpleMode } from '../SimpleMode';

function createKernelSession(activeMode: 'chat' | 'plan' | 'task' = 'chat'): WorkflowSession {
  return {
    sessionId: 'kernel-session-1',
    sessionKind: 'simple_root',
    displayTitle: 'Test session',
    workspacePath: '/tmp/project',
    status: 'active',
    activeMode,
    modeSnapshots: {
      chat: {
        phase: 'ready',
        draftInput: '',
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
        runId: null,
        backgroundStatus: null,
        resumableFromCheckpoint: false,
        lastCheckpointId: null,
      },
      task: {
        phase: 'idle',
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
    linkedModeSessions: {} as Record<string, string>,
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
    createdAt: '2026-03-02T00:00:00Z',
    updatedAt: '2026-03-02T00:00:00Z',
    lastCheckpointId: null,
  };
}

function resetStates() {
  const execution = {
    status: 'idle',
    isCancelling: false,
    connectionStatus: 'connected',
    isSubmitting: false,
    apiError: null,
    start: vi.fn(async () => undefined),
    sendFollowUp: vi.fn(async () => undefined),
    pause: vi.fn(),
    resume: vi.fn(),
    cancel: vi.fn(),
    reset: vi.fn(),
    initialize: vi.fn(),
    cleanup: vi.fn(),
    isAnalyzingStrategy: false,
    clearStrategyAnalysis: vi.fn(),
    isChatSession: false,
    streamingOutput: [],
    analysisCoverage: null,
    logs: [],
    history: [],
    clearHistory: vi.fn(),
    deleteHistory: vi.fn(),
    renameHistory: vi.fn(),
    restoreFromHistory: vi.fn(),
    sessionUsageTotals: null,
    turnUsageTotals: null,
    taskId: null,
    standaloneSessionId: null,
    attachments: [],
    addAttachment: vi.fn(),
    removeAttachment: vi.fn(),
    clearAttachments: vi.fn(),
    backgroundSessions: {},
    backgroundCurrentSession: vi.fn(),
    switchToSession: vi.fn(),
    removeBackgroundSession: vi.fn(),
    foregroundParentSessionId: null,
    foregroundBgId: null,
    activeAgentName: null,
  };
  storeHarness.setExecutionState(execution);

  storeHarness.setSettingsState({
    backend: 'claude-code',
    provider: 'anthropic',
    model: '',
    workspacePath: '/tmp/project',
    sidebarCollapsed: false,
    setSidebarCollapsed: vi.fn(),
    autoPanelHoverEnabled: false,
  });

  storeHarness.setWorkflowKernelState({
    sessionId: 'kernel-session-1',
    activeRootSessionId: 'kernel-session-1',
    session: createKernelSession('chat'),
    sessionCatalog: [],
    openSession: vi.fn(async function () {
      return storeHarness.getWorkflowKernelState().session;
    }),
    recoverSession: vi.fn(async () => null),
    getSessionCatalogState: vi.fn(async () => ({
      activeSessionId: 'kernel-session-1',
      sessions: [],
    })),
    resumeBackgroundRuns: vi.fn(async () => []),
    getModeTranscript: vi.fn(async (_sessionId: string, mode: 'chat' | 'plan' | 'task') => ({
      sessionId: 'kernel-session-1',
      mode,
      revision: 1,
      lines: [],
    })),
    appendContextItems: vi.fn(async function () {
      return storeHarness.getWorkflowKernelState().session;
    }),
    appendModeTranscript: vi.fn(async (_sessionId: string, mode: 'chat' | 'plan' | 'task', lines: unknown[]) => ({
      sessionId: 'kernel-session-1',
      mode,
      revision: 2,
      lines,
    })),
    storeModeTranscript: vi.fn(async (_sessionId: string, mode: 'chat' | 'plan' | 'task', lines: unknown[]) => ({
      sessionId: 'kernel-session-1',
      mode,
      revision: 1,
      lines,
    })),
    activateSession: vi.fn(async () => ({
      session: storeHarness.getWorkflowKernelState().session,
      events: [],
      checkpoints: [],
    })),
    transitionMode: vi.fn(async (targetMode: 'chat' | 'plan' | 'task') => {
      const session = createKernelSession(targetMode);
      storeHarness.setWorkflowKernelState({
        ...storeHarness.getWorkflowKernelState(),
        session,
      });
      return session;
    }),
    transitionAndSubmitInput: vi.fn(async function () {
      return storeHarness.getWorkflowKernelState().session;
    }),
    submitInput: vi.fn(async function () {
      return storeHarness.getWorkflowKernelState().session;
    }),
    linkModeSession: vi.fn(async (mode: 'chat' | 'plan' | 'task', modeSessionId: string) => {
      const currentSession = storeHarness.getWorkflowKernelState().session as ReturnType<typeof createKernelSession>;
      currentSession.linkedModeSessions[mode] = modeSessionId;
      return currentSession;
    }),
    cancelOperation: vi.fn(async function () {
      return storeHarness.getWorkflowKernelState().session;
    }),
    refreshSessionState: vi.fn(async function () {
      return {
        session: storeHarness.getWorkflowKernelState().session,
        events: [],
        checkpoints: [],
      };
    }),
    reset: vi.fn(),
  });

  storeHarness.setWorkflowOrchestratorState({
    pendingInterviewQuestion: null,
    phase: 'idle',
    sessionId: null,
    startWorkflow: vi.fn(async () => {
      storeHarness.setWorkflowOrchestratorState({
        ...storeHarness.getWorkflowOrchestratorState(),
        sessionId: 'task-session-1',
      });
      storeHarness.setTaskModeState({
        ...storeHarness.getTaskModeState(),
        sessionId: 'task-session-1',
      });
      return { modeSessionId: 'task-session-1' };
    }),
    submitInterviewAnswer: vi.fn(async () => undefined),
    skipInterviewQuestion: vi.fn(async () => undefined),
    overrideConfigNatural: vi.fn(),
    addPrdFeedback: vi.fn(),
    cancelWorkflow: vi.fn(async () => undefined),
    isCancelling: false,
    resetWorkflow: vi.fn(),
  });

  storeHarness.setPlanOrchestratorState({
    pendingClarifyQuestion: null,
    phase: 'idle',
    isBusy: false,
    sessionId: null,
    startPlanWorkflow: vi.fn(async () => {
      storeHarness.setPlanOrchestratorState({
        ...storeHarness.getPlanOrchestratorState(),
        sessionId: 'plan-session-1',
      });
      storeHarness.setPlanModeState({
        ...storeHarness.getPlanModeState(),
        sessionId: 'plan-session-1',
      });
      return { modeSessionId: 'plan-session-1' };
    }),
    submitClarification: vi.fn(async () => undefined),
    skipClarification: vi.fn(async () => undefined),
    cancelWorkflow: vi.fn(async () => undefined),
    ensureTerminalCompletionCardFromKernel: vi.fn(async () => undefined),
    isCancelling: false,
    resetWorkflow: vi.fn(),
  });

  storeHarness.setTaskModeState({
    sessionId: null,
  });

  storeHarness.setPlanModeState({
    sessionId: null,
  });

  storeHarness.setToolPermissionState({
    pendingRequest: null,
    requestQueue: [],
    isResponding: false,
    respond: vi.fn(async () => undefined),
    sessionLevel: 'ask',
    setSessionLevel: vi.fn(async () => undefined),
  });

  storeHarness.setContextSourcesState({
    resetAutoAssociation: vi.fn(),
  });
}

function renderSimpleMode(children?: ReactNode) {
  return render(children ?? <SimpleMode />);
}

describe('SimpleMode', () => {
  beforeEach(() => {
    resetStates();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('mounts with initialize and unmounts with cleanup', () => {
    const { unmount } = renderSimpleMode();
    expect(storeHarness.getExecutionState().initialize).toHaveBeenCalledTimes(1);
    expect(storeHarness.getWorkflowKernelState().resumeBackgroundRuns).toHaveBeenCalledTimes(1);

    unmount();
    expect(storeHarness.getExecutionState().cleanup).toHaveBeenCalledTimes(1);
  });

  it('shows api error', () => {
    storeHarness.setExecutionState({
      ...storeHarness.getExecutionState(),
      apiError: 'Connection refused',
    });
    renderSimpleMode();
    expect(screen.getByText('Connection refused')).toBeInTheDocument();
  });

  it('starts task workflow and links task session', async () => {
    renderSimpleMode();

    fireEvent.click(screen.getByTestId('mode-task'));
    await waitFor(() => {
      expect(screen.getByTestId('toolbar-workflow-mode')).toHaveTextContent('task');
    });

    fireEvent.change(screen.getByTestId('composer-input'), { target: { value: 'Implement auth flow' } });
    fireEvent.click(screen.getByTestId('composer-submit'));

    await waitFor(() => {
      expect(storeHarness.getWorkflowOrchestratorState().startWorkflow).toHaveBeenCalledWith('Implement auth flow');
      expect(storeHarness.getWorkflowKernelState().linkModeSession).toHaveBeenCalledWith('task', 'task-session-1');
    });
  });

  it('starts plan workflow and links plan session', async () => {
    renderSimpleMode();

    fireEvent.click(screen.getByTestId('mode-plan'));
    await waitFor(() => {
      expect(screen.getByTestId('toolbar-workflow-mode')).toHaveTextContent('plan');
    });

    fireEvent.change(screen.getByTestId('composer-input'), { target: { value: 'Plan migration rollout' } });
    fireEvent.click(screen.getByTestId('composer-submit'));

    await waitFor(() => {
      expect(storeHarness.getPlanOrchestratorState().startPlanWorkflow).toHaveBeenCalledWith('Plan migration rollout');
      expect(storeHarness.getWorkflowKernelState().linkModeSession).toHaveBeenCalledWith('plan', 'plan-session-1');
    });
  });

  it('requires confirmation to switch workflow mode while execution is running', async () => {
    storeHarness.setExecutionState({
      ...storeHarness.getExecutionState(),
      status: 'running',
    });
    renderSimpleMode();

    fireEvent.click(screen.getByTestId('mode-task'));
    expect(screen.getByTestId('toolbar-workflow-mode')).toHaveTextContent('chat');
    expect(screen.getByTestId('mode-switch-dialog')).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('mode-switch-confirm'));
    await waitFor(() => {
      expect(screen.getByTestId('toolbar-workflow-mode')).toHaveTextContent('task');
    });
  });

  it('uses kernel snapshot phase as single source of truth', async () => {
    storeHarness.setWorkflowOrchestratorState({
      ...storeHarness.getWorkflowOrchestratorState(),
      phase: 'executing',
    });

    const kernelState = storeHarness.getWorkflowKernelState();
    const kernelSession = kernelState.session as ReturnType<typeof createKernelSession>;
    storeHarness.setWorkflowKernelState({
      ...kernelState,
      session: {
        ...kernelSession,
        modeSnapshots: {
          ...kernelSession.modeSnapshots,
          task: {
            ...kernelSession.modeSnapshots.task,
            phase: 'idle',
          },
        },
      },
    });

    renderSimpleMode();
    fireEvent.click(screen.getByTestId('mode-task'));
    await waitFor(() => {
      expect(screen.getByTestId('toolbar-workflow-mode')).toHaveTextContent('task');
    });

    fireEvent.click(screen.getByTestId('toggle-output'));
    await waitFor(() => {
      expect(screen.getByTestId('tabbed-right-panel')).toHaveAttribute('data-workflow-phase', 'idle');
    });

    await new Promise((resolve) => setTimeout(resolve, 1700));
    expect(storeHarness.getWorkflowKernelState().refreshSessionState).not.toHaveBeenCalled();
  });

  it('uses kernel plan phase instead of plan orchestrator fallback phase', async () => {
    const kernelSession = createKernelSession('plan');
    kernelSession.modeSnapshots.plan = {
      ...kernelSession.modeSnapshots.plan!,
      phase: 'executing',
    };
    storeHarness.setWorkflowKernelState({
      ...storeHarness.getWorkflowKernelState(),
      session: kernelSession,
    });
    storeHarness.setPlanOrchestratorState({
      ...storeHarness.getPlanOrchestratorState(),
      phase: 'clarification_error',
    });

    renderSimpleMode();
    await waitFor(() => {
      expect(screen.getByTestId('toolbar-workflow-mode')).toHaveTextContent('plan');
    });

    fireEvent.click(screen.getByTestId('toggle-output'));
    await waitFor(() => {
      expect(screen.getByTestId('tabbed-right-panel')).toHaveAttribute('data-workflow-phase', 'executing');
    });
  });

  it('shows task interview input panel when kernel has pending interview even if orchestrator question is empty', async () => {
    const kernelSession = createKernelSession('task');
    kernelSession.linkedModeSessions.task = 'task-session-1';
    kernelSession.modeSnapshots.task = {
      ...kernelSession.modeSnapshots.task!,
      phase: 'interviewing',
      pendingInterview: {
        interviewId: 'interview-1',
        questionId: 'q1',
        question: 'Need auth?',
        hint: null,
        required: true,
        inputType: 'boolean',
        options: [],
        allowCustom: false,
        questionNumber: 1,
        totalQuestions: 3,
      },
    };
    storeHarness.setWorkflowKernelState({
      ...storeHarness.getWorkflowKernelState(),
      session: kernelSession,
    });
    storeHarness.setWorkflowOrchestratorState({
      ...storeHarness.getWorkflowOrchestratorState(),
      phase: 'analyzing',
      pendingInterviewQuestion: null,
    });

    renderSimpleMode();

    await waitFor(() => {
      expect(screen.getByTestId('toolbar-workflow-mode')).toHaveTextContent('task');
    });
    expect(screen.getByTestId('interview-input-panel')).toBeInTheDocument();
  });

  it('routes plan clarification submit from kernel pending clarification when orchestrator state is stale', async () => {
    const kernelSession = createKernelSession('plan');
    kernelSession.linkedModeSessions.plan = 'plan-session-1';
    kernelSession.modeSnapshots.plan = {
      ...kernelSession.modeSnapshots.plan!,
      phase: 'clarifying',
      pendingClarification: {
        questionId: 'q1',
        question: 'Target audience?',
        hint: null,
        inputType: 'text',
        options: [],
        required: false,
      },
    };
    storeHarness.setWorkflowKernelState({
      ...storeHarness.getWorkflowKernelState(),
      session: kernelSession,
    });
    storeHarness.setPlanOrchestratorState({
      ...storeHarness.getPlanOrchestratorState(),
      phase: 'planning',
      pendingClarifyQuestion: null,
    });

    renderSimpleMode();

    await waitFor(() => {
      expect(screen.getByTestId('toolbar-workflow-mode')).toHaveTextContent('plan');
    });

    fireEvent.change(screen.getByPlaceholderText('Type your answer...'), { target: { value: 'Developers' } });
    fireEvent.click(screen.getByText('Submit'));

    await waitFor(() => {
      expect(storeHarness.getPlanOrchestratorState().submitClarification).toHaveBeenCalledWith({
        questionId: 'q1',
        answer: 'Developers',
        skipped: false,
      });
    });
    expect(storeHarness.getExecutionState().sendFollowUp).not.toHaveBeenCalled();
  });

  it('forces full transcript render while exporting image', async () => {
    const mockCapture = vi.mocked(captureElementToBlob);
    const mockBlobToBase64 = vi.mocked(blobToBase64);
    const mockSaveBinary = vi.mocked(saveBinaryWithDialog);
    const mockTimestamp = vi.mocked(localTimestampForFilename);
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });

    const captureResolver: { current: ((value: Blob) => void) | null } = { current: null };
    mockCapture.mockImplementation(
      () =>
        new Promise<Blob>((resolve) => {
          captureResolver.current = resolve;
        }),
    );
    mockBlobToBase64.mockResolvedValue('base64-content');
    mockSaveBinary.mockResolvedValue(true);
    mockTimestamp.mockReturnValue('20260304-120000');

    renderSimpleMode();
    expect(screen.getByTestId('chat-transcript')).toHaveAttribute('data-force-full-render', 'false');

    fireEvent.click(screen.getByTestId('export-image'));

    await waitFor(() => {
      expect(screen.getByTestId('chat-transcript')).toHaveAttribute('data-force-full-render', 'true');
    });
    expect(mockCapture).toHaveBeenCalledTimes(1);

    if (captureResolver.current) {
      captureResolver.current(new Blob(['image'], { type: 'image/png' }));
    }
    await waitFor(() => {
      expect(screen.getByTestId('chat-transcript')).toHaveAttribute('data-force-full-render', 'false');
    });

    rafSpy.mockRestore();
  });
});
