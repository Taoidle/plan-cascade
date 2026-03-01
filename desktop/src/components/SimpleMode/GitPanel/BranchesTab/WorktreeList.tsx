/**
 * WorktreeList Component
 *
 * Displays active git worktrees with status, branch, and action buttons.
 * Integrates with the Tauri worktree commands.
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import type { BranchInfo, Worktree, WorktreeStatus } from '../../../../types/git';
import type { CommandResponse } from '../../../../lib/tauri';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface WorktreeListProps {
  repoPath: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function statusLabelKey(status: WorktreeStatus): string {
  const keys: Record<WorktreeStatus, string> = {
    creating: 'worktreeList.statusCreating',
    active: 'worktreeList.statusActive',
    in_progress: 'worktreeList.statusInProgress',
    ready: 'worktreeList.statusReady',
    merging: 'worktreeList.statusMerging',
    completed: 'worktreeList.statusCompleted',
    error: 'worktreeList.statusError',
  };
  return keys[status] || status;
}

function statusColor(status: WorktreeStatus): string {
  switch (status) {
    case 'active':
      return 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400';
    case 'in_progress':
      return 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-400';
    case 'ready':
      return 'bg-indigo-100 dark:bg-indigo-900/30 text-indigo-700 dark:text-indigo-400';
    case 'creating':
    case 'merging':
      return 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-400';
    case 'completed':
      return 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400';
    case 'error':
      return 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-400';
    default:
      return 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400';
  }
}

// ---------------------------------------------------------------------------
// Confirm Dialog
// ---------------------------------------------------------------------------

function ConfirmRemoveDialog({
  worktree,
  onConfirm,
  onCancel,
}: {
  worktree: Worktree;
  onConfirm: (force: boolean) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation('git');
  const [force, setForce] = useState(false);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/40 dark:bg-black/60" onClick={onCancel} />
      <div className="relative z-10 w-full max-w-sm mx-4 bg-white dark:bg-gray-800 rounded-xl shadow-xl border border-gray-200 dark:border-gray-700 p-5">
        <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100 mb-2">
          {t('worktreeList.removeWorktree')}
        </h3>
        <p className="text-sm text-gray-700 dark:text-gray-300 mb-3">
          {t('worktreeList.removeConfirm')}{' '}
          <code className="px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-xs font-mono">{worktree.name}</code>?
        </p>

        <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300 mb-4">
          <input
            type="checkbox"
            checked={force}
            onChange={(e) => setForce(e.target.checked)}
            className="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
          />
          {t('worktreeList.forceRemove')}
        </label>

        <div className="flex justify-end gap-2">
          <button
            onClick={onCancel}
            className="px-3 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
          >
            {t('worktreeList.cancel')}
          </button>
          <button
            onClick={() => onConfirm(force)}
            className="px-3 py-1.5 text-sm rounded-lg font-medium text-white bg-red-600 hover:bg-red-700 transition-colors"
          >
            {t('worktreeList.remove')}
          </button>
        </div>
      </div>
    </div>
  );
}

function CreateWorktreeDialog({
  taskName,
  setTaskName,
  targetBranch,
  setTargetBranch,
  basePath,
  setBasePath,
  branches,
  isCreating,
  error,
  onCreate,
  onCancel,
}: {
  taskName: string;
  setTaskName: (value: string) => void;
  targetBranch: string;
  setTargetBranch: (value: string) => void;
  basePath: string;
  setBasePath: (value: string) => void;
  branches: BranchInfo[];
  isCreating: boolean;
  error: string | null;
  onCreate: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation('git');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/40 dark:bg-black/60" onClick={onCancel} />
      <div className="relative z-10 w-full max-w-md mx-4 bg-white dark:bg-gray-800 rounded-xl shadow-xl border border-gray-200 dark:border-gray-700 p-5">
        <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100 mb-3">
          {t('worktreeList.createTitle', { defaultValue: 'Create Worktree' })}
        </h3>

        <div className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-gray-600 dark:text-gray-300 mb-1">
              {t('worktreeList.taskNameLabel', { defaultValue: 'Task name' })}
            </label>
            <input
              type="text"
              value={taskName}
              onChange={(e) => setTaskName(e.target.value)}
              placeholder={t('worktreeList.taskNamePlaceholder', { defaultValue: 'feature-user-auth' })}
              className="w-full px-2 py-1.5 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 text-gray-800 dark:text-gray-200"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-gray-600 dark:text-gray-300 mb-1">
              {t('worktreeList.targetBranchLabel', { defaultValue: 'Target branch' })}
            </label>
            <select
              value={targetBranch}
              onChange={(e) => setTargetBranch(e.target.value)}
              className="w-full px-2 py-1.5 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 text-gray-800 dark:text-gray-200"
            >
              {branches.map((branch) => (
                <option key={branch.name} value={branch.name}>
                  {branch.name}
                  {branch.is_head ? ` (${t('worktreeList.current', { defaultValue: 'current' })})` : ''}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-xs font-medium text-gray-600 dark:text-gray-300 mb-1">
              {t('worktreeList.basePathLabel', { defaultValue: 'Base path (optional)' })}
            </label>
            <input
              type="text"
              value={basePath}
              onChange={(e) => setBasePath(e.target.value)}
              placeholder={t('worktreeList.basePathPlaceholder', { defaultValue: '.worktree' })}
              className="w-full px-2 py-1.5 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 text-gray-800 dark:text-gray-200"
            />
          </div>

          {error && <p className="text-xs text-red-600 dark:text-red-400">{error}</p>}
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <button
            onClick={onCancel}
            className="px-3 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
          >
            {t('worktreeList.cancel')}
          </button>
          <button
            onClick={onCreate}
            disabled={isCreating}
            className={clsx(
              'px-3 py-1.5 text-sm rounded-lg font-medium text-white transition-colors',
              isCreating ? 'bg-blue-400 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700',
            )}
          >
            {isCreating
              ? t('worktreeList.creating', { defaultValue: 'Creating...' })
              : t('worktreeList.createButton', { defaultValue: 'Create' })}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// WorktreeCard
// ---------------------------------------------------------------------------

function WorktreeCard({ worktree, onRemove }: { worktree: Worktree; onRemove: (worktree: Worktree) => void }) {
  const { t } = useTranslation('git');
  const isTerminal = worktree.status === 'completed' || worktree.status === 'error';

  return (
    <div
      className={clsx(
        'px-3 py-2 rounded-lg border transition-colors',
        worktree.status === 'active'
          ? 'border-green-200 dark:border-green-800 bg-green-50/50 dark:bg-green-900/10'
          : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800/50',
      )}
    >
      <div className="flex items-center justify-between mb-1">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">{worktree.name}</span>
          <span className={clsx('shrink-0 px-1.5 py-0.5 text-2xs font-medium rounded', statusColor(worktree.status))}>
            {t(statusLabelKey(worktree.status))}
          </span>
        </div>

        <div className="flex items-center gap-1 shrink-0">
          {!isTerminal && (
            <button
              onClick={() => onRemove(worktree)}
              className="p-1 rounded text-gray-400 hover:text-red-500 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
              title={t('worktreeList.removeWorktree')}
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                />
              </svg>
            </button>
          )}
        </div>
      </div>

      <div className="flex items-center gap-3 text-2xs text-gray-500 dark:text-gray-400">
        <span className="flex items-center gap-1">
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
          </svg>
          {worktree.branch}
        </span>
        <span className="truncate" title={worktree.path}>
          {worktree.path}
        </span>
      </div>

      {worktree.error && (
        <div className="mt-1 text-2xs text-red-600 dark:text-red-400 truncate" title={worktree.error}>
          {worktree.error}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// WorktreeList Component
// ---------------------------------------------------------------------------

export function WorktreeList({ repoPath }: WorktreeListProps) {
  const { t } = useTranslation('git');
  const [worktrees, setWorktrees] = useState<Worktree[]>([]);
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<Worktree | null>(null);
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [taskName, setTaskName] = useState('');
  const [targetBranch, setTargetBranch] = useState('');
  const [basePath, setBasePath] = useState('');
  const [createError, setCreateError] = useState<string | null>(null);

  const fetchWorktrees = useCallback(async () => {
    if (!repoPath) return;
    setIsLoading(true);
    try {
      const res = await invoke<CommandResponse<Worktree[]>>('list_worktrees', {
        repoPath,
      });
      if (res.success && res.data) {
        setWorktrees(res.data);
      }
    } catch {
      // Silently fail
    } finally {
      setIsLoading(false);
    }
  }, [repoPath]);

  const fetchBranches = useCallback(async () => {
    if (!repoPath) return;
    try {
      const res = await invoke<CommandResponse<BranchInfo[]>>('git_list_branches', { repoPath });
      if (res.success && res.data) {
        const data = res.data;
        setBranches(data);
        const current = data.find((branch) => branch.is_head);
        setTargetBranch((prev) => {
          if (prev && data.some((branch) => branch.name === prev)) {
            return prev;
          }
          return current?.name || data[0]?.name || '';
        });
      }
    } catch {
      // Keep branch list empty and rely on server validation
    }
  }, [repoPath]);

  useEffect(() => {
    fetchWorktrees();
  }, [fetchWorktrees]);

  useEffect(() => {
    if (showCreateDialog) {
      void fetchBranches();
    }
  }, [showCreateDialog, fetchBranches]);

  const handleRemove = useCallback(
    async (force: boolean) => {
      if (!removeTarget) return;
      try {
        await invoke<CommandResponse<void>>('remove_worktree', {
          repoPath,
          worktreeId: removeTarget.id,
          force,
        });
        setRemoveTarget(null);
        await fetchWorktrees();
      } catch {
        // Silently fail
      }
    },
    [removeTarget, repoPath, fetchWorktrees],
  );

  const handleCreateWorktree = useCallback(async () => {
    if (!repoPath) return;
    const trimmedTaskName = taskName.trim();
    if (!trimmedTaskName) {
      setCreateError(t('worktreeList.taskNameRequired', { defaultValue: 'Task name is required' }));
      return;
    }
    if (!targetBranch.trim()) {
      setCreateError(t('worktreeList.targetBranchRequired', { defaultValue: 'Target branch is required' }));
      return;
    }

    setIsCreating(true);
    setCreateError(null);
    try {
      const res = await invoke<CommandResponse<Worktree>>('create_worktree', {
        repoPath,
        taskName: trimmedTaskName,
        targetBranch: targetBranch.trim(),
        basePath: basePath.trim() || null,
        prdPath: null,
        executionMode: 'auto',
      });
      if (!res.success) {
        setCreateError(res.error || 'Failed to create worktree');
        return;
      }

      setShowCreateDialog(false);
      setTaskName('');
      setBasePath('');
      setCreateError(null);
      await fetchWorktrees();
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCreating(false);
    }
  }, [repoPath, taskName, targetBranch, basePath, fetchWorktrees, t]);

  const handleNewWorktree = useCallback(() => {
    setTaskName('');
    setBasePath('');
    setCreateError(null);
    setShowCreateDialog(true);
  }, []);

  // Filter out completed/terminal worktrees for display
  const activeWorktrees = worktrees.filter((wt) => wt.status !== 'completed');

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="shrink-0 flex items-center justify-between px-3 py-2 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <h4 className="text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400">
            {t('worktreeList.title')}
          </h4>
          <span className="text-2xs text-gray-400 dark:text-gray-500">({activeWorktrees.length})</span>
        </div>
        <button
          onClick={handleNewWorktree}
          className="p-1 rounded text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
          title={t('worktreeList.newWorktree')}
        >
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 min-h-0 overflow-y-auto p-2 space-y-2">
        {isLoading && activeWorktrees.length === 0 && (
          <div className="flex items-center justify-center py-6">
            <div className="animate-spin h-4 w-4 border-2 border-gray-400 border-t-transparent rounded-full" />
          </div>
        )}

        {!isLoading && activeWorktrees.length === 0 && (
          <div className="flex flex-col items-center justify-center py-6 text-sm text-gray-500 dark:text-gray-400">
            <svg
              className="w-8 h-8 mb-2 text-gray-300 dark:text-gray-600"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={1.5}
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
              />
            </svg>
            <span>{t('worktreeList.noActiveWorktrees')}</span>
          </div>
        )}

        {activeWorktrees.map((wt) => (
          <WorktreeCard key={wt.id} worktree={wt} onRemove={setRemoveTarget} />
        ))}
      </div>

      {/* Remove Confirmation Dialog */}
      {removeTarget && (
        <ConfirmRemoveDialog worktree={removeTarget} onConfirm={handleRemove} onCancel={() => setRemoveTarget(null)} />
      )}

      {showCreateDialog && (
        <CreateWorktreeDialog
          taskName={taskName}
          setTaskName={setTaskName}
          targetBranch={targetBranch}
          setTargetBranch={setTargetBranch}
          basePath={basePath}
          setBasePath={setBasePath}
          branches={branches}
          isCreating={isCreating}
          error={createError}
          onCreate={handleCreateWorktree}
          onCancel={() => setShowCreateDialog(false)}
        />
      )}
    </div>
  );
}
