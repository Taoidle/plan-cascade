/**
 * Task Mode Store
 *
 * Zustand store for Task Mode lifecycle management.
 * Manages mode switching, PRD generation/review, execution monitoring,
 * and quality gate results via Tauri IPC commands and events.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { CrossModeConversationTurn } from '../types/crossModeContext';

// ============================================================================
// Types
// ============================================================================

/** Execution mode recommendation from strategy analysis */
export type ExecutionMode = 'chat' | 'task';

/** Risk level from strategy analysis */
export type RiskLevel = 'low' | 'medium' | 'high';

/** Parallelization benefit */
export type Benefit = 'none' | 'moderate' | 'significant';

/** Task mode session status - mirrors Rust TaskModeStatus */
export type TaskModeSessionStatus =
  | 'initialized'
  | 'generating_prd'
  | 'reviewing_prd'
  | 'executing'
  | 'completed'
  | 'failed'
  | 'cancelled';

/** Quality gate phase */
export type GatePhase = 'pre_validation' | 'validation' | 'post_validation';

/** Quality gate status */
export type GateStatus = 'pending' | 'running' | 'passed' | 'failed' | 'skipped';

/** Strategy analysis result from Rust backend */
export interface StrategyAnalysis {
  functionalAreas: string[];
  estimatedStories: number;
  hasDependencies: boolean;
  riskLevel: RiskLevel;
  parallelizationBenefit: Benefit;
  recommendedMode: ExecutionMode;
  confidence: number;
  reasoning: string;
  strategyDecision: {
    strategy: string;
    confidence: number;
    reasoning: string;
  };
}

/** A story in the task PRD */
export interface TaskStory {
  id: string;
  title: string;
  description: string;
  priority: string;
  dependencies: string[];
  acceptanceCriteria: string[];
}

/** Execution batch */
export interface ExecutionBatch {
  /** Batch index (0-based). Rust field name is `index`, serialized as `"index"` via camelCase. */
  index: number;
  storyIds: string[];
}

/** Task PRD */
export interface TaskPrd {
  title: string;
  description: string;
  stories: TaskStory[];
  batches: ExecutionBatch[];
}

/** Task mode session from Rust backend */
export interface TaskModeSession {
  sessionId: string;
  description: string;
  status: TaskModeSessionStatus;
  strategyAnalysis: StrategyAnalysis | null;
  prd: TaskPrd | null;
  progress: BatchExecutionProgress | null;
  createdAt: string;
}

/** Batch execution progress from Rust backend */
export interface BatchExecutionProgress {
  currentBatch: number;
  totalBatches: number;
  storiesCompleted: number;
  storiesFailed: number;
  totalStories: number;
  storyStatuses: Record<string, string>;
  currentPhase: string;
}

/** Task execution status from Rust backend */
export interface TaskExecutionStatus {
  sessionId: string;
  status: TaskModeSessionStatus;
  currentBatch: number;
  totalBatches: number;
  storyStatuses: Record<string, string>;
  storiesCompleted: number;
  storiesFailed: number;
}

/** Execution report from Rust backend */
export interface ExecutionReport {
  sessionId: string;
  totalStories: number;
  storiesCompleted: number;
  storiesFailed: number;
  totalDurationMs: number;
  agentAssignments: Record<string, string>;
  success: boolean;
}

/** Individual gate result in quality gate results */
export interface GateResult {
  gateId: string;
  gateName: string;
  phase: GatePhase;
  status: GateStatus;
  message?: string;
  duration?: number;
}

/** Code review dimension score */
export interface DimensionScore {
  dimension: string;
  score: number;
  maxScore: number;
  feedback: string;
}

/** Quality gate results per story */
export interface StoryQualityGateResults {
  storyId: string;
  overallStatus: GateStatus;
  gates: GateResult[];
  codeReviewScores?: DimensionScore[];
  totalScore?: number;
}

/** Tauri command response wrapper */
interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

