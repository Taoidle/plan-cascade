import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { WorkspaceTreeSidebar, type WorkspaceTreeSidebarProps } from './WorkspaceTreeSidebar';
import type { ExecutionHistoryItem, SessionSnapshot } from '../../store/execution';
import type { WorkflowSessionCatalogItem } from '../../types/workflowKernel';
import { ToolCallStreamFilter } from '../../utils/toolCallFilter';

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: () => {} },
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => {
      const translations: Record<string, string> = {
        'sidebar.newTask': 'New Task',
        'sidebar.addDirectory': 'Add Directory',
        'sidebar.sessions': 'Sessions',
        'sidebar.skills': 'Skills',
        'sidebar.plugins': 'Plugins',
        'sidebar.agents': 'Agents',
        'sidebar.prompts': 'Prompts',
        'sidebar.clearAll': 'Clear All Sessions',
        'sidebar.noDirectories': 'No directories pinned',
        'sidebar.noDirectoriesHint': 'Click "Directory" to add a workspace',
        'sidebar.rename': 'rename',
        'sidebar.renamePrompt': 'Rename session',
        'sidebar.renameDialogDescription': 'Enter a new title for this session.',
        'sidebar.renameInputLabel': 'Session title',
        'sidebar.renameInputPlaceholder': 'Enter session title',
        'sidebar.cancelAction': 'Cancel',
        'sidebar.archiveConfirmAction': 'Confirm',
        'sidebar.renameConfirm': 'Confirm',
        'sidebar.deleteSession': 'Delete session',
        'sidebar.deleteSessionConfirm': 'Delete this session permanently? This cannot be undone.',
        'sidebar.deleteConfirmAction': 'Delete',
        'sidebar.archiveSession': 'Archive session',
        'sidebar.archiveAction': 'archive',
        'sidebar.restoreAction': 'restore',
        'sidebar.restoreSession': 'Restore session',
        'sidebar.removeDirectory': 'Remove from sidebar',
        'sidebar.newTaskInDir': 'New task in this directory',
        'sidebar.clearAllSessions': 'Clear All Sessions',
        'sidebar.selectSessions': 'Select',
        'sidebar.cancelSelection': 'Cancel',
        'sidebar.selectionCount': '{{count}} selected',
        'sidebar.deleteSelected': 'Delete Selected',
        'sidebar.bulkDeleteSessionConfirm': 'Delete {{count}} selected sessions permanently? This cannot be undone.',
        'sidebar.selectSession': 'Select session {{title}}',
        'sidebar.sort.label': 'Sort paths',
        'sidebar.sort.recent': 'Sort: Recent',
        'sidebar.sort.name': 'Sort: Name',
        'sidebar.archived.show': 'Show Archived',
        'sidebar.archived.hide': 'Hide Archived',
        'sidebar.badges.live': 'Live',
        'sidebar.badges.history': 'History',
        'sidebar.badges.archived': 'Archived',
        'sidebar.status.idle': 'Idle',
        'sidebar.status.running': 'Running',
        'sidebar.status.attention': 'Attention',
        'sidebar.time.justNow': 'just now',
        'sidebar.noWorkspace': 'No Workspace',
      };
      const template = translations[key] || options?.defaultValue || key;
      return template.replace(/\{\{(\w+)\}\}/g, (_, name) =>
        String((options as Record<string, unknown> | undefined)?.[name] ?? ''),
      );
    },
    i18n: { language: 'en' },
  }),
}));

let mockPinnedDirectories: string[] = [];
let mockWorkspacePath = '/repo/app';
const addPinnedDirectory = vi.fn();
const removePinnedDirectory = vi.fn();
const setWorkspacePath = vi.fn();
const setSessionPathSort = vi.fn();
const setShowArchivedSessions = vi.fn();
let mockSessionPathSort: 'recent' | 'name' = 'recent';
let mockShowArchivedSessions = false;

