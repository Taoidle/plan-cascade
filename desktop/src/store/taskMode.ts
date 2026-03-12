/**
 * Task Mode Store
 *
 * Command client for Task Mode lifecycle IPC.
 * Session lifecycle truth is owned by workflow kernel snapshots.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import i18n from '../i18n';
import { useContextSourcesStore } from './contextSources';
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

/** Task mode session status - MUST stay in sync with Rust TaskModeStatus */
export type TaskModeSessionStatus =
  | 'initialized'
  | 'exploring'
  | 'generating_prd'
  | 'reviewing_prd'
  | 'executing'
  | 'completed'
  | 'failed'
  | 'cancelled';

const KNOWN_TASK_MODE_SESSION_STATUSES: TaskModeSessionStatus[] = [
  'initialized',
  'exploring',
  'generating_prd',
  'reviewing_prd',
  'executing',
  'completed',
  'failed',
  'cancelled',
];

/**
 * Runtime guard for backend enum drift.
 * Unknown values degrade to `initialized` and surface a warning via store error.
 */
export function normalizeTaskModeSessionStatus(raw: unknown): {
  status: TaskModeSessionStatus;
  warning: string | null;
} {
  if (typeof raw === 'string' && KNOWN_TASK_MODE_SESSION_STATUSES.includes(raw as TaskModeSessionStatus)) {
    return { status: raw as TaskModeSessionStatus, warning: null };
  }
  return {
    status: 'initialized',
    warning: `Unknown task mode session status '${String(raw)}'; defaulted to 'initialized'.`,
  };
}

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

export type StrategyRecommendationSource = 'deterministic' | 'llm_enhanced' | 'fallback_deterministic';

export interface RecommendedWorkflowConfig {
  flowLevel: 'quick' | 'standard' | 'full';
  tddMode: 'off' | 'flexible' | 'strict';
  specInterviewEnabled: boolean;
  qualityGatesEnabled: boolean;
  maxParallel: number;
  skipVerification: boolean;
  skipReview: boolean;
  globalAgentOverride: string | null;
  implAgentOverride: string | null;
}

export interface TaskStrategyRecommendation {
  analysis: StrategyAnalysis;
  recommendedConfig: RecommendedWorkflowConfig;
  recommendationSource: StrategyRecommendationSource;
  reasoning: string;
  confidence: number;
  configRationale: string[];
}

export type TaskConfigConfirmationState = 'pending' | 'confirmed';

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

export interface PrdFeedbackApplySummary {
  addedStoryIds: string[];
  removedStoryIds: string[];
  updatedStoryIds: string[];
  batchChanges: string[];
  warnings: string[];
}

export interface TaskWorkflowConfigPayload {
  flowLevel: 'quick' | 'standard' | 'full';
  tddMode: 'off' | 'flexible' | 'strict';
  enableInterview: boolean;
  qualityGatesEnabled: boolean;
  selectedQualityGateIds: string[];
  qualityRetryMaxAttempts: number | null;
  customQualityGates: import('../types/workflowQuality').QualityCustomGate[];
  maxParallel: number;
  skipVerification: boolean;
  skipReview: boolean;
  globalAgentOverride: string | null;
  implAgentOverride: string | null;
}

export interface PrdFeedbackApplyResult {
  prd: TaskPrd;
  summary: PrdFeedbackApplySummary;
}

/** Task mode session from Rust backend */
export interface TaskModeSession {
  sessionId: string;
  kernelSessionId?: string | null;
  description: string;
  status: TaskModeSessionStatus;
  strategyAnalysis: StrategyAnalysis | null;
  strategyRecommendation: TaskStrategyRecommendation | null;
  configConfirmationState: TaskConfigConfirmationState;
  confirmedConfig: TaskWorkflowConfigPayload | null;
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
  blockingStatus?: 'passed' | 'failed';
  softFailedGateCount?: number;
  gateSource?: 'llm' | 'fallback_heuristic' | 'skipped' | 'mixed';
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

function normalizeSessionId(value: string | null | undefined): string | null {
  const normalized = value?.trim() ?? '';
  return normalized.length > 0 ? normalized : null;
}

let activeTaskSessionId: string | null = null;

function resolveSessionId(explicit?: string | null): string | null {
  return normalizeSessionId(explicit) ?? activeTaskSessionId;
}

async function resolveRequestLlm(overrideProvider?: string, overrideModel?: string, overrideBaseUrl?: string) {
  const settingsStore = (await import('./settings')).useSettingsStore.getState();
  const finalProvider = overrideProvider || settingsStore.provider;
  const finalModel = overrideModel || settingsStore.model;
  const { resolveProviderBaseUrl } = await import('../lib/providers');
  const finalBaseUrl =
    overrideBaseUrl !== undefined
      ? overrideBaseUrl
      : finalProvider
        ? resolveProviderBaseUrl(finalProvider, settingsStore)
        : undefined;
  return { finalProvider, finalModel, finalBaseUrl, settingsStore };
}

// ============================================================================
// State Interface
// ============================================================================

export interface TaskModeState {
  isLoading: boolean;
  isCancelling: boolean;
  error: string | null;
  _requestId: number;

