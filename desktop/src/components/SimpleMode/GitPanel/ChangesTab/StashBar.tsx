/**
 * StashBar Component
 *
 * Small bar showing stash count with Stash/Pop buttons and a dropdown
 * to view the stash list.
 *
 * Feature-002, Story-005
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useGitStore } from '../../../../store/git';

export function StashBar() {
  const { t } = useTranslation('git');
  const stashList = useGitStore((s) => s.stashList);
  const stashSave = useGitStore((s) => s.stashSave);
  const stashPop = useGitStore((s) => s.stashPop);
  const stashDrop = useGitStore((s) => s.stashDrop);
  const status = useGitStore((s) => s.status);

  const [dropdownOpen, setDropdownOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const hasChanges =
    (status?.unstaged.length ?? 0) > 0 || (status?.untracked.length ?? 0) > 0 || (status?.staged.length ?? 0) > 0;

  // Close dropdown when clicking outside
  useEffect(() => {
    if (!dropdownOpen) return;
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [dropdownOpen]);

  const handleStash = useCallback(async () => {
    await stashSave();
  }, [stashSave]);

  const handlePop = useCallback(async () => {
    await stashPop();
    setDropdownOpen(false);
  }, [stashPop]);

  const handleDrop = useCallback(
    async (index: number) => {
      await stashDrop(index);
    },
    [stashDrop],
  );

  return (
    <div className="flex items-center justify-between px-3 py-1.5 border-b border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-800/30">
      <div className="flex items-center gap-2">
        <svg
          className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8"
          />
        </svg>
        <span className="text-2xs text-gray-500 dark:text-gray-400">
          {t('stashBar.stash')}
          {stashList.length > 0 && (
            <span className="ml-1 font-medium text-gray-700 dark:text-gray-300">({stashList.length})</span>
          )}
        </span>
      </div>

      <div className="relative flex items-center gap-1" ref={dropdownRef}>
        {/* Stash button */}
        <button
          onClick={handleStash}
          disabled={!hasChanges}
          className={clsx(
            'text-2xs px-2 py-0.5 rounded transition-colors',
            hasChanges
              ? 'text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
              : 'text-gray-400 dark:text-gray-500 cursor-not-allowed',
          )}
        >
          {t('stashBar.stash')}
        </button>

        {/* Pop button */}
        <button
          onClick={handlePop}
          disabled={stashList.length === 0}
          className={clsx(
            'text-2xs px-2 py-0.5 rounded transition-colors',
            stashList.length > 0
              ? 'text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
              : 'text-gray-400 dark:text-gray-500 cursor-not-allowed',
          )}
        >
          {t('stashBar.pop')}
        </button>

        {/* Dropdown toggle */}
        {stashList.length > 0 && (
          <button
            onClick={() => setDropdownOpen((v) => !v)}
            className="text-2xs p-0.5 rounded text-gray-400 dark:text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
          >
            <svg
              className={clsx('w-3 h-3 transition-transform', dropdownOpen && 'rotate-180')}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </button>
        )}

        {/* Dropdown */}
        {dropdownOpen && stashList.length > 0 && (
          <div className="absolute right-0 top-full mt-1 z-20 w-72 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg overflow-hidden">
            <div className="max-h-48 overflow-y-auto">
              {stashList.map((entry) => (
                <div
                  key={entry.index}
                  className="flex items-center justify-between px-3 py-2 text-xs border-b border-gray-100 dark:border-gray-800 last:border-b-0 hover:bg-gray-50 dark:hover:bg-gray-800/60"
                >
                  <div className="flex-1 min-w-0 mr-2">
                    <p className="text-gray-700 dark:text-gray-300 truncate">
                      stash@{'{' + entry.index + '}'}: {entry.message}
                    </p>
                    <p className="text-2xs text-gray-400 dark:text-gray-500 mt-0.5">{entry.date}</p>
                  </div>
                  <button
                    onClick={() => handleDrop(entry.index)}
                    className="shrink-0 p-1 rounded text-gray-400 hover:text-red-500 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                    title={t('stashBar.dropStash')}
                  >
                    <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default StashBar;
