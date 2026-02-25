/**
 * PermissionSelector
 *
 * Dropdown selector for session-level permission mode.
 * Placed in the chat header area.
 */

import { useState, useRef, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { PermissionLevel } from '../../types/permission';

interface PermissionSelectorProps {
  level: PermissionLevel;
  onLevelChange: (level: PermissionLevel) => void;
  sessionId: string;
  dropdownDirection?: 'up' | 'down';
}

const LEVELS: PermissionLevel[] = ['strict', 'standard', 'permissive'];

const LEVEL_ICON = '\u{1F6E1}'; // shield

export function PermissionSelector({ level, onLevelChange, dropdownDirection = 'down' }: PermissionSelectorProps) {
  const { t } = useTranslation('simpleMode');
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const handleSelect = useCallback(
    (newLevel: PermissionLevel) => {
      onLevelChange(newLevel);
      setOpen(false);
    },
    [onLevelChange],
  );

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  const label = t(`permission.level.${level}`);
  const description = t(`permission.levelDescription.${level}`);

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen(!open)}
        className={clsx(
          'flex items-center gap-1 px-2 py-1 rounded text-[11px] font-medium transition-colors',
          'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
          'border border-gray-200 dark:border-gray-700',
        )}
        title={t('permission.tooltip', { level: label, description })}
      >
        <span className="text-[13px]">{LEVEL_ICON}</span>
        <span>{label}</span>
      </button>

      {open && (
        <div
          className={clsx(
            'absolute left-0 w-56 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg z-50 py-1',
            dropdownDirection === 'up' ? 'bottom-full mb-1' : 'top-full mt-1',
            'animate-in fade-in-0 zoom-in-95 duration-150',
          )}
        >
          {LEVELS.map((l) => (
            <button
              key={l}
              onClick={() => handleSelect(l)}
              className={clsx(
                'w-full text-left px-3 py-2 text-xs transition-colors',
                level === l
                  ? 'bg-violet-50 dark:bg-violet-900/20 text-violet-700 dark:text-violet-300'
                  : 'text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
              )}
            >
              <div className="font-medium">{t(`permission.level.${l}`)}</div>
              <div className="text-[10px] text-gray-500 dark:text-gray-400 mt-0.5">
                {t(`permission.levelDescription.${l}`)}
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
