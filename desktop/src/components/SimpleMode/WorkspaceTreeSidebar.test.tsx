/**
 * WorkspaceTreeSidebar - Background Sessions Tests
 *
 * Story 008: Verify that background sessions are rendered separately from history
 * with status indicators, switch/remove actions, and proper i18n labels.
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

// Mock settings store
vi.mock('../../store/settings', () => ({
  useSettingsStore: vi.fn((selector) => {
    const state = {
      workspacePath: '/test/project',
      pinnedDirectories: [],
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
  });

  it('should NOT render "Active Sessions" section when there are no background sessions', () => {
    render(<WorkspaceTreeSidebar {...defaultProps} backgroundSessions={{}} />);

    expect(screen.queryByText('Active Sessions')).not.toBeInTheDocument();
  });

  it('should render "Active Sessions" section when background sessions exist', () => {
    const bgSessions: Record<string, SessionSnapshot> = {
      'bg-1': createMockSnapshot({ id: 'bg-1', taskDescription: 'Build API endpoint' }),
    };

    render(
      <WorkspaceTreeSidebar
        {...defaultProps}
        backgroundSessions={bgSessions}
        onSwitchSession={vi.fn()}
        onRemoveSession={vi.fn()}
      />
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
      />
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
      />
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
      />
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
      />
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
      />
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
      />
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
      />
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
      />
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
      />
    );

    expect(screen.getByText('Task Alpha')).toBeInTheDocument();
    expect(screen.getByText('Task Beta')).toBeInTheDocument();
    expect(screen.getByText('Task Gamma')).toBeInTheDocument();

    // Verify count badge
    expect(screen.getByText('3')).toBeInTheDocument();
  });
});
