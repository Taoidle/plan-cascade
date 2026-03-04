import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { usePlanOrchestratorStore } from '../../../store/planOrchestrator';
import type { PlanClarificationResolutionCardData } from '../../../types/planModeCard';

export function PlanClarificationResolutionCard({
  data,
  interactive,
}: {
  data: PlanClarificationResolutionCardData;
  interactive: boolean;
}) {
  const { t } = useTranslation('planMode');
  const phase = usePlanOrchestratorStore((s) => s.phase);
  const retryClarification = usePlanOrchestratorStore((s) => s.retryClarification);
  const skipClarification = usePlanOrchestratorStore((s) => s.skipClarification);
  const cancelWorkflow = usePlanOrchestratorStore((s) => s.cancelWorkflow);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const isInteractive = interactive && phase === 'clarification_error' && !isSubmitting;

  const runAction = async (action: () => Promise<void>) => {
    if (!isInteractive) return;
    setIsSubmitting(true);
    try {
      await action();
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="rounded-lg border border-amber-300 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20 overflow-hidden">
      <div className="px-3 py-2 bg-amber-100/60 dark:bg-amber-900/30 border-b border-amber-200 dark:border-amber-800">
        <p className="text-xs font-semibold uppercase tracking-wide text-amber-700 dark:text-amber-300">{data.title}</p>
      </div>
      <div className="px-3 py-2 space-y-2">
        <p className="text-xs text-amber-800 dark:text-amber-200">{data.message}</p>
        {data.reasonCode && (
          <p className="text-2xs text-amber-600 dark:text-amber-400">
            {t('clarify.errorCode', { defaultValue: 'Reason code: {{code}}', code: data.reasonCode })}
          </p>
        )}
        <div className="flex flex-wrap items-center gap-2">
          {data.canRetry && (
            <button
              onClick={() => {
                void runAction(retryClarification);
              }}
              disabled={!isInteractive}
              className="px-2.5 py-1 rounded text-xs font-medium bg-amber-600 text-white hover:bg-amber-700 disabled:opacity-60 disabled:cursor-not-allowed"
            >
              {t('clarify.retry', { defaultValue: 'Retry' })}
            </button>
          )}
          {data.canSkip && (
            <button
              onClick={() => {
                void runAction(skipClarification);
              }}
              disabled={!isInteractive}
              className="px-2.5 py-1 rounded text-xs font-medium border border-amber-400 dark:border-amber-700 text-amber-700 dark:text-amber-300 hover:bg-amber-100/70 dark:hover:bg-amber-900/30 disabled:opacity-60 disabled:cursor-not-allowed"
            >
              {t('clarify.skip', { defaultValue: 'Skip clarification' })}
            </button>
          )}
          {data.canCancel && (
            <button
              onClick={() => {
                void runAction(cancelWorkflow);
              }}
              disabled={!isInteractive}
              className="px-2.5 py-1 rounded text-xs font-medium border border-red-300 dark:border-red-800 text-red-600 dark:text-red-300 hover:bg-red-50 dark:hover:bg-red-900/20 disabled:opacity-60 disabled:cursor-not-allowed"
            >
              {t('clarify.cancelWorkflow', { defaultValue: 'Cancel workflow' })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export default PlanClarificationResolutionCard;
