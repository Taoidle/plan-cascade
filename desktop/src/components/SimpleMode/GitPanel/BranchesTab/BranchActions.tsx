/**
 * BranchActions Component
 *
 * Modal dialogs for branch operations: Create, Delete, Rename.
 * All dialogs are modal overlays with backdrop.
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import type { BranchInfo, CommandResponse } from '../../../../types/git';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type BranchActionType = 'create' | 'delete' | 'rename';

interface BranchActionsProps {
  type: BranchActionType;
  branch: BranchInfo | null;
  branches: BranchInfo[];
  repoPath: string;
  onClose: () => void;
  onSuccess: () => void;
}

// ---------------------------------------------------------------------------
// Dialog Shell
// ---------------------------------------------------------------------------

function DialogShell({ title, onClose, children }: { title: string; onClose: () => void; children: React.ReactNode }) {
  const dialogRef = useRef<HTMLDivElement>(null);

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [onClose]);

  // Focus trap
  useEffect(() => {
    const firstInput = dialogRef.current?.querySelector('input, select, button') as HTMLElement | null;
    firstInput?.focus();
  }, []);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40 dark:bg-black/60" onClick={onClose} />

      {/* Dialog */}
      <div
        ref={dialogRef}
        className="relative z-10 w-full max-w-md mx-4 bg-white dark:bg-gray-800 rounded-xl shadow-xl border border-gray-200 dark:border-gray-700"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-200 dark:border-gray-700">
          <h2 className="text-base font-semibold text-gray-900 dark:text-gray-100">{title}</h2>
          <button
            onClick={onClose}
            className="p-1 rounded-lg text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Body */}
        <div className="px-5 py-4">{children}</div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// CreateBranchDialog
// ---------------------------------------------------------------------------

function CreateBranchDialog({
  branches,
  repoPath,
  onClose,
  onSuccess,
}: {
  branches: BranchInfo[];
  repoPath: string;
  onClose: () => void;
  onSuccess: () => void;
}) {
  const { t } = useTranslation('git');
  const [name, setName] = useState('');
  const [base, setBase] = useState(() => {
    const current = branches.find((b) => b.is_head);
    return current?.name || 'HEAD';
  });
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreate = useCallback(async () => {
    if (!name.trim()) {
      setError(t('branchActions.branchNameRequired'));
      return;
    }

    // Validate branch name (basic)
    if (/\s/.test(name) || name.includes('..') || name.startsWith('-')) {
      setError(t('branchActions.invalidBranchName'));
      return;
    }

    setIsCreating(true);
    setError(null);

    try {
      const res = await invoke<CommandResponse<void>>('git_create_branch', {
        repoPath,
        name: name.trim(),
        base,
      });
      if (res.success) {
        onSuccess();
      } else {
        setError(res.error || 'Failed to create branch');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCreating(false);
    }
  }, [name, base, repoPath, onSuccess]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !isCreating) {
        handleCreate();
      }
    },
    [handleCreate, isCreating],
  );

  return (
    <DialogShell title={t('branchActions.createBranch')} onClose={onClose}>
      <div className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            {t('branchActions.branchNameLabel')}
          </label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('branchActions.branchNamePlaceholder')}
            className="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 dark:focus:ring-blue-400"
            autoFocus
          />
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            {t('branchActions.baseBranch')}
          </label>
          <select
            value={base}
            onChange={(e) => setBase(e.target.value)}
            className="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500 dark:focus:ring-blue-400"
          >
            {branches.map((b) => (
              <option key={b.name} value={b.name}>
                {b.name}
                {b.is_head ? ` ${t('branchActions.current')}` : ''}
              </option>
            ))}
          </select>
        </div>

        {error && <div className="text-sm text-red-600 dark:text-red-400">{error}</div>}

        <div className="flex justify-end gap-2 pt-2">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
          >
            {t('branchActions.cancel')}
          </button>
          <button
            onClick={handleCreate}
            disabled={isCreating || !name.trim()}
            className={clsx(
              'px-4 py-2 text-sm rounded-lg font-medium text-white transition-colors',
              isCreating || !name.trim() ? 'bg-blue-400 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700',
            )}
          >
            {isCreating ? t('branchActions.creating') : t('branchActions.create')}
          </button>
        </div>
      </div>
    </DialogShell>
  );
}

// ---------------------------------------------------------------------------
// DeleteBranchDialog
// ---------------------------------------------------------------------------

