import * as Dialog from '@radix-ui/react-dialog';
import { clsx } from 'clsx';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useUpdateStore } from '../../store/update';

function formatPublishedAt(timestamp: number | null, locale: string): string | null {
  if (!timestamp) return null;
  try {
    return new Intl.DateTimeFormat(locale, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(timestamp * 1000));
  } catch {
    return null;
  }
}

function formatProgressLabel(downloadedBytes: number, totalBytes: number | null): string {
  if (!totalBytes || totalBytes <= 0) {
    return `${(downloadedBytes / 1024 / 1024).toFixed(1)} MB`;
  }
  const percent = Math.min(100, Math.round((downloadedBytes / totalBytes) * 100));
  return `${percent}%`;
}

export function AppUpdateDialog() {
  const { t, i18n } = useTranslation('updates');
  const { currentVersion, activeInfo, dialogOpen, closeDialog, installState, progress, error } = useUpdateStore();
  const downloadAndInstallAvailableUpdate = useUpdateStore((s) => s.downloadAndInstallAvailableUpdate);
  const restartToApplyUpdate = useUpdateStore((s) => s.restartToApplyUpdate);
  const skipCurrentVersion = useUpdateStore((s) => s.skipCurrentVersion);

  const publishedAt = useMemo(
    () => formatPublishedAt(activeInfo?.published_at ?? null, i18n.resolvedLanguage || 'en'),
    [activeInfo?.published_at, i18n.resolvedLanguage],
  );

  const channelKey = activeInfo?.channel ? `channel.${activeInfo.channel}` : null;
  const progressPercent =
    progress?.total_bytes && progress.total_bytes > 0
      ? Math.min(100, (progress.downloaded_bytes / progress.total_bytes) * 100)
      : 0;

  return (
    <Dialog.Root open={dialogOpen} onOpenChange={(open) => !open && closeDialog()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[70] bg-black/50 backdrop-blur-sm" />
        <Dialog.Content
          className={clsx(
            'fixed left-1/2 top-1/2 z-[71] w-[min(560px,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2',
            'rounded-2xl border border-gray-200 bg-white p-6 shadow-2xl dark:border-gray-800 dark:bg-gray-900',
            'focus:outline-none',
          )}
          aria-describedby={undefined}
        >
          <Dialog.Title className="text-xl font-semibold text-gray-900 dark:text-white">
            {installState === 'restart_required' ? t('dialog.readyTitle') : t('dialog.title')}
          </Dialog.Title>

          <div className="mt-4 space-y-4 text-sm text-gray-600 dark:text-gray-300">
            <div className="flex items-center gap-2">
              <span className="font-medium text-gray-900 dark:text-white">{t('dialog.currentVersion')}</span>
              <span>{currentVersion ?? activeInfo?.current_version ?? '0.0.0'}</span>
              {channelKey && (
                <span className="rounded-full bg-primary-100 px-2 py-0.5 text-xs font-medium text-primary-700 dark:bg-primary-900/40 dark:text-primary-300">
                  {t(channelKey)}
                </span>
              )}
            </div>

            {activeInfo?.target_version && (
              <div>
                <div className="font-medium text-gray-900 dark:text-white">{t('dialog.newVersion')}</div>
                <div>{activeInfo.target_version}</div>
              </div>
            )}

            {publishedAt && (
              <div>
                <div className="font-medium text-gray-900 dark:text-white">{t('dialog.publishedAt')}</div>
                <div>{publishedAt}</div>
              </div>
            )}

            {activeInfo?.notes && (
              <div>
                <div className="font-medium text-gray-900 dark:text-white">{t('dialog.releaseNotes')}</div>
                <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap rounded-xl bg-gray-50 p-3 text-xs leading-6 text-gray-700 dark:bg-gray-950 dark:text-gray-300">
                  {activeInfo.notes}
                </pre>
              </div>
            )}

            {installState === 'downloading' && (
              <div className="space-y-2">
                <div className="font-medium text-gray-900 dark:text-white">
                  {progress?.stage === 'verifying' ? t('dialog.verifying') : t('dialog.downloading')}
                </div>
                <div className="h-2 overflow-hidden rounded-full bg-gray-200 dark:bg-gray-800">
                  <div
                    className="h-full rounded-full bg-primary-500 transition-[width] duration-300"
                    style={{ width: `${progressPercent}%` }}
                  />
                </div>
                <div className="text-xs text-gray-500 dark:text-gray-400">
                  {progress
                    ? formatProgressLabel(progress.downloaded_bytes, progress.total_bytes)
                    : t('dialog.preparing')}
                </div>
              </div>
            )}

            {error && (
              <div className="rounded-xl border border-red-200 bg-red-50 px-3 py-2 text-red-700 dark:border-red-900/40 dark:bg-red-950/30 dark:text-red-300">
                {error}
              </div>
            )}
          </div>

          <div className="mt-6 flex flex-wrap items-center justify-end gap-3">
            {installState !== 'restart_required' && (
              <>
                <button
                  type="button"
                  onClick={closeDialog}
                  className="rounded-lg bg-gray-100 px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
                >
                  {t('actions.later')}
                </button>
                {activeInfo?.target_version && (
                  <button
                    type="button"
                    onClick={skipCurrentVersion}
                    className="rounded-lg bg-gray-100 px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
                  >
                    {t('actions.skipVersion')}
                  </button>
                )}
                <button
                  type="button"
                  disabled={installState === 'downloading'}
                  onClick={() => void downloadAndInstallAvailableUpdate()}
                  className="rounded-lg bg-primary-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-primary-700 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {installState === 'downloading' ? t('actions.installing') : t('actions.downloadAndInstall')}
                </button>
              </>
            )}

            {installState === 'restart_required' && (
              <>
                <button
                  type="button"
                  onClick={closeDialog}
                  className="rounded-lg bg-gray-100 px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
                >
                  {t('actions.later')}
                </button>
                <button
                  type="button"
                  onClick={() => void restartToApplyUpdate()}
                  className="rounded-lg bg-primary-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-primary-700"
                >
                  {t('actions.restartToUpdate')}
                </button>
              </>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
