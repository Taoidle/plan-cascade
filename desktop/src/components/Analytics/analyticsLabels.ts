import type { TFunction } from 'i18next';
import type { AnalyticsExecutionScope, AnalyticsUsageEvent, AnalyticsWorkflowMode } from '../../store/analytics';

export function workflowLabel(
  t: TFunction<'analytics'>,
  workflowMode: AnalyticsWorkflowMode | null | undefined,
): string {
  if (!workflowMode) return t('labels.unknown', 'Unknown');
  return t(`workflow.${workflowMode}`, workflowMode);
}

export function scopeLabel(t: TFunction<'analytics'>, scope: AnalyticsExecutionScope | null | undefined): string {
  if (!scope) return t('labels.unknown', 'Unknown');
  return t(`scope.${scope}`, scope);
}

export function phaseLabel(t: TFunction<'analytics'>, phaseId: string | null | undefined): string {
  if (!phaseId) return t('labels.unknown', 'Unknown');
  return t(`phase.${phaseId}`, phaseId);
}

export function stepStoryLabel(
  t: TFunction<'analytics'>,
  event: Pick<AnalyticsUsageEvent, 'step_id' | 'story_id' | 'gate_id'>,
): string {
  if (event.story_id) return `${t('labels.story', 'Story')} ${event.story_id}`;
  if (event.step_id) return `${t('labels.step', 'Step')} ${event.step_id}`;
  if (event.gate_id) return `${t('labels.gate', 'Gate')} ${event.gate_id}`;
  return t('labels.none', 'None');
}

export function agentLabel(
  t: TFunction<'analytics'>,
  event: Pick<AnalyticsUsageEvent, 'agent_name' | 'agent_role' | 'execution_scope'>,
): string {
  if (event.agent_name) return event.agent_name;
  if (event.agent_role) return event.agent_role;
  return scopeLabel(t, event.execution_scope);
}
