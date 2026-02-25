/**
 * Skill & Memory Types
 *
 * TypeScript types mirroring the Rust backend models for skills and memories.
 * Used by the Zustand store and UI components.
 */

// ============================================================================
// Skill Types
// ============================================================================

/** Source tier for a skill */
export type SkillSource =
  | { type: 'builtin' }
  | { type: 'external'; source_name: string }
  | { type: 'user' }
  | { type: 'project_local' }
  | { type: 'generated' };

/** Convenience label for display */
export type SkillSourceLabel = 'builtin' | 'external' | 'user' | 'project_local' | 'generated';

/** Phase in which a skill is injected */
export type InjectionPhase = 'planning' | 'implementation' | 'retry' | 'always';

/** Lightweight skill summary (no body text) */
export interface SkillSummary {
  id: string;
  name: string;
  description: string;
  version: string | null;
  tags: string[];
  source: SkillSource;
  priority: number;
  enabled: boolean;
  detected: boolean;
  user_invocable: boolean;
  has_hooks: boolean;
  inject_into: InjectionPhase[];
  path: string;
}

/** Full skill document (includes body) */
export interface SkillDocument {
  id: string;
  name: string;
  description: string;
  version: string | null;
  tags: string[];
  body: string;
  path: string;
  hash: string;
  last_modified: number | null;
  user_invocable: boolean;
  allowed_tools: string[];
  license: string | null;
  metadata: Record<string, string>;
  hooks: SkillHooks | null;
  source: SkillSource;
  priority: number;
  detect: SkillDetection | null;
  inject_into: InjectionPhase[];
  enabled: boolean;
}

export interface SkillHooks {
  pre_tool_use: ToolHookRule[];
  post_tool_use: ToolHookRule[];
  stop: HookAction[];
}

export interface ToolHookRule {
  matcher: string;
  hooks: HookAction[];
}

export interface HookAction {
  hook_type: string;
  command: string;
}

export interface SkillDetection {
  files: string[];
  patterns: string[];
}

/** Matched skill with relevance info */
export interface SkillMatch {
  score: number;
  match_reason: MatchReason;
  skill: SkillSummary;
}

export type MatchReason =
  | { type: 'auto_detected' }
  | { type: 'lexical_match'; query: string }
  | { type: 'user_forced' };

/** Skill index statistics */
export interface SkillIndexStats {
  total: number;
  builtin_count: number;
  external_count: number;
  user_count: number;
  project_local_count: number;
  generated_count: number;
  enabled_count: number;
  detected_count: number;
}

/** Overview returned by get_skills_overview */
export interface SkillsOverview {
  stats: SkillIndexStats;
  detected_skills: SkillSummary[];
  sources: string[];
}

// ============================================================================
// Memory Types
// ============================================================================

/** Categories of project memory */
export type MemoryCategory = 'preference' | 'convention' | 'pattern' | 'correction' | 'fact';

/** All valid memory categories */
export const MEMORY_CATEGORIES: MemoryCategory[] = ['preference', 'convention', 'pattern', 'correction', 'fact'];

/** A single memory entry */
export interface MemoryEntry {
  id: string;
  project_path: string;
  category: MemoryCategory;
  content: string;
  keywords: string[];
  importance: number;
  access_count: number;
  source_session_id: string | null;
  source_context: string | null;
  created_at: string;
  updated_at: string;
  last_accessed_at: string;
}

/** Memory search result with relevance */
export interface MemorySearchResult {
  entry: MemoryEntry;
  relevance_score: number;
}

/** Memory statistics */
export interface MemoryStats {
  total_count: number;
  category_counts: Record<string, number>;
  avg_importance: number;
}

/** Result of automatic memory extraction from a session */
export interface MemoryExtractionResult {
  extracted_count: number;
  inserted_count: number;
  merged_count: number;
  skipped_count: number;
}

// ============================================================================
// Helper Functions
// ============================================================================

/** Get the display label for a skill source */
export function getSkillSourceLabel(source: SkillSource): SkillSourceLabel {
  if ('type' in source) {
    return source.type as SkillSourceLabel;
  }
  return 'builtin';
}

/** Get a human-readable name for a skill source */
export function getSkillSourceDisplayName(source: SkillSource): string {
  switch (source.type) {
    case 'builtin':
      return 'Built-in';
    case 'external':
      return source.source_name || 'External';
    case 'user':
      return 'User';
    case 'project_local':
      return 'Project';
    case 'generated':
      return 'Generated';
    default:
      return 'Unknown';
  }
}

/** Get a human-readable name for a memory category */
export function getMemoryCategoryDisplayName(category: MemoryCategory): string {
  switch (category) {
    case 'preference':
      return 'Preference';
    case 'convention':
      return 'Convention';
    case 'pattern':
      return 'Pattern';
    case 'correction':
      return 'Correction';
    case 'fact':
      return 'Fact';
    default:
      return 'Unknown';
  }
}
