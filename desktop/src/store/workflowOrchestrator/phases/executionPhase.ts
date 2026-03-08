import { invoke } from '@tauri-apps/api/core';
import i18n from '../../../i18n';
import type { TaskPrd } from '../../taskMode';
import { useSettingsStore } from '../../settings';
import { useTaskModeStore } from '../../taskMode';
import type { DesignDocCardData } from '../../../types/workflowCard';
import {
  injectWorkflowCard as injectCard,
  injectWorkflowError as injectError,
  injectWorkflowInfo as injectInfo,
} from '../cardInjection';
import type { WorkflowPhaseRuntime } from './runtime';

interface ExecutionPhaseDeps {
  subscribeToProgressEvents: (set: unknown, get: unknown, runToken: number) => Promise<void>;
}

export async function runDesignDocAndExecutionPhase(
  runtime: WorkflowPhaseRuntime,
  prd: TaskPrd,
  deps: ExecutionPhaseDeps,
): Promise<void> {
  const { set, get, runToken, isRunActive, resolveTaskSessionId } = runtime;
  if (!isRunActive(get, runToken)) return;

  const effectiveSessionId = resolveTaskSessionId(get, set);
  if (!effectiveSessionId) {
    set({ phase: 'failed', error: 'No active task session' });
    injectError(
      i18n.t('workflow.orchestrator.executionFailed', { ns: 'simpleMode' }),
      i18n.t('workflow.orchestrator.sessionMissing', {
        ns: 'simpleMode',
        defaultValue: 'No active task session found.',
      }),
    );
    return;
  }

  set({ phase: 'generating_design_doc', editablePrd: prd });
  injectInfo(i18n.t('workflow.orchestrator.generatingDesignDoc', { ns: 'simpleMode' }), 'info');

  try {
    const projectPath = useSettingsStore.getState().workspacePath || null;
    const designResult = await invoke<{
      success: boolean;
      data?: {
        design_doc: {
          overview: { title: string; summary: string };
          architecture: {
            system_overview: string;
            data_flow: string;
            infrastructure: { existing_services: string[]; new_services: string[] };
            components: {
              name: string;
              description: string;
              responsibilities: string[];
              dependencies: string[];
              features: string[];
            }[];
            patterns: {
              name: string;
              description: string;
              rationale: string;
              applies_to: string[];
            }[];
          };
          decisions: {
            id: string;
            title: string;
            context: string;
            decision: string;
            rationale: string;
            alternatives_considered: string[];
            status: string;
            applies_to: string[];
          }[];
          feature_mappings: Record<
            string,
            {
              description: string;
              components: string[];
              patterns: string[];
              decisions: string[];
            }
          >;
        };
        saved_path: string | null;
        generation_info: unknown;
      };
      error?: string;
    }>('prepare_design_doc_for_task', { sessionId: effectiveSessionId, prd, projectPath });
    if (!isRunActive(get, runToken)) return;

    if (designResult.success && designResult.data) {
      const doc = designResult.data.design_doc;
      const cardData: DesignDocCardData = {
        title: doc.overview.title,
        summary: doc.overview.summary,
        systemOverview: doc.architecture.system_overview,
        dataFlow: doc.architecture.data_flow,
        infrastructure: {
          existingServices: doc.architecture.infrastructure?.existing_services ?? [],
          newServices: doc.architecture.infrastructure?.new_services ?? [],
        },
        componentsCount: doc.architecture.components.length,
        componentNames: doc.architecture.components.map((c) => c.name),
        components: doc.architecture.components.map((c) => ({
          name: c.name,
          description: c.description,
          responsibilities: c.responsibilities ?? [],
          dependencies: c.dependencies ?? [],
          features: c.features ?? [],
        })),
        patternsCount: doc.architecture.patterns.length,
        patternNames: doc.architecture.patterns.map((p) => p.name),
        patterns: doc.architecture.patterns.map((p) => ({
          name: p.name,
          description: p.description,
          rationale: p.rationale,
          appliesTo: p.applies_to ?? [],
        })),
        decisionsCount: doc.decisions.length,
        decisions: doc.decisions.map((d) => ({
          id: d.id,
          title: d.title,
          context: d.context,
          decision: d.decision,
          rationale: d.rationale,
          alternatives: d.alternatives_considered ?? [],
          status: d.status,
          appliesTo: d.applies_to ?? [],
        })),
        featureMappingsCount: Object.keys(doc.feature_mappings).length,
        featureMappings: Object.entries(doc.feature_mappings).map(([featureId, mapping]) => ({
          featureId,
          description: mapping.description ?? '',
          components: mapping.components ?? [],
          patterns: mapping.patterns ?? [],
          decisions: mapping.decisions ?? [],
        })),
        savedPath: designResult.data.saved_path,
      };
      injectCard('design_doc_card', cardData);
    }
    if (!designResult.success) {
      injectInfo(i18n.t('workflow.orchestrator.designDocFailed', { ns: 'simpleMode' }), 'warning');
    }
  } catch {
    injectInfo(i18n.t('workflow.orchestrator.designDocFailed', { ns: 'simpleMode' }), 'warning');
  }

  if (!isRunActive(get, runToken)) return;
  set({ phase: 'executing', isCancelling: false });
  injectInfo(i18n.t('workflow.orchestrator.prdApproved', { ns: 'simpleMode' }), 'success');

  try {
    await deps.subscribeToProgressEvents(set, get, runToken);
    if (!isRunActive(get, runToken)) return;
    const workflowConfig = (
      get() as {
        config: {
          flowLevel: 'quick' | 'standard' | 'full';
          tddMode: 'off' | 'flexible' | 'strict';
          specInterviewEnabled: boolean;
          qualityGatesEnabled: boolean;
          maxParallel: number;
          skipVerification: boolean;
          skipReview: boolean;
          globalAgentOverride: string | null;
          implAgentOverride: string | null;
        };
      }
    ).config;
    const approved = await useTaskModeStore.getState().approvePrd(prd, effectiveSessionId, {
      flowLevel: workflowConfig.flowLevel,
      tddMode: workflowConfig.tddMode,
      enableInterview: workflowConfig.specInterviewEnabled,
      qualityGatesEnabled: workflowConfig.qualityGatesEnabled,
      maxParallel: workflowConfig.maxParallel,
      skipVerification: workflowConfig.skipVerification,
      skipReview: workflowConfig.skipReview,
      globalAgentOverride: workflowConfig.globalAgentOverride,
      implAgentOverride: workflowConfig.implAgentOverride,
    });
    if (!isRunActive(get, runToken)) return;

    const taskModeError = useTaskModeStore.getState().error;
    if (!approved || taskModeError) {
      const message = taskModeError || 'Task execution could not be started';
      set({ phase: 'failed', error: message });
      injectError(i18n.t('workflow.orchestrator.executionFailed', { ns: 'simpleMode' }), message);
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    set({ phase: 'failed', error: msg });
    injectError(i18n.t('workflow.orchestrator.executionFailed', { ns: 'simpleMode' }), msg);
  }
}
