/**
 * Plan Mode Orchestrator Store
 *
 * Orchestrates the Plan Mode workflow lifecycle in Simple mode.
 * Drives the state machine: idle → analyzing → [clarifying] → planning
 * → reviewing_plan → executing → completed/failed.
 *
 * Delegates to:
 * - usePlanModeStore: backend session lifecycle
 * - useExecutionStore: appendStreamLine for card injection into chat transcript
 */

import { create } from 'zustand';
import i18n from '../i18n';
import { useExecutionStore } from './execution';
import { usePlanModeStore } from './planMode';
import { useSettingsStore } from './settings';
import { buildConversationHistory } from '../lib/contextBridge';
import type { CardPayload } from '../types/workflowCard';
import type {
  PlanModePhase,
  PlanAnalysisCardData,
  PlanCardData,
  PlanClarifyAnswerCardData,
  PlanClarifyQuestionCardData,
  PlanStepUpdateCardData,
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

  /** Unlisten function for progress events */
  _progressUnlisten: UnlistenFn | null;

  // Actions
  startPlanWorkflow: (description: string) => Promise<void>;
  submitClarification: (answer: PlanClarifyAnswerCardData) => Promise<void>;
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
  useExecutionStore.getState().appendStreamLine(JSON.stringify(payload), 'card');
}

function injectInfo(message: string) {
  injectCard('workflow_info', { message, level: 'info' });
}

function injectError(title: string, description: string) {
  injectCard('workflow_error', { title, description, suggestedFix: '' });
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
  _progressUnlisten: null as UnlistenFn | null,
};

// ============================================================================
// Store
// ============================================================================

