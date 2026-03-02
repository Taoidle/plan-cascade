import type { ScopedDocumentRef, QueryRunSummary } from '../../lib/knowledgeApi';
import { ragListQueryRuns, ragQuery } from '../../lib/knowledgeApi';
import type { KnowledgeState } from '../knowledge';
import type { SetState } from './types';

export function createQuerySlice(
  set: SetState,
): Pick<KnowledgeState, 'queryCollection' | 'fetchQueryRuns' | 'setSearchQuery' | 'clearQueryResults'> {
  return {
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
  };
}
