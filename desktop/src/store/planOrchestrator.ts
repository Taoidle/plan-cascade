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
import { useExecutionStore } from './execution';
import { usePlanModeStore } from './planMode';
import { useWorkflowKernelStore } from './workflowKernel';
import { useSettingsStore } from './settings';
import { useContextSourcesStore } from './contextSources';
import { selectKernelPlanRuntime } from './workflowKernelSelectors';
import { buildConversationHistory } from '../lib/contextBridge';
import { getNextTurnId } from '../lib/conversationUtils';
import { createWorkflowKernelActionIntent } from '../lib/workflowKernelIntent';
import type { CardPayload } from '../types/workflowCard';
import type {
  PlanModePhase,
  PlanAnalysisCardData,
  PlanCardData,
  PlanClarifyAnswerCardData,
  PlanClarifyQuestionCardData,
  PlanClarificationResolutionCardData,
  PlanStepUpdateCardData,
  PlanStepOutputCardData,
  PlanCompletionCardData,
  PlanPersonaIndicatorData,
  PlanModeProgressPayload,
  PlanExecutionReport,
} from '../types/planModeCard';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

// ============================================================================
// Types
// ============================================================================

interface PlanOrchestratorState {
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
  approvePlan: (plan: PlanCardData) => Promise<void>;
  retryStep: (stepId: string) => Promise<void>;
  cancelWorkflow: () => Promise<void>;
  resetWorkflow: () => void;
}

// ============================================================================
// Helpers
// ============================================================================

function injectCard(cardType: string, data: Record<string, unknown>, interactive = false) {
  const payload: CardPayload = {
    cardType: cardType as CardPayload['cardType'],
    cardId: `${cardType}-${Date.now()}`,
    data: data as unknown as CardPayload['data'],
    interactive,
  };
  useExecutionStore.getState().appendCard(payload);
}

function injectInfo(message: string) {
  injectCard('workflow_info', { message, level: 'info' });
}

function injectError(title: string, description: string) {
  injectCard('workflow_error', { title, description, suggestedFix: '' });
}

function normalizeStepOutputFormat(format: string | undefined): PlanStepOutputCardData['format'] {
  if (format === 'markdown' || format === 'json' || format === 'html' || format === 'code') return format;
  return 'text';
}

function buildPlanCompletionCardDataFromReport(report: PlanExecutionReport, success: boolean): PlanCompletionCardData {
  return {
    success,
    planTitle: report.planTitle,
    totalSteps: report.totalSteps,
    stepsCompleted: report.stepsCompleted,
    stepsFailed: report.stepsFailed,
    totalDurationMs: report.totalDurationMs,
    stepSummaries: report.stepSummaries,
  };
}

function buildPlanCompletionCardDataFallback(
  plan: PlanCardData,
  stepStatuses: Record<string, string>,
  success: boolean,
): PlanCompletionCardData {
  const totalSteps = plan.steps.length;
  const stepsCompleted = plan.steps.filter((step) => stepStatuses[step.id] === 'completed').length;
  const stepsFailed = plan.steps.filter((step) => stepStatuses[step.id] === 'failed').length;
  const stepSummaries = Object.fromEntries(
    plan.steps.map((step) => {
      const status = stepStatuses[step.id];
      if (status === 'completed') {
        return [step.id, 'Completed'];
      }
      if (status === 'failed') {
        return [step.id, 'Failed'];
      }
      if (typeof status === 'string' && status.trim().length > 0) {
        return [step.id, `Status: ${status}`];
      }
      return [step.id, 'No summary available'];
    }),
  );

  return {
    success,
    planTitle: plan.title,
    totalSteps,
    stepsCompleted,
    stepsFailed,
    totalDurationMs: 0,
    stepSummaries,
  };
}

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

