/**
 * Git Store
 *
 * Zustand store for git state management. Provides reactive access to
 * repository status, staging, commits, stash, and merge operations via
 * Tauri IPC commands.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from '../lib/tauri';
import { useSettingsStore } from './settings';

// ============================================================================
// Git Types (mirroring Rust types in services/git/types.rs)
// ============================================================================

export type FileStatusKind =
  | 'added'
  | 'modified'
  | 'deleted'
  | 'renamed'
  | 'copied'
  | 'untracked'
  | 'ignored'
  | 'unmerged'
  | 'type_changed';

export interface FileStatus {
  path: string;
  original_path?: string;
  kind: FileStatusKind;
}

export interface GitFullStatus {
  staged: FileStatus[];
  unstaged: FileStatus[];
  untracked: FileStatus[];
  conflicted: FileStatus[];
  branch: string;
  upstream?: string;
  ahead: number;
  behind: number;
}

export interface DiffLineKind {
  context: 'context';
  addition: 'addition';
  deletion: 'deletion';
  hunk_header: 'hunk_header';
}

export interface DiffLine {
  kind: 'context' | 'addition' | 'deletion' | 'hunk_header';
  content: string;
  old_line_no?: number;
  new_line_no?: number;
}

export interface DiffHunk {
  header: string;
  old_start: number;
  old_count: number;
  new_start: number;
  new_count: number;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  is_new: boolean;
  is_deleted: boolean;
  is_renamed: boolean;
  old_path?: string;
  hunks: DiffHunk[];
}

export interface DiffOutput {
  files: FileDiff[];
  total_additions: number;
  total_deletions: number;
}

export interface StashEntry {
  index: number;
  message: string;
  date: string;
}

export type MergeStateKind = 'none' | 'merging' | 'rebasing' | 'cherry_picking' | 'reverting';

export interface MergeState {
  kind: MergeStateKind;
  head: string;
  merge_head?: string;
  branch_name?: string;
}

export interface CommitNode {
  sha: string;
  short_sha: string;
  parents: string[];
  author_name: string;
  author_email: string;
  date: string;
  message: string;
  refs: string[];
}

export interface BranchInfo {
  name: string;
  is_head: boolean;
  tip_sha: string;
  upstream?: string;
  ahead: number;
  behind: number;
  last_commit_message?: string;
}

// ============================================================================
// Store Types
// ============================================================================

export type GitTabId = 'changes' | 'history' | 'branches';

interface GitState {
  // Data
  status: GitFullStatus | null;
  stagedDiffs: DiffOutput | null;
  unstagedDiffs: DiffOutput | null;
  selectedTab: GitTabId;
  mergeState: MergeState | null;
  stashList: StashEntry[];
  commitLog: CommitNode[];
  branches: BranchInfo[];

  // UI state
  isLoading: boolean;
  error: string | null;
  commitMessage: string;
  isAmend: boolean;

  // Actions
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
  // Initial state
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

  // ---- Actions ----

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
      // Diff failures are non-critical; status still works
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
      set({ mergeState });
    } catch {
      set({ mergeState: null });
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
      // Refresh status and diffs after staging
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
}));

export default useGitStore;
