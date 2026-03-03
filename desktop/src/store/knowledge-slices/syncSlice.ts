import {
  ragApplyCollectionUpdates,
  ragCheckCollectionUpdates,
  ragEnsureDocsCollection,
  ragGetDocsStatus,
  ragRebuildDocsCollection,
  ragSyncDocsCollection,
} from '../../lib/knowledgeApi';
import type { KnowledgeState } from '../knowledge';
import type { GetState, SetState } from './types';

export function createSyncSlice(
  set: SetState,
  get: GetState,
): Pick<
  KnowledgeState,
  | 'fetchDocsStatus'
  | 'ensureDocsCollection'
  | 'syncDocsCollection'
  | 'rebuildDocsCollection'
  | 'clearError'
  | 'checkForUpdates'
  | 'applyUpdates'
> {
  return {
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

    rebuildDocsCollection: async (
      workspacePath: string,
      projectId: string,
      mode: 'safe_swap' | 'replace' = 'safe_swap',
    ) => {
      if (!workspacePath) return false;
      set({ isSyncingDocs: true, error: null });
      try {
        const result = await ragRebuildDocsCollection(workspacePath, projectId, mode);
        if (!result.success) {
          set({
            isSyncingDocs: false,
            error: result.error ?? 'Failed to rebuild docs collection',
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
          const store = get();
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
  };
}
