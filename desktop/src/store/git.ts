/**
 * Git Store
 *
 * Zustand store for Git state management.
 * Handles merge state, conflict tracking, and cross-tab communication.
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  MergeState,
  MergeStateKind,
  ConflictFile,
  MergeBranchResult,
  CommandResponse,
} from '../types/git';

interface GitStoreState {
  // Merge state
  mergeState: MergeState | null;
  mergeSourceBranch: string | null;
  conflictFiles: ConflictFile[];
  resolvedFiles: Set<string>;
  isInMerge: boolean;
  isFetching: boolean;
  isPulling: boolean;
  isPushing: boolean;

  // Actions
  refreshMergeState: (repoPath: string) => Promise<void>;
  startMerge: (repoPath: string, branch: string) => Promise<MergeBranchResult | null>;
  abortMerge: (repoPath: string) => Promise<boolean>;
  completeMerge: (repoPath: string) => Promise<boolean>;
  refreshConflictFiles: (repoPath: string) => Promise<void>;
  markFileResolved: (filePath: string) => void;
  markFileUnresolved: (filePath: string) => void;
  clearMergeState: () => void;
  setFetching: (v: boolean) => void;
  setPulling: (v: boolean) => void;
  setPushing: (v: boolean) => void;
}

export const useGitStore = create<GitStoreState>((set, get) => ({
  mergeState: null,
  mergeSourceBranch: null,
  conflictFiles: [],
  resolvedFiles: new Set(),
  isInMerge: false,
  isFetching: false,
  isPulling: false,
  isPushing: false,

  refreshMergeState: async (repoPath: string) => {
    try {
      const res = await invoke<CommandResponse<MergeState>>('git_get_merge_state', {
        repoPath,
      });
      if (res.success && res.data) {
        const isInMerge = res.data.kind !== 'none';
        set({
          mergeState: res.data,
          isInMerge,
        });
        if (isInMerge) {
          await get().refreshConflictFiles(repoPath);
        }
      }
    } catch {
      // Silently fail
    }
  },

  startMerge: async (repoPath: string, branch: string): Promise<MergeBranchResult | null> => {
    try {
      set({ mergeSourceBranch: branch });
      const res = await invoke<CommandResponse<MergeBranchResult>>('git_merge_branch', {
        repoPath,
        branch,
      });
      if (res.success && res.data) {
        const result = res.data;
        if (result.has_conflicts) {
          set({ isInMerge: true });
          await get().refreshMergeState(repoPath);
          await get().refreshConflictFiles(repoPath);
        } else if (result.success) {
          set({ isInMerge: false, mergeSourceBranch: null });
        }
        return result;
      }
      return null;
    } catch {
      return null;
    }
  },

  abortMerge: async (repoPath: string): Promise<boolean> => {
    try {
      const res = await invoke<CommandResponse<void>>('git_merge_abort', { repoPath });
      if (res.success) {
        set({
          isInMerge: false,
          mergeState: null,
          mergeSourceBranch: null,
          conflictFiles: [],
          resolvedFiles: new Set(),
        });
        return true;
      }
      return false;
    } catch {
      return false;
    }
  },

  completeMerge: async (repoPath: string): Promise<boolean> => {
    try {
      const res = await invoke<CommandResponse<string>>('git_merge_continue', { repoPath });
      if (res.success) {
        set({
          isInMerge: false,
          mergeState: null,
          mergeSourceBranch: null,
          conflictFiles: [],
          resolvedFiles: new Set(),
        });
        return true;
      }
      return false;
    } catch {
      return false;
    }
  },

  refreshConflictFiles: async (repoPath: string) => {
    try {
      const res = await invoke<CommandResponse<ConflictFile[]>>('git_get_conflict_files', {
        repoPath,
      });
      if (res.success && res.data) {
        set({ conflictFiles: res.data });
      }
    } catch {
      // Silently fail
    }
  },

  markFileResolved: (filePath: string) => {
    set((state) => {
      const next = new Set(state.resolvedFiles);
      next.add(filePath);
      return { resolvedFiles: next };
    });
  },

  markFileUnresolved: (filePath: string) => {
    set((state) => {
      const next = new Set(state.resolvedFiles);
      next.delete(filePath);
      return { resolvedFiles: next };
    });
  },

  clearMergeState: () => {
    set({
      isInMerge: false,
      mergeState: null,
      mergeSourceBranch: null,
      conflictFiles: [],
      resolvedFiles: new Set(),
    });
  },

  setFetching: (v: boolean) => set({ isFetching: v }),
  setPulling: (v: boolean) => set({ isPulling: v }),
  setPushing: (v: boolean) => set({ isPushing: v }),
}));
