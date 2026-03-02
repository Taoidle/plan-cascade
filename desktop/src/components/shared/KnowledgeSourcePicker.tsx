/**
 * KnowledgeSourcePicker
 *
 * Popover content showing a tree of Knowledge Collections and Documents.
 * Supports collection-level and document-level selection with indeterminate state.
 * Documents are lazy-loaded when a collection is expanded.
 * Includes search input for client-side filtering.
 */

import { useState, useCallback, useMemo, useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, ChevronRightIcon, MagnifyingGlassIcon } from '@radix-ui/react-icons';
import { useContextSourcesStore } from '../../store/contextSources';
import { useProjectsStore } from '../../store/projects';
import { useSettingsStore } from '../../store/settings';
import { ragRecordPickerSearch, ragSearchDocuments, type DocumentSearchMatch } from '../../lib/knowledgeApi';

export function KnowledgeSourcePicker() {
  const { t } = useTranslation('simpleMode');
  const {
    availableCollections,
    collectionDocuments,
    selectedCollections,
    selectedDocuments,
    isLoadingCollections,
    isLoadingDocuments,
    toggleCollection,
    toggleDocument,
    loadDocuments,
  } = useContextSourcesStore();

  const [expandedCollections, setExpandedCollections] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState('');
  const [remoteMatches, setRemoteMatches] = useState<DocumentSearchMatch[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const projectId = useProjectsStore((s) => s.selectedProject?.id ?? 'default');
  const serverSearchEnabled = useSettingsStore((s) => s.kbPickerServerSearch);

  const toggleExpand = useCallback(
    (collectionId: string) => {
      setExpandedCollections((prev) => {
        const next = new Set(prev);
        if (next.has(collectionId)) {
          next.delete(collectionId);
        } else {
          next.add(collectionId);
          // Lazy-load documents if not yet loaded
          if (!collectionDocuments[collectionId]) {
            loadDocuments(collectionId);
          }
        }
        return next;
      });
    },
    [collectionDocuments, loadDocuments],
  );

  /** Determine checkbox state for a collection: checked, unchecked, or indeterminate. */
  const getCollectionCheckState = (collectionId: string): 'checked' | 'unchecked' | 'indeterminate' => {
    const isCollectionSelected = selectedCollections.includes(collectionId);
    const docs = collectionDocuments[collectionId] || [];

    if (docs.length === 0) {
      return isCollectionSelected ? 'checked' : 'unchecked';
    }

    const selectedCount = docs.filter((doc) =>
      selectedDocuments.some((ref) => ref.collection_id === collectionId && ref.document_uid === doc.document_uid),
    ).length;

    if (selectedCount === 0 && !isCollectionSelected) return 'unchecked';
    if (selectedCount === docs.length) return 'checked';
    return 'indeterminate';
  };

  // Client-side search filtering
  const lowerQuery = searchQuery.toLowerCase().trim();
  useEffect(() => {
    if (!lowerQuery) {
      setRemoteMatches([]);
      setIsSearching(false);
      return;
    }
    if (!serverSearchEnabled) {
      setRemoteMatches([]);
      setIsSearching(false);
      return;
    }
    let cancelled = false;
    const timeout = setTimeout(async () => {
      setIsSearching(true);
      try {
        const result = await ragSearchDocuments(projectId, lowerQuery, undefined, 80);
        if (cancelled) return;
        const matches = result.success && result.data ? result.data : [];
        setRemoteMatches(matches);
        void ragRecordPickerSearch(matches.length === 0);
      } finally {
        if (!cancelled) {
          setIsSearching(false);
        }
      }
    }, 240);
    return () => {
      cancelled = true;
      clearTimeout(timeout);
    };
  }, [projectId, lowerQuery, serverSearchEnabled]);

  const remoteByCollection = useMemo(() => {
    const grouped = new Map<string, DocumentSearchMatch[]>();
    for (const item of remoteMatches) {
      const bucket = grouped.get(item.collection_id);
      if (bucket) {
        bucket.push(item);
      } else {
        grouped.set(item.collection_id, [item]);
      }
    }
    return grouped;
  }, [remoteMatches]);

  const filteredCollections = useMemo(() => {
    if (!lowerQuery) return availableCollections;
    return availableCollections.filter((col) => {
      // Match collection name
      if (col.name.toLowerCase().includes(lowerQuery)) return true;
      if (remoteByCollection.has(col.id)) return true;
      // Match any document in the collection
      const docs = collectionDocuments[col.id] || [];
      return docs.some((d) => (d.display_name || d.document_id || '').toLowerCase().includes(lowerQuery));
    });
  }, [availableCollections, collectionDocuments, lowerQuery, remoteByCollection]);

  if (isLoadingCollections) {
    return (
      <div className="p-3 text-xs text-gray-500 dark:text-gray-400">
        {t('contextSources.knowledgePicker.loading', { defaultValue: 'Loading...' })}
      </div>
    );
  }

  if (availableCollections.length === 0) {
    return (
      <div className="p-3 text-xs text-gray-500 dark:text-gray-400">
        {t('contextSources.knowledgePicker.noCollections', {
          defaultValue: 'No knowledge collections available',
        })}
      </div>
    );
  }

  return (
    <div className="py-1">
      <div className="px-3 py-1.5 text-xs font-semibold text-gray-600 dark:text-gray-300 border-b border-gray-100 dark:border-gray-700">
        {t('contextSources.knowledgePicker.title', { defaultValue: 'Knowledge Sources' })}
      </div>

      {/* Search input */}
      <div className="px-2 py-1.5 border-b border-gray-100 dark:border-gray-700">
        <div className="relative">
          <MagnifyingGlassIcon className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-gray-400" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t('contextSources.knowledgePicker.searchPlaceholder', {
              defaultValue: 'Search collections...',
            })}
            className={clsx(
              'w-full pl-6 pr-2 py-1 text-2xs rounded',
              'bg-gray-50 dark:bg-gray-750 border border-gray-200 dark:border-gray-600',
              'text-gray-700 dark:text-gray-300 placeholder-gray-400',
              'focus:outline-none focus:ring-1 focus:ring-amber-400',
            )}
          />
        </div>
      </div>

      <div className="max-h-64 overflow-y-auto">
        {lowerQuery && isSearching && (
          <div className="px-3 py-2 text-2xs text-gray-400">
            {t('contextSources.knowledgePicker.loading', { defaultValue: 'Loading...' })}
          </div>
        )}
        {filteredCollections.length === 0 && lowerQuery && (
          <div className="px-3 py-2 text-2xs text-gray-400">
            {t('contextSources.knowledgePicker.noResults', { defaultValue: 'No matching collections' })}
          </div>
        )}
        {filteredCollections.map((collection) => {
          const isExpanded = expandedCollections.has(collection.id) || !!lowerQuery;
          const checkState = getCollectionCheckState(collection.id);
          const allDocs = collectionDocuments[collection.id] || [];
          const remoteDocs = (remoteByCollection.get(collection.id) || []).map((match) => ({
            document_uid: match.document_uid,
            display_name: match.display_name,
            document_id: match.display_name,
            chunk_count: 0,
          }));
          const loading = isLoadingDocuments[collection.id];

          // Filter documents when searching
          const docs = lowerQuery
            ? remoteDocs.length > 0
              ? remoteDocs
              : allDocs.filter(
                  (d) =>
                    (d.display_name || d.document_id || '').toLowerCase().includes(lowerQuery) ||
                    collection.name.toLowerCase().includes(lowerQuery),
                )
            : allDocs;

          return (
            <div key={collection.id}>
              {/* Collection row */}
              <div
                className={clsx(
                  'flex items-center gap-1.5 px-2 py-1.5',
                  'hover:bg-gray-50 dark:hover:bg-gray-750',
                  'cursor-pointer select-none',
                )}
              >
                <button
                  onClick={() => toggleExpand(collection.id)}
                  className="p-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                >
                  {isExpanded ? <ChevronDownIcon className="w-3 h-3" /> : <ChevronRightIcon className="w-3 h-3" />}
                </button>

                <input
                  type="checkbox"
                  checked={checkState === 'checked'}
                  ref={(el) => {
                    if (el) el.indeterminate = checkState === 'indeterminate';
                  }}
                  onChange={() => toggleCollection(collection.id)}
                  className="w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-amber-600 focus:ring-amber-500"
                />

                <span
                  className="flex-1 text-xs text-gray-700 dark:text-gray-300 truncate cursor-pointer"
                  onClick={() => toggleExpand(collection.id)}
                >
                  {collection.name}
                </span>

                <span className="text-2xs text-gray-400 dark:text-gray-500">
                  {collection.chunk_count} {t('contextSources.knowledgePicker.chunks', { defaultValue: 'chunks' })}
                </span>
              </div>

              {/* Document rows (expanded) */}
              {isExpanded && (
                <div className="ml-5 border-l border-gray-100 dark:border-gray-700">
                  {loading && (
                    <div className="px-3 py-1 text-2xs text-gray-400">
                      {t('contextSources.knowledgePicker.loading', { defaultValue: 'Loading...' })}
                    </div>
                  )}
                  {!loading && docs.length === 0 && (
                    <div className="px-3 py-1 text-2xs text-gray-400">
                      {t('knowledge.documents.noDocuments', { defaultValue: 'No documents' })}
                    </div>
                  )}
                  {docs.map((doc) => (
                    <div
                      key={doc.document_uid}
                      className={clsx(
                        'flex items-center gap-1.5 px-2 py-1',
                        'hover:bg-gray-50 dark:hover:bg-gray-750',
                        'cursor-pointer select-none',
                      )}
                    >
                      <input
                        type="checkbox"
                        checked={selectedDocuments.some(
                          (ref) => ref.collection_id === collection.id && ref.document_uid === doc.document_uid,
                        )}
                        onChange={() => {
                          if (!collectionDocuments[collection.id]) {
                            void loadDocuments(collection.id);
                          }
                          toggleDocument(collection.id, doc.document_uid);
                        }}
                        className="w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-amber-600 focus:ring-amber-500"
                      />
                      <span className="flex-1 text-2xs text-gray-600 dark:text-gray-400 truncate">
                        {doc.display_name}
                      </span>
                      <span className="text-2xs text-gray-400 dark:text-gray-500">{doc.chunk_count}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
