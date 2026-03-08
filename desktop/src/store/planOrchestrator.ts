/**
 * Plan Mode Orchestrator Store
 *
 * Orchestrates the Plan Mode workflow lifecycle in Simple mode.
 * Drives the state machine: idle → analyzing → [clarifying] → planning
 * → reviewing_plan → executing → completed/failed.
 *
 * Delegates to:
 * - usePlanModeStore: backend session lifecycle
 * - useExecutionStore: appendCard for structured card injection into chat transcript
 */

import { create } from 'zustand';
import i18n from '../i18n';
import { usePlanModeStore } from './planMode';
import { useWorkflowKernelStore } from './workflowKernel';
import { useContextSourcesStore } from './contextSources';
import { selectKernelPlanRuntime } from './workflowKernelSelectors';
import { failResult, okResult, type ActionResult } from '../types/actionResult';
import type {
  PlanModePhase,
  PlanAnalysisCardData,
  PlanCardData,
  PlanClarifyAnswerCardData,
  PlanClarifyQuestionCardData,
  PlanStepUpdateCardData,
  PlanStepOutputCardData,
  PlanModeProgressPayload,
  PlanExecutionReport,
} from '../types/planModeCard';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  buildPlanCompletionCardDataFallback,
  buildPlanCompletionCardDataFromReport,
  injectPlanCard as injectCard,
  injectPlanError as injectError,
} from './planOrchestrator/cardInjection';
import {
  proceedToPlanningFlow,
  retryClarificationFlow,
  skipClarificationFlow,
  startPlanWorkflowFlow,
  submitClarificationFlow,
} from './planOrchestrator/sessionFlow';
import { approvePlanFlow, cancelWorkflowFlow, retryStepFlow } from './planOrchestrator/executionFlow';
import { resolveRootSessionForMode } from './modeTranscriptRouting';

// ============================================================================
// Types
// ============================================================================

export interface PlanOrchestratorState {
  /** Current plan workflow phase */
  phase: PlanModePhase;

  /** Plan mode session ID */
  sessionId: string | null;

  /** Original task description */
  taskDescription: string;

  /** Analysis result */
  analysis: PlanAnalysisCardData | null;

  /** Pending clarification question waiting for user input */
  pendingClarifyQuestion: PlanClarifyQuestionCardData | null;

  /** Working copy of plan during review */
  editablePlan: PlanCardData | null;

  /** Step execution status projection (derived from progress events) */
  stepStatuses: Record<string, string>;

  /** Execution report projection */
  report: PlanExecutionReport | null;

  /** Is workflow busy */
  isBusy: boolean;

  /** True while waiting for backend execution_cancelled confirmation */
  isCancelling: boolean;

  /** Unlisten function for progress events */
  _progressUnlisten: UnlistenFn | null;

  /** Guards async continuation after cancellation/reset */
  _runToken: number;

  /** Prevent duplicate completion-card injection for a single run token */
  _completionCardInjectedRunToken: number | null;

  // Actions
  startPlanWorkflow: (
    description: string,
    kernelSessionId?: string | null,
  ) => Promise<{ modeSessionId: string | null }>;
  submitClarification: (answer: PlanClarifyAnswerCardData) => Promise<{
    ok: boolean;
    errorCode?: string | null;
  }>;
  retryClarification: () => Promise<void>;
  skipClarification: () => Promise<void>;
  proceedToPlanning: () => Promise<void>;
  approvePlan: (plan: PlanCardData) => Promise<ActionResult>;
  retryStep: (stepId: string) => Promise<void>;
  cancelWorkflow: () => Promise<void>;
  ensureTerminalCompletionCardFromKernel: () => Promise<void>;
  resetWorkflow: () => void;
}

// ============================================================================
// Helpers
// ============================================================================

function buildPlanContextSources(sessionId?: string | null) {
  const contextSourcesStore = useContextSourcesStore.getState();
  contextSourcesStore.setMemorySessionId(sessionId?.trim() || null);
  return contextSourcesStore.buildConfig();
}

