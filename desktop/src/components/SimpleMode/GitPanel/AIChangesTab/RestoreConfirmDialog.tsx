/**
 * RestoreConfirmDialog Component
 *
 * Modal dialog confirming file restoration before a turn rollback.
 * Lists files that will be restored or deleted.
 */

import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { RestoredFile } from '../../../../types/fileChanges';

interface RestoreConfirmDialogProps {
  turnIndex: number;
  expectedFiles: { path: string; willDelete: boolean }[];
  onConfirm: () => void;
  onCancel: () => void;
  restoring: boolean;
  result: RestoredFile[] | null;
}

export function RestoreConfirmDialog({
  turnIndex,
  expectedFiles,
  onConfirm,
  onCancel,
  restoring,
  result,
}: RestoreConfirmDialogProps) {
  const { t } = useTranslation('git');
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div
        className={clsx(
          'w-full max-w-md rounded-lg border border-gray-200 dark:border-gray-700',
          'bg-white dark:bg-gray-900 shadow-xl p-5'
        )}
      >
        {result ? (
          <>
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100 mb-3">
              {t('aiChanges.restoreComplete')}
            </h3>
            <ul className="space-y-1 mb-4 max-h-48 overflow-y-auto">
              {result.map((r) => (
                <li
                  key={r.path}
                  className="flex items-center gap-2 text-xs text-gray-600 dark:text-gray-400"
                >
                  <span
                    className={clsx(
                      'inline-block w-14 text-center rounded px-1 py-0.5 text-2xs font-medium',
                      r.action === 'deleted'
                        ? 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'
                        : 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
                    )}
                  >
                    {r.action === 'deleted' ? t('aiChanges.actionDeleted') : t('aiChanges.actionRestored')}
                  </span>
                  <span className="font-mono truncate">{r.path}</span>
                </li>
              ))}
            </ul>
            <div className="flex justify-end">
              <button
                onClick={onCancel}
                className="px-3 py-1.5 text-sm font-medium rounded bg-primary-600 text-white hover:bg-primary-700"
              >
                {t('aiChanges.done')}
              </button>
            </div>
          </>
        ) : (
          <>
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100 mb-2">
              {t('aiChanges.restoreConfirm', { index: turnIndex })}
            </h3>
            <p className="text-xs text-gray-500 dark:text-gray-400 mb-3">
              {t('aiChanges.affectedFiles')}
            </p>
            <ul className="space-y-1 mb-4 max-h-48 overflow-y-auto">
              {expectedFiles.map((f) => (
                <li
                  key={f.path}
                  className="flex items-center gap-2 text-xs text-gray-600 dark:text-gray-400"
                >
                  <span
                    className={clsx(
                      'inline-block w-14 text-center rounded px-1 py-0.5 text-2xs font-medium',
                      f.willDelete
                        ? 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'
                        : 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300'
                    )}
                  >
                    {f.willDelete ? t('aiChanges.actionDelete') : t('aiChanges.actionRestore')}
                  </span>
                  <span className="font-mono truncate">{f.path}</span>
                </li>
              ))}
            </ul>
            <div className="flex justify-end gap-2">
              <button
                onClick={onCancel}
                disabled={restoring}
                className="px-3 py-1.5 text-sm font-medium rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50"
              >
                {t('aiChanges.cancel')}
              </button>
              <button
                onClick={onConfirm}
                disabled={restoring}
                className="px-3 py-1.5 text-sm font-medium rounded bg-orange-600 text-white hover:bg-orange-700 disabled:opacity-50"
              >
                {restoring ? t('aiChanges.restoring') : t('aiChanges.restore')}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
