/**
 * Codebase Index Store
 *
 * Zustand store for managing workspace codebase index browsing,
 * file listing, and semantic search from the Codebase panel.
 */

import { create } from 'zustand';
import type {
  IndexedProjectEntry,
  CodebaseProjectDetail,
  FileIndexRow,
  SemanticSearchResult,
} from '../lib/codebaseApi';
import {
  listCodebaseProjects,
  getCodebaseDetail,
  listCodebaseFiles,
  deleteCodebaseProject,
  searchCodebase,
  triggerReindex,
} from '../lib/codebaseApi';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

const PAGE_SIZE = 50;

export interface CodebaseState {
  // Data
  projects: IndexedProjectEntry[];
  selectedProjectPath: string | null;
  projectDetail: CodebaseProjectDetail | null;
  files: FileIndexRow[];
  filesTotalCount: number;
  searchResults: SemanticSearchResult[];

  // UI state
  loading: boolean;
  detailLoading: boolean;
  filesLoading: boolean;
  searchLoading: boolean;
  error: string | null;

  // Filters
  filesPage: number;
  filesLanguageFilter: string | null;
  filesSearchPattern: string;

  // Actions
  loadProjects: () => Promise<void>;
  selectProject: (path: string | null) => void;
  loadProjectDetail: (path: string) => Promise<void>;
  loadFiles: (path: string) => Promise<void>;
  deleteProject: (path: string) => Promise<void>;
  reindexProject: (path: string) => Promise<void>;
  searchProject: (path: string, query: string, topK?: number) => Promise<void>;
  setFilesPage: (page: number) => void;
  setFilesLanguageFilter: (lang: string | null) => void;
  setFilesSearchPattern: (pattern: string) => void;
  clearSearch: () => void;
  clearError: () => void;
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useCodebaseStore = create<CodebaseState>()((set, get) => ({
  projects: [],
  selectedProjectPath: null,
  projectDetail: null,
  files: [],
  filesTotalCount: 0,
  searchResults: [],

  loading: false,
  detailLoading: false,
  filesLoading: false,
  searchLoading: false,
  error: null,

  filesPage: 0,
  filesLanguageFilter: null,
  filesSearchPattern: '',

  loadProjects: async () => {
    set({ loading: true, error: null });
    const res = await listCodebaseProjects();
    if (res.success && res.data) {
      set({ projects: res.data, loading: false });
    } else {
      set({ loading: false, error: res.error ?? 'Failed to load projects' });
    }
  },

  selectProject: (path) => {
    set({
      selectedProjectPath: path,
      projectDetail: null,
      files: [],
      filesTotalCount: 0,
      searchResults: [],
      filesPage: 0,
      filesLanguageFilter: null,
      filesSearchPattern: '',
    });
    if (path) {
      get().loadProjectDetail(path);
      get().loadFiles(path);
    }
  },

  loadProjectDetail: async (path) => {
    set({ detailLoading: true });
    const res = await getCodebaseDetail(path);
    if (res.success && res.data) {
      set({ projectDetail: res.data, detailLoading: false });
    } else {
      set({ detailLoading: false, error: res.error ?? 'Failed to load detail' });
    }
  },

  loadFiles: async (path) => {
    const { filesPage, filesLanguageFilter, filesSearchPattern } = get();
    set({ filesLoading: true });
    const res = await listCodebaseFiles(path, {
      languageFilter: filesLanguageFilter,
      searchPattern: filesSearchPattern || null,
      offset: filesPage * PAGE_SIZE,
      limit: PAGE_SIZE,
    });
    if (res.success && res.data) {
      set({
        files: res.data.files,
        filesTotalCount: res.data.total,
        filesLoading: false,
      });
    } else {
      set({ filesLoading: false, error: res.error ?? 'Failed to load files' });
    }
  },

  deleteProject: async (path) => {
    const res = await deleteCodebaseProject(path);
    if (res.success) {
      const { selectedProjectPath } = get();
      set((state) => ({
        projects: state.projects.filter((p) => p.project_path !== path),
        ...(selectedProjectPath === path
          ? {
              selectedProjectPath: null,
              projectDetail: null,
              files: [],
              filesTotalCount: 0,
              searchResults: [],
            }
          : {}),
      }));
    } else {
      set({ error: res.error ?? 'Failed to delete project' });
    }
  },

  reindexProject: async (path) => {
    await triggerReindex(path);
    // Reload detail after a brief delay to let indexing start
    setTimeout(() => {
      get().loadProjectDetail(path);
    }, 1000);
  },

  searchProject: async (path, query, topK) => {
    set({ searchLoading: true, searchResults: [] });
    const res = await searchCodebase(path, query, topK);
    if (res.success && res.data) {
      set({ searchResults: res.data, searchLoading: false });
    } else {
      set({ searchLoading: false, error: res.error ?? 'Search failed' });
    }
  },

  setFilesPage: (page) => {
    set({ filesPage: page });
    const { selectedProjectPath } = get();
    if (selectedProjectPath) get().loadFiles(selectedProjectPath);
  },

  setFilesLanguageFilter: (lang) => {
    set({ filesLanguageFilter: lang, filesPage: 0 });
    const { selectedProjectPath } = get();
    if (selectedProjectPath) get().loadFiles(selectedProjectPath);
  },

  setFilesSearchPattern: (pattern) => {
    set({ filesSearchPattern: pattern, filesPage: 0 });
    const { selectedProjectPath } = get();
    if (selectedProjectPath) get().loadFiles(selectedProjectPath);
  },

  clearSearch: () => {
    set({ searchResults: [] });
  },

  clearError: () => {
    set({ error: null });
  },
}));
