import { invoke } from '@tauri-apps/api/core';
import i18n from '../../../i18n';
import { resolvePersonaDisplayName } from '../../../lib/personaI18n';
import type { ExplorationCardData } from '../../../types/workflowCard';
import {
  injectWorkflowCard as injectCard,
  injectWorkflowError as injectError,
  injectWorkflowInfo as injectInfo,
} from '../cardInjection';
import type { WorkflowPhaseRuntime } from './runtime';

interface ExplorePhaseDeps {
  normalizeExplorationCardData: (data: ExplorationCardData) => ExplorationCardData;
}

export async function runExplorePhase(runtime: WorkflowPhaseRuntime, deps: ExplorePhaseDeps): Promise<void> {
  const { set, get, runToken, isRunActive, resolveTaskSessionId } = runtime;
  if (!isRunActive(get, runToken)) return;

  const { config, taskDescription } = get() as {
    config: { flowLevel: 'quick' | 'standard' | 'full' };
    taskDescription: string;
  };
  const effectiveSessionId = resolveTaskSessionId(get, set);
  if (!effectiveSessionId) {
    set({ phase: 'failed', error: 'No active task session' });
    injectError(
      i18n.t('workflow.orchestrator.explorationFailed', { ns: 'simpleMode' }),
      i18n.t('workflow.orchestrator.sessionMissing', {
        ns: 'simpleMode',
        defaultValue: 'No active task session found.',
      }),
    );
    return;
  }

  if (config.flowLevel === 'quick') return;

  set({ phase: 'exploring' });

  const { resolvePhaseAgent, formatModelDisplay } = await import('../../../lib/phaseAgentResolver');
  if (!isRunActive(get, runToken)) return;
  const explorationResolved = resolvePhaseAgent('plan_exploration');

  injectCard('persona_indicator', {
    role: 'SeniorEngineer',
    displayName: resolvePersonaDisplayName(i18n.t.bind(i18n), 'SeniorEngineer'),
    phase: 'exploring',
    model: formatModelDisplay(explorationResolved),
  });
  injectInfo(i18n.t('workflow.orchestrator.exploringProject', { ns: 'simpleMode' }), 'info');

  try {
    const contextSources =
      (await import('../../contextSources')).useContextSourcesStore.getState().buildConfig() ?? null;
    const result = await invoke<{
      success: boolean;
      data: ExplorationCardData | null;
      error: string | null;
    }>('explore_project', {
      request: {
        sessionId: effectiveSessionId,
        flowLevel: config.flowLevel,
        taskDescription,
        provider: explorationResolved.provider || null,
        model: explorationResolved.model || null,
        apiKey: null,
        baseUrl: explorationResolved.baseUrl || null,
        locale: i18n.language,
        contextSources,
      },
    });
    if (!isRunActive(get, runToken)) return;

    if (result.success && result.data) {
      const normalized = deps.normalizeExplorationCardData(result.data);
      set({ explorationResult: normalized });
      injectCard('exploration_card', normalized);
    } else {
      injectInfo(i18n.t('workflow.orchestrator.explorationFailed', { ns: 'simpleMode' }), 'warning');
    }
  } catch {
    injectInfo(i18n.t('workflow.orchestrator.explorationFailed', { ns: 'simpleMode' }), 'warning');
  }
}
