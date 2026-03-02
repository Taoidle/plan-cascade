import type { DocumentInput } from '../../lib/knowledgeApi';
import { ragDeleteDocument, ragIngestDocuments, ragListDocuments } from '../../lib/knowledgeApi';
import type { KnowledgeState } from '../knowledge';
import type { GetState, SetState } from './types';

export function createDocumentsSlice(
  set: SetState,
  get: GetState,
): Pick<
  KnowledgeState,
  | 'ingestDocuments'
  | 'fetchDocuments'
  | 'deleteDocument'
  | 'setUploadJobProgress'
  | 'setActiveUploadJob'
  | 'clearActiveUploadJob'
> {
  return {
    ingestDocuments: async (projectId: string, collectionId: string, documents: DocumentInput[]) => {
      set((state) => ({
        isIngesting: true,
        uploadProgress: 10,
        error: null,
        activeUploadJobByCollection: Object.fromEntries(
          Object.entries(state.activeUploadJobByCollection).filter(([cid]) => cid !== collectionId),
        ),
      }));
      try {
        const result = await ragIngestDocuments({
          projectId,
          collectionId,
          documents,
        });
        if (result.success && result.data) {
          set((state) => ({
            collections: state.collections.map((c) => (c.id === collectionId ? result.data! : c)),
            activeCollection: state.activeCollection?.id === collectionId ? result.data! : state.activeCollection,
            isIngesting: false,
            uploadProgress: 100,
            activeUploadJobByCollection: Object.fromEntries(
              Object.entries(state.activeUploadJobByCollection).filter(([cid]) => cid !== collectionId),
            ),
          }));
          return true;
        }
        set({
          isIngesting: false,
          uploadProgress: 0,
          activeUploadJobByCollection: Object.fromEntries(
            Object.entries(get().activeUploadJobByCollection).filter(([cid]) => cid !== collectionId),
          ),
          error: result.error ?? 'Failed to ingest documents',
        });
        return false;
      } catch (err) {
        set({
          isIngesting: false,
          uploadProgress: 0,
          activeUploadJobByCollection: Object.fromEntries(
            Object.entries(get().activeUploadJobByCollection).filter(([cid]) => cid !== collectionId),
          ),
          error: err instanceof Error ? err.message : String(err),
        });
        return false;
      }
    },

    fetchDocuments: async (collectionId: string) => {
      set({ isLoadingDocuments: true, error: null });
      try {
        const result = await ragListDocuments(collectionId);
        if (result.success && result.data) {
          set((state) => ({
            documentsByCollection: {
              ...state.documentsByCollection,
              [collectionId]: result.data!,
            },
            documents: state.activeCollection?.id === collectionId ? result.data! : state.documents,
            isLoadingDocuments: false,
          }));
        } else {
          set({ isLoadingDocuments: false, error: result.error ?? 'Failed to fetch documents' });
        }
      } catch (err) {
        set({ isLoadingDocuments: false, error: err instanceof Error ? err.message : String(err) });
      }
    },

    deleteDocument: async (collectionId: string, documentUid: string) => {
      set({ isDeleting: true, isDeletingDocument: true, error: null });
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
            isDeletingDocument: false,
          }));
          return true;
        }
        set({
          isDeleting: false,
          isDeletingDocument: false,
          error: result.error ?? 'Failed to delete document',
        });
        return false;
      } catch (err) {
        set({
          isDeleting: false,
          isDeletingDocument: false,
          error: err instanceof Error ? err.message : String(err),
        });
        return false;
      }
    },

    setUploadJobProgress: (jobId: string, progress: number) => {
      set((state) => ({
        uploadProgressByJob: {
          ...state.uploadProgressByJob,
          [jobId]: progress,
        },
        uploadProgress: progress,
      }));
    },

    setActiveUploadJob: (collectionId: string, jobId: string) => {
      set((state) => ({
        activeUploadJobByCollection: {
          ...state.activeUploadJobByCollection,
          [collectionId]: jobId,
        },
      }));
    },

    clearActiveUploadJob: (collectionId: string) => {
      set((state) => ({
        activeUploadJobByCollection: Object.fromEntries(
          Object.entries(state.activeUploadJobByCollection).filter(([cid]) => cid !== collectionId),
        ),
      }));
    },
  };
}
