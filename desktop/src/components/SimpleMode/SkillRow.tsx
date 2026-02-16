/**
 * SkillRow Component
 *
 * Compact row for displaying a skill in the sidebar panel.
 * Shows checkbox toggle, name, and source badge.
 */

import { useCallback } from 'react';
import { clsx } from 'clsx';
import { SkillSourceBadge } from '../SkillMemory/SkillSourceBadge';
import type { SkillSummary } from '../../types/skillMemory';

interface SkillRowProps {
  skill: SkillSummary;
  onToggle: (id: string, enabled: boolean) => void;
  onClick?: (skill: SkillSummary) => void;
}

export function SkillRow({ skill, onToggle, onClick }: SkillRowProps) {
  const handleToggle = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      e.stopPropagation();
      onToggle(skill.id, e.target.checked);
    },
    [skill.id, onToggle]
  );

  const handleClick = useCallback(() => {
    onClick?.(skill);
  }, [skill, onClick]);

  return (
    <div
      data-testid={`skill-row-${skill.id}`}
      className={clsx(
        'group flex items-center gap-2 px-2 py-1.5 rounded-md transition-colors',
        'hover:bg-gray-50 dark:hover:bg-gray-800',
        onClick && 'cursor-pointer'
      )}
      onClick={handleClick}
      role={onClick ? 'button' : undefined}
      tabIndex={onClick ? 0 : undefined}
      onKeyDown={(e) => {
        if (onClick && (e.key === 'Enter' || e.key === ' ')) {
          e.preventDefault();
          handleClick();
        }
      }}
    >
      {/* Checkbox */}
      <input
        type="checkbox"
        checked={skill.enabled}
        onChange={handleToggle}
        onClick={(e) => e.stopPropagation()}
        className={clsx(
          'h-3.5 w-3.5 rounded border-gray-300 dark:border-gray-600 shrink-0',
          'text-primary-600 focus:ring-primary-500'
        )}
        aria-label={`Toggle ${skill.name}`}
      />

      {/* Name */}
      <span className="flex-1 min-w-0 text-xs text-gray-700 dark:text-gray-300 truncate">
        {skill.name}
      </span>

      {/* Source badge */}
      <SkillSourceBadge source={skill.source} compact />
    </div>
  );
}

export default SkillRow;