/** Tauri event payload for task mode progress — matches Rust TaskModeProgressEvent */
interface TaskModeProgressPayload {
  sessionId: string;
  eventType: string;
  currentBatch: number;
  totalBatches: number;
  storyId: string | null;
  storyStatus: string | null;
  agentName: string | null;
  gateResults: GateResult[] | null;
  error: string | null;
  progressPct: number;
}

// ============================================================================
// State Interface
// ============================================================================

export interface TaskModeState {
  /** Whether we are in task mode */
  isTaskMode: boolean;

  /** Current session ID */
  sessionId: string | null;

  /** Strategy analysis (shows recommendation) */
  strategyAnalysis: StrategyAnalysis | null;

  /** Whether strategy analysis has been dismissed */
  suggestionDismissed: boolean;

  /** Current session status */
  sessionStatus: TaskModeSessionStatus | 'idle';

  /** Generated/approved PRD */
  prd: TaskPrd | null;

  /** Current batch index */
  currentBatch: number;

  /** Total batches */
  totalBatches: number;

  /** Per-story execution status */
  storyStatuses: Record<string, string>;

  /** Per-story quality gate results */
  qualityGateResults: Record<string, StoryQualityGateResults>;

  /** Execution report (after completion) */
  report: ExecutionReport | null;

  /** Loading state */
  isLoading: boolean;

  /** Error message */
  error: string | null;

  /** Unlisten function for Tauri events */
  _unlistenFn: UnlistenFn | null;

  // Actions
  /** Analyze task description for mode recommendation */
  analyzeForMode: (description: string) => Promise<void>;

  /** Dismiss the mode suggestion */
  dismissSuggestion: () => void;

  /** Enter task mode with a description */
  enterTaskMode: (description: string) => Promise<void>;

  /** Generate PRD from current session, optionally with conversation history for context */
  generatePrd: (
    conversationHistory?: CrossModeConversationTurn[],
    maxContextTokens?: number,
    overrideProvider?: string,
    overrideModel?: string,
    overrideBaseUrl?: string,
  ) => Promise<void>;

  /** Approve PRD (optionally with edits) and start execution */
  approvePrd: (prd: TaskPrd) => Promise<void>;

  /** Get current execution status */
  refreshStatus: () => Promise<void>;

  /** Cancel current execution */
  cancelExecution: () => Promise<void>;

  /** Get execution report */
  fetchReport: () => Promise<void>;

  /** Exit task mode */
  exitTaskMode: () => Promise<void>;

  /** Subscribe to Tauri task-mode-progress events */
  subscribeToEvents: () => Promise<void>;

  /** Unsubscribe from events */
  unsubscribeFromEvents: () => void;

  /** Reset store to initial state */
  reset: () => void;
}

// ============================================================================
// Default State
// ============================================================================

const DEFAULT_STATE = {
  isTaskMode: false,
  sessionId: null,
  strategyAnalysis: null,
  suggestionDismissed: false,
  sessionStatus: 'idle' as const,
  prd: null,
  currentBatch: 0,
  totalBatches: 0,
  storyStatuses: {},
  qualityGateResults: {},
  report: null,
  isLoading: false,
  error: null,
  _unlistenFn: null,
};

// ============================================================================
// Store
// ============================================================================

