/**
 * CompletionReportCard
 *
 * Final summary with metrics showing success/failure counts, duration, and agent assignments.
 */

import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { CompletionReportCardData } from '../../../types/workflowCard';

export function CompletionReportCard({ data }: { data: CompletionReportCardData }) {
  const { t } = useTranslation('simpleMode');
  const agentEntries = Object.entries(data.agentAssignments);
  const durationStr = data.duration > 0 ? formatDuration(data.duration) : null;

  return (
    <div
      className={clsx(
        'rounded-lg border overflow-hidden',
        data.success
          ? 'border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20'
          : 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20',
      )}
    >
      {/* Header */}
      <div
        className={clsx(
          'px-3 py-2 border-b',
          data.success
            ? 'bg-green-100/50 dark:bg-green-900/30 border-green-200 dark:border-green-800'
            : 'bg-red-100/50 dark:bg-red-900/30 border-red-200 dark:border-red-800',
        )}
      >
        <div className="flex items-center gap-2">
          {data.success ? (
            <svg className="w-4 h-4 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
          ) : (
            <svg className="w-4 h-4 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
          )}
          <span
            className={clsx(
              'text-sm font-semibold',
              data.success ? 'text-green-700 dark:text-green-300' : 'text-red-700 dark:text-red-300',
            )}
          >
            {data.success ? t('workflow.report.executionComplete') : t('workflow.report.executionFailed')}
          </span>
        </div>
      </div>

      <div className="px-3 py-2 space-y-2">
        {/* Metrics grid */}
        <div className="grid grid-cols-4 gap-2">
          <MetricBox
            label={t('workflow.report.total')}
            value={data.totalStories}
            color="text-gray-700 dark:text-gray-300"
          />
          <MetricBox
            label={t('workflow.report.completed')}
            value={data.completed}
            color="text-green-600 dark:text-green-400"
          />
          <MetricBox label={t('workflow.report.failed')} value={data.failed} color="text-red-600 dark:text-red-400" />
          {durationStr && (
            <MetricBox
              label={t('workflow.report.duration')}
              value={durationStr}
              color="text-blue-600 dark:text-blue-400"
            />
          )}
        </div>

        {/* Agent assignments */}
        {agentEntries.length > 0 && (
          <div>
            <span className="text-2xs font-medium text-gray-500 dark:text-gray-400">
              {t('workflow.report.agentAssignments')}
            </span>
            <div className="mt-1 flex flex-wrap gap-1">
              {agentEntries.map(([storyId, agent]) => (
                <span
                  key={storyId}
                  className="text-2xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400"
                >
                  {storyId}: {agent}
                </span>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function MetricBox({ label, value, color }: { label: string; value: number | string; color: string }) {
  return (
    <div className="text-center">
      <span className="text-2xs text-gray-500 dark:text-gray-400 block">{label}</span>
      <span className={clsx('text-sm font-semibold', color)}>{value}</span>
    </div>
  );
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) return `${minutes}m ${remainingSeconds}s`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h ${remainingMinutes}m`;
}
