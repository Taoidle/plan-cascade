/**
 * WorkspaceTreeSidebar - Background Sessions Tests
 *
 * Story 008: Verify that background sessions are rendered separately from history
 * with status indicators, switch/remove actions, and proper i18n labels.
 * Also verifies fork hierarchy display within workspace directory tree.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// Mock react-i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'sidebar.newTask': 'New Task',
        'sidebar.addDirectory': 'Add Directory',
        'sidebar.skills': 'Skills',
        'sidebar.plugins': 'Plugins',
        'sidebar.clearAll': 'Clear All Sessions',
        'sidebar.current': 'Current',
        'sidebar.otherSessions': 'Other',
        'sidebar.noDirectories': 'No directories pinned',
        'sidebar.noDirectoriesHint': 'Click "Directory" to add a workspace',
        'sidebar.rename': 'rename',
        'sidebar.deleteSession': 'Delete session',
        'sidebar.activeSessions': 'Active Sessions',
        'sidebar.switchSession': 'Switch to this session',
        'sidebar.removeSession': 'Remove session',
        'sidebar.currentFork': '(current)',
        'sidebar.removeDirectory': 'Remove from sidebar',
        'sidebar.newTaskInDir': 'New task in this directory',
        'sidebar.toggleSidebar': 'Toggle sidebar',
        'sidebar.sessions': 'Sessions',
        'skillPanel.title': 'Skills & Memory',
        'skillPanel.manageAll': 'Manage All...',
        'skillPanel.loading': 'Loading...',
        'skillPanel.detectedSkills': 'Auto-Detected Skills',
        'skillPanel.projectSkills': 'Project Skills',
        'skillPanel.memories': 'Memories',
      };
      return translations[key] || key;
    },
    i18n: { language: 'en' },
  }),
}));

// Settings store mock — default: no pinned directories
let mockPinnedDirectories: string[] = [];
let mockWorkspacePath = '/test/project';

vi.mock('../../store/settings', () => ({
  useSettingsStore: vi.fn((selector) => {
    const state = {
      workspacePath: mockWorkspacePath,
      pinnedDirectories: mockPinnedDirectories,
      addPinnedDirectory: vi.fn(),
      removePinnedDirectory: vi.fn(),
      setWorkspacePath: vi.fn(),
    };
    return typeof selector === 'function' ? selector(state) : state;
  }),
}));

// Mock skillMemory store
vi.mock('../../store/skillMemory', () => ({
  useSkillMemoryStore: vi.fn((selector) => {
    const state = {
      skills: [],
      skillsLoading: false,
      memories: [],
      memoriesLoading: false,
      panelOpen: false,
      dialogOpen: false,
      activeTab: 'skills',
      toastMessage: null,
      toastType: 'info',
      loadSkills: vi.fn(),
      loadMemories: vi.fn(),
      loadMemoryStats: vi.fn(),
      toggleSkill: vi.fn(),
      togglePanel: vi.fn(),
      openDialog: vi.fn(),
      closeDialog: vi.fn(),
      setActiveTab: vi.fn(),
      clearToast: vi.fn(),
    };
    return typeof selector === 'function' ? selector(state) : state;
  }),
}));

// Mock child components that depend on the store
vi.mock('./SkillMemoryPanel', () => ({
  SkillMemoryPanel: () => null,
}));

vi.mock('../SkillMemory/SkillMemoryDialog', () => ({
  SkillMemoryDialog: () => null,
}));

vi.mock('../SkillMemory/SkillMemoryToast', () => ({
  SkillMemoryToast: () => null,
}));

import { WorkspaceTreeSidebar, type WorkspaceTreeSidebarProps } from './WorkspaceTreeSidebar';
import type { SessionSnapshot } from '../../store/execution';

// Helper to create a mock SessionSnapshot
function createMockSnapshot(overrides: Partial<SessionSnapshot> = {}): SessionSnapshot {
  return {
    id: `bg-${Date.now()}-${Math.random()}`,
    taskDescription: 'Background task',
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
    toolCallFilter: { reset: vi.fn(), feed: vi.fn() } as unknown as SessionSnapshot['toolCallFilter'],
    llmBackend: 'claude-code',
    llmProvider: 'anthropic',
    llmModel: '',
    parentSessionId: undefined,
    workspacePath: undefined,
    ...overrides,
  };
}

const defaultProps: WorkspaceTreeSidebarProps = {
  history: [],
  onRestore: vi.fn(),
  onDelete: vi.fn(),
  onRename: vi.fn(),
  onClear: vi.fn(),
  onNewTask: vi.fn(),
  currentTask: null,
};

describe('WorkspaceTreeSidebar - Background Sessions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPinnedDirectories = [];
    mockWorkspacePath = '/test/project';
  });

  it('should NOT render "Active Sessions" section when there are no background sessions', () => {
    render(<WorkspaceTreeSidebar {...defaultProps} backgroundSessions={{}} />);

    expect(screen.queryByText('Active Sessions')).not.toBeInTheDocument();
  });

  it('should render "Active Sessions" section when background sessions exist (no matching directory)', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({ id: 'bg-1', taskDescription: 'Build API endpoint' }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    expect(screen.getByText('Active Sessions')).toBeInTheDocument();
  });

  it('should render background session labels (first ~50 chars of taskDescription)', () => {
    const longDescription = 'A'.repeat(80);
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({
        id: 'bg-1',
        taskDescription: longDescription,
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    // Should display a truncated version of the description
    const sessionLabel = screen.getByText(longDescription.slice(0, 50) + '...');
    expect(sessionLabel).toBeInTheDocument();
  });

  it('should render "Untitled Session" for background sessions with no taskDescription', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({ id: 'bg-1', taskDescription: '' }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    expect(screen.getByText('Untitled Session')).toBeInTheDocument();
  });

  it('should show a pulsing blue dot for running background sessions', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({ id: 'bg-1', status: 'running', taskDescription: 'Running task' }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    // Find the status dot element (data-testid)
    const dot = screen.getByTestId('bg-status-dot-bg-1');
    expect(dot).toBeInTheDocument();
    expect(dot.className).toContain('bg-blue-500');
    expect(dot.className).toContain('animate-pulse');
  });

  it('should show a green dot for completed background sessions', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({ id: 'bg-1', status: 'completed', taskDescription: 'Completed task' }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    const dot = screen.getByTestId('bg-status-dot-bg-1');
    expect(dot.className).toContain('bg-green-500');
    expect(dot.className).not.toContain('animate-pulse');
  });

  it('should show a red dot for failed background sessions', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({ id: 'bg-1', status: 'failed', taskDescription: 'Failed task' }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    const dot = screen.getByTestId('bg-status-dot-bg-1');
    expect(dot.className).toContain('bg-red-500');
    expect(dot.className).not.toContain('animate-pulse');
  });

  it('should show a gray dot for idle background sessions', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({ id: 'bg-1', status: 'idle', taskDescription: 'Idle session' }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    const dot = screen.getByTestId('bg-status-dot-bg-1');
    expect(dot.className).toContain('bg-gray-400');
  });

  it('should call onSwitchSession when a background session is clicked', () => {
    const onSwitchSession = vi.fn();
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({ id: 'bg-1', taskDescription: 'Click me' }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={onSwitchSession}
        onRemoveSession={vi.fn()}
      />,
    );

    const sessionItem = screen.getByTestId('bg-session-item-bg-1');
    fireEvent.click(sessionItem);

    expect(onSwitchSession).toHaveBeenCalledWith('bg-1');
  });

  it('should call onRemoveSession when the remove button is clicked', () => {
    const onRemoveSession = vi.fn();
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({ id: 'bg-1', taskDescription: 'Remove me' }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={onRemoveSession}
      />,
    );

    const removeBtn = screen.getByTestId('bg-remove-btn-bg-1');
    fireEvent.click(removeBtn);

    expect(onRemoveSession).toHaveBeenCalledWith('bg-1');
  });

  it('should render multiple background sessions', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({ id: 'bg-1', status: 'running', taskDescription: 'Task Alpha' }),
      'bg-2': createMockSnapshot({ id: 'bg-2', status: 'completed', taskDescription: 'Task Beta' }),
      'bg-3': createMockSnapshot({ id: 'bg-3', status: 'failed', taskDescription: 'Task Gamma' }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    expect(screen.getByText('Task Alpha')).toBeInTheDocument();
    expect(screen.getByText('Task Beta')).toBeInTheDocument();
    expect(screen.getByText('Task Gamma')).toBeInTheDocument();

    // Verify count badge
    expect(screen.getByText('3')).toBeInTheDocument();
  });

  // ---------------------------------------------------------------------------
  // Fork hierarchy (tree) tests — no matching directory (Active Sessions section)
  // ---------------------------------------------------------------------------

  it('should nest forked sessions under their parent', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-parent': createMockSnapshot({
        id: 'bg-parent',
        taskDescription: 'Parent Session',
      }),
      'bg-child': createMockSnapshot({
        id: 'bg-child',
        taskDescription: 'Child Fork',
        parentSessionId: 'bg-parent',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    // Both sessions should be visible
    expect(screen.getByText('Parent Session')).toBeInTheDocument();
    expect(screen.getByText('Child Fork')).toBeInTheDocument();

    // The child should be nested (rendered inside the parent's tree node)
    const parentItem = screen.getByTestId('bg-session-item-bg-parent');
    const childItem = screen.getByTestId('bg-session-item-bg-child');
    expect(parentItem).toBeInTheDocument();
    expect(childItem).toBeInTheDocument();
  });

  it('should nest multiple forks under the same parent', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-parent': createMockSnapshot({
        id: 'bg-parent',
        taskDescription: 'Original Session',
      }),
      'bg-fork-1': createMockSnapshot({
        id: 'bg-fork-1',
        taskDescription: 'Fork 1',
        parentSessionId: 'bg-parent',
      }),
      'bg-fork-2': createMockSnapshot({
        id: 'bg-fork-2',
        taskDescription: 'Fork 2',
        parentSessionId: 'bg-parent',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    expect(screen.getByText('Original Session')).toBeInTheDocument();
    expect(screen.getByText('Fork 1')).toBeInTheDocument();
    expect(screen.getByText('Fork 2')).toBeInTheDocument();

    // Count badge should show total count (3)
    expect(screen.getByText('3')).toBeInTheDocument();
  });

  it('should render orphan sessions (parent does not exist) as root nodes', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-orphan': createMockSnapshot({
        id: 'bg-orphan',
        taskDescription: 'Orphan Session',
        parentSessionId: 'bg-nonexistent',
      }),
      'bg-root': createMockSnapshot({
        id: 'bg-root',
        taskDescription: 'Root Session',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    // Both should render as root-level items
    expect(screen.getByText('Orphan Session')).toBeInTheDocument();
    expect(screen.getByText('Root Session')).toBeInTheDocument();
  });

  it('should show current foreground under parent when foregroundParentSessionId is set', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-parent': createMockSnapshot({
        id: 'bg-parent',
        taskDescription: 'Parent Session',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
        foregroundParentSessionId="bg-parent"
      />,
    );

    expect(screen.getByText('Parent Session')).toBeInTheDocument();
    // The current fork indicator should be visible
    expect(screen.getByTestId('current-fork-indicator')).toBeInTheDocument();
    // Both the label and the badge render "(current)" text
    const currentTexts = screen.getAllByText('(current)');
    expect(currentTexts.length).toBeGreaterThanOrEqual(1);
  });

  it('should render sessions without parentSessionId as root nodes (backward compat)', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({
        id: 'bg-1',
        taskDescription: 'Session A',
        parentSessionId: undefined,
      }),
      'bg-2': createMockSnapshot({
        id: 'bg-2',
        taskDescription: 'Session B',
        parentSessionId: undefined,
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    expect(screen.getByText('Session A')).toBeInTheDocument();
    expect(screen.getByText('Session B')).toBeInTheDocument();
  });

  // ---------------------------------------------------------------------------
  // Fork hierarchy inside workspace directory tree
  // ---------------------------------------------------------------------------

  it('should render bg sessions inside their workspace directory when it matches a pinned dir', () => {
    mockPinnedDirectories = ['/test/project'];

    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({
        id: 'bg-1',
        taskDescription: 'Directory Session',
        workspacePath: '/test/project',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    // Session should be visible inside the directory
    expect(screen.getByText('Directory Session')).toBeInTheDocument();
    // "Active Sessions" section should NOT appear (session matched a directory)
    expect(screen.queryByText('Active Sessions')).not.toBeInTheDocument();
    // Directory node "project" should be visible
    expect(screen.getByText('project')).toBeInTheDocument();
  });

  it('should show fork hierarchy inside a workspace directory', () => {
    mockPinnedDirectories = ['/test/project'];

    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-parent': createMockSnapshot({
        id: 'bg-parent',
        taskDescription: 'Original Session',
        workspacePath: '/test/project',
      }),
      'bg-fork': createMockSnapshot({
        id: 'bg-fork',
        taskDescription: 'Forked Session',
        parentSessionId: 'bg-parent',
        workspacePath: '/test/project',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    // Both sessions visible under the directory
    expect(screen.getByText('Original Session')).toBeInTheDocument();
    expect(screen.getByText('Forked Session')).toBeInTheDocument();
    // No separate "Active Sessions" section
    expect(screen.queryByText('Active Sessions')).not.toBeInTheDocument();
    // Fork is nested under parent
    const parentItem = screen.getByTestId('bg-session-item-bg-parent');
    const forkItem = screen.getByTestId('bg-session-item-bg-fork');
    expect(parentItem).toBeInTheDocument();
    expect(forkItem).toBeInTheDocument();
  });

  it('should show current foreground indicator inside workspace directory tree', () => {
    mockPinnedDirectories = ['/test/project'];

    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-parent': createMockSnapshot({
        id: 'bg-parent',
        taskDescription: 'Parent In Dir',
        workspacePath: '/test/project',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
        foregroundParentSessionId="bg-parent"
      />,
    );

    expect(screen.getByText('Parent In Dir')).toBeInTheDocument();
    expect(screen.getByTestId('current-fork-indicator')).toBeInTheDocument();
    expect(screen.queryByText('Active Sessions')).not.toBeInTheDocument();
  });

  it('should show bg sessions alongside history sessions in the same directory', () => {
    mockPinnedDirectories = ['/test/project'];

    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({
        id: 'bg-1',
        taskDescription: 'Active BG Session',
        workspacePath: '/test/project',
        status: 'running',
      }),
    };

    const historyItem = {
      id: 'hist-1',
      taskDescription: 'Completed History',
      workspacePath: '/test/project',
      startedAt: Date.now() - 60000,
      success: true,
      title: 'Completed History',
      strategy: null as never,
      status: 'completed' as const,
      duration: 5000,
      completedStories: 1,
      totalStories: 1,
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        history={[historyItem]}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    // Both active bg session and history session visible under same directory
    expect(screen.getByText('Active BG Session')).toBeInTheDocument();
    expect(screen.getByText('Completed History')).toBeInTheDocument();
    // Directory badge shows total count (2)
    expect(screen.getByText('2')).toBeInTheDocument();
  });

  it('should hide history item when an active session references originHistoryId', () => {
    mockPinnedDirectories = ['/test/project'];

    const historyItem = {
      id: 'hist-1',
      taskDescription: 'Original History Session',
      workspacePath: '/test/project',
      startedAt: Date.now() - 60000,
      success: true,
      title: 'Original History Session',
      strategy: null as never,
      status: 'completed' as const,
      duration: 5000,
      completedStories: 1,
      totalStories: 1,
    };

    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({
        id: 'bg-1',
        taskDescription: 'Active Fork',
        workspacePath: '/test/project',
        originHistoryId: 'hist-1',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        history={[historyItem]}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    expect(screen.getByText('Active Fork')).toBeInTheDocument();
    expect(screen.queryByText('Original History Session')).not.toBeInTheDocument();
  });

  it('should hide history item when an active background session has the same sessionId', () => {
    mockPinnedDirectories = ['/test/project'];

    const historyItem = {
      id: 'hist-fork-1',
      taskDescription: 'Fork History Session',
      workspacePath: '/test/project',
      startedAt: Date.now() - 30000,
      success: true,
      title: 'Fork History Session',
      strategy: null as never,
      status: 'completed' as const,
      duration: 3000,
      completedStories: 1,
      totalStories: 1,
      sessionId: 'standalone:fork-session-1',
    };

    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-fork': createMockSnapshot({
        id: 'bg-fork',
        taskDescription: 'Active Fork Session',
        workspacePath: '/test/project',
        standaloneSessionId: 'fork-session-1',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        history={[historyItem]}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    expect(screen.getByText('Active Fork Session')).toBeInTheDocument();
    expect(screen.queryByText('Fork History Session')).not.toBeInTheDocument();
  });

  it('should split bg sessions between directory and Active Sessions when only some match', () => {
    mockPinnedDirectories = ['/test/project'];

    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-matched': createMockSnapshot({
        id: 'bg-matched',
        taskDescription: 'Matched Session',
        workspacePath: '/test/project',
      }),
      'bg-unmatched': createMockSnapshot({
        id: 'bg-unmatched',
        taskDescription: 'Unmatched Session',
        // No workspacePath — goes to Active Sessions fallback
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />,
    );

    // Matched session under directory
    expect(screen.getByText('Matched Session')).toBeInTheDocument();
    // Unmatched session in "Active Sessions"
    expect(screen.getByText('Unmatched Session')).toBeInTheDocument();
    expect(screen.getByText('Active Sessions')).toBeInTheDocument();
  });

  // ===========================================================================
  // Ghost Entry (foregroundBgId) rendering
  // ===========================================================================

  it('should render ghost node with foreground styling', () => {
    const ghostId = 'bg-ghost-1';
    const bgSessions: Record<string, SessionSnapshot> = {
      [ghostId]: createMockSnapshot({
        id: ghostId,
        taskDescription: 'Ghost Session',
        status: 'running',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
        foregroundBgId={ghostId}
      />,
    );

    const ghostItem = screen.getByTestId(`bg-session-item-${ghostId}`);
    expect(ghostItem).toBeInTheDocument();
    // Ghost node should have primary background styling
    expect(ghostItem.className).toContain('bg-primary-50');
    // Should show (current) badge
    expect(screen.getByText('(current)')).toBeInTheDocument();
  });

  it('should not show (current) child indicator when foregroundBgId is set', () => {
    const parentId = 'bg-parent-1';
    const bgSessions: Record<string, SessionSnapshot> = {
      [parentId]: createMockSnapshot({
        id: parentId,
        taskDescription: 'Parent Session',
        status: 'running',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
        foregroundParentSessionId={parentId}
        foregroundBgId={parentId}
      />,
    );

    // When foregroundBgId is set, the (current) child indicator should NOT appear
    // The ghost node itself shows (current) instead
    expect(screen.queryByTestId('current-fork-indicator')).not.toBeInTheDocument();
  });

  it('should still show (current) child indicator when foregroundBgId is null', () => {
    const parentId = 'bg-parent-2';
    const bgSessions: Record<string, SessionSnapshot> = {
      [parentId]: createMockSnapshot({
        id: parentId,
        taskDescription: 'Parent Session',
        status: 'running',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
        foregroundParentSessionId={parentId}
        foregroundBgId={null}
      />,
    );

    // Without ghost, the (current) child indicator should appear under parent
    expect(screen.getByTestId('current-fork-indicator')).toBeInTheDocument();
  });

  it('should render children under ghost node correctly', () => {
    const ghostId = 'bg-ghost-2';
    const childId = 'bg-child-1';
    const bgSessions: Record<string, SessionSnapshot> = {
      [ghostId]: createMockSnapshot({
        id: ghostId,
        taskDescription: 'Ghost Parent',
        status: 'running',
      }),
      [childId]: createMockSnapshot({
        id: childId,
        taskDescription: 'Child Session',
        status: 'idle',
        parentSessionId: ghostId,
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
        foregroundBgId={ghostId}
      />,
    );

    // Ghost parent and child should both be rendered
    expect(screen.getByTestId(`bg-session-item-${ghostId}`)).toBeInTheDocument();
    expect(screen.getByTestId(`bg-session-item-${childId}`)).toBeInTheDocument();
    expect(screen.getByText('Child Session')).toBeInTheDocument();
  });

  it('ghost node should not trigger onSwitchSession', () => {
    const ghostId = 'bg-ghost-3';
    const onSwitch = vi.fn();
    const bgSessions: Record<string, SessionSnapshot> = {
      [ghostId]: createMockSnapshot({
        id: ghostId,
        taskDescription: 'Ghost No Click',
        status: 'running',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={onSwitch}
        onRemoveSession={vi.fn()}
        foregroundBgId={ghostId}
      />,
    );

    const ghostItem = screen.getByTestId(`bg-session-item-${ghostId}`);
    fireEvent.click(ghostItem);

    // Ghost node should not trigger switch
    expect(onSwitch).not.toHaveBeenCalled();
  });

  it('ghost node should display currentSessionDescription instead of stale snapshot', () => {
    const ghostId = 'bg-ghost-4';
    const bgSessions: Record<string, SessionSnapshot> = {
      [ghostId]: createMockSnapshot({
        id: ghostId,
        taskDescription: 'Stale description from snapshot',
        status: 'running',
      }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        currentTask="Live foreground description"
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
        foregroundBgId={ghostId}
      />,
    );

    // Ghost node should show the live description, not the stale snapshot description
    const ghostItem = screen.getByTestId(`bg-session-item-${ghostId}`);
    expect(ghostItem.textContent).toContain('Live foreground description');
    expect(ghostItem.textContent).not.toContain('Stale description from snapshot');
  });
});
