import { invoke } from '@tauri-apps/api/core';
import i18n from '../../../i18n';
import { resolvePersonaDisplayName } from '../../../lib/personaI18n';
import type { RequirementAnalysisCardData } from '../../../types/workflowCard';
import { useSettingsStore } from '../../settings';
import { useSpecInterviewStore } from '../../specInterview';
import {
  injectWorkflowCard as injectCard,
  injectWorkflowError as injectError,
  injectWorkflowInfo as injectInfo,
} from '../cardInjection';
import type { WorkflowPhaseRuntime } from './runtime';

interface RequirementPhaseDeps {
  // Reserved for future extension.
  _unused?: never;
}

export async function runRequirementPhase(runtime: WorkflowPhaseRuntime, _deps: RequirementPhaseDeps): Promise<void> {
  const { set, get, runToken, isRunActive, resolveTaskSessionId } = runtime;
  if (!isRunActive(get, runToken)) return;

  const { config, taskDescription, explorationResult } = get() as {
    config: { flowLevel: 'quick' | 'standard' | 'full' };
    taskDescription: string;
    explorationResult: unknown;
  };
  const effectiveSessionId = resolveTaskSessionId(get, set);
  if (!effectiveSessionId) {
    set({ phase: 'failed', error: 'No active task session' });
    injectError(
      i18n.t('workflow.orchestrator.requirementAnalysisFailed', { ns: 'simpleMode' }),
      i18n.t('workflow.orchestrator.sessionMissing', {
        ns: 'simpleMode',
        defaultValue: 'No active task session found.',
      }),
    );
    return;
  }

  if (config.flowLevel === 'quick') return;

  set({ phase: 'requirement_analysis' });

  const { resolvePhaseAgent, formatModelDisplay } = await import('../../../lib/phaseAgentResolver');
  if (!isRunActive(get, runToken)) return;
  const reqResolved = resolvePhaseAgent('plan_requirements');

  injectCard('persona_indicator', {
    role: 'ProductManager',
    displayName: resolvePersonaDisplayName(i18n.t.bind(i18n), 'ProductManager'),
    phase: 'requirement_analysis',
    model: formatModelDisplay(reqResolved),
  });
  injectInfo(
    i18n.t('workflow.orchestrator.analyzingRequirements', {
      ns: 'simpleMode',
      defaultValue: 'Analyzing requirements...',
    }),
    'info',
  );

  try {
    const explorationContext = explorationResult ? JSON.stringify(explorationResult) : null;
    const specStore = useSpecInterviewStore.getState();
    const interviewResult = specStore.compiledSpec ? JSON.stringify(specStore.compiledSpec) : null;

    const contextSources =
      (await import('../../contextSources')).useContextSourcesStore.getState().buildConfig() ?? null;
    const projectPath = useSettingsStore.getState().workspacePath || null;
    const result = await invoke<{
      success: boolean;
      data: RequirementAnalysisCardData | null;
      error: string | null;
    }>('run_requirement_analysis', {
      request: {
        sessionId: effectiveSessionId,
        taskDescription,
        interviewResult,
        explorationContext,
        provider: reqResolved.provider || null,
        model: reqResolved.model || null,
        apiKey: null,
        baseUrl: reqResolved.baseUrl || null,
        locale: i18n.language,
        contextSources,
        projectPath,
      },
    });
    if (!isRunActive(get, runToken)) return;

    if (result.success && result.data) {
      set({ requirementAnalysis: result.data });
      injectCard('requirement_analysis_card', result.data);
    } else {
      injectInfo(
        i18n.t('workflow.orchestrator.requirementAnalysisFailed', {
          ns: 'simpleMode',
          defaultValue: 'Requirement analysis could not be completed. Continuing...',
        }),
        'warning',
      );
    }
  } catch {
    injectInfo(
      i18n.t('workflow.orchestrator.requirementAnalysisFailed', {
        ns: 'simpleMode',
        defaultValue: 'Requirement analysis could not be completed. Continuing...',
      }),
      'warning',
    );
  }
}
