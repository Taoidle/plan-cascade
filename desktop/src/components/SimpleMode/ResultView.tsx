/**
 * ResultView Component
 *
 * Displays execution results in Simple mode.
 * Shows success/failure status, summary, and detailed story breakdown.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  CheckCircledIcon,
  CrossCircledIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  CheckIcon,
  Cross2Icon,
  ClockIcon,
} from '@radix-ui/react-icons';
import { useExecutionStore } from '../../store/execution';
import type { Story } from '../../store/execution';

interface ResultViewProps {
  result: {
    success: boolean;
    message: string;
    completedStories: number;
    totalStories: number;
    duration: number;
    error?: string;
  } | null;
}

export function ResultView({ result }: ResultViewProps) {
  const { t } = useTranslation('simpleMode');
  const { stories } = useExecutionStore();
  const [expandedStories, setExpandedStories] = useState<Set<string>>(new Set());

  if (!result) return null;

  const { success, message, completedStories, totalStories, duration, error } = result;
  const failedStories = stories.filter((s) => s.status === 'failed');
  const successRate = totalStories > 0 ? Math.round((completedStories / totalStories) * 100) : 0;

  const toggleStory = (storyId: string) => {
    setExpandedStories((prev) => {
      const next = new Set(prev);
      if (next.has(storyId)) {
        next.delete(storyId);
      } else {
        next.add(storyId);
      }
      return next;
    });
  };

  return (
    <div className="max-w-2xl 3xl:max-w-3xl 5xl:max-w-4xl mx-auto w-full space-y-6">
      {/* Main Result Card */}
      <div
        className={clsx(
          'p-6 rounded-xl',
          'bg-white dark:bg-gray-800',
          'border-2',
          success ? 'border-green-200 dark:border-green-800' : 'border-red-200 dark:border-red-800',
          'animate-fade-in',
        )}
      >
        {/* Status Icon */}
        <div className="flex justify-center mb-4">
          {success ? (
            <CheckCircledIcon className="w-16 h-16 text-green-500 animate-scale-in" />
          ) : (
            <CrossCircledIcon className="w-16 h-16 text-red-500 animate-scale-in" />
          )}
        </div>

        {/* Status Message */}
        <h2
          className={clsx(
            'text-xl font-semibold text-center mb-2',
            success ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400',
          )}
        >
          {success ? t('result.completedSuccessfully') : t('result.executionFailed')}
        </h2>

        <p className="text-center text-gray-600 dark:text-gray-400 mb-6">{message}</p>

        {/* Stats */}
        <div className="grid grid-cols-3 gap-4 text-center">
          <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-900">
            <div className="text-2xl font-bold text-gray-900 dark:text-white">
              {completedStories}/{totalStories}
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400">{t('labels.stories', { ns: 'common' })}</div>
          </div>
          <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-900">
            <div className="text-2xl font-bold text-gray-900 dark:text-white">{formatDuration(duration)}</div>
            <div className="text-sm text-gray-500 dark:text-gray-400">{t('labels.duration', { ns: 'common' })}</div>
          </div>
          <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-900">
            <div
              className={clsx(
                'text-2xl font-bold',
                successRate >= 80
                  ? 'text-green-600 dark:text-green-400'
                  : successRate >= 50
                    ? 'text-yellow-600 dark:text-yellow-400'
                    : 'text-red-600 dark:text-red-400',
              )}
            >
              {successRate}%
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400">{t('labels.successRate', { ns: 'common' })}</div>
          </div>
        </div>

        {/* Error Details */}
        {error && (
          <div className="mt-4 p-4 rounded-lg bg-red-50 dark:bg-red-900/20">
            <h3 className="font-medium text-red-600 dark:text-red-400 mb-1">{t('result.errorDetails')}</h3>
            <pre className="text-sm text-red-500 dark:text-red-300 whitespace-pre-wrap font-mono">{error}</pre>
          </div>
        )}
      </div>

      {/* Story Breakdown */}
      {stories.length > 0 && (
        <div className="space-y-3">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">{t('result.storyBreakdown')}</h3>

          {stories.map((story) => (
            <StoryResultItem
              key={story.id}
              story={story}
              isExpanded={expandedStories.has(story.id)}
              onToggle={() => toggleStory(story.id)}
            />
          ))}
        </div>
      )}

      {/* Failed Stories Summary */}
      {failedStories.length > 0 && (
        <div className="p-4 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
          <h3 className="font-medium text-red-600 dark:text-red-400 mb-2">
            {t('result.failedStories', { count: failedStories.length })}
          </h3>
          <ul className="space-y-1">
            {failedStories.map((story) => (
              <li key={story.id} className="text-sm text-red-500 dark:text-red-300">
                <span className="font-medium">{story.title}</span>
                {story.error && <span className="text-red-400 dark:text-red-400"> - {story.error}</span>}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

interface StoryResultItemProps {
  story: Story;
  isExpanded: boolean;
  onToggle: () => void;
}

function StoryResultItem({ story, isExpanded, onToggle }: StoryResultItemProps) {
  const getStatusIcon = () => {
    switch (story.status) {
      case 'completed':
        return <CheckIcon className="w-4 h-4 text-green-500" />;
      case 'failed':
        return <Cross2Icon className="w-4 h-4 text-red-500" />;
      default:
        return <ClockIcon className="w-4 h-4 text-gray-400" />;
    }
  };

  const getStatusColor = () => {
    switch (story.status) {
      case 'completed':
        return 'border-l-green-500';
      case 'failed':
        return 'border-l-red-500';
      default:
        return 'border-l-gray-300 dark:border-l-gray-600';
    }
  };

  const hasDuration = story.startedAt && story.completedAt;
  const storyDuration = hasDuration ? new Date(story.completedAt!).getTime() - new Date(story.startedAt!).getTime() : 0;

  return (
    <div
      className={clsx(
        'rounded-lg overflow-hidden',
        'bg-white dark:bg-gray-800',
        'border border-gray-200 dark:border-gray-700',
        'border-l-4',
        getStatusColor(),
        'transition-all duration-200',
      )}
    >
      <button
        onClick={onToggle}
        className={clsx(
          'w-full flex items-center gap-3 p-3',
          'hover:bg-gray-50 dark:hover:bg-gray-700/50',
          'transition-colors',
        )}
      >
        {isExpanded ? (
          <ChevronDownIcon className="w-4 h-4 text-gray-400" />
        ) : (
          <ChevronRightIcon className="w-4 h-4 text-gray-400" />
        )}

        {getStatusIcon()}

        <span className="flex-1 text-left font-medium text-gray-900 dark:text-white truncate">{story.title}</span>

        {hasDuration && (
          <span className="text-xs text-gray-500 dark:text-gray-400">{formatDuration(storyDuration)}</span>
        )}
      </button>

      {isExpanded && (
        <div className="px-4 pb-3 pt-1 border-t border-gray-100 dark:border-gray-700 animate-slide-down">
          <dl className="space-y-2 text-sm">
            <div className="flex justify-between">
              <dt className="text-gray-500 dark:text-gray-400">Status</dt>
              <dd
                className={clsx(
                  'font-medium',
                  story.status === 'completed' && 'text-green-600 dark:text-green-400',
                  story.status === 'failed' && 'text-red-600 dark:text-red-400',
                  story.status === 'pending' && 'text-gray-600 dark:text-gray-400',
                )}
              >
                {story.status.charAt(0).toUpperCase() + story.status.slice(1).replace('_', ' ')}
              </dd>
            </div>

            {hasDuration && (
              <div className="flex justify-between">
                <dt className="text-gray-500 dark:text-gray-400">Duration</dt>
                <dd className="text-gray-900 dark:text-white">{formatDuration(storyDuration)}</dd>
              </div>
            )}

            {story.retryCount !== undefined && story.retryCount > 0 && (
              <div className="flex justify-between">
                <dt className="text-gray-500 dark:text-gray-400">Retry Attempts</dt>
                <dd className="text-yellow-600 dark:text-yellow-400">{story.retryCount}</dd>
              </div>
            )}

            {story.error && (
              <div className="pt-2">
                <dt className="text-red-500 dark:text-red-400 mb-1">Error</dt>
                <dd className="p-2 rounded bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-300 font-mono text-xs whitespace-pre-wrap">
                  {story.error}
                </dd>
              </div>
            )}

            {story.description && (
              <div className="pt-2">
                <dt className="text-gray-500 dark:text-gray-400 mb-1">Description</dt>
                <dd className="text-gray-700 dark:text-gray-300">{story.description}</dd>
              </div>
            )}
          </dl>
        </div>
      )}
    </div>
  );
}

function formatDuration(ms: number): string {
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

export default ResultView;