  analyzeForMode: (description: string) => Promise<StrategyAnalysis | null>;
  enterTaskMode: (description: string, kernelSessionId?: string | null) => Promise<TaskModeSession | null>;
  confirmTaskConfiguration: (
    workflowConfig: TaskWorkflowConfigPayload,
    sessionId?: string | null,
  ) => Promise<TaskModeSession | null>;
  generatePrd: (
    conversationHistory?: CrossModeConversationTurn[],
    maxContextTokens?: number,
    overrideProvider?: string,
    overrideModel?: string,
    overrideBaseUrl?: string,
    sessionId?: string | null,
  ) => Promise<TaskPrd | null>;
  approvePrd: (
    prd: TaskPrd,
    sessionId?: string | null,
    workflowConfig?: TaskWorkflowConfigPayload | null,
  ) => Promise<boolean>;
  applyPrdFeedback: (
    feedback: string,
    conversationHistory?: CrossModeConversationTurn[],
    maxContextTokens?: number,
    overrideProvider?: string,
    overrideModel?: string,
    overrideBaseUrl?: string,
    sessionId?: string | null,
  ) => Promise<PrdFeedbackApplyResult | null>;
  refreshStatus: (sessionId?: string | null) => Promise<TaskExecutionStatus | null>;
  cancelExecution: (sessionId?: string | null) => Promise<boolean>;
  cancelOperation: (sessionId?: string | null) => Promise<boolean>;
  fetchReport: (sessionId?: string | null) => Promise<ExecutionReport | null>;
  exitTaskMode: (sessionId?: string | null) => Promise<boolean>;
  reset: () => void;
}

// ============================================================================
// Default State
// ============================================================================

const DEFAULT_STATE = {
  isLoading: false,
  isCancelling: false,
  error: null as string | null,
  _requestId: 0,
};

// ============================================================================
// Store
// ============================================================================

export const useTaskModeStore = create<TaskModeState>()((set, get) => ({
  ...DEFAULT_STATE,

  analyzeForMode: async (description: string) => {
    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });
    try {
      const result = await invoke<CommandResponse<StrategyAnalysis>>('analyze_task_for_mode', { description });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error ?? 'Analysis failed' });
        return null;
      }
      set({ isLoading: false });
      return result.data;
    } catch (e) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: String(e) });
      return null;
    }
  },

  enterTaskMode: async (description: string, kernelSessionId?: string | null) => {
    const requestId = get()._requestId + 1;
    set({ isLoading: true, isCancelling: false, error: null, _requestId: requestId });
    try {
      const { resolvePhaseAgent } = await import('../lib/phaseAgentResolver');
      const strategyResolved = resolvePhaseAgent('plan_strategy');
      const result = await invoke<CommandResponse<TaskModeSession>>('enter_task_mode', {
        request: {
          description,
          kernelSessionId: normalizeSessionId(kernelSessionId),
          locale: i18n.language || null,
          provider: strategyResolved.provider || null,
          model: strategyResolved.model || null,
          baseUrl: strategyResolved.baseUrl || null,
        },
      });
      if (get()._requestId !== requestId) return null;

      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error ?? 'Failed to enter task mode' });
        return null;
      }

      const session = result.data;
      activeTaskSessionId = normalizeSessionId(session.sessionId);
      useContextSourcesStore.getState().setMemorySessionId(activeTaskSessionId);

      const normalizedStatus = normalizeTaskModeSessionStatus(session.status);
      set({ isLoading: false, error: normalizedStatus.warning });
      if (normalizedStatus.warning) {
        return {
          ...session,
          status: normalizedStatus.status,
        };
      }
      return session;
    } catch (e) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: String(e) });
      return null;
    }
  },

  confirmTaskConfiguration: async (workflowConfig: TaskWorkflowConfigPayload, sessionId?: string | null) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active session' });
      return null;
    }

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });

    try {
      const result = await invoke<CommandResponse<TaskModeSession>>('confirm_task_configuration', {
        request: {
          sessionId: resolvedSessionId,
          workflowConfig: {
            flowLevel: workflowConfig.flowLevel,
            tddMode: workflowConfig.tddMode,
            enableInterview: workflowConfig.enableInterview,
            qualityGatesEnabled: workflowConfig.qualityGatesEnabled,
            selectedQualityGateIds: workflowConfig.selectedQualityGateIds,
            qualityRetryMaxAttempts: workflowConfig.qualityRetryMaxAttempts,
            customQualityGates: workflowConfig.customQualityGates,
            maxParallel: workflowConfig.maxParallel,
            skipVerification: workflowConfig.skipVerification,
            skipReview: workflowConfig.skipReview,
            globalAgentOverride: workflowConfig.globalAgentOverride,
            implAgentOverride: workflowConfig.implAgentOverride,
          },
        },
      });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error ?? 'Failed to confirm task configuration' });
        return null;
      }
      set({ isLoading: false });
      return result.data;
    } catch (e) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: String(e) });
      return null;
    }
  },

  generatePrd: async (
    conversationHistory?: CrossModeConversationTurn[],
    maxContextTokens?: number,
    overrideProvider?: string,
    overrideModel?: string,
    overrideBaseUrl?: string,
    sessionId?: string | null,
  ) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active session' });
      return null;
    }

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });

    try {
      const { finalProvider, finalModel, finalBaseUrl, settingsStore } = await resolveRequestLlm(
        overrideProvider,
        overrideModel,
        overrideBaseUrl,
      );

      const contextSourcesStore = useContextSourcesStore.getState();
      contextSourcesStore.setMemorySessionId(resolvedSessionId);
      const contextSources = contextSourcesStore.buildConfig() ?? null;

      const result = await invoke<CommandResponse<TaskPrd>>('generate_task_prd', {
        request: {
          sessionId: resolvedSessionId,
          provider: finalProvider || null,
          model: finalModel || null,
          apiKey: null,
          baseUrl: finalBaseUrl || null,
          conversationHistory: conversationHistory && conversationHistory.length > 0 ? conversationHistory : null,
          maxContextTokens: maxContextTokens ?? null,
          locale: i18n.language || null,
          contextSources,
          projectPath: settingsStore.workspacePath || null,
        },
      });
      if (get()._requestId !== requestId) return null;

      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error ?? 'PRD generation failed' });
        return null;
      }

      set({ isLoading: false });
      return result.data;
    } catch (e) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: String(e) });
      return null;
    }
  },

  approvePrd: async (prd: TaskPrd, sessionId?: string | null, workflowConfig?: TaskWorkflowConfigPayload | null) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active session' });
      return false;
    }

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });

    try {
      const settingsStore = (await import('./settings')).useSettingsStore.getState();
      const { provider, model, defaultAgent, phaseConfigs } = settingsStore;
      const { resolveProviderBaseUrl } = await import('../lib/providers');
      const baseUrl = provider ? resolveProviderBaseUrl(provider, settingsStore) : undefined;

      const contextSourcesStore = useContextSourcesStore.getState();
      contextSourcesStore.setMemorySessionId(resolvedSessionId);
      const contextSources = contextSourcesStore.buildConfig() ?? null;

      const result = await invoke<CommandResponse<boolean>>('approve_task_prd', {
        request: {
          sessionId: resolvedSessionId,
          prd,
          provider: provider || null,
          model: model || null,
          baseUrl: baseUrl || null,
          executionMode: null,
          workflowConfig: workflowConfig
            ? {
                flowLevel: workflowConfig.flowLevel,
                tddMode: workflowConfig.tddMode,
                enableInterview: workflowConfig.enableInterview,
                qualityGatesEnabled: workflowConfig.qualityGatesEnabled,
                selectedQualityGateIds: workflowConfig.selectedQualityGateIds,
                qualityRetryMaxAttempts: workflowConfig.qualityRetryMaxAttempts,
                customQualityGates: workflowConfig.customQualityGates,
                maxParallel: workflowConfig.maxParallel,
                skipVerification: workflowConfig.skipVerification,
                skipReview: workflowConfig.skipReview,
                globalAgentOverride: workflowConfig.globalAgentOverride,
                implAgentOverride: workflowConfig.implAgentOverride,
              }
            : null,
          globalDefaultAgent: defaultAgent || null,
          phaseConfigs,
          locale: i18n.language || null,
          contextSources,
          projectPath: settingsStore.workspacePath || null,
        },
      });
      if (get()._requestId !== requestId) return false;

      if (!result.success) {
        set({ isLoading: false, error: result.error ?? 'PRD approval failed' });
        return false;
      }

      set({ isLoading: false });
      return true;
    } catch (e) {
      if (get()._requestId !== requestId) return false;
      set({ isLoading: false, error: String(e) });
      return false;
    }
  },

  applyPrdFeedback: async (
    feedback: string,
    conversationHistory?: CrossModeConversationTurn[],
    maxContextTokens?: number,
    overrideProvider?: string,
    overrideModel?: string,
    overrideBaseUrl?: string,
    sessionId?: string | null,
  ) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    const normalizedFeedback = feedback.trim();
    if (!resolvedSessionId) {
      set({ error: 'No active session' });
      return null;
    }
    if (!normalizedFeedback) {
      set({ error: 'Feedback cannot be empty' });
      return null;
    }

    const requestId = get()._requestId + 1;
    set({ isLoading: true, error: null, _requestId: requestId });

    try {
      const { finalProvider, finalModel, finalBaseUrl, settingsStore } = await resolveRequestLlm(
        overrideProvider,
        overrideModel,
        overrideBaseUrl,
      );

      const contextSourcesStore = useContextSourcesStore.getState();
      contextSourcesStore.setMemorySessionId(resolvedSessionId);
      const contextSources = contextSourcesStore.buildConfig() ?? null;

      const result = await invoke<CommandResponse<PrdFeedbackApplyResult>>('apply_task_prd_feedback', {
        request: {
          sessionId: resolvedSessionId,
          feedback: normalizedFeedback,
          provider: finalProvider || null,
          model: finalModel || null,
          apiKey: null,
          baseUrl: finalBaseUrl || null,
          conversationHistory: conversationHistory && conversationHistory.length > 0 ? conversationHistory : null,
          maxContextTokens: maxContextTokens ?? null,
          locale: i18n.language || null,
          contextSources,
          projectPath: settingsStore.workspacePath || null,
        },
      });
      if (get()._requestId !== requestId) return null;
      if (!result.success || !result.data) {
        set({ isLoading: false, error: result.error ?? 'PRD feedback apply failed' });
        return null;
      }

      set({ isLoading: false, error: null });
      return result.data;
    } catch (e) {
      if (get()._requestId !== requestId) return null;
      set({ isLoading: false, error: String(e) });
      return null;
    }
  },

  refreshStatus: async (sessionId?: string | null) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) return null;

    try {
      const result = await invoke<CommandResponse<TaskExecutionStatus>>('get_task_execution_status', {
        sessionId: resolvedSessionId,
      });
      if (!result.success || !result.data) return null;

      const normalizedStatus = normalizeTaskModeSessionStatus(result.data.status);
      if (normalizedStatus.warning) {
        set({ error: normalizedStatus.warning });
      }

      return {
        ...result.data,
        status: normalizedStatus.status,
      };
    } catch {
      return null;
    }
  },

  cancelExecution: async (sessionId?: string | null) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active session' });
      return false;
    }
    if (get().isCancelling) return false;

    const requestId = get()._requestId + 1;
    set({ _requestId: requestId, isCancelling: true, error: null });

    try {
      const result = await invoke<CommandResponse<boolean>>('cancel_task_execution', { sessionId: resolvedSessionId });
      if (get()._requestId !== requestId) return false;

      if (!result.success) {
        set({ isCancelling: false, error: result.error ?? 'Cancel failed' });
        return false;
      }
      return true;
    } catch (e) {
      if (get()._requestId !== requestId) return false;
      set({ isCancelling: false, error: String(e) });
      return false;
    }
  },

  cancelOperation: async (sessionId?: string | null) => {
    const requestId = get()._requestId + 1;
    set({ _requestId: requestId, error: null });

    try {
      const result = await invoke<CommandResponse<boolean>>('cancel_task_operation', {
        sessionId: resolveSessionId(sessionId),
      });
      if (get()._requestId !== requestId) return false;
      if (!result.success) {
        set({ error: result.error ?? 'Cancel failed' });
        return false;
      }
      return true;
    } catch (e) {
      if (get()._requestId !== requestId) return false;
      set({ error: String(e) });
      return false;
    }
  },

  fetchReport: async (sessionId?: string | null) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    if (!resolvedSessionId) {
      set({ error: 'No active session' });
      return null;
    }

    try {
      const result = await invoke<CommandResponse<ExecutionReport>>('get_task_execution_report', {
        sessionId: resolvedSessionId,
      });
      if (result.success && result.data) {
        return result.data;
      }
      set({ error: result.error ?? 'Failed to fetch report' });
      return null;
    } catch (e) {
      set({ error: String(e) });
      return null;
    }
  },

  exitTaskMode: async (sessionId?: string | null) => {
    const resolvedSessionId = resolveSessionId(sessionId);
    const requestId = get()._requestId + 1;
    set({ _requestId: requestId, error: null });

    if (resolvedSessionId) {
      try {
        const result = await invoke<CommandResponse<boolean>>('exit_task_mode', { sessionId: resolvedSessionId });
        if (!result.success) {
          set({ error: result.error ?? 'Failed to exit task mode' });
          return false;
        }
      } catch (e) {
        set({ error: String(e) });
        return false;
      }
    }

    activeTaskSessionId = null;
    useContextSourcesStore.getState().setMemorySessionId(null);
    return true;
  },

  reset: () => {
    activeTaskSessionId = null;
    useContextSourcesStore.getState().setMemorySessionId(null);
    set((state) => ({ ...DEFAULT_STATE, _requestId: state._requestId + 1 }));
  },
}));

export default useTaskModeStore;
