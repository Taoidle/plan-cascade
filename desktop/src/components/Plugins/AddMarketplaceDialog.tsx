/**
 * AddMarketplaceDialog Component
 *
 * Dialog for adding a new marketplace source.
 * Supports GitHub shorthand (owner/repo), git URLs, and local paths.
 */

import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import * as Dialog from '@radix-ui/react-dialog';
import { Cross2Icon, ReloadIcon, PlusIcon } from '@radix-ui/react-icons';
import { usePluginStore } from '../../store/plugins';

export function AddMarketplaceDialog() {
  const { t } = useTranslation('settings');

  const addMarketplaceDialogOpen = usePluginStore((s) => s.addMarketplaceDialogOpen);
  const closeAddMarketplaceDialog = usePluginStore((s) => s.closeAddMarketplaceDialog);
  const addMarketplace = usePluginStore((s) => s.addMarketplace);
  const addingMarketplace = usePluginStore((s) => s.addingMarketplace);

  const [source, setSource] = useState('');

  const handleAdd = useCallback(async () => {
    if (!source.trim()) return;
    await addMarketplace(source.trim());
    setSource('');
  }, [source, addMarketplace]);

  const handleClose = useCallback(() => {
    if (!addingMarketplace) {
      closeAddMarketplaceDialog();
      setSource('');
    }
  }, [addingMarketplace, closeAddMarketplaceDialog]);

  return (
    <Dialog.Root open={addMarketplaceDialogOpen} onOpenChange={(open) => !open && handleClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 z-[60] animate-[fadeIn_0.15s]" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-[60]',
            'w-[480px] max-w-[90vw]',
            'bg-white dark:bg-gray-900 rounded-xl shadow-2xl',
            'border border-gray-200 dark:border-gray-700',
            'animate-[contentShow_0.2s]',
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-sm font-semibold text-gray-900 dark:text-white">
              {t('plugins.addMarketplace')}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button
                disabled={addingMarketplace}
                className={clsx(
                  'p-1.5 rounded-md',
                  'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                  'hover:bg-gray-100 dark:hover:bg-gray-800',
                  'disabled:opacity-50',
                )}
              >
                <Cross2Icon className="w-4 h-4" />
              </button>
            </Dialog.Close>
          </div>

          {/* Body */}
          <div className="p-4 space-y-4">
            <p className="text-xs text-gray-500 dark:text-gray-400">{t('plugins.addMarketplaceDesc')}</p>

            {/* Source input */}
            <div>
              <input
                type="text"
                value={source}
                onChange={(e) => setSource(e.target.value)}
                placeholder={t('plugins.addMarketplacePlaceholder')}
                disabled={addingMarketplace}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && source.trim()) {
                    handleAdd();
                  }
                }}
                className={clsx(
                  'w-full px-3 py-2 rounded-md text-sm',
                  'bg-gray-50 dark:bg-gray-800',
                  'border border-gray-200 dark:border-gray-700',
                  'text-gray-700 dark:text-gray-300',
                  'placeholder:text-gray-400 dark:placeholder:text-gray-500',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                  'disabled:opacity-50',
                )}
              />
            </div>

            {/* Progress */}
            {addingMarketplace && (
              <div className="flex items-center gap-2">
                <ReloadIcon className="w-3.5 h-3.5 animate-spin text-primary-600" />
                <span className="text-xs text-gray-600 dark:text-gray-300">{t('plugins.fetchingMarketplace')}</span>
              </div>
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-gray-200 dark:border-gray-700">
            <button
              onClick={handleClose}
              disabled={addingMarketplace}
              className={clsx(
                'px-3 py-1.5 rounded-md text-xs font-medium',
                'bg-gray-100 dark:bg-gray-800',
                'text-gray-700 dark:text-gray-300',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'disabled:opacity-50',
                'transition-colors',
              )}
            >
              {t('plugins.cancel')}
            </button>
            <button
              onClick={handleAdd}
              disabled={addingMarketplace || !source.trim()}
              className={clsx(
                'inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium',
                'bg-primary-600 hover:bg-primary-700 text-white',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors',
              )}
            >
              {addingMarketplace ? (
                <ReloadIcon className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <PlusIcon className="w-3.5 h-3.5" />
              )}
              {t('plugins.addMarketplace')}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default AddMarketplaceDialog;
