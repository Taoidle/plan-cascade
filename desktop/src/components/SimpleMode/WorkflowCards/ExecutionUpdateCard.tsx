/**
 * ExecutionUpdateCard
 *
 * Inline execution progress updates showing batch/story status.
 */

import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { ExecutionUpdateCardData } from '../../../types/workflowCard';

export function ExecutionUpdateCard({ data }: { data: ExecutionUpdateCardData }) {
  const { t } = useTranslation('simpleMode');
  const progressPct = Math.min(100, Math.max(0, data.progressPct));

  return (
    <div className="rounded-lg border border-blue-200 dark:border-blue-800 bg-blue-50 dark:bg-blue-900/20 px-3 py-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <EventIcon eventType={data.eventType} />
          <span className="text-xs font-medium text-blue-700 dark:text-blue-300">
            {formatEventType(data.eventType, t)}
          </span>
          {data.storyTitle && (
            <span className="text-xs text-blue-600/80 dark:text-blue-400/80 truncate max-w-48">{data.storyTitle}</span>
          )}
          {data.agent && (
            <span className="text-2xs px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-900/40 text-blue-500 dark:text-blue-400">
              {data.agent}
            </span>
          )}
        </div>
        <span className="text-2xs text-blue-500 dark:text-blue-400">
          {t('workflow.execution.batchLabel', { current: data.currentBatch + 1, total: data.totalBatches })}
        </span>
      </div>

      {/* Progress bar */}
      <div className="mt-1.5 h-1 rounded-full bg-blue-100 dark:bg-blue-900/50 overflow-hidden">
        <div
          className={clsx(
            'h-full rounded-full transition-all duration-300',
            data.eventType === 'story_failed' ? 'bg-red-500' : 'bg-blue-500',
          )}
          style={{ width: `${progressPct}%` }}
        />
      </div>

      <p className="text-2xs text-blue-500/70 dark:text-blue-400/70 mt-1">{data.status}</p>
    </div>
  );
}

function EventIcon({ eventType }: { eventType: ExecutionUpdateCardData['eventType'] }) {
  const cls = 'w-3.5 h-3.5';
  switch (eventType) {
    case 'batch_start':
      return (
        <svg className={clsx(cls, 'text-blue-500')} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
      );
    case 'story_start':
      return (
        <svg className={clsx(cls, 'text-blue-500 animate-spin')} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="10" strokeWidth={2} strokeDasharray="31.4" strokeDashoffset="10" />
        </svg>
      );
    case 'story_complete':
      return (
        <svg className={clsx(cls, 'text-green-500')} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
        </svg>
      );
    case 'story_failed':
      return (
        <svg className={clsx(cls, 'text-red-500')} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
        </svg>
      );
    case 'batch_complete':
      return (
        <svg className={clsx(cls, 'text-green-500')} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
      );
    default:
      return null;
  }
}

function formatEventType(type: ExecutionUpdateCardData['eventType'], t: (key: string) => string): string {
  switch (type) {
    case 'batch_start':
      return t('workflow.execution.batchStarted');
    case 'story_start':
      return t('workflow.execution.storyStarted');
    case 'story_complete':
      return t('workflow.execution.storyCompleted');
    case 'story_failed':
      return t('workflow.execution.storyFailed');
    case 'batch_complete':
      return t('workflow.execution.batchComplete');
    default:
      return type;
  }
}
