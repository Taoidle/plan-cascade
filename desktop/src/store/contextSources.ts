/**
 * Context Sources Store
 *
 * Zustand store for user-controlled domain knowledge injection.
 * Manages which Knowledge collections/documents, Memory categories/items,
 * and Skills are selected for injection into Chat Mode and Task Mode prompts.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { KnowledgeCollection, DocumentSummary } from '../lib/knowledgeApi';
import { ragListCollections, ragListDocuments } from '../lib/knowledgeApi';
import type { MemoryEntry, MemoryStats, MemorySearchResult, SkillSummary } from '../types/skillMemory';
import { useProjectsStore } from './projects';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

/** Configuration sent to the backend for conditional context injection. */
export interface ContextSourceConfig {
  /** Project ID for knowledge base queries (e.g. "default" or a UUID). */
  project_id: string;
  knowledge?: {
    enabled: boolean;
    selected_collections: string[];
    selected_documents: string[];
  };
  memory?: {
    enabled: boolean;
    selected_categories: string[];
    selected_memory_ids: string[];
  };
  skills?: {
    enabled: boolean;
    selected_skill_ids: string[];
  };
}

export interface ContextSourcesState {
  // === Knowledge State ===
  knowledgeEnabled: boolean;
  selectedCollections: string[];
  selectedDocuments: string[];
  availableCollections: KnowledgeCollection[];
  collectionDocuments: Record<string, DocumentSummary[]>;
  isLoadingCollections: boolean;
  isLoadingDocuments: Record<string, boolean>;

  // === Memory State ===
  memoryEnabled: boolean;
  selectedMemoryCategories: string[];
  selectedMemoryIds: string[];
  availableMemoryStats: MemoryStats | null;
  categoryMemories: Record<string, MemoryEntry[]>;
  isLoadingMemoryStats: boolean;
  isLoadingCategoryMemories: Record<string, boolean>;
  memoryPickerSearchQuery: string;
  memorySearchResults: MemoryEntry[] | null;
  isSearchingMemories: boolean;

  // === Skills State ===
  skillsEnabled: boolean;
  selectedSkillIds: string[];
  availableSkills: SkillSummary[];
  isLoadingSkills: boolean;
  skillPickerSearchQuery: string;

  // === Knowledge Actions ===
  toggleKnowledge: (enabled: boolean) => void;
  toggleCollection: (collectionId: string) => void;
  toggleDocument: (collectionId: string, documentId: string) => void;
  selectAllInCollection: (collectionId: string) => void;
  deselectAllInCollection: (collectionId: string) => void;
  loadCollections: (projectId: string) => Promise<void>;
  loadDocuments: (collectionId: string) => Promise<void>;

  // === Memory Actions ===
  toggleMemory: (enabled: boolean) => void;
  toggleMemoryCategory: (category: string) => void;
  toggleMemoryItem: (memoryId: string) => void;
  loadMemoryStats: (projectPath: string) => Promise<void>;
  loadCategoryMemories: (projectPath: string, category: string) => Promise<void>;
  searchMemoriesForPicker: (projectPath: string, query: string) => Promise<void>;
  clearMemorySearch: () => void;

  // === Skills Actions ===
  toggleSkills: (enabled: boolean) => void;
  toggleSkillItem: (skillId: string) => void;
  toggleSkillGroup: (sourceType: string) => void;
  loadAvailableSkills: (projectPath: string) => Promise<void>;
  setSkillPickerSearchQuery: (query: string) => void;

