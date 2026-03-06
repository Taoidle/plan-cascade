import { invoke } from '@tauri-apps/api/core';
import i18n from '../../../i18n';
import type { TaskPrd } from '../../taskMode';
import { useSettingsStore } from '../../settings';
import { resolvePersonaDisplayName } from '../../../lib/personaI18n';
import type { ArchitectureReviewCardData } from '../../../types/workflowCard';
import {
  injectWorkflowCard as injectCard,
  injectWorkflowError as injectError,
  injectWorkflowInfo as injectInfo,
} from '../cardInjection';
import type { WorkflowPhaseRuntime } from './runtime';

interface ArchitecturePhaseDeps {
  runDesignDocAndExecutionPhase: (runtime: WorkflowPhaseRuntime, prd: TaskPrd) => Promise<void>;
}

export async function runArchitecturePhase(
  runtime: WorkflowPhaseRuntime,
  prd: TaskPrd,
  deps: ArchitecturePhaseDeps,
): Promise<void> {
  const { set, get, runToken, isRunActive, resolveTaskSessionId } = runtime;
  if (!isRunActive(get, runToken)) return;

  const { architectureReviewRound, explorationResult } = get() as {
    architectureReviewRound: number;
    explorationResult: unknown;
  };
  const effectiveSessionId = resolveTaskSessionId(get, set);
  if (!effectiveSessionId) {
    set({ phase: 'failed', error: 'No active task session' });
    injectError(
      i18n.t('workflow.orchestrator.architectureReviewFailed', { ns: 'simpleMode' }),
      i18n.t('workflow.orchestrator.sessionMissing', {
        ns: 'simpleMode',
        defaultValue: 'No active task session found.',
      }),
    );
    return;
  }

  if (architectureReviewRound >= 3) {
    injectInfo(
      i18n.t('workflow.orchestrator.architectureReviewMaxRounds', {
        ns: 'simpleMode',
        defaultValue: 'Architecture review limit reached (3 rounds). Proceeding with current PRD.',
      }),
      'warning',
    );
    return;
  }

  set({ phase: 'architecture_review', architectureReviewRound: architectureReviewRound + 1 });
  const { resolvePhaseAgent, formatModelDisplay } = await import('../../../lib/phaseAgentResolver');
  if (!isRunActive(get, runToken)) return;
  const archResolved = resolvePhaseAgent('plan_architecture');

  injectCard('persona_indicator', {
    role: 'SoftwareArchitect',
    displayName: resolvePersonaDisplayName(i18n.t.bind(i18n), 'SoftwareArchitect'),
    phase: 'architecture_review',
    model: formatModelDisplay(archResolved),
  });
  injectInfo(
    i18n.t('workflow.orchestrator.reviewingArchitecture', {
      ns: 'simpleMode',
      defaultValue: 'Reviewing architecture...',
    }),
    'info',
  );

  try {
    const explorationContext = explorationResult ? JSON.stringify(explorationResult) : null;
    const archContextSources =
      (await import('../../contextSources')).useContextSourcesStore.getState().buildConfig() ?? null;
    const projectPath = useSettingsStore.getState().workspacePath || null;
    const result = await invoke<{
      success: boolean;
      data: ArchitectureReviewCardData | null;
      error: string | null;
    }>('run_architecture_review', {
      request: {
        sessionId: effectiveSessionId,
        prdJson: JSON.stringify(prd),
        explorationContext,
        provider: archResolved.provider || null,
        model: archResolved.model || null,
        apiKey: null,
        baseUrl: archResolved.baseUrl || null,
        locale: i18n.language,
        contextSources: archContextSources,
        projectPath,
      },
    });
    if (!isRunActive(get, runToken)) return;

    if (result.success && result.data) {
      set({ architectureReview: result.data });
      injectCard('architecture_review_card', result.data, true);
      return;
    }

    injectInfo(
      i18n.t('workflow.orchestrator.architectureReviewFailed', {
        ns: 'simpleMode',
        defaultValue: 'Architecture review could not be completed. Continuing...',
      }),
      'warning',
    );
    await deps.runDesignDocAndExecutionPhase(runtime, prd);
  } catch {
    injectInfo(
      i18n.t('workflow.orchestrator.architectureReviewFailed', {
        ns: 'simpleMode',
        defaultValue: 'Architecture review could not be completed. Continuing...',
      }),
      'warning',
    );
    await deps.runDesignDocAndExecutionPhase(runtime, prd);
  }
}
