/**
 * Timeline Store
 *
 * Manages timeline state for checkpoint and branch management.
 * Uses Zustand for state management with Tauri command integration.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  Checkpoint,
  CheckpointBranch,
  CheckpointDiff,
  RestoreResult,
  TimelineMetadata,
} from '../types/timeline';
import type { CommandResponse } from '../types/project';

interface TimelineState {
  /** Current project path */
  projectPath: string | null;

  /** Current session ID */
  sessionId: string | null;

  /** Timeline metadata for the current session */
  timeline: TimelineMetadata | null;

  /** Currently selected checkpoint */
  selectedCheckpoint: Checkpoint | null;

  /** Checkpoint being compared from */
  compareFromCheckpoint: Checkpoint | null;

  /** Checkpoint diff for comparison */
  diff: CheckpointDiff | null;

  /** Loading states */
  loading: {
    timeline: boolean;
    checkpoint: boolean;
    diff: boolean;
    restore: boolean;
  };

  /** Error message */
  error: string | null;

  /** Actions */
  setSession: (projectPath: string, sessionId: string) => void;
  fetchTimeline: () => Promise<void>;
  createCheckpoint: (label: string, trackedFiles: string[]) => Promise<Checkpoint | null>;
  selectCheckpoint: (checkpoint: Checkpoint | null) => void;
  deleteCheckpoint: (checkpointId: string) => Promise<boolean>;
  restoreCheckpoint: (checkpointId: string, createBackup: boolean, trackedFiles: string[]) => Promise<RestoreResult | null>;
  forkBranch: (checkpointId: string, branchName: string) => Promise<CheckpointBranch | null>;
  switchBranch: (branchId: string) => Promise<boolean>;
  deleteBranch: (branchId: string) => Promise<boolean>;
  renameBranch: (branchId: string, newName: string) => Promise<boolean>;
  setCompareFrom: (checkpoint: Checkpoint | null) => void;
  calculateDiff: (fromId: string, toId: string) => Promise<CheckpointDiff | null>;
  clearError: () => void;
  reset: () => void;
}

