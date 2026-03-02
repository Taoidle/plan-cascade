/**
 * KnowledgeBasePanel Component
 *
 * Main container for the Project Knowledge Base feature with two-panel layout:
 * - Left: Collection list with create/delete actions
 * - Right: Operations workspace (collections/documents/retrieval/sync-health)
 */

import { useState, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { KnowledgeCollection } from '../../lib/knowledgeApi';
import { useKnowledgeStore } from '../../store/knowledge';
import { useProjectsStore } from '../../store/projects';
import { CollectionDetail } from './CollectionDetail';
import { DocumentUploader } from './DocumentUploader';
import { RetrievalLab } from './RetrievalLab';
import { SyncHealthPanel } from './SyncHealthPanel';
import { CollectionListPane } from './panel/CollectionListPane';
import { CreateCollectionDialog } from './panel/CreateCollectionDialog';
import { EditCollectionDialog } from './panel/EditCollectionDialog';
import { DeleteConfirmDialog } from './panel/DeleteConfirmDialog';
import { RightPanelTabs, type RightTab } from './panel/RightPanelTabs';

export function KnowledgeBasePanel() {
  const { t } = useTranslation('knowledge');
  const {
    collections,
    activeCollection,
    isLoadingCollections,
    isUpdatingCollection,
    isIngesting,
    isDeletingCollection,
    error,
    fetchCollections,
    selectCollection,
    createCollection,
    deleteCollection,
    updateCollection,
    clearError,
  } = useKnowledgeStore();

  const { selectedProject } = useProjectsStore();
  const projectId = selectedProject?.id ?? 'default';

  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [editTarget, setEditTarget] = useState<KnowledgeCollection | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<RightTab>('collections');

  useEffect(() => {
    fetchCollections(projectId);
  }, [projectId, fetchCollections]);

  const handleCreate = useCallback(
    async (name: string, description: string, workspacePath?: string) => {
      const ok = await createCollection(projectId, name, description, []);
      if (ok) {
        if (workspacePath) {
          const created = useKnowledgeStore.getState().collections.find((c) => c.name === name);
          if (created) {
            await updateCollection(created.id, undefined, undefined, workspacePath);
          }
        }
        setShowCreateDialog(false);
        fetchCollections(projectId);
      }
    },
    [projectId, createCollection, updateCollection, fetchCollections],
  );

  const handleEdit = useCallback(
    async (collectionId: string, name: string, description: string, workspacePath?: string | null) => {
      const ok = await updateCollection(collectionId, name, description, workspacePath);
      if (ok) {
        setEditTarget(null);
        fetchCollections(projectId);
      }
    },
    [updateCollection, fetchCollections, projectId],
  );

  const handleDelete = useCallback(async () => {
    if (!deleteTarget) return;
    const ok = await deleteCollection(projectId, deleteTarget);
    if (ok) {
      setDeleteTarget(null);
    }
  }, [projectId, deleteTarget, deleteCollection]);

  return (
    <div className="h-full flex flex-col">
      {error && (
        <div
          className={clsx(
            'px-4 py-2 flex items-center justify-between',
            'bg-red-50 dark:bg-red-900/20',
            'border-b border-red-200 dark:border-red-800',
          )}
        >
          <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
          <button onClick={clearError} className="text-sm text-red-600 hover:text-red-800 dark:text-red-400">
            {t('dismiss')}
          </button>
        </div>
      )}

      <div className="flex-1 flex overflow-hidden">
        <div
          className={clsx(
            'h-full border-r border-gray-200 dark:border-gray-700',
            'bg-gray-50 dark:bg-gray-900',
            activeCollection ? 'hidden md:block md:w-1/3 lg:w-1/4' : 'w-full md:w-1/3 lg:w-1/4',
          )}
        >
          <CollectionListPane
            collections={collections}
            activeCollection={activeCollection}
            isLoadingCollections={isLoadingCollections}
            onCreate={() => setShowCreateDialog(true)}
            onSelect={(collection) => {
              selectCollection(collection);
              setActiveTab('collections');
            }}
            onEdit={setEditTarget}
            onDelete={setDeleteTarget}
          />
        </div>

        <div
          className={clsx(
            'h-full flex-1',
            'bg-white dark:bg-gray-950',
            activeCollection ? 'w-full md:w-2/3 lg:w-3/4' : 'hidden md:flex md:items-center md:justify-center',
          )}
        >
          {activeCollection ? (
            <div className="h-full flex flex-col">
              <RightPanelTabs activeTab={activeTab} onChange={setActiveTab} onBack={() => selectCollection(null)} />

              <div className="flex-1 overflow-y-auto">
                {activeTab === 'collections' && <CollectionDetail collection={activeCollection} />}
                {activeTab === 'documents' && (
                  <DocumentUploader projectId={projectId} collectionId={activeCollection.id} />
                )}
                {activeTab === 'retrieval' && <RetrievalLab projectId={projectId} collectionId={activeCollection.id} />}
                {activeTab === 'health' && <SyncHealthPanel projectId={projectId} collection={activeCollection} />}
              </div>
            </div>
          ) : (
            <div className="text-center px-4">
              <svg
                className="mx-auto w-12 h-12 text-gray-300 dark:text-gray-600 mb-3"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={1.5}
                  d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
                />
              </svg>
              <p className="text-sm text-gray-500 dark:text-gray-400">{t('selectCollection')}</p>
            </div>
          )}
        </div>
      </div>

      <CreateCollectionDialog
        isOpen={showCreateDialog}
        onClose={() => setShowCreateDialog(false)}
        onSubmit={handleCreate}
        isLoading={isIngesting}
      />
      <EditCollectionDialog
        isOpen={editTarget !== null}
        collection={editTarget}
        onClose={() => setEditTarget(null)}
        onSubmit={handleEdit}
        isLoading={isUpdatingCollection}
      />
      <DeleteConfirmDialog
        isOpen={deleteTarget !== null}
        collectionName={deleteTarget ?? ''}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        isLoading={isDeletingCollection}
      />
    </div>
  );
}
