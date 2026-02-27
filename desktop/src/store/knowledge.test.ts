/**
 * Knowledge Base Store Tests
 *
 * Tests all store actions: fetchCollections, createCollection,
 * deleteCollection, ingestDocuments, fetchDocuments, deleteDocument,
 * queryCollection, and state management helpers.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

// Mock Tauri APIs before importing the store
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// Import after mocks
import { useKnowledgeStore } from './knowledge';

const mockInvoke = vi.mocked(invoke);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function resetStore() {
  useKnowledgeStore.setState({
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
  });
}

const MOCK_COLLECTION = {
  id: 'col-1',
  name: 'test-collection',
  project_id: 'proj-1',
  description: 'Test collection',
  chunk_count: 10,
  created_at: '2025-01-01T00:00:00Z',
  updated_at: '2025-01-01T00:00:00Z',
};

const MOCK_COLLECTION_2 = {
  ...MOCK_COLLECTION,
  id: 'col-2',
  name: 'second-collection',
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Knowledge Store', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  // ========================================================================
  // fetchCollections
  // ========================================================================

  describe('fetchCollections', () => {
    it('fetches and sets collections on success', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: [MOCK_COLLECTION],
        error: null,
      });

      await useKnowledgeStore.getState().fetchCollections('proj-1');

      const state = useKnowledgeStore.getState();
      expect(state.collections).toHaveLength(1);
      expect(state.collections[0].name).toBe('test-collection');
      expect(state.isLoading).toBe(false);
      expect(state.error).toBeNull();
    });

    it('sets error on API failure response', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Database error',
      });

      await useKnowledgeStore.getState().fetchCollections('proj-1');

      const state = useKnowledgeStore.getState();
      expect(state.collections).toHaveLength(0);
      expect(state.isLoading).toBe(false);
      expect(state.error).toBe('Database error');
    });

    it('sets error on exception', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      await useKnowledgeStore.getState().fetchCollections('proj-1');

      const state = useKnowledgeStore.getState();
      expect(state.isLoading).toBe(false);
      expect(state.error).toBe('Network error');
    });

    it('sets isLoading during fetch', async () => {
      let resolveInvoke: (value: unknown) => void;
      mockInvoke.mockReturnValueOnce(
        new Promise((resolve) => {
          resolveInvoke = resolve;
        }),
      );

      const promise = useKnowledgeStore.getState().fetchCollections('proj-1');
      expect(useKnowledgeStore.getState().isLoading).toBe(true);

      resolveInvoke!({ success: true, data: [], error: null });
      await promise;

      expect(useKnowledgeStore.getState().isLoading).toBe(false);
    });
  });

  // ========================================================================
  // selectCollection
  // ========================================================================

  describe('selectCollection', () => {
    it('sets active collection and clears related state', () => {
      useKnowledgeStore.setState({
        documents: [{ document_id: 'd1', chunk_count: 5, preview: 'test' }],
        queryResults: [{ chunk_text: 'text', document_id: 'd1', collection_name: 'col', score: 0.9, metadata: {} }],
        searchQuery: 'old query',
        totalSearched: 100,
      });

      useKnowledgeStore.getState().selectCollection(MOCK_COLLECTION);

      const state = useKnowledgeStore.getState();
      expect(state.activeCollection).toEqual(MOCK_COLLECTION);
      expect(state.documents).toHaveLength(0);
      expect(state.queryResults).toHaveLength(0);
      expect(state.searchQuery).toBe('');
      expect(state.totalSearched).toBe(0);
    });

    it('clears active collection when set to null', () => {
      useKnowledgeStore.setState({ activeCollection: MOCK_COLLECTION });

      useKnowledgeStore.getState().selectCollection(null);

      expect(useKnowledgeStore.getState().activeCollection).toBeNull();
    });
  });

  // ========================================================================
  // createCollection
  // ========================================================================

  describe('createCollection', () => {
    it('creates collection and adds to list on success', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: MOCK_COLLECTION,
        error: null,
      });

      const result = await useKnowledgeStore.getState().createCollection('proj-1', 'test-collection', 'desc', []);

      expect(result).toBe(true);
      const state = useKnowledgeStore.getState();
      expect(state.collections).toHaveLength(1);
      expect(state.isIngesting).toBe(false);
      expect(state.uploadProgress).toBe(100);
    });

    it('returns false and sets error on failure', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Collection exists',
      });

      const result = await useKnowledgeStore.getState().createCollection('proj-1', 'test-collection', 'desc', []);

      expect(result).toBe(false);
      expect(useKnowledgeStore.getState().error).toBe('Collection exists');
      expect(useKnowledgeStore.getState().uploadProgress).toBe(0);
    });
  });

  // ========================================================================
  // deleteCollection
  // ========================================================================

  describe('deleteCollection', () => {
    it('removes collection from list on success', async () => {
      useKnowledgeStore.setState({ collections: [MOCK_COLLECTION, MOCK_COLLECTION_2] });

      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      const result = await useKnowledgeStore.getState().deleteCollection('proj-1', 'test-collection');

      expect(result).toBe(true);
      const state = useKnowledgeStore.getState();
      expect(state.collections).toHaveLength(1);
      expect(state.collections[0].name).toBe('second-collection');
    });

    it('clears activeCollection if it was the deleted one', async () => {
      useKnowledgeStore.setState({
        collections: [MOCK_COLLECTION],
        activeCollection: MOCK_COLLECTION,
      });

      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await useKnowledgeStore.getState().deleteCollection('proj-1', 'test-collection');

      expect(useKnowledgeStore.getState().activeCollection).toBeNull();
    });

    it('preserves activeCollection if different from deleted', async () => {
      useKnowledgeStore.setState({
        collections: [MOCK_COLLECTION, MOCK_COLLECTION_2],
        activeCollection: MOCK_COLLECTION_2,
      });

      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      await useKnowledgeStore.getState().deleteCollection('proj-1', 'test-collection');

      expect(useKnowledgeStore.getState().activeCollection?.name).toBe('second-collection');
    });
  });

  // ========================================================================
  // updateCollection
  // ========================================================================

  describe('updateCollection', () => {
    it('updates collection in list on success', async () => {
      const updated = {
        ...MOCK_COLLECTION,
        name: 'new-name',
        description: 'new desc',
        workspace_path: '/home/user/project',
      };
      useKnowledgeStore.setState({ collections: [MOCK_COLLECTION] });

      mockInvoke.mockResolvedValueOnce({ success: true, data: updated, error: null });

      const result = await useKnowledgeStore
        .getState()
        .updateCollection('col-1', 'new-name', 'new desc', '/home/user/project');

      expect(result).toBe(true);
      const state = useKnowledgeStore.getState();
      expect(state.collections[0].name).toBe('new-name');
      expect(state.collections[0].description).toBe('new desc');
      expect(state.isLoading).toBe(false);
    });

    it('updates activeCollection if it matches', async () => {
      const updated = { ...MOCK_COLLECTION, description: 'updated desc' };
      useKnowledgeStore.setState({
        collections: [MOCK_COLLECTION],
        activeCollection: MOCK_COLLECTION,
      });

      mockInvoke.mockResolvedValueOnce({ success: true, data: updated, error: null });

      await useKnowledgeStore.getState().updateCollection('col-1', undefined, 'updated desc');

      expect(useKnowledgeStore.getState().activeCollection?.description).toBe('updated desc');
    });

    it('sets error on failure', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Update failed',
      });

      const result = await useKnowledgeStore.getState().updateCollection('col-1', 'new-name');

      expect(result).toBe(false);
      expect(useKnowledgeStore.getState().error).toBe('Update failed');
    });

    it('preserves other collections when updating one', async () => {
      const updated = { ...MOCK_COLLECTION, description: 'changed' };
      useKnowledgeStore.setState({ collections: [MOCK_COLLECTION, MOCK_COLLECTION_2] });

      mockInvoke.mockResolvedValueOnce({ success: true, data: updated, error: null });

      await useKnowledgeStore.getState().updateCollection('col-1', undefined, 'changed');

      const state = useKnowledgeStore.getState();
      expect(state.collections).toHaveLength(2);
      expect(state.collections[0].description).toBe('changed');
      expect(state.collections[1].name).toBe('second-collection');
    });
  });

  // ========================================================================
  // ingestDocuments
  // ========================================================================

  describe('ingestDocuments', () => {
    it('updates collection in list on success', async () => {
      const updated = { ...MOCK_COLLECTION, chunk_count: 20 };
      useKnowledgeStore.setState({ collections: [MOCK_COLLECTION] });

      mockInvoke.mockResolvedValueOnce({ success: true, data: updated, error: null });

      const result = await useKnowledgeStore
        .getState()
        .ingestDocuments('proj-1', 'test-collection', [{ id: 'd1', content: 'hello' }]);

      expect(result).toBe(true);
      expect(useKnowledgeStore.getState().collections[0].chunk_count).toBe(20);
      expect(useKnowledgeStore.getState().uploadProgress).toBe(100);
    });

    it('updates activeCollection if it matches', async () => {
      const updated = { ...MOCK_COLLECTION, chunk_count: 20 };
      useKnowledgeStore.setState({
        collections: [MOCK_COLLECTION],
        activeCollection: MOCK_COLLECTION,
      });

      mockInvoke.mockResolvedValueOnce({ success: true, data: updated, error: null });

      await useKnowledgeStore.getState().ingestDocuments('proj-1', 'test-collection', [{ id: 'd1', content: 'hello' }]);

      expect(useKnowledgeStore.getState().activeCollection?.chunk_count).toBe(20);
    });

    it('sets error on failure', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Ingest failed',
      });

      const result = await useKnowledgeStore.getState().ingestDocuments('proj-1', 'test-collection', []);

      expect(result).toBe(false);
      expect(useKnowledgeStore.getState().error).toBe('Ingest failed');
    });
  });

  // ========================================================================
  // fetchDocuments
  // ========================================================================

  describe('fetchDocuments', () => {
    it('fetches and sets documents on success', async () => {
      const docs = [
        { document_id: 'doc-1', chunk_count: 5, preview: 'Hello world' },
        { document_id: 'doc-2', chunk_count: 3, preview: 'Foo bar' },
      ];
      mockInvoke.mockResolvedValueOnce({ success: true, data: docs, error: null });

      await useKnowledgeStore.getState().fetchDocuments('col-1');

      const state = useKnowledgeStore.getState();
      expect(state.documents).toHaveLength(2);
      expect(state.documents[0].document_id).toBe('doc-1');
      expect(state.isLoading).toBe(false);
    });

    it('sets error on failure', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Not found',
      });

      await useKnowledgeStore.getState().fetchDocuments('col-1');

      expect(useKnowledgeStore.getState().error).toBe('Not found');
    });
  });

  // ========================================================================
  // deleteDocument
  // ========================================================================

  describe('deleteDocument', () => {
    it('removes document from list on success', async () => {
      useKnowledgeStore.setState({
        documents: [
          { document_id: 'doc-1', chunk_count: 5, preview: 'Hello' },
          { document_id: 'doc-2', chunk_count: 3, preview: 'World' },
        ],
      });

      mockInvoke.mockResolvedValueOnce({ success: true, data: true, error: null });

      const result = await useKnowledgeStore.getState().deleteDocument('col-1', 'doc-1');

      expect(result).toBe(true);
      const state = useKnowledgeStore.getState();
      expect(state.documents).toHaveLength(1);
      expect(state.documents[0].document_id).toBe('doc-2');
    });

    it('returns false on failure', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Delete failed',
      });

      const result = await useKnowledgeStore.getState().deleteDocument('col-1', 'doc-1');

      expect(result).toBe(false);
      expect(useKnowledgeStore.getState().error).toBe('Delete failed');
    });
  });

  // ========================================================================
  // queryCollection
  // ========================================================================

  describe('queryCollection', () => {
    it('sets query results on success', async () => {
      const queryResult = {
        results: [
          { chunk_text: 'Relevant text', document_id: 'd1', collection_name: 'col', score: 0.95, metadata: {} },
        ],
        total_searched: 50,
        collection_name: 'test-collection',
      };
      mockInvoke.mockResolvedValueOnce({ success: true, data: queryResult, error: null });

      await useKnowledgeStore.getState().queryCollection('proj-1', 'test-collection', 'search term');

      const state = useKnowledgeStore.getState();
      expect(state.queryResults).toHaveLength(1);
      expect(state.queryResults[0].score).toBe(0.95);
      expect(state.totalSearched).toBe(50);
      expect(state.searchQuery).toBe('search term');
      expect(state.isQuerying).toBe(false);
    });

    it('sets error on failure', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Query failed',
      });

      await useKnowledgeStore.getState().queryCollection('proj-1', 'col', 'query');

      expect(useKnowledgeStore.getState().error).toBe('Query failed');
      expect(useKnowledgeStore.getState().isQuerying).toBe(false);
    });
  });

  // ========================================================================
  // State helpers
  // ========================================================================

  describe('state helpers', () => {
    it('setSearchQuery updates searchQuery', () => {
      useKnowledgeStore.getState().setSearchQuery('new query');
      expect(useKnowledgeStore.getState().searchQuery).toBe('new query');
    });

    it('clearQueryResults resets query state', () => {
      useKnowledgeStore.setState({
        queryResults: [{ chunk_text: 't', document_id: 'd', collection_name: 'c', score: 0.5, metadata: {} }],
        totalSearched: 10,
        searchQuery: 'old',
      });

      useKnowledgeStore.getState().clearQueryResults();

      const state = useKnowledgeStore.getState();
      expect(state.queryResults).toHaveLength(0);
      expect(state.totalSearched).toBe(0);
      expect(state.searchQuery).toBe('');
    });

    it('clearError clears the error', () => {
      useKnowledgeStore.setState({ error: 'Some error' });
      useKnowledgeStore.getState().clearError();
      expect(useKnowledgeStore.getState().error).toBeNull();
    });
  });
});
