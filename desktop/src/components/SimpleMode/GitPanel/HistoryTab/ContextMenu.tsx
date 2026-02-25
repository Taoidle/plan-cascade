/**
 * ContextMenu Component
 *
 * Right-click context menu for commits in the history graph.
 * Provides actions: Copy SHA, Create branch, Cherry-pick, Revert.
 * Includes confirmation dialogs for destructive operations.
 *
 * Feature-003: Commit History Graph with SVG Visualization
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from '../../../../lib/tauri';
import type { CommitNode } from '../../../../types/git';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ContextMenuState {
  /** SHA of the commit that was right-clicked */
  sha: string;
  /** X position of the context menu */
  x: number;
  /** Y position of the context menu */
  y: number;
}

interface ContextMenuProps {
  /** Context menu state (position and commit SHA) */
  state: ContextMenuState;
  /** Repository path */
  repoPath: string | null;
  /** The commit associated with this context menu */
  commit: CommitNode | null;
  /** Callback to close the context menu */
  onClose: () => void;
  /** Callback to refresh the graph after a mutation */
  onRefresh: () => void;
}

type DialogState =
  | { type: 'none' }
  | { type: 'create-branch'; branchName: string }
  | { type: 'cherry-pick-confirm' }
  | { type: 'revert-confirm' }
  | { type: 'loading'; action: string }
  | { type: 'error'; message: string }
  | { type: 'success'; message: string };

// ---------------------------------------------------------------------------
// ContextMenu Component
// ---------------------------------------------------------------------------

