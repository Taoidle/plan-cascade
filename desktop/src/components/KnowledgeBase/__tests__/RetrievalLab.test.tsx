import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { RetrievalLab } from '../RetrievalLab';
import { useKnowledgeStore } from '../../../store/knowledge';

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue || key,
  }),
}));

describe('RetrievalLab', () => {
  const fetchDocuments = vi.fn(async () => {});
  const fetchQueryRuns = vi.fn(async () => {});
  const queryCollection = vi.fn(async () => {});

  beforeEach(() => {
    vi.clearAllMocks();

    useKnowledgeStore.setState({
      activeCollection: {
        id: 'col-1',
        name: 'Collection 1',
        project_id: 'proj-1',
        description: '',
        chunk_count: 0,
        created_at: '',
        updated_at: '',
      },
      documents: [
        {
          document_uid: 'doc-1',
          display_name: 'Design Spec',
          source_kind: 'upload',
          source_locator: 'upload://manual/id/design.md',
          source_type: 'md',
          trackable: false,
          last_indexed_at: '2026-01-01T00:00:00Z',
          chunk_count: 2,
          preview: 'preview',
        },
      ],
      queryResults: [],
      totalSearched: 0,
      searchQuery: '',
      isQuerying: false,
      isLoadingQueryRuns: false,
      queryRuns: [
        {
          id: 7,
          project_id: 'proj-1',
          query: 'design',
          collection_scope: 'Collection 1',
          retrieval_profile: 'precision',
          top_k: 10,
          vector_candidates: 40,
          bm25_candidates: 30,
          merged_candidates: 20,
          rerank_ms: 11,
          total_ms: 53,
          result_count: 4,
          created_at: '2026-03-01T12:00:00Z',
        },
      ],
      fetchDocuments,
      fetchQueryRuns,
      queryCollection,
      setSearchQuery: (query: string) => useKnowledgeStore.setState({ searchQuery: query }),
      clearQueryResults: () => useKnowledgeStore.setState({ queryResults: [], totalSearched: 0, searchQuery: '' }),
    });
  });

  it('passes selected retrieval profile to queryCollection', async () => {
    render(<RetrievalLab projectId="proj-1" collectionId="col-1" />);

    fireEvent.change(screen.getByPlaceholderText('query.placeholder'), { target: { value: 'remote api' } });
    fireEvent.change(screen.getByDisplayValue('Balanced'), { target: { value: 'recall' } });
    fireEvent.click(screen.getByRole('button', { name: 'query.search' }));

    await waitFor(() => {
      expect(queryCollection).toHaveBeenCalledWith('proj-1', 'col-1', 'remote api', 10, 'recall', undefined);
    });
  });

  it('renders retrieval profile from run history', () => {
    render(<RetrievalLab projectId="proj-1" collectionId="col-1" />);

    expect(screen.getByText('precision')).toBeInTheDocument();
  });
});
