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
import { ragListCollections, ragListDocuments, ragEnsureDocsCollection } from '../lib/knowledgeApi';
import type { MemoryEntry, MemoryScope, MemoryStats, MemorySearchResult, SkillSummary } from '../types/skillMemory';
import { useProjectsStore } from './projects';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

const MEMORY_COMMAND_TIMEOUT_MS = 8000;

async function invokeCommandResponseWithTimeout<T>(
  command: string,
  args: Record<string, unknown>,
  timeoutMs = MEMORY_COMMAND_TIMEOUT_MS,
): Promise<CommandResponse<T>> {
  return await new Promise<CommandResponse<T>>((resolve) => {
    const timer = setTimeout(() => {
      resolve({
        success: false,
        data: null,
        error: `${command} timed out after ${timeoutMs}ms`,
      });
    }, timeoutMs);

    invoke<CommandResponse<T>>(command, args)
      .then((response) => {
        clearTimeout(timer);
        resolve(response);
      })
      .catch((error: unknown) => {
        clearTimeout(timer);
        resolve({
          success: false,
          data: null,
          error: error instanceof Error ? error.message : String(error),
        });
      });
  });
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
    excluded_memory_ids: string[];
    selected_scopes: MemoryScope[];
    session_id?: string | null;
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
  selectedMemoryScopes: MemoryScope[];
  memorySessionId: string | null;
  selectedMemoryCategories: string[];
  selectedMemoryIds: string[]; // excluded ids for default-included memory injection
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

  // === Knowledge Auto-Association ===
  /** Tracks the last workspace path for which auto-association was performed. */
  _autoAssociatedPath: string | null;
  /** Auto-associate knowledge collections whose workspace_path matches the given workspace. */
  autoAssociateForWorkspace: (workspacePath: string, projectId: string) => Promise<void>;
  /** Reset auto-association guard so the next workspace change triggers re-association. */
  resetAutoAssociation: () => void;

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
  toggleMemoryScope: (scope: MemoryScope) => void;
  setMemorySessionId: (sessionId: string | null) => void;
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

  /** Build the config object for backend invocation. */
  buildConfig: () => ContextSourceConfig;
}

