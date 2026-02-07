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

export type Strategy = 'direct' | 'hybrid_auto' | 'hybrid_worktree' | 'mega_plan' | null;

/** Dimension scores from strategy analysis (0.0 - 1.0 each) */
export interface DimensionScores {
  scope: number;
  complexity: number;
  risk: number;
  parallelization: number;
}

/** Result of automatic strategy analysis from the Rust backend */
export interface StrategyAnalysis {
  strategy: string;
  confidence: number;
  reasoning: string;
  estimated_stories: number;
  estimated_features: number;
  estimated_duration_hours: number;
  complexity_indicators: string[];
  recommendations: string[];
  dimension_scores: DimensionScores;
}

/** A strategy option returned by get_strategy_options */
export interface StrategyOptionInfo {
  value: string;
  label: string;
  description: string;
  min_stories: number;
  max_stories: number;
}

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

// ============================================================================
// Streaming Output Types
// ============================================================================

export type StreamLineType = 'text' | 'info' | 'error' | 'success' | 'warning' | 'tool' | 'thinking' | 'code';

export interface StreamLine {
  id: number;
  content: string;
  type: StreamLineType;
  timestamp: number;
}

// ============================================================================
// Quality Gate Result Types
// ============================================================================

export type QualityGateStatus = 'pending' | 'running' | 'passed' | 'failed';

export interface QualityGateResult {
  gateId: string;
  gateName: string;
  storyId: string;
  status: QualityGateStatus;
  output?: string;
  duration?: number;
  startedAt?: number;
  completedAt?: number;
}

// ============================================================================
// Error State Types
// ============================================================================

export type ErrorSeverity = 'warning' | 'error' | 'critical';

export interface ExecutionError {
  id: string;
  storyId?: string;
  severity: ErrorSeverity;
  title: string;
  description: string;
  suggestedFix?: string;
  stackTrace?: string;
  timestamp: number;
  dismissed: boolean;
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

  /** Strategy analysis result from auto-analyzer */
  strategyAnalysis: StrategyAnalysis | null;

  /** Whether strategy analysis is in progress */
  isAnalyzingStrategy: boolean;

  /** Available strategy options metadata */
  strategyOptions: StrategyOptionInfo[];

  /** Streaming output buffer for real-time display */
  streamingOutput: StreamLine[];

  /** Counter for unique stream line IDs */
  streamLineCounter: number;

  /** Quality gate results per story */
  qualityGateResults: QualityGateResult[];

  /** Actionable error states */
  executionErrors: ExecutionError[];

  /** Estimated time remaining in milliseconds */
  estimatedTimeRemaining: number | null;

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

  /** Analyze task strategy via Rust backend */
  analyzeStrategy: (description: string) => Promise<StrategyAnalysis | null>;

  /** Load available strategy options */
  loadStrategyOptions: () => Promise<void>;

  /** Clear strategy analysis */
  clearStrategyAnalysis: () => void;

  /** Append a streaming output line */
  appendStreamLine: (content: string, type: StreamLineType) => void;

  /** Clear streaming output buffer */
  clearStreamingOutput: () => void;

  /** Update quality gate result for a story */
  updateQualityGate: (result: QualityGateResult) => void;

  /** Add an execution error */
  addExecutionError: (error: Omit<ExecutionError, 'id' | 'timestamp' | 'dismissed'>) => void;

  /** Dismiss an execution error */
  dismissError: (errorId: string) => void;

  /** Clear all execution errors */
  clearExecutionErrors: () => void;

  /** Retry a failed story */
  retryStory: (storyId: string) => Promise<void>;
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
  strategyAnalysis: null as StrategyAnalysis | null,
  isAnalyzingStrategy: false,
  strategyOptions: [] as StrategyOptionInfo[],
  streamingOutput: [] as StreamLine[],
  streamLineCounter: 0,
  qualityGateResults: [] as QualityGateResult[],
  executionErrors: [] as ExecutionError[],
  estimatedTimeRemaining: null as number | null,
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
      streamingOutput: [],
      streamLineCounter: 0,
      qualityGateResults: [],
      executionErrors: [],
      estimatedTimeRemaining: null,
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

