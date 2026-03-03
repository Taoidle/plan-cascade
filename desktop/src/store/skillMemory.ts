/**
 * Skill & Memory Store
 *
 * Zustand store managing skills and memories state with IPC actions
 * to the Tauri Rust backend. Provides CRUD operations, filtering,
 * search, and UI toggle state.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import i18n from '../i18n';
import { reportNonFatal } from '../lib/nonFatal';
import type {
  SkillSummary,
  SkillDocument,
  SkillsOverview,
  SkillIndexStats,
  MemoryEntry,
  MemoryCategory,
  MemoryScope,
  MemoryStats,
  MemoryReviewCandidate,
  MemoryReviewDecision,
  SkillMatch,
  SkillSourceLabel,
} from '../types/skillMemory';

// ============================================================================
// CommandResponse wrapper (mirrors Rust CommandResponse<T>)
// ============================================================================

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

interface UnifiedMemoryQueryResultV2 {
  trace_id: string;
  degraded: boolean;
  candidate_count: number;
  results: Array<{
    entry: MemoryEntry;
    relevance_score: number;
  }>;
}

interface MemoryReviewSummaryV2 {
  updated: number;
}

function memoryScopesForRequest(scope: MemoryScope, sessionId: string | null): MemoryScope[] {
  if (scope === 'session') {
    return sessionId?.trim() ? ['session'] : [];
  }
  return [scope];
}

function memoryErrorWithTrace(message: string): string {
  const traceId =
    typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
      ? crypto.randomUUID()
      : `memory-${Date.now()}`;
  return `${message} (trace_id: ${traceId}, retry available)`;
}

function tSkillMemory(key: string, defaultValue: string): string {
  const translated = i18n.t(key, { ns: 'simpleMode', defaultValue });
  return typeof translated === 'string' && translated !== key ? translated : defaultValue;
}

// ============================================================================
// State Types
// ============================================================================

/** Active source filter for the skills tab */
export type SkillSourceFilter = SkillSourceLabel | 'all';

/** Active category filter for the memory tab */
export type MemoryCategoryFilter = MemoryCategory | 'all';

/** Tab selection in the management dialog */
export type SkillMemoryTab = 'skills' | 'memory';

interface SkillMemoryState {
  // --- Skills State ---
  skills: SkillSummary[];
  skillsLoading: boolean;
  skillsError: string | null;
  skillDetail: SkillDocument | null;
  skillDetailLoading: boolean;
  skillsOverview: SkillsOverview | null;
  skillSearchQuery: string;
  skillSourceFilter: SkillSourceFilter;

  // --- Memory State ---
  memories: MemoryEntry[];
  memoriesLoading: boolean;
  memoriesError: string | null;
  memoryStats: MemoryStats | null;
  memorySearchQuery: string;
  memoryCategoryFilter: MemoryCategoryFilter;
  memoryScope: MemoryScope;
  memorySessionId: string | null;
  memoryPage: number;
  memoryPageSize: number;
  memoryHasMore: boolean;
  pendingMemoryCandidates: MemoryReviewCandidate[];
  pendingMemoryCandidatesLoading: boolean;

  // --- UI State ---
  panelOpen: boolean;
  dialogOpen: boolean;
  activeTab: SkillMemoryTab;
  toastMessage: string | null;
  toastType: 'success' | 'error' | 'info';

  // --- Skill Actions ---
  loadSkills: (projectPath: string) => Promise<void>;
  loadSkillsOverview: (projectPath: string) => Promise<void>;
  loadSkillDetail: (projectPath: string, id: string) => Promise<void>;
  toggleSkill: (id: string, enabled: boolean) => Promise<void>;
  toggleGeneratedSkill: (id: string, enabled: boolean) => Promise<void>;
  searchSkills: (projectPath: string, query: string) => Promise<void>;
  refreshSkillIndex: (projectPath: string) => Promise<void>;
  deleteSkill: (id: string, projectPath: string) => Promise<void>;
  setSkillSearchQuery: (query: string) => void;
  setSkillSourceFilter: (filter: SkillSourceFilter) => void;

