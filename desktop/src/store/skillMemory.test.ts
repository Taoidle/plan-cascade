/**
 * Skill & Memory Store Tests
 *
 * Tests for the Zustand skillMemory store including state management,
 * UI toggles, and IPC action mocking.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useSkillMemoryStore } from './skillMemory';
import type { SkillSummary, MemoryEntry, SkillsOverview } from '../types/skillMemory';

// Mock invoke is already mocked in test setup
const mockInvoke = vi.mocked(invoke);

// Helper factories
function createMockSkillSummary(overrides: Partial<SkillSummary> = {}): SkillSummary {
  return {
    id: 'skill-1',
    name: 'Test Skill',
    description: 'A test skill',
    version: null,
    tags: ['test'],
    source: { type: 'builtin' },
    priority: 10,
    enabled: true,
    detected: false,
    user_invocable: false,
    has_hooks: false,
    inject_into: ['always'],
    path: '/skills/test.md',
    ...overrides,
  };
}

function createMockMemoryEntry(overrides: Partial<MemoryEntry> = {}): MemoryEntry {
  return {
    id: 'mem-1',
    project_path: '/test/project',
    category: 'fact',
    content: 'Test memory content',
    keywords: ['test'],
    importance: 0.5,
    access_count: 0,
    source_session_id: null,
    source_context: null,
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T00:00:00Z',
    last_accessed_at: '2025-01-01T00:00:00Z',
    ...overrides,
  };
}

describe('useSkillMemoryStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSkillMemoryStore.getState().reset();
  });

  // ========================================================================
  // UI State Tests
  // ========================================================================

  describe('UI State', () => {
    it('should initialize with panel closed and dialog closed', () => {
      const state = useSkillMemoryStore.getState();
      expect(state.panelOpen).toBe(false);
      expect(state.dialogOpen).toBe(false);
    });

    it('should toggle panel open/closed', () => {
      const { togglePanel } = useSkillMemoryStore.getState();
      togglePanel();
      expect(useSkillMemoryStore.getState().panelOpen).toBe(true);
      togglePanel();
      expect(useSkillMemoryStore.getState().panelOpen).toBe(false);
    });

    it('should open dialog with default tab', () => {
      const { openDialog } = useSkillMemoryStore.getState();
      openDialog();
      expect(useSkillMemoryStore.getState().dialogOpen).toBe(true);
      expect(useSkillMemoryStore.getState().activeTab).toBe('skills');
    });

    it('should open dialog with specific tab', () => {
      const { openDialog } = useSkillMemoryStore.getState();
      openDialog('memory');
      expect(useSkillMemoryStore.getState().dialogOpen).toBe(true);
      expect(useSkillMemoryStore.getState().activeTab).toBe('memory');
    });

    it('should close dialog', () => {
      const { openDialog, closeDialog } = useSkillMemoryStore.getState();
      openDialog();
      expect(useSkillMemoryStore.getState().dialogOpen).toBe(true);
      closeDialog();
      expect(useSkillMemoryStore.getState().dialogOpen).toBe(false);
    });

    it('should set active tab', () => {
      const { setActiveTab } = useSkillMemoryStore.getState();
      setActiveTab('memory');
      expect(useSkillMemoryStore.getState().activeTab).toBe('memory');
      setActiveTab('skills');
      expect(useSkillMemoryStore.getState().activeTab).toBe('skills');
    });

    it('should show and clear toast', () => {
      const { showToast, clearToast } = useSkillMemoryStore.getState();
      showToast('Test message', 'success');
      expect(useSkillMemoryStore.getState().toastMessage).toBe('Test message');
      expect(useSkillMemoryStore.getState().toastType).toBe('success');
      clearToast();
      expect(useSkillMemoryStore.getState().toastMessage).toBeNull();
    });

    it('should set skill search query', () => {
      const { setSkillSearchQuery } = useSkillMemoryStore.getState();
      setSkillSearchQuery('react');
      expect(useSkillMemoryStore.getState().skillSearchQuery).toBe('react');
    });

    it('should set skill source filter', () => {
      const { setSkillSourceFilter } = useSkillMemoryStore.getState();
      setSkillSourceFilter('builtin');
      expect(useSkillMemoryStore.getState().skillSourceFilter).toBe('builtin');
    });

    it('should set memory search query', () => {
      const { setMemorySearchQuery } = useSkillMemoryStore.getState();
      setMemorySearchQuery('convention');
      expect(useSkillMemoryStore.getState().memorySearchQuery).toBe('convention');
    });

    it('should set memory category filter', () => {
      const { setMemoryCategoryFilter } = useSkillMemoryStore.getState();
      setMemoryCategoryFilter('pattern');
      expect(useSkillMemoryStore.getState().memoryCategoryFilter).toBe('pattern');
    });

    it('should reset to default state', () => {
      const store = useSkillMemoryStore.getState();
      store.openDialog('memory');
      store.setSkillSearchQuery('test');
      store.showToast('message');
      store.reset();
      const state = useSkillMemoryStore.getState();
      expect(state.dialogOpen).toBe(false);
      expect(state.skillSearchQuery).toBe('');
      expect(state.toastMessage).toBeNull();
    });
  });

  // ========================================================================
  // Skill Actions Tests
  // ========================================================================

  describe('Skill Actions', () => {
    it('should load skills successfully', async () => {
      const mockSkills = [
        createMockSkillSummary({ id: 'skill-1', name: 'Skill A' }),
        createMockSkillSummary({ id: 'skill-2', name: 'Skill B' }),
      ];
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: mockSkills,
        error: null,
      });

      await useSkillMemoryStore.getState().loadSkills('/test/project');

      const state = useSkillMemoryStore.getState();
      expect(state.skills).toHaveLength(2);
      expect(state.skillsLoading).toBe(false);
      expect(state.skillsError).toBeNull();
      expect(mockInvoke).toHaveBeenCalledWith('list_skills', {
        projectPath: '/test/project',
        sourceFilter: null,
        includeDisabled: true,
      });
    });

    it('should handle load skills error', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Failed to load',
      });

      await useSkillMemoryStore.getState().loadSkills('/test/project');

      const state = useSkillMemoryStore.getState();
      expect(state.skills).toHaveLength(0);
      expect(state.skillsError).toBe('Failed to load');
      expect(state.skillsLoading).toBe(false);
    });

    it('should toggle skill with optimistic update', async () => {
      // Pre-populate skills
      useSkillMemoryStore.setState({
        skills: [createMockSkillSummary({ id: 'skill-1', enabled: true })],
      });

      mockInvoke.mockResolvedValueOnce({ success: true, data: null, error: null });

      await useSkillMemoryStore.getState().toggleSkill('skill-1', false);

      const skill = useSkillMemoryStore.getState().skills.find((s) => s.id === 'skill-1');
      expect(skill?.enabled).toBe(false);
    });

    it('should revert toggle on failure', async () => {
      useSkillMemoryStore.setState({
        skills: [createMockSkillSummary({ id: 'skill-1', enabled: true })],
      });

      mockInvoke.mockResolvedValueOnce({ success: false, data: null, error: 'Failed' });

      await useSkillMemoryStore.getState().toggleSkill('skill-1', false);

      const skill = useSkillMemoryStore.getState().skills.find((s) => s.id === 'skill-1');
      expect(skill?.enabled).toBe(true);
    });

    it('should delete skill', async () => {
      useSkillMemoryStore.setState({
        skills: [
          createMockSkillSummary({ id: 'skill-1' }),
          createMockSkillSummary({ id: 'skill-2' }),
        ],
      });

      mockInvoke.mockResolvedValueOnce({ success: true, data: null, error: null });

      await useSkillMemoryStore.getState().deleteSkill('skill-1', '/test/project');

      expect(useSkillMemoryStore.getState().skills).toHaveLength(1);
      expect(useSkillMemoryStore.getState().skills[0].id).toBe('skill-2');
    });
  });

  // ========================================================================
  // Memory Actions Tests
  // ========================================================================

  describe('Memory Actions', () => {
    it('should load memories successfully', async () => {
      const mockMemories = [
        createMockMemoryEntry({ id: 'mem-1' }),
        createMockMemoryEntry({ id: 'mem-2' }),
      ];
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: mockMemories,
        error: null,
      });

      await useSkillMemoryStore.getState().loadMemories('/test/project');

      const state = useSkillMemoryStore.getState();
      expect(state.memories).toHaveLength(2);
      expect(state.memoriesLoading).toBe(false);
      expect(state.memoriesError).toBeNull();
    });

    it('should handle load memories error', async () => {
      mockInvoke.mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Failed to load memories',
      });

      await useSkillMemoryStore.getState().loadMemories('/test/project');

      const state = useSkillMemoryStore.getState();
      expect(state.memories).toHaveLength(0);
      expect(state.memoriesError).toBe('Failed to load memories');
    });

    it('should add memory and prepend to list', async () => {
      useSkillMemoryStore.setState({
        memories: [createMockMemoryEntry({ id: 'mem-existing' })],
      });

      const newMemory = createMockMemoryEntry({ id: 'mem-new', content: 'New memory' });
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: newMemory,
        error: null,
      });

      await useSkillMemoryStore.getState().addMemory(
        '/test/project',
        'fact',
        'New memory',
        ['test'],
        0.7
      );

      const state = useSkillMemoryStore.getState();
      expect(state.memories).toHaveLength(2);
      expect(state.memories[0].id).toBe('mem-new');
    });

    it('should update memory in place', async () => {
      const original = createMockMemoryEntry({ id: 'mem-1', content: 'Original' });
      useSkillMemoryStore.setState({ memories: [original] });

      const updated = { ...original, content: 'Updated' };
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: updated,
        error: null,
      });

      await useSkillMemoryStore.getState().updateMemory('mem-1', { content: 'Updated' });

      const state = useSkillMemoryStore.getState();
      expect(state.memories[0].content).toBe('Updated');
    });

    it('should delete memory from list', async () => {
      useSkillMemoryStore.setState({
        memories: [
          createMockMemoryEntry({ id: 'mem-1' }),
          createMockMemoryEntry({ id: 'mem-2' }),
        ],
      });

      mockInvoke.mockResolvedValueOnce({ success: true, data: null, error: null });

      await useSkillMemoryStore.getState().deleteMemory('mem-1');

      expect(useSkillMemoryStore.getState().memories).toHaveLength(1);
      expect(useSkillMemoryStore.getState().memories[0].id).toBe('mem-2');
    });

    it('should paginate with loadMoreMemories', async () => {
      useSkillMemoryStore.setState({
        memories: [createMockMemoryEntry({ id: 'mem-1' })],
        memoryPage: 0,
        memoryPageSize: 20,
      });

      const moreMemories = [createMockMemoryEntry({ id: 'mem-21' })];
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: moreMemories,
        error: null,
      });

      await useSkillMemoryStore.getState().loadMoreMemories('/test/project');

      const state = useSkillMemoryStore.getState();
      expect(state.memories).toHaveLength(2);
      expect(state.memoryPage).toBe(1);
    });
  });
});
