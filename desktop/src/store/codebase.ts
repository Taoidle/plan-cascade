/**
 * Codebase Index Store
 *
 * Zustand store for managing workspace codebase index browsing,
 * file listing, and semantic search from the Codebase panel.
 */

import { create } from 'zustand';
import type {
  IndexedProjectStatusEntry,
  CodebaseProjectDetail,
  FileIndexRow,
  SearchHit,
  CodeSearchMode,
  ContextItem,
  IndexStatusEvent,
  CodebaseContextAppendResult,
} from '../lib/codebaseApi';
import {
  addCodebaseContext,
  listCodebaseProjects,
  listCodebaseProjectsV2,
  getCodebaseDetail,
  listCodebaseFiles,
  deleteCodebaseProject,
  searchCodebase,
  triggerReindex,
  getIndexStatus,
} from '../lib/codebaseApi';
import { useWorkflowKernelStore } from './workflowKernel';
import type { WorkflowMode } from '../types/workflowKernel';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

const PAGE_SIZE = 50;
const DEFAULT_SCOPE_STATUS = 'idle';

interface ProjectRequestVersion {
  detail: number;
  files: number;
  search: number;
}

interface ProjectScopedCodebaseState {
  detail: CodebaseProjectDetail | null;
  files: FileIndexRow[];
  filesTotalCount: number;
  searchResults: SearchHit[];
  status: IndexStatusEvent;
  requestVersion: ProjectRequestVersion;
}

export interface ContextPushSummary {
  appendedCount: number;
  contextRefIds: string[];
  sessionId: string;
  targetMode: 'chat' | 'plan' | 'task';
}

export interface CodebaseState {
  // Data
  projects: IndexedProjectStatusEntry[];
  byProjectPath: Record<string, ProjectScopedCodebaseState>;
  selectedProjectPath: string | null;
  projectDetail: CodebaseProjectDetail | null;
  files: FileIndexRow[];
  filesTotalCount: number;
  searchResults: SearchHit[];
  contextItems: ContextItem[];
  lastContextPush: ContextPushSummary | null;
  requestVersion: number;

  // UI state
  loading: boolean;
  detailLoading: boolean;
  filesLoading: boolean;
  searchLoading: boolean;
  contextPushLoading: boolean;
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
  pushContextToMode: (targetMode: 'chat' | 'plan' | 'task') => Promise<void>;
  setError: (message: string | null) => void;
  clearSearch: () => void;
  clearError: () => void;
}

function makeFallbackStatus(projectPath: string, fileCount: number): IndexStatusEvent {
  const derivedStatus: IndexStatusEvent['status'] = fileCount > 0 ? 'indexed' : DEFAULT_SCOPE_STATUS;
  return {
    project_path: projectPath,
    status: derivedStatus,
    indexed_files: fileCount,
    total_files: fileCount,
    error_message: null,
    total_symbols: 0,
    embedding_chunks: 0,
    embedding_provider_name: null,
    lsp_enrichment: 'none',
    phase: 'done',
    job_id: null,
    updated_at: null,
  };
}

function createProjectScope(projectPath: string, status?: IndexStatusEvent): ProjectScopedCodebaseState {
  return {
    detail: null,
    files: [],
    filesTotalCount: 0,
    searchResults: [],
    status: status ?? makeFallbackStatus(projectPath, 0),
    requestVersion: {
      detail: 0,
      files: 0,
      search: 0,
    },
  };
}

function pickScope(state: CodebaseState, path: string): ProjectScopedCodebaseState {
  return state.byProjectPath[path] ?? createProjectScope(path);
}

