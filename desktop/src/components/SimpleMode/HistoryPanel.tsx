/**
 * HistoryPanel Component
 *
 * Displays recent task executions with timestamp, description,
 * status, and duration. Allows clearing history.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  Cross1Icon,
  TrashIcon,
  CheckCircledIcon,
  CrossCircledIcon,
  ClockIcon,
} from '@radix-ui/react-icons';
import { useExecutionStore } from '../../store/execution';
import type { ExecutionHistoryItem } from '../../store/execution';

interface HistoryPanelProps {
  onClose: () => void;
}

export function HistoryPanel({ onClose }: HistoryPanelProps) {
  const { t } = useTranslation('simpleMode');
  const { history, clearHistory } = useExecutionStore();

  return (
    <div className="max-w-2xl mx-auto w-full animate-fade-in">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
          {t('history.title')}
        </h2>
        <div className="flex items-center gap-2">
          {history.length > 0 && (
            <button
              onClick={clearHistory}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm',
                'text-red-600 dark:text-red-400',
                'hover:bg-red-50 dark:hover:bg-red-900/20',
                'transition-colors'
              )}
            >
              <TrashIcon className="w-4 h-4" />
              {t('buttons.clear', { ns: 'common' })}
            </button>
          )}
          <button
            onClick={onClose}
            className={clsx(
              'p-1.5 rounded-lg',
              'text-gray-500 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              'transition-colors'
            )}
          >
            <Cross1Icon className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* History List */}
      {history.length === 0 ? (
        <div className="text-center py-12">
          <ClockIcon className="w-12 h-12 mx-auto text-gray-300 dark:text-gray-600 mb-3" />
          <p className="text-gray-500 dark:text-gray-400">
            {t('history.empty.title')}
          </p>
          <p className="text-sm text-gray-400 dark:text-gray-500 mt-1">
            {t('history.empty.subtitle')}
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {history.map((item) => (
            <HistoryItem key={item.id} item={item} />
          ))}
        </div>
      )}
    </div>
  );
}

interface HistoryItemProps {
  item: ExecutionHistoryItem;
}

function HistoryItem({ item }: HistoryItemProps) {
  const { t } = useTranslation('simpleMode');

  const getStatusIcon = () => {
    if (item.success) {
      return <CheckCircledIcon className="w-5 h-5 text-green-500" />;
    }
    return <CrossCircledIcon className="w-5 h-5 text-red-500" />;
  };

  const formatDate = (timestamp: number): string => {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    // Less than 1 minute
    if (diff < 60000) {
      return 'Just now';
    }

    // Less than 1 hour
    if (diff < 3600000) {
      const minutes = Math.floor(diff / 60000);
      return `${minutes}m ago`;
    }

    // Less than 24 hours
    if (diff < 86400000) {
      const hours = Math.floor(diff / 3600000);
      return `${hours}h ago`;
    }

    // Less than 7 days
    if (diff < 604800000) {
      const days = Math.floor(diff / 86400000);
      return `${days}d ago`;
    }

    // Full date
    return date.toLocaleDateString(undefined, {
      month: 'short',
      day: 'numeric',
      year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
    });
  };

  const formatDuration = (ms: number): string => {
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
  };

  const formatStrategy = (strategy: string | null): string => {
    if (!strategy) return t('strategies.auto');
    switch (strategy) {
      case 'direct':
        return t('strategies.direct');
      case 'hybrid_auto':
        return t('strategies.hybrid');
      case 'mega_plan':
        return t('strategies.mega');
      default:
        return strategy;
    }
  };

  return (
    <div
      className={clsx(
        'p-4 rounded-lg',
        'bg-white dark:bg-gray-800',
        'border border-gray-200 dark:border-gray-700',
        'hover:border-gray-300 dark:hover:border-gray-600',
        'transition-colors'
      )}
    >
      <div className="flex items-start gap-3">
        {getStatusIcon()}

        <div className="flex-1 min-w-0">
          <p className="font-medium text-gray-900 dark:text-white truncate">
            {item.taskDescription}
          </p>

          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 mt-1 text-sm text-gray-500 dark:text-gray-400">
            <span>{formatDate(item.startedAt)}</span>
            <span>{formatDuration(item.duration)}</span>
            <span className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-xs">
              {formatStrategy(item.strategy)}
            </span>
          </div>

          {/* Story counts */}
          <div className="flex items-center gap-2 mt-2 text-sm">
            <span className="text-green-600 dark:text-green-400">
              {item.completedStories} {t('history.completed')}
            </span>
            {item.totalStories > item.completedStories && (
              <span className="text-red-600 dark:text-red-400">
                {item.totalStories - item.completedStories} {t('history.failed')}
              </span>
            )}
          </div>

          {/* Error preview */}
          {item.error && (
            <p className="mt-2 text-sm text-red-500 dark:text-red-400 truncate">
              {item.error}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

export default HistoryPanel;
