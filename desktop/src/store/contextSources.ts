/**
 * Context Sources Store
 *
 * Zustand store for user-controlled domain knowledge injection.
 * Manages which Knowledge collections/documents, Memory categories/items,
 * and Skills are selected for injection into Chat Mode and Task Mode prompts.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { KnowledgeCollection, DocumentSummary, ScopedDocumentRef } from '../lib/knowledgeApi';
import { ragListCollections, ragListDocuments, ragEnsureDocsCollection } from '../lib/knowledgeApi';
import type { MemoryEntry, MemoryScope, MemoryStats, SkillSummary } from '../types/skillMemory';
import type { ContextSourceConfig, MemorySelectionMode } from '../types/contextSources';
import { useProjectsStore } from './projects';
import { useSettingsStore } from './settings';
import { reportNonFatal } from '../lib/nonFatal';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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

interface SkillCommandInvocation {
  skill_id: string;
  skill_name: string;
  session_id: string | null;
  pinned: boolean;
  selection_origin: string;
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

export interface ContextSourcesState {
  // === Knowledge State ===
  knowledgeEnabled: boolean;
  selectedCollections: string[];
  selectedDocuments: ScopedDocumentRef[];
  availableCollections: KnowledgeCollection[];
  collectionDocuments: Record<string, DocumentSummary[]>;
  isLoadingCollections: boolean;
  isLoadingDocuments: Record<string, boolean>;

  // === Memory State ===
  memoryEnabled: boolean;
  memorySelectionMode: 'auto' | 'only_selected';
  selectedMemoryScopes: MemoryScope[];
  memorySessionId: string | null;
  selectedMemoryCategories: string[];
  /** @deprecated Compat alias of excludedMemoryIds; use excludedMemoryIds/includedMemoryIds instead. */
  selectedMemoryIds: string[];
  includedMemoryIds: string[];
  excludedMemoryIds: string[];
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
  invokedSkillIds: string[];
  skillSelectionMode: 'auto' | 'explicit';
  skillInvocationSessionId: string | null;
  availableSkills: SkillSummary[];
  isLoadingSkills: boolean;
  skillPickerSearchQuery: string;

  // === Knowledge Auto-Association ===
  /** Tracks workspace+project scopes for which auto-association was performed. */
  _autoAssociatedScopes: Record<string, true>;
  /** Auto-associate knowledge collections whose workspace_path matches the given workspace. */
  autoAssociateForWorkspace: (workspacePath: string, projectId: string) => Promise<void>;
  /** Reset auto-association guard so the next workspace change triggers re-association. */
  resetAutoAssociation: () => void;

  // === Knowledge Actions ===
  toggleKnowledge: (enabled: boolean) => void;
  toggleCollection: (collectionId: string) => void;
  toggleDocument: (collectionId: string, documentUid: string) => void;
  selectAllInCollection: (collectionId: string) => void;
  deselectAllInCollection: (collectionId: string) => void;
  loadCollections: (projectId: string) => Promise<void>;
  loadDocuments: (collectionId: string) => Promise<void>;

  // === Memory Actions ===
  toggleMemory: (enabled: boolean) => void;
  toggleMemoryScope: (scope: MemoryScope) => void;
  setMemorySessionId: (sessionId: string | null) => void;
  setMemorySelectionMode: (mode: 'auto' | 'only_selected') => void;
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
  invokeSkillCommand: (projectPath: string, skillId: string, sessionId?: string | null) => Promise<boolean>;
  clearInvokedSkill: (skillId: string) => void;
  clearInvokedSkills: () => void;
  bindSkillInvocationSession: (sessionId: string | null) => void;
  loadAvailableSkills: (projectPath: string) => Promise<void>;
  reconcileSelectedSkills: (availableSkills: SkillSummary[]) => void;
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

function scopedRefKey(collectionId: string, documentUid: string): string {
  return `${collectionId}::${documentUid}`;
}

function toScopedRef(collectionId: string, documentUid: string): ScopedDocumentRef {
  return { collection_id: collectionId, document_uid: documentUid };
}

function normalizePath(path: string): string {
  return path.replace(/\\/g, '/').replace(/\/+$/, '');
}

