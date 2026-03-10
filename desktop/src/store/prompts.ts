/**
 * Prompts Store
 *
 * Manages prompt template state for the Prompt Library.
 * Uses Zustand for state management with Tauri command integration.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import i18n from '../i18n';
import type { PromptTemplate, PromptCreateRequest, PromptUpdateRequest, CommandResponse } from '../types/prompt';

interface PromptsState {
  /** List of all prompts */
  prompts: PromptTemplate[];

  /** Loading state */
  loading: boolean;

  /** Error message */
  error: string | null;

  /** Search query */
  searchQuery: string;

  /** Selected category filter */
  selectedCategory: string | null;

  /** UI panel state */
  panelOpen: boolean;

  /** Dialog open state */
  dialogOpen: boolean;

  /** Palette open state (/ trigger) */
  paletteOpen: boolean;

  /** Currently selected prompt for editing */
  selectedPrompt: PromptTemplate | null;

  /** Content pending insertion into InputBox (set by PromptPanel, consumed by InputBox) */
  pendingInsertContent: string | null;

  /** Actions */
  fetchPrompts: (category?: string, search?: string) => Promise<void>;
  createPrompt: (req: PromptCreateRequest) => Promise<PromptTemplate | null>;
  updatePrompt: (id: string, req: PromptUpdateRequest) => Promise<PromptTemplate | null>;
  deletePrompt: (id: string) => Promise<boolean>;
  recordUse: (id: string) => Promise<void>;
  togglePin: (id: string) => Promise<void>;

  togglePanel: () => void;
  openDialog: (prompt?: PromptTemplate | null) => void;
  closeDialog: () => void;
  openPalette: () => void;
  closePalette: () => void;
  setSearchQuery: (query: string) => void;
  setSelectedCategory: (category: string | null) => void;
  clearError: () => void;
  setPendingInsert: (content: string) => void;
  clearPendingInsert: () => void;
}

export const usePromptsStore = create<PromptsState>((set, get) => ({
  prompts: [],
  loading: false,
  error: null,
  searchQuery: '',
  selectedCategory: null,
  panelOpen: false,
  dialogOpen: false,
  paletteOpen: false,
  selectedPrompt: null,
  pendingInsertContent: null,

  fetchPrompts: async (category?: string, search?: string) => {
    set({
      loading: true,
      error: null,
      selectedCategory: category ?? null,
      searchQuery: search ?? '',
    });

    try {
      const response = await invoke<CommandResponse<PromptTemplate[]>>('list_prompts', {
        category: category !== undefined ? category : null,
        search: search || null,
        locale: i18n.language || null,
      });

      if (response.success && response.data) {
        set({ prompts: response.data, loading: false });
      } else {
        set({ error: response.error || 'Failed to fetch prompts', loading: false });
      }
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : 'Failed to fetch prompts',
        loading: false,
      });
    }
  },

  createPrompt: async (req: PromptCreateRequest) => {
    try {
      const response = await invoke<CommandResponse<PromptTemplate>>('create_prompt', {
        request: req,
      });

      if (response.success && response.data) {
        await get().fetchPrompts(get().selectedCategory ?? undefined, get().searchQuery || undefined);
        return response.data;
      } else {
        set({ error: response.error || 'Failed to create prompt' });
        return null;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to create prompt' });
      return null;
    }
  },

  updatePrompt: async (id: string, req: PromptUpdateRequest) => {
    try {
      const response = await invoke<CommandResponse<PromptTemplate>>('update_prompt', {
        id,
        request: req,
      });

      if (response.success && response.data) {
        await get().fetchPrompts(get().selectedCategory ?? undefined, get().searchQuery || undefined);
        return response.data;
      } else {
        set({ error: response.error || 'Failed to update prompt' });
        return null;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to update prompt' });
      return null;
    }
  },

  deletePrompt: async (id: string) => {
    try {
      const response = await invoke<CommandResponse<void>>('delete_prompt', { id });

      if (response.success) {
        await get().fetchPrompts(get().selectedCategory ?? undefined, get().searchQuery || undefined);
        return true;
      } else {
        set({ error: response.error || 'Failed to delete prompt' });
        return false;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to delete prompt' });
      return false;
    }
  },

  recordUse: async (id: string) => {
    try {
      await invoke<CommandResponse<void>>('record_prompt_use', { id });
      // Update local state without full refetch
      set((state) => ({
        prompts: state.prompts.map((p) => (p.id === id ? { ...p, use_count: p.use_count + 1 } : p)),
      }));
    } catch {
      // Silent fail for usage tracking
    }
  },

  togglePin: async (id: string) => {
    try {
      const response = await invoke<CommandResponse<PromptTemplate>>('toggle_prompt_pin', {
        id,
        locale: i18n.language || null,
      });

      if (response.success && response.data) {
        set((state) => ({
          prompts: [...state.prompts.map((p) => (p.id === id ? response.data! : p))].sort((a, b) => {
            if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
            if (a.use_count !== b.use_count) return b.use_count - a.use_count;
            return a.title.localeCompare(b.title);
          }),
        }));
      }
    } catch {
      // Silent fail for pin toggle
    }
  },

  togglePanel: () => set((s) => ({ panelOpen: !s.panelOpen })),
  openDialog: (prompt = null) => set({ dialogOpen: true, selectedPrompt: prompt ?? null }),
  closeDialog: () => set({ dialogOpen: false, selectedPrompt: null }),
  openPalette: () => set({ paletteOpen: true }),
  closePalette: () => set({ paletteOpen: false }),
  setSearchQuery: (query: string) => set({ searchQuery: query }),
  setSelectedCategory: (category: string | null) => set({ selectedCategory: category }),
  clearError: () => set({ error: null }),
  setPendingInsert: (content: string) => set({ pendingInsertContent: content }),
  clearPendingInsert: () => set({ pendingInsertContent: null }),
}));

export default usePromptsStore;