  // --- Memory Actions ---
  loadMemories: (projectPath: string) => Promise<void>;
  loadMoreMemories: (projectPath: string) => Promise<void>;
  loadMemoryStats: (projectPath: string) => Promise<void>;
  addMemory: (
    projectPath: string,
    category: MemoryCategory,
    content: string,
    keywords: string[],
    importance?: number,
  ) => Promise<void>;
  updateMemory: (
    id: string,
    updates: {
      content?: string;
      category?: MemoryCategory;
      importance?: number;
      keywords?: string[];
    },
  ) => Promise<void>;
  deleteMemory: (id: string) => Promise<void>;
  clearMemories: (projectPath: string) => Promise<void>;
  searchMemories: (projectPath: string, query: string) => Promise<void>;
  loadPendingMemoryCandidates: (projectPath: string, limit?: number) => Promise<void>;
  reviewPendingMemoryCandidates: (
    projectPath: string,
    memoryIds: string[],
    decision: MemoryReviewDecision,
  ) => Promise<void>;
  runMaintenance: (projectPath: string) => Promise<void>;
  setMemorySearchQuery: (query: string) => void;
  setMemoryCategoryFilter: (filter: MemoryCategoryFilter) => void;
  setMemoryScope: (scope: MemoryScope) => void;
  setMemorySessionId: (sessionId: string | null) => void;

  // --- UI Actions ---
  togglePanel: () => void;
  openDialog: (tab?: SkillMemoryTab) => void;
  closeDialog: () => void;
  setActiveTab: (tab: SkillMemoryTab) => void;
  showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
  clearToast: () => void;
  reset: () => void;
}

// ============================================================================
// Default State
// ============================================================================

const defaultState = {
  skills: [] as SkillSummary[],
  skillsLoading: false,
  skillsError: null as string | null,
  skillDetail: null as SkillDocument | null,
  skillDetailLoading: false,
  skillsOverview: null as SkillsOverview | null,
  skillSearchQuery: '',
  skillSourceFilter: 'all' as SkillSourceFilter,

  memories: [] as MemoryEntry[],
  memoriesLoading: false,
  memoriesError: null as string | null,
  memoryStats: null as MemoryStats | null,
  memorySearchQuery: '',
  memoryCategoryFilter: 'all' as MemoryCategoryFilter,
  memoryScope: 'project' as MemoryScope,
  memorySessionId: null as string | null,
  memoryPage: 0,
  memoryPageSize: 20,
  memoryHasMore: true,
  pendingMemoryCandidates: [] as MemoryReviewCandidate[],
  pendingMemoryCandidatesLoading: false,

  panelOpen: false,
  dialogOpen: false,
  activeTab: 'skills' as SkillMemoryTab,
  toastMessage: null as string | null,
  toastType: 'info' as const,
};

// ============================================================================
// Store
// ============================================================================