  analyzeStrategy: async (description: string) => {
    if (!description.trim()) return null;

    set({ isAnalyzingStrategy: true });
    get().addLog('Analyzing task strategy...');

    try {
      const result = await invoke<CommandResponse<StrategyAnalysis>>('analyze_task_strategy', {
        description,
        context: null,
      });

      if (result.success && result.data) {
        const analysis = result.data;
        set({
          strategyAnalysis: analysis,
          isAnalyzingStrategy: false,
          strategy: analysis.strategy as Strategy,
        });
        get().addLog(
          `Strategy recommendation: ${analysis.strategy} (confidence: ${(analysis.confidence * 100).toFixed(0)}%)`
        );
        return analysis;
      } else {
        throw new Error(result.error || 'Strategy analysis failed');
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      set({ isAnalyzingStrategy: false });
      get().addLog(`Strategy analysis error: ${errorMessage}`);
      return null;
    }
  },

  loadStrategyOptions: async () => {
    try {
      const result = await invoke<CommandResponse<StrategyOptionInfo[]>>('get_strategy_options');
      if (result.success && result.data) {
        set({ strategyOptions: result.data });
      }
    } catch {
      // Non-critical, options can load later
    }
  },

  clearStrategyAnalysis: () => {
    set({
      strategyAnalysis: null,
      isAnalyzingStrategy: false,
    });
  },

  appendStreamLine: (content, type) => {
    const counter = get().streamLineCounter + 1;
    const line: StreamLine = {
      id: counter,
      content,
      type,
      timestamp: Date.now(),
    };
    set((state) => {
      // Keep buffer capped at 500 lines by trimming old entries when appending
      const trimmed = state.streamingOutput.length >= 500
        ? state.streamingOutput.slice(-499)
        : state.streamingOutput;
      return {
        streamingOutput: [...trimmed, line],
        streamLineCounter: counter,
      };
    });
  },

  clearStreamingOutput: () => {
    set({ streamingOutput: [], streamLineCounter: 0 });
  },

  updateQualityGate: (result) => {
    set((state) => {
      const existing = state.qualityGateResults.findIndex(
        (r) => r.gateId === result.gateId && r.storyId === result.storyId
      );
      if (existing >= 0) {
        const updated = [...state.qualityGateResults];
        updated[existing] = result;
        return { qualityGateResults: updated };
      }
      return { qualityGateResults: [...state.qualityGateResults, result] };
    });
  },

  addExecutionError: (error) => {
    const newError: ExecutionError = {
      ...error,
      id: `err-${Date.now()}-${Math.random().toString(36).substr(2, 6)}`,
      timestamp: Date.now(),
      dismissed: false,
    };
    set((state) => ({
      executionErrors: [...state.executionErrors, newError],
    }));
    get().addLog(`[${error.severity.toUpperCase()}] ${error.title}: ${error.description}`);
  },

  dismissError: (errorId) => {
    set((state) => ({
      executionErrors: state.executionErrors.map((e) =>
        e.id === errorId ? { ...e, dismissed: true } : e
      ),
    }));
  },

  clearExecutionErrors: () => {
    set({ executionErrors: [] });
  },

  retryStory: async (storyId) => {
    const story = get().stories.find((s) => s.id === storyId);
    if (!story) return;

    // Reset story state
    get().updateStory(storyId, {
      status: 'in_progress',
      progress: 0,
      error: undefined,
      retryCount: (story.retryCount || 0) + 1,
    });

    // Clear related errors
    set((state) => ({
      executionErrors: state.executionErrors.filter((e) => e.storyId !== storyId),
    }));

    get().addLog(`Retrying story: ${story.title} (attempt ${(story.retryCount || 0) + 1})`);

    try {
      await invoke<CommandResponse<boolean>>('retry_story', {
        session_id: get().taskId,
        story_id: storyId,
      });
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Retry failed';
      get().updateStory(storyId, {
        status: 'failed',
        error: errorMessage,
      });
      get().addExecutionError({
        storyId,
        severity: 'error',
        title: `Retry failed for ${story.title}`,
        description: errorMessage,
        suggestedFix: 'Check the error output and try again, or skip this story.',
      });
    }
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

      // Feed TextDelta into streaming output buffer
      if (streamEvent.TextDelta) {
        get().appendStreamLine(streamEvent.TextDelta.content, 'text');
      }

      if (streamEvent.Error) {
        get().appendStreamLine(streamEvent.Error.message, 'error');
        get().addExecutionError({
          severity: 'critical',
          title: 'Execution Failed',
          description: streamEvent.Error.message,
          suggestedFix: 'Check the error details and retry the execution.',
        });
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

        get().appendStreamLine('Execution completed successfully.', 'success');
        set({
          status: 'completed',
          progress: 100,
          estimatedTimeRemaining: 0,
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
      execution: { id: string; tool_name: string; success?: boolean; arguments?: string; result?: string };
      update_type: string;
      session_id: string;
    }>('claude_code:tool', (event) => {
      const { execution, update_type } = event.payload;

      if (update_type === 'started') {
        get().addLog(`Tool started: ${execution.tool_name}`);
        get().appendStreamLine(`[tool] ${execution.tool_name} started`, 'tool');
      } else if (update_type === 'completed') {
        const status = execution.success ? 'success' : 'failed';
        get().addLog(`Tool completed: ${execution.tool_name} (${status})`);
        get().appendStreamLine(`[tool] ${execution.tool_name} ${status}`, execution.success ? 'success' : 'error');
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
          get().appendStreamLine(`Session error: ${session.error_message || 'Unknown error'}`, 'error');
          get().addExecutionError({
            severity: 'error',
            title: 'Session Error',
            description: session.error_message || 'Unknown error',
            suggestedFix: 'The session encountered an error. Try restarting the execution.',
          });
          set({
            status: 'failed',
            apiError: session.error_message || 'Session error',
          });
          get().addLog(`Session error: ${session.error_message || 'Unknown error'}`);
        } else if (session.state === 'cancelled') {
          get().appendStreamLine('Session cancelled.', 'warning');
          set({ status: 'idle' });
          get().addLog('Session cancelled');
        }
      }
    });
    unlisteners.push(unlistenSession);

    // Listen for unified stream events (from the unified streaming service)
    const unlistenUnified = await listen<{
      type: string;
      session_id?: string;
      content?: string;
      tool_id?: string;
      tool_name?: string;
      arguments?: string;
      result?: string;
      error?: string;
      message?: string;
      story_id?: string;
      story_title?: string;
      story_index?: number;
      total_stories?: number;
      success?: boolean;
      passed?: boolean;
      summary?: Record<string, unknown>;
      thinking_id?: string;
    }>('execution:unified_stream', (event) => {
      const payload = event.payload;

      switch (payload.type) {
        case 'text_delta':
          if (payload.content) {
            get().appendStreamLine(payload.content, 'text');
          }
          break;

        case 'thinking_start':
          get().appendStreamLine('[thinking...]', 'thinking');
          break;

        case 'thinking_delta':
          if (payload.content) {
            get().appendStreamLine(payload.content, 'thinking');
          }
          break;

        case 'tool_start':
          if (payload.tool_name) {
            get().appendStreamLine(`[tool] ${payload.tool_name} started`, 'tool');
          }
          break;

        case 'tool_result':
          if (payload.error) {
            get().appendStreamLine(`[tool error] ${payload.error}`, 'error');
          } else if (payload.result) {
            get().appendStreamLine(`[tool result] ${payload.result.substring(0, 200)}`, 'info');
          }
          break;

        case 'error':
          if (payload.message) {
            get().appendStreamLine(`[error] ${payload.message}`, 'error');
            get().addExecutionError({
              severity: 'error',
              title: 'Stream Error',
              description: payload.message,
              suggestedFix: 'Check the error details and retry if needed.',
            });
          }
          break;

        case 'story_start':
          if (payload.story_id && payload.story_title) {
            get().appendStreamLine(
              `Starting story ${(payload.story_index || 0) + 1}/${payload.total_stories || '?'}: ${payload.story_title}`,
              'info'
            );
            get().updateStory(payload.story_id, {
              status: 'in_progress',
              startedAt: new Date().toISOString(),
            });
            set({ currentStoryId: payload.story_id });

            // Estimate time remaining based on average story completion time
            const state = get();
            const completedStories = state.stories.filter((s) => s.status === 'completed');
            if (completedStories.length > 0 && state.startedAt) {
              const elapsed = Date.now() - state.startedAt;
              const avgTimePerStory = elapsed / completedStories.length;
              const remainingStories = (payload.total_stories || state.stories.length) - completedStories.length;
              set({ estimatedTimeRemaining: Math.round(avgTimePerStory * remainingStories) });
            }
          }
          break;

        case 'story_complete':
          if (payload.story_id) {
            const success = payload.success !== false;
            get().updateStory(payload.story_id, {
              status: success ? 'completed' : 'failed',
              progress: success ? 100 : 0,
              completedAt: new Date().toISOString(),
              error: payload.error,
            });
            get().appendStreamLine(
              `Story ${success ? 'completed' : 'failed'}: ${payload.story_id}${payload.error ? ' - ' + payload.error : ''}`,
              success ? 'success' : 'error'
            );

            if (!success && payload.error) {
              const story = get().stories.find((s) => s.id === payload.story_id);
              get().addExecutionError({
                storyId: payload.story_id,
                severity: 'error',
                title: `Story failed: ${story?.title || payload.story_id}`,
                description: payload.error,
                suggestedFix: 'Review the error output and retry this story.',
              });
            }
          }
          break;

        case 'quality_gates_result':
          if (payload.story_id && payload.summary) {
            const summary = payload.summary as Record<string, { passed?: boolean; output?: string; duration?: number }>;
            const passed = payload.passed !== false;

            // Parse individual gate results from summary
            for (const [gateName, gateResult] of Object.entries(summary)) {
              get().updateQualityGate({
                gateId: gateName.toLowerCase().replace(/\s+/g, '_'),
                gateName,
                storyId: payload.story_id,
                status: gateResult.passed !== false ? 'passed' : 'failed',
                output: gateResult.output,
                duration: gateResult.duration,
                completedAt: Date.now(),
              });
            }

            get().appendStreamLine(
              `Quality gates ${passed ? 'passed' : 'failed'} for story: ${payload.story_id}`,
              passed ? 'success' : 'warning'
            );
          }
          break;

        case 'session_complete':
          if (payload.success !== undefined) {
            const completedStories = payload.success
              ? (payload.total_stories || get().stories.length)
              : get().stories.filter((s) => s.status === 'completed').length;
            const totalStories = payload.total_stories || get().stories.length;

            set({
              status: payload.success ? 'completed' : 'failed',
              progress: payload.success ? 100 : get().progress,
              estimatedTimeRemaining: 0,
              result: {
                success: payload.success,
                message: payload.success ? 'Execution completed' : 'Execution failed',
                completedStories,
                totalStories,
                duration: Date.now() - (get().startedAt || Date.now()),
              },
            });
            get().appendStreamLine(
              payload.success ? 'All stories completed successfully.' : 'Execution finished with failures.',
              payload.success ? 'success' : 'error'
            );
            get().saveToHistory();
          }
          break;
      }
    });
    unlisteners.push(unlistenUnified);
  } catch (error) {
    console.error('Failed to set up Tauri event listeners:', error);
  }
}

export default useExecutionStore;
