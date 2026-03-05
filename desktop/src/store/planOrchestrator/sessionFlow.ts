import i18n from '../../i18n';
import { useExecutionStore } from '../execution';
import { usePlanModeStore } from '../planMode';
import { useWorkflowKernelStore } from '../workflowKernel';
import { useSettingsStore } from '../settings';
import { selectKernelPlanRuntime } from '../workflowKernelSelectors';
import { buildConversationHistory } from '../../lib/contextBridge';
import { getNextTurnId } from '../../lib/conversationUtils';
import { failResult, type ActionResult } from '../../types/actionResult';
import type { ContextSourceConfig } from '../../types/contextSources';
import type {
  PlanModePhase,
  PlanClarifyAnswerCardData,
  PlanClarifyQuestionCardData,
  PlanPersonaIndicatorData,
} from '../../types/planModeCard';
import type { PlanOrchestratorState } from '../planOrchestrator';
import {
  injectClarificationResolutionCard,
  injectPlanCard as injectCard,
  injectPlanError as injectError,
  injectPlanInfo as injectInfo,
} from './cardInjection';

type PlanGet = () => PlanOrchestratorState;
type PlanSet = (
  partial: Partial<PlanOrchestratorState> | ((state: PlanOrchestratorState) => Partial<PlanOrchestratorState>),
) => void;

interface SessionFlowDeps {
  get: PlanGet;
  set: PlanSet;
  buildPlanContextSources: (sessionId?: string | null) => ContextSourceConfig | undefined;
  resolvePlanSessionId: (get: PlanGet, set: PlanSet) => string | null;
  normalizeKernelPlanPhase: (phase: string | null | undefined) => PlanModePhase;
  syncKernelPlanPhase: (phase: PlanModePhase, reasonCode?: string) => Promise<void>;
}

export async function startPlanWorkflowFlow(
  description: string,
  deps: SessionFlowDeps,
): Promise<{ modeSessionId: string | null }> {
  const { get, set, buildPlanContextSources, normalizeKernelPlanPhase, syncKernelPlanPhase } = deps;
  const runToken = get()._runToken + 1;
  let modeSessionId: string | null = null;
  const planStore = usePlanModeStore.getState();
  const settings = useSettingsStore.getState();
  const { resolveProviderBaseUrl } = await import('../../lib/providers');
  const { resolvePhaseAgent } = await import('../../lib/phaseAgentResolver');

  set({ isBusy: true, isCancelling: false, taskDescription: description, phase: 'analyzing', _runToken: runToken });

  const executionState = useExecutionStore.getState();
  const turnId = getNextTurnId(executionState.streamingOutput);
  executionState.appendStreamLine(description, 'info', undefined, undefined, { turnId, turnBoundary: 'user' });

  injectCard('plan_persona_indicator', {
    role: 'planner',
    displayName: i18n.t('planMode:personas.planner', 'Planner'),
    phase: 'analyzing',
  } satisfies PlanPersonaIndicatorData);

  injectInfo(i18n.t('planMode:orchestrator.analyzingTask', 'Analyzing task...'));

  const conversationHistory = buildConversationHistory();
  const contextStr =
    conversationHistory.length > 0
      ? conversationHistory.map((t) => `user: ${t.user}\nassistant: ${t.assistant}`).join('\n')
      : undefined;

  const strategyAgent = resolvePhaseAgent('plan_strategy');
  const provider = strategyAgent.provider || settings.provider;
  const model = strategyAgent.model || settings.model;
  const baseUrl = strategyAgent.baseUrl ?? (provider ? resolveProviderBaseUrl(provider, settings) : undefined);
  const projectPath = settings.workspacePath || undefined;
  const contextSources = buildPlanContextSources(null);

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

  if (analysis) {
    injectCard('plan_analysis_card', analysis as unknown as Record<string, unknown>);
  }

  if (enteredPhase === 'clarifying') {
    if (get()._runToken !== runToken) return { modeSessionId: modeSessionId ?? null };
    set({ phase: 'clarifying', isBusy: false, pendingClarifyQuestion: currentQuestion ?? null });

    injectCard('plan_persona_indicator', {
      role: 'analyst',
      displayName: i18n.t('planMode:personas.analyst', 'Analyst'),
      phase: 'clarifying',
    } satisfies PlanPersonaIndicatorData);
    injectInfo(i18n.t('planMode:orchestrator.needsClarification', 'Some details need clarification before planning.'));

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
    if (get()._runToken !== runToken) return { modeSessionId: modeSessionId ?? null };
    await get().proceedToPlanning();
  }
  return { modeSessionId: modeSessionId ?? null };
}

export async function submitClarificationFlow(
  answer: PlanClarifyAnswerCardData,
  deps: SessionFlowDeps,
): Promise<{ ok: boolean; errorCode?: string | null }> {
  const { get, set, resolvePlanSessionId, buildPlanContextSources, syncKernelPlanPhase } = deps;
  const runToken = get()._runToken;
  const effectiveSessionId = resolvePlanSessionId(get, set);
  if (!effectiveSessionId) {
    injectError(i18n.t('planMode:orchestrator.clarificationFailed', 'Clarification failed.'), 'No active session');
    set({ isBusy: false, pendingClarifyQuestion: null });
    return { ok: false, errorCode: 'missing_session' };
  }

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
    injectInfo(i18n.t('planMode:orchestrator.clarificationComplete', 'Clarification complete.'));
    set({ pendingClarifyQuestion: null, isBusy: false });
    if (get()._runToken !== runToken) return { ok: false, errorCode: 'stale_run_token' };
    await get().proceedToPlanning();
    return { ok: true };
  }

  if (updatedSession.currentQuestion) {
    if (get()._runToken !== runToken) return { ok: false, errorCode: 'stale_run_token' };
    set({ pendingClarifyQuestion: updatedSession.currentQuestion, isBusy: false });
    injectCard('plan_clarify_question', updatedSession.currentQuestion as unknown as Record<string, unknown>);
    return { ok: true };
  }

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

export async function retryClarificationFlow(deps: SessionFlowDeps): Promise<void> {
  const { get, set, syncKernelPlanPhase } = deps;
  const runToken = get()._runToken;
  const description = get().taskDescription.trim();
  if (!description) return;

  injectInfo(i18n.t('planMode:orchestrator.retryClarification', 'Retrying clarification...'));
  set({ phase: 'analyzing', isBusy: true, pendingClarifyQuestion: null });
  await syncKernelPlanPhase('analyzing', 'clarification_retry');

  if (get()._runToken !== runToken) return;
  await get().startPlanWorkflow(description);
}

export async function skipClarificationFlow(deps: SessionFlowDeps): Promise<void> {
  const { get, set, resolvePlanSessionId } = deps;
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
}

export async function proceedToPlanningFlow(deps: SessionFlowDeps): Promise<void> {
  const { get, set, resolvePlanSessionId, buildPlanContextSources } = deps;
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

  if (get()._runToken !== runToken) return;
  set({ editablePlan: plan, phase: 'reviewing_plan', isBusy: false });
  injectCard('plan_card', { ...plan, editable: true } as unknown as Record<string, unknown>, true);
}

export function mapSessionFlowError(error: unknown): ActionResult {
  const msg = error instanceof Error ? error.message : String(error);
  return failResult('config_confirm_failed', msg);
}
