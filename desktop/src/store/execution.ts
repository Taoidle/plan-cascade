/**
 * Execution Store (v5.0 Pure Rust Backend)
 *
 * Manages task execution state with real-time updates from Tauri events.
 * Replaces the legacy WebSocket-based approach with Tauri IPC.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export type ExecutionStatus = 'idle' | 'running' | 'paused' | 'completed' | 'failed';

export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'reconnecting';

export type Strategy = 'direct' | 'hybrid_auto' | 'mega_plan' | null;

export interface Story {
  id: string;
  title: string;
  description?: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  progress: number;
  error?: string;
  startedAt?: string;
  completedAt?: string;
  retryCount?: number;
}

export interface Batch {
  batchNum: number;
  totalBatches: number;
  storyIds: string[];
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  startedAt?: string;
  completedAt?: string;
}

export interface ExecutionResult {
  success: boolean;
  message: string;
  completedStories: number;
  totalStories: number;
  duration: number;
  error?: string;
}

export interface ExecutionHistoryItem {
  id: string;
  taskDescription: string;
  strategy: Strategy;
  status: ExecutionStatus;
  startedAt: number;
  completedAt?: number;
  duration: number;
  completedStories: number;
  totalStories: number;
  success: boolean;
  error?: string;
}

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

interface ExecutionState {
  /** Current execution status */
  status: ExecutionStatus;

  /** Backend connection status (always connected in Tauri) */
  connectionStatus: ConnectionStatus;

  /** Task ID from server */
  taskId: string | null;

  /** Task description */
  taskDescription: string;

  /** Selected strategy */
  strategy: Strategy;

  /** List of stories */
  stories: Story[];

  /** List of batches */
  batches: Batch[];

  /** Current batch number */
  currentBatch: number;

  /** Currently executing story ID */
  currentStoryId: string | null;

  /** Overall progress (0-100) */
  progress: number;

  /** Execution result */
  result: ExecutionResult | null;

  /** Start timestamp */
  startedAt: number | null;

  /** Execution logs */
  logs: string[];

  /** Execution history */
  history: ExecutionHistoryItem[];

  /** Is submitting (API call in progress) */
  isSubmitting: boolean;

  /** API error message */
  apiError: string | null;

  // Actions
  /** Initialize Tauri event listeners */
  initialize: () => void;

  /** Cleanup event listeners */
  cleanup: () => void;

  /** Start execution */
  start: (description: string, mode: 'simple' | 'expert') => Promise<void>;

  /** Pause execution */
  pause: () => Promise<void>;

  /** Resume execution */
  resume: () => Promise<void>;

  /** Cancel execution */
  cancel: () => Promise<void>;

  /** Reset state */
  reset: () => void;

  /** Update story status */
  updateStory: (storyId: string, updates: Partial<Story>) => void;

  /** Add log entry */
  addLog: (message: string) => void;

  /** Set stories from server */
  setStories: (stories: Story[]) => void;

  /** Set strategy */
  setStrategy: (strategy: Strategy) => void;

  /** Load history from localStorage */
  loadHistory: () => void;

  /** Save to history */
  saveToHistory: () => void;

  /** Clear history */
  clearHistory: () => void;
}

const HISTORY_KEY = 'plan-cascade-execution-history';
const MAX_HISTORY_ITEMS = 10;

// Track event unlisteners
let unlisteners: UnlistenFn[] = [];

const initialState = {
  status: 'idle' as ExecutionStatus,
  connectionStatus: 'disconnected' as ConnectionStatus,
  taskId: null as string | null,
  taskDescription: '',
  strategy: null as Strategy,
  stories: [] as Story[],
  batches: [] as Batch[],
  currentBatch: 0,
  currentStoryId: null as string | null,
  progress: 0,
  result: null as ExecutionResult | null,
  startedAt: null as number | null,
  logs: [] as string[],
  history: [] as ExecutionHistoryItem[],
  isSubmitting: false,
  apiError: null as string | null,
};