function normalizeKernelPlanPhase(phase: string | null | undefined): PlanModePhase {
  switch ((phase ?? '').toLowerCase()) {
    case 'analyzing':
      return 'analyzing';
    case 'clarifying':
      return 'clarifying';
    case 'clarification_error':
      return 'clarification_error';
    case 'planning':
      return 'planning';
    case 'reviewing_plan':
      return 'reviewing_plan';
    case 'executing':
      return 'executing';
    case 'completed':
      return 'completed';
    case 'failed':
      return 'failed';
    case 'cancelled':
      return 'cancelled';
    default:
      return 'idle';
  }
}

function resolvePlanSessionId(get: PlanOrchestratorGet, set: PlanOrchestratorSet): string | null {
  const localSessionId = get().sessionId?.trim() ?? '';
  if (localSessionId.length > 0) return localSessionId;

  const linkedSessionId =
    selectKernelPlanRuntime(useWorkflowKernelStore.getState().session).linkedSessionId?.trim() ?? '';
  if (linkedSessionId.length > 0) {
    set({ sessionId: linkedSessionId });
    return linkedSessionId;
  }
  return null;
}

type PlanOrchestratorGet = () => PlanOrchestratorState;
type PlanOrchestratorSet = (partial: Partial<PlanOrchestratorState>) => void;

function stopProgressSubscription(get: PlanOrchestratorGet, set: PlanOrchestratorSet, fallbackUnlisten?: UnlistenFn) {
  const activeUnlisten = get()._progressUnlisten;
  if (activeUnlisten) {
    activeUnlisten();
  } else if (fallbackUnlisten) {
    fallbackUnlisten();
  }
  set({ _progressUnlisten: null });
}

