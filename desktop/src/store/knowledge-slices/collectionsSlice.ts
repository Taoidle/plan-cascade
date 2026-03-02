import type { DocumentInput, KnowledgeCollection } from '../../lib/knowledgeApi';
import {
  ragDeleteCollection,
  ragIngestDocuments,
  ragListCollections,
  ragUpdateCollection,
} from '../../lib/knowledgeApi';
import type { KnowledgeState } from '../knowledge';
import type { GetState, SetState } from './types';

export function createCollectionsSlice(
  set: SetState,
  get: GetState,
): Pick<
  KnowledgeState,
  'fetchCollections' | 'selectCollection' | 'createCollection' | 'deleteCollection' | 'updateCollection'
> {
  return {
    fetchCollections: async (projectId: string) => {
      set({ isLoading: true, isLoadingCollections: true, error: null });
      try {
        const result = await ragListCollections(projectId);
        if (result.success && result.data) {
          set({ collections: result.data, isLoading: false, isLoadingCollections: false });
        } else {
          set({
            isLoading: false,
            isLoadingCollections: false,
            error: result.error ?? 'Failed to fetch collections',
          });
        }
      } catch (err) {
        set({
          isLoading: false,
          isLoadingCollections: false,
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
            isLoadingDocuments: false,
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
        }
        set({
          isIngesting: false,
          uploadProgress: 0,
          error: result.error ?? 'Failed to create collection',
        });
        return false;
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
      set({ isDeleting: true, isDeletingCollection: true, error: null });
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
            isDeletingCollection: false,
          }));
          return true;
        }
        set({
          isDeleting: false,
          isDeletingCollection: false,
          error: result.error ?? 'Failed to delete collection',
        });
        return false;
      } catch (err) {
        set({
          isDeleting: false,
          isDeletingCollection: false,
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
      set({ isLoading: true, isUpdatingCollection: true, error: null });
      try {
        const result = await ragUpdateCollection(collectionId, name, description, workspacePath);
        if (result.success && result.data) {
          set((state) => ({
            collections: state.collections.map((c) => (c.id === collectionId ? result.data! : c)),
            activeCollection: state.activeCollection?.id === collectionId ? result.data! : state.activeCollection,
            isLoading: false,
            isUpdatingCollection: false,
          }));
          return true;
        }
        set({
          isLoading: false,
          isUpdatingCollection: false,
          error: result.error ?? 'Failed to update collection',
        });
        return false;
      } catch (err) {
        set({
          isLoading: false,
          isUpdatingCollection: false,
          error: err instanceof Error ? err.message : String(err),
        });
        return false;
      }
    },
  };
}
