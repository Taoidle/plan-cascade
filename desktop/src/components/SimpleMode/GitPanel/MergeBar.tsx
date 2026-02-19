/**
 * MergeBar Component
 *
 * Compact bar shown in any tab when a merge operation is in progress.
 * Provides quick access to the conflict resolver or merge abort.
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useGitStore } from '../../../store/git';
import { useSettingsStore } from '../../../store/settings';

interface MergeBarProps {
  onOpenConflictResolver?: () => void;
}

export function MergeBar({ onOpenConflictResolver }: MergeBarProps) {
  const { t } = useTranslation('git');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const { isInMerge, mergeState, mergeSourceBranch, conflictFiles, resolvedFiles, abortMerge } =
    useGitStore();
  const [isAborting, setIsAborting] = useState(false);

  const handleAbort = useCallback(async () => {
    if (!workspacePath) return;
    setIsAborting(true);
    await abortMerge(workspacePath);
    setIsAborting(false);
  }, [workspacePath, abortMerge]);

  if (!isInMerge || !mergeState) return null;

  const resolvedCount = conflictFiles.filter((f) => resolvedFiles.has(f.path)).length;
  const totalCount = conflictFiles.length;

  const kindLabels: Record<string, string> = {
    merging: t('mergeBar.merge'),
    rebasing: t('mergeBar.rebase'),
    cherry_picking: t('mergeBar.cherryPick'),
    reverting: t('mergeBar.revert'),
  };
  const kindLabel = kindLabels[mergeState.kind] || t('mergeBar.merge');

  return (
    <div className="shrink-0 flex items-center justify-between px-3 py-1.5 bg-amber-50 dark:bg-amber-900/20 border-b border-amber-200 dark:border-amber-800">
      <div className="flex items-center gap-2 text-sm">
        <svg
          className="w-4 h-4 text-amber-500"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z"
          />
        </svg>
        <span className="font-medium text-amber-700 dark:text-amber-400">
          {kindLabel} {t('mergeBar.inProgress')}
        </span>
        {mergeSourceBranch && (
          <span className="text-amber-600 dark:text-amber-500">
            ({mergeSourceBranch})
          </span>
        )}
        {totalCount > 0 && (
          <span className="text-xs text-amber-600 dark:text-amber-500">
            {t('mergeBar.conflictsResolved', { count: `${resolvedCount}/${totalCount}` })}
          </span>
        )}
      </div>

      <div className="flex items-center gap-2">
        {onOpenConflictResolver && totalCount > 0 && (
          <button
            onClick={onOpenConflictResolver}
            className="px-2 py-1 text-xs rounded font-medium text-amber-700 dark:text-amber-400 hover:bg-amber-100 dark:hover:bg-amber-800/30 transition-colors"
          >
            {t('mergeBar.resolveConflicts')}
          </button>
        )}
        <button
          onClick={handleAbort}
          disabled={isAborting}
          className={clsx(
            'px-2 py-1 text-xs rounded transition-colors',
            'text-amber-700 dark:text-amber-400 hover:bg-amber-100 dark:hover:bg-amber-800/30',
            isAborting && 'opacity-50 cursor-not-allowed'
          )}
        >
          {isAborting ? t('mergeBar.aborting') : t('mergeBar.abort')}
        </button>
      </div>
    </div>
  );
}
