/**
 * KnowledgeBasePanel Component
 *
 * Main container for the Project Knowledge Base feature with two-panel layout:
 * - Left: Collection list with create/delete actions
 * - Right: Collection details, document upload, and query interface
 */

import { useState, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useKnowledgeStore } from '../../store/knowledge';
import { useProjectsStore } from '../../store/projects';
import { CollectionDetail } from './CollectionDetail';
import { DocumentUploader } from './DocumentUploader';
import { KnowledgeQuery } from './KnowledgeQuery';

// ---------------------------------------------------------------------------
// CreateCollectionDialog
// ---------------------------------------------------------------------------

interface CreateCollectionDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (name: string, description: string) => void;
  isLoading: boolean;
}

function CreateCollectionDialog({ isOpen, onClose, onSubmit, isLoading }: CreateCollectionDialogProps) {
  const { t } = useTranslation('knowledge');
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className={clsx(
        'w-full max-w-md rounded-xl p-6',
        'bg-white dark:bg-gray-800',
        'border border-gray-200 dark:border-gray-700',
        'shadow-xl'
      )}>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
          {t('createCollection')}
        </h3>
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('collectionName')}
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('collectionNamePlaceholder')}
              className={clsx(
                'w-full px-3 py-2 rounded-lg',
                'border border-gray-300 dark:border-gray-600',
                'bg-white dark:bg-gray-700',
                'text-gray-900 dark:text-white',
                'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                'text-sm'
              )}
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('description')}
            </label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t('descriptionPlaceholder')}
              rows={3}
              className={clsx(
                'w-full px-3 py-2 rounded-lg',
                'border border-gray-300 dark:border-gray-600',
                'bg-white dark:bg-gray-700',
                'text-gray-900 dark:text-white',
                'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                'text-sm resize-none'
              )}
            />
          </div>
        </div>
        <div className="flex justify-end gap-3 mt-6">
          <button
            onClick={onClose}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-700',
              'transition-colors'
            )}
          >
            {t('cancel', { ns: 'common' })}
          </button>
          <button
            onClick={() => {
              if (name.trim()) {
                onSubmit(name.trim(), description.trim());
              }
            }}
            disabled={!name.trim() || isLoading}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'bg-primary-600 hover:bg-primary-700',
              'text-white',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors'
            )}
          >
            {isLoading ? t('creating') : t('create')}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// DeleteConfirmDialog
// ---------------------------------------------------------------------------

interface DeleteConfirmDialogProps {
  isOpen: boolean;
  collectionName: string;
  onClose: () => void;
  onConfirm: () => void;
  isLoading: boolean;
}

function DeleteConfirmDialog({ isOpen, collectionName, onClose, onConfirm, isLoading }: DeleteConfirmDialogProps) {
  const { t } = useTranslation('knowledge');

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className={clsx(
        'w-full max-w-sm rounded-xl p-6',
        'bg-white dark:bg-gray-800',
        'border border-gray-200 dark:border-gray-700',
        'shadow-xl'
      )}>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-2">
          {t('deleteCollection')}
        </h3>
        <p className="text-sm text-gray-600 dark:text-gray-400 mb-6">
          {t('deleteConfirm', { name: collectionName })}
        </p>
        <div className="flex justify-end gap-3">
          <button
            onClick={onClose}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-700',
              'transition-colors'
            )}
          >
            {t('cancel', { ns: 'common' })}
          </button>
          <button
            onClick={onConfirm}
            disabled={isLoading}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'bg-red-600 hover:bg-red-700',
              'text-white',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors'
            )}
          >
            {isLoading ? t('deleting') : t('delete')}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// KnowledgeBasePanel
// ---------------------------------------------------------------------------

/** Active tab in the right panel. */
type RightTab = 'details' | 'upload' | 'query';

