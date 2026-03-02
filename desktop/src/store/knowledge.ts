/**
 * Knowledge Base Store
 *
 * Zustand store for project knowledge base management, organized into domain slices:
 * - collections
 * - documents
 * - query
 * - sync
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
import { createCollectionsSlice } from './knowledge-slices/collectionsSlice';
import { createDocumentsSlice } from './knowledge-slices/documentsSlice';
import { createQuerySlice } from './knowledge-slices/querySlice';
import { createSyncSlice } from './knowledge-slices/syncSlice';

export interface KnowledgeState {
  collections: KnowledgeCollection[];
  activeCollection: KnowledgeCollection | null;

  documents: DocumentSummary[];
  documentsByCollection: Record<string, DocumentSummary[]>;

  queryResults: SearchResult[];
  queryStateByCollection: Record<
    string,
    {
      queryResults: SearchResult[];
      totalSearched: number;
      searchQuery: string;
    }
  >;

  totalSearched: number;
  searchQuery: string;

  queryRuns: QueryRunSummary[];
  queryRunsByCollection: Record<string, QueryRunSummary[]>;

  docsStatus: DocsKbStatus | null;

  isLoading: boolean;
  isLoadingCollections: boolean;
  isLoadingDocuments: boolean;
  isUpdatingCollection: boolean;
  isIngesting: boolean;
  isQuerying: boolean;
  isDeleting: boolean;
  isDeletingCollection: boolean;
  isDeletingDocument: boolean;
  isLoadingQueryRuns: boolean;
  isLoadingDocsStatus: boolean;
  isSyncingDocs: boolean;

  uploadProgress: number;
  uploadProgressByJob: Record<string, number>;
  activeUploadJobByCollection: Record<string, string>;

  pendingUpdates: CollectionUpdateCheck | null;
  isCheckingUpdates: boolean;
  isApplyingUpdates: boolean;

  error: string | null;

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
  setUploadJobProgress: (jobId: string, progress: number) => void;
  setActiveUploadJob: (collectionId: string, jobId: string) => void;
  clearActiveUploadJob: (collectionId: string) => void;

  queryCollection: (
    projectId: string,
    collectionId: string,
    query: string,
    topK?: number,
    retrievalProfile?: string,
    documentFilters?: ScopedDocumentRef[],
  ) => Promise<void>;
  fetchQueryRuns: (projectId: string, collectionId: string, limit?: number) => Promise<void>;
  setSearchQuery: (query: string) => void;
  clearQueryResults: () => void;

  fetchDocsStatus: (workspacePath: string, projectId: string) => Promise<void>;
  ensureDocsCollection: (workspacePath: string, projectId: string) => Promise<boolean>;
  syncDocsCollection: (workspacePath: string, projectId: string) => Promise<boolean>;
  clearError: () => void;
  checkForUpdates: (collectionId: string) => Promise<void>;
  applyUpdates: (collectionId: string) => Promise<void>;
}

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
  isLoadingCollections: false,
  isLoadingDocuments: false,
  isUpdatingCollection: false,
  isIngesting: false,
  isQuerying: false,
  isDeleting: false,
  isDeletingCollection: false,
  isDeletingDocument: false,
  isLoadingQueryRuns: false,
  isLoadingDocsStatus: false,
  isSyncingDocs: false,
  uploadProgress: 0,
  uploadProgressByJob: {} as Record<string, number>,
  activeUploadJobByCollection: {} as Record<string, string>,
  pendingUpdates: null as CollectionUpdateCheck | null,
  isCheckingUpdates: false,
  isApplyingUpdates: false,
  error: null,
};

export const useKnowledgeStore = create<KnowledgeState>()((set, get) => ({
  ...DEFAULT_STATE,
  ...createCollectionsSlice(set, get),
  ...createDocumentsSlice(set, get),
  ...createQuerySlice(set),
  ...createSyncSlice(set, get),
}));

export default useKnowledgeStore;
