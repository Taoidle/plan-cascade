import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { KnowledgeCollection } from '../../../lib/knowledgeApi';

interface CollectionListPaneProps {
  collections: KnowledgeCollection[];
  activeCollection: KnowledgeCollection | null;
  isLoadingCollections: boolean;
  onCreate: () => void;
  onSelect: (collection: KnowledgeCollection) => void;
  onEdit: (collection: KnowledgeCollection) => void;
  onDelete: (collectionName: string) => void;
}

export function CollectionListPane({
  collections,
  activeCollection,
  isLoadingCollections,
  onCreate,
  onSelect,
  onEdit,
  onDelete,
}: CollectionListPaneProps) {
  const { t } = useTranslation('knowledge');

  return (
    <div className="h-full flex flex-col">
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-gray-900 dark:text-white">{t('title')}</h2>
          <button
            onClick={onCreate}
            className={clsx(
              'px-3 py-1.5 rounded-lg text-xs font-medium',
              'bg-primary-600 hover:bg-primary-700',
              'text-white',
              'transition-colors',
            )}
          >
            {t('newCollection')}
          </button>
        </div>
        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{t('subtitle', { count: collections.length })}</p>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoadingCollections ? (
          <div className="px-4 py-8 text-center">
            <div className="animate-pulse text-sm text-gray-500">{t('loading')}</div>
          </div>
        ) : collections.length === 0 ? (
          <div className="px-4 py-8 text-center">
            <p className="text-sm text-gray-500 dark:text-gray-400">{t('noCollections')}</p>
            <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">{t('noCollectionsHint')}</p>
          </div>
        ) : (
          <div className="divide-y divide-gray-200 dark:divide-gray-700">
            {collections.map((collection) => (
              <div
                key={collection.id}
                role="button"
                tabIndex={0}
                onClick={() => onSelect(collection)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    onSelect(collection);
                  }
                }}
                className={clsx(
                  'w-full text-left px-4 py-3 cursor-pointer',
                  'hover:bg-gray-100 dark:hover:bg-gray-800',
                  'transition-colors',
                  activeCollection?.id === collection.id &&
                    'bg-primary-50 dark:bg-primary-900/20 border-l-2 border-primary-500',
                )}
              >
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium text-gray-900 dark:text-white truncate">{collection.name}</span>
                  <div className="flex items-center gap-1">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        onEdit(collection);
                      }}
                      className="text-gray-400 hover:text-primary-500 dark:hover:text-primary-400 p-1"
                      title={t('editCollection')}
                    >
                      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
                        />
                      </svg>
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        onDelete(collection.name);
                      }}
                      className="text-gray-400 hover:text-red-500 dark:hover:text-red-400 p-1"
                      title={t('delete')}
                    >
                      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                        />
                      </svg>
                    </button>
                  </div>
                </div>
                <div className="flex items-center gap-3 mt-1">
                  <span className="text-xs text-gray-500 dark:text-gray-400">
                    {t('chunks', { count: collection.chunk_count })}
                  </span>
                  {collection.description && (
                    <span className="text-xs text-gray-400 dark:text-gray-500 truncate">{collection.description}</span>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
