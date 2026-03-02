import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useContextSourcesStore } from './contextSources';
import { useProjectsStore } from './projects';
import { useSettingsStore } from './settings';
import type { Project } from '../types/project';

const { mockRagListCollections, mockRagEnsureDocsCollection } = vi.hoisted(() => ({
  mockRagListCollections: vi.fn(),
  mockRagEnsureDocsCollection: vi.fn(),
}));

vi.mock('../lib/knowledgeApi', () => ({
  ragListCollections: mockRagListCollections,
  ragListDocuments: vi.fn(),
  ragEnsureDocsCollection: mockRagEnsureDocsCollection,
}));

function resetContextSourcesState() {
  useContextSourcesStore.setState({
    knowledgeEnabled: false,
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
    _autoAssociatedPath: null,
  });
}

describe('useContextSourcesStore', () => {
  beforeEach(() => {
    resetContextSourcesState();
    useProjectsStore.setState({ selectedProject: null });
    useSettingsStore.setState({ knowledgeAutoEnsureDocsCollection: false });
    mockRagListCollections.mockResolvedValue({ success: true, data: [], error: null });
    mockRagEnsureDocsCollection.mockResolvedValue({ success: true, data: null, error: null });
  });

  it('buildConfig in auto mode maps to excluded_memory_ids only', () => {
    useContextSourcesStore.setState({
      memorySelectionMode: 'auto',
      includedMemoryIds: ['mem-included-1'],
      excludedMemoryIds: ['mem-excluded-1', 'mem-excluded-2'],
      selectedMemoryIds: ['legacy-should-be-ignored'],
    });

    const config = useContextSourcesStore.getState().buildConfig();
    expect(config.memory?.selected_memory_ids).toEqual([]);
    expect(config.memory?.excluded_memory_ids).toEqual(['mem-excluded-1', 'mem-excluded-2']);
  });

  it('buildConfig in only_selected mode maps to selected_memory_ids only', () => {
    useContextSourcesStore.setState({
      memorySelectionMode: 'only_selected',
      includedMemoryIds: ['mem-picked-1', 'mem-picked-2'],
      excludedMemoryIds: ['mem-excluded-ignored'],
      selectedMemoryIds: ['legacy-ignored'],
    });

    const config = useContextSourcesStore.getState().buildConfig();
    expect(config.memory?.selected_memory_ids).toEqual(['mem-picked-1', 'mem-picked-2']);
    expect(config.memory?.excluded_memory_ids).toEqual([]);
  });

  it('buildConfig keeps legacy selectedMemoryIds as exclusion fallback in auto mode', () => {
    useContextSourcesStore.setState({
      memorySelectionMode: 'auto',
      excludedMemoryIds: [],
      selectedMemoryIds: ['legacy-excluded-1'],
    });

    const config = useContextSourcesStore.getState().buildConfig();
    expect(config.memory?.selected_memory_ids).toEqual([]);
    expect(config.memory?.excluded_memory_ids).toEqual(['legacy-excluded-1']);
  });

  it('toggleMemoryItem in auto mode toggles excludedMemoryIds and compat alias', () => {
    const store = useContextSourcesStore.getState();
    store.setMemorySelectionMode('auto');
    store.toggleMemoryItem('mem-1');

    let state = useContextSourcesStore.getState();
    expect(state.excludedMemoryIds).toEqual(['mem-1']);
    expect(state.selectedMemoryIds).toEqual(['mem-1']);

    state.toggleMemoryItem('mem-1');
    state = useContextSourcesStore.getState();
    expect(state.excludedMemoryIds).toEqual([]);
    expect(state.selectedMemoryIds).toEqual([]);
  });

  it('toggleMemoryItem in only_selected mode toggles includedMemoryIds and compat alias', () => {
    const store = useContextSourcesStore.getState();
    store.setMemorySelectionMode('only_selected');
    store.toggleMemoryItem('mem-2');

    let state = useContextSourcesStore.getState();
    expect(state.includedMemoryIds).toEqual(['mem-2']);
    expect(state.selectedMemoryIds).toEqual(['mem-2']);

    state.toggleMemoryItem('mem-2');
    state = useContextSourcesStore.getState();
    expect(state.includedMemoryIds).toEqual([]);
    expect(state.selectedMemoryIds).toEqual([]);
  });

  it('buildConfig uses selected project id when available', () => {
    const selectedProject: Project = {
      id: 'project-123',
      name: 'Project 123',
      path: '/tmp/project-123',
      last_activity: new Date().toISOString(),
      session_count: 0,
      message_count: 0,
    };
    useProjectsStore.setState({
      selectedProject,
    });

    const config = useContextSourcesStore.getState().buildConfig();
    expect(config.project_id).toBe('project-123');
  });

  it('autoAssociateForWorkspace does not auto-ensure docs when opt-in is disabled', async () => {
    useSettingsStore.setState({ knowledgeAutoEnsureDocsCollection: false });
    mockRagListCollections.mockResolvedValue({
      success: true,
      data: [
        {
          id: 'col-1',
          name: 'Workspace Notes',
          project_id: 'proj-1',
          description: '',
          chunk_count: 0,
          created_at: '',
          updated_at: '',
          workspace_path: '/workspace/demo',
        },
      ],
      error: null,
    });

    await useContextSourcesStore.getState().autoAssociateForWorkspace('/workspace/demo', 'proj-1');

    expect(mockRagEnsureDocsCollection).not.toHaveBeenCalled();
  });

  it('autoAssociateForWorkspace ensures docs when opt-in is enabled', async () => {
    useSettingsStore.setState({ knowledgeAutoEnsureDocsCollection: true });
    mockRagListCollections.mockResolvedValue({
      success: true,
      data: [],
      error: null,
    });
    mockRagEnsureDocsCollection.mockResolvedValue({
      success: true,
      data: {
        id: 'docs-col',
        name: '[Docs] /workspace/demo',
        project_id: 'proj-1',
        description: '',
        chunk_count: 0,
        created_at: '',
        updated_at: '',
        workspace_path: '/workspace/demo',
      },
      error: null,
    });

    await useContextSourcesStore.getState().autoAssociateForWorkspace('/workspace/demo', 'proj-1');

    expect(mockRagEnsureDocsCollection).toHaveBeenCalledWith('/workspace/demo', 'proj-1');
  });
});
