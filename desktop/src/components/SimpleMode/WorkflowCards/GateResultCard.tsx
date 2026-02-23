/**
 * GateResultCard
 *
 * Displays per-story quality gate pass/fail results with individual gate details.
 */

import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { GateResultCardData } from '../../../types/workflowCard';
import type { GateStatus } from '../../../store/taskMode';

export function GateResultCard({ data }: { data: GateResultCardData }) {
  const { t } = useTranslation('simpleMode');
  const isPassed = data.overallStatus === 'passed';

  return (
    <div
      className={clsx(
        'rounded-lg border px-3 py-2',
        isPassed
          ? 'border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20'
          : 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20'
      )}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <StatusIcon status={data.overallStatus} />
          <span
            className={clsx(
              'text-xs font-medium',
              isPassed ? 'text-green-700 dark:text-green-300' : 'text-red-700 dark:text-red-300'
            )}
          >
            {t('workflow.gates.qualityGate', { title: data.storyTitle })}
          </span>
        </div>
        <span
          className={clsx(
            'text-2xs px-1.5 py-0.5 rounded font-medium',
            isPassed
              ? 'bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-400'
              : 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400'
          )}
        >
          {data.overallStatus}
        </span>
      </div>

      {/* Individual gates */}
      {data.gates.length > 0 && (
        <div className="mt-1.5 flex flex-wrap gap-1">
          {data.gates.map((gate) => (
            <span
              key={gate.gateId}
              className={clsx(
                'text-2xs px-1.5 py-0.5 rounded',
                gateStatusColor(gate.status)
              )}
              title={gate.message || gate.gateName}
            >
              {gate.gateName}: {gate.status}
            </span>
          ))}
        </div>
      )}

      {/* Code review scores */}
      {data.codeReviewScores.length > 0 && (
        <div className="mt-1.5 grid grid-cols-5 gap-1">
          {data.codeReviewScores.map((score) => (
            <div key={score.dimension} className="text-center">
              <span className="text-2xs text-gray-500 dark:text-gray-400 block truncate">
                {score.dimension}
              </span>
              <span
                className={clsx(
                  'text-xs font-medium',
                  score.score >= score.maxScore * 0.8
                    ? 'text-green-600 dark:text-green-400'
                    : score.score >= score.maxScore * 0.5
                      ? 'text-amber-600 dark:text-amber-400'
                      : 'text-red-600 dark:text-red-400'
                )}
              >
                {score.score}/{score.maxScore}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function StatusIcon({ status }: { status: GateStatus }) {
  const cls = 'w-3.5 h-3.5';
  switch (status) {
    case 'passed':
      return (
        <svg className={clsx(cls, 'text-green-500')} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
        </svg>
      );
    case 'failed':
      return (
        <svg className={clsx(cls, 'text-red-500')} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
        </svg>
      );
    default:
      return (
        <svg className={clsx(cls, 'text-gray-400')} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="10" strokeWidth={2} />
        </svg>
      );
  }
}

function gateStatusColor(status: GateStatus): string {
  switch (status) {
    case 'passed': return 'bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-400';
    case 'failed': return 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400';
    case 'running': return 'bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400';
    case 'skipped': return 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400';
    default: return 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400';
  }
}
