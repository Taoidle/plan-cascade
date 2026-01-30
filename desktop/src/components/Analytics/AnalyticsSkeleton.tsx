/**
 * Analytics Skeleton Component
 *
 * Loading skeleton for the analytics dashboard.
 */

import { clsx } from 'clsx';

export function AnalyticsSkeleton() {
  return (
    <div className="space-y-6 animate-pulse">
      {/* Overview Cards Skeleton */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {Array.from({ length: 4 }).map((_, i) => (
          <div
            key={i}
            className={clsx(
              'bg-white dark:bg-gray-900 rounded-xl',
              'border border-gray-200 dark:border-gray-800',
              'p-6'
            )}
          >
            <div className="flex items-start justify-between">
              <div className="w-10 h-10 bg-gray-200 dark:bg-gray-700 rounded-lg" />
              <div className="w-16 h-6 bg-gray-200 dark:bg-gray-700 rounded-full" />
            </div>
            <div className="mt-4 space-y-2">
              <div className="w-24 h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="w-32 h-8 bg-gray-200 dark:bg-gray-700 rounded" />
            </div>
          </div>
        ))}
      </div>

      {/* Charts Grid Skeleton */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Cost Chart Skeleton */}
        <div
          className={clsx(
            'bg-white dark:bg-gray-900 rounded-xl',
            'border border-gray-200 dark:border-gray-800',
            'p-6'
          )}
        >
          <div className="w-40 h-6 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
          <div className="h-[300px] bg-gray-100 dark:bg-gray-800 rounded-lg" />
        </div>

        {/* Token Breakdown Skeleton */}
        <div
          className={clsx(
            'bg-white dark:bg-gray-900 rounded-xl',
            'border border-gray-200 dark:border-gray-800',
            'p-6'
          )}
        >
          <div className="w-40 h-6 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
          <div className="flex gap-6">
            <div className="w-[180px] h-[180px] bg-gray-100 dark:bg-gray-800 rounded-full" />
            <div className="flex-1 space-y-3">
              {Array.from({ length: 5 }).map((_, i) => (
                <div key={i} className="flex items-center gap-3">
                  <div className="w-3 h-3 bg-gray-200 dark:bg-gray-700 rounded-sm" />
                  <div className="flex-1 h-4 bg-gray-200 dark:bg-gray-700 rounded" />
                  <div className="w-12 h-4 bg-gray-200 dark:bg-gray-700 rounded" />
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* Table Skeleton */}
      <div
        className={clsx(
          'bg-white dark:bg-gray-900 rounded-xl',
          'border border-gray-200 dark:border-gray-800',
          'p-6'
        )}
      >
        <div className="w-40 h-6 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
        <div className="space-y-3">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="flex gap-4">
              <div className="w-24 h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="w-32 h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="w-20 h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="flex-1 h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="w-16 h-4 bg-gray-200 dark:bg-gray-700 rounded" />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export default AnalyticsSkeleton;
