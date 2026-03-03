import { useContextSelectionStore } from '../store/contextSelection';
import { useContextSourcesStore } from '../store/contextSources';
import { useProjectsStore } from '../store/projects';
import { useSettingsStore } from '../store/settings';
import { buildContextSourceConfig } from './contextConfigBuilder';
import type { ContextSelectionSnapshot, ContextSourceConfig } from '../types/contextSources';

type SessionSource = 'claude' | 'standalone';

function normalizeMemoryScopes(
  scopes: ContextSelectionSnapshot['memory']['selectedScopes'],
  sessionId: string | null,
): ContextSelectionSnapshot['memory']['selectedScopes'] {
  const unique = Array.from(new Set(scopes));
  const filtered = unique.filter((scope) => scope !== 'session' || !!sessionId?.trim());
  return filtered.length > 0
    ? (filtered as ContextSelectionSnapshot['memory']['selectedScopes'])
    : (['project', 'global'] as ContextSelectionSnapshot['memory']['selectedScopes']);
}

function mapLegacyStateToSelection(): ContextSelectionSnapshot {
  const legacy = useContextSourcesStore.getState();
  const sessionId = legacy.memorySessionId?.trim() || null;
  const selectionMode = legacy.memorySelectionMode === 'only_selected' ? 'only_selected' : 'auto_exclude';
  const compatExcluded = legacy.excludedMemoryIds.length > 0 ? legacy.excludedMemoryIds : legacy.selectedMemoryIds;

  return {
    knowledge: {
      enabled: legacy.knowledgeEnabled,
      selectedCollections: legacy.selectedCollections,
      selectedDocuments: legacy.selectedDocuments,
    },
    memory: {
      enabled: legacy.memoryEnabled,
      selectionMode,
      selectedScopes: normalizeMemoryScopes(legacy.selectedMemoryScopes, sessionId),
      sessionId,
      selectedCategories: legacy.selectedMemoryCategories,
      selectedMemoryIds: selectionMode === 'only_selected' ? legacy.includedMemoryIds : compatExcluded,
      includedMemoryIds: legacy.includedMemoryIds,
      excludedMemoryIds: compatExcluded,
      statuses: [],
      reviewMode: 'active_only',
    },
    skills: {
      enabled: legacy.skillsEnabled,
      selectedSkillIds: legacy.selectedSkillIds,
      selectionMode: legacy.selectedSkillIds.length > 0 ? 'explicit' : legacy.skillSelectionMode,
    },
    sessionBinding: {
      activeSessionId: sessionId,
      source: sessionId?.startsWith('standalone:')
        ? 'standalone'
        : sessionId?.startsWith('claude:')
          ? 'claude'
          : 'none',
      updatedAt: null,
    },
    uiMeta: {
      stateSource: 'legacy',
      lastSyncedAt: null,
      mismatchCount: 0,
      buildCount: 0,
      dailyStats: [],
    },
  };
}

export function isUnifiedContextSelectionEnabled(): boolean {
  return useSettingsStore.getState().simpleContextUnifiedStore;
}

export function getContextSelectionSource(): 'legacy' | 'unified' {
  return isUnifiedContextSelectionEnabled() ? 'unified' : 'legacy';
}

export function getEffectiveContextSelectionSnapshot(): ContextSelectionSnapshot {
  if (!isUnifiedContextSelectionEnabled()) {
    return mapLegacyStateToSelection();
  }
  return useContextSelectionStore.getState().getEffectiveSelection();
}

export function resolveSessionScopedContext(
  sessionId: string | null,
  source: SessionSource,
): ContextSourceConfig | null {
  const projectId = useProjectsStore.getState().selectedProject?.id ?? 'default';
  const scopedSessionId = sessionId?.trim() ? `${source}:${sessionId.trim()}` : null;

  if (!isUnifiedContextSelectionEnabled()) {
    const legacy = useContextSourcesStore.getState();
    legacy.setMemorySessionId(scopedSessionId);
    return legacy.buildConfig() ?? null;
  }

  useContextSourcesStore.getState().setMemorySessionId(scopedSessionId);
  useContextSelectionStore.getState().patchSessionBinding(scopedSessionId, source);

  const selection = useContextSelectionStore.getState().getEffectiveSelection();
  return buildContextSourceConfig(selection, projectId);
}
