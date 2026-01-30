/**
 * TimelineSkeleton Component
 *
 * Loading skeleton for timeline view.
 */

import { clsx } from 'clsx';

export function TimelineSkeleton() {
  return (
    <div className="w-full p-4 animate-pulse">
      {/* Header skeleton */}
      <div className="flex items-center justify-between mb-6">
        <div className="h-6 w-32 bg-gray-200 dark:bg-gray-700 rounded" />
        <div className="h-8 w-24 bg-gray-200 dark:bg-gray-700 rounded" />
      </div>

      {/* Branch selector skeleton */}
      <div className="h-10 w-48 bg-gray-200 dark:bg-gray-700 rounded mb-6" />

      {/* Timeline nodes skeleton */}
      <div className="space-y-4">
        {[1, 2, 3, 4, 5].map((i) => (
          <div key={i} className="flex items-start gap-4">
            {/* Timeline line and node */}
            <div className="flex flex-col items-center">
              <div className="w-4 h-4 rounded-full bg-gray-300 dark:bg-gray-600" />
              {i < 5 && (
                <div className="w-0.5 h-16 bg-gray-200 dark:bg-gray-700" />
              )}
            </div>

            {/* Checkpoint card skeleton */}
            <div
              className={clsx(
                'flex-1 p-4 rounded-lg',
                'bg-white dark:bg-gray-800',
                'border border-gray-200 dark:border-gray-700'
              )}
            >
              <div className="h-4 w-1/3 bg-gray-200 dark:bg-gray-700 rounded mb-2" />
              <div className="h-3 w-1/4 bg-gray-100 dark:bg-gray-800 rounded mb-3" />
              <div className="flex items-center gap-2">
                <div className="h-3 w-12 bg-gray-100 dark:bg-gray-800 rounded" />
                <div className="h-3 w-16 bg-gray-100 dark:bg-gray-800 rounded" />
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default TimelineSkeleton;
