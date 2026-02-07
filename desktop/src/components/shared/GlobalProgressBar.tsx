/**
 * GlobalProgressBar Component
 *
 * Renders at the top of the execution view showing stories completed vs total
 * with batch-level progress granularity and estimated time remaining.
 *
 * Story 008: Real-time Execution Feedback
 */

import { useMemo } from 'react';
import { clsx } from 'clsx';
import {
  CheckIcon,
  Cross2Icon,
  UpdateIcon,
  ClockIcon,
} from '@radix-ui/react-icons';
import { useExecutionStore } from '../../store/execution';

// ============================================================================
// Types
// ============================================================================

interface GlobalProgressBarProps {
  /** Additional class names */
  className?: string;
  /** Show story labels below the bar */
  showStoryLabels?: boolean;
  /** Compact variant (for SimpleMode) */
  compact?: boolean;
}

// ============================================================================
// Helpers
// ============================================================================

function formatTimeRemaining(ms: number): string {
  if (ms <= 0) return '0s';
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    return `~${hours}h ${minutes % 60}m`;
  }
  if (minutes > 0) {
    return `~${minutes}m ${seconds % 60}s`;
  }
  return `~${seconds}s`;
}

function formatElapsed(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    return `${hours}h ${minutes % 60}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  }
  return `${seconds}s`;
}

// ============================================================================
// Story Status Icon
// ============================================================================

function StoryStatusIcon({ status }: { status: string }) {
  switch (status) {
    case 'completed':
      return <CheckIcon className="w-3 h-3 text-success-500" />;
    case 'failed':
      return <Cross2Icon className="w-3 h-3 text-error-500" />;
    case 'in_progress':
      return <UpdateIcon className="w-3 h-3 text-primary-500 animate-spin" />;
    default:
      return <div className="w-3 h-3 rounded-full border border-gray-400 dark:border-gray-600" />;
  }
}

// ============================================================================
// GlobalProgressBar Component
// ============================================================================

export function GlobalProgressBar({
  className,
  showStoryLabels = true,
  compact = false,
}: GlobalProgressBarProps) {
  const {
    stories,
    batches,
    currentBatch,
    progress,
    status,
    startedAt,
    estimatedTimeRemaining,
  } = useExecutionStore();

  const completedCount = useMemo(
    () => stories.filter((s) => s.status === 'completed').length,
    [stories]
  );

  const failedCount = useMemo(
    () => stories.filter((s) => s.status === 'failed').length,
    [stories]
  );

  const totalCount = stories.length;

  const elapsed = useMemo(() => {
    if (!startedAt) return 0;
    return Date.now() - startedAt;
  }, [startedAt, progress]); // Re-compute when progress changes

  const totalBatches = useMemo(() => {
    if (batches.length === 0) return 1;
    return Math.max(...batches.map((b) => b.totalBatches), 1);
  }, [batches]);

  // Compute per-batch segment widths for the progress bar
  const batchSegments = useMemo(() => {
    if (batches.length === 0 || totalCount === 0) {
      return [{ width: 100, status: 'pending' as const, batchNum: 1 }];
    }

    return Array.from({ length: totalBatches }, (_, idx) => {
      const batchNum = idx + 1;
      const batch = batches.find((b) => b.batchNum === batchNum);
      const batchStories = batch ? batch.storyIds.length : 0;
      const width = totalCount > 0 ? (batchStories / totalCount) * 100 : 100 / totalBatches;

      let batchStatus: 'pending' | 'in_progress' | 'completed' | 'failed' = 'pending';
      if (batch) {
        batchStatus = batch.status;
      } else if (batchNum < currentBatch) {
        batchStatus = 'completed';
      } else if (batchNum === currentBatch) {
        batchStatus = 'in_progress';
      }

      return { width, status: batchStatus, batchNum };
    });
  }, [batches, totalBatches, totalCount, currentBatch]);

  if (totalCount === 0) return null;

  const percentage = Math.round(progress);
  const isRunning = status === 'running';
  const isComplete = status === 'completed';
  const isFailed = status === 'failed';

  return (
    <div className={clsx('w-full', className)}>
      {/* Header stats */}
      <div className={clsx(
        'flex items-center justify-between mb-2',
        compact ? 'text-xs' : 'text-sm'
      )}>
        <div className="flex items-center gap-3">
          <span className="font-semibold text-gray-900 dark:text-white">
            {completedCount}/{totalCount} stories
          </span>
          {failedCount > 0 && (
            <span className="text-error-500 font-medium">
              ({failedCount} failed)
            </span>
          )}
          {batches.length > 1 && (
            <span className="text-gray-500 dark:text-gray-400">
              Batch {currentBatch}/{totalBatches}
            </span>
          )}
        </div>

        <div className="flex items-center gap-3">
          {/* Elapsed time */}
          {startedAt && (
            <span className="text-gray-500 dark:text-gray-400 flex items-center gap-1">
              <ClockIcon className="w-3.5 h-3.5" />
              {formatElapsed(elapsed)}
            </span>
          )}

          {/* Estimated time remaining */}
          {isRunning && estimatedTimeRemaining !== null && estimatedTimeRemaining > 0 && (
            <span className="text-gray-500 dark:text-gray-400">
              ETA: {formatTimeRemaining(estimatedTimeRemaining)}
            </span>
          )}

          {/* Percentage */}
          <span className={clsx(
            'font-bold',
            isComplete ? 'text-success-600 dark:text-success-400' :
            isFailed ? 'text-error-600 dark:text-error-400' :
            'text-primary-600 dark:text-primary-400'
          )}>
            {percentage}%
          </span>
        </div>
      </div>

      {/* Progress bar with batch segments */}
      <div className={clsx(
        'w-full rounded-full overflow-hidden',
        'bg-gray-200 dark:bg-gray-700',
        compact ? 'h-2' : 'h-3'
      )}>
        <div className="flex h-full" style={{ width: `${Math.min(progress, 100)}%` }}>
          {batchSegments.map((segment) => {
            const segColor =
              segment.status === 'completed' ? 'bg-success-500' :
              segment.status === 'failed' ? 'bg-error-500' :
              segment.status === 'in_progress' ? 'bg-primary-500' :
              'bg-gray-400 dark:bg-gray-500';

            return (
              <div
                key={segment.batchNum}
                className={clsx(
                  'h-full transition-all duration-500 ease-out',
                  segColor,
                  segment.status === 'in_progress' && isRunning && 'animate-pulse'
                )}
                style={{
                  width: `${segment.width}%`,
                  minWidth: segment.status !== 'pending' ? '2px' : 0,
                }}
              />
            );
          })}
        </div>
      </div>

      {/* Batch indicators (dots below progress bar) */}
      {batches.length > 1 && !compact && (
        <div className="flex items-center justify-center gap-1.5 mt-2">
          {batchSegments.map((segment) => (
            <div
              key={segment.batchNum}
              className={clsx(
                'w-2 h-2 rounded-full transition-all duration-300',
                segment.status === 'completed' && 'bg-success-500',
                segment.status === 'failed' && 'bg-error-500',
                segment.status === 'in_progress' && 'bg-primary-500 animate-pulse ring-2 ring-primary-300 dark:ring-primary-700',
                segment.status === 'pending' && 'bg-gray-300 dark:bg-gray-600'
              )}
              title={`Batch ${segment.batchNum}: ${segment.status}`}
            />
          ))}
        </div>
      )}

      {/* Story labels below the bar */}
      {showStoryLabels && !compact && stories.length <= 12 && (
        <div className={clsx(
          'flex flex-wrap gap-2 mt-3',
          stories.length > 6 ? 'text-2xs' : 'text-xs'
        )}>
          {stories.map((story) => (
            <div
              key={story.id}
              className={clsx(
                'flex items-center gap-1 px-2 py-0.5 rounded-full',
                'border transition-all duration-200',
                story.status === 'completed' && 'bg-success-50 dark:bg-success-950 border-success-200 dark:border-success-800',
                story.status === 'failed' && 'bg-error-50 dark:bg-error-950 border-error-200 dark:border-error-800',
                story.status === 'in_progress' && 'bg-primary-50 dark:bg-primary-950 border-primary-200 dark:border-primary-800 ring-1 ring-primary-300 dark:ring-primary-700',
                story.status === 'pending' && 'bg-gray-50 dark:bg-gray-900 border-gray-200 dark:border-gray-700'
              )}
            >
              <StoryStatusIcon status={story.status} />
              <span className={clsx(
                'truncate max-w-[120px]',
                story.status === 'completed' && 'text-success-700 dark:text-success-300',
                story.status === 'failed' && 'text-error-700 dark:text-error-300',
                story.status === 'in_progress' && 'text-primary-700 dark:text-primary-300',
                story.status === 'pending' && 'text-gray-500 dark:text-gray-400'
              )}>
                {story.title}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default GlobalProgressBar;