vi.mock('../../store/settings', () => ({
  useSettingsStore: vi.fn((selector) => {
    const state = {
      workspacePath: mockWorkspacePath,
      pinnedDirectories: mockPinnedDirectories,
      addPinnedDirectory,
      removePinnedDirectory,
      setWorkspacePath,
      sessionPathSort: mockSessionPathSort,
      setSessionPathSort,
      showArchivedSessions: mockShowArchivedSessions,
      setShowArchivedSessions,
    };
    return typeof selector === 'function' ? selector(state) : state;
  }),
}));

const mockSkillMemoryStore = {
  skills: [],
  loadSkills: vi.fn(),
  loadMemories: vi.fn(),
  openDialog: vi.fn(),
};
vi.mock('../../store/skillMemory', () => ({
  useSkillMemoryStore: vi.fn((selector) =>
    typeof selector === 'function' ? selector(mockSkillMemoryStore) : mockSkillMemoryStore,
  ),
}));

const mockPluginStore = {
  plugins: [],
  loadPlugins: vi.fn(),
  openDialog: vi.fn(),
  setActiveTab: vi.fn(),
};
vi.mock('../../store/plugins', () => ({
  usePluginStore: vi.fn((selector) => (typeof selector === 'function' ? selector(mockPluginStore) : mockPluginStore)),
}));

const mockAgentsStore = {
  agents: [],
  fetchAgents: vi.fn(),
  openDialog: vi.fn(),
};
vi.mock('../../store/agents', () => ({
  useAgentsStore: vi.fn((selector) => (typeof selector === 'function' ? selector(mockAgentsStore) : mockAgentsStore)),
}));

const mockPromptsStore = {
  prompts: [],
  fetchPrompts: vi.fn(),
  openDialog: vi.fn(),
};
vi.mock('../../store/prompts', () => ({
  usePromptsStore: vi.fn((selector) =>
    typeof selector === 'function' ? selector(mockPromptsStore) : mockPromptsStore,
  ),
}));

vi.mock('./SkillMemoryPanel', () => ({ SkillMemoryPanel: () => <div data-testid="skill-memory-panel" /> }));
vi.mock('./PluginPanel', () => ({ PluginPanel: () => <div data-testid="plugin-panel" /> }));
vi.mock('./AgentPanel', () => ({ AgentPanel: () => <div data-testid="agent-panel" /> }));
vi.mock('./PromptPanel', () => ({ PromptPanel: () => <div data-testid="prompt-panel" /> }));
vi.mock('../SkillMemory/SkillMemoryDialog', () => ({ SkillMemoryDialog: () => null }));
vi.mock('../Plugins/PluginDialog', () => ({ PluginDialog: () => null }));
vi.mock('../Agents/AgentDialog', () => ({ AgentDialog: () => null }));
vi.mock('../Prompts/PromptDialog', () => ({ PromptDialog: () => null }));
vi.mock('../SkillMemory/SkillMemoryToast', () => ({ SkillMemoryToast: () => null }));

function createHistoryItem(overrides: Partial<ExecutionHistoryItem> = {}): ExecutionHistoryItem {
  return {
    id: overrides.id ?? 'history-1',
    title: overrides.title,
    taskDescription: overrides.taskDescription ?? 'Build feature',
    workspacePath: overrides.workspacePath ?? '/repo/app',
    strategy: overrides.strategy ?? 'direct',
    status: overrides.status ?? 'completed',
    startedAt: overrides.startedAt ?? Date.now() - 60_000,
    completedAt: overrides.completedAt,
    duration: overrides.duration ?? 1000,
    completedStories: overrides.completedStories ?? 1,
    totalStories: overrides.totalStories ?? 1,
    success: overrides.success ?? true,
    error: overrides.error,
    conversationContent: overrides.conversationContent,
    conversationLines: overrides.conversationLines,
    sessionId: overrides.sessionId,
    llmBackend: overrides.llmBackend,
    llmProvider: overrides.llmProvider,
    llmModel: overrides.llmModel,
  };
}