function DeleteBranchDialog({
  branch,
  repoPath,
  onClose,
  onSuccess,
}: {
  branch: BranchInfo;
  repoPath: string;
  onClose: () => void;
  onSuccess: () => void;
}) {
  const { t } = useTranslation('git');
  const [force, setForce] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmMainDelete, setConfirmMainDelete] = useState(false);

  const isProtected = /^(main|master)$/.test(branch.name);

  const handleDelete = useCallback(async () => {
    if (isProtected && !confirmMainDelete) {
      setConfirmMainDelete(true);
      return;
    }

    setIsDeleting(true);
    setError(null);

    try {
      const res = await invoke<CommandResponse<void>>('git_delete_branch', {
        repoPath,
        name: branch.name,
        force,
      });
      if (res.success) {
        onSuccess();
      } else {
        setError(res.error || 'Failed to delete branch');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsDeleting(false);
    }
  }, [branch, force, repoPath, onSuccess, isProtected, confirmMainDelete]);

  return (
    <DialogShell title={t('branchActions.deleteBranch')} onClose={onClose}>
      <div className="space-y-4">
        <p className="text-sm text-gray-700 dark:text-gray-300">
          {t('branchActions.deleteConfirm')}{' '}
          <code className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-sm font-mono">{branch.name}</code>?
        </p>

        {isProtected && (
          <div className="p-3 rounded-lg bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800">
            <p className="text-sm font-medium text-amber-700 dark:text-amber-400">
              {t('branchActions.protectedWarning')} ({branch.name})
            </p>
            {confirmMainDelete && (
              <p className="text-xs text-amber-600 dark:text-amber-500 mt-1">{t('branchActions.confirmDeleteAgain')}</p>
            )}
          </div>
        )}

        <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
          <input
            type="checkbox"
            checked={force}
            onChange={(e) => setForce(e.target.checked)}
            className="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
          />
          {t('branchActions.forceDelete')}
        </label>

        {error && <div className="text-sm text-red-600 dark:text-red-400">{error}</div>}

        <div className="flex justify-end gap-2 pt-2">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
          >
            {t('branchActions.cancel')}
          </button>
          <button
            onClick={handleDelete}
            disabled={isDeleting}
            className={clsx(
              'px-4 py-2 text-sm rounded-lg font-medium text-white transition-colors',
              isDeleting ? 'bg-red-400 cursor-not-allowed' : 'bg-red-600 hover:bg-red-700',
            )}
          >
            {isDeleting
              ? t('branchActions.deleting')
              : confirmMainDelete && isProtected
                ? t('branchActions.confirmDelete')
                : t('branchActions.deleteBranch')}
          </button>
        </div>
      </div>
    </DialogShell>
  );
}

// ---------------------------------------------------------------------------
// RenameBranchDialog
// ---------------------------------------------------------------------------

function RenameBranchDialog({
  branch,
  repoPath,
  onClose,
  onSuccess,
}: {
  branch: BranchInfo;
  repoPath: string;
  onClose: () => void;
  onSuccess: () => void;
}) {
  const { t } = useTranslation('git');
  const [newName, setNewName] = useState(branch.name);
  const [isRenaming, setIsRenaming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleRename = useCallback(async () => {
    if (!newName.trim() || newName.trim() === branch.name) {
      setError(t('branchActions.newNameDifferent'));
      return;
    }

    if (/\s/.test(newName) || newName.includes('..') || newName.startsWith('-')) {
      setError(t('branchActions.invalidBranchName'));
      return;
    }

    setIsRenaming(true);
    setError(null);

    try {
      const res = await invoke<CommandResponse<void>>('git_rename_branch', {
        repoPath,
        oldName: branch.name,
        newName: newName.trim(),
      });
      if (res.success) {
        onSuccess();
      } else {
        setError(res.error || 'Failed to rename branch');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsRenaming(false);
    }
  }, [branch, newName, repoPath, onSuccess]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !isRenaming) {
        handleRename();
      }
    },
    [handleRename, isRenaming],
  );

  return (
    <DialogShell title={t('branchActions.renameBranch')} onClose={onClose}>
      <div className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            {t('branchActions.currentName')}
          </label>
          <input
            type="text"
            value={branch.name}
            readOnly
            className="w-full px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-500 dark:text-gray-400 cursor-not-allowed"
          />
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            {t('branchActions.newName')}
          </label>
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('branchActions.newNamePlaceholder')}
            className="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 dark:focus:ring-blue-400"
            autoFocus
          />
        </div>

        {error && <div className="text-sm text-red-600 dark:text-red-400">{error}</div>}

        <div className="flex justify-end gap-2 pt-2">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
          >
            {t('branchActions.cancel')}
          </button>
          <button
            onClick={handleRename}
            disabled={isRenaming || !newName.trim() || newName.trim() === branch.name}
            className={clsx(
              'px-4 py-2 text-sm rounded-lg font-medium text-white transition-colors',
              isRenaming || !newName.trim() || newName.trim() === branch.name
                ? 'bg-blue-400 cursor-not-allowed'
                : 'bg-blue-600 hover:bg-blue-700',
            )}
          >
            {isRenaming ? t('branchActions.renaming') : t('branchActions.rename')}
          </button>
        </div>
      </div>
    </DialogShell>
  );
}

// ---------------------------------------------------------------------------
// BranchActions (exported entry point)
// ---------------------------------------------------------------------------

export function BranchActions({ type, branch, branches, repoPath, onClose, onSuccess }: BranchActionsProps) {
  switch (type) {
    case 'create':
      return <CreateBranchDialog branches={branches} repoPath={repoPath} onClose={onClose} onSuccess={onSuccess} />;
    case 'delete':
      return branch ? (
        <DeleteBranchDialog branch={branch} repoPath={repoPath} onClose={onClose} onSuccess={onSuccess} />
      ) : null;
    case 'rename':
      return branch ? (
        <RenameBranchDialog branch={branch} repoPath={repoPath} onClose={onClose} onSuccess={onSuccess} />
      ) : null;
    default:
      return null;
  }
}
