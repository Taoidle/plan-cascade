import { describe, expect, it } from 'vitest';
import { buildContextSourceConfig } from './contextConfigBuilder';
import type { ContextSelectionSnapshot } from '../types/contextSources';

type SelectionOverrides = {
  knowledge?: Partial<ContextSelectionSnapshot['knowledge']>;
  memory?: Partial<ContextSelectionSnapshot['memory']>;
  skills?: Partial<ContextSelectionSnapshot['skills']>;
  sessionBinding?: Partial<ContextSelectionSnapshot['sessionBinding']>;
  uiMeta?: Partial<ContextSelectionSnapshot['uiMeta']>;
};

function makeSelection(overrides?: SelectionOverrides): ContextSelectionSnapshot {
  return {
    knowledge: {
      enabled: false,
      selectedCollections: [],
      selectedDocuments: [],
      ...(overrides?.knowledge || {}),
    },
    memory: {
      enabled: true,
      selectionMode: 'auto_exclude',
      selectedScopes: ['project', 'global'],
      sessionId: null,
      selectedCategories: [],
      selectedMemoryIds: [],
      includedMemoryIds: [],
      excludedMemoryIds: [],
      statuses: [],
      reviewMode: 'active_only',
      ...(overrides?.memory || {}),
    },
    skills: {
      enabled: false,
      selectedSkillIds: [],
      selectionMode: 'auto',
      ...(overrides?.skills || {}),
    },
    sessionBinding: {
      activeSessionId: null,
      source: 'none',
      updatedAt: null,
      ...(overrides?.sessionBinding || {}),
    },
    uiMeta: {
      stateSource: 'unified',
      lastSyncedAt: null,
      mismatchCount: 0,
      buildCount: 0,
      dailyStats: [],
      ...(overrides?.uiMeta || {}),
    },
  };
}

describe('buildContextSourceConfig', () => {
  it('maps memory auto_exclude mode to excluded_memory_ids', () => {
    const selection = makeSelection({
      memory: {
        selectionMode: 'auto_exclude',
        excludedMemoryIds: ['m-1', 'm-2'],
        includedMemoryIds: ['m-included-ignored'],
      },
    });
    const config = buildContextSourceConfig(selection, 'project-a');

    expect(config.project_id).toBe('project-a');
    expect(config.memory?.selection_mode).toBe('auto_exclude');
    expect(config.memory?.selected_memory_ids).toEqual([]);
    expect(config.memory?.excluded_memory_ids).toEqual(['m-1', 'm-2']);
  });

  it('maps memory only_selected mode to selected_memory_ids', () => {
    const selection = makeSelection({
      memory: {
        selectionMode: 'only_selected',
        includedMemoryIds: ['m-3', 'm-4'],
        excludedMemoryIds: ['m-excluded-ignored'],
      },
    });
    const config = buildContextSourceConfig(selection, 'project-b');

    expect(config.memory?.selection_mode).toBe('only_selected');
    expect(config.memory?.selected_memory_ids).toEqual(['m-3', 'm-4']);
    expect(config.memory?.excluded_memory_ids).toEqual([]);
  });

  it('maps skills with selected ids to explicit selection_mode', () => {
    const selection = makeSelection({
      skills: {
        enabled: true,
        selectionMode: 'auto',
        selectedSkillIds: ['skill-1'],
      },
    });
    const config = buildContextSourceConfig(selection, 'project-c');
    expect(config.skills?.selection_mode).toBe('explicit');
    expect(config.skills?.selected_skill_ids).toEqual(['skill-1']);
  });

  it('keeps auto skills mode when no explicit ids are provided', () => {
    const selection = makeSelection({
      skills: {
        enabled: true,
        selectionMode: 'auto',
        selectedSkillIds: [],
      },
    });
    const config = buildContextSourceConfig(selection, 'project-d');
    expect(config.skills?.selection_mode).toBe('auto');
    expect(config.skills?.selected_skill_ids).toEqual([]);
  });
});
