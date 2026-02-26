/**
 * Plan Completion Card
 *
 * Final report card for a completed plan execution.
 * Shows success/failure status, step summaries, and duration.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, ChevronRightIcon } from '@radix-ui/react-icons';
import type { PlanCompletionCardData } from '../../../types/planModeCard';

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remaining = seconds % 60;
  return `${minutes}m ${remaining}s`;
}

export function PlanCompletionCard({ data }: { data: PlanCompletionCardData }) {
  const { t } = useTranslation('planMode');
  const [showSummaries, setShowSummaries] = useState(false);
  const summaryEntries = Object.entries(data.stepSummaries);

  return (
    <div
      className={clsx(
        'rounded-lg border',
        data.success
          ? 'border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20'
          : 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20',
      )}
    >
      {/* Header */}
      <div
        className={clsx(
          'px-3 py-2 border-b flex items-center justify-between',
          data.success
            ? 'bg-green-100/50 dark:bg-green-900/30 border-green-200 dark:border-green-800'
            : 'bg-red-100/50 dark:bg-red-900/30 border-red-200 dark:border-red-800',
        )}
      >
        <div className="flex items-center gap-2">
          <span className="text-sm">{data.success ? '\u2705' : '\u274C'}</span>
          <span
            className={clsx(
              'text-xs font-semibold',
              data.success ? 'text-green-700 dark:text-green-300' : 'text-red-700 dark:text-red-300',
            )}
          >
            {data.planTitle}
          </span>
        </div>
        <span
          className={clsx(
            'text-2xs px-1.5 py-0.5 rounded font-medium',
            data.success
              ? 'bg-green-100 dark:bg-green-900/40 text-green-600 dark:text-green-400'
              : 'bg-red-100 dark:bg-red-900/40 text-red-600 dark:text-red-400',
          )}
        >
          {data.success ? t('completion.success', 'Success') : t('completion.failed', 'Failed')}
        </span>
      </div>

      {/* Stats */}
      <div className="px-3 py-2 grid grid-cols-4 gap-2">
        <div className="text-center">
          <div className="text-lg font-bold text-gray-800 dark:text-gray-200">{data.totalSteps}</div>
          <div className="text-2xs text-gray-500">{t('completion.totalSteps', 'Total')}</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-green-600 dark:text-green-400">{data.stepsCompleted}</div>
          <div className="text-2xs text-gray-500">{t('completion.completed', 'Completed')}</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-red-600 dark:text-red-400">{data.stepsFailed}</div>
          <div className="text-2xs text-gray-500">{t('completion.failed', 'Failed')}</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-gray-800 dark:text-gray-200">
            {formatDuration(data.totalDurationMs)}
          </div>
          <div className="text-2xs text-gray-500">{t('completion.duration', 'Duration')}</div>
        </div>
      </div>

      {/* Step summaries (expandable) */}
      {summaryEntries.length > 0 && (
        <div className="border-t border-gray-200 dark:border-gray-700">
          <button
            onClick={() => setShowSummaries(!showSummaries)}
            className="w-full px-3 py-1.5 flex items-center gap-1 text-2xs text-gray-500 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
          >
            {showSummaries ? <ChevronDownIcon className="w-3 h-3" /> : <ChevronRightIcon className="w-3 h-3" />}
            {t('completion.stepSummaries', 'Step Summaries')} ({summaryEntries.length})
          </button>
          {showSummaries && (
            <div className="px-3 pb-2 space-y-1.5">
              {summaryEntries.map(([stepId, summary]) => (
                <div key={stepId} className="text-2xs">
                  <span className="font-mono text-gray-400">{stepId}:</span>{' '}
                  <span className="text-gray-600 dark:text-gray-400">{summary}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
