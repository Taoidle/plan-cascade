/**
 * Projects Component Tests
 *
 * Tests project listing, project selection, session list rendering,
 * and empty/error states.
 *
 * Story 009: React Component Test Coverage
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Projects } from '../Projects/index';
import { createMockProject, createMockSession } from './test-utils';
import type { Project, Session } from '../../types/project';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { count?: number }) => {
      const translations: Record<string, string> = {
        'projects.title': 'Projects',
        'projects.searchPlaceholder': 'Search projects...',
        'projects.sortBy': 'Sort by',
        'projects.sort.recentActivity': 'Recent Activity',
        'projects.sort.name': 'Name',
        'projects.sort.sessionCount': 'Session Count',
        'projects.selectProject': 'Select a project to view sessions',
        'projects.noProjects': 'No projects found',
        'projects.noSearchResults': 'No results found',
        'projects.sessions': `${opts?.count || 0} sessions`,
        'projects.searchSessions': 'Search sessions...',
        'projects.noSessions': 'No sessions',
        'common.retry': 'Retry',
      };
      return translations[key] || key;
    },
  }),
}));

// Shared mock functions
const mockFetchProjects = vi.fn();
const mockSelectProject = vi.fn();
const mockSelectSession = vi.fn();
const mockFetchSessions = vi.fn();
const mockSearchProjects = vi.fn();
const mockSearchSessions = vi.fn();
const mockSetSortBy = vi.fn();
const mockSetSearchQuery = vi.fn();
const mockResumeSession = vi.fn();

let mockProjectsState: {
  projects: Project[];
  selectedProject: Project | null;
  sessions: Session[];
  selectedSession: Session | null;
  sessionDetails: null;
  sortBy: string;
  searchQuery: string;
  loading: { projects: boolean; sessions: boolean; details: boolean };
  error: string | null;
  fetchProjects: typeof mockFetchProjects;
  selectProject: typeof mockSelectProject;
  selectSession: typeof mockSelectSession;
  fetchSessions: typeof mockFetchSessions;
  searchProjects: typeof mockSearchProjects;
  searchSessions: typeof mockSearchSessions;
  setSortBy: typeof mockSetSortBy;
  setSearchQuery: typeof mockSetSearchQuery;
  resumeSession: typeof mockResumeSession;
};

function resetMockProjectsState() {
  mockProjectsState = {
    projects: [],
    selectedProject: null,
    sessions: [],
    selectedSession: null,
    sessionDetails: null,
    sortBy: 'recent_activity',
    searchQuery: '',
    loading: { projects: false, sessions: false, details: false },
    error: null,
    fetchProjects: mockFetchProjects,
    selectProject: mockSelectProject,
    selectSession: mockSelectSession,
    fetchSessions: mockFetchSessions,
    searchProjects: mockSearchProjects,
    searchSessions: mockSearchSessions,
    setSortBy: mockSetSortBy,
    setSearchQuery: mockSetSearchQuery,
    resumeSession: mockResumeSession,
  };
}

vi.mock('../../store/projects', () => ({
  useProjectsStore: () => mockProjectsState,
}));

// Mock Radix Select to avoid portal issues
vi.mock('@radix-ui/react-select', () => ({
  Root: ({ children, value }: { children: React.ReactNode; value: string; onValueChange: (v: string) => void }) => (
    <div data-testid="select-root" data-value={value}>{children}</div>
  ),
  Trigger: ({ children }: { children: React.ReactNode }) => <button data-testid="select-trigger">{children}</button>,
  Value: () => <span data-testid="select-value">Value</span>,
  Icon: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  Portal: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Content: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Viewport: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Item: ({ children, value }: { children: React.ReactNode; value: string }) => (
    <div data-testid={`select-item-${value}`}>{children}</div>
  ),
  ItemText: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  ItemIndicator: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
}));

// Mock sub-components with minimal implementations for isolation
vi.mock('../Projects/ProjectCard', () => ({
  ProjectCard: ({ project, isSelected, onClick }: { project: Project; isSelected: boolean; onClick: () => void }) => (
    <div
      data-testid={`project-card-${project.id}`}
      data-selected={isSelected}
      onClick={onClick}
      role="button"
    >
      <span>{project.name}</span>
      <span>{project.session_count} sessions</span>
    </div>
  ),
}));

vi.mock('../Projects/SessionCard', () => ({
  SessionCard: ({ session, isSelected, onClick, onResume }: {
    session: Session; isSelected: boolean; onClick: () => void; onResume: () => void;
  }) => (
    <div
      data-testid={`session-card-${session.id}`}
      data-selected={isSelected}
      onClick={onClick}
    >
      <span>{session.first_message_preview || 'No preview'}</span>
      <button onClick={onResume}>Resume</button>
    </div>
  ),
}));

vi.mock('../Projects/ProjectSkeleton', () => ({
  ProjectSkeleton: () => <div data-testid="project-skeleton">Loading...</div>,
}));

vi.mock('../Projects/SessionSkeleton', () => ({
  SessionSkeleton: () => <div data-testid="session-skeleton">Loading...</div>,
}));

vi.mock('../Projects/SessionDetails', () => ({
  SessionDetails: () => <div data-testid="session-details">Session Details View</div>,
}));

vi.mock('../Projects/utils', () => ({
  debounce: (fn: (...args: unknown[]) => void) => fn,
}));

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

describe('Projects', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetMockProjectsState();
  });

  it('renders the two-panel layout with project browser', () => {
    render(<Projects />);

    expect(screen.getByText('Projects')).toBeInTheDocument();
    expect(screen.getByText('Select a project to view sessions')).toBeInTheDocument();
  });

  it('shows loading skeletons while projects are being fetched', () => {
    mockProjectsState.loading = { projects: true, sessions: false, details: false };

    render(<Projects />);

    const skeletons = screen.getAllByTestId('project-skeleton');
    expect(skeletons.length).toBe(3);
  });

  it('renders project cards when projects are loaded', () => {
    const projects = [
      createMockProject({ id: 'proj-1', name: 'Project Alpha', session_count: 3 }),
      createMockProject({ id: 'proj-2', name: 'Project Beta', session_count: 7 }),
      createMockProject({ id: 'proj-3', name: 'Project Gamma', session_count: 1 }),
    ];
    mockProjectsState.projects = projects;

    render(<Projects />);

    expect(screen.getByTestId('project-card-proj-1')).toBeInTheDocument();
    expect(screen.getByTestId('project-card-proj-2')).toBeInTheDocument();
    expect(screen.getByTestId('project-card-proj-3')).toBeInTheDocument();
    expect(screen.getByText('Project Alpha')).toBeInTheDocument();
    expect(screen.getByText('Project Beta')).toBeInTheDocument();
  });

  it('calls selectProject when a project card is clicked', () => {
    const project = createMockProject({ id: 'proj-1', name: 'My Project' });
    mockProjectsState.projects = [project];

    render(<Projects />);

    fireEvent.click(screen.getByTestId('project-card-proj-1'));

    expect(mockSelectProject).toHaveBeenCalledWith(project);
  });

  it('highlights the selected project card', () => {
    const project = createMockProject({ id: 'proj-1' });
    mockProjectsState.projects = [project];
    mockProjectsState.selectedProject = project;

    render(<Projects />);

    const card = screen.getByTestId('project-card-proj-1');
    expect(card).toHaveAttribute('data-selected', 'true');
  });

  it('shows empty state when no projects exist', () => {
    mockProjectsState.projects = [];

    render(<Projects />);

    expect(screen.getByText('No projects found')).toBeInTheDocument();
  });

  it('displays error message and retry button on fetch error', () => {
    mockProjectsState.error = 'Failed to load projects';

    render(<Projects />);

    expect(screen.getByText('Failed to load projects')).toBeInTheDocument();
    expect(screen.getByText('Retry')).toBeInTheDocument();
  });

  it('renders session list when a project is selected', () => {
    const project = createMockProject({ id: 'proj-1', name: 'My Project' });
    const sessions = [
      createMockSession({ id: 'sess-1', first_message_preview: 'Add user auth' }),
      createMockSession({ id: 'sess-2', first_message_preview: 'Fix bug in parser' }),
    ];
    mockProjectsState.selectedProject = project;
    mockProjectsState.sessions = sessions;

    render(<Projects />);

    expect(screen.getByText('My Project')).toBeInTheDocument();
    expect(screen.getByTestId('session-card-sess-1')).toBeInTheDocument();
    expect(screen.getByTestId('session-card-sess-2')).toBeInTheDocument();
    expect(screen.getByText('Add user auth')).toBeInTheDocument();
    expect(screen.getByText('Fix bug in parser')).toBeInTheDocument();
  });

  it('fetches projects on mount', () => {
    render(<Projects />);

    expect(mockFetchProjects).toHaveBeenCalled();
  });

  it('displays project search input', () => {
    render(<Projects />);

    expect(screen.getByPlaceholderText('Search projects...')).toBeInTheDocument();
  });
});
