/**
 * RestoreDialog Component
 *
 * Confirmation modal for restoring to a checkpoint.
 * Shows warning message, affected files, and backup option.
 */

import { useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import * as Dialog from '@radix-ui/react-dialog';
import * as Checkbox from '@radix-ui/react-checkbox';
import { Cross2Icon, CheckIcon, ExclamationTriangleIcon, FileIcon } from '@radix-ui/react-icons';
import type { Checkpoint, CheckpointDiff } from '../../types/timeline';

interface RestoreDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  checkpoint: Checkpoint;
  diff?: CheckpointDiff | null;
  onConfirm: (createBackup: boolean) => void;
  isRestoring?: boolean;
}

export function RestoreDialog({
  open,
  onOpenChange,
  checkpoint,
  diff,
  onConfirm,
  isRestoring = false,
}: RestoreDialogProps) {
  const { t } = useTranslation();
  const [createBackup, setCreateBackup] = useState(true);

  const handleConfirm = useCallback(() => {
    onConfirm(createBackup);
  }, [createBackup, onConfirm]);

  // Get files that will change
  const changedFiles = diff
    ? [
        ...diff.added_files.map((f) => ({ ...f, action: 'remove' as const })),
        ...diff.modified_files.map((f) => ({ ...f, action: 'revert' as const })),
        ...diff.deleted_files.map((f) => ({ ...f, action: 'restore' as const })),
      ]
    : [];

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay
          className={clsx(
            'fixed inset-0 bg-black/50',
            'data-[state=open]:animate-in data-[state=closed]:animate-out',
            'data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0'
          )}
        />
        <Dialog.Content
          className={clsx(
            'fixed left-[50%] top-[50%] translate-x-[-50%] translate-y-[-50%]',
            'w-full max-w-lg max-h-[85vh]',
            'bg-white dark:bg-gray-900',
            'rounded-lg shadow-xl',
            'border border-gray-200 dark:border-gray-700',
            'data-[state=open]:animate-in data-[state=closed]:animate-out',
            'data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
            'data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95',
            'data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%]',
            'data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%]',
            'duration-200'
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
              <ExclamationTriangleIcon className="w-5 h-5 text-orange-500" />
              {t('timeline.restoreConfirm')}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button
                className={clsx(
                  'p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800',
                  'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                  'transition-colors'
                )}
              >
                <Cross2Icon className="w-5 h-5" />
              </button>
            </Dialog.Close>
          </div>

          {/* Content */}
          <div className="p-4 space-y-4 overflow-y-auto max-h-[50vh]">
            {/* Checkpoint info */}
            <div
              className={clsx(
                'p-3 rounded-lg',
                'bg-gray-50 dark:bg-gray-800',
                'border border-gray-200 dark:border-gray-700'
              )}
            >
              <p className="font-medium text-gray-900 dark:text-white">
                {checkpoint.label}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                {new Date(checkpoint.timestamp).toLocaleString()}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                {checkpoint.files_snapshot.length} {t('timeline.files')}
              </p>
            </div>

            {/* Warning message */}
            <div
              className={clsx(
                'p-3 rounded-lg',
                'bg-orange-50 dark:bg-orange-900/20',
                'border border-orange-200 dark:border-orange-800'
              )}
            >
              <p className="text-sm text-orange-800 dark:text-orange-200">
                {t('timeline.restoreWarning')}
              </p>
            </div>

            {/* Files that will change */}
            {changedFiles.length > 0 && (
              <div>
                <p className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  {t('timeline.filesWillChange')}
                </p>
                <ul className="space-y-1 max-h-32 overflow-y-auto">
                  {changedFiles.slice(0, 10).map((file) => (
                    <li
                      key={file.path}
                      className="flex items-center gap-2 text-sm"
                    >
                      <FileIcon className="w-3 h-3 text-gray-400" />
                      <span
                        className={clsx(
                          'truncate',
                          file.action === 'remove' && 'text-red-600 dark:text-red-400',
                          file.action === 'restore' && 'text-green-600 dark:text-green-400',
                          file.action === 'revert' && 'text-yellow-600 dark:text-yellow-400'
                        )}
                      >
                        {file.path}
                      </span>
                    </li>
                  ))}
                  {changedFiles.length > 10 && (
                    <li className="text-sm text-gray-400">
                      +{changedFiles.length - 10} {t('timeline.moreFiles')}
                    </li>
                  )}
                </ul>
              </div>
            )}

            {/* Backup checkbox */}
            <div className="flex items-center gap-2">
              <Checkbox.Root
                id="backup"
                checked={createBackup}
                onCheckedChange={(checked) => setCreateBackup(checked === true)}
                className={clsx(
                  'w-5 h-5 rounded border-2 flex items-center justify-center',
                  'border-gray-300 dark:border-gray-600',
                  'data-[state=checked]:bg-primary-600 data-[state=checked]:border-primary-600',
                  'transition-colors'
                )}
              >
                <Checkbox.Indicator>
                  <CheckIcon className="w-3 h-3 text-white" />
                </Checkbox.Indicator>
              </Checkbox.Root>
              <label
                htmlFor="backup"
                className="text-sm text-gray-700 dark:text-gray-300 cursor-pointer"
              >
                {t('timeline.createBackup')}
              </label>
            </div>
          </div>

          {/* Footer */}
          <div className="flex items-center justify-end gap-2 p-4 border-t border-gray-200 dark:border-gray-700">
            <Dialog.Close asChild>
              <button
                disabled={isRestoring}
                className={clsx(
                  'px-4 py-2 rounded-lg text-sm font-medium',
                  'bg-gray-100 dark:bg-gray-800',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                  'transition-colors'
                )}
              >
                {t('timeline.cancelRestore')}
              </button>
            </Dialog.Close>
            <button
              onClick={handleConfirm}
              disabled={isRestoring}
              className={clsx(
                'px-4 py-2 rounded-lg text-sm font-medium',
                'bg-orange-600 text-white',
                'hover:bg-orange-700',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors flex items-center gap-2'
              )}
            >
              {isRestoring ? (
                <>
                  <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  {t('status.inProgress')}
                </>
              ) : (
                t('timeline.confirmRestore')
              )}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default RestoreDialog;