export function KnowledgeBasePanel() {
  const { t } = useTranslation('knowledge');
  const {
    collections,
    activeCollection,
    isLoading,
    isIngesting,
    isDeleting,
    error,
    fetchCollections,
    selectCollection,
    createCollection,
    deleteCollection,
    clearError,
  } = useKnowledgeStore();

  const { selectedProject } = useProjectsStore();
  const projectId = selectedProject?.id ?? 'default';

  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<RightTab>('details');

  // Fetch collections on mount and project change
  useEffect(() => {
    fetchCollections(projectId);
  }, [projectId, fetchCollections]);

  const handleCreate = useCallback(async (name: string, description: string) => {
    const ok = await createCollection(projectId, name, description, []);
    if (ok) {
      setShowCreateDialog(false);
      fetchCollections(projectId);
    }
  }, [projectId, createCollection, fetchCollections]);

  const handleDelete = useCallback(async () => {
    if (!deleteTarget) return;
    const ok = await deleteCollection(projectId, deleteTarget);
    if (ok) {
      setDeleteTarget(null);
    }
  }, [projectId, deleteTarget, deleteCollection]);

  return (
    <div className="h-full flex flex-col">
      {/* Error banner */}
      {error && (
        <div className={clsx(
          'px-4 py-2 flex items-center justify-between',
          'bg-red-50 dark:bg-red-900/20',
          'border-b border-red-200 dark:border-red-800'
        )}>
          <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
          <button onClick={clearError} className="text-sm text-red-600 hover:text-red-800 dark:text-red-400">
            {t('dismiss')}
          </button>
        </div>
      )}

      <div className="flex-1 flex overflow-hidden">
        {/* Left Panel - Collection List */}
        <div className={clsx(
          'h-full border-r border-gray-200 dark:border-gray-700',
          'bg-gray-50 dark:bg-gray-900',
          activeCollection ? 'hidden md:block md:w-1/3 lg:w-1/4' : 'w-full md:w-1/3 lg:w-1/4'
        )}>
          <div className="h-full flex flex-col">
            {/* Header */}
            <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700">
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-semibold text-gray-900 dark:text-white">
                  {t('title')}
                </h2>
                <button
                  onClick={() => setShowCreateDialog(true)}
                  className={clsx(
                    'px-3 py-1.5 rounded-lg text-xs font-medium',
                    'bg-primary-600 hover:bg-primary-700',
                    'text-white',
                    'transition-colors'
                  )}
                >
                  {t('newCollection')}
                </button>
              </div>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                {t('subtitle', { count: collections.length })}
              </p>
            </div>

            {/* Collection list */}
            <div className="flex-1 overflow-y-auto">
              {isLoading ? (
                <div className="px-4 py-8 text-center">
                  <div className="animate-pulse text-sm text-gray-500">{t('loading')}</div>
                </div>
              ) : collections.length === 0 ? (
                <div className="px-4 py-8 text-center">
                  <p className="text-sm text-gray-500 dark:text-gray-400">
                    {t('noCollections')}
                  </p>
                  <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
                    {t('noCollectionsHint')}
                  </p>
                </div>
              ) : (
                <div className="divide-y divide-gray-200 dark:divide-gray-700">
                  {collections.map((collection) => (
                    <button
                      key={collection.id}
                      onClick={() => {
                        selectCollection(collection);
                        setActiveTab('details');
                      }}
                      className={clsx(
                        'w-full text-left px-4 py-3',
                        'hover:bg-gray-100 dark:hover:bg-gray-800',
                        'transition-colors',
                        activeCollection?.id === collection.id &&
                          'bg-primary-50 dark:bg-primary-900/20 border-l-2 border-primary-500'
                      )}
                    >
                      <div className="flex items-center justify-between">
                        <span className="text-sm font-medium text-gray-900 dark:text-white truncate">
                          {collection.name}
                        </span>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setDeleteTarget(collection.name);
                          }}
                          className="text-gray-400 hover:text-red-500 dark:hover:text-red-400 p-1"
                          title={t('delete')}
                        >
                          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                          </svg>
                        </button>
                      </div>
                      <div className="flex items-center gap-3 mt-1">
                        <span className="text-xs text-gray-500 dark:text-gray-400">
                          {t('chunks', { count: collection.chunk_count })}
                        </span>
                        {collection.description && (
                          <span className="text-xs text-gray-400 dark:text-gray-500 truncate">
                            {collection.description}
                          </span>
                        )}
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Right Panel - Detail / Upload / Query */}
        <div className={clsx(
          'h-full flex-1',
          'bg-white dark:bg-gray-950',
          activeCollection ? 'w-full md:w-2/3 lg:w-3/4' : 'hidden md:flex md:items-center md:justify-center'
        )}>
          {activeCollection ? (
            <div className="h-full flex flex-col">
              {/* Tab bar */}
              <div className="flex items-center border-b border-gray-200 dark:border-gray-700 px-4">
                {/* Back button (mobile) */}
                <button
                  onClick={() => selectCollection(null)}
                  className="md:hidden mr-3 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                >
                  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
                  </svg>
                </button>

                {(['details', 'upload', 'query'] as RightTab[]).map((tab) => (
                  <button
                    key={tab}
                    onClick={() => setActiveTab(tab)}
                    className={clsx(
                      'px-4 py-3 text-sm font-medium border-b-2 transition-colors',
                      activeTab === tab
                        ? 'border-primary-500 text-primary-600 dark:text-primary-400'
                        : 'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
                    )}
                  >
                    {t(`tabs.${tab}`)}
                  </button>
                ))}
              </div>

              {/* Tab content */}
              <div className="flex-1 overflow-y-auto">
                {activeTab === 'details' && (
                  <CollectionDetail collection={activeCollection} />
                )}
                {activeTab === 'upload' && (
                  <DocumentUploader
                    projectId={projectId}
                    collectionName={activeCollection.name}
                  />
                )}
                {activeTab === 'query' && (
                  <KnowledgeQuery
                    projectId={projectId}
                    collectionName={activeCollection.name}
                  />
                )}
              </div>
            </div>
          ) : (
            <div className="text-center px-4">
              <svg className="mx-auto w-12 h-12 text-gray-300 dark:text-gray-600 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
              </svg>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                {t('selectCollection')}
              </p>
            </div>
          )}
        </div>
      </div>

      {/* Dialogs */}
      <CreateCollectionDialog
        isOpen={showCreateDialog}
        onClose={() => setShowCreateDialog(false)}
        onSubmit={handleCreate}
        isLoading={isIngesting}
      />
      <DeleteConfirmDialog
        isOpen={deleteTarget !== null}
        collectionName={deleteTarget ?? ''}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        isLoading={isDeleting}
      />
    </div>
  );
}
