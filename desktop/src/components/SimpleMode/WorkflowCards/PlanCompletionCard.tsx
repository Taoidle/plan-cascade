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
  const [showFailures, setShowFailures] = useState(false);
  const [showConclusion, setShowConclusion] = useState(true);
  const summaryEntries = Object.entries(data.stepSummaries);
  const failureEntries = Object.entries(data.failureReasons ?? {});
  const highlightEntries = data.highlights ?? [];
  const nextActions = data.nextActions ?? [];
  const finalConclusion = data.finalConclusionMarkdown?.trim() ?? '';
  const terminalState = data.terminalState ?? data.terminalStatus ?? (data.success ? 'completed' : 'failed');
  const statusLabel =
    terminalState === 'cancelled'
      ? t('completion.cancelled', 'Cancelled')
      : terminalState === 'needs_review'
        ? t('completion.needsReview', 'Needs Review')
        : terminalState === 'completed_with_warnings'
          ? t('completion.completedWithWarnings', 'Completed with warnings')
          : data.success
            ? t('completion.success', 'Success')
            : t('completion.failed', 'Failed');

  return (
    <div
      className={clsx(
        'rounded-lg border',
        terminalState === 'cancelled'
          ? 'border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20'
          : data.success
            ? 'border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20'
            : 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20',
      )}
    >
      {/* Header */}
      <div
        className={clsx(
          'px-3 py-2 border-b flex items-center justify-between',
          terminalState === 'cancelled'
            ? 'bg-amber-100/50 dark:bg-amber-900/30 border-amber-200 dark:border-amber-800'
            : data.success
              ? 'bg-green-100/50 dark:bg-green-900/30 border-green-200 dark:border-green-800'
              : 'bg-red-100/50 dark:bg-red-900/30 border-red-200 dark:border-red-800',
        )}
      >
        <div className="flex items-center gap-2">
          <span className="text-sm">
            {terminalState === 'cancelled' ? '\u26A0' : data.success ? '\u2705' : '\u274C'}
          </span>
          <span
            className={clsx(
              'text-xs font-semibold',
              terminalState === 'cancelled'
                ? 'text-amber-700 dark:text-amber-300'
                : data.success
                  ? 'text-green-700 dark:text-green-300'
                  : 'text-red-700 dark:text-red-300',
            )}
          >
            {data.planTitle}
          </span>
        </div>
        <span
          className={clsx(
            'text-2xs px-1.5 py-0.5 rounded font-medium',
            terminalState === 'cancelled'
              ? 'bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300'
              : data.success
                ? 'bg-green-100 dark:bg-green-900/40 text-green-600 dark:text-green-400'
                : 'bg-red-100 dark:bg-red-900/40 text-red-600 dark:text-red-400',
          )}
        >
          {statusLabel}
        </span>
      </div>

      {/* Stats */}
      <div className="px-3 py-2 grid grid-cols-3 md:grid-cols-6 gap-2">
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
          <div className="text-lg font-bold text-amber-600 dark:text-amber-400">{data.stepsSoftFailed ?? 0}</div>
          <div className="text-2xs text-gray-500">{t('completion.softFailed', 'Warnings')}</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-blue-600 dark:text-blue-400">{data.stepsNeedsReview ?? 0}</div>
          <div className="text-2xs text-gray-500">{t('completion.needsReview', 'Review')}</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-amber-600 dark:text-amber-400">{data.stepsCancelled ?? 0}</div>
          <div className="text-2xs text-gray-500">{t('completion.cancelledSteps', 'Cancelled')}</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-blue-600 dark:text-blue-400">{data.stepsAttempted ?? 0}</div>
          <div className="text-2xs text-gray-500">{t('completion.attempted', 'Attempted')}</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-gray-800 dark:text-gray-200">
            {formatDuration(data.totalDurationMs)}
          </div>
          <div className="text-2xs text-gray-500">{t('completion.duration', 'Duration')}</div>
        </div>
      </div>

      {(finalConclusion.length > 0 || highlightEntries.length > 0 || nextActions.length > 0) && (
        <div className="border-t border-gray-200 dark:border-gray-700">
          <button
            onClick={() => setShowConclusion(!showConclusion)}
            className="w-full px-3 py-1.5 flex items-center gap-1 text-2xs text-gray-500 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
          >
            {showConclusion ? <ChevronDownIcon className="w-3 h-3" /> : <ChevronRightIcon className="w-3 h-3" />}
            {t('completion.finalConclusion', 'Final Conclusion')}
          </button>
          {showConclusion && (
            <div className="px-3 pb-2 space-y-2">
              {finalConclusion.length > 0 && (
                <div className="text-2xs text-gray-600 dark:text-gray-300 whitespace-pre-wrap break-words bg-white/80 dark:bg-gray-900/40 rounded p-2">
                  {finalConclusion}
                </div>
              )}
              {highlightEntries.length > 0 && (
                <div className="space-y-1">
                  <div className="text-2xs font-medium text-gray-500">{t('completion.highlights', 'Highlights')}</div>
                  {highlightEntries.map((item, index) => (
                    <div key={`highlight-${index}`} className="text-2xs text-gray-600 dark:text-gray-400">
                      - {item}
                    </div>
                  ))}
                </div>
              )}
              {nextActions.length > 0 && (
                <div className="space-y-1">
                  <div className="text-2xs font-medium text-gray-500">
                    {t('completion.nextActions', 'Next Actions')}
                  </div>
                  {nextActions.map((item, index) => (
                    <div key={`next-${index}`} className="text-2xs text-gray-600 dark:text-gray-400">
                      {index + 1}. {item}
                    </div>
                  ))}
                </div>
              )}
              {data.retryStats && (
                <div className="text-2xs text-gray-500">
                  {t('completion.retryStats', 'Retry stats')}: {data.retryStats.totalRetries}/
                  {data.retryStats.stepsRetried}/{data.retryStats.exhaustedFailures}
                </div>
              )}
              {terminalState === 'cancelled' && typeof data.stepsFailedBeforeCancel === 'number' && (
                <div className="text-2xs text-amber-700 dark:text-amber-300">
                  {t('completion.failedBeforeCancel', 'Failed before cancel')}: {data.stepsFailedBeforeCancel}
                </div>
              )}
              {data.terminalVerdictTrace && data.terminalVerdictTrace.length > 0 && (
                <div className="space-y-1">
                  <div className="text-2xs font-medium text-gray-500">
                    {t('completion.verdictTrace', 'Verdict trace')}
                  </div>
                  {data.terminalVerdictTrace.map((item, index) => (
                    <div key={`trace-${index}`} className="text-2xs text-gray-600 dark:text-gray-400">
                      - {item}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      )}

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
      {failureEntries.length > 0 && (
        <div className="border-t border-gray-200 dark:border-gray-700">
          <button
            onClick={() => setShowFailures(!showFailures)}
            className="w-full px-3 py-1.5 flex items-center gap-1 text-2xs text-gray-500 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
          >
            {showFailures ? <ChevronDownIcon className="w-3 h-3" /> : <ChevronRightIcon className="w-3 h-3" />}
            {t('completion.failureReasons', 'Failure Reasons')} ({failureEntries.length})
          </button>
          {showFailures && (
            <div className="px-3 pb-2 space-y-1.5">
              {failureEntries.map(([stepId, reason]) => (
                <div key={stepId} className="text-2xs">
                  <span className="font-mono text-gray-400">{stepId}:</span>{' '}
                  <span className="text-gray-600 dark:text-gray-400">{reason}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
