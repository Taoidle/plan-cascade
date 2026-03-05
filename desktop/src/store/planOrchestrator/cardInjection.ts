import i18n from '../../i18n';
import { useExecutionStore } from '../execution';
import type { CardPayload } from '../../types/workflowCard';
import type {
  PlanCardData,
  PlanClarificationResolutionCardData,
  PlanCompletionCardData,
  PlanExecutionReport,
  PlanStepOutputCardData,
} from '../../types/planModeCard';

export function injectPlanCard(cardType: string, data: Record<string, unknown>, interactive = false): void {
  const payload: CardPayload = {
    cardType: cardType as CardPayload['cardType'],
    cardId: `${cardType}-${Date.now()}`,
    data: data as unknown as CardPayload['data'],
    interactive,
  };
  useExecutionStore.getState().appendCard(payload);
}

export function injectPlanInfo(message: string): void {
  injectPlanCard('workflow_info', { message, level: 'info' });
}

export function injectPlanError(title: string, description: string): void {
  injectPlanCard('workflow_error', { title, description, suggestedFix: '' });
}

export function injectClarificationResolutionCard(reasonCode: string | null, message: string): void {
  injectPlanCard(
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

export function normalizeStepOutputFormat(format: string | undefined): PlanStepOutputCardData['format'] {
  if (format === 'markdown' || format === 'json' || format === 'html' || format === 'code') return format;
  return 'text';
}

export function buildPlanCompletionCardDataFromReport(
  report: PlanExecutionReport,
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

export function buildPlanCompletionCardDataFallback(
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
