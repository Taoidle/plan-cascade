/**
 * SummaryCard Component
 *
 * Displays execution summary statistics: total stories, passed/failed,
 * total time, tokens, and cost.
 *
 * Story 004: Execution Report visualization components
 */

import { clsx } from 'clsx';
import {
  CheckCircledIcon,
  CrossCircledIcon,
  ClockIcon,
  LightningBoltIcon,
} from '@radix-ui/react-icons';
import type { ReportSummary } from '../../store/executionReport';

// ============================================================================
// Component
// ============================================================================

interface SummaryCardProps {
  summary: ReportSummary;
}

export function SummaryCard({ summary }: SummaryCardProps) {
  const successColor = summary.successRate >= 80
    ? 'text-green-600 dark:text-green-400'
    : summary.successRate >= 50
      ? 'text-amber-600 dark:text-amber-400'
      : 'text-red-600 dark:text-red-400';

  return (
    <div
      className={clsx(
        'p-4 rounded-lg',
        'border border-gray-200 dark:border-gray-700',
        'bg-white dark:bg-gray-800'
      )}
      data-testid="summary-card"
    >
      <h3 className="text-sm font-semibold text-gray-800 dark:text-gray-200 mb-3">
        Execution Summary
      </h3>

      {/* Success rate banner */}
      <div className={clsx('text-2xl font-bold mb-4', successColor)}>
        {summary.successRate}% Success
      </div>

      {/* Stats grid */}
      <div className="grid grid-cols-2 gap-3">
        {/* Total Stories */}
        <div className="flex items-center gap-2">
          <LightningBoltIcon className="w-4 h-4 text-blue-500" />
          <div>
            <p className="text-xs text-gray-500 dark:text-gray-400">Total Stories</p>
            <p className="text-sm font-semibold text-gray-800 dark:text-gray-200">
              {summary.totalStories}
            </p>
          </div>
        </div>

        {/* Passed */}
        <div className="flex items-center gap-2">
          <CheckCircledIcon className="w-4 h-4 text-green-500" />
          <div>
            <p className="text-xs text-gray-500 dark:text-gray-400">Passed</p>
            <p className="text-sm font-semibold text-green-700 dark:text-green-300">
              {summary.storiesPassed}
            </p>
          </div>
        </div>

        {/* Failed */}
        <div className="flex items-center gap-2">
          <CrossCircledIcon className="w-4 h-4 text-red-500" />
          <div>
            <p className="text-xs text-gray-500 dark:text-gray-400">Failed</p>
            <p className="text-sm font-semibold text-red-700 dark:text-red-300">
              {summary.storiesFailed}
            </p>
          </div>
        </div>

        {/* Duration */}
        <div className="flex items-center gap-2">
          <ClockIcon className="w-4 h-4 text-gray-500" />
          <div>
            <p className="text-xs text-gray-500 dark:text-gray-400">Duration</p>
            <p className="text-sm font-semibold text-gray-800 dark:text-gray-200">
              {(summary.totalTimeMs / 1000).toFixed(1)}s
            </p>
          </div>
        </div>

        {/* Tokens (if available) */}
        {summary.totalTokens !== null && (
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 flex items-center justify-center text-xs text-purple-500 font-bold">
              T
            </div>
            <div>
              <p className="text-xs text-gray-500 dark:text-gray-400">Tokens</p>
              <p className="text-sm font-semibold text-gray-800 dark:text-gray-200">
                {summary.totalTokens.toLocaleString()}
              </p>
            </div>
          </div>
        )}

        {/* Cost (if available) */}
        {summary.estimatedCost !== null && (
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 flex items-center justify-center text-xs text-green-500 font-bold">
              $
            </div>
            <div>
              <p className="text-xs text-gray-500 dark:text-gray-400">Est. Cost</p>
              <p className="text-sm font-semibold text-gray-800 dark:text-gray-200">
                ${summary.estimatedCost.toFixed(4)}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default SummaryCard;
