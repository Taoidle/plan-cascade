/**
 * ProgressView Component
 *
 * Displays execution progress in Simple mode.
 * Shows current story being executed and overall progress.
 */

import { clsx } from 'clsx';
import { useExecutionStore } from '../../store/execution';
import { CheckIcon, Cross2Icon, UpdateIcon } from '@radix-ui/react-icons';

export function ProgressView() {
  const { stories, currentStoryId, progress, strategy } = useExecutionStore();

  return (
    <div className="max-w-2xl mx-auto w-full space-y-6">
      {/* Strategy Badge */}
      {strategy && (
        <div className="flex items-center justify-center">
          <span
            className={clsx(
              'px-3 py-1 rounded-full text-sm font-medium',
              'bg-primary-100 dark:bg-primary-900',
              'text-primary-700 dark:text-primary-300'
            )}
          >
            Strategy: {strategy}
          </span>
        </div>
      )}

      {/* Progress Bar */}
      <div className="space-y-2">
        <div className="flex justify-between text-sm text-gray-600 dark:text-gray-400">
          <span>Progress</span>
          <span>{Math.round(progress)}%</span>
        </div>
        <div className="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
          <div
            className="h-full bg-primary-600 transition-all duration-300"
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>

      {/* Stories List */}
      <div className="space-y-3">
        {stories.map((story) => (
          <StoryItem
            key={story.id}
            story={story}
            isCurrent={story.id === currentStoryId}
          />
        ))}
      </div>
    </div>
  );
}

interface StoryItemProps {
  story: {
    id: string;
    title: string;
    status: string;
    progress: number;
  };
  isCurrent: boolean;
}

function StoryItem({ story, isCurrent }: StoryItemProps) {
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
        isCurrent && 'ring-2 ring-primary-500'
      )}
    >
      {getStatusIcon()}
      <div className="flex-1 min-w-0">
        <div className="font-medium text-gray-900 dark:text-white truncate">
          {story.title}
        </div>
        {story.status === 'in_progress' && (
          <div className="mt-1 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
            <div
              className="h-full bg-primary-500 transition-all duration-300"
              style={{ width: `${story.progress}%` }}
            />
          </div>
        )}
      </div>
    </div>
  );
}

export default ProgressView;
