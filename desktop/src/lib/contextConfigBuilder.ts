import type { ContextSelectionSnapshot, ContextSourceConfig } from '../types/contextSources';
import type { MemoryScope } from '../types/skillMemory';

function normalizeMemoryScopes(scopes: MemoryScope[], sessionId: string | null): MemoryScope[] {
  const unique: MemoryScope[] = [];
  for (const scope of scopes) {
    if (!unique.includes(scope)) unique.push(scope);
  }
  const filtered = unique.filter((scope) => scope !== 'session' || !!sessionId?.trim());
  return filtered.length > 0 ? filtered : ['project', 'global'];
}

export function buildContextSourceConfig(selection: ContextSelectionSnapshot, projectId: string): ContextSourceConfig {
  const normalizedSessionId = selection.memory.sessionId?.trim() || null;
  const normalizedScopes = normalizeMemoryScopes(selection.memory.selectedScopes, normalizedSessionId);
  const selectionMode = selection.memory.selectionMode;

  const selectedMemoryIds = selectionMode === 'only_selected' ? selection.memory.includedMemoryIds : [];
  const excludedMemoryIds = selectionMode === 'only_selected' ? [] : selection.memory.excludedMemoryIds;

  const config: ContextSourceConfig = {
    project_id: projectId,
    memory: {
      enabled: selection.memory.enabled,
      selected_categories: selection.memory.selectedCategories,
      selected_memory_ids: selectedMemoryIds,
      excluded_memory_ids: excludedMemoryIds,
      selected_scopes: normalizedScopes,
      session_id: normalizedSessionId,
      statuses: selection.memory.statuses,
      review_mode: selection.memory.reviewMode,
      selection_mode: selectionMode,
    },
  };

  if (selection.knowledge.enabled) {
    config.knowledge = {
      enabled: true,
      selected_collections: selection.knowledge.selectedCollections,
      selected_documents: selection.knowledge.selectedDocuments,
    };
  }

  if (selection.skills.enabled) {
    config.skills = {
      enabled: true,
      selected_skill_ids: selection.skills.selectedSkillIds,
      selection_mode:
        selection.skills.selectedSkillIds.length > 0
          ? 'explicit'
          : selection.skills.selectionMode === 'explicit'
            ? 'explicit'
            : 'auto',
    };
  }

  return config;
}
