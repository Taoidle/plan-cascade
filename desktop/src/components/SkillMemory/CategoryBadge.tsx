/**
 * CategoryBadge Component
 *
 * Displays a colored badge for memory categories.
 * Each category has a distinct color for visual differentiation.
 */

import { clsx } from 'clsx';
import type { MemoryCategory } from '../../types/skillMemory';
import { getMemoryCategoryDisplayName } from '../../types/skillMemory';

interface CategoryBadgeProps {
  category: MemoryCategory;
  className?: string;
  /** Compact mode for list rows */
  compact?: boolean;
}

const categoryStyles: Record<MemoryCategory, { bg: string; text: string }> = {
  preference: {
    bg: 'bg-blue-100 dark:bg-blue-900/30',
    text: 'text-blue-700 dark:text-blue-300',
  },
  convention: {
    bg: 'bg-amber-100 dark:bg-amber-900/30',
    text: 'text-amber-700 dark:text-amber-300',
  },
  pattern: {
    bg: 'bg-green-100 dark:bg-green-900/30',
    text: 'text-green-700 dark:text-green-300',
  },
  correction: {
    bg: 'bg-red-100 dark:bg-red-900/30',
    text: 'text-red-700 dark:text-red-300',
  },
  fact: {
    bg: 'bg-purple-100 dark:bg-purple-900/30',
    text: 'text-purple-700 dark:text-purple-300',
  },
};

export function CategoryBadge({ category, className, compact = false }: CategoryBadgeProps) {
  const style = categoryStyles[category] || categoryStyles.fact;
  const label = getMemoryCategoryDisplayName(category);

  return (
    <span
      data-testid="category-badge"
      className={clsx(
        'inline-flex items-center rounded-full font-medium shrink-0',
        compact ? 'px-1.5 py-0.5 text-2xs' : 'px-2 py-0.5 text-xs',
        style.bg,
        style.text,
        className,
      )}
    >
      {label}
    </span>
  );
}

export default CategoryBadge;