function autoAssociateScopeKey(workspacePath: string, projectId: string): string {
  return `${normalizePath(workspacePath)}::${projectId}`;
}

function toMemorySelectionModeV2(mode: 'auto' | 'only_selected'): MemorySelectionMode {
  return mode === 'only_selected' ? 'only_selected' : 'auto_exclude';
}

function buildLegacyConfigFromState(state: ContextSourcesState, projectId: string): ContextSourceConfig {
  const normalizedScopes = normalizeMemoryScopes(state.selectedMemoryScopes, state.memorySessionId);
  const compatExcluded = state.excludedMemoryIds.length > 0 ? state.excludedMemoryIds : state.selectedMemoryIds;
  const selectedIds = state.memorySelectionMode === 'only_selected' ? state.includedMemoryIds : [];
  const excludedIds = state.memorySelectionMode === 'only_selected' ? [] : compatExcluded;

  const config: ContextSourceConfig = {
    project_id: projectId,
    memory: {
      enabled: state.memoryEnabled,
      selected_categories: state.selectedMemoryCategories,
      selected_memory_ids: selectedIds,
      excluded_memory_ids: excludedIds,
      selected_scopes: normalizedScopes,
      session_id: state.memorySessionId,
      statuses: [],
      review_mode: 'active_only',
      selection_mode: toMemorySelectionModeV2(state.memorySelectionMode),
    },
  };

  if (state.knowledgeEnabled) {
    config.knowledge = {
      enabled: true,
      selected_collections: state.selectedCollections,
      selected_documents: state.selectedDocuments,
    };
  }

  if (state.skillsEnabled) {
    config.skills = {
      enabled: true,
      selected_skill_ids: state.selectedSkillIds,
      invoked_skill_ids: state.invokedSkillIds,
      selection_mode: state.selectedSkillIds.length > 0 ? 'explicit' : state.skillSelectionMode,
      review_filter: 'approved_only',
    };
  }

  return config;
}