export function ContextMenu({ state, repoPath, commit, onClose, onRefresh }: ContextMenuProps) {
  const { t } = useTranslation('git');
  const [dialogState, setDialogState] = useState<DialogState>({ type: 'none' });
  const menuRef = useRef<HTMLDivElement>(null);
  const branchInputRef = useRef<HTMLInputElement>(null);

  // ---------------------------------------------------------------------------
  // Position the menu within viewport bounds
  // ---------------------------------------------------------------------------

  const [position, setPosition] = useState({ x: state.x, y: state.y });

  useEffect(() => {
    const menu = menuRef.current;
    if (!menu) return;

    const rect = menu.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    let x = state.x;
    let y = state.y;

    if (x + rect.width > viewportWidth) {
      x = viewportWidth - rect.width - 8;
    }
    if (y + rect.height > viewportHeight) {
      y = viewportHeight - rect.height - 8;
    }

    setPosition({ x: Math.max(4, x), y: Math.max(4, y) });
  }, [state.x, state.y]);

  // Focus branch input when dialog opens
  useEffect(() => {
    if (dialogState.type === 'create-branch') {
      setTimeout(() => branchInputRef.current?.focus(), 50);
    }
  }, [dialogState]);

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  const handleCopySha = useCallback(async () => {
    if (!commit) return;
    try {
      await navigator.clipboard.writeText(commit.sha);
      setDialogState({ type: 'success', message: t('contextMenu.shaCopied') });
      setTimeout(onClose, 800);
    } catch {
      setDialogState({ type: 'error', message: t('contextMenu.copyFailed') });
    }
  }, [commit, onClose]);

  const handleCreateBranch = useCallback(
    async (name: string) => {
      if (!repoPath || !commit || !name.trim()) return;

      setDialogState({ type: 'loading', action: t('contextMenu.creatingBranch') });

      try {
        const result = await invoke<CommandResponse<void>>('git_create_branch', {
          repoPath,
          name: name.trim(),
          base: commit.sha,
        });

        if (result.success) {
          setDialogState({ type: 'success', message: t('contextMenu.branchCreated', { name: name.trim() }) });
          setTimeout(() => {
            onRefresh();
            onClose();
          }, 800);
        } else {
          setDialogState({ type: 'error', message: result.error || t('contextMenu.createBranchFailed') });
        }
      } catch (err) {
        setDialogState({
          type: 'error',
          message: err instanceof Error ? err.message : t('contextMenu.createBranchFailed'),
        });
      }
    },
    [repoPath, commit, onRefresh, onClose],
  );

  const handleCherryPick = useCallback(async () => {
    if (!repoPath || !commit) return;

    setDialogState({ type: 'loading', action: t('contextMenu.cherryPicking') });

    try {
      // Cherry-pick via git command (no dedicated Tauri command, use shell)
      const { Command } = await import('@tauri-apps/plugin-shell');
      const cmd = Command.create('git', ['cherry-pick', commit.sha], {
        cwd: repoPath,
      });
      const output = await cmd.execute();

      if (output.code === 0) {
        setDialogState({ type: 'success', message: t('contextMenu.cherryPicked', { sha: commit.short_sha }) });
        setTimeout(() => {
          onRefresh();
          onClose();
        }, 800);
      } else {
        setDialogState({
          type: 'error',
          message: output.stderr || t('contextMenu.cherryPickFailed'),
        });
      }
    } catch (err) {
      setDialogState({
        type: 'error',
        message: err instanceof Error ? err.message : t('contextMenu.cherryPickFailed'),
      });
    }
  }, [repoPath, commit, onRefresh, onClose]);

  const handleRevert = useCallback(async () => {
    if (!repoPath || !commit) return;

    setDialogState({ type: 'loading', action: t('contextMenu.reverting') });

    try {
      const { Command } = await import('@tauri-apps/plugin-shell');
      const cmd = Command.create('git', ['revert', '--no-edit', commit.sha], {
        cwd: repoPath,
      });
      const output = await cmd.execute();

      if (output.code === 0) {
        setDialogState({ type: 'success', message: t('contextMenu.reverted', { sha: commit.short_sha }) });
        setTimeout(() => {
          onRefresh();
          onClose();
        }, 800);
      } else {
        setDialogState({
          type: 'error',
          message: output.stderr || t('contextMenu.revertFailed'),
        });
      }
    } catch (err) {
      setDialogState({
        type: 'error',
        message: err instanceof Error ? err.message : t('contextMenu.revertFailed'),
      });
    }
  }, [repoPath, commit, onRefresh, onClose]);

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  if (!commit) return null;

  return (
    <>
      {/* Backdrop for clicking outside */}
      <div
        className="fixed inset-0 z-50"
        onClick={(e) => {
          e.stopPropagation();
          onClose();
        }}
      />

      {/* Menu */}
      <div
        ref={menuRef}
        className={clsx(
          'fixed z-50 min-w-[180px]',
          'bg-white dark:bg-gray-800',
          'border border-gray-200 dark:border-gray-700',
          'rounded-lg shadow-lg',
          'py-1',
          'animate-fade-in',
        )}
        style={{ left: position.x, top: position.y }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Default menu items */}
        {dialogState.type === 'none' && (
          <>
            {/* Copy SHA */}
            <button
              onClick={handleCopySha}
              className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
            >
              <svg className="w-3.5 h-3.5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                />
              </svg>
              {t('contextMenu.copySha')}
              <span className="ml-auto text-[10px] text-gray-400 font-mono">{commit.short_sha}</span>
            </button>

            {/* Separator */}
            <div className="my-1 border-t border-gray-200 dark:border-gray-700" />

            {/* Create branch here */}
            <button
              onClick={() => setDialogState({ type: 'create-branch', branchName: '' })}
              className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
            >
              <svg className="w-3.5 h-3.5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
              </svg>
              {t('contextMenu.createBranchHere')}
            </button>

            {/* Cherry-pick */}
            <button
              onClick={() => setDialogState({ type: 'cherry-pick-confirm' })}
              className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
            >
              <svg className="w-3.5 h-3.5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"
                />
              </svg>
              {t('contextMenu.cherryPick')}
            </button>

            {/* Revert */}
            <button
              onClick={() => setDialogState({ type: 'revert-confirm' })}
              className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"
                />
              </svg>
              {t('contextMenu.revert')}
            </button>
          </>
        )}

        {/* Create branch dialog */}
        {dialogState.type === 'create-branch' && (
          <div className="px-3 py-2 space-y-2">
            <p className="text-xs font-medium text-gray-700 dark:text-gray-300">
              {t('contextMenu.createBranchAt', { sha: commit.short_sha })}
            </p>
            <input
              ref={branchInputRef}
              type="text"
              value={dialogState.branchName}
              onChange={(e) => setDialogState({ type: 'create-branch', branchName: e.target.value })}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && dialogState.branchName.trim()) {
                  handleCreateBranch(dialogState.branchName);
                }
                if (e.key === 'Escape') {
                  setDialogState({ type: 'none' });
                }
              }}
              placeholder={t('contextMenu.branchName')}
              className={clsx(
                'w-full px-2 py-1.5 text-xs rounded-md',
                'bg-white dark:bg-gray-900',
                'border border-gray-200 dark:border-gray-700',
                'text-gray-800 dark:text-gray-200',
                'placeholder-gray-400 dark:placeholder-gray-500',
                'focus:outline-none focus:ring-1 focus:ring-blue-500',
              )}
            />
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setDialogState({ type: 'none' })}
                className="px-2 py-1 text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
              >
                {t('contextMenu.cancel')}
              </button>
              <button
                onClick={() => handleCreateBranch(dialogState.branchName)}
                disabled={!dialogState.branchName.trim()}
                className={clsx(
                  'px-2 py-1 text-xs rounded-md',
                  'bg-blue-500 text-white',
                  'hover:bg-blue-600',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                  'transition-colors',
                )}
              >
                {t('contextMenu.create')}
              </button>
            </div>
          </div>
        )}

        {/* Cherry-pick confirmation */}
        {dialogState.type === 'cherry-pick-confirm' && (
          <div className="px-3 py-2 space-y-2">
            <p className="text-xs text-gray-700 dark:text-gray-300">
              {t('contextMenu.cherryPickConfirm', { sha: commit.short_sha })}
            </p>
            <p className="text-[10px] text-gray-500 dark:text-gray-400">{t('contextMenu.cherryPickWarning')}</p>
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setDialogState({ type: 'none' })}
                className="px-2 py-1 text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
              >
                {t('contextMenu.cancel')}
              </button>
              <button
                onClick={handleCherryPick}
                className="px-2 py-1 text-xs rounded-md bg-blue-500 text-white hover:bg-blue-600 transition-colors"
              >
                {t('contextMenu.cherryPick')}
              </button>
            </div>
          </div>
        )}

        {/* Revert confirmation */}
        {dialogState.type === 'revert-confirm' && (
          <div className="px-3 py-2 space-y-2">
            <p className="text-xs text-gray-700 dark:text-gray-300">
              {t('contextMenu.revertConfirm', { sha: commit.short_sha })}
            </p>
            <p className="text-[10px] text-gray-500 dark:text-gray-400">{t('contextMenu.revertWarning')}</p>
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setDialogState({ type: 'none' })}
                className="px-2 py-1 text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
              >
                {t('contextMenu.cancel')}
              </button>
              <button
                onClick={handleRevert}
                className="px-2 py-1 text-xs rounded-md bg-red-500 text-white hover:bg-red-600 transition-colors"
              >
                {t('contextMenu.revert')}
              </button>
            </div>
          </div>
        )}

        {/* Loading state */}
        {dialogState.type === 'loading' && (
          <div className="px-3 py-3 flex items-center gap-2 text-xs text-gray-600 dark:text-gray-400">
            <div className="animate-spin h-3 w-3 border border-gray-400 border-t-transparent rounded-full" />
            {dialogState.action}
          </div>
        )}

        {/* Success state */}
        {dialogState.type === 'success' && (
          <div className="px-3 py-3 flex items-center gap-2 text-xs text-green-600 dark:text-green-400">
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
            {dialogState.message}
          </div>
        )}

        {/* Error state */}
        {dialogState.type === 'error' && (
          <div className="px-3 py-2 space-y-2">
            <p className="text-xs text-red-600 dark:text-red-400">{dialogState.message}</p>
            <button
              onClick={() => setDialogState({ type: 'none' })}
              className="px-2 py-1 text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
            >
              {t('contextMenu.dismiss')}
            </button>
          </div>
        )}
      </div>
    </>
  );
}

export default ContextMenu;
