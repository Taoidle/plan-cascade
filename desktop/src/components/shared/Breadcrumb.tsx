/**
 * Breadcrumb Component
 *
 * Renders the current navigation path with clickable items for
 * navigating to parent levels. Supports mode-level and sub-view
 * breadcrumb items.
 *
 * Story 005: Navigation Flow Refinement
 */

import { clsx } from 'clsx';
import { ChevronRightIcon, HomeIcon } from '@radix-ui/react-icons';
import { useModeStore } from '../../store/mode';

// ============================================================================
// Types
// ============================================================================

interface BreadcrumbProps {
  className?: string;
}

// ============================================================================
// Breadcrumb Component
// ============================================================================

export function Breadcrumb({ className }: BreadcrumbProps) {
  const { breadcrumbs, navigateToBreadcrumb } = useModeStore();

  if (breadcrumbs.length === 0) return null;

  return (
    <nav
      aria-label="Breadcrumb"
      className={clsx('flex items-center gap-1 text-sm', className)}
    >
      <ol className="flex items-center gap-1">
        {breadcrumbs.map((item, index) => {
          const isLast = index === breadcrumbs.length - 1;
          const isFirst = index === 0;

          return (
            <li key={item.id} className="flex items-center gap-1">
              {/* Separator */}
              {index > 0 && (
                <ChevronRightIcon
                  className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 flex-shrink-0"
                  aria-hidden="true"
                />
              )}

              {/* Breadcrumb item */}
              {item.navigable && !isLast ? (
                <button
                  onClick={() => navigateToBreadcrumb(item.id)}
                  className={clsx(
                    'flex items-center gap-1.5 px-1.5 py-0.5 rounded',
                    'text-gray-500 dark:text-gray-400',
                    'hover:text-gray-900 dark:hover:text-white',
                    'hover:bg-gray-100 dark:hover:bg-gray-800',
                    'transition-colors duration-150',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1'
                  )}
                >
                  {isFirst && (
                    <HomeIcon className="w-3.5 h-3.5 flex-shrink-0" />
                  )}
                  <span className="truncate max-w-[120px]">{item.label}</span>
                </button>
              ) : (
                <span
                  className={clsx(
                    'flex items-center gap-1.5 px-1.5 py-0.5',
                    isLast
                      ? 'text-gray-900 dark:text-white font-medium'
                      : 'text-gray-500 dark:text-gray-400'
                  )}
                  aria-current={isLast ? 'page' : undefined}
                >
                  {isFirst && (
                    <HomeIcon className="w-3.5 h-3.5 flex-shrink-0" />
                  )}
                  <span className="truncate max-w-[160px]">{item.label}</span>
                </span>
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

export default Breadcrumb;
