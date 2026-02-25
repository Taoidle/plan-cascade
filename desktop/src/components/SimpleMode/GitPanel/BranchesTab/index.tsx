/**
 * BranchesTab Component
 *
 * Main view split into Branches section (top, ~60%) and Worktrees section (bottom, ~40%).
 * Manages branch operations and worktree display.
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useGitBranches } from '../../../../hooks/useGitBranches';
import { useGitStore } from '../../../../store/git';
import { useSettingsStore } from '../../../../store/settings';
import { BranchList } from './BranchList';
import { BranchActions, type BranchActionType } from './BranchActions';
import { WorktreeList } from './WorktreeList';
import type { BranchInfo, MergeBranchResult } from '../../../../types/git';

export function BranchesTab() {
  const { t } = useTranslation('git');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const { localBranches, remoteBranches, currentBranch, isLoading, error, refresh } = useGitBranches();

  const { startMerge, isInMerge } = useGitStore();

  // Dialog state
  const [dialogType, setDialogType] = useState<BranchActionType | null>(null);
  const [dialogBranch, setDialogBranch] = useState<BranchInfo | null>(null);
  const [mergeResult, setMergeResult] = useState<MergeBranchResult | null>(null);

  const repoPath = workspacePath || '';

  // Handlers for branch actions
  const handleCreate = useCallback(() => {
    setDialogType('create');
    setDialogBranch(null);
  }, []);

  const handleDelete = useCallback((branch: BranchInfo) => {
    setDialogType('delete');
    setDialogBranch(branch);
  }, []);

  const handleRename = useCallback((branch: BranchInfo) => {
    setDialogType('rename');
    setDialogBranch(branch);
  }, []);

  const handleMerge = useCallback(
    async (branchName: string) => {
      if (!repoPath) return;
      const result = await startMerge(repoPath, branchName);
      if (result) {
        setMergeResult(result);
        if (!result.has_conflicts && result.success) {
          await refresh();
        }
      }
    },
    [repoPath, startMerge, refresh],
  );

  const handleDialogClose = useCallback(() => {
    setDialogType(null);
    setDialogBranch(null);
  }, []);

  const handleDialogSuccess = useCallback(async () => {
    handleDialogClose();
    await refresh();
  }, [handleDialogClose, refresh]);

  if (!workspacePath) {
    return (
      <div className="flex items-center justify-center h-full text-sm text-gray-500 dark:text-gray-400">
        {t('branchesTab.noWorkspace')}
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="shrink-0 flex items-center justify-between px-3 py-2 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-medium text-gray-800 dark:text-gray-200">{t('branchesTab.title')}</h3>
          {currentBranch && (
            <span className="text-xs text-gray-500 dark:text-gray-400">
              {t('branchesTab.on')}{' '}
              <span className="font-medium text-gray-700 dark:text-gray-300">{currentBranch.name}</span>
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={handleCreate}
            className="p-1 rounded text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
            title={t('branchesTab.createBranch')}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
          </button>
          <button
            onClick={refresh}
            disabled={isLoading}
            className={clsx(
              'p-1 rounded transition-colors',
              'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700',
              isLoading && 'opacity-50 cursor-not-allowed',
            )}
            title={t('branchesTab.refresh')}
          >
            <svg
              className={clsx('w-4 h-4', isLoading && 'animate-spin')}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              />
            </svg>
          </button>
        </div>
      </div>

      {/* Merge result toast */}
      {mergeResult && !mergeResult.has_conflicts && (
        <div
          className={clsx(
            'shrink-0 px-3 py-2 text-sm border-b',
            mergeResult.success
              ? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400 border-green-200 dark:border-green-800'
              : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 border-red-200 dark:border-red-800',
          )}
        >
          <div className="flex items-center justify-between">
            <span>
              {mergeResult.success ? t('branchesTab.mergeSuccess') : mergeResult.error || t('branchesTab.mergeFailed')}
            </span>
            <button onClick={() => setMergeResult(null)} className="text-xs underline ml-2">
              {t('branchesTab.dismiss')}
            </button>
          </div>
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="shrink-0 px-3 py-2 text-sm bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 border-b border-red-200 dark:border-red-800">
          {error}
        </div>
      )}

      {/* Loading */}
      {isLoading && localBranches.length === 0 && (
        <div className="flex items-center justify-center py-8">
          <div className="animate-spin h-5 w-5 border-2 border-gray-400 border-t-transparent rounded-full" />
        </div>
      )}

      {/* Content */}
      <div className="flex-1 min-h-0 flex flex-col">
        {/* Branches Section (~60%) */}
        <div className="flex-[6] min-h-0 border-b border-gray-200 dark:border-gray-700 overflow-hidden flex flex-col">
          <BranchList
            localBranches={localBranches}
            remoteBranches={remoteBranches}
            currentBranch={currentBranch}
            repoPath={repoPath}
            onRefresh={refresh}
            onMerge={handleMerge}
            onDelete={handleDelete}
            onRename={handleRename}
          />
        </div>

        {/* Worktrees Section (~40%) */}
        <div className="flex-[4] min-h-0 overflow-hidden flex flex-col">
          <WorktreeList repoPath={repoPath} />
        </div>
      </div>

      {/* Action Dialogs */}
      {dialogType && (
        <BranchActions
          type={dialogType}
          branch={dialogBranch}
          branches={localBranches}
          repoPath={repoPath}
          onClose={handleDialogClose}
          onSuccess={handleDialogSuccess}
        />
      )}
    </div>
  );
}

export default BranchesTab;
