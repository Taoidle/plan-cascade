import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import * as Dialog from '@radix-ui/react-dialog';
import type { McpServer } from '../../types/mcp';

interface McpExportDialogProps {
  open: boolean;
  includeSecrets: boolean;
  onOpenChange: (open: boolean) => void;
  onIncludeSecretsChange: (value: boolean) => void;
  onConfirm: () => void;
}

export function McpExportDialog({
  open,
  includeSecrets,
  onOpenChange,
  onIncludeSecretsChange,
  onConfirm,
}: McpExportDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 animate-in fade-in-0 z-40" />
        <Dialog.Content
          className={clsx(
            'fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-[94vw] max-w-md',
            'bg-white dark:bg-gray-900 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700',
          )}
        >
          <div className="p-4 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-white">
              {t('mcp.export')}
            </Dialog.Title>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
              {t('mcp.exportRedactedDefault', {
                defaultValue: 'Sensitive fields are redacted by default.',
              })}
            </p>
          </div>

          <div className="p-4 space-y-3">
            <label className="flex items-start gap-2 text-sm text-gray-700 dark:text-gray-300">
              <input
                type="checkbox"
                checked={includeSecrets}
                onChange={(e) => onIncludeSecretsChange(e.target.checked)}
                className="mt-0.5"
              />
              <span>
                {t('mcp.exportIncludeSecrets', {
                  defaultValue: 'Include plaintext secrets in export file',
                })}
              </span>
            </label>
            {includeSecrets && (
              <p className="text-xs rounded border border-amber-200 bg-amber-50 dark:border-amber-900/40 dark:bg-amber-900/20 text-amber-700 dark:text-amber-300 px-2 py-1.5">
                {t('mcp.exportIncludeSecretsWarn', {
                  defaultValue: 'This export will contain plaintext credentials. Handle file securely.',
                })}
              </p>
            )}
          </div>

          <div className="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="px-3 py-2 rounded-md text-sm bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300"
            >
              {t('common.cancel')}
            </button>
            <button
              type="button"
              onClick={onConfirm}
              className="px-3 py-2 rounded-md text-sm text-white bg-primary-600 hover:bg-primary-700"
            >
              {t('mcp.export')}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

interface McpConfirmSensitiveExportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}

export function McpConfirmSensitiveExportDialog({
  open,
  onOpenChange,
  onConfirm,
}: McpConfirmSensitiveExportDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 animate-in fade-in-0 z-40" />
        <Dialog.Content
          className={clsx(
            'fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-[94vw] max-w-sm',
            'bg-white dark:bg-gray-900 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700',
          )}
        >
          <div className="p-4">
            <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-white">
              {t('mcp.exportConfirmIncludeTitle', {
                defaultValue: 'Confirm plaintext secret export',
              })}
            </Dialog.Title>
            <p className="text-sm text-gray-600 dark:text-gray-300 mt-2">
              {t('mcp.exportConfirmIncludeBody', {
                defaultValue: 'The exported file will include plaintext secrets. Continue?',
              })}
            </p>
          </div>
          <div className="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="px-3 py-2 rounded-md text-sm bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300"
            >
              {t('common.cancel')}
            </button>
            <button
              type="button"
              onClick={onConfirm}
              className="px-3 py-2 rounded-md text-sm text-white bg-red-600 hover:bg-red-700"
            >
              {t('common.done')}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

interface McpDeleteConfirmDialogProps {
  server: McpServer | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}

export function McpDeleteConfirmDialog({ server, onOpenChange, onConfirm }: McpDeleteConfirmDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog.Root
      open={!!server}
      onOpenChange={(open) => {
        onOpenChange(open);
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 animate-in fade-in-0 z-40" />
        <Dialog.Content
          className={clsx(
            'fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-[94vw] max-w-sm',
            'bg-white dark:bg-gray-900 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700',
          )}
        >
          <div className="p-4">
            <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-white">
              {t('mcp.confirmDeleteTitle', { defaultValue: 'Delete MCP server?' })}
            </Dialog.Title>
            <p className="text-sm text-gray-600 dark:text-gray-300 mt-2">{t('mcp.confirmDeleteWithDisconnect')}</p>
            {server?.name && <p className="text-xs text-gray-500 dark:text-gray-400 mt-2">{server.name}</p>}
          </div>
          <div className="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="px-3 py-2 rounded-md text-sm bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300"
            >
              {t('common.cancel')}
            </button>
            <button
              type="button"
              onClick={onConfirm}
              className="px-3 py-2 rounded-md text-sm text-white bg-red-600 hover:bg-red-700"
            >
              {t('mcp.delete', { defaultValue: 'Delete' })}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
