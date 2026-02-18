/**
 * Git Store
 *
 * Zustand store for git state management. Provides reactive access to
 * repository status, staging, commits, stash, merge/conflict operations,
 * and commit graph visualization state via Tauri IPC commands.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from '../lib/tauri';
import { useSettingsStore } from './settings';
import type {
  FileStatusKind,
  FileStatus,
  GitFullStatus,
  DiffOutput,
  StashEntry,
  MergeState,
  MergeStateKind,
  CommitNode,
  BranchInfo,
  GraphLayout,
  CompareSelection,
  DiffLine,
  DiffHunk,
  FileDiff,
  DiffLineKind,
  ConflictFile,
  MergeBranchResult,
} from '../types/git';

// Re-export types for consumers that import from store
export type {
  FileStatusKind,
  FileStatus,
  GitFullStatus,
  DiffOutput,
  DiffLine,
  DiffHunk,
  FileDiff,
  DiffLineKind,
  StashEntry,
  MergeStateKind,
  MergeState,
  CommitNode,
  BranchInfo,
  GraphLayout,
  CompareSelection,
  ConflictFile,
  MergeBranchResult,
};

// ============================================================================
// Store Types
// ============================================================================

export type GitTabId = 'changes' | 'history' | 'branches';

interface GitState {
  // --- Data (feature-002: Changes Tab) ---
  status: GitFullStatus | null;
  stagedDiffs: DiffOutput | null;
  unstagedDiffs: DiffOutput | null;
  selectedTab: GitTabId;
  mergeState: MergeState | null;
  stashList: StashEntry[];
  commitLog: CommitNode[];
  branches: BranchInfo[];

  // --- UI state (feature-002) ---
  isLoading: boolean;
  error: string | null;
  commitMessage: string;
  isAmend: boolean;

  // --- Graph UI state (feature-003: Commit Graph) ---
  selectedCommitSha: string | null;
  compareSelection: CompareSelection | null;
  commitDetailExpanded: boolean;
  graphLayout: GraphLayout | null;
  branchFilter: string | null;
  searchQuery: string;
  selectedCommitDiff: DiffOutput | null;

  // --- Merge/Conflict state (feature-004: Branches & Merge) ---
  mergeSourceBranch: string | null;
  conflictFiles: ConflictFile[];
  resolvedFiles: Set<string>;
  isInMerge: boolean;
  isFetching: boolean;
  isPulling: boolean;
  isPushing: boolean;

  // --- Actions (feature-002: operational) ---
  refreshStatus: () => Promise<void>;
  refreshDiffs: () => Promise<void>;
  refreshStashList: () => Promise<void>;
  refreshMergeState: () => Promise<void>;
  refreshAll: () => Promise<void>;
  stageFiles: (paths: string[]) => Promise<void>;
  unstageFiles: (paths: string[]) => Promise<void>;
  stageAll: () => Promise<void>;
  discardChanges: (paths: string[]) => Promise<void>;
  commit: (message: string, amend?: boolean) => Promise<boolean>;
  stashSave: (message?: string) => Promise<void>;
  stashPop: (index?: number) => Promise<void>;
  stashDrop: (index: number) => Promise<void>;
  setSelectedTab: (tab: GitTabId) => void;
  setCommitMessage: (message: string) => void;
  setIsAmend: (amend: boolean) => void;
  setError: (error: string | null) => void;
  getDiffForFile: (filePath: string) => Promise<DiffOutput | null>;
  stageHunk: (filePath: string, hunkIndex: number, isStaged: boolean) => Promise<void>;

  // --- Actions (feature-003: graph UI) ---
  setSelectedCommitSha: (sha: string | null) => void;
  setCompareSelection: (selection: CompareSelection | null) => void;
  setCommitDetailExpanded: (expanded: boolean) => void;
  setCommits: (commits: CommitNode[]) => void;
  setGraphLayout: (layout: GraphLayout | null) => void;
  setBranches: (branches: BranchInfo[]) => void;
  setBranchFilter: (branch: string | null) => void;
  setSearchQuery: (query: string) => void;
  setSelectedCommitDiff: (diff: DiffOutput | null) => void;
  resetGraphState: () => void;

  // --- Actions (feature-004: merge/conflict) ---
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

// ============================================================================
// Helpers
// ============================================================================

function getRepoPath(): string | null {
  return useSettingsStore.getState().workspacePath;
}

async function invokeGit<T>(command: string, args: Record<string, unknown>): Promise<T> {
  const response = await invoke<CommandResponse<T>>(command, args);
  if (!response.success || response.data === null) {
    throw new Error(response.error || `${command} failed`);
  }
  return response.data;
}

// ============================================================================
// Store
// ============================================================================

export const useGitStore = create<GitState>((set, get) => ({
  // --- Initial state (feature-002) ---
  status: null,
  stagedDiffs: null,
  unstagedDiffs: null,
  selectedTab: 'changes',
  mergeState: null,
  stashList: [],
  commitLog: [],
  branches: [],
  isLoading: false,
  error: null,
  commitMessage: '',
  isAmend: false,

  // --- Initial state (feature-003: graph) ---
  selectedCommitSha: null,
  compareSelection: null,
  commitDetailExpanded: false,
  graphLayout: null,
  branchFilter: null,
  searchQuery: '',
  selectedCommitDiff: null,

  // --- Initial state (feature-004: merge/conflict) ---
  mergeSourceBranch: null,
  conflictFiles: [],
  resolvedFiles: new Set(),
  isInMerge: false,
  isFetching: false,
  isPulling: false,
  isPushing: false,

  // ---- Operational Actions (feature-002) ----

  refreshStatus: async () => {
    const repoPath = getRepoPath();
    if (!repoPath) return;

    set({ isLoading: true, error: null });
    try {
      const status = await invokeGit<GitFullStatus>('git_full_status', {
        repoPath,
      });
      set({ status, isLoading: false });
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoading: false,
      });
    }
  },

  refreshDiffs: async () => {
    const repoPath = getRepoPath();
    if (!repoPath) return;

    try {
      const [staged, unstaged] = await Promise.all([
        invokeGit<DiffOutput>('git_diff_staged', { repoPath }),
        invokeGit<DiffOutput>('git_diff_unstaged', { repoPath }),
      ]);
      set({ stagedDiffs: staged, unstagedDiffs: unstaged });
    } catch (e) {
      console.warn('Failed to refresh diffs:', e);
    }
  },

  refreshStashList: async () => {
    const repoPath = getRepoPath();
    if (!repoPath) return;

    try {
      const stashes = await invokeGit<StashEntry[]>('git_list_stashes', {
        repoPath,
      });
      set({ stashList: stashes });
    } catch {
      // Stash list failure is non-critical
    }
  },

  refreshMergeState: async () => {
    const repoPath = getRepoPath();
    if (!repoPath) return;

    try {
      const mergeState = await invokeGit<MergeState>('git_get_merge_state', {
        repoPath,
      });
      const isInMerge = mergeState.kind !== 'none';
      set({ mergeState, isInMerge });
      if (isInMerge) {
        await get().refreshConflictFiles(repoPath);
      }
    } catch {
      set({ mergeState: null, isInMerge: false });
    }
  },

  refreshAll: async () => {
    const { refreshStatus, refreshDiffs, refreshStashList, refreshMergeState } = get();
    await Promise.all([
      refreshStatus(),
      refreshDiffs(),
      refreshStashList(),
      refreshMergeState(),
    ]);
  },

  stageFiles: async (paths: string[]) => {
    const repoPath = getRepoPath();
    if (!repoPath || paths.length === 0) return;

    try {
      await invokeGit<null>('git_stage_files', { repoPath, paths });
      await Promise.all([get().refreshStatus(), get().refreshDiffs()]);
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  unstageFiles: async (paths: string[]) => {
    const repoPath = getRepoPath();
    if (!repoPath || paths.length === 0) return;

    try {
      await invokeGit<null>('git_unstage_files', { repoPath, paths });
      await Promise.all([get().refreshStatus(), get().refreshDiffs()]);
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  stageAll: async () => {
    const { status } = get();
    if (!status) return;

    const allPaths = [
      ...status.unstaged.map((f) => f.path),
      ...status.untracked.map((f) => f.path),
    ];
    if (allPaths.length > 0) {
      await get().stageFiles(allPaths);
    }
  },

  discardChanges: async (paths: string[]) => {
    const repoPath = getRepoPath();
    if (!repoPath || paths.length === 0) return;

    try {
      await invokeGit<null>('git_discard_changes', { repoPath, paths });
      await Promise.all([get().refreshStatus(), get().refreshDiffs()]);
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  commit: async (message: string, amend?: boolean) => {
    const repoPath = getRepoPath();
    if (!repoPath || !message.trim()) return false;

    try {
      const command = amend ? 'git_amend_commit' : 'git_commit';
      await invokeGit<string>(command, { repoPath, message });
      set({ commitMessage: '', isAmend: false });
      await get().refreshAll();
      return true;
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },

  stashSave: async (message?: string) => {
    const repoPath = getRepoPath();
    if (!repoPath) return;

    try {
      await invokeGit<null>('git_stash_save', { repoPath, message: message || null });
      await get().refreshAll();
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  stashPop: async (index?: number) => {
    const repoPath = getRepoPath();
    if (!repoPath) return;

    try {
      await invokeGit<null>('git_stash_pop', { repoPath, index: index ?? null });
      await get().refreshAll();
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  stashDrop: async (index: number) => {
    const repoPath = getRepoPath();
    if (!repoPath) return;

    try {
      await invokeGit<null>('git_stash_drop', { repoPath, index });
      await get().refreshStashList();
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  setSelectedTab: (tab: GitTabId) => set({ selectedTab: tab }),
  setCommitMessage: (message: string) => set({ commitMessage: message }),
  setIsAmend: (amend: boolean) => set({ isAmend: amend }),
  setError: (error: string | null) => set({ error }),

  getDiffForFile: async (filePath: string) => {
    const repoPath = getRepoPath();
    if (!repoPath) return null;

    try {
      return await invokeGit<DiffOutput>('git_diff_file', { repoPath, filePath });
    } catch {
      return null;
    }
  },

  stageHunk: async (filePath: string, hunkIndex: number, isStaged: boolean) => {
    const repoPath = getRepoPath();
    if (!repoPath) return;

    try {
      await invokeGit<null>('git_stage_hunk', {
        repoPath,
        filePath,
        hunkIndex,
        reverse: isStaged,
      });
      await Promise.all([get().refreshStatus(), get().refreshDiffs()]);
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  // ---- Graph UI Actions (feature-003) ----

  setSelectedCommitSha: (sha) =>
    set({
      selectedCommitSha: sha,
      commitDetailExpanded: sha !== null,
      compareSelection: null,
    }),

  setCompareSelection: (selection) => set({ compareSelection: selection }),

  setCommitDetailExpanded: (expanded) => set({ commitDetailExpanded: expanded }),

  setCommits: (commits) => set({ commitLog: commits }),

  setGraphLayout: (layout) => set({ graphLayout: layout }),

  setBranches: (branches) => set({ branches }),

  setBranchFilter: (branch) => set({ branchFilter: branch }),

  setSearchQuery: (query) => set({ searchQuery: query }),

  setSelectedCommitDiff: (diff) => set({ selectedCommitDiff: diff }),

  resetGraphState: () =>
    set({
      selectedCommitSha: null,
      compareSelection: null,
      commitDetailExpanded: false,
      graphLayout: null,
      branchFilter: null,
      searchQuery: '',
      selectedCommitDiff: null,
    }),

  // ---- Merge/Conflict Actions (feature-004) ----

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
          await get().refreshMergeState();
          const rp = getRepoPath();
          if (rp) await get().refreshConflictFiles(rp);
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

export default useGitStore;