function normalizeMemoryScopes(scopes: MemoryScope[], sessionId: string | null): MemoryScope[] {
  const unique: MemoryScope[] = [];
  for (const scope of scopes) {
    if (!unique.includes(scope)) unique.push(scope);
  }
  const filtered = unique.filter((scope) => scope !== 'session' || !!sessionId?.trim());
  return filtered.length > 0 ? filtered : ['project', 'global'];
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
  _autoAssociatedPath: null,

  // === Memory State ===
  memoryEnabled: true,
  selectedMemoryScopes: ['global', 'project', 'session'],
  memorySessionId: null,
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
  // Knowledge Auto-Association
  // =========================================================================

  autoAssociateForWorkspace: async (workspacePath, projectId) => {
    // Skip if already auto-associated for this workspace
    if (get()._autoAssociatedPath === workspacePath) return;

    try {
      const result = await ragListCollections(projectId);
      if (!result.success || !result.data) {
        set({ _autoAssociatedPath: workspacePath });
        return;
      }

      const collections = result.data;

      // Normalize path for comparison: backslash→forward slash, strip trailing slash
      const normalizePath = (p: string) => p.replace(/\\/g, '/').replace(/\/+$/, '');

      const normalizedWorkspace = normalizePath(workspacePath);

      // Find collections whose workspace_path matches
      const matching = collections.filter(
        (c) => c.workspace_path && normalizePath(c.workspace_path) === normalizedWorkspace,
      );

      const docsPrefix = '[Docs] ';
      const hasDocsCollection = collections.some(
        (c) =>
          c.name.startsWith(docsPrefix) && c.workspace_path && normalizePath(c.workspace_path) === normalizedWorkspace,
      );

      if (matching.length > 0) {
        const matchingIds = matching.map((c) => c.id);
        set({
          knowledgeEnabled: true,
          selectedCollections: matchingIds,
          availableCollections: collections,
          _autoAssociatedPath: workspacePath,
        });
      } else {
        set({
          availableCollections: collections,
          _autoAssociatedPath: workspacePath,
        });
      }

      // Trigger docs collection creation if none exists for this workspace
      if (!hasDocsCollection) {
        try {
          const docsResult = await ragEnsureDocsCollection(workspacePath, projectId);
          if (docsResult.success && docsResult.data) {
            // Reload collections and auto-select the new docs collection
            const refreshed = await ragListCollections(projectId);
            if (refreshed.success && refreshed.data) {
              const docsCol = refreshed.data.find((c) => c.id === docsResult.data!.id);
              const currentSelected = get().selectedCollections;
              const newSelected =
                docsCol && !currentSelected.includes(docsCol.id) ? [...currentSelected, docsCol.id] : currentSelected;
              set({
                knowledgeEnabled: true,
                availableCollections: refreshed.data,
                selectedCollections: newSelected,
              });
            }
          }
        } catch {
          // Non-critical: docs indexing failure shouldn't block workspace usage
        }
      }
    } catch {
      set({ _autoAssociatedPath: workspacePath });
    }
  },

  resetAutoAssociation: () => {
    set({ _autoAssociatedPath: null });
  },

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

  toggleMemoryScope: (scope) => {
    const { selectedMemoryScopes, memorySessionId, categoryMemories, isLoadingCategoryMemories } = get();
    if (scope === 'session' && !memorySessionId?.trim()) return;
    const exists = selectedMemoryScopes.includes(scope);
    const nextScopes = exists ? selectedMemoryScopes.filter((s) => s !== scope) : [...selectedMemoryScopes, scope];
    const normalizedScopes = normalizeMemoryScopes(nextScopes, memorySessionId);
    // Scope change invalidates loaded category snapshots
    set({
      selectedMemoryScopes: normalizedScopes,
      categoryMemories: Object.keys(categoryMemories).length > 0 ? {} : categoryMemories,
      isLoadingCategoryMemories: Object.keys(isLoadingCategoryMemories).length > 0 ? {} : isLoadingCategoryMemories,
    });
  },

  setMemorySessionId: (sessionId) => {
    const trimmed = sessionId?.trim() || null;
    const { selectedMemoryScopes } = get();
    const scopesWithSession: MemoryScope[] =
      trimmed && !selectedMemoryScopes.includes('session')
        ? [...selectedMemoryScopes, 'session']
        : selectedMemoryScopes;
    set({
      memorySessionId: trimmed,
      selectedMemoryScopes: normalizeMemoryScopes(scopesWithSession, trimmed),
    });
  },

  toggleMemoryCategory: (category) => {
    const { selectedMemoryCategories } = get();
    const isSelected = selectedMemoryCategories.includes(category);

    if (isSelected) {
      set({
        selectedMemoryCategories: selectedMemoryCategories.filter((c) => c !== category),
      });
    } else {
      set({
        selectedMemoryCategories: [...selectedMemoryCategories, category],
      });
    }
  },

  toggleMemoryItem: (memoryId) => {
    const { selectedMemoryIds } = get();
    const isExcluded = selectedMemoryIds.includes(memoryId);
    if (isExcluded) {
      set({ selectedMemoryIds: selectedMemoryIds.filter((id) => id !== memoryId) });
    } else {
      set({ selectedMemoryIds: [...selectedMemoryIds, memoryId] });
    }
  },

  loadMemoryStats: async (projectPath) => {
    set({ isLoadingMemoryStats: true });
    try {
      const { selectedMemoryScopes, memorySessionId } = get();
      const scopes = normalizeMemoryScopes(selectedMemoryScopes, memorySessionId);
      const settledStats = await Promise.allSettled(
        scopes.map((scope) =>
          invokeCommandResponseWithTimeout<MemoryStats>('get_memory_stats', {
            projectPath,
            scope,
            sessionId: memorySessionId,
          }),
        ),
      );

      const nonNullStats: MemoryStats[] = [];
      for (const result of settledStats) {
        if (result.status !== 'fulfilled') continue;
        const response = result.value;
        if (response.success && response.data) {
          nonNullStats.push(response.data);
        }
      }

      if (nonNullStats.length === 0) {
        set({ availableMemoryStats: null, isLoadingMemoryStats: false });
        return;
      }

      let totalCount = 0;
      let weightedImportanceSum = 0;
      const categoryCounts: Record<string, number> = {};
      for (const stats of nonNullStats) {
        totalCount += stats.total_count;
        weightedImportanceSum += stats.avg_importance * stats.total_count;
        for (const [cat, count] of Object.entries(stats.category_counts)) {
          categoryCounts[cat] = (categoryCounts[cat] || 0) + count;
        }
      }

      const aggregate: MemoryStats = {
        total_count: totalCount,
        category_counts: categoryCounts,
        avg_importance: totalCount > 0 ? weightedImportanceSum / totalCount : 0,
      };
      set({ availableMemoryStats: aggregate, isLoadingMemoryStats: false });
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
      const { selectedMemoryScopes, memorySessionId } = get();
      const scopes = normalizeMemoryScopes(selectedMemoryScopes, memorySessionId);
      const listResults = await Promise.all(
        scopes.map(async (scope) => {
          const response = await invoke<CommandResponse<MemoryEntry[]>>('list_project_memories', {
            projectPath,
            category,
            offset: 0,
            limit: 50,
            scope,
            sessionId: memorySessionId,
          });
          return response.success && response.data ? response.data : [];
        }),
      );

      const mergedMap = new Map<string, MemoryEntry>();
      for (const list of listResults) {
        for (const entry of list) {
          mergedMap.set(entry.id, entry);
        }
      }
      const merged = Array.from(mergedMap.values()).sort((a, b) => b.importance - a.importance);

      set((state) => ({
        categoryMemories: { ...state.categoryMemories, [category]: merged },
        isLoadingCategoryMemories: { ...state.isLoadingCategoryMemories, [category]: false },
      }));
    } catch {
      set({
        isLoadingCategoryMemories: { ...get().isLoadingCategoryMemories, [category]: false },
      });
    }
  },

  searchMemoriesForPicker: async (projectPath, query) => {
    set({ isSearchingMemories: true, memoryPickerSearchQuery: query });
    try {
      const { selectedMemoryScopes, memorySessionId } = get();
      const scopes = normalizeMemoryScopes(selectedMemoryScopes, memorySessionId);
      const searchResults = await Promise.all(
        scopes.map(async (scope) => {
          const response = await invoke<CommandResponse<MemorySearchResult[]>>('search_project_memories', {
            projectPath,
            query,
            categories: null,
            topK: 20,
            scope,
            sessionId: memorySessionId,
          });
          return response.success && response.data ? response.data : [];
        }),
      );

      const mergedMap = new Map<string, { entry: MemoryEntry; score: number }>();
      for (const results of searchResults) {
        for (const result of results) {
          const prev = mergedMap.get(result.entry.id);
          if (!prev || result.relevance_score > prev.score) {
            mergedMap.set(result.entry.id, { entry: result.entry, score: result.relevance_score });
          }
        }
      }

      const merged = Array.from(mergedMap.values())
        .sort((a, b) => b.score - a.score)
        .map((item) => item.entry);

      set({
        memorySearchResults: merged,
        isSearchingMemories: false,
      });
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
      selectedMemoryScopes,
      memorySessionId,
      selectedMemoryCategories,
      selectedMemoryIds,
      skillsEnabled,
      selectedSkillIds,
    } = get();

    const projectId = useProjectsStore.getState().selectedProject?.id ?? 'default';
    const config: ContextSourceConfig = { project_id: projectId };

    const normalizedScopes = normalizeMemoryScopes(selectedMemoryScopes, memorySessionId);
    config.memory = {
      enabled: memoryEnabled,
      selected_categories: selectedMemoryCategories,
      selected_memory_ids: [],
      excluded_memory_ids: selectedMemoryIds,
      selected_scopes: normalizedScopes,
      session_id: memorySessionId,
    };

    if (knowledgeEnabled) {
      config.knowledge = {
        enabled: true,
        selected_collections: selectedCollections,
        selected_documents: selectedDocuments,
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
