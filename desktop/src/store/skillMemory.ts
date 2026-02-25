/**
 * Skill & Memory Store
 *
 * Zustand store managing skills and memories state with IPC actions
 * to the Tauri Rust backend. Provides CRUD operations, filtering,
 * search, and UI toggle state.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  SkillSummary,
  SkillDocument,
  SkillsOverview,
  SkillIndexStats,
  MemoryEntry,
  MemoryCategory,
  MemoryStats,
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
  memoryPage: number;
  memoryPageSize: number;
  memoryHasMore: boolean;

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
    importance?: number
  ) => Promise<void>;
  updateMemory: (
    id: string,
    updates: {
      content?: string;
      category?: MemoryCategory;
      importance?: number;
      keywords?: string[];
    }
  ) => Promise<void>;
  deleteMemory: (id: string) => Promise<void>;
  clearMemories: (projectPath: string) => Promise<void>;
  searchMemories: (projectPath: string, query: string) => Promise<void>;
  runMaintenance: (projectPath: string) => Promise<void>;
  setMemorySearchQuery: (query: string) => void;
  setMemoryCategoryFilter: (filter: MemoryCategoryFilter) => void;

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
  memoryPage: 0,
  memoryPageSize: 20,
  memoryHasMore: true,

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
        set({ skillsError: response.error || 'Failed to load skills', skillsLoading: false });
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
    } catch {
      // Silently fail - overview is optional
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
        get().showToast(response.error || 'Skill not found', 'error');
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
        get().showToast(response.error || 'Failed to toggle skill', 'error');
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
        get().showToast(response.error || 'Failed to toggle skill', 'error');
      }
    } catch (error) {
      set((state) => ({
        skills: state.skills.map((s) => (s.id === id ? { ...s, enabled: !enabled } : s)),
      }));
    }
  },

  searchSkills: async (projectPath: string, query: string) => {
    set({ skillsLoading: true, skillSearchQuery: query });
    try {
      const response = await invoke<CommandResponse<SkillSummary[]>>('list_skills', {
        projectPath,
        sourceFilter: null,
        includeDisabled: true,
      });
      if (response.success && response.data) {
        // Client-side filter by query
        const filtered = query
          ? response.data.filter(
              (s) =>
                s.name.toLowerCase().includes(query.toLowerCase()) ||
                s.description.toLowerCase().includes(query.toLowerCase()) ||
                s.tags.some((t) => t.toLowerCase().includes(query.toLowerCase()))
            )
          : response.data;
        set({ skills: filtered, skillsLoading: false });
      } else {
        set({ skillsError: response.error || 'Search failed', skillsLoading: false });
      }
    } catch (error) {
      set({
        skillsError: error instanceof Error ? error.message : String(error),
        skillsLoading: false,
      });
    }
  },

  refreshSkillIndex: async (projectPath: string) => {
    try {
      const response = await invoke<CommandResponse<SkillIndexStats>>('refresh_skill_index', {
        projectPath,
      });
      if (response.success) {
        get().showToast('Skill index refreshed', 'success');
        // Reload skills list
        await get().loadSkills(projectPath);
      } else {
        get().showToast(response.error || 'Failed to refresh', 'error');
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
        get().showToast('Skill deleted', 'success');
      } else {
        get().showToast(response.error || 'Failed to delete skill', 'error');
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
      const { memoryCategoryFilter, memoryPageSize } = get();
      const category = memoryCategoryFilter === 'all' ? null : memoryCategoryFilter;
      const response = await invoke<CommandResponse<MemoryEntry[]>>('list_project_memories', {
        projectPath,
        category,
        offset: 0,
        limit: memoryPageSize,
      });
      if (response.success && response.data) {
        set({
          memories: response.data,
          memoriesLoading: false,
          memoryHasMore: response.data.length >= memoryPageSize,
        });
      } else {
        set({ memoriesError: response.error || 'Failed to load memories', memoriesLoading: false });
      }
    } catch (error) {
      set({
        memoriesError: error instanceof Error ? error.message : String(error),
        memoriesLoading: false,
      });
    }
  },

  loadMoreMemories: async (projectPath: string) => {
    const { memoryPage, memoryPageSize, memories, memoryCategoryFilter } = get();
    const nextPage = memoryPage + 1;
    const category = memoryCategoryFilter === 'all' ? null : memoryCategoryFilter;
    try {
      const response = await invoke<CommandResponse<MemoryEntry[]>>('list_project_memories', {
        projectPath,
        category,
        offset: nextPage * memoryPageSize,
        limit: memoryPageSize,
      });
      if (response.success && response.data) {
        set({
          memories: [...memories, ...response.data],
          memoryPage: nextPage,
          memoryHasMore: response.data.length >= memoryPageSize,
        });
      }
    } catch {
      // Silently fail for pagination
    }
  },

  loadMemoryStats: async (projectPath: string) => {
    try {
      const response = await invoke<CommandResponse<MemoryStats>>('get_memory_stats', {
        projectPath,
      });
      if (response.success && response.data) {
        set({ memoryStats: response.data });
      }
    } catch {
      // Silently fail - stats are optional
    }
  },

  addMemory: async (
    projectPath: string,
    category: MemoryCategory,
    content: string,
    keywords: string[],
    importance?: number
  ) => {
    try {
      const response = await invoke<CommandResponse<MemoryEntry>>('add_project_memory', {
        projectPath,
        category,
        content,
        keywords,
        importance: importance ?? 0.5,
      });
      if (response.success && response.data) {
        set((state) => ({
          memories: [response.data!, ...state.memories],
        }));
        get().showToast('Memory added', 'success');
      } else {
        get().showToast(response.error || 'Failed to add memory', 'error');
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
        get().showToast('Memory updated', 'success');
      } else {
        get().showToast(response.error || 'Failed to update memory', 'error');
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
        get().showToast('Memory deleted', 'success');
      } else {
        get().showToast(response.error || 'Failed to delete memory', 'error');
      }
    } catch (error) {
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  clearMemories: async (projectPath: string) => {
    try {
      const response = await invoke<CommandResponse<number>>('clear_project_memories', {
        projectPath,
      });
      if (response.success) {
        set({ memories: [], memoryStats: null, memoryHasMore: false });
        get().showToast('All memories cleared', 'success');
      } else {
        get().showToast(response.error || 'Failed to clear memories', 'error');
      }
    } catch (error) {
      get().showToast(error instanceof Error ? error.message : String(error), 'error');
    }
  },

  searchMemories: async (projectPath: string, query: string) => {
    set({ memoriesLoading: true, memorySearchQuery: query });
    if (!query.trim()) {
      await get().loadMemories(projectPath);
      return;
    }
    try {
      const { memoryCategoryFilter } = get();
      const categories =
        memoryCategoryFilter === 'all' ? null : [memoryCategoryFilter];
      const response = await invoke<CommandResponse<Array<{ entry: MemoryEntry; relevance_score: number }>>>(
        'search_project_memories',
        {
          projectPath,
          query,
          categories,
          topK: 50,
        }
      );
      if (response.success && response.data) {
        set({
          memories: response.data.map((r) => r.entry),
          memoriesLoading: false,
          memoryHasMore: false,
        });
      } else {
        set({ memoriesError: response.error || 'Search failed', memoriesLoading: false });
      }
    } catch (error) {
      set({
        memoriesError: error instanceof Error ? error.message : String(error),
        memoriesLoading: false,
      });
    }
  },

  runMaintenance: async (projectPath: string) => {
    try {
      await invoke<CommandResponse<{ decayed_count: number; pruned_count: number; compacted_count: number }>>('run_memory_maintenance', {
        projectPath,
      });
      // Silent success — maintenance is non-critical
    } catch {
      // Silent failure — maintenance is non-critical
    }
  },

  setMemorySearchQuery: (query: string) => set({ memorySearchQuery: query }),
  setMemoryCategoryFilter: (filter: MemoryCategoryFilter) => set({ memoryCategoryFilter: filter }),

  // --- UI Actions ---

  togglePanel: () => set((state) => ({ panelOpen: !state.panelOpen })),

  openDialog: (tab?: SkillMemoryTab) =>
    set({ dialogOpen: true, activeTab: tab ?? get().activeTab }),

  closeDialog: () => set({ dialogOpen: false }),

  setActiveTab: (tab: SkillMemoryTab) => set({ activeTab: tab }),

  showToast: (message: string, type: 'success' | 'error' | 'info' = 'info') =>
    set({ toastMessage: message, toastType: type }),

  clearToast: () => set({ toastMessage: null }),

  reset: () => set(defaultState),
}));

export default useSkillMemoryStore;