export const useTimelineStore = create<TimelineState>((set, get) => ({
  projectPath: null,
  sessionId: null,
  timeline: null,
  selectedCheckpoint: null,
  compareFromCheckpoint: null,
  diff: null,
  loading: {
    timeline: false,
    checkpoint: false,
    diff: false,
    restore: false,
  },
  error: null,

  setSession: (projectPath: string, sessionId: string) => {
    set({
      projectPath,
      sessionId,
      timeline: null,
      selectedCheckpoint: null,
      compareFromCheckpoint: null,
      diff: null,
      error: null,
    });
    get().fetchTimeline();
  },

  fetchTimeline: async () => {
    const { projectPath, sessionId } = get();
    if (!projectPath || !sessionId) return;

    set((state) => ({
      loading: { ...state.loading, timeline: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<TimelineMetadata>>('get_timeline', {
        projectPath,
        sessionId,
      });

      if (response.success && response.data) {
        set((state) => ({
          timeline: response.data,
          loading: { ...state.loading, timeline: false },
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to fetch timeline',
          loading: { ...state.loading, timeline: false },
        }));
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to fetch timeline',
        loading: { ...state.loading, timeline: false },
      }));
    }
  },

  createCheckpoint: async (label: string, trackedFiles: string[]) => {
    const { projectPath, sessionId } = get();
    if (!projectPath || !sessionId) return null;

    set((state) => ({
      loading: { ...state.loading, checkpoint: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<Checkpoint>>('create_checkpoint', {
        projectPath,
        sessionId,
        label,
        trackedFiles,
      });

      if (response.success && response.data) {
        // Refresh timeline to include new checkpoint
        await get().fetchTimeline();
        set((state) => ({
          loading: { ...state.loading, checkpoint: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to create checkpoint',
          loading: { ...state.loading, checkpoint: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to create checkpoint',
        loading: { ...state.loading, checkpoint: false },
      }));
      return null;
    }
  },

  selectCheckpoint: (checkpoint: Checkpoint | null) => {
    set({ selectedCheckpoint: checkpoint, diff: null });
  },

  deleteCheckpoint: async (checkpointId: string) => {
    const { projectPath, sessionId } = get();
    if (!projectPath || !sessionId) return false;

    try {
      const response = await invoke<CommandResponse<void>>('delete_checkpoint', {
        projectPath,
        sessionId,
        checkpointId,
      });

      if (response.success) {
        await get().fetchTimeline();
        // Clear selection if deleted checkpoint was selected
        if (get().selectedCheckpoint?.id === checkpointId) {
          set({ selectedCheckpoint: null });
        }
        return true;
      } else {
        set({ error: response.error || 'Failed to delete checkpoint' });
        return false;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to delete checkpoint' });
      return false;
    }
  },

  restoreCheckpoint: async (checkpointId: string, createBackup: boolean, trackedFiles: string[]) => {
    const { projectPath, sessionId } = get();
    if (!projectPath || !sessionId) return null;

    set((state) => ({
      loading: { ...state.loading, restore: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<RestoreResult>>('restore_checkpoint', {
        projectPath,
        sessionId,
        checkpointId,
        createBackup,
        currentTrackedFiles: trackedFiles,
      });

      if (response.success && response.data) {
        await get().fetchTimeline();
        set((state) => ({
          loading: { ...state.loading, restore: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to restore checkpoint',
          loading: { ...state.loading, restore: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to restore checkpoint',
        loading: { ...state.loading, restore: false },
      }));
      return null;
    }
  },

  forkBranch: async (checkpointId: string, branchName: string) => {
    const { projectPath, sessionId } = get();
    if (!projectPath || !sessionId) return null;

    try {
      const response = await invoke<CommandResponse<CheckpointBranch>>('fork_branch', {
        projectPath,
        sessionId,
        checkpointId,
        branchName,
      });

      if (response.success && response.data) {
        await get().fetchTimeline();
        return response.data;
      } else {
        set({ error: response.error || 'Failed to create branch' });
        return null;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to create branch' });
      return null;
    }
  },

  switchBranch: async (branchId: string) => {
    const { projectPath, sessionId } = get();
    if (!projectPath || !sessionId) return false;

    try {
      const response = await invoke<CommandResponse<CheckpointBranch>>('switch_branch', {
        projectPath,
        sessionId,
        branchId,
      });

      if (response.success) {
        await get().fetchTimeline();
        return true;
      } else {
        set({ error: response.error || 'Failed to switch branch' });
        return false;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to switch branch' });
      return false;
    }
  },

  deleteBranch: async (branchId: string) => {
    const { projectPath, sessionId } = get();
    if (!projectPath || !sessionId) return false;

    try {
      const response = await invoke<CommandResponse<void>>('delete_branch', {
        projectPath,
        sessionId,
        branchId,
      });

      if (response.success) {
        await get().fetchTimeline();
        return true;
      } else {
        set({ error: response.error || 'Failed to delete branch' });
        return false;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to delete branch' });
      return false;
    }
  },

  renameBranch: async (branchId: string, newName: string) => {
    const { projectPath, sessionId } = get();
    if (!projectPath || !sessionId) return false;

    try {
      const response = await invoke<CommandResponse<CheckpointBranch>>('rename_branch', {
        projectPath,
        sessionId,
        branchId,
        newName,
      });

      if (response.success) {
        await get().fetchTimeline();
        return true;
      } else {
        set({ error: response.error || 'Failed to rename branch' });
        return false;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to rename branch' });
      return false;
    }
  },

  setCompareFrom: (checkpoint: Checkpoint | null) => {
    set({ compareFromCheckpoint: checkpoint, diff: null });
  },

  calculateDiff: async (fromId: string, toId: string) => {
    const { projectPath, sessionId } = get();
    if (!projectPath || !sessionId) return null;

    set((state) => ({
      loading: { ...state.loading, diff: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<CheckpointDiff>>('get_checkpoint_diff', {
        projectPath,
        sessionId,
        fromCheckpointId: fromId,
        toCheckpointId: toId,
      });

      if (response.success && response.data) {
        set((state) => ({
          diff: response.data,
          loading: { ...state.loading, diff: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to calculate diff',
          loading: { ...state.loading, diff: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to calculate diff',
        loading: { ...state.loading, diff: false },
      }));
      return null;
    }
  },

  clearError: () => {
    set({ error: null });
  },

  reset: () => {
    set({
      projectPath: null,
      sessionId: null,
      timeline: null,
      selectedCheckpoint: null,
      compareFromCheckpoint: null,
      diff: null,
      loading: {
        timeline: false,
        checkpoint: false,
        diff: false,
        restore: false,
      },
      error: null,
    });
  },
}));

export default useTimelineStore;
