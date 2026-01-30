/**
 * Execution Store
 *
 * Manages task execution state with real-time updates from WebSocket.
 * Replaces mock simulation with actual backend communication.
 */

import { create } from 'zustand';
import { api, ApiError } from '../lib/api';
import {
  getWebSocketManager,
  initWebSocket,
  ConnectionStatus,
  ServerEventType,
} from '../lib/websocket';

export type ExecutionStatus = 'idle' | 'running' | 'paused' | 'completed' | 'failed';

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

interface ExecutionState {
  /** Current execution status */
  status: ExecutionStatus;

  /** WebSocket connection status */
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
  /** Initialize WebSocket connection */
  initialize: () => void;

  /** Cleanup WebSocket connection */
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
    const wsManager = initWebSocket();

    // Subscribe to connection status changes
    wsManager.onStatusChange((status) => {
      set({ connectionStatus: status });
      if (status === 'connected') {
        get().addLog('Connected to server');
        // Request current status when reconnected
        wsManager.requestStatus();
      } else if (status === 'disconnected') {
        get().addLog('Disconnected from server');
      } else if (status === 'reconnecting') {
        get().addLog('Reconnecting to server...');
      }
    });

    // Subscribe to all events
    wsManager.on('*', (data) => {
      const eventType = data.type as ServerEventType;
      handleWebSocketEvent(eventType, data, get, set);
    });

