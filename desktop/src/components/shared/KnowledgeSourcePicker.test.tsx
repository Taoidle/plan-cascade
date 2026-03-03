import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { KnowledgeSourcePicker } from './KnowledgeSourcePicker';
import { useContextSourcesStore } from '../../store/contextSources';
import { useProjectsStore } from '../../store/projects';

const { mockRagSearchDocuments, mockRagListDocuments, mockRagRecordPickerSearch } = vi.hoisted(() => ({
  mockRagSearchDocuments: vi.fn(),
  mockRagListDocuments: vi.fn(),
  mockRagRecordPickerSearch: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue || key,
  }),
}));

vi.mock('../../lib/knowledgeApi', async () => {
  const actual = await vi.importActual<typeof import('../../lib/knowledgeApi')>('../../lib/knowledgeApi');
  return {
    ...actual,
    ragSearchDocuments: mockRagSearchDocuments,
    ragListDocuments: mockRagListDocuments,
    ragRecordPickerSearch: mockRagRecordPickerSearch,
  };
});

function resetContextSourcesState() {
  useContextSourcesStore.setState({
    knowledgeEnabled: true,
    selectedCollections: [],
    selectedDocuments: [],
    availableCollections: [],
    collectionDocuments: {},
    isLoadingCollections: false,
    isLoadingDocuments: {},
    memoryEnabled: true,
    memorySelectionMode: 'auto',
    selectedMemoryScopes: ['global', 'project', 'session'],
    memorySessionId: null,
    selectedMemoryCategories: [],
    selectedMemoryIds: [],
    includedMemoryIds: [],
    excludedMemoryIds: [],
    availableMemoryStats: null,
    categoryMemories: {},
    isLoadingMemoryStats: false,
    isLoadingCategoryMemories: {},
    memoryPickerSearchQuery: '',
    memorySearchResults: null,
    isSearchingMemories: false,
    skillsEnabled: false,
    selectedSkillIds: [],
    availableSkills: [],
    isLoadingSkills: false,
    skillPickerSearchQuery: '',
    _autoAssociatedScopes: {},
  });
}

describe('KnowledgeSourcePicker', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetContextSourcesState();
    useProjectsStore.setState({
      selectedProject: {
        id: 'proj-1',
        name: 'Project 1',
        path: '/tmp/proj-1',
        last_activity: new Date().toISOString(),
        session_count: 0,
        message_count: 0,
      },
    });
    useContextSourcesStore.setState({
      availableCollections: [
        {
          id: 'col-1',
          name: 'Collection A',
          project_id: 'proj-1',
          description: '',
          chunk_count: 0,
          created_at: '',
          updated_at: '',
        },
        {
          id: 'col-2',
          name: 'Collection B',
          project_id: 'proj-1',
          description: '',
          chunk_count: 0,
          created_at: '',
          updated_at: '',
        },
      ],
    });
    mockRagListDocuments.mockResolvedValue({ success: true, data: [], error: null });
  });

  it('finds documents from collections that have not been expanded', async () => {
    mockRagSearchDocuments.mockResolvedValue({
      success: true,
      data: [
        {
          collection_id: 'col-2',
          document_uid: 'doc-remote',
          display_name: 'Remote API Spec',
        },
      ],
      error: null,
    });

    render(<KnowledgeSourcePicker />);
    const input = screen.getByPlaceholderText('Search collections...');
    fireEvent.change(input, { target: { value: 'remote' } });

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 320));
    });

    await waitFor(() => expect(mockRagSearchDocuments).toHaveBeenCalledWith('proj-1', 'remote', undefined, 80));
    expect(mockRagRecordPickerSearch).toHaveBeenCalledWith(false);
    expect(screen.getByText('Remote API Spec')).toBeInTheDocument();
  });

  it('loads target collection docs when selecting a remote search result', async () => {
    mockRagSearchDocuments.mockResolvedValue({
      success: true,
      data: [
        {
          collection_id: 'col-2',
          document_uid: 'doc-remote',
          display_name: 'Remote API Spec',
        },
      ],
      error: null,
    });

    render(<KnowledgeSourcePicker />);
    const input = screen.getByPlaceholderText('Search collections...');
    fireEvent.change(input, { target: { value: 'remote' } });

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 320));
    });
    await waitFor(() => screen.getByText('Remote API Spec'));

    const row = screen.getByText('Remote API Spec').closest('div');
    const checkbox = row?.querySelector('input[type="checkbox"]') as HTMLInputElement | null;
    expect(checkbox).toBeTruthy();
    fireEvent.click(checkbox!);

    await waitFor(() => {
      expect(mockRagListDocuments).toHaveBeenCalledWith('col-2');
    });
  });
});