  /** Build the config object for backend invocation. Returns undefined if nothing enabled. */
  buildConfig: () => ContextSourceConfig | undefined;
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useContextSourcesStore = create<ContextSourcesState>()((set, get) => ({
  // === Knowledge State ===
  knowledgeEnabled: false,
  selectedCollections: [],
  selectedDocuments: [],
  availableCollections: [],
  collectionDocuments: {},
  isLoadingCollections: false,
  isLoadingDocuments: {},

  // === Memory State ===
  memoryEnabled: false,
  selectedMemoryCategories: [],
  selectedMemoryIds: [],
  availableMemoryStats: null,
  categoryMemories: {},
  isLoadingMemoryStats: false,
  isLoadingCategoryMemories: {},
  memoryPickerSearchQuery: '',
  memorySearchResults: null,
  isSearchingMemories: false,

  // === Skills State ===
  skillsEnabled: false,
  selectedSkillIds: [],
  availableSkills: [],
  isLoadingSkills: false,
  skillPickerSearchQuery: '',

  // =========================================================================
  // Knowledge Actions
  // =========================================================================

  toggleKnowledge: (enabled) => {
    set({ knowledgeEnabled: enabled });
    if (!enabled) {
      set({ selectedCollections: [], selectedDocuments: [] });
    }
  },

  toggleCollection: (collectionId) => {
    const { selectedCollections, collectionDocuments, selectedDocuments } = get();
    const isSelected = selectedCollections.includes(collectionId);

    if (isSelected) {
      const docs = collectionDocuments[collectionId] || [];
      const docIds = new Set(docs.map((d) => d.document_id));
      set({
        selectedCollections: selectedCollections.filter((id) => id !== collectionId),
        selectedDocuments: selectedDocuments.filter((id) => !docIds.has(id)),
      });
    } else {
      const docs = collectionDocuments[collectionId] || [];
      const docIds = docs.map((d) => d.document_id);
      const newDocs = new Set([...selectedDocuments, ...docIds]);
      set({
        selectedCollections: [...selectedCollections, collectionId],
        selectedDocuments: [...newDocs],
      });
    }
  },

  toggleDocument: (collectionId, documentId) => {
    const { selectedDocuments, selectedCollections, collectionDocuments } = get();
    const isSelected = selectedDocuments.includes(documentId);

    let newDocs: string[];
    if (isSelected) {
      newDocs = selectedDocuments.filter((id) => id !== documentId);
    } else {
      newDocs = [...selectedDocuments, documentId];
    }

    const allDocs = collectionDocuments[collectionId] || [];
    const allDocIds = allDocs.map((d) => d.document_id);
    const allSelected = allDocIds.length > 0 && allDocIds.every((id) => newDocs.includes(id));
    const anySelected = allDocIds.some((id) => newDocs.includes(id));

    let newCollections = selectedCollections;
    if (allSelected && !selectedCollections.includes(collectionId)) {
      newCollections = [...selectedCollections, collectionId];
    } else if (!anySelected && selectedCollections.includes(collectionId)) {
      newCollections = selectedCollections.filter((id) => id !== collectionId);
    }

    set({ selectedDocuments: newDocs, selectedCollections: newCollections });
  },

  selectAllInCollection: (collectionId) => {
    const { collectionDocuments, selectedDocuments, selectedCollections } = get();
    const docs = collectionDocuments[collectionId] || [];
    const docIds = docs.map((d) => d.document_id);
    const newDocs = new Set([...selectedDocuments, ...docIds]);
    const newCollections = selectedCollections.includes(collectionId)
      ? selectedCollections
      : [...selectedCollections, collectionId];
    set({ selectedDocuments: [...newDocs], selectedCollections: newCollections });
  },

  deselectAllInCollection: (collectionId) => {
    const { collectionDocuments, selectedDocuments, selectedCollections } = get();
    const docs = collectionDocuments[collectionId] || [];
    const docIds = new Set(docs.map((d) => d.document_id));
    set({
      selectedDocuments: selectedDocuments.filter((id) => !docIds.has(id)),
      selectedCollections: selectedCollections.filter((id) => id !== collectionId),
    });
  },

  loadCollections: async (projectId) => {
    set({ isLoadingCollections: true });
    try {
      const result = await ragListCollections(projectId);
      if (result.success && result.data) {
        set({ availableCollections: result.data, isLoadingCollections: false });
      } else {
        set({ isLoadingCollections: false });
      }
    } catch {
      set({ isLoadingCollections: false });
    }
  },

  loadDocuments: async (collectionId) => {
    const { isLoadingDocuments } = get();
    if (isLoadingDocuments[collectionId]) return;

    set({ isLoadingDocuments: { ...get().isLoadingDocuments, [collectionId]: true } });
    try {
      const result = await ragListDocuments(collectionId);
      if (result.success && result.data) {
        set((state) => ({
          collectionDocuments: { ...state.collectionDocuments, [collectionId]: result.data! },
          isLoadingDocuments: { ...state.isLoadingDocuments, [collectionId]: false },
        }));
      } else {
        set({ isLoadingDocuments: { ...get().isLoadingDocuments, [collectionId]: false } });
      }
    } catch {
      set({ isLoadingDocuments: { ...get().isLoadingDocuments, [collectionId]: false } });
    }
  },

  // =========================================================================
  // Memory Actions
  // =========================================================================

  toggleMemory: (enabled) => {
    set({ memoryEnabled: enabled });
    if (!enabled) {
      set({ selectedMemoryCategories: [], selectedMemoryIds: [] });
    }
  },

  toggleMemoryCategory: (category) => {
    const { selectedMemoryCategories, categoryMemories, selectedMemoryIds } = get();
    const isSelected = selectedMemoryCategories.includes(category);

    if (isSelected) {
      // Deselect category and all its memory items
      const memories = categoryMemories[category] || [];
      const memIds = new Set(memories.map((m) => m.id));
      set({
        selectedMemoryCategories: selectedMemoryCategories.filter((c) => c !== category),
        selectedMemoryIds: selectedMemoryIds.filter((id) => !memIds.has(id)),
      });
    } else {
      // Select category and all its loaded memory items
      const memories = categoryMemories[category] || [];
      const memIds = memories.map((m) => m.id);
      const newIds = new Set([...selectedMemoryIds, ...memIds]);
      set({
        selectedMemoryCategories: [...selectedMemoryCategories, category],
        selectedMemoryIds: [...newIds],
      });
    }
  },

  toggleMemoryItem: (memoryId) => {
    const { selectedMemoryIds, selectedMemoryCategories, categoryMemories } = get();
    const isSelected = selectedMemoryIds.includes(memoryId);

    let newIds: string[];
    if (isSelected) {
      newIds = selectedMemoryIds.filter((id) => id !== memoryId);
    } else {
      newIds = [...selectedMemoryIds, memoryId];
    }

    // Re-evaluate category selections based on item state
    const newCategories = [...selectedMemoryCategories];
    for (const [cat, memories] of Object.entries(categoryMemories)) {
      if (memories.length === 0) continue;
      const catMemIds = memories.map((m) => m.id);
      const allSelected = catMemIds.every((id) => newIds.includes(id));
      const catIdx = newCategories.indexOf(cat);

      if (allSelected && catIdx === -1) {
        newCategories.push(cat);
      } else if (!allSelected && catIdx !== -1) {
        newCategories.splice(catIdx, 1);
      }
    }

    set({ selectedMemoryIds: newIds, selectedMemoryCategories: newCategories });
  },

  loadMemoryStats: async (projectPath) => {
    set({ isLoadingMemoryStats: true });
    try {
      const response = await invoke<CommandResponse<MemoryStats>>('get_memory_stats', {
        projectPath,
      });
      if (response.success && response.data) {
        set({ availableMemoryStats: response.data, isLoadingMemoryStats: false });
      } else {
        set({ isLoadingMemoryStats: false });
      }
    } catch {
      set({ isLoadingMemoryStats: false });
    }
  },

  loadCategoryMemories: async (projectPath, category) => {
    const { isLoadingCategoryMemories } = get();
    if (isLoadingCategoryMemories[category]) return;

    set({
      isLoadingCategoryMemories: { ...get().isLoadingCategoryMemories, [category]: true },
    });
    try {
      const response = await invoke<CommandResponse<MemoryEntry[]>>('list_project_memories', {
        projectPath,
        category,
        offset: 0,
        limit: 50,
      });
      if (response.success && response.data) {
        set((state) => ({
          categoryMemories: { ...state.categoryMemories, [category]: response.data! },
          isLoadingCategoryMemories: { ...state.isLoadingCategoryMemories, [category]: false },
        }));
      } else {
        set({
          isLoadingCategoryMemories: { ...get().isLoadingCategoryMemories, [category]: false },
        });
      }
    } catch {
      set({
        isLoadingCategoryMemories: { ...get().isLoadingCategoryMemories, [category]: false },
      });
    }
  },

  searchMemoriesForPicker: async (projectPath, query) => {
    set({ isSearchingMemories: true, memoryPickerSearchQuery: query });
    try {
      const response = await invoke<CommandResponse<MemorySearchResult[]>>('search_project_memories', {
        projectPath,
        query,
        categories: null,
        topK: 20,
      });
      if (response.success && response.data) {
        set({
          memorySearchResults: response.data.map((r) => r.entry),
          isSearchingMemories: false,
        });
      } else {
        set({ memorySearchResults: [], isSearchingMemories: false });
      }
    } catch {
      set({ memorySearchResults: [], isSearchingMemories: false });
    }
  },

  clearMemorySearch: () => {
    set({ memoryPickerSearchQuery: '', memorySearchResults: null, isSearchingMemories: false });
  },

  // =========================================================================
  // Skills Actions
  // =========================================================================

  toggleSkills: (enabled) => {
    set({ skillsEnabled: enabled });
    if (!enabled) {
      set({ selectedSkillIds: [] });
    }
  },

  toggleSkillItem: (skillId) => {
    const { selectedSkillIds } = get();
    if (selectedSkillIds.includes(skillId)) {
      set({ selectedSkillIds: selectedSkillIds.filter((id) => id !== skillId) });
    } else {
      set({ selectedSkillIds: [...selectedSkillIds, skillId] });
    }
  },

  toggleSkillGroup: (sourceType) => {
    const { availableSkills, selectedSkillIds } = get();
    // "detected" is a virtual group: skills where detected=true regardless of source
    const groupSkills =
      sourceType === '__detected__' || sourceType === 'detected'
        ? availableSkills.filter((s) => s.detected && s.enabled)
        : availableSkills.filter((s) => s.source.type === sourceType && s.enabled);
    const groupIds = groupSkills.map((s) => s.id);
    const allSelected = groupIds.length > 0 && groupIds.every((id) => selectedSkillIds.includes(id));

    if (allSelected) {
      // Deselect all in group
      const groupIdSet = new Set(groupIds);
      set({ selectedSkillIds: selectedSkillIds.filter((id) => !groupIdSet.has(id)) });
    } else {
      // Select all in group
      const newIds = new Set([...selectedSkillIds, ...groupIds]);
      set({ selectedSkillIds: [...newIds] });
    }
  },

  loadAvailableSkills: async (projectPath) => {
    set({ isLoadingSkills: true });
    try {
      const response = await invoke<CommandResponse<SkillSummary[]>>('list_skills', {
        projectPath,
        sourceFilter: null,
        includeDisabled: false,
      });
      if (response.success && response.data) {
        set({ availableSkills: response.data, isLoadingSkills: false });
      } else {
        set({ isLoadingSkills: false });
      }
    } catch {
      set({ isLoadingSkills: false });
    }
  },

  setSkillPickerSearchQuery: (query) => {
    set({ skillPickerSearchQuery: query });
  },

  // =========================================================================
  // Build Config
  // =========================================================================

  buildConfig: () => {
    const {
      knowledgeEnabled,
      selectedCollections,
      selectedDocuments,
      memoryEnabled,
      selectedMemoryCategories,
      selectedMemoryIds,
      skillsEnabled,
      selectedSkillIds,
    } = get();

    if (!knowledgeEnabled && !memoryEnabled && !skillsEnabled) {
      return undefined;
    }

    const projectId = useProjectsStore.getState().selectedProject?.id ?? 'default';
    const config: ContextSourceConfig = { project_id: projectId };

    if (knowledgeEnabled) {
      config.knowledge = {
        enabled: true,
        selected_collections: selectedCollections,
        selected_documents: selectedDocuments,
      };
    }

    if (memoryEnabled) {
      config.memory = {
        enabled: true,
        selected_categories: selectedMemoryCategories,
        selected_memory_ids: selectedMemoryIds,
      };
    }

    if (skillsEnabled) {
      config.skills = {
        enabled: true,
        selected_skill_ids: selectedSkillIds,
      };
    }

    return config;
  },
}));

export default useContextSourcesStore;
