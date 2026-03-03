import type { ScopedDocumentRef } from '../lib/knowledgeApi';
import type { MemoryScope } from './skillMemory';

export type MemorySelectionMode = 'auto_exclude' | 'only_selected';
export type SkillSelectionMode = 'auto' | 'explicit';

/** Configuration sent to the backend for conditional context injection. */
export interface ContextSourceConfig {
  /** Project ID for knowledge base queries (e.g. "default" or a UUID). */
  project_id: string;
  knowledge?: {
    enabled: boolean;
    selected_collections: string[];
    selected_documents: ScopedDocumentRef[];
  };
  memory?: {
    enabled: boolean;
    selected_categories: string[];
    selected_memory_ids: string[];
    excluded_memory_ids: string[];
    selected_scopes: MemoryScope[];
    session_id?: string | null;
    statuses?: string[];
    review_mode?: 'active_only' | 'include_pending_review';
    /**
     * Optional v2 selector mode.
     * - auto_exclude: auto-retrieve memories and apply excluded_memory_ids
     * - only_selected: inject only selected_memory_ids
     */
    selection_mode?: MemorySelectionMode;
  };
  skills?: {
    enabled: boolean;
    selected_skill_ids: string[];
    selection_mode: SkillSelectionMode;
  };
}

export interface ContextSelectionKnowledge {
  enabled: boolean;
  selectedCollections: string[];
  selectedDocuments: ScopedDocumentRef[];
}

export interface ContextSelectionMemory {
  enabled: boolean;
  selectionMode: MemorySelectionMode;
  selectedScopes: MemoryScope[];
  sessionId: string | null;
  selectedCategories: string[];
  selectedMemoryIds: string[];
  includedMemoryIds: string[];
  excludedMemoryIds: string[];
  statuses: string[];
  reviewMode: 'active_only' | 'include_pending_review';
}

export interface ContextSelectionSkills {
  enabled: boolean;
  selectedSkillIds: string[];
  selectionMode: SkillSelectionMode;
}

export interface ContextSelectionSessionBinding {
  activeSessionId: string | null;
  source: 'claude' | 'standalone' | 'external' | 'none';
  updatedAt: string | null;
}

export interface ContextSelectionUiMeta {
  stateSource: 'legacy' | 'unified';
  lastSyncedAt: string | null;
  mismatchCount: number;
  buildCount: number;
  dailyStats: Array<{
    date: string;
    buildCount: number;
    mismatchCount: number;
  }>;
}

export interface ContextSelectionSnapshot {
  knowledge: ContextSelectionKnowledge;
  memory: ContextSelectionMemory;
  skills: ContextSelectionSkills;
  sessionBinding: ContextSelectionSessionBinding;
  uiMeta: ContextSelectionUiMeta;
}

export interface LegacyContextSelectionInput {
  knowledgeEnabled: boolean;
  selectedCollections: string[];
  selectedDocuments: ScopedDocumentRef[];
  memoryEnabled: boolean;
  memorySelectionMode: 'auto' | 'only_selected';
  selectedMemoryScopes: MemoryScope[];
  memorySessionId: string | null;
  selectedMemoryCategories: string[];
  selectedMemoryIds: string[];
  includedMemoryIds: string[];
  excludedMemoryIds: string[];
  skillsEnabled: boolean;
  selectedSkillIds: string[];
  skillSelectionMode: SkillSelectionMode;
}
