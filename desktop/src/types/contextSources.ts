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
