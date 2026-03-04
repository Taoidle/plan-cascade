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
import { buildConversationHistory } from '../lib/contextBridge';
import { getNextTurnId } from '../lib/conversationUtils';
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
  startPlanWorkflow: (description: string) => Promise<void>;
  submitClarification: (answer: PlanClarifyAnswerCardData) => Promise<{
    ok: boolean;
    errorCode?: string | null;
  }>;
  retryClarification: () => Promise<void>;
  skipClarification: () => Promise<void>;
  proceedToPlanning: () => Promise<void>;
  approvePlan: (plan: PlanCardData) => Promise<void>;
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

function buildPlanCompletionCardDataFromReport(
  report: NonNullable<ReturnType<typeof usePlanModeStore.getState>['report']>,
  success: boolean,
): PlanCompletionCardData {
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
    const planStore = usePlanModeStore.getState();
    const settings = useSettingsStore.getState();
    const { resolveProviderBaseUrl } = await import('../lib/providers');

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

    // Resolve provider base URL (handles multi-endpoint providers like Qwen, GLM, MiniMax)
    const baseUrl = settings.provider ? resolveProviderBaseUrl(settings.provider, settings) : undefined;
    const projectPath = settings.workspacePath || undefined;
    const contextSources = buildPlanContextSources(null);

    // Enter plan mode (runs analysis)
    const enteredSession = await planStore.enterPlanMode(
      description,
      settings.provider,
      settings.model,
      baseUrl,
      projectPath,
      contextSources,
      contextStr,
      i18n.language,
    );
    if (get()._runToken !== runToken) return;

    const planStoreState = usePlanModeStore.getState();
    const fallbackSession =
      !enteredSession && planStoreState.sessionId
        ? {
            sessionId: planStoreState.sessionId,
            phase: planStoreState.sessionPhase,
            analysis: planStoreState.analysis,
            currentQuestion: planStoreState.currentQuestion,
          }
        : null;
    const activeSession = enteredSession ?? fallbackSession;
    const { error } = planStoreState;

    if (error || !activeSession) {
      if (get()._runToken !== runToken) return;
      injectError(
        i18n.t('planMode:orchestrator.analysisFailed', 'Analysis Failed'),
        error || 'Failed to enter plan mode',
      );
      set({ isBusy: false, phase: 'failed' });
      return;
    }

    const { analysis, phase: enteredPhase, sessionId, currentQuestion } = activeSession;

    if (get()._runToken !== runToken) return;
    set({ sessionId, analysis });
    buildPlanContextSources(sessionId);

    // Inject analysis card
    if (analysis) {
      injectCard('plan_analysis_card', analysis as unknown as Record<string, unknown>);
    }

    // Proceed based on phase
    if (enteredPhase === 'clarifying') {
      if (get()._runToken !== runToken) return;
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
      if (get()._runToken !== runToken) return;
      await get().proceedToPlanning();
    }
  },

  submitClarification: async (answer: PlanClarifyAnswerCardData) => {
    const runToken = get()._runToken;
    // Inject answer card immediately
    injectCard('plan_clarify_answer', answer as unknown as Record<string, unknown>);

    set({ pendingClarifyQuestion: null, isBusy: true });
    injectInfo(i18n.t('planMode:orchestrator.generatingQuestion', 'Generating next question...'));

    const planStore = usePlanModeStore.getState();
    const settings = useSettingsStore.getState();
    const projectPath = settings.workspacePath || undefined;
    const contextSources = buildPlanContextSources(get().sessionId);
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
    set({ pendingClarifyQuestion: null });
    injectInfo(i18n.t('planMode:orchestrator.clarificationSkipped', 'Clarification skipped.'));
    const planStore = usePlanModeStore.getState();
    await planStore.skipClarification();
    if (get()._runToken !== runToken) return;
    await get().proceedToPlanning();
  },

  proceedToPlanning: async () => {
    const runToken = get()._runToken;
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
    const contextSources = buildPlanContextSources(get().sessionId);

    const planStore = usePlanModeStore.getState();
    await planStore.generatePlan(
      undefined,
      undefined,
      undefined,
      projectPath,
      contextSources,
      contextStr,
      i18n.language,
    );
    if (get()._runToken !== runToken) return;

    const { plan, error } = usePlanModeStore.getState();

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
    set({ phase: 'executing', isBusy: true, isCancelling: false });

    injectInfo(i18n.t('planMode:orchestrator.planApproved', 'Plan approved! Starting execution...'));

    injectCard('plan_persona_indicator', {
      role: 'executor',
      displayName: i18n.t('planMode:personas.executor', 'Executor'),
      phase: 'executing',
    } satisfies PlanPersonaIndicatorData);

    // Subscribe to progress events
    const unlisten = await listen<PlanModeProgressPayload>('plan-mode-progress', (event) => {
      if (get()._runToken !== runToken) return;
      const payload = event.payload;
      const { sessionId } = get();
      if (payload.sessionId !== sessionId) return;

      const planModeState = usePlanModeStore.getState();
      const planModePatch: {
        currentBatch: number;
        totalBatches: number;
        stepStatuses?: Record<string, string>;
        error?: string | null;
        isCancelling?: boolean;
      } = {
        currentBatch: payload.currentBatch,
        totalBatches: payload.totalBatches,
      };
      if (payload.stepId && payload.stepStatus) {
        planModePatch.stepStatuses = {
          ...planModeState.stepStatuses,
          [payload.stepId]: payload.stepStatus,
        };
      }

      // Inject step update cards
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
          .fetchStepOutput(completedStepId)
          .then((stepOutput) => {
            if (get()._runToken !== runToken) return;
            if (!stepOutput) {
              const latestError = usePlanModeStore.getState().error;
              if (latestError) {
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

      // On completion, inject completion card
      if (payload.eventType === 'execution_completed') {
        const mergedStepStatuses = {
          ...planModeState.stepStatuses,
          ...(planModePatch.stepStatuses ?? {}),
        };
        const hasFailedSteps = Object.values(mergedStepStatuses).some((status) => status === 'failed');
        const terminalPhase: PlanModePhase = hasFailedSteps ? 'failed' : 'completed';

        planModePatch.isCancelling = false;
        usePlanModeStore.setState(planModePatch);
        set({
          phase: terminalPhase,
          isBusy: false,
          isCancelling: false,
          _progressUnlisten: null,
        });
        unlisten();

        void (async () => {
          try {
            await planModeState.fetchReport();
          } catch {
            // best-effort report fetch; fallback card is injected below if unavailable.
          }
          if (get()._runToken !== runToken) return;
          if (get()._completionCardInjectedRunToken === runToken) return;

          const latestReport = usePlanModeStore.getState().report;
          const report = latestReport && latestReport.sessionId === payload.sessionId ? latestReport : null;
          const completionData = report
            ? buildPlanCompletionCardDataFromReport(report, terminalPhase === 'completed')
            : buildPlanCompletionCardDataFallback(plan, mergedStepStatuses, terminalPhase === 'completed');

          injectCard('plan_completion_card', completionData as unknown as Record<string, unknown>);
          set({ _completionCardInjectedRunToken: runToken });
        })();
      } else if (payload.eventType === 'execution_cancelled') {
        if (get()._runToken !== runToken) return;
        planModePatch.isCancelling = false;
        usePlanModeStore.setState(planModePatch);
        set({ phase: 'cancelled', isBusy: false, isCancelling: false, _progressUnlisten: null });
        unlisten();
      } else {
        if (payload.eventType === 'step_failed' && payload.error) {
          planModePatch.error = payload.error;
        }
        usePlanModeStore.setState(planModePatch);
      }
    });

    if (get()._runToken !== runToken) {
      unlisten();
      return;
    }
    set({ _progressUnlisten: unlisten });

    // Trigger approval
    const planStore = usePlanModeStore.getState();
    const settings = useSettingsStore.getState();
    const projectPath = settings.workspacePath || undefined;
    const contextSources = buildPlanContextSources(get().sessionId);
    const conversationHistory = buildConversationHistory();
    const contextStr =
      conversationHistory.length > 0
        ? conversationHistory.map((t) => `user: ${t.user}\nassistant: ${t.assistant}`).join('\n')
        : undefined;
    try {
      await planStore.approvePlan(
        plan,
        undefined,
        undefined,
        undefined,
        projectPath,
        contextSources,
        contextStr,
        i18n.language,
      );
    } catch (error) {
      if (get()._progressUnlisten) {
        get()._progressUnlisten?.();
      } else {
        unlisten();
      }
      set({ _progressUnlisten: null, phase: 'reviewing_plan', isBusy: false, isCancelling: false });
      injectError(
        i18n.t('planMode:orchestrator.approveFailed', 'Failed to start plan execution'),
        error instanceof Error ? error.message : String(error),
      );
      return;
    }

    if (get()._runToken !== runToken) {
      if (get()._progressUnlisten) {
        get()._progressUnlisten?.();
      } else {
        unlisten();
      }
      set({ _progressUnlisten: null });
      return;
    }

    const latestPlanState = usePlanModeStore.getState();
    if (latestPlanState.error) {
      if (get()._progressUnlisten) {
        get()._progressUnlisten?.();
      } else {
        unlisten();
      }
      set({ _progressUnlisten: null, phase: 'reviewing_plan', isBusy: false, isCancelling: false });
      injectError(
        i18n.t('planMode:orchestrator.approveFailed', 'Failed to start plan execution'),
        latestPlanState.error,
      );
    }
  },

  cancelWorkflow: async () => {
    const { phase, sessionId, _progressUnlisten, _runToken } = get();
    const linkedSessionId = useWorkflowKernelStore.getState().session?.linkedModeSessions?.plan ?? null;
    const effectiveSessionId = sessionId || linkedSessionId || usePlanModeStore.getState().sessionId || null;
    if (!sessionId && effectiveSessionId) {
      set({ sessionId: effectiveSessionId });
      usePlanModeStore.setState({ sessionId: effectiveSessionId, isPlanMode: true });
    }

    if (phase === 'executing' && effectiveSessionId) {
      if (get().isCancelling) return;
      set({ isCancelling: true, isBusy: true });
      const planStore = usePlanModeStore.getState();
      await planStore.cancelExecution();
      const planState = usePlanModeStore.getState();
      if (planState.error) {
        set({ isCancelling: false, isBusy: false });
        injectError(i18n.t('planMode:orchestrator.cancelFailed', 'Cancel Failed'), planState.error);
        throw new Error(planState.error);
      }
      injectInfo(i18n.t('planMode:orchestrator.cancelling', 'Cancelling plan execution...'));
      return;
    } else {
      const nextRunToken = _runToken + 1;
      set({ _runToken: nextRunToken, isCancelling: false });
      const planStore = usePlanModeStore.getState();
      await planStore.cancelOperation();

      if (_progressUnlisten) {
        _progressUnlisten();
      }

      await planStore.exitPlanMode();
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