async function subscribePlanProgressEvents(
  runToken: number,
  plan: PlanCardData,
  get: PlanOrchestratorGet,
  set: PlanOrchestratorSet,
): Promise<UnlistenFn> {
  const stepUpdateEvents = new Set<PlanStepUpdateCardData['eventType']>([
    'batch_started',
    'step_started',
    'step_completed',
    'step_failed',
    'step_retrying',
    'batch_blocked',
  ]);
  let unlistenRef: UnlistenFn | null = null;
  const unlisten = await listen<PlanModeProgressPayload>('plan-mode-progress', (event) => {
    if (get()._runToken !== runToken) return;
    const payload = event.payload;
    const { stepStatuses: currentStepStatuses, sessionId } = get();
    if (sessionId && payload.sessionId !== sessionId) return;
    const hasAuthoritativeKernelTranscript = !!resolveRootSessionForMode('plan', payload.sessionId);

    const orchestratorPatch: {
      stepStatuses?: Record<string, string>;
      isCancelling?: boolean;
    } = {
      stepStatuses: currentStepStatuses,
    };
    if (payload.stepId && payload.stepStatus) {
      orchestratorPatch.stepStatuses = {
        ...currentStepStatuses,
        [payload.stepId]: payload.stepStatus,
      };
    }

    const stepTitle = plan.steps.find((s) => s.id === payload.stepId)?.title;
    if (
      !hasAuthoritativeKernelTranscript &&
      stepUpdateEvents.has(payload.eventType as PlanStepUpdateCardData['eventType'])
    ) {
      const stepOutputFromEvent = payload.stepOutput;
      injectCard('plan_step_update', {
        eventType: payload.eventType as PlanStepUpdateCardData['eventType'],
        currentBatch: payload.currentBatch,
        totalBatches: payload.totalBatches,
        stepId: payload.stepId,
        stepTitle,
        stepStatus: payload.stepStatus,
        progressPct: payload.progressPct,
        error: payload.error,
        attemptCount: payload.attemptCount,
        errorCode: payload.errorCode,
        diagnostics: stepOutputFromEvent
          ? {
              summary: stepOutputFromEvent.summary,
              content: stepOutputFromEvent.content,
              fullContent: stepOutputFromEvent.fullContent ?? stepOutputFromEvent.content,
              format:
                stepOutputFromEvent.format === 'markdown' ||
                stepOutputFromEvent.format === 'json' ||
                stepOutputFromEvent.format === 'html' ||
                stepOutputFromEvent.format === 'code'
                  ? stepOutputFromEvent.format
                  : 'text',
              truncated: stepOutputFromEvent.truncated ?? false,
              originalLength: stepOutputFromEvent.originalLength ?? stepOutputFromEvent.content.length,
              shownLength: stepOutputFromEvent.shownLength ?? stepOutputFromEvent.content.length,
              qualityState: stepOutputFromEvent.qualityState,
              incompleteReason: stepOutputFromEvent.incompleteReason,
              attemptCount: stepOutputFromEvent.attemptCount,
              toolEvidence: stepOutputFromEvent.toolEvidence ?? [],
              iterations: stepOutputFromEvent.iterations,
              stopReason: stepOutputFromEvent.stopReason,
              errorCode: stepOutputFromEvent.errorCode,
            }
          : undefined,
      } satisfies PlanStepUpdateCardData);
    }

    if (!hasAuthoritativeKernelTranscript && payload.eventType === 'step_completed' && payload.stepId) {
      const completedStepId = payload.stepId;
      const completedStepTitle = stepTitle ?? completedStepId;
      const stepOutputFromEvent = payload.stepOutput;
      if (stepOutputFromEvent) {
        injectCard('plan_step_output', {
          stepId: completedStepId,
          stepTitle: completedStepTitle,
          summary: stepOutputFromEvent.summary,
          content: stepOutputFromEvent.summary ?? stepOutputFromEvent.content,
          fullContent: stepOutputFromEvent.fullContent ?? stepOutputFromEvent.content,
          format:
            stepOutputFromEvent.format === 'markdown' ||
            stepOutputFromEvent.format === 'json' ||
            stepOutputFromEvent.format === 'html' ||
            stepOutputFromEvent.format === 'code'
              ? stepOutputFromEvent.format
              : 'text',
          artifacts: stepOutputFromEvent.artifacts ?? [],
          truncated: stepOutputFromEvent.truncated ?? false,
          originalLength: stepOutputFromEvent.originalLength ?? stepOutputFromEvent.content.length,
          shownLength: stepOutputFromEvent.shownLength ?? stepOutputFromEvent.content.length,
          qualityState: stepOutputFromEvent.qualityState,
          incompleteReason: stepOutputFromEvent.incompleteReason,
          attemptCount: stepOutputFromEvent.attemptCount,
          toolEvidence: stepOutputFromEvent.toolEvidence ?? [],
          iterations: stepOutputFromEvent.iterations,
          stopReason: stepOutputFromEvent.stopReason,
          errorCode: stepOutputFromEvent.errorCode,
          criteriaMet: stepOutputFromEvent.criteriaMet ?? [],
        } satisfies PlanStepOutputCardData);
      } else {
        injectError(
          i18n.t('planMode:orchestrator.stepOutputLoadFailed', 'Step Output Unavailable'),
          `Missing step output in progress event for step '${completedStepId}'`,
        );
      }
    }

    if (payload.eventType === 'execution_completed') {
      const mergedStepStatuses = {
        ...currentStepStatuses,
        ...(orchestratorPatch.stepStatuses ?? {}),
      };

      orchestratorPatch.isCancelling = false;
      set({
        isBusy: false,
        isCancelling: false,
        stepStatuses: mergedStepStatuses,
        _progressUnlisten: null,
      });
      unlistenRef?.();

      if (get()._completionCardInjectedRunToken !== runToken) {
        const progressReport = payload.terminalReport ?? null;
        const existingReport = get().report;
        const effectiveReport =
          progressReport && progressReport.sessionId === payload.sessionId ? progressReport : existingReport;
        if (hasAuthoritativeKernelTranscript) {
          set({ _completionCardInjectedRunToken: runToken, report: effectiveReport ?? null });
        } else if (effectiveReport && effectiveReport.sessionId === payload.sessionId) {
          const immediateCompletionData = buildPlanCompletionCardDataFromReport(effectiveReport);
          injectCard('plan_completion_card', immediateCompletionData as unknown as Record<string, unknown>);
          set({ _completionCardInjectedRunToken: runToken, report: effectiveReport });
        } else {
          const fallbackTerminal: PlanModePhase = Object.values(mergedStepStatuses).some(
            (status) => status === 'failed',
          )
            ? 'failed'
            : 'completed';
          const fallbackData = buildPlanCompletionCardDataFallback(plan, mergedStepStatuses, fallbackTerminal);
          injectCard('plan_completion_card', fallbackData as unknown as Record<string, unknown>);
          set({ _completionCardInjectedRunToken: runToken });
        }
      }
      return;
    }

    if (payload.eventType === 'execution_cancelled') {
      if (get()._runToken !== runToken) return;
      orchestratorPatch.isCancelling = false;
      set({
        isBusy: false,
        isCancelling: false,
        stepStatuses: orchestratorPatch.stepStatuses ?? currentStepStatuses,
        _progressUnlisten: null,
      });
      unlistenRef?.();
      if (get()._completionCardInjectedRunToken !== runToken) {
        const progressReport = payload.terminalReport ?? null;
        if (hasAuthoritativeKernelTranscript) {
          set({ _completionCardInjectedRunToken: runToken, report: progressReport });
        } else if (progressReport && progressReport.sessionId === payload.sessionId) {
          const cancelledData = buildPlanCompletionCardDataFromReport(progressReport);
          injectCard('plan_completion_card', cancelledData as unknown as Record<string, unknown>);
          set({ _completionCardInjectedRunToken: runToken, report: progressReport });
        } else {
          const fallbackData = buildPlanCompletionCardDataFallback(
            plan,
            orchestratorPatch.stepStatuses ?? currentStepStatuses,
            'cancelled',
          );
          injectCard('plan_completion_card', fallbackData as unknown as Record<string, unknown>);
          set({ _completionCardInjectedRunToken: runToken });
        }
      }
      return;
    }

    set({ stepStatuses: orchestratorPatch.stepStatuses ?? currentStepStatuses });
  });
  unlistenRef = unlisten;
  return unlisten;
}

