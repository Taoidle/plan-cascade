/**
 * SkillSourceBadge Component
 *
 * Displays a colored badge indicating the source tier of a skill.
 * Colors: builtin=gray, external=blue, user=green, project_local=green, generated=purple.
 */

import { clsx } from 'clsx';
import type { SkillSource } from '../../types/skillMemory';
import { getSkillSourceDisplayName } from '../../types/skillMemory';

interface SkillSourceBadgeProps {
  source: SkillSource;
  className?: string;
  /** Compact mode for sidebar rows */
  compact?: boolean;
}

const sourceStyles: Record<string, { bg: string; text: string }> = {
  builtin: {
    bg: 'bg-gray-100 dark:bg-gray-700',
    text: 'text-gray-600 dark:text-gray-300',
  },
  external: {
    bg: 'bg-blue-100 dark:bg-blue-900/30',
    text: 'text-blue-700 dark:text-blue-300',
  },
  user: {
    bg: 'bg-green-100 dark:bg-green-900/30',
    text: 'text-green-700 dark:text-green-300',
  },
  project_local: {
    bg: 'bg-green-100 dark:bg-green-900/30',
    text: 'text-green-700 dark:text-green-300',
  },
  generated: {
    bg: 'bg-purple-100 dark:bg-purple-900/30',
    text: 'text-purple-700 dark:text-purple-300',
  },
};

export function SkillSourceBadge({ source, className, compact = false }: SkillSourceBadgeProps) {
  const style = sourceStyles[source.type] || sourceStyles.builtin;
  const label = getSkillSourceDisplayName(source);

  return (
    <span
      data-testid="skill-source-badge"
      className={clsx(
        'inline-flex items-center rounded-full font-medium shrink-0',
        compact ? 'px-1.5 py-0.5 text-2xs' : 'px-2 py-0.5 text-xs',
        style.bg,
        style.text,
        className
      )}
    >
      {label}
    </span>
  );
}

export default SkillSourceBadge;
