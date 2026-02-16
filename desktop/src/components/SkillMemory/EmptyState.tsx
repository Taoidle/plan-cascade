/**
 * EmptyState Component
 *
 * Displays an empty state placeholder with icon, title, and optional description.
 * Used in both skill and memory list views when no items match.
 */

import { clsx } from 'clsx';

interface EmptyStateProps {
  title: string;
  description?: string;
  icon?: React.ReactNode;
  className?: string;
  action?: {
    label: string;
    onClick: () => void;
  };
}

export function EmptyState({ title, description, icon, className, action }: EmptyStateProps) {
  return (
    <div
      data-testid="empty-state"
      className={clsx(
        'flex flex-col items-center justify-center text-center py-8 px-4',
        className
      )}
    >
      {icon ? (
        <div className="mb-3 text-gray-300 dark:text-gray-600">{icon}</div>
      ) : (
        <svg
          className="w-10 h-10 text-gray-300 dark:text-gray-600 mb-3"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4"
          />
        </svg>
      )}
      <p className="text-sm font-medium text-gray-500 dark:text-gray-400">{title}</p>
      {description && (
        <p className="text-xs text-gray-400 dark:text-gray-500 mt-1 max-w-xs">{description}</p>
      )}
      {action && (
        <button
          onClick={action.onClick}
          className={clsx(
            'mt-3 px-3 py-1.5 rounded-md text-xs font-medium transition-colors',
            'bg-primary-600 text-white hover:bg-primary-700',
            'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1'
          )}
        >
          {action.label}
        </button>
      )}
    </div>
  );
}

export default EmptyState;
