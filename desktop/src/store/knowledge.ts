/**
 * Knowledge Base Store
 *
 * Zustand store for project knowledge base management. Manages
 * collections, document upload progress, and query results via
 * Tauri IPC commands.
 */

import { create } from 'zustand';
import type {
  KnowledgeCollection,
  DocumentInput,
  DocumentSummary,
  SearchResult,
  CollectionUpdateCheck,
  DocsKbStatus,
  QueryRunSummary,
  ScopedDocumentRef,
} from '../lib/knowledgeApi';
import {
  ragListCollections,
  ragIngestDocuments,
  ragQuery,
  ragDeleteCollection,
  ragUpdateCollection,
  ragListDocuments,
  ragDeleteDocument,
  ragCheckCollectionUpdates,
  ragApplyCollectionUpdates,
  ragGetDocsStatus,
  ragEnsureDocsCollection,
  ragSyncDocsCollection,
  ragListQueryRuns,
} from '../lib/knowledgeApi';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface KnowledgeState {
  /** All collections for the active project. */
  collections: KnowledgeCollection[];

  /** Currently selected collection. */
  activeCollection: KnowledgeCollection | null;

  /** Documents in the active collection. */
  documents: DocumentSummary[];
  /** Documents cache bucketed by collection_id. */
  documentsByCollection: Record<string, DocumentSummary[]>;

  /** Query results from the last search. */
  queryResults: SearchResult[];
  /** Query state cache bucketed by collection_id. */
  queryStateByCollection: Record<
    string,
    {
      queryResults: SearchResult[];
      totalSearched: number;
      searchQuery: string;
    }
  >;

  /** Total results searched in last query. */
  totalSearched: number;

  /** Current search query text. */
  searchQuery: string;

  /** Recent retrieval execution runs for active collection. */
  queryRuns: QueryRunSummary[];
  /** Query runs cache bucketed by collection_id. */
  queryRunsByCollection: Record<string, QueryRunSummary[]>;

  /** Docs knowledge status for current workspace/project. */
  docsStatus: DocsKbStatus | null;

  /** Loading states. */
  isLoading: boolean;
  isIngesting: boolean;
  isQuerying: boolean;
  isDeleting: boolean;
  isLoadingQueryRuns: boolean;
  isLoadingDocsStatus: boolean;
  isSyncingDocs: boolean;

  /** Upload progress (0 to 100). */
  uploadProgress: number;

  /** Pending update check result. */
  pendingUpdates: CollectionUpdateCheck | null;
  isCheckingUpdates: boolean;
  isApplyingUpdates: boolean;

  /** Error message. */
  error: string | null;

  /** Actions. */
  fetchCollections: (projectId: string) => Promise<void>;
  selectCollection: (collection: KnowledgeCollection | null) => void;
  createCollection: (
    projectId: string,
    collectionName: string,
    description: string,
    documents: DocumentInput[],
  ) => Promise<boolean>;
  deleteCollection: (projectId: string, collectionName: string) => Promise<boolean>;
  updateCollection: (
    collectionId: string,
    name?: string,
    description?: string,
    workspacePath?: string | null,
  ) => Promise<boolean>;
  ingestDocuments: (projectId: string, collectionId: string, documents: DocumentInput[]) => Promise<boolean>;
  fetchDocuments: (collectionId: string) => Promise<void>;
  deleteDocument: (collectionId: string, documentUid: string) => Promise<boolean>;
  queryCollection: (
    projectId: string,
    collectionId: string,
    query: string,
    topK?: number,
    retrievalProfile?: string,
    documentFilters?: ScopedDocumentRef[],
  ) => Promise<void>;
  fetchQueryRuns: (projectId: string, collectionId: string, limit?: number) => Promise<void>;
  fetchDocsStatus: (workspacePath: string, projectId: string) => Promise<void>;
  ensureDocsCollection: (workspacePath: string, projectId: string) => Promise<boolean>;
  syncDocsCollection: (workspacePath: string, projectId: string) => Promise<boolean>;
  setSearchQuery: (query: string) => void;
  clearQueryResults: () => void;
  clearError: () => void;
  checkForUpdates: (collectionId: string) => Promise<void>;
  applyUpdates: (collectionId: string) => Promise<void>;
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_STATE = {
  collections: [],
  activeCollection: null,
  documents: [],
  documentsByCollection: {} as Record<string, DocumentSummary[]>,
  queryResults: [],
  queryStateByCollection: {} as Record<
    string,
    {
      queryResults: SearchResult[];
      totalSearched: number;
      searchQuery: string;
    }
  >,
  totalSearched: 0,
  searchQuery: '',
  queryRuns: [],
  queryRunsByCollection: {} as Record<string, QueryRunSummary[]>,
  docsStatus: null as DocsKbStatus | null,
  isLoading: false,
  isIngesting: false,
  isQuerying: false,
  isDeleting: false,
  isLoadingQueryRuns: false,
  isLoadingDocsStatus: false,
  isSyncingDocs: false,
  uploadProgress: 0,
  pendingUpdates: null as CollectionUpdateCheck | null,
  isCheckingUpdates: false,
  isApplyingUpdates: false,
  error: null,
};

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useKnowledgeStore = create<KnowledgeState>()((set, get) => ({
  ...DEFAULT_STATE,

  fetchCollections: async (projectId: string) => {
    set({ isLoading: true, error: null });
    try {
      const result = await ragListCollections(projectId);
      if (result.success && result.data) {
        set({ collections: result.data, isLoading: false });
      } else {
        set({
          isLoading: false,
          error: result.error ?? 'Failed to fetch collections',
        });
      }
    } catch (err) {
      set({
        isLoading: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  selectCollection: (collection: KnowledgeCollection | null) => {
    set((state) => {
      if (!collection) {
        return {
          activeCollection: null,
          documents: [],
          queryResults: [],
          totalSearched: 0,
          searchQuery: '',
          queryRuns: [],
          isQuerying: false,
        };
      }
      const docs = state.documentsByCollection[collection.id] ?? [];
      const queryState = state.queryStateByCollection[collection.id] ?? {
        queryResults: [],
        totalSearched: 0,
        searchQuery: '',
      };
      const queryRuns = state.queryRunsByCollection[collection.id] ?? [];
      return {
        activeCollection: collection,
        documents: docs,
        queryResults: queryState.queryResults,
        totalSearched: queryState.totalSearched,
        searchQuery: queryState.searchQuery,
        queryRuns,
        isQuerying: false,
      };
    });
  },

  createCollection: async (
    projectId: string,
    collectionName: string,
    description: string,
    documents: DocumentInput[],
  ) => {
    set({ isIngesting: true, uploadProgress: 0, error: null });
    try {
      const result = await ragIngestDocuments({
        projectId,
        collectionName,
        description,
        documents,
      });
      if (result.success && result.data) {
        set((state) => ({
          collections: [...state.collections, result.data!],
          documentsByCollection: { ...state.documentsByCollection, [result.data!.id]: [] },
          queryStateByCollection: {
            ...state.queryStateByCollection,
            [result.data!.id]: {
              queryResults: [],
              totalSearched: 0,
              searchQuery: '',
            },
          },
          queryRunsByCollection: {
            ...state.queryRunsByCollection,
            [result.data!.id]: [],
          },
          isIngesting: false,
          uploadProgress: 100,
        }));
        return true;
      } else {
        set({
          isIngesting: false,
          uploadProgress: 0,
          error: result.error ?? 'Failed to create collection',
        });
        return false;
      }
    } catch (err) {
      set({
        isIngesting: false,
        uploadProgress: 0,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  deleteCollection: async (projectId: string, collectionName: string) => {
    set({ isDeleting: true, error: null });
    try {
      const result = await ragDeleteCollection(collectionName, projectId);
      if (result.success) {
        const targetCollectionId = get().collections.find((c) => c.name === collectionName)?.id;
        set((state) => ({
          collections: state.collections.filter((c) => c.name !== collectionName),
          activeCollection: state.activeCollection?.name === collectionName ? null : state.activeCollection,
          documentsByCollection: targetCollectionId
            ? Object.fromEntries(
                Object.entries(state.documentsByCollection).filter(
                  ([collectionId]) => collectionId !== targetCollectionId,
                ),
              )
            : state.documentsByCollection,
          queryStateByCollection: targetCollectionId
            ? Object.fromEntries(
                Object.entries(state.queryStateByCollection).filter(
                  ([collectionId]) => collectionId !== targetCollectionId,
                ),
              )
            : state.queryStateByCollection,
          queryRunsByCollection: targetCollectionId
            ? Object.fromEntries(
                Object.entries(state.queryRunsByCollection).filter(
                  ([collectionId]) => collectionId !== targetCollectionId,
                ),
              )
            : state.queryRunsByCollection,
          isDeleting: false,
        }));
        return true;
      } else {
        set({
          isDeleting: false,
          error: result.error ?? 'Failed to delete collection',
        });
        return false;
      }
    } catch (err) {
      set({
        isDeleting: false,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  updateCollection: async (
    collectionId: string,
    name?: string,
    description?: string,
    workspacePath?: string | null,
  ) => {
    set({ isLoading: true, error: null });
    try {
      const result = await ragUpdateCollection(collectionId, name, description, workspacePath);
      if (result.success && result.data) {
        set((state) => ({
          collections: state.collections.map((c) => (c.id === collectionId ? result.data! : c)),
          activeCollection: state.activeCollection?.id === collectionId ? result.data! : state.activeCollection,
          isLoading: false,
        }));
        return true;
      } else {
        set({
          isLoading: false,
          error: result.error ?? 'Failed to update collection',
        });
        return false;
      }
    } catch (err) {
      set({
        isLoading: false,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  ingestDocuments: async (projectId: string, collectionId: string, documents: DocumentInput[]) => {
    set({ isIngesting: true, uploadProgress: 10, error: null });
    try {
      const result = await ragIngestDocuments({
        projectId,
        collectionId,
        documents,
      });
      if (result.success && result.data) {
        // Update the collection in the list
        set((state) => ({
          collections: state.collections.map((c) => (c.id === collectionId ? result.data! : c)),
          activeCollection: state.activeCollection?.id === collectionId ? result.data! : state.activeCollection,
          isIngesting: false,
          uploadProgress: 100,
        }));
        return true;
      } else {
        set({
          isIngesting: false,
          uploadProgress: 0,
          error: result.error ?? 'Failed to ingest documents',
        });
        return false;
      }
    } catch (err) {
      set({
        isIngesting: false,
        uploadProgress: 0,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  fetchDocuments: async (collectionId: string) => {
    set({ isLoading: true, error: null });
    try {
      const result = await ragListDocuments(collectionId);
      if (result.success && result.data) {
        set((state) => ({
          documentsByCollection: {
            ...state.documentsByCollection,
            [collectionId]: result.data!,
          },
          documents: state.activeCollection?.id === collectionId ? result.data! : state.documents,
          isLoading: false,
        }));
      } else {
        set({ isLoading: false, error: result.error ?? 'Failed to fetch documents' });
      }
    } catch (err) {
      set({ isLoading: false, error: err instanceof Error ? err.message : String(err) });
    }
  },

  deleteDocument: async (collectionId: string, documentUid: string) => {
    set({ isDeleting: true, error: null });
    try {
      const result = await ragDeleteDocument(collectionId, documentUid);
      if (result.success) {
        set((state) => ({
          documentsByCollection: {
            ...state.documentsByCollection,
            [collectionId]: (state.documentsByCollection[collectionId] ?? []).filter(
              (d) => d.document_uid !== documentUid,
            ),
          },
          documents:
            state.activeCollection?.id === collectionId
              ? state.documents.filter((d) => d.document_uid !== documentUid)
              : state.documents,
          isDeleting: false,
        }));
        return true;
      } else {
        set({ isDeleting: false, error: result.error ?? 'Failed to delete document' });
        return false;
      }
    } catch (err) {
      set({ isDeleting: false, error: err instanceof Error ? err.message : String(err) });
      return false;
    }
  },

  queryCollection: async (
    projectId: string,
    collectionId: string,
    query: string,
    topK?: number,
    retrievalProfile?: string,
    documentFilters?: ScopedDocumentRef[],
  ) => {
    set({ isQuerying: true, error: null, searchQuery: query });
    try {
      const result = await ragQuery({
        projectId,
        query,
        topK,
        collectionIds: [collectionId],
        retrievalProfile,
        documentFilters,
      });
      if (result.success && result.data) {
        let latestRuns: QueryRunSummary[] | null = null;
        const runsResult = await ragListQueryRuns(projectId, [collectionId], 20);
        if (runsResult.success && runsResult.data) {
          latestRuns = runsResult.data;
        }
        set((state) => ({
          queryStateByCollection: {
            ...state.queryStateByCollection,
            [collectionId]: {
              queryResults: result.data!.results,
              totalSearched: result.data!.total_searched,
              searchQuery: query,
            },
          },
          queryResults: state.activeCollection?.id === collectionId ? result.data!.results : state.queryResults,
          totalSearched:
            state.activeCollection?.id === collectionId ? result.data!.total_searched : state.totalSearched,
          queryRunsByCollection: latestRuns
            ? {
                ...state.queryRunsByCollection,
                [collectionId]: latestRuns,
              }
            : state.queryRunsByCollection,
          queryRuns: state.activeCollection?.id === collectionId && latestRuns ? latestRuns : state.queryRuns,
          isQuerying: false,
        }));
      } else {
        set({
          isQuerying: false,
          error: result.error ?? 'Failed to query collection',
        });
      }
    } catch (err) {
      set({
        isQuerying: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  fetchQueryRuns: async (projectId: string, collectionId: string, limit?: number) => {
    set({ isLoadingQueryRuns: true, error: null });
    try {
      const result = await ragListQueryRuns(projectId, [collectionId], limit ?? 20);
      if (result.success && result.data) {
        set((state) => ({
          queryRunsByCollection: {
            ...state.queryRunsByCollection,
            [collectionId]: result.data!,
          },
          queryRuns: state.activeCollection?.id === collectionId ? result.data! : state.queryRuns,
          isLoadingQueryRuns: false,
        }));
      } else {
        set({
          isLoadingQueryRuns: false,
          error: result.error ?? 'Failed to fetch query runs',
        });
      }
    } catch (err) {
      set({
        isLoadingQueryRuns: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  fetchDocsStatus: async (workspacePath: string, projectId: string) => {
    if (!workspacePath) {
      set({ docsStatus: null });
      return;
    }
    set({ isLoadingDocsStatus: true, error: null });
    try {
      const result = await ragGetDocsStatus(workspacePath, projectId);
      if (result.success && result.data) {
        set({ docsStatus: result.data, isLoadingDocsStatus: false });
      } else {
        set({
          isLoadingDocsStatus: false,
          error: result.error ?? 'Failed to get docs status',
        });
      }
    } catch (err) {
      set({
        isLoadingDocsStatus: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  ensureDocsCollection: async (workspacePath: string, projectId: string) => {
    if (!workspacePath) return false;
    set({ isSyncingDocs: true, error: null });
    try {
      const result = await ragEnsureDocsCollection(workspacePath, projectId);
      if (!result.success) {
        set({
          isSyncingDocs: false,
          error: result.error ?? 'Failed to ensure docs collection',
        });
        return false;
      }
      const store = get();
      await store.fetchCollections(projectId);
      await store.fetchDocsStatus(workspacePath, projectId);
      set({ isSyncingDocs: false });
      return true;
    } catch (err) {
      set({
        isSyncingDocs: false,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  syncDocsCollection: async (workspacePath: string, projectId: string) => {
    if (!workspacePath) return false;
    set({ isSyncingDocs: true, error: null });
    try {
      const result = await ragSyncDocsCollection(workspacePath, projectId);
      if (!result.success) {
        set({
          isSyncingDocs: false,
          error: result.error ?? 'Failed to sync docs collection',
        });
        return false;
      }
      const store = get();
      await store.fetchCollections(projectId);
      await store.fetchDocsStatus(workspacePath, projectId);
      set({ isSyncingDocs: false });
      return true;
    } catch (err) {
      set({
        isSyncingDocs: false,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  setSearchQuery: (query: string) => {
    set((state) => {
      const activeCollectionId = state.activeCollection?.id;
      if (!activeCollectionId) {
        return { searchQuery: query };
      }
      const current = state.queryStateByCollection[activeCollectionId] ?? {
        queryResults: [],
        totalSearched: 0,
        searchQuery: '',
      };
      return {
        searchQuery: query,
        queryStateByCollection: {
          ...state.queryStateByCollection,
          [activeCollectionId]: { ...current, searchQuery: query },
        },
      };
    });
  },

  clearQueryResults: () => {
    set((state) => {
      const activeCollectionId = state.activeCollection?.id;
      if (!activeCollectionId) {
        return { queryResults: [], totalSearched: 0, searchQuery: '' };
      }
      return {
        queryResults: [],
        totalSearched: 0,
        searchQuery: '',
        queryStateByCollection: {
          ...state.queryStateByCollection,
          [activeCollectionId]: {
            queryResults: [],
            totalSearched: 0,
            searchQuery: '',
          },
        },
      };
    });
  },

  clearError: () => set({ error: null }),

  checkForUpdates: async (collectionId: string) => {
    set({ isCheckingUpdates: true, pendingUpdates: null, error: null });
    try {
      const result = await ragCheckCollectionUpdates(collectionId);
      if (result.success && result.data) {
        set({ pendingUpdates: result.data, isCheckingUpdates: false });
      } else {
        set({
          isCheckingUpdates: false,
          error: result.error ?? 'Failed to check for updates',
        });
      }
    } catch (err) {
      set({
        isCheckingUpdates: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  applyUpdates: async (collectionId: string) => {
    set({ isApplyingUpdates: true, error: null });
    try {
      const result = await ragApplyCollectionUpdates(collectionId);
      if (result.success && result.data) {
        set((state) => ({
          collections: state.collections.map((c) => (c.id === collectionId ? result.data! : c)),
          activeCollection: state.activeCollection?.id === collectionId ? result.data! : state.activeCollection,
          isApplyingUpdates: false,
          pendingUpdates: null,
        }));
        // Refresh documents
        const store = useKnowledgeStore.getState();
        store.fetchDocuments(collectionId);
      } else {
        set({
          isApplyingUpdates: false,
          error: result.error ?? 'Failed to apply updates',
        });
      }
    } catch (err) {
      set({
        isApplyingUpdates: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },
}));

export default useKnowledgeStore;