export const useExecutionStore = create<ExecutionState>()((set, get) => ({
  ...initialState,

  initialize: () => {
    // In Tauri, we're always "connected" via IPC
    set({ connectionStatus: 'connected' });
    get().addLog('Connected to Rust backend');

    // Set up Tauri event listeners for execution updates
    setupTauriEventListeners(get, set);

    // Load history
    get().loadHistory();
  },

  cleanup: () => {
    // Clean up all event listeners
    for (const unlisten of unlisteners) {
      unlisten();
    }
    unlisteners = [];
    set({ connectionStatus: 'disconnected' });
  },

  start: async (description, mode) => {
    set({
      isSubmitting: true,
      apiError: null,
      status: 'running',
      taskDescription: description,
      startedAt: Date.now(),
      result: null,
      logs: [],
      stories: [],
      batches: [],
      currentBatch: 0,
      currentStoryId: null,
      progress: 0,
    });

    get().addLog(`Starting execution in ${mode} mode...`);
    get().addLog(`Task: ${description}`);

    try {
      // Use standalone execution via Tauri invoke
      const result = await invoke<CommandResponse<{ task_id: string }>>('execute_standalone', {
        message: description,
        provider: 'anthropic',
        model: 'claude-sonnet-4-20250514',
        project_path: '.',
      });

      if (result.success && result.data) {
        set({
          taskId: result.data.task_id,
          isSubmitting: false,
        });
        get().addLog(`Task started with ID: ${result.data.task_id}`);
      } else {
        throw new Error(result.error || 'Failed to start execution');
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';

      set({
        status: 'failed',
        isSubmitting: false,
        apiError: errorMessage,
        result: {
          success: false,
          message: 'Failed to start execution',
          completedStories: 0,
          totalStories: 0,
          duration: Date.now() - (get().startedAt || Date.now()),
          error: errorMessage,
        },
      });

      get().addLog(`Error: ${errorMessage}`);
      get().saveToHistory();
    }
  },

  pause: async () => {
    try {
      // Note: Pause may not be implemented in standalone mode
      set({ status: 'paused' });
      get().addLog('Execution paused');
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({ apiError: errorMessage });
      get().addLog(`Failed to pause: ${errorMessage}`);
    }
  },

  resume: async () => {
    try {
      // Note: Resume may not be implemented in standalone mode
      set({ status: 'running' });
      get().addLog('Execution resumed');
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({ apiError: errorMessage });
      get().addLog(`Failed to resume: ${errorMessage}`);
    }
  },

  cancel: async () => {
    try {
      // Cancel current session if running
      const { taskId } = get();
      if (taskId) {
        try {
          await invoke<CommandResponse<boolean>>('cancel_execution', {
            session_id: taskId,
          });
        } catch {
          // Session might not exist in the new architecture
        }
      }

      set({
        status: 'idle',
        currentStoryId: null,
        result: {
          success: false,
          message: 'Execution cancelled by user',
          completedStories: get().stories.filter((s) => s.status === 'completed').length,
          totalStories: get().stories.length,
          duration: Date.now() - (get().startedAt || Date.now()),
        },
      });
      get().addLog('Execution cancelled');
      get().saveToHistory();
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({ apiError: errorMessage });
      get().addLog(`Failed to cancel: ${errorMessage}`);
    }
  },

  reset: () => {
    set({
      ...initialState,
      connectionStatus: get().connectionStatus,
      history: get().history,
    });
  },

  updateStory: (storyId, updates) => {
    set((state) => ({
      stories: state.stories.map((s) =>
        s.id === storyId ? { ...s, ...updates } : s
      ),
    }));

    // Recalculate progress
    const stories = get().stories;
    if (stories.length > 0) {
      const completed = stories.filter((s) => s.status === 'completed').length;
      set({ progress: (completed / stories.length) * 100 });
    }
  },

  addLog: (message) => {
    const timestamp = new Date().toISOString().slice(11, 19);
    set((state) => ({
      logs: [...state.logs, `[${timestamp}] ${message}`],
    }));
  },

  setStories: (stories) => {
    set({ stories });
    get().addLog(`PRD loaded with ${stories.length} stories`);
  },

  setStrategy: (strategy) => {
    set({ strategy });
    get().addLog(`Strategy selected: ${strategy}`);
  },

  loadHistory: () => {
    try {
      const stored = localStorage.getItem(HISTORY_KEY);
      if (stored) {
        const history = JSON.parse(stored) as ExecutionHistoryItem[];
        set({ history });
      }
    } catch {
      // Ignore localStorage errors
    }
  },

  saveToHistory: () => {
    const state = get();
    if (!state.taskDescription) return;

    const historyItem: ExecutionHistoryItem = {
      id: state.taskId || `local_${Date.now()}`,
      taskDescription: state.taskDescription,
      strategy: state.strategy,
      status: state.status,
      startedAt: state.startedAt || Date.now(),
      completedAt: Date.now(),
      duration: Date.now() - (state.startedAt || Date.now()),
      completedStories: state.stories.filter((s) => s.status === 'completed').length,
      totalStories: state.stories.length,
      success: state.status === 'completed',
      error: state.result?.error,
    };

    set((prevState) => {
      const newHistory = [historyItem, ...prevState.history].slice(0, MAX_HISTORY_ITEMS);

      // Save to localStorage
      try {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(newHistory));
      } catch {
        // Ignore localStorage errors
      }

      return { history: newHistory };
    });
  },

  clearHistory: () => {
    try {
      localStorage.removeItem(HISTORY_KEY);
    } catch {
      // Ignore localStorage errors
    }
    set({ history: [] });
  },
}));