interface PlanExecutionStartParams {
  runToken: number;
  plan: PlanCardData;
  rollbackPhase: PlanModePhase;
  startErrorTitle: string;
  invokeExecution: () => Promise<void>;
  get: PlanOrchestratorGet;
  set: PlanOrchestratorSet;
}

async function startPlanExecutionWithProgress({
  runToken,
  plan,
  rollbackPhase,
  startErrorTitle,
  invokeExecution,
  get,
  set,
}: PlanExecutionStartParams): Promise<ActionResult> {
  const unlisten = await subscribePlanProgressEvents(runToken, plan, get, set);
  if (get()._runToken !== runToken) {
    unlisten();
    return failResult('stale_run_token', 'Execution start cancelled due to stale run token');
  }
  set({ _progressUnlisten: unlisten });

  try {
    await invokeExecution();
  } catch (error) {
    stopProgressSubscription(get, set, unlisten);
    set({ phase: rollbackPhase, isBusy: false, isCancelling: false });
    const message = error instanceof Error ? error.message : String(error);
    injectError(startErrorTitle, message);
    return failResult('execution_start_failed', message);
  }

  if (get()._runToken !== runToken) {
    stopProgressSubscription(get, set, unlisten);
    return failResult('stale_run_token', 'Execution start cancelled due to stale run token');
  }

  const latestError = usePlanModeStore.getState().error;
  if (latestError) {
    stopProgressSubscription(get, set, unlisten);
    set({ phase: rollbackPhase, isBusy: false, isCancelling: false });
    injectError(startErrorTitle, latestError);
    return failResult('execution_start_failed', latestError);
  }

  return okResult();
}

// ============================================================================
// Default State
// ============================================================================

