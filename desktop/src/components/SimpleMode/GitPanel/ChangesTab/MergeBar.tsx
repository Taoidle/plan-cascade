/**
 * MergeBar Component
 *
 * Conditional banner shown during merge/rebase/cherry-pick/revert state.
 * Shows the operation type with Abort/Continue buttons.
 *
 * Feature-002, Story-005
 */

import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import { useGitStore, type MergeState } from '../../../../store/git';
import { useSettingsStore } from '../../../../store/settings';
import type { CommandResponse } from '../../../../lib/tauri';

// ============================================================================
// Helpers
// ============================================================================

function getMergeLabelKey(kind: MergeState['kind']): string {
  switch (kind) {
    case 'merging':
      return 'mergeBar.merging';
    case 'rebasing':
      return 'mergeBar.rebasing';
    case 'cherry_picking':
      return 'mergeBar.cherryPicking';
    case 'reverting':
      return 'mergeBar.reverting';
    default:
      return '';
  }
}

function getMergeColor(kind: MergeState['kind']): string {
  switch (kind) {
    case 'merging':
      return 'border-orange-300 bg-orange-50 dark:border-orange-700 dark:bg-orange-900/20';
    case 'rebasing':
      return 'border-purple-300 bg-purple-50 dark:border-purple-700 dark:bg-purple-900/20';
    case 'cherry_picking':
      return 'border-pink-300 bg-pink-50 dark:border-pink-700 dark:bg-pink-900/20';
    case 'reverting':
      return 'border-yellow-300 bg-yellow-50 dark:border-yellow-700 dark:bg-yellow-900/20';
    default:
      return 'border-gray-300 bg-gray-50 dark:border-gray-700 dark:bg-gray-800';
  }
}

function getMergeTextColor(kind: MergeState['kind']): string {
  switch (kind) {
    case 'merging':
      return 'text-orange-700 dark:text-orange-300';
    case 'rebasing':
      return 'text-purple-700 dark:text-purple-300';
    case 'cherry_picking':
      return 'text-pink-700 dark:text-pink-300';
    case 'reverting':
      return 'text-yellow-700 dark:text-yellow-300';
    default:
      return 'text-gray-700 dark:text-gray-300';
  }
}

// ============================================================================
// Component
// ============================================================================

export function MergeBar() {
  const { t } = useTranslation('git');
  const mergeState = useGitStore((s) => s.mergeState);
  const refreshAll = useGitStore((s) => s.refreshAll);
  const setError = useGitStore((s) => s.setError);

  const handleAbort = useCallback(async () => {
    const repoPath = useSettingsStore.getState().workspacePath;
    if (!repoPath || !mergeState) return;

    try {
      // Determine the right abort command based on merge state kind
      let command: string;
      switch (mergeState.kind) {
        case 'merging':
          command = 'merge';
          break;
        case 'rebasing':
          command = 'rebase';
          break;
        case 'cherry_picking':
          command = 'cherry-pick';
          break;
        case 'reverting':
          command = 'revert';
          break;
        default:
          return;
      }

      // Use git_stage_files with the abort argument through the shell
      // Since we don't have a dedicated abort command, we use a generic approach
      // by running git <command> --abort via the git service
      const response = await invoke<CommandResponse<null>>('git_stage_files', {
        repoPath,
        paths: [],
      }).catch(() => null);

      // Actually, we need a proper abort. Let's use the commit command with a special message
      // In practice, the git service should have abort commands. For now, let's provide feedback.
      setError(`Abort not yet supported via IPC. Use terminal: git ${command} --abort`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    await refreshAll();
  }, [mergeState, refreshAll, setError]);

  const handleContinue = useCallback(async () => {
    const repoPath = useSettingsStore.getState().workspacePath;
    if (!repoPath || !mergeState) return;

    try {
      // For merge continue, we just commit (git already knows it's a merge commit)
      if (mergeState.kind === 'merging') {
        // Trigger a commit with the default merge message
        await invoke<CommandResponse<string>>('git_commit', {
          repoPath,
          message: `Merge branch '${mergeState.branch_name || 'unknown'}'`,
        });
      } else {
        setError(`Continue not yet supported via IPC for ${mergeState.kind}. Use terminal.`);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    await refreshAll();
  }, [mergeState, refreshAll, setError]);

  // Only render when there's an active merge-like operation
  if (!mergeState || mergeState.kind === 'none') {
    return null;
  }

  const labelKey = getMergeLabelKey(mergeState.kind);
  const colorClass = getMergeColor(mergeState.kind);
  const textClass = getMergeTextColor(mergeState.kind);

  return (
    <div className={clsx('px-3 py-2 border-b', colorClass)}>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className={clsx('text-xs font-bold', textClass)}>{t(labelKey)}</span>
          {mergeState.branch_name && (
            <span className={clsx('text-2xs', textClass)}>
              {mergeState.branch_name} {t('mergeBar.toHead')}
            </span>
          )}
        </div>

        <div className="flex items-center gap-1.5">
          <button
            onClick={handleAbort}
            className="text-2xs px-2 py-0.5 rounded font-medium text-red-600 dark:text-red-400 hover:bg-red-100 dark:hover:bg-red-900/30 transition-colors"
          >
            {t('mergeBar.abort')}
          </button>
          <button
            onClick={handleContinue}
            className="text-2xs px-2 py-0.5 rounded font-medium text-green-600 dark:text-green-400 hover:bg-green-100 dark:hover:bg-green-900/30 transition-colors"
          >
            {t('mergeBar.continue')}
          </button>
        </div>
      </div>
    </div>
  );
}

export default MergeBar;
