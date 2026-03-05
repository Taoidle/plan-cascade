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
  normalizeStepOutputFormat,
} from './planOrchestrator/cardInjection';
import {
  proceedToPlanningFlow,
  retryClarificationFlow,
  skipClarificationFlow,
  startPlanWorkflowFlow,
  submitClarificationFlow,
} from './planOrchestrator/sessionFlow';
import { approvePlanFlow, cancelWorkflowFlow, retryStepFlow } from './planOrchestrator/executionFlow';

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
  startPlanWorkflow: (description: string) => Promise<{ modeSessionId: string | null }>;
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

async function syncKernelPlanPhase(phase: PlanModePhase, reasonCode?: string): Promise<void> {
  const transitionAndSubmitInput = useWorkflowKernelStore.getState().transitionAndSubmitInput;
  const session = useWorkflowKernelStore.getState().session;
  if (!session || session.activeMode !== 'plan') return;

  try {
    await transitionAndSubmitInput('plan', {
      type: 'system_phase_update',
      content: `phase:${phase}`,
      metadata: {
        mode: 'plan',
        phase,
        reasonCode: reasonCode ?? null,
      },
    });
  } catch {
    // best effort kernel phase sync
  }
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
  let unlistenRef: UnlistenFn | null = null;
  const unlisten = await listen<PlanModeProgressPayload>('plan-mode-progress', (event) => {
    if (get()._runToken !== runToken) return;
    const payload = event.payload;
    const { sessionId, stepStatuses: currentStepStatuses } = get();
    if (payload.sessionId !== sessionId) return;

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
    injectCard('plan_step_update', {
      eventType: payload.eventType as PlanStepUpdateCardData['eventType'],
      currentBatch: payload.currentBatch,
      totalBatches: payload.totalBatches,
      stepId: payload.stepId,
      stepTitle,
      stepStatus: payload.stepStatus,
      progressPct: payload.progressPct,
      error: payload.error,
    } satisfies PlanStepUpdateCardData);

    if (payload.eventType === 'step_completed' && payload.stepId) {
      const completedStepId = payload.stepId;
      const completedStepTitle = stepTitle ?? completedStepId;
      const stepOutputFromEvent = payload.stepOutput;
      if (stepOutputFromEvent) {
        injectCard('plan_step_output', {
          stepId: completedStepId,
          stepTitle: completedStepTitle,
          content: stepOutputFromEvent.content,
          format: normalizeStepOutputFormat(stepOutputFromEvent.format),
          criteriaMet: stepOutputFromEvent.criteriaMet ?? [],
        } satisfies PlanStepOutputCardData);
      } else {
        const planStore = usePlanModeStore.getState();
        void planStore
          .fetchStepOutput(completedStepId, sessionId)
          .then((stepOutput) => {
            if (get()._runToken !== runToken) return;
            if (!stepOutput) {
              const latestError = usePlanModeStore.getState().error;
              // Back-compat: older backends only persist outputs after full execution.
              if (latestError && !latestError.includes('No output for step')) {
                injectError(
                  i18n.t('planMode:orchestrator.stepOutputLoadFailed', 'Step Output Unavailable'),
                  latestError,
                );
              }
              return;
            }
            injectCard('plan_step_output', {
              stepId: completedStepId,
              stepTitle: completedStepTitle,
              content: stepOutput.content,
              format: normalizeStepOutputFormat(stepOutput.format),
              criteriaMet: stepOutput.criteriaMet ?? [],
            } satisfies PlanStepOutputCardData);
          })
          .catch((error) => {
            if (get()._runToken !== runToken) return;
            injectError(
              i18n.t('planMode:orchestrator.stepOutputLoadFailed', 'Step Output Unavailable'),
              error instanceof Error ? error.message : String(error),
            );
          });
      }
    }

    if (payload.eventType === 'execution_completed') {
      const mergedStepStatuses = {
        ...currentStepStatuses,
        ...(orchestratorPatch.stepStatuses ?? {}),
      };
      const hasFailedSteps = Object.values(mergedStepStatuses).some((status) => status === 'failed');
      const terminalPhase: PlanModePhase = hasFailedSteps ? 'failed' : 'completed';

      orchestratorPatch.isCancelling = false;
      set({
        phase: terminalPhase,
        isBusy: false,
        isCancelling: false,
        stepStatuses: mergedStepStatuses,
        _progressUnlisten: null,
      });
      unlistenRef?.();

      void (async () => {
        let fetchedReport: PlanExecutionReport | null = null;
        try {
          fetchedReport = await usePlanModeStore.getState().fetchReport(sessionId);
        } catch {
          fetchedReport = null;
        }
        if (get()._runToken !== runToken) return;
        if (get()._completionCardInjectedRunToken === runToken) return;

        const report = fetchedReport && fetchedReport.sessionId === payload.sessionId ? fetchedReport : get().report;
        const completionData = report
          ? buildPlanCompletionCardDataFromReport(report, terminalPhase === 'completed')
          : buildPlanCompletionCardDataFallback(plan, mergedStepStatuses, terminalPhase === 'completed');

        injectCard('plan_completion_card', completionData as unknown as Record<string, unknown>);
        set({ _completionCardInjectedRunToken: runToken, report: report ?? null });
      })();
      return;
    }

    if (payload.eventType === 'execution_cancelled') {
      if (get()._runToken !== runToken) return;
      orchestratorPatch.isCancelling = false;
      set({
        phase: 'cancelled',
        isBusy: false,
        isCancelling: false,
        stepStatuses: orchestratorPatch.stepStatuses ?? currentStepStatuses,
        _progressUnlisten: null,
      });
      unlistenRef?.();
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
  rollbackReasonCode?: string;
  startErrorTitle: string;
  invokeExecution: () => Promise<void>;
  get: PlanOrchestratorGet;
  set: PlanOrchestratorSet;
}

async function startPlanExecutionWithProgress({
  runToken,
  plan,
  rollbackPhase,
  rollbackReasonCode,
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
    await syncKernelPlanPhase(rollbackPhase, rollbackReasonCode);
    const message = error instanceof Error ? error.message : String(error);
    injectError(startErrorTitle, message);
    return failResult(rollbackReasonCode ?? 'execution_start_failed', message);
  }

  if (get()._runToken !== runToken) {
    stopProgressSubscription(get, set, unlisten);
    return failResult('stale_run_token', 'Execution start cancelled due to stale run token');
  }

  const latestError = usePlanModeStore.getState().error;
  if (latestError) {
    stopProgressSubscription(get, set, unlisten);
    set({ phase: rollbackPhase, isBusy: false, isCancelling: false });
    await syncKernelPlanPhase(rollbackPhase, rollbackReasonCode);
    injectError(startErrorTitle, latestError);
    return failResult(rollbackReasonCode ?? 'execution_start_failed', latestError);
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

  startPlanWorkflow: async (description: string) =>
    startPlanWorkflowFlow(description, {
      get,
      set,
      buildPlanContextSources,
      resolvePlanSessionId,
      normalizeKernelPlanPhase,
      syncKernelPlanPhase,
    }),

  submitClarification: async (answer: PlanClarifyAnswerCardData) =>
    submitClarificationFlow(answer, {
      get,
      set,
      buildPlanContextSources,
      resolvePlanSessionId,
      normalizeKernelPlanPhase,
      syncKernelPlanPhase,
    }),

  retryClarification: async () =>
    retryClarificationFlow({
      get,
      set,
      buildPlanContextSources,
      resolvePlanSessionId,
      normalizeKernelPlanPhase,
      syncKernelPlanPhase,
    }),

  skipClarification: async () =>
    skipClarificationFlow({
      get,
      set,
      buildPlanContextSources,
      resolvePlanSessionId,
      normalizeKernelPlanPhase,
      syncKernelPlanPhase,
    }),

  proceedToPlanning: async () =>
    proceedToPlanningFlow({
      get,
      set,
      buildPlanContextSources,
      resolvePlanSessionId,
      normalizeKernelPlanPhase,
      syncKernelPlanPhase,
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

  resetWorkflow: () => {
    const { _progressUnlisten } = get();
    if (_progressUnlisten) _progressUnlisten();
    buildPlanContextSources(null);
    set((state) => ({ ...DEFAULT_STATE, _runToken: state._runToken + 1 }));
  },
}));
