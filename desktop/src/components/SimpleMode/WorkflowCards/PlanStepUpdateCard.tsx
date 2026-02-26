/**
 * Plan Step Update Card
 *
 * Displays step execution progress events during the executing phase.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { PlanStepUpdateCardData } from '../../../types/planModeCard';

function eventIcon(eventType: string): string {
  switch (eventType) {
    case 'batch_started':
      return '\u25B6';
    case 'step_started':
      return '\u23F3';
    case 'step_completed':
      return '\u2705';
    case 'step_failed':
      return '\u274C';
    case 'batch_completed':
      return '\u2714';
    default:
      return '\u2022';
  }
}

function eventLabel(eventType: string, t: (key: string, defaultValue: string) => string): string {
  switch (eventType) {
    case 'batch_started':
      return t('execution.batchStarted', 'Batch Started');
    case 'step_started':
      return t('execution.stepStarted', 'Step Started');
    case 'step_completed':
      return t('execution.stepCompleted', 'Step Completed');
    case 'step_failed':
      return t('execution.stepFailed', 'Step Failed');
    case 'batch_completed':
      return t('execution.batchComplete', 'Batch Complete');
    default:
      return eventType;
  }
}

export function PlanStepUpdateCard({ data }: { data: PlanStepUpdateCardData }) {
  const { t } = useTranslation('planMode');
  const isFailed = data.eventType === 'step_failed';

  return (
    <div
      className={clsx(
        'rounded-lg border px-3 py-2',
        isFailed
          ? 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20'
          : 'border-blue-200 dark:border-blue-800 bg-blue-50 dark:bg-blue-900/20',
      )}
    >
      {/* Header */}
      <div className="flex items-center gap-2">
        <span className="text-sm">{eventIcon(data.eventType)}</span>
        <span
          className={clsx(
            'text-xs font-medium',
            isFailed ? 'text-red-700 dark:text-red-300' : 'text-blue-700 dark:text-blue-300',
          )}
        >
          {eventLabel(data.eventType, t)}
        </span>
        {data.stepTitle && <span className="text-xs text-gray-600 dark:text-gray-400 truncate">{data.stepTitle}</span>}
        <span className="ml-auto text-2xs text-gray-500">
          {t('execution.batchLabel', 'Batch {{current}}/{{total}}', {
            current: data.currentBatch + 1,
            total: data.totalBatches,
          })}
        </span>
      </div>

      {/* Progress bar */}
      {data.progressPct > 0 && (
        <div className="mt-1.5 w-full bg-gray-200 dark:bg-gray-700 rounded-full h-1.5">
          <div
            className={clsx('h-1.5 rounded-full transition-all duration-300', isFailed ? 'bg-red-500' : 'bg-blue-500')}
            style={{ width: `${Math.min(data.progressPct, 100)}%` }}
          />
        </div>
      )}

      {/* Error message */}
      {data.error && <p className="mt-1 text-2xs text-red-600 dark:text-red-400">{data.error}</p>}
    </div>
  );
}
