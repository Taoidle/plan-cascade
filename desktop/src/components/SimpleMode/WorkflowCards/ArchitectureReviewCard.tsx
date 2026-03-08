/**
 * ArchitectureReviewCard
 *
 * Displays the SoftwareArchitect persona's architecture review results.
 * Shows concerns (severity-colored), suggestions, and PRD modifications
 * with checkboxes for user to accept/reject. Interactive when first shown.
 * Uses cyan color scheme.
 */

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useWorkflowOrchestratorStore } from '../../../store/workflowOrchestrator';
import { useWorkflowKernelStore } from '../../../store/workflowKernel';
import { submitWorkflowActionIntentViaCoordinator } from '../../../store/simpleWorkflowCoordinator';
import type { ArchitectureReviewCardData } from '../../../types/workflowCard';
import type { StreamLine } from '../../../store/execution';
import { reportInteractiveActionFailure } from '../../../lib/workflowObservability';
const severityColors = {
  high: 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 border-red-200 dark:border-red-800',
  medium: 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300 border-amber-200 dark:border-amber-800',
  low: 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 border-blue-200 dark:border-blue-800',
} as const;

export function ArchitectureReviewCard({
  data,
  interactive = false,
  cardId,
}: {
  data: ArchitectureReviewCardData;
  interactive?: boolean;
  cardId?: string;
}) {
  const { t } = useTranslation('simpleMode');
  const [expanded, setExpanded] = useState(false);
  const [selectedMods, setSelectedMods] = useState<Set<number>>(() => new Set(data.prdModifications.map((_, i) => i)));
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const approveArchitecture = useWorkflowOrchestratorStore((s) => s.approveArchitecture);
  const orchestratorPhase = useWorkflowOrchestratorStore((s) => s.phase);
  const workflowSession = useWorkflowKernelStore((s) => s.session);
  const latestInteractiveArchitectureCardId = useWorkflowKernelStore((state) => {
    const rootSessionId = state.session?.sessionId;
    if (!rootSessionId) return null;
    const lines = state.getCachedModeTranscript(rootSessionId, 'task').lines as StreamLine[];
    for (let index = lines.length - 1; index >= 0; index -= 1) {
      const line = lines[index];
      if (line.type !== 'card' || !line.cardPayload?.interactive) continue;
      if (line.cardPayload.cardType === 'architecture_review_card') {
        return line.cardPayload.cardId;
      }
    }
    return null;
  });
  const hasModifications = data.prdModifications.length > 0;

  const toggleMod = (index: number) => {
    setSelectedMods((prev) => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  };

  const handleContinueExecution = async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    setSubmitError(null);
    try {
      try {
        await submitWorkflowActionIntentViaCoordinator({
          mode: 'task',
          type: 'execution_control',
          source: 'architecture_review_card',
          action: hasModifications ? 'bypass_architecture_changes' : 'continue_after_architecture_review',
          content: hasModifications ? 'architecture_review:bypass_changes' : 'architecture_review:continue',
          metadata: {
            concernCount: data.concerns.length,
            suggestionCount: data.suggestions.length,
            modificationCount: data.prdModifications.length,
          },
        });
      } catch {
        // Keep orchestration available even if kernel logging fails.
      }
      const result = await approveArchitecture?.(true, []);
      if (result && !result.ok) {
        const message = result.message || 'Failed to continue after architecture review';
        setSubmitError(message);
        await reportInteractiveActionFailure({
          card: 'architecture_review_card',
          action: hasModifications ? 'bypass_architecture_changes' : 'continue_after_architecture_review',
          errorCode:
            result.errorCode || (hasModifications ? 'architecture_bypass_failed' : 'architecture_continue_failed'),
          message,
          session: workflowSession,
        });
      }
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleApplyModifications = async () => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    setSubmitError(null);
    const selected = data.prdModifications.filter((_, i) => selectedMods.has(i));
    try {
      try {
        await submitWorkflowActionIntentViaCoordinator({
          mode: 'task',
          type: 'execution_control',
          source: 'architecture_review_card',
          action: 'apply_architecture_changes',
          content: 'architecture_review:apply_changes',
          metadata: {
            selectedModificationCount: selected.length,
            totalModificationCount: data.prdModifications.length,
          },
        });
      } catch {
        // Keep orchestration available even if kernel logging fails.
      }
      const result = await approveArchitecture?.(false, selected);
      if (result && !result.ok) {
        const message = result.message || 'Failed to apply architecture feedback';
        setSubmitError(message);
        await reportInteractiveActionFailure({
          card: 'architecture_review_card',
          action: 'apply_architecture_changes',
          errorCode: result.errorCode || 'architecture_apply_failed',
          message,
          session: workflowSession,
        });
      }
    } finally {
      setIsSubmitting(false);
    }
  };

  const isKernelTaskActive = workflowSession?.status === 'active' && workflowSession.activeMode === 'task';
  const kernelTaskPhase = workflowSession?.modeSnapshots.task?.phase ?? 'idle';
  const isLatestCard =
    !cardId || !latestInteractiveArchitectureCardId || latestInteractiveArchitectureCardId === cardId;
  const isArchitectureReviewActive =
    orchestratorPhase === 'architecture_review' || kernelTaskPhase === 'architecture_review';
  const isInteractive =
    interactive &&
    isArchitectureReviewActive &&
    isLatestCard &&
    !isSubmitting &&
    (orchestratorPhase === 'architecture_review' || isKernelTaskActive);

  return (
    <div className="rounded-lg border border-cyan-200 dark:border-cyan-800 bg-cyan-50 dark:bg-cyan-900/20 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-cyan-100/50 dark:bg-cyan-900/30 border-b border-cyan-200 dark:border-cyan-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-cyan-700 dark:text-cyan-300 uppercase tracking-wide">
              {t('workflow.architectureReview.title')}
            </span>
            <span className="text-2xs px-1.5 py-0.5 rounded bg-cyan-200 dark:bg-cyan-800 text-cyan-600 dark:text-cyan-400">
              {data.personaRole}
            </span>
            {data.approved && (
              <span className="text-2xs px-1.5 py-0.5 rounded bg-green-200 dark:bg-green-800 text-green-600 dark:text-green-400">
                {t('workflow.architectureReview.approved', 'Approved')}
              </span>
            )}
          </div>
          <button
            onClick={() => setExpanded((v) => !v)}
            className="text-2xs text-cyan-600 dark:text-cyan-400 hover:text-cyan-800 dark:hover:text-cyan-200 transition-colors"
          >
            {expanded ? '▲' : '▼'}
          </button>
        </div>
      </div>

      <div className="px-3 py-2 space-y-2">
        {/* Concerns */}
        {data.concerns.length > 0 && (
          <div>
            <span className="text-2xs font-medium text-cyan-600 dark:text-cyan-400">
              {t('workflow.architectureReview.concerns')}
            </span>
            <div className="mt-0.5 space-y-1">
              {data.concerns.map((concern, i) => (
                <div
                  key={i}
                  className={`text-2xs px-2 py-1 rounded border ${
                    severityColors[concern.severity as keyof typeof severityColors] || severityColors.medium
                  }`}
                >
                  <span className="font-medium uppercase mr-1">[{concern.severity}]</span>
                  {concern.description}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Suggestions */}
        {data.suggestions.length > 0 && (
          <div>
            <span className="text-2xs font-medium text-cyan-600 dark:text-cyan-400">
              {t('workflow.architectureReview.suggestions')}
            </span>
            <ul className="mt-0.5 space-y-0.5">
              {data.suggestions.slice(0, expanded ? undefined : 3).map((sug, i) => (
                <li key={i} className="text-2xs text-cyan-700 dark:text-cyan-300 flex items-start gap-1">
                  <span className="text-cyan-400 dark:text-cyan-500 shrink-0 mt-px">•</span>
                  <span>{sug}</span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* PRD Modifications (interactive checkboxes) */}
        {data.prdModifications.length > 0 && (
          <div>
            <span className="text-2xs font-medium text-cyan-600 dark:text-cyan-400">
              {t('workflow.architectureReview.prdChanges', 'Suggested PRD Changes')}
            </span>
            <div className="mt-0.5 space-y-1">
              {data.prdModifications.map((mod, i) => (
                <label
                  key={i}
                  className="flex items-start gap-1.5 text-2xs text-cyan-700 dark:text-cyan-300 cursor-pointer"
                >
                  {isInteractive ? (
                    <input
                      type="checkbox"
                      checked={selectedMods.has(i)}
                      onChange={() => toggleMod(i)}
                      className="mt-0.5 rounded border-cyan-300 dark:border-cyan-700 text-cyan-600"
                    />
                  ) : (
                    <span className="text-cyan-400 dark:text-cyan-500 shrink-0 mt-px">•</span>
                  )}
                  <span>
                    <span className="font-medium">[{mod.type}]</span>
                    {mod.targetStoryId && (
                      <span className="text-cyan-500 dark:text-cyan-400 ml-1">#{mod.targetStoryId}</span>
                    )}
                    <span className="ml-1">{mod.preview}</span>
                    <span className="ml-1 text-cyan-500/80 dark:text-cyan-400/80">({mod.reason})</span>
                  </span>
                </label>
              ))}
            </div>
          </div>
        )}

        {/* Expanded: Full Analysis */}
        {expanded && data.analysis && (
          <div className="pt-1 border-t border-cyan-200 dark:border-cyan-800">
            <span className="text-2xs font-medium text-cyan-600 dark:text-cyan-400">
              {t('workflow.architectureReview.fullAnalysis', 'Full Analysis')}
            </span>
            <div className="mt-0.5 text-2xs text-cyan-700/80 dark:text-cyan-300/80 whitespace-pre-wrap">
              {data.analysis}
            </div>
          </div>
        )}

        {/* Action buttons (interactive mode only) */}
        {isInteractive && (
          <div className="pt-2 flex items-center gap-2 border-t border-cyan-200 dark:border-cyan-800">
            {hasModifications && (
              <button
                onClick={handleApplyModifications}
                disabled={isSubmitting}
                className="text-2xs px-3 py-1 rounded bg-amber-600 hover:bg-amber-700 text-white transition-colors"
              >
                {isSubmitting
                  ? t('workflow.common.processing', { defaultValue: 'Processing...' })
                  : t('workflow.architectureReview.applyModifications')}
              </button>
            )}
            <button
              onClick={handleContinueExecution}
              disabled={isSubmitting}
              className="text-2xs px-3 py-1 rounded bg-green-600 hover:bg-green-700 text-white transition-colors"
            >
              {isSubmitting
                ? t('workflow.common.processing', { defaultValue: 'Processing...' })
                : hasModifications
                  ? t('workflow.architectureReview.bypassAndExecute')
                  : t('workflow.architectureReview.continueExecution')}
            </button>
            {submitError && <span className="text-2xs text-rose-600 dark:text-rose-300">{submitError}</span>}
          </div>
        )}
      </div>
    </div>
  );
}
