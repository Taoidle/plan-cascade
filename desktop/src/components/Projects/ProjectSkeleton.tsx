/**
 * ProjectSkeleton Component
 *
 * Loading skeleton for project cards.
 */

import { clsx } from 'clsx';

export function ProjectSkeleton() {
  return (
    <div
      className={clsx(
        'w-full p-4 rounded-lg animate-pulse',
        'bg-white dark:bg-gray-800',
        'border border-gray-200 dark:border-gray-700'
      )}
    >
      {/* Title skeleton */}
      <div className="h-4 w-1/3 bg-gray-200 dark:bg-gray-700 rounded mb-2" />

      {/* Path skeleton */}
      <div className="h-3 w-2/3 bg-gray-100 dark:bg-gray-800 rounded mb-3" />

      {/* Stats row skeleton */}
      <div className="flex items-center gap-4">
        <div className="h-3 w-16 bg-gray-100 dark:bg-gray-800 rounded" />
        <div className="h-3 w-12 bg-gray-100 dark:bg-gray-800 rounded" />
        <div className="h-3 w-14 bg-gray-100 dark:bg-gray-800 rounded ml-auto" />
      </div>
    </div>
  );
}

export default ProjectSkeleton;
