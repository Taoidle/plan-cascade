/**
 * ProgressView Component
 *
 * Displays execution progress in Simple mode.
 * Shows batch progress, story status, and overall progress with animations.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  CheckIcon,
  Cross2Icon,
  UpdateIcon,
  PauseIcon,
  PlayIcon,
  StopIcon,
} from '@radix-ui/react-icons';
import { useExecutionStore } from '../../store/execution';

export function ProgressView() {
  const { t } = useTranslation('simpleMode');
  const {
    stories,
    batches,
    currentBatch,
    currentStoryId,
    progress,
    strategy,
    status,
    pause,
    resume,
    cancel,
    logs,
  } = useExecutionStore();

  const isPaused = status === 'paused';
  const totalBatches = batches.length > 0 ? batches[batches.length - 1]?.totalBatches || 1 : 1;

  const formatStrategy = (s: string): string => {
    switch (s) {
      case 'direct':
        return t('strategies.direct');
      case 'hybrid_auto':
        return t('strategies.hybridAuto');
      case 'mega_plan':
        return t('strategies.megaPlan');
      default:
        return s;
    }
  };

  return (
    <div className="max-w-2xl 3xl:max-w-3xl 5xl:max-w-4xl mx-auto w-full space-y-6">
      {/* Strategy Badge */}
      {strategy && (
        <div className="flex items-center justify-center">
          <span
            className={clsx(
              'px-3 py-1 rounded-full text-sm font-medium',
              'bg-primary-100 dark:bg-primary-900',
              'text-primary-700 dark:text-primary-300',
              'animate-fade-in'
            )}
          >
            {t('progress.strategy', { strategy: formatStrategy(strategy) })}
          </span>
        </div>
      )}

      {/* Overall Progress Bar */}
      <div className="space-y-2">
        <div className="flex justify-between text-sm text-gray-600 dark:text-gray-400">
          <span>{t('progress.overallProgress')}</span>
          <span>{Math.round(progress)}%</span>
        </div>
        <div className="h-3 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
          <div
            className={clsx(
              'h-full bg-gradient-to-r from-primary-500 to-primary-600',
              'transition-all duration-500 ease-out',
              status === 'running' && 'animate-progress-pulse'
            )}
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>

      {/* Batch Progress */}
      {batches.length > 0 && totalBatches > 1 && (
        <div className="flex items-center justify-center gap-2">
          <span className="text-sm text-gray-600 dark:text-gray-400">
            {t('progress.batch', { current: currentBatch, total: totalBatches })}
          </span>
          <div className="flex gap-1">
            {Array.from({ length: totalBatches }).map((_, idx) => {
              const batch = batches.find((b) => b.batchNum === idx + 1);
              const isComplete = batch?.status === 'completed';
              const isCurrent = idx + 1 === currentBatch;
              const isFailed = batch?.status === 'failed';

              return (
                <div
                  key={idx}
                  className={clsx(
                    'w-3 h-3 rounded-full transition-all duration-300',
                    isComplete && 'bg-green-500',
                    isFailed && 'bg-red-500',
                    isCurrent && !isComplete && !isFailed && 'bg-primary-500 animate-pulse',
                    !isComplete && !isCurrent && !isFailed && 'bg-gray-300 dark:bg-gray-600'
                  )}
                />
              );
            })}
          </div>
        </div>
      )}

      {/* Control Buttons */}
      <div className="flex justify-center gap-2">
        {status === 'running' && (
          <button
            onClick={pause}
            className={clsx(
              'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium',
              'bg-gray-100 dark:bg-gray-700',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-600',
              'transition-colors'
            )}
          >
            <PauseIcon className="w-4 h-4" />
            {t('buttons.pause', { ns: 'common' })}
          </button>
        )}
        {isPaused && (
          <button
            onClick={resume}
            className={clsx(
              'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium',
              'bg-primary-600 text-white',
              'hover:bg-primary-700',
              'transition-colors'
            )}
          >
            <PlayIcon className="w-4 h-4" />
            {t('buttons.resume', { ns: 'common' })}
          </button>
        )}
        <button
          onClick={cancel}
          className={clsx(
            'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium',
            'bg-red-100 dark:bg-red-900/30',
            'text-red-700 dark:text-red-300',
            'hover:bg-red-200 dark:hover:bg-red-900/50',
            'transition-colors'
          )}
        >
          <StopIcon className="w-4 h-4" />
          {t('buttons.stop', { ns: 'common' })}
        </button>
      </div>

      {/* Stories List */}
      <div className="space-y-3">
        {stories.map((story, index) => (
          <StoryItem
            key={story.id}
            story={story}
            isCurrent={story.id === currentStoryId}
            index={index}
          />
        ))}
      </div>

      {/* Execution Logs */}
      {logs.length > 0 && (
        <div className="mt-6">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            {t('progress.executionLog')}
          </h3>
          <div
            className={clsx(
              'max-h-40 overflow-y-auto p-3 rounded-lg',
              'bg-gray-50 dark:bg-gray-900',
              'border border-gray-200 dark:border-gray-700',
              'font-mono text-xs'
            )}
          >
            {logs.slice(-20).map((log, idx) => (
              <div
                key={idx}
                className="text-gray-600 dark:text-gray-400 py-0.5"
              >
                {log}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

interface StoryItemProps {
  story: {
    id: string;
    title: string;
    status: string;
    progress: number;
    error?: string;
    retryCount?: number;
  };
  isCurrent: boolean;
  index: number;
}

function StoryItem({ story, isCurrent, index }: StoryItemProps) {
  const { t } = useTranslation('simpleMode');

  const getStatusIcon = () => {
    switch (story.status) {
      case 'completed':
        return <CheckIcon className="w-4 h-4 text-green-500" />;
      case 'failed':
        return <Cross2Icon className="w-4 h-4 text-red-500" />;
      case 'in_progress':
        return <UpdateIcon className="w-4 h-4 text-primary-500 animate-spin" />;
      default:
        return <div className="w-4 h-4 rounded-full border-2 border-gray-300 dark:border-gray-600" />;
    }
  };

  return (
    <div
      className={clsx(
        'flex items-center gap-3 p-4 rounded-lg',
        'bg-white dark:bg-gray-800',
        'border border-gray-200 dark:border-gray-700',
        isCurrent && 'ring-2 ring-primary-500 shadow-md',
        isCurrent && 'animate-fade-in',
        'transition-all duration-300'
      )}
      style={{
        animationDelay: `${index * 100}ms`,
      }}
    >
      {getStatusIcon()}
      <div className="flex-1 min-w-0">
        <div className="font-medium text-gray-900 dark:text-white truncate">
          {story.title}
        </div>

        {/* Retry indicator */}
        {story.retryCount && story.retryCount > 0 && (
          <div className="text-xs text-yellow-600 dark:text-yellow-400 mt-0.5">
            {t('progress.retryAttempt', { count: story.retryCount })}
          </div>
        )}

        {/* Error message */}
        {story.status === 'failed' && story.error && (
          <div className="text-xs text-red-500 dark:text-red-400 mt-1 truncate">
            {story.error}
          </div>
        )}

        {/* Progress bar for in-progress stories */}
        {story.status === 'in_progress' && (
          <div className="mt-2 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
            <div
              className={clsx(
                'h-full bg-primary-500 transition-all duration-300',
                'animate-progress-pulse'
              )}
              style={{ width: `${story.progress}%` }}
            />
          </div>
        )}
      </div>

      {/* Progress percentage for in-progress */}
      {story.status === 'in_progress' && (
        <span className="text-sm text-gray-500 dark:text-gray-400">
          {Math.round(story.progress)}%
        </span>
      )}
    </div>
  );
}

export default ProgressView;
