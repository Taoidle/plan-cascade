import i18n from '../../i18n';
import { usePlanModeStore } from '../planMode';
import { useWorkflowKernelStore } from '../workflowKernel';
import { useSettingsStore } from '../settings';
import { createWorkflowKernelActionIntent } from '../../lib/workflowKernelIntent';
import { failResult, type ActionResult } from '../../types/actionResult';
import type { ContextSourceConfig } from '../../types/contextSources';
import type { PlanCardData, PlanModePhase, PlanPersonaIndicatorData } from '../../types/planModeCard';
import type { PlanOrchestratorState } from '../planOrchestrator';
import {
  injectPlanCard as injectCard,
  injectPlanError as injectError,
  injectPlanInfo as injectInfo,
} from './cardInjection';

type PlanGet = () => PlanOrchestratorState;
type PlanSet = (
  partial: Partial<PlanOrchestratorState> | ((state: PlanOrchestratorState) => Partial<PlanOrchestratorState>),
) => void;

interface ExecutionFlowDeps {
  get: PlanGet;
  set: PlanSet;
  resolvePlanSessionId: (get: PlanGet, set: PlanSet) => string | null;
  buildPlanContextSources: (sessionId?: string | null) => ContextSourceConfig | undefined;
  startPlanExecutionWithProgress: (input: {
    runToken: number;
    plan: PlanCardData;
    rollbackPhase: PlanModePhase;
    startErrorTitle: string;
    invokeExecution: () => Promise<void>;
    get: PlanGet;
    set: PlanSet;
  }) => Promise<ActionResult>;
  defaultState: Partial<PlanOrchestratorState>;
}

export async function approvePlanFlow(plan: PlanCardData, deps: ExecutionFlowDeps): Promise<ActionResult> {
  const { get, set, resolvePlanSessionId, buildPlanContextSources, startPlanExecutionWithProgress } = deps;
  const runToken = get()._runToken;
  const effectiveSessionId = resolvePlanSessionId(get, set);
  if (!effectiveSessionId) {
    injectError(i18n.t('planMode:orchestrator.approveFailed', 'Failed to start plan execution'), 'No active session');
    return failResult('missing_session', 'No active session');
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
  return startPlanExecutionWithProgress({
    runToken,
    plan,
    rollbackPhase: 'reviewing_plan',
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
          undefined,
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
}

export async function retryStepFlow(stepId: string, deps: ExecutionFlowDeps): Promise<void> {
  const { get, set, resolvePlanSessionId, buildPlanContextSources, startPlanExecutionWithProgress } = deps;
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
  await startPlanExecutionWithProgress({
    runToken,
    plan,
    rollbackPhase: previousPhase === 'executing' ? 'failed' : previousPhase,
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
          undefined,
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
}

export async function cancelWorkflowFlow(deps: ExecutionFlowDeps): Promise<void> {
  const { get, set, resolvePlanSessionId, buildPlanContextSources, defaultState } = deps;
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
  }

  const nextRunToken = _runToken + 1;
  set({ _runToken: nextRunToken, isCancelling: false });
  const planStore = usePlanModeStore.getState();
  await planStore.cancelOperation(effectiveSessionId);

  if (_progressUnlisten) {
    _progressUnlisten();
  }

  await planStore.exitPlanMode(effectiveSessionId);
  buildPlanContextSources(null);
  set({ ...defaultState, _runToken: nextRunToken });
  injectInfo(i18n.t('planMode:orchestrator.cancelled', 'Plan mode cancelled.'));
}