export const usePlanOrchestratorStore = create<PlanOrchestratorState>((set, get) => ({
  ...DEFAULT_STATE,

  startPlanWorkflow: async (description: string) => {
    const planStore = usePlanModeStore.getState();
    const settings = useSettingsStore.getState();
    const { resolveProviderBaseUrl } = await import('../lib/providers');

    set({ isBusy: true, taskDescription: description, phase: 'analyzing' });

    // Add user message to chat transcript so it appears as a chat bubble
    useExecutionStore.getState().appendStreamLine(description, 'info');

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

    // Enter plan mode (runs analysis)
    await planStore.enterPlanMode(description, settings.provider, settings.model, baseUrl, contextStr, i18n.language);

    const { analysis, sessionPhase, sessionId, error } = usePlanModeStore.getState();

    if (error) {
      injectError(i18n.t('planMode:orchestrator.analysisFailed', 'Analysis Failed'), error);
      set({ isBusy: false, phase: 'failed' });
      return;
    }

    set({ sessionId, analysis });

    // Inject analysis card
    if (analysis) {
      injectCard('plan_analysis_card', analysis as unknown as Record<string, unknown>);
    }

    // Proceed based on phase
    if (sessionPhase === 'clarifying') {
      const { currentQuestion } = usePlanModeStore.getState();

      set({ phase: 'clarifying', isBusy: false, pendingClarifyQuestion: currentQuestion ?? null });

      injectCard('plan_persona_indicator', {
        role: 'analyst',
        displayName: i18n.t('planMode:personas.analyst', 'Analyst'),
        phase: 'clarifying',
      } satisfies PlanPersonaIndicatorData);
      injectInfo(
        i18n.t('planMode:orchestrator.needsClarification', 'Some details need clarification before planning.'),
      );

      // Inject first question card if available
      if (currentQuestion) {
        injectCard('plan_clarify_question', currentQuestion as unknown as Record<string, unknown>);
      }
    } else {
      // Skip to planning
      await get().proceedToPlanning();
    }
  },

  submitClarification: async (answer: PlanClarifyAnswerCardData) => {
    // Inject answer card immediately
    injectCard('plan_clarify_answer', answer as unknown as Record<string, unknown>);

    set({ pendingClarifyQuestion: null, isBusy: true });
    injectInfo(i18n.t('planMode:orchestrator.generatingQuestion', 'Generating next question...'));

    const planStore = usePlanModeStore.getState();
    const updatedSession = await planStore.submitClarification(answer, undefined, undefined, undefined, i18n.language);

    if (!updatedSession) {
      // Submission failed — fallback to planning
      injectInfo(i18n.t('planMode:orchestrator.clarificationFailed', 'Clarification failed, proceeding to planning.'));
      set({ isBusy: false });
      await get().proceedToPlanning();
      return;
    }

    if (updatedSession.phase === 'planning') {
      // Clarification complete — transition to planning
      injectInfo(i18n.t('planMode:orchestrator.clarificationComplete', 'Clarification complete.'));
      set({ pendingClarifyQuestion: null, isBusy: false });
      await get().proceedToPlanning();
    } else if (updatedSession.currentQuestion) {
      // Next question available
      set({ pendingClarifyQuestion: updatedSession.currentQuestion, isBusy: false });
      injectCard('plan_clarify_question', updatedSession.currentQuestion as unknown as Record<string, unknown>);
    } else {
      // No question and still clarifying — fallback to planning
      set({ pendingClarifyQuestion: null, isBusy: false });
      await get().proceedToPlanning();
    }
  },

  skipClarification: async () => {
    set({ pendingClarifyQuestion: null });
    injectInfo(i18n.t('planMode:orchestrator.clarificationSkipped', 'Clarification skipped.'));
    const planStore = usePlanModeStore.getState();
    await planStore.skipClarification();
    await get().proceedToPlanning();
  },

  proceedToPlanning: async () => {
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

    const planStore = usePlanModeStore.getState();
    await planStore.generatePlan(undefined, undefined, undefined, contextStr, i18n.language);

    const { plan, error } = usePlanModeStore.getState();

    if (error || !plan) {
      injectError(i18n.t('planMode:orchestrator.planFailed', 'Plan Generation Failed'), error || 'No plan produced');
      set({ isBusy: false, phase: 'failed' });
      return;
    }

    // Inject plan card (interactive for review)
    set({ editablePlan: plan, phase: 'reviewing_plan', isBusy: false });
    injectCard('plan_card', { ...plan, editable: true } as unknown as Record<string, unknown>, true);
  },

  approvePlan: async (plan: PlanCardData) => {
    set({ phase: 'executing', isBusy: true });

    injectInfo(i18n.t('planMode:orchestrator.planApproved', 'Plan approved! Starting execution...'));

    injectCard('plan_persona_indicator', {
      role: 'executor',
      displayName: i18n.t('planMode:personas.executor', 'Executor'),
      phase: 'executing',
    } satisfies PlanPersonaIndicatorData);

    // Subscribe to progress events
    const unlisten = await listen<PlanModeProgressPayload>('plan-mode-progress', (event) => {
      const payload = event.payload;
      const { sessionId } = get();
      if (payload.sessionId !== sessionId) return;

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

      // On completion, inject completion card
      if (payload.eventType === 'execution_completed') {
        const planModeState = usePlanModeStore.getState();
        planModeState.fetchReport().then(() => {
          const { report } = usePlanModeStore.getState();
          if (report) {
            injectCard('plan_completion_card', {
              success: report.success,
              planTitle: report.planTitle,
              totalSteps: report.totalSteps,
              stepsCompleted: report.stepsCompleted,
              stepsFailed: report.stepsFailed,
              totalDurationMs: report.totalDurationMs,
              stepSummaries: report.stepSummaries,
            } satisfies PlanCompletionCardData);
          }
          set({ phase: report?.success ? 'completed' : 'failed', isBusy: false });
        });
      } else if (payload.eventType === 'execution_cancelled') {
        set({ phase: 'cancelled', isBusy: false });
      }
    });

    set({ _progressUnlisten: unlisten });

    // Trigger approval
    const planStore = usePlanModeStore.getState();
    await planStore.approvePlan(plan, undefined, undefined, undefined, i18n.language);
  },

  cancelWorkflow: async () => {
    const { phase, _progressUnlisten } = get();

    if (phase === 'executing') {
      const planStore = usePlanModeStore.getState();
      await planStore.cancelExecution();
    }

    if (_progressUnlisten) {
      _progressUnlisten();
    }

    const planStore = usePlanModeStore.getState();
    await planStore.exitPlanMode();

    set({ ...DEFAULT_STATE });
    injectInfo(i18n.t('planMode:orchestrator.cancelled', 'Plan mode cancelled.'));
  },

  resetWorkflow: () => {
    const { _progressUnlisten } = get();
    if (_progressUnlisten) _progressUnlisten();
    set({ ...DEFAULT_STATE });
  },
}));
