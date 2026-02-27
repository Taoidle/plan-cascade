/**
 * CollectionDetail Component
 *
 * Displays metadata for a selected knowledge collection including
 * chunk count, description, timestamps, and a document list with
 * per-document delete capability and batch update checking.
 */

import { useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { KnowledgeCollection } from '../../lib/knowledgeApi';
import { useKnowledgeStore } from '../../store/knowledge';

interface CollectionDetailProps {
  collection: KnowledgeCollection;
}

export function CollectionDetail({ collection }: CollectionDetailProps) {
  const { t } = useTranslation('knowledge');
  const {
    documents,
    fetchDocuments,
    deleteDocument,
    isDeleting,
    pendingUpdates,
    isCheckingUpdates,
    isApplyingUpdates,
    checkForUpdates,
    applyUpdates,
  } = useKnowledgeStore();

  useEffect(() => {
    fetchDocuments(collection.id);
  }, [collection.id, fetchDocuments]);

  const formatDate = (dateStr: string) => {
    try {
      return new Date(dateStr).toLocaleString();
    } catch {
      return dateStr;
    }
  };

  const handleDeleteDocument = async (documentId: string) => {
    const ok = await deleteDocument(collection.id, documentId);
    if (ok) {
      // Refresh document list
      fetchDocuments(collection.id);
    }
  };

  const hasChanges =
    pendingUpdates &&
    pendingUpdates.collection_id === collection.id &&
    (pendingUpdates.modified.length > 0 || pendingUpdates.deleted.length > 0 || pendingUpdates.new_files.length > 0);

  const isCurrentCheck = pendingUpdates && pendingUpdates.collection_id === collection.id;

  return (
    <div className="p-6 space-y-6">
      {/* Collection Name + Update Actions */}
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="text-xl font-semibold text-gray-900 dark:text-white">{collection.name}</h3>
          {collection.description && (
            <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">{collection.description}</p>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <button
            onClick={() => checkForUpdates(collection.id)}
            disabled={isCheckingUpdates || isApplyingUpdates}
            className={clsx(
              'text-xs px-3 py-1.5 rounded-md font-medium transition-colors',
              'bg-blue-50 text-blue-700 hover:bg-blue-100',
              'dark:bg-blue-900/30 dark:text-blue-300 dark:hover:bg-blue-900/50',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {isCheckingUpdates ? t('updates.checking') : t('updates.checkForUpdates')}
          </button>
          {hasChanges && (
            <button
              onClick={() => applyUpdates(collection.id)}
              disabled={isApplyingUpdates}
              className={clsx(
                'text-xs px-3 py-1.5 rounded-md font-medium transition-colors',
                'bg-green-50 text-green-700 hover:bg-green-100',
                'dark:bg-green-900/30 dark:text-green-300 dark:hover:bg-green-900/50',
                'disabled:opacity-50 disabled:cursor-not-allowed',
              )}
            >
              {isApplyingUpdates ? t('updates.applying') : t('updates.applyUpdates')}
            </button>
          )}
        </div>
      </div>

      {/* Update Summary Panel */}
      {isCurrentCheck && pendingUpdates && (
        <div
          className={clsx(
            'rounded-lg p-4',
            'border',
            hasChanges
              ? 'bg-amber-50 dark:bg-amber-900/20 border-amber-200 dark:border-amber-700'
              : 'bg-green-50 dark:bg-green-900/20 border-green-200 dark:border-green-700',
          )}
        >
          {hasChanges ? (
            <div className="space-y-2">
              <h4 className="text-sm font-medium text-amber-800 dark:text-amber-200">{t('updates.updatesFound')}</h4>
              <div className="text-xs space-y-1 text-amber-700 dark:text-amber-300">
                {pendingUpdates.modified.length > 0 && (
                  <div>
                    {t('updates.modifiedDocs', { count: pendingUpdates.modified.length })}
                    <ul className="ml-4 list-disc">
                      {pendingUpdates.modified.map((d) => (
                        <li key={d.document_id} className="truncate">
                          {d.source_path || d.document_id}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
                {pendingUpdates.deleted.length > 0 && (
                  <div>
                    {t('updates.deletedDocs', { count: pendingUpdates.deleted.length })}
                    <ul className="ml-4 list-disc">
                      {pendingUpdates.deleted.map((d) => (
                        <li key={d.document_id} className="truncate">
                          {d.source_path || d.document_id}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
                {pendingUpdates.new_files.length > 0 && (
                  <div>{t('updates.newDocs', { count: pendingUpdates.new_files.length })}</div>
                )}
                <div className="text-gray-500 dark:text-gray-400">
                  {t('updates.unchangedDocs', { count: pendingUpdates.unchanged })}
                </div>
              </div>
            </div>
          ) : (
            <p className="text-sm text-green-700 dark:text-green-300">
              {pendingUpdates.unchanged > 0 ? t('updates.noUpdatesFound') : t('updates.noTrackableDocs')}
            </p>
          )}
        </div>
      )}

      {/* Stats Grid */}
      <div className="grid grid-cols-2 lg:grid-cols-3 gap-4">
        <StatCard label={t('detail.chunkCount')} value={String(collection.chunk_count)} />
        <StatCard label={t('detail.projectId')} value={collection.project_id} />
        <StatCard label={t('detail.collectionId')} value={collection.id} truncate />
        <StatCard
          label={t('detail.workspacePath')}
          value={collection.workspace_path || t('detail.noWorkspacePath')}
          truncate
        />
      </div>

      {/* Document List */}
      {documents.length > 0 && (
        <div>
          <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
            {t('documents.title', { count: documents.length })}
          </h4>
          <div className="divide-y divide-gray-200 dark:divide-gray-700 rounded-lg border border-gray-200 dark:border-gray-700">
            {documents.map((doc) => (
              <div key={doc.document_id} className="flex items-center justify-between px-4 py-3">
                <div className="min-w-0 flex-1 mr-3">
                  <p className="text-sm font-medium text-gray-900 dark:text-white truncate">{doc.document_id}</p>
                  <div className="flex items-center gap-3 mt-1">
                    <span className="text-xs text-gray-500 dark:text-gray-400">
                      {t('documents.chunks', { count: doc.chunk_count })}
                    </span>
                    {doc.preview && (
                      <span className="text-xs text-gray-400 dark:text-gray-500 truncate">{doc.preview}</span>
                    )}
                  </div>
                </div>
                <button
                  onClick={() => handleDeleteDocument(doc.document_id)}
                  disabled={isDeleting}
                  className={clsx(
                    'text-xs px-2.5 py-1 rounded',
                    'text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-900/20',
                    'disabled:opacity-50 disabled:cursor-not-allowed',
                    'transition-colors',
                  )}
                >
                  {isDeleting ? t('deleting') : t('delete')}
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Timestamps */}
      <div
        className={clsx('rounded-lg p-4', 'bg-gray-50 dark:bg-gray-900', 'border border-gray-200 dark:border-gray-700')}
      >
        <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">{t('detail.timestamps')}</h4>
        <div className="space-y-2">
          <div className="flex justify-between text-sm">
            <span className="text-gray-500 dark:text-gray-400">{t('detail.createdAt')}</span>
            <span className="text-gray-900 dark:text-white font-mono text-xs">{formatDate(collection.created_at)}</span>
          </div>
          <div className="flex justify-between text-sm">
            <span className="text-gray-500 dark:text-gray-400">{t('detail.updatedAt')}</span>
            <span className="text-gray-900 dark:text-white font-mono text-xs">{formatDate(collection.updated_at)}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// StatCard
// ---------------------------------------------------------------------------

interface StatCardProps {
  label: string;
  value: string;
  truncate?: boolean;
}

function StatCard({ label, value, truncate }: StatCardProps) {
  return (
    <div
      className={clsx('rounded-lg p-4', 'bg-gray-50 dark:bg-gray-900', 'border border-gray-200 dark:border-gray-700')}
    >
      <div className="text-xs text-gray-500 dark:text-gray-400 mb-1">{label}</div>
      <div className={clsx('text-lg font-semibold text-gray-900 dark:text-white', truncate && 'truncate text-sm')}>
        {value}
      </div>
    </div>
  );
}
