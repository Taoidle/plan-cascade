/**
 * MemoryRow Component
 *
 * Compact row for displaying a memory entry in the sidebar panel.
 * Shows category badge, truncated content, and importance indicator.
 */

import { useCallback } from 'react';
import { clsx } from 'clsx';
import { CategoryBadge } from '../SkillMemory/CategoryBadge';
import { ImportanceBar } from '../SkillMemory/ImportanceBar';
import type { MemoryEntry } from '../../types/skillMemory';

interface MemoryRowProps {
  memory: MemoryEntry;
  onClick?: (memory: MemoryEntry) => void;
}

export function MemoryRow({ memory, onClick }: MemoryRowProps) {
  const handleClick = useCallback(() => {
    onClick?.(memory);
  }, [memory, onClick]);

  const truncatedContent = memory.content.length > 80 ? memory.content.slice(0, 80) + '...' : memory.content;

  return (
    <div
      data-testid={`memory-row-${memory.id}`}
      className={clsx(
        'group flex items-start gap-2 px-2 py-1.5 rounded-md transition-colors',
        'hover:bg-gray-50 dark:hover:bg-gray-800',
        onClick && 'cursor-pointer',
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
      {/* Category + content */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5 mb-0.5">
          <CategoryBadge category={memory.category} compact />
          <ImportanceBar value={memory.importance} className="flex-1" />
        </div>
        <p className="text-xs text-gray-600 dark:text-gray-400 line-clamp-2">{truncatedContent}</p>
      </div>
    </div>
  );
}

export default MemoryRow;
