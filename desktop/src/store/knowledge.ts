/**
 * Knowledge Base Store
 *
 * Zustand store for project knowledge base management. Manages
 * collections, document upload progress, and query results via
 * Tauri IPC commands.
 */

import { create } from 'zustand';
import type { KnowledgeCollection, DocumentInput, DocumentSummary, SearchResult } from '../lib/knowledgeApi';
import {
  ragListCollections,
  ragIngestDocuments,
  ragQuery,
  ragDeleteCollection,
  ragUpdateCollection,
  ragListDocuments,
  ragDeleteDocument,
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

  /** Query results from the last search. */
  queryResults: SearchResult[];

  /** Total results searched in last query. */
  totalSearched: number;

  /** Current search query text. */
  searchQuery: string;

  /** Loading states. */
  isLoading: boolean;
  isIngesting: boolean;
  isQuerying: boolean;
  isDeleting: boolean;

  /** Upload progress (0 to 100). */
  uploadProgress: number;

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
  ingestDocuments: (projectId: string, collectionName: string, documents: DocumentInput[]) => Promise<boolean>;
  fetchDocuments: (collectionId: string) => Promise<void>;
  deleteDocument: (collectionId: string, documentId: string) => Promise<boolean>;
  queryCollection: (projectId: string, collectionName: string, query: string, topK?: number) => Promise<void>;
  setSearchQuery: (query: string) => void;
  clearQueryResults: () => void;
  clearError: () => void;
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_STATE = {
  collections: [],
  activeCollection: null,
  documents: [],
  queryResults: [],
  totalSearched: 0,
  searchQuery: '',
  isLoading: false,
  isIngesting: false,
  isQuerying: false,
  isDeleting: false,
  uploadProgress: 0,
  error: null,
};

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useKnowledgeStore = create<KnowledgeState>()((set, _get) => ({
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
    set({
      activeCollection: collection,
      documents: [],
      queryResults: [],
      totalSearched: 0,
      searchQuery: '',
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
      const result = await ragIngestDocuments(collectionName, projectId, description, documents);
      if (result.success && result.data) {
        set((state) => ({
          collections: [...state.collections, result.data!],
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
        set((state) => ({
          collections: state.collections.filter((c) => c.name !== collectionName),
          activeCollection: state.activeCollection?.name === collectionName ? null : state.activeCollection,
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

  ingestDocuments: async (projectId: string, collectionName: string, documents: DocumentInput[]) => {
    set({ isIngesting: true, uploadProgress: 10, error: null });
    try {
      const result = await ragIngestDocuments(collectionName, projectId, null, documents);
      if (result.success && result.data) {
        // Update the collection in the list
        set((state) => ({
          collections: state.collections.map((c) => (c.name === collectionName ? result.data! : c)),
          activeCollection: state.activeCollection?.name === collectionName ? result.data! : state.activeCollection,
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
        set({ documents: result.data, isLoading: false });
      } else {
        set({ isLoading: false, error: result.error ?? 'Failed to fetch documents' });
      }
    } catch (err) {
      set({ isLoading: false, error: err instanceof Error ? err.message : String(err) });
    }
  },

  deleteDocument: async (collectionId: string, documentId: string) => {
    set({ isDeleting: true, error: null });
    try {
      const result = await ragDeleteDocument(collectionId, documentId);
      if (result.success) {
        set((state) => ({
          documents: state.documents.filter((d) => d.document_id !== documentId),
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

  queryCollection: async (projectId: string, collectionName: string, query: string, topK?: number) => {
    set({ isQuerying: true, error: null, searchQuery: query });
    try {
      const result = await ragQuery(collectionName, projectId, query, topK);
      if (result.success && result.data) {
        set({
          queryResults: result.data.results,
          totalSearched: result.data.total_searched,
          isQuerying: false,
        });
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

  setSearchQuery: (query: string) => {
    set({ searchQuery: query });
  },

  clearQueryResults: () => {
    set({ queryResults: [], totalSearched: 0, searchQuery: '' });
  },

  clearError: () => set({ error: null }),
}));

export default useKnowledgeStore;
