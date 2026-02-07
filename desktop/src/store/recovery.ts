/**
 * Recovery Store
 *
 * Manages recovery state for detecting and resuming interrupted executions.
 * Uses Zustand for state management with Tauri command integration.
 *
 * Story-004: Resume & Recovery System
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// ============================================================================
// Types
// ============================================================================

/** Execution mode matching Rust ExecutionMode */
export type ExecutionMode = 'direct' | 'hybrid_auto' | 'hybrid_worktree' | 'mega_plan';

/** Execution mode display labels */
export const EXECUTION_MODE_LABELS: Record<ExecutionMode, string> = {
  direct: 'Direct',
  hybrid_auto: 'Hybrid Auto',
  hybrid_worktree: 'Hybrid Worktree',
  mega_plan: 'Mega Plan',
};

/** An incomplete task detected by the recovery detector */
export interface IncompleteTask {
  /** Unique execution identifier */
  id: string;
  /** Associated session identifier */
  session_id: string | null;
  /** Human-readable task name */
  name: string;
  /** Execution mode that was running */
  execution_mode: ExecutionMode;
  /** Current status when interrupted */
  status: string;
  /** Project path for the execution */
  project_path: string;
  /** Total number of stories */
  total_stories: number;
  /** Stories completed before interruption */
  completed_stories: number;
  /** Story that was active when interrupted */
  current_story_id: string | null;
  /** Progress percentage (0-100) */
  progress: number;
  /** Last checkpoint/update timestamp (ISO 8601) */
  last_checkpoint_timestamp: string | null;
  /** Whether the execution can be resumed */
  recoverable: boolean;
  /** Reason if not recoverable */
  recovery_note: string | null;
  /** Number of checkpoints available */
  checkpoint_count: number;
  /** Error message if the execution had failed */
  error_message: string | null;
}

/** Restored context returned by resume */
export interface RestoredContext {
  execution_id: string;
  execution_mode: ExecutionMode;
  project_path: string;
  name: string;
  completed_story_ids: string[];
  remaining_story_ids: string[];
  context_snapshot: Record<string, unknown>;
  total_stories: number;
  completed_stories: number;
  progress: number;
}

/** Resume result from the backend */
export interface ResumeResult {
  success: boolean;
  execution_id: string;
  context: RestoredContext | null;
  error: string | null;
  events: ResumeEvent[];
}

/** Resume event for progress tracking */
export interface ResumeEvent {
  type: 'Started' | 'ContextRestored' | 'StorySkipped' | 'Resuming' | 'Completed' | 'Error';
  payload: Record<string, unknown>;
}

/** Standard command response from Tauri */
interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

// ============================================================================
// Store
// ============================================================================

export interface RecoveryState {
  /** List of detected incomplete tasks */
  incompleteTasks: IncompleteTask[];

  /** Whether detection is in progress */
  isDetecting: boolean;

  /** Whether a resume operation is in progress */
  isResuming: boolean;

  /** The task ID currently being resumed */
  resumingTaskId: string | null;

  /** Whether the recovery prompt should be shown */
  showPrompt: boolean;

  /** Error from last operation */
  error: string | null;

  /** Resume events received during recovery */
  resumeEvents: ResumeEvent[];

  /** Last resume result */
  lastResumeResult: ResumeResult | null;

  // Actions

  /** Detect incomplete tasks by scanning the database */
  detectIncompleteTasks: () => Promise<IncompleteTask[]>;

  /** Resume an interrupted task */
  resumeTask: (taskId: string) => Promise<ResumeResult | null>;

  /** Discard an interrupted task */
  discardTask: (taskId: string) => Promise<boolean>;

  /** Dismiss the recovery prompt */
  dismissPrompt: () => void;

  /** Show the recovery prompt */
  showRecoveryPrompt: () => void;

  /** Clear error state */
  clearError: () => void;

  /** Initialize resume event listener */
  initializeListener: () => Promise<void>;

  /** Cleanup event listener */
  cleanupListener: () => void;
}

let resumeUnlisten: UnlistenFn | null = null;

export const useRecoveryStore = create<RecoveryState>()((set, get) => ({
  incompleteTasks: [],
  isDetecting: false,
  isResuming: false,
  resumingTaskId: null,
  showPrompt: false,
  error: null,
  resumeEvents: [],
  lastResumeResult: null,

  detectIncompleteTasks: async () => {
    set({ isDetecting: true, error: null });

    try {
      const result = await invoke<CommandResponse<IncompleteTask[]>>('detect_incomplete_tasks');

      if (result.success && result.data) {
        const tasks = result.data;
        set({
          incompleteTasks: tasks,
          isDetecting: false,
          showPrompt: tasks.length > 0,
        });
        return tasks;
      } else {
        throw new Error(result.error || 'Failed to detect incomplete tasks');
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({
        isDetecting: false,
        error: errorMessage,
        incompleteTasks: [],
        showPrompt: false,
      });
      return [];
    }
  },

  resumeTask: async (taskId: string) => {
    set({ isResuming: true, resumingTaskId: taskId, error: null, resumeEvents: [] });

    try {
      const result = await invoke<CommandResponse<ResumeResult>>('resume_task', {
        taskId,
      });

      if (result.success && result.data) {
        const resumeResult = result.data;

        set({
          isResuming: false,
          resumingTaskId: null,
          lastResumeResult: resumeResult,
          // Remove the resumed task from the list
          incompleteTasks: get().incompleteTasks.filter((t) => t.id !== taskId),
        });

        // Hide prompt if no more tasks
        if (get().incompleteTasks.length === 0) {
          set({ showPrompt: false });
        }

        return resumeResult;
      } else {
        throw new Error(result.error || 'Failed to resume task');
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({
        isResuming: false,
        resumingTaskId: null,
        error: errorMessage,
      });
      return null;
    }
  },

  discardTask: async (taskId: string) => {
    set({ error: null });

    try {
      const result = await invoke<CommandResponse<string>>('discard_task', {
        taskId,
      });

      if (result.success) {
        // Remove the discarded task from the list
        const remaining = get().incompleteTasks.filter((t) => t.id !== taskId);
        set({
          incompleteTasks: remaining,
          showPrompt: remaining.length > 0,
        });
        return true;
      } else {
        throw new Error(result.error || 'Failed to discard task');
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({ error: errorMessage });
      return false;
    }
  },

  dismissPrompt: () => {
    set({ showPrompt: false });
  },

  showRecoveryPrompt: () => {
    if (get().incompleteTasks.length > 0) {
      set({ showPrompt: true });
    }
  },

  clearError: () => {
    set({ error: null });
  },

  initializeListener: async () => {
    // Clean up any existing listener
    get().cleanupListener();

    try {
      resumeUnlisten = await listen<ResumeEvent>('recovery:resume', (event) => {
        set((state) => ({
          resumeEvents: [...state.resumeEvents, event.payload],
        }));
      });
    } catch (error) {
      console.error('Failed to set up recovery event listener:', error);
    }
  },

  cleanupListener: () => {
    if (resumeUnlisten) {
      resumeUnlisten();
      resumeUnlisten = null;
    }
  },
}));

export default useRecoveryStore;
