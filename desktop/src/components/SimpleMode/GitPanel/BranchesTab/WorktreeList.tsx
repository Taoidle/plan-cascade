/**
 * WorktreeList Component
 *
 * Displays active git worktrees with status, branch, and action buttons.
 * Integrates with the Tauri worktree commands.
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

import { useState, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import type { Worktree, WorktreeStatus, CommandResponse } from '../../../../types/git';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface WorktreeListProps {
  repoPath: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function statusLabel(status: WorktreeStatus): string {
  const labels: Record<WorktreeStatus, string> = {
    creating: 'Creating',
    active: 'Active',
    in_progress: 'In Progress',
    ready: 'Ready',
    merging: 'Merging',
    completed: 'Completed',
    error: 'Error',
  };
  return labels[status] || status;
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
  const [force, setForce] = useState(false);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/40 dark:bg-black/60" onClick={onCancel} />
      <div className="relative z-10 w-full max-w-sm mx-4 bg-white dark:bg-gray-800 rounded-xl shadow-xl border border-gray-200 dark:border-gray-700 p-5">
        <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100 mb-2">
          Remove Worktree
        </h3>
        <p className="text-sm text-gray-700 dark:text-gray-300 mb-3">
          Are you sure you want to remove the worktree{' '}
          <code className="px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-xs font-mono">
            {worktree.name}
          </code>
          ?
        </p>

        <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300 mb-4">
          <input
            type="checkbox"
            checked={force}
            onChange={(e) => setForce(e.target.checked)}
            className="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
          />
          Force remove (even with uncommitted changes)
        </label>

        <div className="flex justify-end gap-2">
          <button
            onClick={onCancel}
            className="px-3 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={() => onConfirm(force)}
            className="px-3 py-1.5 text-sm rounded-lg font-medium text-white bg-red-600 hover:bg-red-700 transition-colors"
          >
            Remove
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// WorktreeCard
// ---------------------------------------------------------------------------

function WorktreeCard({
  worktree,
  onRemove,
}: {
  worktree: Worktree;
  onRemove: (worktree: Worktree) => void;
}) {
  const isTerminal = worktree.status === 'completed' || worktree.status === 'error';

  return (
    <div
      className={clsx(
        'px-3 py-2 rounded-lg border transition-colors',
        worktree.status === 'active'
          ? 'border-green-200 dark:border-green-800 bg-green-50/50 dark:bg-green-900/10'
          : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800/50'
      )}
    >
      <div className="flex items-center justify-between mb-1">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
            {worktree.name}
          </span>
          <span
            className={clsx(
              'shrink-0 px-1.5 py-0.5 text-2xs font-medium rounded',
              statusColor(worktree.status)
            )}
          >
            {statusLabel(worktree.status)}
          </span>
        </div>

        <div className="flex items-center gap-1 shrink-0">
          {!isTerminal && (
            <button
              onClick={() => onRemove(worktree)}
              className="p-1 rounded text-gray-400 hover:text-red-500 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
              title="Remove worktree"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
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
  const [worktrees, setWorktrees] = useState<Worktree[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<Worktree | null>(null);

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

  useEffect(() => {
    fetchWorktrees();
  }, [fetchWorktrees]);

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
    [removeTarget, repoPath, fetchWorktrees]
  );

  const handleNewWorktree = useCallback(() => {
    // For now, this is a placeholder. In a full implementation, this would
    // open a dialog to create a new worktree.
    // The worktree creation is complex (needs task name, target branch, etc.)
    // and is better suited for the task mode workflow.
  }, []);

  // Filter out completed/terminal worktrees for display
  const activeWorktrees = worktrees.filter(
    (wt) => wt.status !== 'completed'
  );

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="shrink-0 flex items-center justify-between px-3 py-2 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <h4 className="text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400">
            Worktrees
          </h4>
          <span className="text-2xs text-gray-400 dark:text-gray-500">
            ({activeWorktrees.length})
          </span>
        </div>
        <button
          onClick={handleNewWorktree}
          className="p-1 rounded text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
          title="New Worktree"
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
            <svg className="w-8 h-8 mb-2 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
            <span>No active worktrees</span>
          </div>
        )}

        {activeWorktrees.map((wt) => (
          <WorktreeCard
            key={wt.id}
            worktree={wt}
            onRemove={setRemoveTarget}
          />
        ))}
      </div>

      {/* Remove Confirmation Dialog */}
      {removeTarget && (
        <ConfirmRemoveDialog
          worktree={removeTarget}
          onConfirm={handleRemove}
          onCancel={() => setRemoveTarget(null)}
        />
      )}
    </div>
  );
}
