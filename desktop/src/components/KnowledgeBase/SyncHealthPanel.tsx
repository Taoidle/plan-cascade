/**
 * SyncHealthPanel Component
 *
 * Operational panel for docs watcher/index sync and collection update health.
 */

import { useCallback, useEffect, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { KnowledgeCollection } from '../../lib/knowledgeApi';
import { useSettingsStore } from '../../store/settings';
import { useKnowledgeStore } from '../../store/knowledge';

interface SyncHealthPanelProps {
  projectId: string;
  collection: KnowledgeCollection;
}

function statusBadgeClass(status: string): string {
  switch (status) {
    case 'indexed':
      return 'bg-green-50 text-green-700 dark:bg-green-900/30 dark:text-green-300';
    case 'changes_pending':
      return 'bg-amber-50 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300';
    case 'queued':
      return 'bg-indigo-50 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-300';
    case 'indexing':
      return 'bg-blue-50 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300';
    case 'retry_waiting':
      return 'bg-orange-50 text-orange-700 dark:bg-orange-900/30 dark:text-orange-300';
    case 'error':
      return 'bg-red-50 text-red-700 dark:bg-red-900/30 dark:text-red-300';
    default:
      return 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300';
  }
}

function formatStatusLabel(status: string, t: (key: string, options?: Record<string, unknown>) => string): string {
  if (status === 'indexed') return t('syncHealth.statusIndexed', { defaultValue: 'Indexed' });
  if (status === 'changes_pending') return t('syncHealth.statusChangesPending', { defaultValue: 'Changes Pending' });
  if (status === 'queued') return t('syncHealth.statusQueued', { defaultValue: 'Queued' });
  if (status === 'indexing') return t('syncHealth.statusIndexing', { defaultValue: 'Indexing' });
  if (status === 'retry_waiting') return t('syncHealth.statusRetryWaiting', { defaultValue: 'Retry Waiting' });
  if (status === 'error') return t('syncHealth.statusError', { defaultValue: 'Error' });
  if (status === 'none') return t('syncHealth.statusNone', { defaultValue: 'Not Initialized' });
  return status;
}

export function SyncHealthPanel({ projectId, collection }: SyncHealthPanelProps) {
  const { t } = useTranslation('knowledge');
  const workspacePath = useSettingsStore((s) => s.workspacePath);

  const {
    docsStatus,
    isLoadingDocsStatus,
    isSyncingDocs,
    pendingUpdates,
    isCheckingUpdates,
    isApplyingUpdates,
    fetchDocsStatus,
    ensureDocsCollection,
    syncDocsCollection,
    rebuildDocsCollection,
    checkForUpdates,
    applyUpdates,
  } = useKnowledgeStore();

  const effectiveWorkspacePath = useMemo(() => {
    if (collection.workspace_path && collection.workspace_path.trim()) {
      return collection.workspace_path;
    }
    return workspacePath;
  }, [collection.workspace_path, workspacePath]);

  useEffect(() => {
    if (!effectiveWorkspacePath) return;
    fetchDocsStatus(effectiveWorkspacePath, projectId);
  }, [effectiveWorkspacePath, projectId, fetchDocsStatus]);

  const handleRefreshStatus = useCallback(async () => {
    if (!effectiveWorkspacePath) return;
    await fetchDocsStatus(effectiveWorkspacePath, projectId);
  }, [effectiveWorkspacePath, projectId, fetchDocsStatus]);

  const handleEnsure = useCallback(async () => {
    if (!effectiveWorkspacePath) return;
    await ensureDocsCollection(effectiveWorkspacePath, projectId);
  }, [effectiveWorkspacePath, projectId, ensureDocsCollection]);

  const handleSync = useCallback(async () => {
    if (!effectiveWorkspacePath) return;
    await syncDocsCollection(effectiveWorkspacePath, projectId);
  }, [effectiveWorkspacePath, projectId, syncDocsCollection]);

  const handleRebuild = useCallback(async () => {
    if (!effectiveWorkspacePath) return;
    await rebuildDocsCollection(effectiveWorkspacePath, projectId, 'safe_swap');
  }, [effectiveWorkspacePath, projectId, rebuildDocsCollection]);

  const hasChanges =
    pendingUpdates &&
    pendingUpdates.collection_id === collection.id &&
    (pendingUpdates.modified.length > 0 || pendingUpdates.deleted.length > 0 || pendingUpdates.new_files.length > 0);

  return (
    <div className="p-6 space-y-6">
      <div>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
          {t('syncHealth.title', { defaultValue: 'Sync & Health' })}
        </h3>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
          {t('syncHealth.subtitle', {
            defaultValue: 'Monitor documentation indexing and workspace-to-knowledge sync health.',
          })}
        </p>
      </div>

      <div className="rounded-lg border border-gray-200 dark:border-gray-700 p-4 space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h4 className="text-sm font-semibold text-gray-900 dark:text-white">
              {t('syncHealth.docsStatus', { defaultValue: 'Docs Collection Status' })}
            </h4>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1 break-all">
              {effectiveWorkspacePath ||
                t('syncHealth.workspaceMissing', { defaultValue: 'No workspace path configured.' })}
            </p>
          </div>
          <span
            className={clsx(
              'text-xs font-medium px-2 py-1 rounded-full',
              statusBadgeClass(docsStatus?.status ?? 'none'),
            )}
          >
            {formatStatusLabel(docsStatus?.status ?? 'none', t)}
          </span>
        </div>

        <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-sm">
          <div className="rounded bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 p-3">
            <div className="text-xs text-gray-500 dark:text-gray-400">
              {t('syncHealth.totalDocs', { defaultValue: 'Total Docs' })}
            </div>
            <div className="mt-1 font-semibold text-gray-900 dark:text-white">{docsStatus?.total_docs ?? 0}</div>
          </div>
          <div className="rounded bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 p-3">
            <div className="text-xs text-gray-500 dark:text-gray-400">
              {t('syncHealth.pendingChanges', { defaultValue: 'Pending Changes' })}
            </div>
            <div className="mt-1 font-semibold text-gray-900 dark:text-white">
              {docsStatus?.pending_changes.length ?? 0}
            </div>
          </div>
          <div className="rounded bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 p-3 col-span-2">
            <div className="text-xs text-gray-500 dark:text-gray-400">
              {t('syncHealth.collectionName', { defaultValue: 'Docs Collection' })}
            </div>
            <div className="mt-1 font-semibold text-gray-900 dark:text-white truncate">
              {docsStatus?.collection_name ?? t('syncHealth.collectionMissing', { defaultValue: 'Not created yet' })}
            </div>
          </div>
        </div>

        {docsStatus?.last_error && (
          <div className="rounded border border-red-200 bg-red-50 p-3 text-xs text-red-700 dark:border-red-900/40 dark:bg-red-950/30 dark:text-red-300">
            <div className="font-semibold mb-1">
              {t('syncHealth.lastError', { defaultValue: 'Last Error' })}
              {docsStatus.last_error_code ? ` (${docsStatus.last_error_code})` : ''}
            </div>
            <div className="break-all">{docsStatus.last_error}</div>
            {docsStatus.next_retry_at && (
              <div className="mt-1">
                {t('syncHealth.nextRetryAt', { defaultValue: 'Next Retry' })}: {docsStatus.next_retry_at}
              </div>
            )}
          </div>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <button
            onClick={handleRefreshStatus}
            disabled={!effectiveWorkspacePath || isLoadingDocsStatus || isSyncingDocs}
            className={clsx(
              'text-sm px-3 py-1.5 rounded-md border',
              'border-gray-300 dark:border-gray-600',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-50 dark:hover:bg-gray-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {isLoadingDocsStatus
              ? t('syncHealth.refreshing', { defaultValue: 'Refreshing...' })
              : t('syncHealth.refresh', { defaultValue: 'Refresh Status' })}
          </button>
          <button
            onClick={handleEnsure}
            disabled={!effectiveWorkspacePath || isSyncingDocs}
            className={clsx(
              'text-sm px-3 py-1.5 rounded-md border',
              'border-gray-300 dark:border-gray-600',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-50 dark:hover:bg-gray-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {isSyncingDocs
              ? t('syncHealth.ensuring', { defaultValue: 'Ensuring...' })
              : t('syncHealth.ensureCollection', { defaultValue: 'Ensure Docs Collection' })}
          </button>
          <button
            onClick={handleSync}
            disabled={!effectiveWorkspacePath || isSyncingDocs}
            className={clsx(
              'text-sm px-3 py-1.5 rounded-md',
              'bg-primary-600 hover:bg-primary-700 text-white',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {isSyncingDocs
              ? t('syncHealth.syncing', { defaultValue: 'Syncing...' })
              : t('syncHealth.syncNow', { defaultValue: 'Sync Docs Now' })}
          </button>
          <button
            onClick={handleRebuild}
            disabled={!effectiveWorkspacePath || isSyncingDocs}
            className={clsx(
              'text-sm px-3 py-1.5 rounded-md',
              'bg-red-600 hover:bg-red-700 text-white',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {isSyncingDocs
              ? t('syncHealth.rebuilding', { defaultValue: 'Rebuilding...' })
              : t('syncHealth.rebuildDocs', { defaultValue: 'Rebuild Docs Index' })}
          </button>
        </div>

        {docsStatus && docsStatus.pending_changes.length > 0 && (
          <div>
            <p className="text-xs font-medium text-gray-600 dark:text-gray-400 mb-2">
              {t('syncHealth.pendingFiles', { defaultValue: 'Pending File Changes' })}
            </p>
            <div className="max-h-40 overflow-y-auto rounded border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900">
              {docsStatus.pending_changes.slice(0, 100).map((path, index) => (
                <div
                  key={`${path}-${index}`}
                  className="px-3 py-1.5 text-xs text-gray-700 dark:text-gray-300 border-b border-gray-100 dark:border-gray-800 last:border-b-0"
                >
                  {path}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      <div className="rounded-lg border border-gray-200 dark:border-gray-700 p-4 space-y-4">
        <div className="flex items-center justify-between gap-3">
          <h4 className="text-sm font-semibold text-gray-900 dark:text-white">
            {t('syncHealth.collectionDiff', { defaultValue: 'Collection Sync Diff' })}
          </h4>
          <div className="flex items-center gap-2">
            <button
              onClick={() => checkForUpdates(collection.id)}
              disabled={isCheckingUpdates || isApplyingUpdates}
              className={clsx(
                'text-xs px-3 py-1.5 rounded-md border',
                'border-gray-300 dark:border-gray-600',
                'text-gray-700 dark:text-gray-300',
                'hover:bg-gray-50 dark:hover:bg-gray-800',
                'disabled:opacity-50 disabled:cursor-not-allowed',
              )}
            >
              {isCheckingUpdates ? t('updates.checking') : t('syncHealth.checkDiff', { defaultValue: 'Check Diff' })}
            </button>
            <button
              onClick={() => applyUpdates(collection.id)}
              disabled={!hasChanges || isApplyingUpdates}
              className={clsx(
                'text-xs px-3 py-1.5 rounded-md',
                'bg-green-600 hover:bg-green-700 text-white',
                'disabled:opacity-50 disabled:cursor-not-allowed',
              )}
            >
              {isApplyingUpdates ? t('updates.applying') : t('syncHealth.applyDiff', { defaultValue: 'Apply Diff' })}
            </button>
          </div>
        </div>

        {pendingUpdates && pendingUpdates.collection_id === collection.id ? (
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div className="rounded bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 p-3">
              <div className="text-xs text-gray-500 dark:text-gray-400">
                {t('syncHealth.modified', { defaultValue: 'Modified' })}
              </div>
              <div className="mt-1 font-semibold text-gray-900 dark:text-white">{pendingUpdates.modified.length}</div>
            </div>
            <div className="rounded bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 p-3">
              <div className="text-xs text-gray-500 dark:text-gray-400">
                {t('syncHealth.deleted', { defaultValue: 'Deleted' })}
              </div>
              <div className="mt-1 font-semibold text-gray-900 dark:text-white">{pendingUpdates.deleted.length}</div>
            </div>
            <div className="rounded bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 p-3">
              <div className="text-xs text-gray-500 dark:text-gray-400">
                {t('syncHealth.newFiles', { defaultValue: 'New Files' })}
              </div>
              <div className="mt-1 font-semibold text-gray-900 dark:text-white">{pendingUpdates.new_files.length}</div>
            </div>
            <div className="rounded bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 p-3">
              <div className="text-xs text-gray-500 dark:text-gray-400">
                {t('syncHealth.unchanged', { defaultValue: 'Unchanged' })}
              </div>
              <div className="mt-1 font-semibold text-gray-900 dark:text-white">{pendingUpdates.unchanged}</div>
            </div>
          </div>
        ) : (
          <p className="text-sm text-gray-500 dark:text-gray-400">
            {t('syncHealth.noDiffData', { defaultValue: 'Run "Check Diff" to inspect workspace drift.' })}
          </p>
        )}
      </div>
    </div>
  );
}

export default SyncHealthPanel;
