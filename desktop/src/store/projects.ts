/**
 * Projects Store
 *
 * Manages project and session state for the Projects browser.
 * Uses Zustand for state management with Tauri command integration.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  Project,
  Session,
  SessionDetails,
  ResumeResult,
  ProjectSortBy,
  CommandResponse,
} from '../types/project';

interface ProjectsState {
  /** List of all projects */
  projects: Project[];

  /** Currently selected project */
  selectedProject: Project | null;

  /** Sessions for selected project */
  sessions: Session[];

  /** Currently selected session */
  selectedSession: Session | null;

  /** Session details when viewing a session */
  sessionDetails: SessionDetails | null;

  /** Current sort option */
  sortBy: ProjectSortBy;

  /** Search query */
  searchQuery: string;

  /** Loading states */
  loading: {
    projects: boolean;
    sessions: boolean;
    details: boolean;
  };

  /** Error messages */
  error: string | null;

  /** Actions */
  fetchProjects: (sortBy?: ProjectSortBy) => Promise<void>;
  selectProject: (project: Project | null) => Promise<void>;
  fetchSessions: (projectPath: string) => Promise<void>;
  selectSession: (session: Session | null) => Promise<void>;
  fetchSessionDetails: (sessionPath: string) => Promise<void>;
  resumeSession: (sessionPath: string) => Promise<ResumeResult | null>;
  searchProjects: (query: string) => Promise<void>;
  searchSessions: (projectPath: string, query: string) => Promise<void>;
  setSortBy: (sortBy: ProjectSortBy) => void;
  setSearchQuery: (query: string) => void;
  clearError: () => void;
}

export const useProjectsStore = create<ProjectsState>((set, get) => ({
  projects: [],
  selectedProject: null,
  sessions: [],
  selectedSession: null,
  sessionDetails: null,
  sortBy: 'recent_activity',
  searchQuery: '',
  loading: {
    projects: false,
    sessions: false,
    details: false,
  },
  error: null,

  fetchProjects: async (sortBy?: ProjectSortBy) => {
    const sort = sortBy || get().sortBy;
    set((state) => ({
      loading: { ...state.loading, projects: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<Project[]>>('list_projects', {
        sortBy: sort,
        limit: 100,
        offset: 0,
      });

      if (response.success && response.data) {
        set((state) => ({
          projects: response.data!,
          sortBy: sort,
          loading: { ...state.loading, projects: false },
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to fetch projects',
          loading: { ...state.loading, projects: false },
        }));
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to fetch projects',
        loading: { ...state.loading, projects: false },
      }));
    }
  },

  selectProject: async (project: Project | null) => {
    set({ selectedProject: project, selectedSession: null, sessionDetails: null });

    if (project) {
      await get().fetchSessions(project.path);
    } else {
      set({ sessions: [] });
    }
  },

  fetchSessions: async (projectPath: string) => {
    set((state) => ({
      loading: { ...state.loading, sessions: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<Session[]>>('list_sessions', {
        projectPath,
      });

      if (response.success && response.data) {
        set((state) => ({
          sessions: response.data!,
          loading: { ...state.loading, sessions: false },
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to fetch sessions',
          loading: { ...state.loading, sessions: false },
        }));
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to fetch sessions',
        loading: { ...state.loading, sessions: false },
      }));
    }
  },

  selectSession: async (session: Session | null) => {
    set({ selectedSession: session, sessionDetails: null });

    if (session) {
      await get().fetchSessionDetails(session.file_path);
    }
  },

  fetchSessionDetails: async (sessionPath: string) => {
    set((state) => ({
      loading: { ...state.loading, details: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<SessionDetails>>('get_session', {
        sessionPath,
      });

      if (response.success && response.data) {
        set((state) => ({
          sessionDetails: response.data,
          loading: { ...state.loading, details: false },
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to fetch session details',
          loading: { ...state.loading, details: false },
        }));
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to fetch session details',
        loading: { ...state.loading, details: false },
      }));
    }
  },

  resumeSession: async (sessionPath: string) => {
    try {
      const response = await invoke<CommandResponse<ResumeResult>>('resume_session', {
        sessionPath,
      });

      if (response.success && response.data) {
        return response.data;
      } else {
        set({ error: response.error || 'Failed to resume session' });
        return null;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to resume session' });
      return null;
    }
  },

  searchProjects: async (query: string) => {
    set((state) => ({
      loading: { ...state.loading, projects: true },
      searchQuery: query,
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<Project[]>>('search_projects', {
        query,
      });

      if (response.success && response.data) {
        set((state) => ({
          projects: response.data!,
          loading: { ...state.loading, projects: false },
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to search projects',
          loading: { ...state.loading, projects: false },
        }));
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to search projects',
        loading: { ...state.loading, projects: false },
      }));
    }
  },

  searchSessions: async (projectPath: string, query: string) => {
    set((state) => ({
      loading: { ...state.loading, sessions: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<Session[]>>('search_sessions', {
        projectPath,
        query,
      });

      if (response.success && response.data) {
        set((state) => ({
          sessions: response.data!,
          loading: { ...state.loading, sessions: false },
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to search sessions',
          loading: { ...state.loading, sessions: false },
        }));
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to search sessions',
        loading: { ...state.loading, sessions: false },
      }));
    }
  },

  setSortBy: (sortBy: ProjectSortBy) => {
    set({ sortBy });
    get().fetchProjects(sortBy);
  },

  setSearchQuery: (query: string) => {
    set({ searchQuery: query });
  },

  clearError: () => {
    set({ error: null });
  },
}));

export default useProjectsStore;
