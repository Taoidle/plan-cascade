/**
 * CheckpointNode Component
 *
 * Individual checkpoint display with expand/collapse functionality.
 * Shows timestamp, label, and action buttons for restore, fork, compare.
 */

import { useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  ClockIcon,
  FileIcon,
  ChevronDownIcon,
  ChevronUpIcon,
  CounterClockwiseClockIcon,
  Link2Icon,
  MixIcon,
} from '@radix-ui/react-icons';
import type { Checkpoint } from '../../types/timeline';

interface CheckpointNodeProps {
  checkpoint: Checkpoint;
  isSelected: boolean;
  isCurrent: boolean;
  isFirst: boolean;
  onClick: () => void;
  onFork: () => void;
  onRestore: () => void;
  onCompare?: () => void;
}

export function CheckpointNode({
  checkpoint,
  isSelected,
  isCurrent,
  isFirst,
  onClick,
  onFork,
  onRestore,
  onCompare,
}: CheckpointNodeProps) {
  const { t } = useTranslation();
  const [isExpanded, setIsExpanded] = useState(false);

  // Format timestamp
  const formatTime = useCallback((isoString: string) => {
    const date = new Date(isoString);
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (minutes < 1) return t('time.justNow');
    if (minutes < 60) return t('time.minutesAgo', { count: minutes });
    if (hours < 24) return t('time.hoursAgo', { count: hours });
    return t('time.daysAgo', { count: days });
  }, [t]);

  // Format full timestamp
  const formatFullTime = useCallback((isoString: string) => {
    const date = new Date(isoString);
    return date.toLocaleString();
  }, []);

  const handleToggleExpand = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      setIsExpanded(!isExpanded);
    },
    [isExpanded]
  );

  const handleFork = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onFork();
    },
    [onFork]
  );

  const handleRestore = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onRestore();
    },
    [onRestore]
  );

  const handleCompare = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onCompare?.();
    },
    [onCompare]
  );

  return (
    <div className="flex items-start gap-4 relative">
      {/* Timeline node */}
      <div className="flex-shrink-0 relative z-10">
        <div
          className={clsx(
            'w-4 h-4 rounded-full border-2 transition-colors',
            isCurrent
              ? 'bg-primary-500 border-primary-500'
              : isSelected
              ? 'bg-primary-100 dark:bg-primary-900 border-primary-400'
              : 'bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600'
          )}
        />
        {isCurrent && (
          <div className="absolute -left-1 -top-1 w-6 h-6 rounded-full bg-primary-500/20 animate-ping" />
        )}
      </div>

      {/* Checkpoint card */}
      <div
        onClick={onClick}
        className={clsx(
          'flex-1 rounded-lg border transition-all cursor-pointer',
          isSelected
            ? 'bg-primary-50 dark:bg-primary-900/30 border-primary-300 dark:border-primary-700 shadow-sm'
            : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600',
          isExpanded ? 'shadow-md' : ''
        )}
      >
        {/* Compact view */}
        <div className="p-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 min-w-0">
              {/* Current indicator */}
              {isCurrent && (
                <span
                  className={clsx(
                    'flex-shrink-0 px-2 py-0.5 text-xs font-medium rounded',
                    'bg-primary-100 dark:bg-primary-900',
                    'text-primary-700 dark:text-primary-300'
                  )}
                >
                  {t('timeline.current')}
                </span>
              )}

              {/* Label */}
              <span
                className={clsx(
                  'font-medium truncate',
                  isSelected
                    ? 'text-primary-700 dark:text-primary-300'
                    : 'text-gray-900 dark:text-white'
                )}
              >
                {checkpoint.label}
              </span>

              {/* Branch indicator */}
              {checkpoint.branch_id && !isFirst && (
                <Link2Icon className="w-3 h-3 text-gray-400 flex-shrink-0" />
              )}
            </div>

            {/* Expand button */}
            <button
              onClick={handleToggleExpand}
              className={clsx(
                'p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-700',
                'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                'transition-colors'
              )}
            >
              {isExpanded ? (
                <ChevronUpIcon className="w-4 h-4" />
              ) : (
                <ChevronDownIcon className="w-4 h-4" />
              )}
            </button>
          </div>

          {/* Stats row */}
          <div className="flex items-center gap-3 mt-2 text-xs text-gray-500 dark:text-gray-400">
            <span className="flex items-center gap-1" title={formatFullTime(checkpoint.timestamp)}>
              <ClockIcon className="w-3 h-3" />
              {formatTime(checkpoint.timestamp)}
            </span>
            <span className="flex items-center gap-1">
              <FileIcon className="w-3 h-3" />
              {checkpoint.files_snapshot.length} {t('timeline.files')}
            </span>
          </div>
        </div>

        {/* Expanded view */}
        {isExpanded && (
          <div className="border-t border-gray-200 dark:border-gray-700 p-3">
            {/* Description */}
            {checkpoint.description && (
              <p className="text-sm text-gray-600 dark:text-gray-400 mb-3">
                {checkpoint.description}
              </p>
            )}

            {/* File list preview */}
            {checkpoint.files_snapshot.length > 0 && (
              <div className="mb-3">
                <p className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                  {t('timeline.trackedFiles')}
                </p>
                <ul className="text-xs text-gray-600 dark:text-gray-400 space-y-0.5 max-h-24 overflow-y-auto">
                  {checkpoint.files_snapshot.slice(0, 5).map((file) => (
                    <li key={file.path} className="flex items-center gap-1 truncate">
                      <FileIcon className="w-3 h-3 flex-shrink-0" />
                      <span className="truncate">{file.path}</span>
                    </li>
                  ))}
                  {checkpoint.files_snapshot.length > 5 && (
                    <li className="text-gray-400">
                      +{checkpoint.files_snapshot.length - 5} {t('timeline.moreFiles')}
                    </li>
                  )}
                </ul>
              </div>
            )}

            {/* Action buttons */}
            <div className="flex items-center gap-2">
              <button
                onClick={handleRestore}
                className={clsx(
                  'flex items-center gap-1 px-2.5 py-1.5 rounded-md text-xs font-medium',
                  'bg-orange-100 dark:bg-orange-900/50',
                  'text-orange-700 dark:text-orange-300',
                  'hover:bg-orange-200 dark:hover:bg-orange-800',
                  'transition-colors'
                )}
              >
                <CounterClockwiseClockIcon className="w-3 h-3" />
                {t('timeline.restore')}
              </button>

              <button
                onClick={handleFork}
                className={clsx(
                  'flex items-center gap-1 px-2.5 py-1.5 rounded-md text-xs font-medium',
                  'bg-purple-100 dark:bg-purple-900/50',
                  'text-purple-700 dark:text-purple-300',
                  'hover:bg-purple-200 dark:hover:bg-purple-800',
                  'transition-colors'
                )}
              >
                <Link2Icon className="w-3 h-3" />
                {t('timeline.fork')}
              </button>

              {onCompare && (
                <button
                  onClick={handleCompare}
                  className={clsx(
                    'flex items-center gap-1 px-2.5 py-1.5 rounded-md text-xs font-medium',
                    'bg-blue-100 dark:bg-blue-900/50',
                    'text-blue-700 dark:text-blue-300',
                    'hover:bg-blue-200 dark:hover:bg-blue-800',
                    'transition-colors'
                  )}
                >
                  <MixIcon className="w-3 h-3" />
                  {t('timeline.compare')}
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default CheckpointNode;
