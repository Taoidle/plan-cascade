import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { ContextSourceBar } from './ContextSourceBar';
import { useContextSourcesStore } from '../../store/contextSources';
import { useProjectsStore } from '../../store/projects';
import { useSettingsStore } from '../../store/settings';
import { useExecutionStore } from '../../store/execution';
import { useWorkflowKernelStore } from '../../store/workflowKernel';

const {
  mockListen,
  mockRagListCollections,
  mockRagListDocuments,
  mockRagEnsureDocsCollection,
  mockRagSyncDocsCollection,
} = vi.hoisted(() => ({
  mockListen: vi.fn().mockResolvedValue(() => {}),
  mockRagListCollections: vi.fn().mockResolvedValue({ success: true, data: [], error: null }),
  mockRagListDocuments: vi.fn().mockResolvedValue({ success: true, data: [], error: null }),
  mockRagEnsureDocsCollection: vi.fn().mockResolvedValue({ success: true, data: null, error: null }),
  mockRagSyncDocsCollection: vi.fn().mockResolvedValue({ success: true, data: null, error: null }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: mockListen,
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

vi.mock('../../lib/knowledgeApi', () => ({
  ragListCollections: mockRagListCollections,
  ragListDocuments: mockRagListDocuments,
  ragEnsureDocsCollection: mockRagEnsureDocsCollection,
  ragSyncDocsCollection: mockRagSyncDocsCollection,
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
    selectedMemoryScopes: ['global', 'project'],
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
    skillSelectionMode: 'auto',
    availableSkills: [],
    isLoadingSkills: false,
    skillPickerSearchQuery: '',
    _autoAssociatedScopes: {},
  });
}

describe('ContextSourceBar integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetContextSourcesState();
    useSettingsStore.setState({ workspacePath: '' });
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
    useExecutionStore.setState({
      taskId: null,
      standaloneSessionId: null,
      foregroundOriginSessionId: null,
    });
    useWorkflowKernelStore.setState({
      session: null,
      sessionId: null,
    });
  });

  it('keeps UI toggle state and memory store state consistent', () => {
    useContextSourcesStore.setState({
      memoryEnabled: true,
      memorySelectionMode: 'only_selected',
      selectedMemoryScopes: ['project', 'global'],
      selectedMemoryCategories: ['fact'],
      includedMemoryIds: ['mem-1'],
      selectedMemoryIds: ['mem-1'],
    });

    render(<ContextSourceBar />);

    const memoryButton = screen.getByRole('button', { name: /^Memory/ });
    // 2 scopes + 1 category + 1 selected item
    expect(within(memoryButton).getByText('4')).toBeInTheDocument();

    fireEvent.click(memoryButton);

    const state = useContextSourcesStore.getState();
    expect(state.memoryEnabled).toBe(false);
    expect(state.selectedMemoryCategories).toEqual([]);
    expect(state.includedMemoryIds).toEqual([]);
    expect(state.excludedMemoryIds).toEqual([]);
    expect(state.selectedMemoryIds).toEqual([]);
  });

  it('resets skill explicit selection when skills are toggled off from toolbar', () => {
    useContextSourcesStore.setState({
      skillsEnabled: true,
      selectedSkillIds: ['skill-a', 'skill-b'],
      skillSelectionMode: 'explicit',
    });

    render(<ContextSourceBar />);
    fireEvent.click(screen.getByRole('button', { name: /^Skills/ }));

    const state = useContextSourcesStore.getState();
    expect(state.skillsEnabled).toBe(false);
    expect(state.selectedSkillIds).toEqual([]);
    expect(state.skillSelectionMode).toBe('auto');
  });
});
