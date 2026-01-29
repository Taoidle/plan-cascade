/**
 * ResultView Component
 *
 * Displays execution results in Simple mode.
 * Shows success/failure status and summary.
 */

import { clsx } from 'clsx';
import { CheckCircledIcon, CrossCircledIcon } from '@radix-ui/react-icons';

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
  if (!result) return null;

  const { success, message, completedStories, totalStories, duration, error } = result;

  return (
    <div className="max-w-2xl mx-auto w-full">
      <div
        className={clsx(
          'p-6 rounded-xl',
          'bg-white dark:bg-gray-800',
          'border-2',
          success
            ? 'border-green-200 dark:border-green-800'
            : 'border-red-200 dark:border-red-800'
        )}
      >
        {/* Status Icon */}
        <div className="flex justify-center mb-4">
          {success ? (
            <CheckCircledIcon className="w-16 h-16 text-green-500" />
          ) : (
            <CrossCircledIcon className="w-16 h-16 text-red-500" />
          )}
        </div>

        {/* Status Message */}
        <h2
          className={clsx(
            'text-xl font-semibold text-center mb-2',
            success ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'
          )}
        >
          {success ? 'Completed Successfully' : 'Execution Failed'}
        </h2>

        <p className="text-center text-gray-600 dark:text-gray-400 mb-6">
          {message}
        </p>

        {/* Stats */}
        <div className="grid grid-cols-3 gap-4 text-center">
          <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-900">
            <div className="text-2xl font-bold text-gray-900 dark:text-white">
              {completedStories}/{totalStories}
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400">
              Stories
            </div>
          </div>
          <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-900">
            <div className="text-2xl font-bold text-gray-900 dark:text-white">
              {formatDuration(duration)}
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400">
              Duration
            </div>
          </div>
          <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-900">
            <div className="text-2xl font-bold text-gray-900 dark:text-white">
              {Math.round((completedStories / totalStories) * 100)}%
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400">
              Success Rate
            </div>
          </div>
        </div>

        {/* Error Details */}
        {error && (
          <div className="mt-4 p-4 rounded-lg bg-red-50 dark:bg-red-900/20">
            <h3 className="font-medium text-red-600 dark:text-red-400 mb-1">
              Error Details
            </h3>
            <pre className="text-sm text-red-500 dark:text-red-300 whitespace-pre-wrap">
              {error}
            </pre>
          </div>
        )}
      </div>
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
