import i18n from '../../i18n';
import { routeModeCard, routeModeStreamLine } from '../modeTranscriptRouting';
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
  void routeModeCard('plan', payload);
}

export function injectPlanInfo(message: string): void {
  injectPlanCard('workflow_info', { message, level: 'info' });
}

export function injectPlanError(title: string, description: string): void {
  injectPlanCard('workflow_error', { title, description, suggestedFix: '' });
}

export function appendPlanUserMessage(message: string): void {
  const trimmed = message.trim();
  if (!trimmed) return;
  void routeModeStreamLine('plan', trimmed, 'info', {
    turnBoundary: 'user',
    turnId: Date.now(),
  });
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

export function buildPlanCompletionCardDataFromReport(report: PlanExecutionReport): PlanCompletionCardData {
  const normalizedTerminalState =
    report.terminalState === 'completed' ||
    report.terminalState === 'completed_with_warnings' ||
    report.terminalState === 'needs_review' ||
    report.terminalState === 'failed' ||
    report.terminalState === 'cancelled'
      ? report.terminalState
      : report.success
        ? 'completed'
        : 'failed';
  return {
    success: report.success,
    terminalState: normalizedTerminalState,
    terminalStatus: report.terminalStatus,
    planTitle: report.planTitle,
    totalSteps: report.totalSteps,
    stepsCompleted: report.stepsCompleted,
    stepsFailed: report.stepsFailed,
    stepsSoftFailed: report.stepsSoftFailed ?? 0,
    stepsNeedsReview: report.stepsNeedsReview ?? 0,
    stepsCancelled: report.stepsCancelled ?? 0,
    stepsAttempted: report.stepsAttempted ?? report.stepsCompleted + report.stepsFailed,
    stepsFailedBeforeCancel: report.stepsFailedBeforeCancel ?? 0,
    totalDurationMs: report.totalDurationMs,
    stepSummaries: report.stepSummaries,
    failureReasons: report.failureReasons,
    cancelledBy: report.cancelledBy,
    runId: report.runId,
    finalConclusionMarkdown: report.finalConclusionMarkdown,
    highlights: report.highlights,
    nextActions: report.nextActions,
    retryStats: report.retryStats,
    terminalVerdictTrace: report.terminalVerdictTrace ?? [],
  };
}

export function buildPlanCompletionCardDataFallback(
  plan: PlanCardData,
  stepStatuses: Record<string, string>,
  terminalState: PlanCompletionCardData['terminalState'],
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

  const effectiveTerminalState = terminalState ?? (stepsFailed > 0 ? 'failed' : 'completed');
  return {
    success: effectiveTerminalState === 'completed' && stepsFailed === 0,
    terminalState: effectiveTerminalState,
    terminalStatus: effectiveTerminalState,
    planTitle: plan.title,
    totalSteps,
    stepsCompleted,
    stepsFailed,
    stepsCancelled: plan.steps.filter((step) => stepStatuses[step.id] === 'cancelled').length,
    stepsAttempted: plan.steps.filter((step) => ['completed', 'failed', 'cancelled'].includes(stepStatuses[step.id]))
      .length,
    stepsFailedBeforeCancel: effectiveTerminalState === 'cancelled' ? stepsFailed : 0,
    totalDurationMs: 0,
    stepSummaries,
    failureReasons: {},
    cancelledBy: effectiveTerminalState === 'cancelled' ? 'user' : null,
    finalConclusionMarkdown:
      effectiveTerminalState === 'completed'
        ? 'Execution finished. Review each step summary and artifacts for final delivery.'
        : 'Execution did not fully complete. Resolve failed or blocked steps before continuing.',
    highlights: [],
    nextActions:
      effectiveTerminalState === 'completed'
        ? ['Validate outputs and merge into the final result.']
        : ['Retry blocked steps and verify dependency outputs first.'],
    retryStats: {
      totalRetries: 0,
      stepsRetried: 0,
      exhaustedFailures: 0,
    },
    terminalVerdictTrace: [],
  };
}