    // Load history
    get().loadHistory();
  },

  cleanup: () => {
    const wsManager = getWebSocketManager();
    wsManager.disconnect();
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
      const response = await api.execute({
        description,
        mode,
      });

      set({
        taskId: response.task_id,
        isSubmitting: false,
      });

      get().addLog(`Task started with ID: ${response.task_id}`);
    } catch (error) {
      const errorMessage = error instanceof ApiError
        ? error.detail || error.message
        : error instanceof Error
          ? error.message
          : 'Unknown error';

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
      await api.pauseExecution();
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
      await api.resumeExecution();
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
      await api.cancelExecution();
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
// WebSocket Event Handlers
// ============================================================================

function handleWebSocketEvent(
  eventType: ServerEventType,
  data: Record<string, unknown>,
  get: () => ExecutionState,
  set: (partial: Partial<ExecutionState>) => void
) {
  switch (eventType) {
    // Connection events
    case 'connected': {
      const currentStatus = data.current_status as Record<string, unknown> | undefined;
      if (currentStatus) {
        syncStateFromServer(currentStatus, get, set);
      }
      break;
    }

    // Execution lifecycle events
    case 'execution_started': {
      set({
        status: 'running',
        taskId: data.task_id as string,
        startedAt: Date.now(),
      });
      get().addLog(`Execution started: ${data.description || 'Task'}`);
      break;
    }

    case 'execution_completed': {
      const completed = (data.stories_completed as number) || 0;
      const total = (data.stories_total as number) || get().stories.length;

      set({
        status: 'completed',
        currentStoryId: null,
        progress: 100,
        result: {
          success: true,
          message: 'All stories completed successfully',
          completedStories: completed,
          totalStories: total,
          duration: (data.duration_seconds as number) * 1000 || Date.now() - (get().startedAt || Date.now()),
        },
      });
      get().addLog('Execution completed successfully');
      get().saveToHistory();
      break;
    }

    case 'execution_failed': {
      const error = data.error as string || 'Unknown error';
      const completed = (data.stories_completed as number) || 0;
      const total = (data.stories_total as number) || get().stories.length;

      set({
        status: 'failed',
        currentStoryId: null,
        result: {
          success: false,
          message: 'Execution failed',
          completedStories: completed,
          totalStories: total,
          duration: Date.now() - (get().startedAt || Date.now()),
          error,
        },
      });
      get().addLog(`Execution failed: ${error}`);
      get().saveToHistory();
      break;
    }

    case 'execution_cancelled': {
      set({
        status: 'idle',
        currentStoryId: null,
        result: {
          success: false,
          message: 'Execution cancelled',
          completedStories: get().stories.filter((s) => s.status === 'completed').length,
          totalStories: get().stories.length,
          duration: Date.now() - (get().startedAt || Date.now()),
        },
      });
      get().addLog('Execution cancelled');
      get().saveToHistory();
      break;
    }

    case 'execution_paused': {
      set({ status: 'paused' });
      get().addLog('Execution paused');
      break;
    }

    case 'execution_resumed': {
      set({ status: 'running' });
      get().addLog('Execution resumed');
      break;
    }

    case 'execution_update': {
      syncStateFromServer(data, get, set);
      break;
    }

    // Strategy events
    case 'strategy_decided': {
      const strategy = data.strategy as Strategy;
      set({ strategy });
      get().addLog(`Strategy selected: ${strategy} - ${data.reasoning || ''}`);
      break;
    }

    // Batch events
    case 'batch_started': {
      const batch: Batch = {
        batchNum: data.batch_num as number,
        totalBatches: data.total_batches as number,
        storyIds: data.story_ids as string[],
        status: 'in_progress',
        startedAt: new Date().toISOString(),
      };

      const currentBatches = get().batches;
      set({
        batches: [...currentBatches, batch],
        currentBatch: data.batch_num as number,
      });
      get().addLog(`Batch ${data.batch_num} of ${data.total_batches} started`);
      break;
    }

    case 'batch_completed': {
      const currentBatches = get().batches;
      set({
        batches: currentBatches.map((b: Batch) =>
          b.batchNum === data.batch_num
            ? { ...b, status: 'completed' as const, completedAt: new Date().toISOString() }
            : b
        ),
      });
      get().addLog(`Batch ${data.batch_num} completed`);
      break;
    }

    case 'batch_failed': {
      const currentBatches = get().batches;
      set({
        batches: currentBatches.map((b: Batch) =>
          b.batchNum === data.batch_num
            ? { ...b, status: 'failed' as const, completedAt: new Date().toISOString() }
            : b
        ),
      });
      get().addLog(`Batch ${data.batch_num} failed`);
      break;
    }

    // Story events
    case 'story_started': {
      const storyId = data.story_id as string;
      const title = data.title as string;

      // Check if story already exists
      const existingStory = get().stories.find((s) => s.id === storyId);
      if (!existingStory) {
        const currentStories = get().stories;
        set({
          stories: [
            ...currentStories,
            {
              id: storyId,
              title: title || storyId,
              status: 'in_progress' as const,
              progress: 0,
              startedAt: new Date().toISOString(),
            },
          ],
        });
      } else {
        get().updateStory(storyId, {
          status: 'in_progress',
          progress: 0,
          startedAt: new Date().toISOString(),
        });
      }

      set({ currentStoryId: storyId });
      get().addLog(`Starting story: ${title || storyId}`);
      break;
    }

    case 'story_progress': {
      const storyId = data.story_id as string;
      const progress = data.progress as number;
      get().updateStory(storyId, { progress });
      break;
    }

    case 'story_completed': {
      const storyId = data.story_id as string;
      get().updateStory(storyId, {
        status: 'completed',
        progress: 100,
        completedAt: new Date().toISOString(),
      });
      get().addLog(`Completed: ${data.title || storyId}`);
      break;
    }

    case 'story_failed': {
      const storyId = data.story_id as string;
      const error = data.error as string;
      get().updateStory(storyId, {
        status: 'failed',
        error,
        completedAt: new Date().toISOString(),
      });
      get().addLog(`Failed: ${data.title || storyId} - ${error}`);
      break;
    }

    case 'story_update': {
      const storyId = data.story_id as string;
      const updates: Partial<Story> = {};
      if (data.status) updates.status = data.status as Story['status'];
      if (data.progress !== undefined) updates.progress = data.progress as number;
      if (data.error) updates.error = data.error as string;
      get().updateStory(storyId, updates);
      break;
    }

    // Retry events
    case 'retry_started': {
      const storyId = data.story_id as string;
      const attempt = data.attempt as number;
      const maxAttempts = data.max_attempts as number;
      get().updateStory(storyId, {
        status: 'in_progress',
        retryCount: attempt,
        error: undefined,
      });
      get().addLog(`Retrying story ${storyId} (attempt ${attempt} of ${maxAttempts})`);
      break;
    }

    // PRD events
    case 'prd_generated': {
      get().addLog('PRD generated');
      break;
    }

    case 'prd_approved': {
      get().addLog('PRD approved, starting execution');
      break;
    }

    // Log events
    case 'log_entry': {
      const message = data.message as string;
      const level = data.level as string;
      if (level !== 'debug') {
        get().addLog(`[${level?.toUpperCase() || 'INFO'}] ${message}`);
      }
      break;
    }

    // Quality gate events
    case 'quality_gate_started': {
      get().addLog(`Quality gate started: ${data.gate_type}`);
      break;
    }

    case 'quality_gate_passed': {
      get().addLog(`Quality gate passed: ${data.gate_type}`);
      break;
    }

    case 'quality_gate_failed': {
      get().addLog(`Quality gate failed: ${data.gate_type}`);
      break;
    }

    default:
      // Ignore unknown events
      break;
  }
}

function syncStateFromServer(
  data: Record<string, unknown>,
  _get: () => ExecutionState,
  set: (partial: Partial<ExecutionState>) => void
) {
  const updates: Partial<ExecutionState> = {};

  // Map server status to client status
  if (data.status) {
    updates.status = data.status as ExecutionStatus;
  }

  if (data.task_id) {
    updates.taskId = data.task_id as string;
  }

  if (data.task_description) {
    updates.taskDescription = data.task_description as string;
  }

  if (data.strategy) {
    updates.strategy = data.strategy as Strategy;
  }

  if (data.current_story_id) {
    updates.currentStoryId = data.current_story_id as string;
  }

  if (data.current_batch !== undefined) {
    updates.currentBatch = data.current_batch as number;
  }

  if (data.overall_progress !== undefined) {
    updates.progress = data.overall_progress as number;
  }

  // Sync stories
  if (data.stories && Array.isArray(data.stories)) {
    updates.stories = (data.stories as Record<string, unknown>[]).map((s) => ({
      id: s.id as string,
      title: s.title as string,
      description: s.description as string | undefined,
      status: s.status as Story['status'],
      progress: s.progress as number,
      error: s.error as string | undefined,
      startedAt: s.started_at as string | undefined,
      completedAt: s.completed_at as string | undefined,
      retryCount: s.retry_count as number | undefined,
    }));
  }

  // Sync batches
  if (data.batches && Array.isArray(data.batches)) {
    updates.batches = (data.batches as Record<string, unknown>[]).map((b) => ({
      batchNum: b.batch_num as number,
      totalBatches: b.total_batches as number,
      storyIds: b.story_ids as string[],
      status: b.status as Batch['status'],
      startedAt: b.started_at as string | undefined,
      completedAt: b.completed_at as string | undefined,
    }));
  }

  set(updates);
}

export default useExecutionStore;