const DEFAULT_STATE = {
  phase: 'idle' as PlanModePhase,
  sessionId: null as string | null,
  taskDescription: '',
  analysis: null as PlanAnalysisCardData | null,
  pendingClarifyQuestion: null as PlanClarifyQuestionCardData | null,
  editablePlan: null as PlanCardData | null,
  stepStatuses: {} as Record<string, string>,
  report: null as PlanExecutionReport | null,
  isBusy: false,
  isCancelling: false,
  _progressUnlisten: null as UnlistenFn | null,
  _runToken: 0,
  _completionCardInjectedRunToken: null as number | null,
};

// ============================================================================
// Store
// ============================================================================

export const usePlanOrchestratorStore = create<PlanOrchestratorState>((set, get) => ({
  ...DEFAULT_STATE,

  startPlanWorkflow: async (description: string, kernelSessionId?: string | null) =>
    startPlanWorkflowFlow(
      description,
      {
        get,
        set,
        buildPlanContextSources,
        resolvePlanSessionId,
        normalizeKernelPlanPhase,
      },
      kernelSessionId,
    ),

  submitClarification: async (answer: PlanClarifyAnswerCardData) =>
    submitClarificationFlow(answer, {
      get,
      set,
      buildPlanContextSources,
      resolvePlanSessionId,
      normalizeKernelPlanPhase,
    }),

  retryClarification: async () =>
    retryClarificationFlow({
      get,
      set,
      buildPlanContextSources,
      resolvePlanSessionId,
      normalizeKernelPlanPhase,
    }),

  skipClarification: async () =>
    skipClarificationFlow({
      get,
      set,
      buildPlanContextSources,
      resolvePlanSessionId,
      normalizeKernelPlanPhase,
    }),

  proceedToPlanning: async () =>
    proceedToPlanningFlow({
      get,
      set,
      buildPlanContextSources,
      resolvePlanSessionId,
      normalizeKernelPlanPhase,
    }),

  approvePlan: async (plan: PlanCardData) =>
    approvePlanFlow(plan, {
      get,
      set,
      resolvePlanSessionId,
      buildPlanContextSources,
      startPlanExecutionWithProgress,
      defaultState: DEFAULT_STATE,
    }),

  retryStep: async (stepId: string) =>
    retryStepFlow(stepId, {
      get,
      set,
      resolvePlanSessionId,
      buildPlanContextSources,
      startPlanExecutionWithProgress,
      defaultState: DEFAULT_STATE,
    }),

  cancelWorkflow: async () =>
    cancelWorkflowFlow({
      get,
      set,
      resolvePlanSessionId,
      buildPlanContextSources,
      startPlanExecutionWithProgress,
      defaultState: DEFAULT_STATE,
    }),

  ensureTerminalCompletionCardFromKernel: async () => {
    const runToken = get()._runToken;
    if (get()._completionCardInjectedRunToken === runToken) return;

    const sessionId = resolvePlanSessionId(get, set);
    if (!sessionId) return;

    const kernelSession = useWorkflowKernelStore.getState().session;
    const kernelPlanPhase = normalizeKernelPlanPhase(selectKernelPlanRuntime(kernelSession).phase);
    if (kernelPlanPhase !== 'completed' && kernelPlanPhase !== 'failed' && kernelPlanPhase !== 'cancelled') {
      return;
    }

    const fetchedReport = await usePlanModeStore.getState().fetchReport(sessionId);
    const plan = get().editablePlan;
    const completionData =
      fetchedReport && fetchedReport.sessionId === sessionId
        ? buildPlanCompletionCardDataFromReport(fetchedReport)
        : plan
          ? buildPlanCompletionCardDataFallback(plan, get().stepStatuses, kernelPlanPhase)
          : null;
    if (!completionData) return;

    injectCard('plan_completion_card', completionData as unknown as Record<string, unknown>);
    set({
      report: fetchedReport ?? get().report,
      _completionCardInjectedRunToken: runToken,
      isBusy: false,
      isCancelling: false,
    });
  },

  resetWorkflow: () => {
    const { _progressUnlisten } = get();
    if (_progressUnlisten) _progressUnlisten();
    buildPlanContextSources(null);
    set((state) => ({ ...DEFAULT_STATE, _runToken: state._runToken + 1 }));
  },
}));
