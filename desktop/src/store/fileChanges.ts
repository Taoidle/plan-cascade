/**
 * Zustand store for LLM file change tracking.
 *
 * Manages AI Changes tab state: fetches changes grouped by turn,
 * loads diffs on demand, and handles file restoration.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from '../lib/tauri';
import type { RestoreExecutionResult, RestorePreviewItem, RestoredFile, TurnChanges } from '../types/fileChanges';

// ============================================================================
// State
// ============================================================================

interface FileChangesState {
  /** Changes grouped by turn, sorted ascending. */
  turnChanges: TurnChanges[];
  loading: boolean;
  error: string | null;

  /** Currently selected turn for detail view (null = none). */
  selectedTurnIndex: number | null;
  /** Set of FileChange.id that are expanded to show diff. */
  expandedChangeIds: Set<string>;
  /** Cached diff content keyed by FileChange.id. */
  diffCache: Map<string, string>;

  // Actions
  fetchChanges: (sessionId: string, projectRoot: string) => Promise<void>;
  fetchDiff: (
    sessionId: string,
    projectRoot: string,
    changeId: string,
    beforeHash: string | null,
    afterHash: string,
  ) => Promise<string | null>;
  previewRestoreToTurn: (
    sessionId: string,
    projectRoot: string,
    turnIndex: number,
  ) => Promise<RestorePreviewItem[] | null>;
  restoreToTurn: (
    sessionId: string,
    projectRoot: string,
    turnIndex: number,
    createSnapshot?: boolean,
  ) => Promise<RestoreExecutionResult | null>;
  undoRestore: (sessionId: string, projectRoot: string, operationId: string) => Promise<RestoredFile[] | null>;
  restoreSingleFile: (sessionId: string, projectRoot: string, filePath: string, hash: string) => Promise<boolean>;
  truncateFromTurn: (sessionId: string, projectRoot: string, turnIndex: number) => Promise<void>;
  selectTurn: (turnIndex: number | null) => void;
  toggleExpanded: (changeId: string) => void;
  prefillDiffCache: (changeId: string, diff: string) => void;
  reset: () => void;
}

// ============================================================================
// Store
// ============================================================================

export const useFileChangesStore = create<FileChangesState>((set, get) => ({
  turnChanges: [],
  loading: false,
  error: null,
  selectedTurnIndex: null,
  expandedChangeIds: new Set(),
  diffCache: new Map(),

  fetchChanges: async (sessionId, projectRoot) => {
    set({ loading: true, error: null });
    try {
      const resp = await invoke<CommandResponse<TurnChanges[]>>('get_file_changes_by_turn', { sessionId, projectRoot });
      if (resp.success && resp.data) {
        set({ turnChanges: resp.data, loading: false });
      } else {
        set({ error: resp.error ?? 'Failed to fetch changes', loading: false });
      }
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  fetchDiff: async (sessionId, projectRoot, changeId, beforeHash, afterHash) => {
    // Return cached value
    const cached = get().diffCache.get(changeId);
    if (cached !== undefined) return cached;

    try {
      const resp = await invoke<CommandResponse<string>>('get_file_change_diff', {
        sessionId,
        projectRoot,
        beforeHash,
        afterHash,
      });
      if (resp.success && resp.data !== null && resp.data !== undefined) {
        const newCache = new Map(get().diffCache);
        newCache.set(changeId, resp.data);
        set({ diffCache: newCache });
        return resp.data;
      }
      return null;
    } catch {
      return null;
    }
  },

  previewRestoreToTurn: async (sessionId, projectRoot, turnIndex) => {
    try {
      const resp = await invoke<CommandResponse<RestorePreviewItem[]>>('preview_restore_to_turn', {
        sessionId,
        projectRoot,
        turnIndex,
      });
      if (resp.success && resp.data) {
        return resp.data;
      }
      set({ error: resp.error ?? 'Restore preview failed' });
      return null;
    } catch (err) {
      set({ error: String(err) });
      return null;
    }
  },

  restoreToTurn: async (sessionId, projectRoot, turnIndex, createSnapshot = true) => {
    try {
      const resp = await invoke<CommandResponse<RestoreExecutionResult>>('restore_files_to_turn_v2', {
        sessionId,
        projectRoot,
        turnIndex,
        createSnapshot,
      });
      if (resp.success && resp.data) {
        return resp.data;
      }
      set({ error: resp.error ?? 'Restore failed' });
      return null;
    } catch (err) {
      set({ error: String(err) });
      return null;
    }
  },

  undoRestore: async (sessionId, projectRoot, operationId) => {
    try {
      const resp = await invoke<CommandResponse<RestoredFile[]>>('undo_restore', {
        sessionId,
        projectRoot,
        operationId,
      });
      if (resp.success && resp.data) {
        return resp.data;
      }
      set({ error: resp.error ?? 'Undo restore failed' });
      return null;
    } catch (err) {
      set({ error: String(err) });
      return null;
    }
  },

  restoreSingleFile: async (sessionId, projectRoot, filePath, hash) => {
    try {
      const resp = await invoke<CommandResponse<boolean>>('restore_single_file', {
        sessionId,
        projectRoot,
        filePath,
        targetHash: hash,
      });
      return resp.success && resp.data === true;
    } catch {
      return false;
    }
  },

  truncateFromTurn: async (sessionId, projectRoot, turnIndex) => {
    try {
      await invoke<CommandResponse<boolean>>('truncate_changes_from_turn', { sessionId, projectRoot, turnIndex });
    } catch {
      // Best-effort cleanup
    }
  },

  selectTurn: (turnIndex) => set({ selectedTurnIndex: turnIndex }),

  prefillDiffCache: (changeId, diff) => {
    const existing = get().diffCache;
    if (existing.has(changeId)) return;
    const newCache = new Map(existing);
    newCache.set(changeId, diff);
    set({ diffCache: newCache });
  },

  toggleExpanded: (changeId) => {
    const expanded = new Set(get().expandedChangeIds);
    if (expanded.has(changeId)) {
      expanded.delete(changeId);
    } else {
      expanded.add(changeId);
    }
    set({ expandedChangeIds: expanded });
  },

  reset: () =>
    set({
      turnChanges: [],
      loading: false,
      error: null,
      selectedTurnIndex: null,
      expandedChangeIds: new Set(),
      diffCache: new Map(),
    }),
}));
