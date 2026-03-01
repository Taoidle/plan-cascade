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
  SearchHit,
  CodeSearchMode,
  ContextItem,
} from '../lib/codebaseApi';
import {
  addCodebaseContext,
  listCodebaseProjects,
  getCodebaseDetail,
  listCodebaseFiles,
  deleteCodebaseProject,
  searchCodebase,
  triggerReindex,
  getIndexStatus,
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
  searchResults: SearchHit[];
  contextItems: ContextItem[];

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
  searchProject: (path: string, query: string, topK?: number, modes?: CodeSearchMode[]) => Promise<void>;
  setFilesPage: (page: number) => void;
  setFilesLanguageFilter: (lang: string | null) => void;
  setFilesSearchPattern: (pattern: string) => void;
  addContextItem: (item: ContextItem) => void;
  removeContextItem: (index: number) => void;
  clearContextItems: () => void;
  pushContextToMode: (targetMode: 'simple' | 'expert' | 'task' | 'plan' | 'chat') => Promise<void>;
  setError: (message: string | null) => void;
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
  contextItems: [],

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
    const res = await triggerReindex(path);
    if (!res.success) {
      set({ error: res.error ?? 'Failed to trigger reindex' });
      return;
    }

    // Poll status so UI updates are event-driven, not a blind timeout.
    const maxAttempts = 12;
    for (let i = 0; i < maxAttempts; i++) {
      const status = await getIndexStatus(path);
      if (status.success && status.data) {
        const s = status.data.status;
        if (s !== 'idle') {
          await get().loadProjectDetail(path);
          return;
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 250));
    }

    // Fallback refresh even if status polling did not observe transition.
    await get().loadProjectDetail(path);
  },

  searchProject: async (path, query, topK, modes) => {
    set({ searchLoading: true, searchResults: [] });
    const v2 = await searchCodebase({
      project_path: path,
      query,
      modes: modes ?? ['hybrid'],
      limit: topK ?? 20,
      include_snippet: true,
    });
    if (v2.success && v2.data) {
      set({ searchResults: v2.data.hits, searchLoading: false });
      return;
    }
    set({
      searchLoading: false,
      error: v2.error ?? 'Search failed',
    });
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

  addContextItem: (item) => {
    set((state) => {
      const exists = state.contextItems.some(
        (it) =>
          it.type === item.type &&
          it.project_path === item.project_path &&
          it.file_path === item.file_path &&
          (it.symbol_name ?? null) === (item.symbol_name ?? null) &&
          (it.line_start ?? null) === (item.line_start ?? null) &&
          (it.line_end ?? null) === (item.line_end ?? null),
      );
      if (exists) return state;
      return { contextItems: [...state.contextItems, item] };
    });
  },

  removeContextItem: (index) => {
    set((state) => ({
      contextItems: state.contextItems.filter((_, i) => i !== index),
    }));
  },

  clearContextItems: () => {
    set({ contextItems: [] });
  },

  pushContextToMode: async (targetMode) => {
    const { contextItems } = get();
    if (contextItems.length === 0) {
      return;
    }
    const res = await addCodebaseContext(targetMode, contextItems);
    if (!res.success) {
      set({ error: res.error ?? 'Failed to add context' });
      return;
    }
    set({ contextItems: [] });
  },

  setError: (message) => {
    set({ error: message });
  },

  clearSearch: () => {
    set({ searchResults: [] });
  },

  clearError: () => {
    set({ error: null });
  },
}));