export const useTaskModeStore = create<TaskModeState>()((set, get) => ({
  ...DEFAULT_STATE,

  analyzeForMode: async (description: string) => {
    set({ isLoading: true, error: null, suggestionDismissed: false });
    try {
      const result = await invoke<CommandResponse<StrategyAnalysis>>('analyze_task_for_mode', { description });
      if (result.success && result.data) {
        set({ strategyAnalysis: result.data, isLoading: false });
      } else {
        set({ isLoading: false, error: result.error ?? 'Analysis failed' });
      }
    } catch (e) {
      set({ isLoading: false, error: String(e) });
    }
  },

  dismissSuggestion: () => {
    set({ suggestionDismissed: true });
  },

  enterTaskMode: async (description: string) => {
    set({ isLoading: true, error: null });
    try {
      const result = await invoke<CommandResponse<TaskModeSession>>('enter_task_mode', { description });
      if (result.success && result.data) {
        const session = result.data;
        set({
          isTaskMode: true,
          sessionId: session.sessionId,
          sessionStatus: session.status as TaskModeSessionStatus,
          strategyAnalysis: session.strategyAnalysis,
          prd: session.prd,
          isLoading: false,
        });
        // Subscribe to events
        await get().subscribeToEvents();
      } else {
        set({ isLoading: false, error: result.error ?? 'Failed to enter task mode' });
      }
    } catch (e) {
      set({ isLoading: false, error: String(e) });
    }
  },

  generatePrd: async (
    conversationHistory?: CrossModeConversationTurn[],
    maxContextTokens?: number,
    overrideProvider?: string,
    overrideModel?: string,
    overrideBaseUrl?: string,
  ) => {
    const { sessionId } = get();
    if (!sessionId) {
      set({ error: 'No active session' });
      return;
    }
    set({ isLoading: true, error: null, sessionStatus: 'generating_prd' });
    try {
      // Read provider/model + endpoint settings from settings store
      const settingsStore = (await import('./settings')).useSettingsStore.getState();
      const { provider, model, glmEndpoint, qwenEndpoint, minimaxEndpoint } = settingsStore;
      const finalProvider = overrideProvider || provider;
      const finalModel = overrideModel || model;
      const { resolveProviderBaseUrl } = await import('../lib/providers');
      const finalBaseUrl =
        overrideBaseUrl !== undefined
          ? overrideBaseUrl
          : finalProvider
            ? resolveProviderBaseUrl(finalProvider, { glmEndpoint, qwenEndpoint, minimaxEndpoint })
            : undefined;
      const { buildConfig: buildContextConfig } = (await import('./contextSources')).useContextSourcesStore.getState();
      const contextSources = buildContextConfig() ?? null;
      const result = await invoke<CommandResponse<TaskPrd>>('generate_task_prd', {
        sessionId,
        provider: finalProvider || null,
        model: finalModel || null,
        baseUrl: finalBaseUrl || null,
        conversationHistory: conversationHistory || [],
        maxContextTokens: maxContextTokens ?? null,
        contextSources,
      });
      if (result.success && result.data) {
        set({
          prd: result.data,
          sessionStatus: 'reviewing_prd',
          isLoading: false,
        });
      } else {
        set({ isLoading: false, error: result.error ?? 'PRD generation failed' });
      }
    } catch (e) {
      set({ isLoading: false, error: String(e) });
    }
  },

  approvePrd: async (prd: TaskPrd) => {
    const { sessionId } = get();
    if (!sessionId) {
      set({ error: 'No active session' });
      return;
    }
    set({ isLoading: true, error: null });
    try {
      const settingsStore = (await import('./settings')).useSettingsStore.getState();
      const { provider, model, phaseConfigs, glmEndpoint, qwenEndpoint, minimaxEndpoint } = settingsStore;
      const { resolveProviderBaseUrl } = await import('../lib/providers');
      const baseUrl = provider
        ? resolveProviderBaseUrl(provider, { glmEndpoint, qwenEndpoint, minimaxEndpoint })
        : undefined;
      const { buildConfig: buildCtxConfig } = (await import('./contextSources')).useContextSourcesStore.getState();
      const contextSources = buildCtxConfig() ?? null;
      const result = await invoke<CommandResponse<boolean>>('approve_task_prd', {
        sessionId,
        prd,
        provider: provider || null,
        model: model || null,
        baseUrl: baseUrl || null,
        phaseConfigs,
        contextSources,
      });
      if (result.success) {
        set({
          prd,
          sessionStatus: 'executing',
          isLoading: false,
        });
      } else {
        set({ isLoading: false, error: result.error ?? 'PRD approval failed' });
      }
    } catch (e) {
      set({ isLoading: false, error: String(e) });
    }
  },

  refreshStatus: async () => {
    const { sessionId } = get();
    if (!sessionId) return;
    try {
      const result = await invoke<CommandResponse<TaskExecutionStatus>>('get_task_execution_status', { sessionId });
      if (result.success && result.data) {
        const status = result.data;
        set({
          sessionStatus: status.status as TaskModeSessionStatus,
          currentBatch: status.currentBatch,
          totalBatches: status.totalBatches,
          storyStatuses: status.storyStatuses,
        });
      }
    } catch {
      // Silently ignore refresh errors
    }
  },

  cancelExecution: async () => {
    const { sessionId } = get();
    if (!sessionId) {
      set({ error: 'No active session' });
      return;
    }
    set({ isLoading: true, error: null });
    try {
      const result = await invoke<CommandResponse<boolean>>('cancel_task_execution', { sessionId });
      if (result.success) {
        set({ sessionStatus: 'cancelled', isLoading: false });
      } else {
        set({ isLoading: false, error: result.error ?? 'Cancel failed' });
      }
    } catch (e) {
      set({ isLoading: false, error: String(e) });
    }
  },

  fetchReport: async () => {
    const { sessionId } = get();
    if (!sessionId) {
      set({ error: 'No active session' });
      return;
    }
    set({ isLoading: true, error: null });
    try {
      const result = await invoke<CommandResponse<ExecutionReport>>('get_task_execution_report', { sessionId });
      if (result.success && result.data) {
        set({ report: result.data, isLoading: false });
      } else {
        set({ isLoading: false, error: result.error ?? 'Failed to fetch report' });
      }
    } catch (e) {
      set({ isLoading: false, error: String(e) });
    }
  },

  exitTaskMode: async () => {
    const { sessionId } = get();
    if (!sessionId) {
      set({ error: 'No active session' });
      return;
    }
    set({ isLoading: true, error: null });
    try {
      const result = await invoke<CommandResponse<boolean>>('exit_task_mode', { sessionId });
      if (result.success) {
        get().unsubscribeFromEvents();
        set({ ...DEFAULT_STATE });
      } else {
        set({ isLoading: false, error: result.error ?? 'Failed to exit task mode' });
      }
    } catch (e) {
      set({ isLoading: false, error: String(e) });
    }
  },

  subscribeToEvents: async () => {
    // Unsubscribe from any existing listener
    get().unsubscribeFromEvents();

    try {
      const unlisten = await listen<TaskModeProgressPayload>('task-mode-progress', (event) => {
        const payload = event.payload;
        const { sessionId, storyStatuses: prevStatuses } = get();
        // Only process events for our session
        if (payload.sessionId !== sessionId) return;

        const updates: Partial<TaskModeState> = {
          currentBatch: payload.currentBatch,
          totalBatches: payload.totalBatches,
        };

        // Accumulate story statuses from individual events
        if (payload.storyId && payload.storyStatus) {
          updates.storyStatuses = { ...prevStatuses, [payload.storyId]: payload.storyStatus };
        }

        // Determine session status from event type
        if (payload.eventType === 'execution_completed') {
          const allStatuses = updates.storyStatuses ?? prevStatuses;
          const failedCount = Object.values(allStatuses).filter((s) => s === 'failed').length;
          updates.sessionStatus = failedCount > 0 ? 'failed' : 'completed';
        } else if (payload.eventType === 'execution_cancelled') {
          updates.sessionStatus = 'cancelled';
        }

        set(updates);
      });
      set({ _unlistenFn: unlisten });
    } catch {
      // Event subscription failed - non-fatal
    }
  },

  unsubscribeFromEvents: () => {
    const { _unlistenFn } = get();
    if (_unlistenFn) {
      _unlistenFn();
      set({ _unlistenFn: null });
    }
  },

  reset: () => {
    get().unsubscribeFromEvents();
    set({ ...DEFAULT_STATE });
  },
}));

export default useTaskModeStore;
