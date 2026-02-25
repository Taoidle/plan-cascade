/**
 * InstallFromUrlDialog Component
 *
 * Dialog for installing a plugin from a git URL.
 * Validates URL format and shows progress during installation.
 */

import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import * as Dialog from '@radix-ui/react-dialog';
import { Cross2Icon, ReloadIcon, DownloadIcon } from '@radix-ui/react-icons';
import { usePluginStore } from '../../store/plugins';

export function InstallFromUrlDialog() {
  const { t } = useTranslation('settings');

  const installDialogOpen = usePluginStore((s) => s.installDialogOpen);
  const closeInstallDialog = usePluginStore((s) => s.closeInstallDialog);
  const installFromGit = usePluginStore((s) => s.installFromGit);
  const installing = usePluginStore((s) => s.installing);
  const installProgress = usePluginStore((s) => s.installProgress);

  const [gitUrl, setGitUrl] = useState('');
  const [urlError, setUrlError] = useState<string | null>(null);

  const validateUrl = useCallback(
    (url: string): boolean => {
      if (!url.trim()) {
        setUrlError(null);
        return false;
      }
      const isValid = url.startsWith('https://') || url.startsWith('http://') || url.startsWith('git@');
      if (!isValid) {
        setUrlError(t('plugins.invalidGitUrl'));
      } else {
        setUrlError(null);
      }
      return isValid;
    },
    [t],
  );

  const handleInstall = useCallback(async () => {
    if (!validateUrl(gitUrl)) return;
    await installFromGit(gitUrl);
    setGitUrl('');
  }, [gitUrl, validateUrl, installFromGit]);

  const handleClose = useCallback(() => {
    if (!installing) {
      closeInstallDialog();
      setGitUrl('');
      setUrlError(null);
    }
  }, [installing, closeInstallDialog]);

  return (
    <Dialog.Root open={installDialogOpen} onOpenChange={(open) => !open && handleClose()}>
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
              {t('plugins.installFromUrl')}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button
                disabled={installing}
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
            <p className="text-xs text-gray-500 dark:text-gray-400">{t('plugins.installFromUrlDesc')}</p>

            {/* URL input */}
            <div>
              <input
                type="text"
                value={gitUrl}
                onChange={(e) => {
                  setGitUrl(e.target.value);
                  if (urlError) validateUrl(e.target.value);
                }}
                placeholder={t('plugins.gitUrlPlaceholder')}
                disabled={installing}
                className={clsx(
                  'w-full px-3 py-2 rounded-md text-sm',
                  'bg-gray-50 dark:bg-gray-800',
                  'border',
                  urlError ? 'border-red-300 dark:border-red-700' : 'border-gray-200 dark:border-gray-700',
                  'text-gray-700 dark:text-gray-300',
                  'placeholder:text-gray-400 dark:placeholder:text-gray-500',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                  'disabled:opacity-50',
                )}
              />
              {urlError && <p className="mt-1 text-2xs text-red-500">{urlError}</p>}
            </div>

            {/* Progress */}
            {installing && installProgress && (
              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <ReloadIcon className="w-3.5 h-3.5 animate-spin text-primary-600" />
                  <span className="text-xs text-gray-600 dark:text-gray-300">{installProgress.message}</span>
                </div>
                <div className="w-full h-1.5 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                  <div
                    className="h-full rounded-full bg-primary-600 transition-all duration-300"
                    style={{ width: `${installProgress.progress * 100}%` }}
                  />
                </div>
              </div>
            )}

            {installing && !installProgress && (
              <div className="flex items-center gap-2">
                <ReloadIcon className="w-3.5 h-3.5 animate-spin text-primary-600" />
                <span className="text-xs text-gray-600 dark:text-gray-300">{t('plugins.installing')}</span>
              </div>
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-gray-200 dark:border-gray-700">
            <button
              onClick={handleClose}
              disabled={installing}
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
              onClick={handleInstall}
              disabled={installing || !gitUrl.trim()}
              className={clsx(
                'inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium',
                'bg-primary-600 hover:bg-primary-700 text-white',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors',
              )}
            >
              {installing ? (
                <ReloadIcon className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <DownloadIcon className="w-3.5 h-3.5" />
              )}
              {t('plugins.install')}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default InstallFromUrlDialog;
