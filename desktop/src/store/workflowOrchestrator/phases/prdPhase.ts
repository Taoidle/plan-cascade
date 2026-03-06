import i18n from '../../../i18n';
import { resolvePersonaDisplayName } from '../../../lib/personaI18n';
import type { ActionResult } from '../../../types/actionResult';
import type { PrdCardData } from '../../../types/workflowCard';
import { useSettingsStore } from '../../settings';
import type { TaskPrd } from '../../taskMode';
import { useTaskModeStore } from '../../taskMode';
import {
  injectWorkflowCard as injectCard,
  injectWorkflowError as injectError,
  injectWorkflowInfo as injectInfo,
} from '../cardInjection';
import type { WorkflowPhaseRuntime } from './runtime';

interface PrdPhaseDeps {
  toPrdCardData: (prd: TaskPrd) => PrdCardData;
  synthesizePlanningTurn: (taskDescription: string, strategyAnalysis: unknown, prd: TaskPrd) => void;
}

export async function runPrdPhase(runtime: WorkflowPhaseRuntime, deps: PrdPhaseDeps): Promise<ActionResult> {
  const { set, get, runToken, isRunActive, resolveTaskSessionId } = runtime;
  if (!isRunActive(get, runToken)) return { ok: false, errorCode: 'stale_run_token', message: 'Stale run token' };

  const effectiveSessionId = resolveTaskSessionId(get, set);
  if (!effectiveSessionId) {
    set({ phase: 'failed', error: 'No active task session' });
    injectError(
      i18n.t('workflow.orchestrator.prdGenerationFailed', { ns: 'simpleMode' }),
      i18n.t('workflow.orchestrator.sessionMissing', {
        ns: 'simpleMode',
        defaultValue: 'No active task session found.',
      }),
    );
    return { ok: false, errorCode: 'session_missing', message: 'No active task session' };
  }

  set({ phase: 'generating_prd' });

  const { resolvePhaseAgent, formatModelDisplay } = await import('../../../lib/phaseAgentResolver');
  if (!isRunActive(get, runToken)) return { ok: false, errorCode: 'stale_run_token', message: 'Stale run token' };
  const prdResolved = resolvePhaseAgent('plan_prd');

  injectCard('persona_indicator', {
    role: 'TechLead',
    displayName: resolvePersonaDisplayName(i18n.t.bind(i18n), 'TechLead'),
    phase: 'generating_prd',
    model: formatModelDisplay(prdResolved),
  });
  injectInfo(i18n.t('workflow.orchestrator.generatingPrd', { ns: 'simpleMode' }), 'info');

  try {
    const state = get() as { taskDescription: string; strategyAnalysis: unknown };
    const settings = useSettingsStore.getState();
    const maxContextTokens = settings.maxTotalTokens ?? 200_000;
    const prd = await useTaskModeStore
      .getState()
      .generatePrd(
        undefined,
        maxContextTokens,
        prdResolved.provider || undefined,
        prdResolved.model || undefined,
        prdResolved.baseUrl,
        effectiveSessionId,
      );
    if (!isRunActive(get, runToken)) return { ok: false, errorCode: 'stale_run_token', message: 'Stale run token' };

    const taskModeError = useTaskModeStore.getState().error;
    if (taskModeError) {
      set({ phase: 'failed', error: taskModeError });
      injectError(i18n.t('workflow.orchestrator.prdGenerationFailed', { ns: 'simpleMode' }), taskModeError);
      return { ok: false, errorCode: 'prd_generation_failed', message: taskModeError };
    }

    if (!prd) {
      set({ phase: 'failed', error: 'PRD generation returned empty result' });
      injectError(
        i18n.t('workflow.orchestrator.prdGenerationFailed', { ns: 'simpleMode' }),
        i18n.t('workflow.orchestrator.prdMissingData', { ns: 'simpleMode' }),
      );
      return { ok: false, errorCode: 'prd_missing_data', message: 'PRD generation returned empty result' };
    }

    deps.synthesizePlanningTurn(state.taskDescription, state.strategyAnalysis, prd);

    set({ phase: 'reviewing_prd', editablePrd: prd });
    injectCard('prd_card', deps.toPrdCardData(prd), true);
    return { ok: true };
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    set({ phase: 'failed', error: msg });
    injectError(i18n.t('workflow.orchestrator.prdGenerationFailed', { ns: 'simpleMode' }), msg);
    return { ok: false, errorCode: 'prd_generation_failed', message: msg };
  }
}