function createWorkflowSession(overrides: Partial<WorkflowSessionCatalogItem> = {}): WorkflowSessionCatalogItem {
  return {
    sessionId: overrides.sessionId ?? 'root-1',
    sessionKind: 'simple_root',
    displayTitle: overrides.displayTitle ?? 'Implement billing',
    workspacePath: 'workspacePath' in overrides ? (overrides.workspacePath ?? null) : '/repo/app',
    status: overrides.status ?? 'active',
    activeMode: overrides.activeMode ?? 'task',
    backgroundState: overrides.backgroundState ?? 'background_running',
    createdAt: overrides.createdAt ?? new Date(Date.now() - 60_000).toISOString(),
    updatedAt: overrides.updatedAt ?? new Date().toISOString(),
    lastError: overrides.lastError ?? null,
    contextLedger: overrides.contextLedger ?? {
      conversationTurnCount: 3,
      artifactRefCount: 1,
      contextSourceKinds: ['chat_transcript_sync'],
      lastCompactionAt: null,
      ledgerVersion: 1,
    },
    modeSnapshots: overrides.modeSnapshots ?? {
      chat: {
        phase: 'ready',
        pendingInput: '',
        activeTurnId: null,
        turnCount: 3,
        lastUserMessage: null,
        lastAssistantMessage: null,
      },
      plan: {
        phase: 'completed',
        planId: null,
        runningStepId: null,
        pendingClarification: null,
        retryableSteps: [],
        planRevision: 1,
        lastEditOperation: null,
      },
      task: {
        phase: 'executing',
        prdId: null,
        currentStoryId: 'story-1',
        interviewSessionId: null,
        pendingInterview: null,
        completedStories: 2,
        failedStories: 0,
      },
    },
    modeRuntimeMeta: overrides.modeRuntimeMeta ?? {},
  };
}

const defaultProps: WorkspaceTreeSidebarProps = {
  history: [],
  onRestore: vi.fn(),
  onDelete: vi.fn(),
  onRename: vi.fn(),
  onClear: vi.fn(),
  onNewTask: vi.fn(),
};