// ============================================================================
// Tauri Event Handlers
// ============================================================================

async function setupTauriEventListeners(
  get: () => ExecutionState,
  set: (partial: Partial<ExecutionState>) => void
) {
  // Clean up any existing listeners first
  for (const unlisten of unlisteners) {
    unlisten();
  }
  unlisteners = [];

  try {
    // Listen for stream events (execution progress)
    const unlistenStream = await listen<{
      event: { TextDelta?: { content: string }; Done?: Record<string, never>; Error?: { message: string } };
      session_id: string;
    }>('claude_code:stream', (event) => {
      const { event: streamEvent, session_id } = event.payload;

      // Only process events for current session
      const currentTaskId = get().taskId;
      if (currentTaskId && currentTaskId !== session_id) {
        return;
      }

      if (streamEvent.Error) {
        set({
          status: 'failed',
          apiError: streamEvent.Error.message,
          result: {
            success: false,
            message: 'Execution failed',
            completedStories: get().stories.filter((s) => s.status === 'completed').length,
            totalStories: get().stories.length,
            duration: Date.now() - (get().startedAt || Date.now()),
            error: streamEvent.Error.message,
          },
        });
        get().addLog(`Error: ${streamEvent.Error.message}`);
        get().saveToHistory();
      }

      if (streamEvent.Done) {
        const completedStories = get().stories.filter((s) => s.status === 'completed').length;
        const totalStories = get().stories.length;

        set({
          status: 'completed',
          progress: 100,
          result: {
            success: true,
            message: 'Execution completed',
            completedStories,
            totalStories,
            duration: Date.now() - (get().startedAt || Date.now()),
          },
        });
        get().addLog('Execution completed');
        get().saveToHistory();
      }
    });
    unlisteners.push(unlistenStream);

    // Listen for tool events
    const unlistenTool = await listen<{
      execution: { id: string; tool_name: string; success?: boolean };
      update_type: string;
      session_id: string;
    }>('claude_code:tool', (event) => {
      const { execution, update_type } = event.payload;

      if (update_type === 'started') {
        get().addLog(`Tool started: ${execution.tool_name}`);
      } else if (update_type === 'completed') {
        get().addLog(`Tool completed: ${execution.tool_name} (${execution.success ? 'success' : 'failed'})`);
      }
    });
    unlisteners.push(unlistenTool);

    // Listen for session events
    const unlistenSession = await listen<{
      session: { id: string; state: string; error_message?: string };
      update_type: string;
    }>('claude_code:session', (event) => {
      const { session, update_type } = event.payload;

      if (update_type === 'state_changed') {
        if (session.state === 'error') {
          set({
            status: 'failed',
            apiError: session.error_message || 'Session error',
          });
          get().addLog(`Session error: ${session.error_message || 'Unknown error'}`);
        } else if (session.state === 'cancelled') {
          set({ status: 'idle' });
          get().addLog('Session cancelled');
        }
      }
    });
    unlisteners.push(unlistenSession);
  } catch (error) {
    console.error('Failed to set up Tauri event listeners:', error);
  }
}

export default useExecutionStore;
