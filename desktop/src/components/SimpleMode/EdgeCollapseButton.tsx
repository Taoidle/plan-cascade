/**
 * EdgeCollapseButton Component
 *
 * A small button absolutely positioned inside the chat area,
 * vertically centered at the left or right edge, floating on top of content.
 * Uses a single ChevronRightIcon with rotation for direction.
 */

import { clsx } from 'clsx';
import { ChevronRightIcon } from '@radix-ui/react-icons';
import { useTranslation } from 'react-i18next';

interface EdgeCollapseButtonProps {
  side: 'left' | 'right';
  expanded: boolean;
  onToggle: () => void;
  label?: string;
}

export function EdgeCollapseButton({ side, expanded, onToggle, label }: EdgeCollapseButtonProps) {
  const { t } = useTranslation('simpleMode');

  const defaultLabel =
    side === 'left'
      ? expanded
        ? t('edgeButton.collapseSidebar', { defaultValue: 'Collapse sidebar' })
        : t('edgeButton.expandSidebar', { defaultValue: 'Expand sidebar' })
      : expanded
        ? t('edgeButton.collapsePanel', { defaultValue: 'Collapse panel' })
        : t('edgeButton.expandPanel', { defaultValue: 'Expand panel' });

  const isLeft = side === 'left';

  // Left side: expanded → point left (rotate-180), collapsed → point right (rotate-0)
  // Right side: expanded → point right (rotate-0), collapsed → point left (rotate-180)
  const shouldRotate = isLeft ? expanded : !expanded;

  return (
    <button
      onClick={onToggle}
      title={label || defaultLabel}
      className={clsx(
        'absolute top-1/2 -translate-y-1/2 z-10',
        'flex items-center justify-center',
        'w-5 h-10',
        'bg-gray-100/80 dark:bg-gray-800/80 backdrop-blur-sm',
        'hover:bg-gray-200 dark:hover:bg-gray-700',
        'text-gray-500 dark:text-gray-400',
        'opacity-30 hover:opacity-100',
        'transition-all duration-200',
        isLeft ? 'left-0 rounded-r-md' : 'right-0 rounded-l-md',
      )}
    >
      <ChevronRightIcon
        className={clsx('w-3.5 h-3.5 transition-transform duration-200', shouldRotate && 'rotate-180')}
      />
    </button>
  );
}
