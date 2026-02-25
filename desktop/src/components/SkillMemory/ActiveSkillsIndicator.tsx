/**
 * ActiveSkillsIndicator Component
 *
 * Compact indicator shown in the chat header displaying the count
 * of currently active skills for the session. Clicking opens the
 * skill management panel.
 */

import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useSkillMemoryStore } from '../../store/skillMemory';

interface ActiveSkillsIndicatorProps {
  className?: string;
}

export function ActiveSkillsIndicator({ className }: ActiveSkillsIndicatorProps) {
  const { t } = useTranslation('simpleMode');
  const skills = useSkillMemoryStore((s) => s.skills);
  const openDialog = useSkillMemoryStore((s) => s.openDialog);

  const enabledCount = useMemo(() => skills.filter((s) => s.enabled).length, [skills]);

  if (enabledCount === 0) return null;

  return (
    <button
      data-testid="active-skills-indicator"
      onClick={() => openDialog('skills')}
      className={clsx(
        'inline-flex items-center gap-1 px-2 py-1 rounded-md text-2xs font-medium transition-colors',
        'bg-primary-50 dark:bg-primary-900/20',
        'text-primary-700 dark:text-primary-300',
        'hover:bg-primary-100 dark:hover:bg-primary-900/40',
        className,
      )}
      title={t('skillPanel.activeSkillsTooltip', { count: enabledCount })}
    >
      <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
      </svg>
      <span>{enabledCount}</span>
    </button>
  );
}

export default ActiveSkillsIndicator;