describe('WorkspaceTreeSidebar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPinnedDirectories = [];
    mockWorkspacePath = '/repo/app';
    mockSessionPathSort = 'recent';
    mockShowArchivedSessions = false;
  });

  it('renders empty state when there are no grouped sessions', () => {
    render(<WorkspaceTreeSidebar {...defaultProps} />);

    expect(screen.getByText('No directories pinned')).toBeInTheDocument();
    expect(screen.queryByText('Live Sessions')).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'History' })).not.toBeInTheDocument();
  });

  it('renders a single path tree with mixed live and history sessions', () => {
    const onSwitchWorkflowSession = vi.fn();
    const onRestore = vi.fn();
    const onNewTaskInPath = vi.fn();
    mockPinnedDirectories = ['/repo'];

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        onRestore={onRestore}
        workflowSessions={[createWorkflowSession({ sessionId: 'root-live', displayTitle: 'Ship auth' })]}
        history={[createHistoryItem({ id: 'history-1', title: 'Past chat', workspacePath: '/repo/app' })]}
        activeWorkflowSessionId="root-live"
        onSwitchWorkflowSession={onSwitchWorkflowSession}
        onNewTaskInPath={onNewTaskInPath}
      />,
    );

    expect(screen.queryByText('Live Sessions')).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'History' })).not.toBeInTheDocument();
    expect(screen.getByText('repo')).toBeInTheDocument();
    expect(screen.getByText('Ship auth')).toBeInTheDocument();
    expect(screen.getByText('Past chat')).toBeInTheDocument();
    expect(screen.getByTitle('History')).toBeInTheDocument();
    expect(screen.getByTitle('task')).toBeInTheDocument();
    expect(screen.queryByText('Task:executing')).not.toBeInTheDocument();
    expect(screen.queryByText('Chat:ready')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /ship auth/i }));
    expect(onSwitchWorkflowSession).toHaveBeenCalledWith('root-live');

    fireEvent.click(screen.getByRole('button', { name: /past chat/i }));
    expect(onRestore).toHaveBeenCalledWith('history-1');

    fireEvent.click(screen.getByTitle('New task in this directory'));
    expect(onNewTaskInPath).toHaveBeenCalledWith('/repo');
  });

  it('supports renaming, archiving, deleting live sessions and clearing all sessions', () => {
    const onRenameWorkflowSession = vi.fn();
    const onArchiveWorkflowSession = vi.fn();
    const onDeleteWorkflowSession = vi.fn();
    const onClearAllSessions = vi.fn();

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        workflowSessions={[createWorkflowSession({ sessionId: 'root-live', displayTitle: 'Ship auth' })]}
        activeWorkflowSessionId="root-live"
        onSwitchWorkflowSession={vi.fn()}
        onRenameWorkflowSession={onRenameWorkflowSession}
        onArchiveWorkflowSession={onArchiveWorkflowSession}
        onDeleteWorkflowSession={onDeleteWorkflowSession}
        onClearAllSessions={onClearAllSessions}
      />,
    );

    fireEvent.click(screen.getAllByTitle('rename')[0]);
    fireEvent.change(screen.getByLabelText('Session title'), { target: { value: 'Renamed live session' } });
    fireEvent.click(screen.getByRole('button', { name: 'Confirm' }));
    expect(onRenameWorkflowSession).toHaveBeenCalledWith('root-live', 'Renamed live session');

    fireEvent.click(screen.getByTitle('Archive session'));
    expect(screen.getByText('Archive this live session?')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Confirm' }));
    expect(onArchiveWorkflowSession).toHaveBeenCalledWith('root-live');

    fireEvent.click(screen.getByTitle('Delete session'));
    expect(screen.getByText('Delete this session permanently? This cannot be undone.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(onDeleteWorkflowSession).toHaveBeenCalledWith('root-live');

    fireEvent.click(screen.getByRole('button', { name: 'Clear All Sessions' }));
    expect(onClearAllSessions).toHaveBeenCalled();
  });

  it('does not delete live sessions when the confirmation is cancelled', () => {
    const onDeleteWorkflowSession = vi.fn();

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        workflowSessions={[createWorkflowSession({ sessionId: 'root-live', displayTitle: 'Ship auth' })]}
        activeWorkflowSessionId="root-live"
        onDeleteWorkflowSession={onDeleteWorkflowSession}
      />,
    );

    fireEvent.click(screen.getByTitle('Delete session'));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onDeleteWorkflowSession).not.toHaveBeenCalled();
  });

  it('does not archive live sessions when the confirmation is cancelled', () => {
    const onArchiveWorkflowSession = vi.fn();

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        workflowSessions={[createWorkflowSession({ sessionId: 'root-live', displayTitle: 'Ship auth' })]}
        activeWorkflowSessionId="root-live"
        onArchiveWorkflowSession={onArchiveWorkflowSession}
      />,
    );

    fireEvent.click(screen.getByTitle('Archive session'));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onArchiveWorkflowSession).not.toHaveBeenCalled();
  });

  it('supports multi-select bulk delete across live and history sessions', () => {
    const onDeleteWorkflowSession = vi.fn();
    const onDelete = vi.fn();

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        onDelete={onDelete}
        workflowSessions={[createWorkflowSession({ sessionId: 'root-live', displayTitle: 'Ship auth' })]}
        history={[createHistoryItem({ id: 'history-1', title: 'Past chat', workspacePath: '/repo/app' })]}
        activeWorkflowSessionId="root-live"
        onDeleteWorkflowSession={onDeleteWorkflowSession}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Select' }));
    fireEvent.click(screen.getByLabelText('Select session Ship auth'));
    fireEvent.click(screen.getByLabelText('Select session Past chat'));
    expect(screen.getByText('2 selected')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Delete Selected' }));
    expect(screen.getByText('Delete 2 selected sessions permanently? This cannot be undone.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    expect(onDeleteWorkflowSession).toHaveBeenCalledWith('root-live');
    expect(onDelete).toHaveBeenCalledWith('history-1');
    expect(screen.queryByLabelText('Select session Ship auth')).not.toBeInTheDocument();
  });

  it('updates the clicked session checkbox immediately in selection mode', () => {
    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        workflowSessions={[
          createWorkflowSession({ sessionId: 'root-live-1', displayTitle: 'Ship auth' }),
          createWorkflowSession({ sessionId: 'root-live-2', displayTitle: 'Fix billing' }),
        ]}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Select' }));

    const firstCheckbox = screen.getByLabelText('Select session Ship auth') as HTMLInputElement;
    const secondCheckbox = screen.getByLabelText('Select session Fix billing') as HTMLInputElement;

    fireEvent.click(firstCheckbox);
    expect(firstCheckbox.checked).toBe(true);
    expect(secondCheckbox.checked).toBe(false);

    fireEvent.click(secondCheckbox);
    expect(firstCheckbox.checked).toBe(true);
    expect(secondCheckbox.checked).toBe(true);
    expect(screen.getByText('2 selected')).toBeInTheDocument();
  });

  it('cancels multi-select mode without deleting sessions', () => {
    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        workflowSessions={[createWorkflowSession({ sessionId: 'root-live', displayTitle: 'Ship auth' })]}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Select' }));
    expect(screen.getByLabelText('Select session Ship auth')).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole('button', { name: 'Cancel' })[0]);
    expect(screen.queryByLabelText('Select session Ship auth')).not.toBeInTheDocument();
  });

  it('toggles a path group open and closed without changing workspace', () => {
    mockWorkspacePath = '/other/workspace';
    mockPinnedDirectories = ['/repo/path1'];

    const { container } = render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        workflowSessions={[
          createWorkflowSession({ sessionId: 'root-2', displayTitle: 'Session 1', workspacePath: '/repo/path1/app' }),
        ]}
      />,
    );

    const pathButton = screen.getByRole('button', { name: /path1/i });
    const collapsible = container.querySelector('[style*="grid-template-rows"]') as HTMLElement;
    expect(collapsible.style.gridTemplateRows).toBe('0fr');

    fireEvent.click(pathButton);
    expect(collapsible.style.gridTemplateRows).toBe('1fr');

    fireEvent.click(pathButton);
    expect(collapsible.style.gridTemplateRows).toBe('0fr');
    expect(setWorkspacePath).not.toHaveBeenCalled();
  });

  it('ignores legacy background session props in the sessions tree', () => {
    const legacyBackgroundSessions: Record<string, SessionSnapshot> = {
      'bg-1': {
        id: 'bg-1',
        taskDescription: 'Legacy background task',
        status: 'running',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: null,
        isChatSession: true,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: Date.now(),
        toolCallFilter: new ToolCallStreamFilter(),
        llmBackend: 'openai',
        llmProvider: 'openai',
        llmModel: 'gpt-4.1',
        parentSessionId: undefined,
        workspacePath: '/legacy/project',
        updatedAt: Date.now(),
      },
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={legacyBackgroundSessions}
        workflowSessions={[createWorkflowSession({ displayTitle: 'Primary session' })]}
      />,
    );

    expect(screen.getByText('Primary session')).toBeInTheDocument();
    expect(screen.queryByText('Legacy background task')).not.toBeInTheDocument();
  });

  it('shows archived sessions when enabled and restores them from the tree', () => {
    mockShowArchivedSessions = true;
    const onRestoreWorkflowSession = vi.fn();

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        workflowSessions={[
          createWorkflowSession({
            sessionId: 'root-archived',
            displayTitle: 'Archived billing',
            status: 'archived',
            backgroundState: 'background_idle',
          }),
        ]}
        onRestoreWorkflowSession={onRestoreWorkflowSession}
        onDeleteWorkflowSession={vi.fn()}
      />,
    );

    expect(screen.getByTitle('Archived')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /archived billing/i }));
    expect(onRestoreWorkflowSession).toHaveBeenCalledWith('root-archived');
  });

  it('exposes icon-based path sorting controls through settings actions', () => {
    render(<WorkspaceTreeSidebar {...defaultProps} />);

    fireEvent.click(screen.getByRole('button', { name: 'Sort paths' }));
    fireEvent.click(screen.getByRole('button', { name: 'Sort: Name' }));
    expect(setSessionPathSort).toHaveBeenCalledWith('name');

    fireEvent.click(screen.getByRole('button', { name: 'Show Archived' }));
    expect(setShowArchivedSessions).toHaveBeenCalledWith(true);
  });
});