function injectClarificationResolutionCard(reasonCode: string | null, message: string) {
  injectCard(
    'plan_clarification_resolution',
    {
      title: i18n.t('planMode:clarify.recoveryTitle', 'Clarification needs attention'),
      message,
      reasonCode,
      canRetry: true,
      canSkip: true,
      canCancel: true,
    } satisfies PlanClarificationResolutionCardData,
    true,
  );
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
      const planStore = usePlanModeStore.getState();
      void planStore
        .fetchStepOutput(completedStepId, sessionId)
        .then((stepOutput) => {
          if (get()._runToken !== runToken) return;
          if (!stepOutput) {
            const latestError = usePlanModeStore.getState().error;
            if (latestError) {
              injectError(i18n.t('planMode:orchestrator.stepOutputLoadFailed', 'Step Output Unavailable'), latestError);
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
}: PlanExecutionStartParams): Promise<boolean> {
  const unlisten = await subscribePlanProgressEvents(runToken, plan, get, set);
  if (get()._runToken !== runToken) {
    unlisten();
    return false;
  }
  set({ _progressUnlisten: unlisten });

  try {
    await invokeExecution();
  } catch (error) {
    stopProgressSubscription(get, set, unlisten);
    set({ phase: rollbackPhase, isBusy: false, isCancelling: false });
    await syncKernelPlanPhase(rollbackPhase, rollbackReasonCode);
    injectError(startErrorTitle, error instanceof Error ? error.message : String(error));
    return false;
  }

  if (get()._runToken !== runToken) {
    stopProgressSubscription(get, set, unlisten);
    return false;
  }

  const latestError = usePlanModeStore.getState().error;
  if (latestError) {
    stopProgressSubscription(get, set, unlisten);
    set({ phase: rollbackPhase, isBusy: false, isCancelling: false });
    await syncKernelPlanPhase(rollbackPhase, rollbackReasonCode);
    injectError(startErrorTitle, latestError);
    return false;
  }

  return true;
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

  startPlanWorkflow: async (description: string) => {
    const runToken = get()._runToken + 1;
    let modeSessionId: string | null = null;
    const planStore = usePlanModeStore.getState();
    const settings = useSettingsStore.getState();
    const { resolveProviderBaseUrl } = await import('../lib/providers');
    const { resolvePhaseAgent } = await import('../lib/phaseAgentResolver');

    set({ isBusy: true, isCancelling: false, taskDescription: description, phase: 'analyzing', _runToken: runToken });

    // Add user message to chat transcript so it appears as a chat bubble
    const executionState = useExecutionStore.getState();
    const turnId = getNextTurnId(executionState.streamingOutput);
    executionState.appendStreamLine(description, 'info', undefined, undefined, { turnId, turnBoundary: 'user' });

    // Inject persona indicator
    injectCard('plan_persona_indicator', {
      role: 'planner',
      displayName: i18n.t('planMode:personas.planner', 'Planner'),
      phase: 'analyzing',
    } satisfies PlanPersonaIndicatorData);

    injectInfo(i18n.t('planMode:orchestrator.analyzingTask', 'Analyzing task...'));

    // Build conversation context
    const conversationHistory = buildConversationHistory();
    const contextStr =
      conversationHistory.length > 0
        ? conversationHistory.map((t) => `user: ${t.user}\nassistant: ${t.assistant}`).join('\n')
        : undefined;

    // Resolve LLM from phase config first, then fall back to global settings.
    const strategyAgent = resolvePhaseAgent('plan_strategy');
    const provider = strategyAgent.provider || settings.provider;
    const model = strategyAgent.model || settings.model;
    const baseUrl = strategyAgent.baseUrl ?? (provider ? resolveProviderBaseUrl(provider, settings) : undefined);
    const projectPath = settings.workspacePath || undefined;
    const contextSources = buildPlanContextSources(null);

    // Enter plan mode (runs analysis)
    const enteredSession = await planStore.enterPlanMode(
      description,
      provider,
      model,
      baseUrl,
      projectPath,
      contextSources,
      contextStr,
      i18n.language,
    );
    if (get()._runToken !== runToken) return { modeSessionId: null };

    const kernelSession = useWorkflowKernelStore.getState().session;
    const kernelPlanRuntime = selectKernelPlanRuntime(kernelSession);
    const kernelPendingClarification = kernelPlanRuntime.pendingClarification;
    const fallbackQuestion = kernelPendingClarification
      ? ({
          questionId: kernelPendingClarification.questionId,
          question: kernelPendingClarification.question,
          hint: kernelPendingClarification.hint,
          inputType:
            kernelPendingClarification.inputType === 'single_select' ||
            kernelPendingClarification.inputType === 'boolean' ||
            kernelPendingClarification.inputType === 'textarea'
              ? kernelPendingClarification.inputType
              : 'text',
          options: kernelPendingClarification.options ?? [],
        } satisfies PlanClarifyQuestionCardData)
      : null;
    const fallbackSessionId = kernelPlanRuntime.linkedSessionId;
    const fallbackPhase = fallbackQuestion ? 'clarifying' : normalizeKernelPlanPhase(kernelPlanRuntime.phase);
    const fallbackSession =
      !enteredSession && fallbackSessionId
        ? {
            sessionId: fallbackSessionId,
            phase: fallbackPhase,
            analysis: null,
            currentQuestion: fallbackQuestion,
          }
        : null;
    const activeSession = enteredSession ?? fallbackSession;
    const { error } = usePlanModeStore.getState();

    if (error || !activeSession) {
      if (get()._runToken !== runToken) return { modeSessionId: null };
      injectError(
        i18n.t('planMode:orchestrator.analysisFailed', 'Analysis Failed'),
        error || 'Failed to enter plan mode',
      );
      set({ isBusy: false, phase: 'failed' });
      return { modeSessionId: null };
    }

    const { analysis, phase: enteredPhase, sessionId, currentQuestion } = activeSession;
    modeSessionId = sessionId;

    if (get()._runToken !== runToken) return { modeSessionId: null };
    set({ sessionId, analysis });
    buildPlanContextSources(sessionId);

    // Inject analysis card
    if (analysis) {
      injectCard('plan_analysis_card', analysis as unknown as Record<string, unknown>);
    }

    // Proceed based on phase
    if (enteredPhase === 'clarifying') {
      if (get()._runToken !== runToken) return { modeSessionId: modeSessionId ?? null };
      set({ phase: 'clarifying', isBusy: false, pendingClarifyQuestion: currentQuestion ?? null });

      injectCard('plan_persona_indicator', {
        role: 'analyst',
        displayName: i18n.t('planMode:personas.analyst', 'Analyst'),
        phase: 'clarifying',
      } satisfies PlanPersonaIndicatorData);
      injectInfo(
        i18n.t('planMode:orchestrator.needsClarification', 'Some details need clarification before planning.'),
      );

      // Inject first question card if available, otherwise degrade gracefully instead of getting stuck.
      if (currentQuestion) {
        injectCard('plan_clarify_question', currentQuestion as unknown as Record<string, unknown>);
      } else {
        const message = i18n.t(
          'planMode:orchestrator.clarificationFailed',
          'Clarification failed. Please choose next action.',
        );
        injectInfo(message);
        injectClarificationResolutionCard('clarification_question_missing', message);
        set({ phase: 'clarification_error', isBusy: false, pendingClarifyQuestion: null });
        await syncKernelPlanPhase('clarification_error', 'clarification_question_missing');
      }
    } else {
      // Skip to planning
      if (get()._runToken !== runToken) return { modeSessionId: modeSessionId ?? null };
      await get().proceedToPlanning();
    }
    return { modeSessionId: modeSessionId ?? null };
  },

  submitClarification: async (answer: PlanClarifyAnswerCardData) => {
    const runToken = get()._runToken;
    const effectiveSessionId = resolvePlanSessionId(get, set);
    if (!effectiveSessionId) {
      injectError(i18n.t('planMode:orchestrator.clarificationFailed', 'Clarification failed.'), 'No active session');
      set({ isBusy: false, pendingClarifyQuestion: null });
      return { ok: false, errorCode: 'missing_session' };
    }

    // Inject answer card immediately
    injectCard('plan_clarify_answer', answer as unknown as Record<string, unknown>);

    set({ pendingClarifyQuestion: null, isBusy: true });
    injectInfo(i18n.t('planMode:orchestrator.generatingQuestion', 'Generating next question...'));

    const planStore = usePlanModeStore.getState();
    const settings = useSettingsStore.getState();
    const projectPath = settings.workspacePath || undefined;
    const contextSources = buildPlanContextSources(effectiveSessionId);
    const conversationHistory = buildConversationHistory();
    const contextStr =
      conversationHistory.length > 0
        ? conversationHistory.map((t) => `user: ${t.user}\nassistant: ${t.assistant}`).join('\n')
        : undefined;
    const updatedSession = await planStore.submitClarification(
      answer,
      undefined,
      undefined,
      undefined,
      projectPath,
      contextSources,
      contextStr,
      i18n.language,
      effectiveSessionId,
    );
    if (get()._runToken !== runToken) return { ok: false, errorCode: 'stale_run_token' };

    if (!updatedSession) {
      // Submission failed — enter clarification recovery state and wait for user decision.
      if (get()._runToken !== runToken) return { ok: false, errorCode: 'stale_run_token' };
      const message = i18n.t(
        'planMode:orchestrator.clarificationFailed',
        'Clarification failed. Please retry, skip, or cancel.',
      );
      injectInfo(message);
      injectClarificationResolutionCard('clarification_submit_failed', message);
      set({ phase: 'clarification_error', pendingClarifyQuestion: null, isBusy: false });
      await syncKernelPlanPhase('clarification_error', 'clarification_submit_failed');
      return { ok: false, errorCode: 'clarification_submit_failed' };
    }

    if (updatedSession.phase === 'planning') {
      // Clarification complete — transition to planning
      injectInfo(i18n.t('planMode:orchestrator.clarificationComplete', 'Clarification complete.'));
      set({ pendingClarifyQuestion: null, isBusy: false });
      if (get()._runToken !== runToken) return { ok: false, errorCode: 'stale_run_token' };
      await get().proceedToPlanning();
      return { ok: true };
    } else if (updatedSession.currentQuestion) {
      // Next question available
      if (get()._runToken !== runToken) return { ok: false, errorCode: 'stale_run_token' };
      set({ pendingClarifyQuestion: updatedSession.currentQuestion, isBusy: false });
      injectCard('plan_clarify_question', updatedSession.currentQuestion as unknown as Record<string, unknown>);
      return { ok: true };
    } else {
      // No question and still clarifying — enter clarification recovery state.
      if (get()._runToken !== runToken) return { ok: false, errorCode: 'stale_run_token' };
      const message = i18n.t(
        'planMode:orchestrator.clarificationFailed',
        'Clarification failed. Please retry, skip, or cancel.',
      );
      set({ phase: 'clarification_error', pendingClarifyQuestion: null, isBusy: false });
      injectInfo(message);
      injectClarificationResolutionCard('clarification_question_missing', message);
      await syncKernelPlanPhase('clarification_error', 'clarification_question_missing');
      return { ok: false, errorCode: 'clarification_question_missing' };
    }
  },

  retryClarification: async () => {
    const runToken = get()._runToken;
    const description = get().taskDescription.trim();
    if (!description) return;

    injectInfo(i18n.t('planMode:orchestrator.retryClarification', 'Retrying clarification...'));
    set({ phase: 'analyzing', isBusy: true, pendingClarifyQuestion: null });
    await syncKernelPlanPhase('analyzing', 'clarification_retry');

    if (get()._runToken !== runToken) return;
    await get().startPlanWorkflow(description);
  },

  skipClarification: async () => {
    const runToken = get()._runToken;
    const effectiveSessionId = resolvePlanSessionId(get, set);
    if (!effectiveSessionId) {
      injectError(i18n.t('planMode:orchestrator.clarificationFailed', 'Clarification failed.'), 'No active session');
      return;
    }
    set({ pendingClarifyQuestion: null });
    injectInfo(i18n.t('planMode:orchestrator.clarificationSkipped', 'Clarification skipped.'));
    const planStore = usePlanModeStore.getState();
    await planStore.skipClarification(effectiveSessionId);
    if (get()._runToken !== runToken) return;
    await get().proceedToPlanning();
  },

  proceedToPlanning: async () => {
    const runToken = get()._runToken;
    const effectiveSessionId = resolvePlanSessionId(get, set);
    if (!effectiveSessionId) {
      injectError(i18n.t('planMode:orchestrator.planFailed', 'Plan Generation Failed'), 'No active session');
      set({ isBusy: false, phase: 'failed' });
      return;
    }
    set({ phase: 'planning', isBusy: true });

    injectCard('plan_persona_indicator', {
      role: 'planner',
      displayName: i18n.t('planMode:personas.planner', 'Planner'),
      phase: 'planning',
    } satisfies PlanPersonaIndicatorData);

    injectInfo(i18n.t('planMode:orchestrator.generatingPlan', 'Generating plan...'));

    const conversationHistory = buildConversationHistory();
    const contextStr =
      conversationHistory.length > 0
        ? conversationHistory.map((t) => `user: ${t.user}\nassistant: ${t.assistant}`).join('\n')
        : undefined;
    const settings = useSettingsStore.getState();
    const projectPath = settings.workspacePath || undefined;
    const contextSources = buildPlanContextSources(effectiveSessionId);

    const planStore = usePlanModeStore.getState();
    const generatedPlan = await planStore.generatePlan(
      undefined,
      undefined,
      undefined,
      projectPath,
      contextSources,
      contextStr,
      i18n.language,
      effectiveSessionId,
    );
    if (get()._runToken !== runToken) return;

    const { error } = usePlanModeStore.getState();
    const plan = generatedPlan;

    if (error || !plan) {
      if (get()._runToken !== runToken) return;
      injectError(i18n.t('planMode:orchestrator.planFailed', 'Plan Generation Failed'), error || 'No plan produced');
      set({ isBusy: false, phase: 'failed' });
      return;
    }

    // Inject plan card (interactive for review)
    if (get()._runToken !== runToken) return;
    set({ editablePlan: plan, phase: 'reviewing_plan', isBusy: false });
    injectCard('plan_card', { ...plan, editable: true } as unknown as Record<string, unknown>, true);
  },

  approvePlan: async (plan: PlanCardData) => {
    const runToken = get()._runToken;
    const effectiveSessionId = resolvePlanSessionId(get, set);
    if (!effectiveSessionId) {
      injectError(i18n.t('planMode:orchestrator.approveFailed', 'Failed to start plan execution'), 'No active session');
      return;
    }
    set({ phase: 'executing', isBusy: true, isCancelling: false, _completionCardInjectedRunToken: null });

    injectInfo(i18n.t('planMode:orchestrator.planApproved', 'Plan approved! Starting execution...'));

    injectCard('plan_persona_indicator', {
      role: 'executor',
      displayName: i18n.t('planMode:personas.executor', 'Executor'),
      phase: 'executing',
    } satisfies PlanPersonaIndicatorData);

    const planStore = usePlanModeStore.getState();
    const settings = useSettingsStore.getState();
    const projectPath = settings.workspacePath || undefined;
    const contextSources = buildPlanContextSources(effectiveSessionId);
    const conversationHistory = buildConversationHistory();
    const contextStr =
      conversationHistory.length > 0
        ? conversationHistory.map((t) => `user: ${t.user}\nassistant: ${t.assistant}`).join('\n')
        : undefined;
    await startPlanExecutionWithProgress({
      runToken,
      plan,
      rollbackPhase: 'reviewing_plan',
      rollbackReasonCode: 'plan_approval_failed',
      startErrorTitle: i18n.t('planMode:orchestrator.approveFailed', 'Failed to start plan execution'),
      invokeExecution: () =>
        (async () => {
          const ok = await planStore.approvePlan(
            plan,
            undefined,
            undefined,
            undefined,
            projectPath,
            contextSources,
            contextStr,
            i18n.language,
            effectiveSessionId,
          );
          if (!ok) {
            throw new Error(usePlanModeStore.getState().error || 'Failed to approve plan');
          }
        })(),
      get,
      set,
    });
  },

  retryStep: async (stepId: string) => {
    const normalizedStepId = stepId.trim();
    if (!normalizedStepId) return;
    if (get()._progressUnlisten || get().phase === 'executing') return;

    const plan = get().editablePlan;
    if (!plan) {
      injectError(i18n.t('planMode:orchestrator.retryFailed', 'Failed to retry plan step'), 'No active plan found');
      return;
    }

    const previousPhase = get().phase;
    const effectiveSessionId = resolvePlanSessionId(get, set);
    if (!effectiveSessionId) {
      injectError(i18n.t('planMode:orchestrator.retryFailed', 'Failed to retry plan step'), 'No active session found');
      return;
    }

    const runToken = get()._runToken + 1;
    set({
      sessionId: effectiveSessionId,
      editablePlan: plan,
      phase: 'executing',
      isBusy: true,
      isCancelling: false,
      _runToken: runToken,
      _completionCardInjectedRunToken: null,
    });

    injectInfo(i18n.t('planMode:orchestrator.retryingStep', 'Retrying step {{id}}...', { id: normalizedStepId }));
    injectCard('plan_persona_indicator', {
      role: 'executor',
      displayName: i18n.t('planMode:personas.executor', 'Executor'),
      phase: 'executing',
    } satisfies PlanPersonaIndicatorData);

    // Best-effort kernel intent trace for retry action.
    const transitionAndSubmitInput = useWorkflowKernelStore.getState().transitionAndSubmitInput;
    void transitionAndSubmitInput(
      'plan',
      createWorkflowKernelActionIntent({
        mode: 'plan',
        type: 'execution_control',
        source: 'plan_orchestrator',
        action: 'retry_step',
        content: `retry_step:${normalizedStepId}`,
        metadata: {
          stepId: normalizedStepId,
        },
      }),
    ).catch(() => {
      // best effort kernel intent recording
    });

    const planStore = usePlanModeStore.getState();
    const settings = useSettingsStore.getState();
    const projectPath = settings.workspacePath || undefined;
    const contextSources = buildPlanContextSources(effectiveSessionId);
    const conversationHistory = buildConversationHistory();
    const contextStr =
      conversationHistory.length > 0
        ? conversationHistory.map((t) => `user: ${t.user}\nassistant: ${t.assistant}`).join('\n')
        : undefined;

    await startPlanExecutionWithProgress({
      runToken,
      plan,
      rollbackPhase: previousPhase === 'executing' ? 'failed' : previousPhase,
      rollbackReasonCode: 'plan_retry_failed',
      startErrorTitle: i18n.t('planMode:orchestrator.retryFailed', 'Failed to retry plan step'),
      invokeExecution: () =>
        (async () => {
          const ok = await planStore.retryPlanStep(
            normalizedStepId,
            undefined,
            undefined,
            undefined,
            projectPath,
            contextSources,
            contextStr,
            i18n.language,
            effectiveSessionId,
          );
          if (!ok) {
            throw new Error(usePlanModeStore.getState().error || 'Failed to retry step');
          }
        })(),
      get,
      set,
    });
  },

  cancelWorkflow: async () => {
    const { phase, sessionId, _progressUnlisten, _runToken } = get();
    const effectiveSessionId = resolvePlanSessionId(get, set);
    if (!sessionId && effectiveSessionId) {
      set({ sessionId: effectiveSessionId });
    }

    if (phase === 'executing' && effectiveSessionId) {
      if (get().isCancelling) return;
      set({ isCancelling: true, isBusy: true });
      const planStore = usePlanModeStore.getState();
      const cancelled = await planStore.cancelExecution(effectiveSessionId);
      const planError = usePlanModeStore.getState().error;
      if (!cancelled && planError) {
        set({ isCancelling: false, isBusy: false });
        injectError(i18n.t('planMode:orchestrator.cancelFailed', 'Cancel Failed'), planError);
        throw new Error(planError);
      }
      injectInfo(i18n.t('planMode:orchestrator.cancelling', 'Cancelling plan execution...'));
      return;
    } else {
      const nextRunToken = _runToken + 1;
      set({ _runToken: nextRunToken, isCancelling: false });
      const planStore = usePlanModeStore.getState();
      await planStore.cancelOperation(effectiveSessionId);

      if (_progressUnlisten) {
        _progressUnlisten();
      }

      await planStore.exitPlanMode(effectiveSessionId);
      buildPlanContextSources(null);
      set({ ...DEFAULT_STATE, _runToken: nextRunToken });
      injectInfo(i18n.t('planMode:orchestrator.cancelled', 'Plan mode cancelled.'));
    }
  },

  resetWorkflow: () => {
    const { _progressUnlisten } = get();
    if (_progressUnlisten) _progressUnlisten();
    buildPlanContextSources(null);
    set((state) => ({ ...DEFAULT_STATE, _runToken: state._runToken + 1 }));
  },
}));