export const useSkillMemoryStore = create<SkillMemoryState>()((set, get) => ({
  ...defaultState,

  // --- Skill Actions ---

  loadSkills: async (projectPath: string) => {
    set({ skillsLoading: true, skillsError: null });
    try {
      const response = await invoke<CommandResponse<SkillSummary[]>>('list_skills', {
        projectPath,
        sourceFilter: null,
        includeDisabled: true,
      });
      if (response.success && response.data) {
        set({ skills: response.data, skillsLoading: false });
      } else {
        set({
          skillsError: response.error || tSkillMemory('skillPanel.toasts.loadSkillsFailed', 'Failed to load skills'),
          skillsLoading: false,
        });
      }
    } catch (error) {
      set({
        skillsError: error instanceof Error ? error.message : String(error),
        skillsLoading: false,
      });
    }
  },

  loadSkillsOverview: async (projectPath: string) => {
    try {
      const response = await invoke<CommandResponse<SkillsOverview>>('get_skills_overview', {
        projectPath,
      });
      if (response.success && response.data) {
        set({ skillsOverview: response.data });
      }
    } catch (error) {
      reportNonFatal('skillMemory.loadSkillsOverview', error, { projectPath });
    }
  },

  loadSkillDetail: async (projectPath: string, id: string) => {
    set({ skillDetailLoading: true, skillDetail: null });
    try {
      const response = await invoke<CommandResponse<SkillDocument>>('get_skill', {
        projectPath,
        id,
      });
      if (response.success && response.data) {
        set({ skillDetail: response.data, skillDetailLoading: false });
      } else {
        set({ skillDetailLoading: false });
        get().showToast(response.error || tSkillMemory('skillPanel.toasts.skillNotFound', 'Skill not found'), 'error');
      }
    } catch (error) {
      set({ skillDetailLoading: false });
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  toggleSkill: async (id: string, enabled: boolean) => {
    // Optimistic update
    set((state) => ({
      skills: state.skills.map((s) => (s.id === id ? { ...s, enabled } : s)),
    }));
    try {
      const response = await invoke<CommandResponse<void>>('toggle_skill', {
        id,
        enabled,
      });
      if (!response.success) {
        // Revert on failure
        set((state) => ({
          skills: state.skills.map((s) => (s.id === id ? { ...s, enabled: !enabled } : s)),
        }));
        get().showToast(
          response.error || tSkillMemory('skillPanel.toasts.toggleSkillFailed', 'Failed to toggle skill'),
          'error',
        );
      }
    } catch (error) {
      set((state) => ({
        skills: state.skills.map((s) => (s.id === id ? { ...s, enabled: !enabled } : s)),
      }));
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  toggleGeneratedSkill: async (id: string, enabled: boolean) => {
    set((state) => ({
      skills: state.skills.map((s) => (s.id === id ? { ...s, enabled } : s)),
    }));
    try {
      const response = await invoke<CommandResponse<void>>('toggle_generated_skill', {
        id,
        enabled,
      });
      if (!response.success) {
        set((state) => ({
          skills: state.skills.map((s) => (s.id === id ? { ...s, enabled: !enabled } : s)),
        }));
        get().showToast(
          response.error || tSkillMemory('skillPanel.toasts.toggleSkillFailed', 'Failed to toggle skill'),
          'error',
        );
      }
    } catch (error) {
      set((state) => ({
        skills: state.skills.map((s) => (s.id === id ? { ...s, enabled: !enabled } : s)),
      }));
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  searchSkills: async (projectPath: string, query: string) => {
    set({ skillsLoading: true, skillSearchQuery: query });
    if (!query.trim()) {
      await get().loadSkills(projectPath);
      return;
    }

    const fallbackToClientFilter = async () => {
      const fallback = await invoke<CommandResponse<SkillSummary[]>>('list_skills', {
        projectPath,
        sourceFilter: null,
        includeDisabled: true,
      });
      if (fallback.success && fallback.data) {
        const filtered = fallback.data.filter(
          (s) =>
            s.name.toLowerCase().includes(query.toLowerCase()) ||
            s.description.toLowerCase().includes(query.toLowerCase()) ||
            s.tags.some((t) => t.toLowerCase().includes(query.toLowerCase())),
        );
        set({ skills: filtered, skillsLoading: false });
      } else {
        set({
          skillsError: fallback.error || tSkillMemory('skillPanel.toasts.searchSkillsFailed', 'Search failed'),
          skillsLoading: false,
        });
      }
    };

    try {
      const response = await invoke<CommandResponse<SkillMatch[]>>('search_skills', {
        projectPath,
        query,
        topK: 50,
      });
      if (response.success && response.data) {
        set({ skills: response.data.map((m) => m.skill), skillsLoading: false });
      } else {
        await fallbackToClientFilter();
      }
    } catch (error) {
      try {
        await fallbackToClientFilter();
      } catch (fallbackError) {
        reportNonFatal('skillMemory.searchSkillsFallback', fallbackError, { projectPath, query });
        set({
          skillsError: error instanceof Error ? error.message : String(error),
          skillsLoading: false,
        });
      }
    }
  },

  refreshSkillIndex: async (projectPath: string) => {
    try {
      const response = await invoke<CommandResponse<SkillIndexStats>>('refresh_skill_index', {
        projectPath,
      });
      if (response.success) {
        get().showToast(tSkillMemory('skillPanel.toasts.skillIndexRefreshed', 'Skill index refreshed'), 'success');
        // Reload skills list
        await get().loadSkills(projectPath);
      } else {
        get().showToast(
          response.error || tSkillMemory('skillPanel.toasts.refreshSkillsFailed', 'Failed to refresh'),
          'error',
        );
      }
    } catch (error) {
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  deleteSkill: async (id: string, projectPath: string) => {
    try {
      const response = await invoke<CommandResponse<void>>('delete_skill', {
        id,
        projectPath,
      });
      if (response.success) {
        set((state) => ({
          skills: state.skills.filter((s) => s.id !== id),
        }));
        get().showToast(tSkillMemory('skillPanel.toasts.skillDeleted', 'Skill deleted'), 'success');
      } else {
        get().showToast(
          response.error || tSkillMemory('skillPanel.toasts.deleteSkillFailed', 'Failed to delete skill'),
          'error',
        );
      }
    } catch (error) {
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  setSkillSearchQuery: (query: string) => set({ skillSearchQuery: query }),
  setSkillSourceFilter: (filter: SkillSourceFilter) => set({ skillSourceFilter: filter }),

  // --- Memory Actions ---

  loadMemories: async (projectPath: string) => {
    set({ memoriesLoading: true, memoriesError: null, memoryPage: 0 });
    try {
      const { memoryCategoryFilter, memoryPageSize, memoryScope, memorySessionId } = get();
      if (memoryScope === 'session' && !memorySessionId?.trim()) {
        set({
          memoriesError: tSkillMemory(
            'skillPanel.toasts.sessionMemoryRequiresSession',
            'Session memory requires an active session',
          ),
          memoriesLoading: false,
          memories: [],
          memoryHasMore: false,
        });
        return;
      }
      const category = memoryCategoryFilter === 'all' ? null : memoryCategoryFilter;
      const response = await invoke<CommandResponse<MemoryEntry[]>>('list_memory_entries_v2', {
        projectPath,
        categories: category ? [category] : null,
        scopes: memoryScopesForRequest(memoryScope, memorySessionId),
        statuses: ['active'],
        offset: 0,
        limit: memoryPageSize,
        sessionId: memorySessionId,
      });
      if (response.success && response.data) {
        set({
          memories: response.data,
          memoriesLoading: false,
          memoryHasMore: response.data.length >= memoryPageSize,
        });
      } else {
        set({
          memoriesError:
            response.error || tSkillMemory('skillPanel.toasts.loadMemoriesFailed', 'Failed to load memories'),
          memoriesLoading: false,
        });
        get().showToast(
          memoryErrorWithTrace(
            response.error || tSkillMemory('skillPanel.toasts.loadMemoriesFailed', 'Failed to load memories'),
          ),
          'error',
        );
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({
        memoriesError: message,
        memoriesLoading: false,
      });
      get().showToast(memoryErrorWithTrace(message), 'error');
    }
  },

  loadMoreMemories: async (projectPath: string) => {
    const { memoryPage, memoryPageSize, memories, memoryCategoryFilter, memoryScope, memorySessionId } = get();
    if (memoryScope === 'session' && !memorySessionId?.trim()) {
      return;
    }
    const nextPage = memoryPage + 1;
    const category = memoryCategoryFilter === 'all' ? null : memoryCategoryFilter;
    try {
      const response = await invoke<CommandResponse<MemoryEntry[]>>('list_memory_entries_v2', {
        projectPath,
        categories: category ? [category] : null,
        scopes: memoryScopesForRequest(memoryScope, memorySessionId),
        statuses: ['active'],
        offset: nextPage * memoryPageSize,
        limit: memoryPageSize,
        sessionId: memorySessionId,
      });
      if (response.success && response.data) {
        set({
          memories: [...memories, ...response.data],
          memoryPage: nextPage,
          memoryHasMore: response.data.length >= memoryPageSize,
        });
      } else if (response.error) {
        get().showToast(memoryErrorWithTrace(response.error), 'error');
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      get().showToast(memoryErrorWithTrace(message), 'error');
    }
  },

  loadMemoryStats: async (projectPath: string) => {
    try {
      const { memoryScope, memorySessionId } = get();
      if (memoryScope === 'session' && !memorySessionId?.trim()) {
        set({ memoryStats: null });
        return;
      }
      const response = await invoke<CommandResponse<MemoryStats>>('memory_stats_v2', {
        projectPath,
        scopes: memoryScopesForRequest(memoryScope, memorySessionId),
        statuses: ['active'],
        sessionId: memorySessionId,
      });
      if (response.success && response.data) {
        set({ memoryStats: response.data });
      } else if (response.error) {
        get().showToast(memoryErrorWithTrace(response.error), 'error');
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      get().showToast(memoryErrorWithTrace(message), 'error');
    }
  },

  addMemory: async (
    projectPath: string,
    category: MemoryCategory,
    content: string,
    keywords: string[],
    importance?: number,
  ) => {
    try {
      const { memoryScope, memorySessionId } = get();
      if (memoryScope === 'session' && !memorySessionId?.trim()) {
        get().showToast(
          tSkillMemory('skillPanel.toasts.sessionMemoryRequiresSession', 'Session memory requires an active session'),
          'error',
        );
        return;
      }
      const response = await invoke<CommandResponse<MemoryEntry>>('add_project_memory', {
        projectPath,
        category,
        content,
        keywords,
        importance: importance ?? 0.5,
        scope: memoryScope,
        sessionId: memorySessionId,
      });
      if (response.success && response.data) {
        set((state) => ({
          memories: [response.data!, ...state.memories],
        }));
        get().showToast(tSkillMemory('skillPanel.toasts.memoryAdded', 'Memory added'), 'success');
      } else {
        get().showToast(
          response.error || tSkillMemory('skillPanel.toasts.addMemoryFailed', 'Failed to add memory'),
          'error',
        );
      }
    } catch (error) {
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  updateMemory: async (id: string, updates) => {
    try {
      const response = await invoke<CommandResponse<MemoryEntry>>('update_project_memory', {
        id,
        content: updates.content ?? null,
        category: updates.category ?? null,
        importance: updates.importance ?? null,
        keywords: updates.keywords ?? null,
      });
      if (response.success && response.data) {
        set((state) => ({
          memories: state.memories.map((m) => (m.id === id ? response.data! : m)),
        }));
        get().showToast(tSkillMemory('skillPanel.toasts.memoryUpdated', 'Memory updated'), 'success');
      } else {
        get().showToast(
          response.error || tSkillMemory('skillPanel.toasts.updateMemoryFailed', 'Failed to update memory'),
          'error',
        );
      }
    } catch (error) {
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  deleteMemory: async (id: string) => {
    try {
      const response = await invoke<CommandResponse<void>>('delete_project_memory', { id });
      if (response.success) {
        set((state) => ({
          memories: state.memories.filter((m) => m.id !== id),
        }));
        get().showToast(tSkillMemory('skillPanel.toasts.memoryDeleted', 'Memory deleted'), 'success');
      } else {
        get().showToast(
          response.error || tSkillMemory('skillPanel.toasts.deleteMemoryFailed', 'Failed to delete memory'),
          'error',
        );
      }
    } catch (error) {
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  clearMemories: async (projectPath: string) => {
    try {
      const { memoryScope, memorySessionId } = get();
      if (memoryScope === 'session' && !memorySessionId?.trim()) {
        get().showToast(
          tSkillMemory('skillPanel.toasts.sessionMemoryRequiresSession', 'Session memory requires an active session'),
          'error',
        );
        return;
      }
      const response = await invoke<CommandResponse<number>>('clear_project_memories', {
        projectPath,
        scope: memoryScope,
        sessionId: memorySessionId,
      });
      if (response.success) {
        set({ memories: [], memoryStats: null, memoryHasMore: false });
        get().showToast(tSkillMemory('skillPanel.toasts.memoriesCleared', 'All memories cleared'), 'success');
      } else {
        get().showToast(
          response.error || tSkillMemory('skillPanel.toasts.clearMemoriesFailed', 'Failed to clear memories'),
          'error',
        );
      }
    } catch (error) {
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  searchMemories: async (projectPath: string, query: string) => {
    set({ memoriesLoading: true, memorySearchQuery: query });
    const { memoryScope, memorySessionId } = get();
    if (memoryScope === 'session' && !memorySessionId?.trim()) {
      set({
        memoriesError: tSkillMemory(
          'skillPanel.toasts.sessionMemoryRequiresSession',
          'Session memory requires an active session',
        ),
        memoriesLoading: false,
        memories: [],
        memoryHasMore: false,
      });
      return;
    }
    if (!query.trim()) {
      await get().loadMemories(projectPath);
      return;
    }
    try {
      const { memoryCategoryFilter } = get();
      const categories = memoryCategoryFilter === 'all' ? null : [memoryCategoryFilter];
      const response = await invoke<CommandResponse<UnifiedMemoryQueryResultV2>>('query_memory_entries_v2', {
        projectPath,
        query,
        categories,
        scopes: memoryScopesForRequest(memoryScope, memorySessionId),
        includeIds: [],
        excludeIds: [],
        statuses: ['active'],
        sessionId: memorySessionId,
        topKTotal: 50,
        minImportance: 0.1,
        enableSemantic: true,
        enableLexical: true,
      });
      if (response.success && response.data) {
        set({
          memories: response.data.results.map((r) => r.entry),
          memoriesLoading: false,
          memoryHasMore: false,
        });
        if (response.data.degraded) {
          get().showToast(
            `Memory search degraded (trace_id: ${response.data.trace_id}). Retry after ranker recovers.`,
            'info',
          );
        }
      } else {
        set({
          memoriesError: response.error || tSkillMemory('skillPanel.toasts.searchMemoriesFailed', 'Search failed'),
          memoriesLoading: false,
        });
        get().showToast(
          memoryErrorWithTrace(
            response.error || tSkillMemory('skillPanel.toasts.searchMemoriesFailed', 'Search failed'),
          ),
          'error',
        );
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({
        memoriesError: message,
        memoriesLoading: false,
      });
      get().showToast(memoryErrorWithTrace(message), 'error');
    }
  },

  loadPendingMemoryCandidates: async (projectPath: string, limit = 200) => {
    set({ pendingMemoryCandidatesLoading: true });
    try {
      const { memoryScope, memorySessionId } = get();
      if (memoryScope === 'session' && !memorySessionId?.trim()) {
        set({ pendingMemoryCandidates: [], pendingMemoryCandidatesLoading: false });
        return;
      }
      const response = await invoke<CommandResponse<MemoryReviewCandidate[]>>('list_pending_memory_candidates_v2', {
        projectPath,
        scopes: memoryScopesForRequest(memoryScope, memorySessionId),
        sessionId: memorySessionId,
        limit,
      });
      if (response.success && response.data) {
        set({
          pendingMemoryCandidates: response.data,
          pendingMemoryCandidatesLoading: false,
        });
      } else {
        set({ pendingMemoryCandidatesLoading: false });
        if (response.error) {
          get().showToast(memoryErrorWithTrace(response.error), 'error');
        }
      }
    } catch (error) {
      set({ pendingMemoryCandidatesLoading: false });
      const message = error instanceof Error ? error.message : String(error);
      get().showToast(memoryErrorWithTrace(message), 'error');
    }
  },

  reviewPendingMemoryCandidates: async (projectPath: string, memoryIds: string[], decision: MemoryReviewDecision) => {
    if (memoryIds.length === 0) {
      return;
    }
    try {
      const response = await invoke<CommandResponse<MemoryReviewSummaryV2>>('review_memory_candidates_v2', {
        memoryIds,
        decision,
      });
      if (!response.success) {
        get().showToast(
          memoryErrorWithTrace(
            response.error ||
              tSkillMemory('skillPanel.toasts.memoryReviewFailed', 'Failed to review memory candidates'),
          ),
          'error',
        );
        return;
      }

      const updated = response.data?.updated ?? memoryIds.length;
      const successMessage =
        decision === 'approve'
          ? tSkillMemory('skillPanel.toasts.memoryApproveSuccess', `Approved ${updated} memory candidates`)
          : decision === 'reject'
            ? tSkillMemory('skillPanel.toasts.memoryRejectSuccess', `Rejected ${updated} memory candidates`)
            : tSkillMemory('skillPanel.toasts.memoryArchiveSuccess', `Archived ${updated} memory candidates`);
      get().showToast(successMessage, 'success');
      await Promise.all([
        get().loadPendingMemoryCandidates(projectPath),
        get().loadMemories(projectPath),
        get().loadMemoryStats(projectPath),
      ]);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      get().showToast(memoryErrorWithTrace(message), 'error');
    }
  },

  runMaintenance: async (projectPath: string) => {
    try {
      const { memoryScope, memorySessionId } = get();
      if (memoryScope === 'session' && !memorySessionId?.trim()) {
        return;
      }
      await invoke<CommandResponse<{ decayed_count: number; pruned_count: number; compacted_count: number }>>(
        'run_memory_maintenance',
        {
          projectPath,
          scope: memoryScope,
          sessionId: memorySessionId,
        },
      );
      // Silent success — maintenance is non-critical
    } catch (error) {
      reportNonFatal('skillMemory.runMaintenance', error, { projectPath });
    }
  },

  setMemorySearchQuery: (query: string) => set({ memorySearchQuery: query }),
  setMemoryCategoryFilter: (filter: MemoryCategoryFilter) => set({ memoryCategoryFilter: filter }),
  setMemoryScope: (scope: MemoryScope) => set({ memoryScope: scope, memoryPage: 0, memoryHasMore: true }),
  setMemorySessionId: (sessionId: string | null) => set({ memorySessionId: sessionId }),

  // --- UI Actions ---

  togglePanel: () => set((state) => ({ panelOpen: !state.panelOpen })),

  openDialog: (tab?: SkillMemoryTab) => set({ dialogOpen: true, activeTab: tab ?? get().activeTab }),

  closeDialog: () => set({ dialogOpen: false }),

  setActiveTab: (tab: SkillMemoryTab) => set({ activeTab: tab }),

  showToast: (message: string, type: 'success' | 'error' | 'info' = 'info') =>
    set({ toastMessage: message, toastType: type }),

  clearToast: () => set({ toastMessage: null }),

  reset: () => set(defaultState),
}));

export default useSkillMemoryStore;