function toContextPushSummary(result: CodebaseContextAppendResult): ContextPushSummary {
  return {
    appendedCount: result.appended_count,
    contextRefIds: result.context_ref_ids,
    sessionId: result.session_id,
    targetMode: result.target_mode as 'chat' | 'plan' | 'task',
  };
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useCodebaseStore = create<CodebaseState>()((set, get) => ({
  projects: [],
  byProjectPath: {},
  selectedProjectPath: null,
  projectDetail: null,
  files: [],
  filesTotalCount: 0,
  searchResults: [],
  contextItems: [],
  lastContextPush: null,
  requestVersion: 0,

  loading: false,
  detailLoading: false,
  filesLoading: false,
  searchLoading: false,
  contextPushLoading: false,
  error: null,

  filesPage: 0,
  filesLanguageFilter: null,
  filesSearchPattern: '',

  loadProjects: async () => {
    set({ loading: true, error: null });
    const v2 = await listCodebaseProjectsV2();
    if (v2.success && v2.data) {
      const projectsV2 = v2.data;
      set((state) => {
        const nextByProjectPath = { ...state.byProjectPath };
        for (const project of projectsV2) {
          const current =
            nextByProjectPath[project.project_path] ?? createProjectScope(project.project_path, project.status);
          nextByProjectPath[project.project_path] = {
            ...current,
            status: project.status,
            detail: current.detail ? { ...current.detail, status: project.status } : current.detail,
          };
        }
        return {
          projects: projectsV2,
          byProjectPath: nextByProjectPath,
          loading: false,
        };
      });
      return;
    }

    const legacy = await listCodebaseProjects();
    if (legacy.success && legacy.data) {
      const mappedProjects: IndexedProjectStatusEntry[] = legacy.data.map((project) => ({
        ...project,
        status: makeFallbackStatus(project.project_path, project.file_count),
      }));
      set((state) => {
        const nextByProjectPath = { ...state.byProjectPath };
        for (const project of mappedProjects) {
          const current =
            nextByProjectPath[project.project_path] ?? createProjectScope(project.project_path, project.status);
          nextByProjectPath[project.project_path] = {
            ...current,
            status: project.status,
            detail: current.detail ? { ...current.detail, status: project.status } : current.detail,
          };
        }
        return {
          projects: mappedProjects,
          byProjectPath: nextByProjectPath,
          loading: false,
        };
      });
      return;
    }

    set({
      loading: false,
      error: v2.error ?? legacy.error ?? 'Failed to load projects',
    });
  },

  selectProject: (path) => {
    const scope = path ? pickScope(get(), path) : null;
    set({
      selectedProjectPath: path,
      projectDetail: scope?.detail ?? null,
      files: scope?.files ?? [],
      filesTotalCount: scope?.filesTotalCount ?? 0,
      searchResults: scope?.searchResults ?? [],
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
    const requestVersion = get().requestVersion + 1;
    set((state) => {
      const scope = pickScope(state, path);
      return {
        requestVersion,
        detailLoading: true,
        byProjectPath: {
          ...state.byProjectPath,
          [path]: {
            ...scope,
            requestVersion: {
              ...scope.requestVersion,
              detail: requestVersion,
            },
          },
        },
      };
    });

    const res = await getCodebaseDetail(path);
    if ((get().byProjectPath[path]?.requestVersion.detail ?? -1) !== requestVersion) {
      return;
    }

    if (res.success && res.data) {
      const detailData = res.data;
      set((state) => {
        const scope = pickScope(state, path);
        const nextScope: ProjectScopedCodebaseState = {
          ...scope,
          detail: detailData,
          status: detailData.status,
        };

        const shouldProjectDetail = state.selectedProjectPath === path;
        return {
          byProjectPath: {
            ...state.byProjectPath,
            [path]: nextScope,
          },
          projectDetail: shouldProjectDetail ? detailData : state.projectDetail,
          detailLoading: false,
        };
      });
    } else {
      set({ detailLoading: false, error: res.error ?? 'Failed to load detail' });
    }
  },

  loadFiles: async (path) => {
    const { filesPage, filesLanguageFilter, filesSearchPattern } = get();
    const requestVersion = get().requestVersion + 1;
    set((state) => {
      const scope = pickScope(state, path);
      return {
        requestVersion,
        filesLoading: true,
        byProjectPath: {
          ...state.byProjectPath,
          [path]: {
            ...scope,
            requestVersion: {
              ...scope.requestVersion,
              files: requestVersion,
            },
          },
        },
      };
    });

    const res = await listCodebaseFiles(path, {
      languageFilter: filesLanguageFilter,
      searchPattern: filesSearchPattern || null,
      offset: filesPage * PAGE_SIZE,
      limit: PAGE_SIZE,
    });
    if ((get().byProjectPath[path]?.requestVersion.files ?? -1) !== requestVersion) {
      return;
    }

    if (res.success && res.data) {
      const filesData = res.data;
      set((state) => {
        const scope = pickScope(state, path);
        const nextScope: ProjectScopedCodebaseState = {
          ...scope,
          files: filesData.files,
          filesTotalCount: filesData.total,
        };
        const isSelected = state.selectedProjectPath === path;
        return {
          byProjectPath: {
            ...state.byProjectPath,
            [path]: nextScope,
          },
          files: isSelected ? filesData.files : state.files,
          filesTotalCount: isSelected ? filesData.total : state.filesTotalCount,
          filesLoading: false,
        };
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
        byProjectPath: Object.fromEntries(
          Object.entries(state.byProjectPath).filter(([projectPath]) => projectPath !== path),
        ),
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
          set((state) => {
            const scope = pickScope(state, path);
            return {
              byProjectPath: {
                ...state.byProjectPath,
                [path]: {
                  ...scope,
                  status: status.data as IndexStatusEvent,
                },
              },
            };
          });
          await get().loadProjects();
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
    const requestVersion = get().requestVersion + 1;
    set((state) => {
      const scope = pickScope(state, path);
      return {
        requestVersion,
        searchLoading: true,
        searchResults: state.selectedProjectPath === path ? [] : state.searchResults,
        byProjectPath: {
          ...state.byProjectPath,
          [path]: {
            ...scope,
            searchResults: [],
            requestVersion: {
              ...scope.requestVersion,
              search: requestVersion,
            },
          },
        },
      };
    });

    const v2 = await searchCodebase({
      project_path: path,
      query,
      modes: modes ?? ['hybrid'],
      limit: topK ?? 20,
      include_snippet: true,
    });
    if ((get().byProjectPath[path]?.requestVersion.search ?? -1) !== requestVersion) {
      return;
    }

    if (v2.success && v2.data) {
      const searchData = v2.data;
      set((state) => {
        const scope = pickScope(state, path);
        const nextScope: ProjectScopedCodebaseState = {
          ...scope,
          searchResults: searchData.hits,
        };
        const isSelected = state.selectedProjectPath === path;
        return {
          byProjectPath: {
            ...state.byProjectPath,
            [path]: nextScope,
          },
          searchResults: isSelected ? searchData.hits : state.searchResults,
          searchLoading: false,
        };
      });
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
    set({ contextPushLoading: true, error: null });

    const workflowState = useWorkflowKernelStore.getState();
    let sessionId = workflowState.sessionId;

    if (!sessionId || workflowState.activeMode !== targetMode) {
      const transitioned = await workflowState.transitionMode(targetMode as WorkflowMode);
      if (!transitioned?.sessionId) {
        const latestWorkflowState = useWorkflowKernelStore.getState();
        set({
          contextPushLoading: false,
          error: latestWorkflowState.error ?? 'Failed to open workflow session',
        });
        return;
      }
      sessionId = transitioned.sessionId;
    }

    const payloadItems = contextItems.map((item) => ({
      ...item,
      source: item.source ?? 'codebase',
      session_id: sessionId,
      target_mode: targetMode,
    }));
    const res = await addCodebaseContext(targetMode, payloadItems, sessionId);
    if (!res.success) {
      set({ contextPushLoading: false, error: res.error ?? 'Failed to add context' });
      return;
    }
    set({
      contextItems: [],
      contextPushLoading: false,
      lastContextPush: res.data ? toContextPushSummary(res.data) : null,
    });
  },

  setError: (message) => {
    set({ error: message });
  },

  clearSearch: () => {
    set((state) => {
      const selected = state.selectedProjectPath;
      if (!selected) {
        return { searchResults: [] };
      }
      const scope = pickScope(state, selected);
      return {
        searchResults: [],
        byProjectPath: {
          ...state.byProjectPath,
          [selected]: {
            ...scope,
            searchResults: [],
          },
        },
      };
    });
  },

  clearError: () => {
    set({ error: null });
  },
}));