function reconcileSelectedSkillIds(
  selectedSkillIds: string[],
  availableSkills: SkillSummary[],
): { selectedSkillIds: string[]; skillSelectionMode: 'auto' | 'explicit' } {
  const enabledIdSet = new Set(availableSkills.filter((skill) => skill.enabled).map((skill) => skill.id));
  const nextSelected = selectedSkillIds.filter((id) => enabledIdSet.has(id));
  return {
    selectedSkillIds: nextSelected,
    skillSelectionMode: nextSelected.length > 0 ? 'explicit' : 'auto',
  };
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
  _autoAssociatedScopes: {},

  // === Memory State ===
  memoryEnabled: true,
  memorySelectionMode: 'auto',
  selectedMemoryScopes: ['global', 'project', 'session'],
  memorySessionId: null,
  selectedMemoryCategories: [],
  selectedMemoryIds: [],
  includedMemoryIds: [],
  excludedMemoryIds: [],
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
  invokedSkillIds: [],
  skillSelectionMode: 'auto',
  skillInvocationSessionId: null,
  availableSkills: [],
  isLoadingSkills: false,
  skillPickerSearchQuery: '',

  // =========================================================================
  // Knowledge Auto-Association
  // =========================================================================

  autoAssociateForWorkspace: async (workspacePath, projectId) => {
    const scopeKey = autoAssociateScopeKey(workspacePath, projectId);
    if (get()._autoAssociatedScopes[scopeKey]) return;

    const markScopeAssociated = () =>
      set((state) => ({
        _autoAssociatedScopes: {
          ...state._autoAssociatedScopes,
          [scopeKey]: true,
        },
      }));

    try {
      const result = await ragListCollections(projectId);
      if (!result.success || !result.data) {
        markScopeAssociated();
        return;
      }

      const collections = result.data;
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
        set((state) => ({
          knowledgeEnabled: true,
          selectedCollections: matchingIds,
          availableCollections: collections,
          _autoAssociatedScopes: {
            ...state._autoAssociatedScopes,
            [scopeKey]: true,
          },
        }));
      } else {
        set((state) => ({
          availableCollections: collections,
          _autoAssociatedScopes: {
            ...state._autoAssociatedScopes,
            [scopeKey]: true,
          },
        }));
      }

      // Trigger docs collection creation only when explicitly enabled in settings.
      const autoEnsureDocs = useSettingsStore.getState().knowledgeAutoEnsureDocsCollection ?? false;
      if (!hasDocsCollection && autoEnsureDocs) {
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
        } catch (error) {
          console.warn('[contextSources] Failed auto-ensuring docs collection', {
            workspacePath,
            projectId,
            error: error instanceof Error ? error.message : String(error),
          });
        }
      }
    } catch (error) {
      console.warn('[contextSources] Auto-association failed', {
        workspacePath,
        projectId,
        error: error instanceof Error ? error.message : String(error),
      });
      markScopeAssociated();
    }
  },

  resetAutoAssociation: () => {
    set({ _autoAssociatedScopes: {} });
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
      set({
        selectedCollections: selectedCollections.filter((id) => id !== collectionId),
        selectedDocuments: selectedDocuments.filter((ref) => ref.collection_id !== collectionId),
      });
    } else {
      const docs = collectionDocuments[collectionId] || [];
      const existingKeys = new Set(selectedDocuments.map((ref) => scopedRefKey(ref.collection_id, ref.document_uid)));
      const merged: ScopedDocumentRef[] = [...selectedDocuments];
      docs.forEach((doc) => {
        const key = scopedRefKey(collectionId, doc.document_uid);
        if (!existingKeys.has(key)) {
          existingKeys.add(key);
          merged.push(toScopedRef(collectionId, doc.document_uid));
        }
      });
      set({
        selectedCollections: [...selectedCollections, collectionId],
        selectedDocuments: merged,
      });
    }
  },

  toggleDocument: (collectionId, documentUid) => {
    const { selectedDocuments, selectedCollections, collectionDocuments } = get();
    const isSelected = selectedDocuments.some(
      (ref) => ref.collection_id === collectionId && ref.document_uid === documentUid,
    );

    let newDocs: ScopedDocumentRef[];
    if (isSelected) {
      newDocs = selectedDocuments.filter(
        (ref) => !(ref.collection_id === collectionId && ref.document_uid === documentUid),
      );
    } else {
      newDocs = [...selectedDocuments, toScopedRef(collectionId, documentUid)];
    }

    const allDocs = collectionDocuments[collectionId] || [];
    const allSelected =
      allDocs.length > 0 &&
      allDocs.every((doc) =>
        newDocs.some((ref) => ref.collection_id === collectionId && ref.document_uid === doc.document_uid),
      );
    const anySelected = allDocs.some((doc) =>
      newDocs.some((ref) => ref.collection_id === collectionId && ref.document_uid === doc.document_uid),
    );

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
    const existingKeys = new Set(selectedDocuments.map((ref) => scopedRefKey(ref.collection_id, ref.document_uid)));
    const merged: ScopedDocumentRef[] = [...selectedDocuments];
    docs.forEach((doc) => {
      const key = scopedRefKey(collectionId, doc.document_uid);
      if (!existingKeys.has(key)) {
        existingKeys.add(key);
        merged.push(toScopedRef(collectionId, doc.document_uid));
      }
    });
    const newCollections = selectedCollections.includes(collectionId)
      ? selectedCollections
      : [...selectedCollections, collectionId];
    set({ selectedDocuments: merged, selectedCollections: newCollections });
  },

  deselectAllInCollection: (collectionId) => {
    const { selectedDocuments, selectedCollections } = get();
    set({
      selectedDocuments: selectedDocuments.filter((ref) => ref.collection_id !== collectionId),
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
    } catch (error) {
      reportNonFatal('contextSources.loadCollections', error, { projectId });
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
    } catch (error) {
      reportNonFatal('contextSources.loadDocuments', error, { collectionId });
      set({ isLoadingDocuments: { ...get().isLoadingDocuments, [collectionId]: false } });
    }
  },

  // =========================================================================
  // Memory Actions
  // =========================================================================

  toggleMemory: (enabled) => {
    set({ memoryEnabled: enabled });
    if (!enabled) {
      set({
        selectedMemoryCategories: [],
        selectedMemoryIds: [],
        includedMemoryIds: [],
        excludedMemoryIds: [],
      });
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

  setMemorySelectionMode: (mode) => {
    set((state) => {
      if (mode === state.memorySelectionMode) return state;
      return {
        memorySelectionMode: mode,
        selectedMemoryIds: mode === 'auto' ? state.excludedMemoryIds : state.includedMemoryIds,
      };
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
    const { memorySelectionMode, includedMemoryIds, excludedMemoryIds } = get();
    if (memorySelectionMode === 'only_selected') {
      const exists = includedMemoryIds.includes(memoryId);
      const nextIncluded = exists
        ? includedMemoryIds.filter((id) => id !== memoryId)
        : [...includedMemoryIds, memoryId];
      set({
        includedMemoryIds: nextIncluded,
        selectedMemoryIds: nextIncluded,
      });
      return;
    }

    const exists = excludedMemoryIds.includes(memoryId);
    const nextExcluded = exists ? excludedMemoryIds.filter((id) => id !== memoryId) : [...excludedMemoryIds, memoryId];
    set({
      excludedMemoryIds: nextExcluded,
      selectedMemoryIds: nextExcluded,
    });
  },

  loadMemoryStats: async (projectPath) => {
    set({ isLoadingMemoryStats: true });
    try {
      const { selectedMemoryScopes, memorySessionId } = get();
      const scopes = normalizeMemoryScopes(selectedMemoryScopes, memorySessionId);
      const response = await invokeCommandResponseWithTimeout<MemoryStats>('memory_stats_v2', {
        projectPath,
        scopes,
        statuses: ['active'],
        sessionId: memorySessionId,
      });
      if (!response.success || !response.data) {
        set({ availableMemoryStats: null, isLoadingMemoryStats: false });
        return;
      }
      set({ availableMemoryStats: response.data, isLoadingMemoryStats: false });
    } catch (error) {
      reportNonFatal('contextSources.loadMemoryStats', error, { projectPath });
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
      const response = await invokeCommandResponseWithTimeout<MemoryEntry[]>('list_memory_entries_v2', {
        projectPath,
        categories: [category],
        scopes,
        statuses: ['active'],
        sessionId: memorySessionId,
        offset: 0,
        limit: 200,
      });
      const merged = response.success && response.data ? response.data.sort((a, b) => b.importance - a.importance) : [];

      set((state) => ({
        categoryMemories: { ...state.categoryMemories, [category]: merged },
        isLoadingCategoryMemories: { ...state.isLoadingCategoryMemories, [category]: false },
      }));
    } catch (error) {
      reportNonFatal('contextSources.loadCategoryMemories', error, { projectPath, category });
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
      const response = await invokeCommandResponseWithTimeout<UnifiedMemoryQueryResultV2>('query_memory_entries_v2', {
        projectPath,
        query,
        categories: null,
        scopes,
        includeIds: [],
        excludeIds: [],
        statuses: ['active'],
        sessionId: memorySessionId,
        topKTotal: 50,
        minImportance: 0.1,
        enableSemantic: true,
        enableLexical: true,
      });

      const merged =
        response.success && response.data
          ? response.data.results.sort((a, b) => b.relevance_score - a.relevance_score).map((item) => item.entry)
          : [];

      set({
        memorySearchResults: merged,
        isSearchingMemories: false,
      });
    } catch (error) {
      reportNonFatal('contextSources.searchMemoriesForPicker', error, { projectPath, query });
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
      set({
        selectedSkillIds: [],
        invokedSkillIds: [],
        skillSelectionMode: 'auto',
        skillInvocationSessionId: null,
      });
    }
  },

  toggleSkillItem: (skillId) => {
    const { selectedSkillIds } = get();
    if (selectedSkillIds.includes(skillId)) {
      const nextSelected = selectedSkillIds.filter((id) => id !== skillId);
      set({
        selectedSkillIds: nextSelected,
        skillSelectionMode: nextSelected.length > 0 ? 'explicit' : 'auto',
      });
    } else {
      set({ selectedSkillIds: [...selectedSkillIds, skillId], skillSelectionMode: 'explicit' });
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
    if (groupIds.length === 0) {
      return;
    }
    const allSelected = groupIds.length > 0 && groupIds.every((id) => selectedSkillIds.includes(id));

    if (allSelected) {
      // Deselect all in group
      const groupIdSet = new Set(groupIds);
      const nextSelected = selectedSkillIds.filter((id) => !groupIdSet.has(id));
      set({
        selectedSkillIds: nextSelected,
        skillSelectionMode: nextSelected.length > 0 ? 'explicit' : 'auto',
      });
    } else {
      // Select all in group
      const newIds = new Set([...selectedSkillIds, ...groupIds]);
      set({ selectedSkillIds: [...newIds], skillSelectionMode: 'explicit' });
    }
  },

  invokeSkillCommand: async (projectPath, skillId, sessionId) => {
    const normalizedSessionId = sessionId?.trim() || null;
    try {
      const response = await invoke<CommandResponse<SkillCommandInvocation>>('invoke_skill_command_v2', {
        projectPath,
        skillId,
        sessionId: normalizedSessionId,
      });
      if (!response.success || !response.data) {
        reportNonFatal('contextSources.invokeSkillCommand', response.error ?? 'Invocation failed', {
          projectPath,
          skillId,
          sessionId: normalizedSessionId,
        });
        return false;
      }

      set((state) => ({
        skillsEnabled: true,
        invokedSkillIds: state.invokedSkillIds.includes(response.data!.skill_id)
          ? state.invokedSkillIds
          : [...state.invokedSkillIds, response.data!.skill_id],
        skillInvocationSessionId: normalizedSessionId,
      }));
      return true;
    } catch (error) {
      reportNonFatal('contextSources.invokeSkillCommand', error, {
        projectPath,
        skillId,
        sessionId: normalizedSessionId,
      });
      return false;
    }
  },

  clearInvokedSkill: (skillId) => {
    set((state) => ({
      invokedSkillIds: state.invokedSkillIds.filter((id) => id !== skillId),
      skillInvocationSessionId:
        state.invokedSkillIds.length <= 1 && state.invokedSkillIds.includes(skillId)
          ? null
          : state.skillInvocationSessionId,
    }));
  },

  clearInvokedSkills: () => {
    set({ invokedSkillIds: [], skillInvocationSessionId: null });
  },

  bindSkillInvocationSession: (sessionId) => {
    const normalizedSessionId = sessionId?.trim() || null;
    set((state) => {
      if (state.skillInvocationSessionId === normalizedSessionId) {
        return state;
      }
      if (!state.skillInvocationSessionId || state.invokedSkillIds.length === 0) {
        return {
          skillInvocationSessionId: normalizedSessionId,
        };
      }
      return {
        invokedSkillIds: [],
        skillInvocationSessionId: normalizedSessionId,
      };
    });
  },

  loadAvailableSkills: async (projectPath) => {
    set({ isLoadingSkills: true });
    try {
      let effectiveResponse = await invoke<CommandResponse<SkillSummary[]>>('list_skills_v2', {
        projectPath,
        sourceFilter: null,
        includeDisabled: false,
      });
      if (!effectiveResponse.success) {
        effectiveResponse = await invoke<CommandResponse<SkillSummary[]>>('list_skills', {
          projectPath,
          sourceFilter: null,
          includeDisabled: false,
        });
      }
      if (effectiveResponse.success && effectiveResponse.data) {
        set((state) => ({
          availableSkills: effectiveResponse.data!,
          isLoadingSkills: false,
          invokedSkillIds: state.invokedSkillIds.filter((id) =>
            effectiveResponse.data!.some((skill) => skill.id === id),
          ),
          ...reconcileSelectedSkillIds(state.selectedSkillIds, effectiveResponse.data!),
        }));
      } else {
        set({ isLoadingSkills: false });
      }
    } catch (error) {
      reportNonFatal('contextSources.loadAvailableSkills', error, { projectPath });
      set({ isLoadingSkills: false });
    }
  },

  reconcileSelectedSkills: (availableSkills) => {
    set((state) => reconcileSelectedSkillIds(state.selectedSkillIds, availableSkills));
  },

  setSkillPickerSearchQuery: (query) => {
    set({ skillPickerSearchQuery: query });
  },

  // =========================================================================
  // Build Config
  // =========================================================================

  buildConfig: () => {
    const state = get();
    const projectId = useProjectsStore.getState().selectedProject?.id ?? 'default';
    return buildLegacyConfigFromState(state, projectId);
  },
}));

export type { ContextSourceConfig } from '../types/contextSources';

export default useContextSourcesStore;
