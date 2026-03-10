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
export type SkillReviewStatus = 'pending_review' | 'approved' | 'rejected' | 'archived';
export type SkillToolPolicyMode = 'advisory' | 'restrictive';
export type SkillSourceInstallType = 'local' | 'git' | 'url';
export type GeneratedSkillImportConflictPolicy = 'rename' | 'replace' | 'skip';

/** Lightweight skill summary (no body text) */
export interface SkillSummary {
  id: string;
  name: string;
  description: string;
  version: string | null;
  tags: string[];
  tool_policy_mode: SkillToolPolicyMode;
  allowed_tools: string[];
  source: SkillSource;
  priority: number;
  enabled: boolean;
  detected: boolean;
  user_invocable: boolean;
  has_hooks: boolean;
  inject_into: InjectionPhase[];
  path: string;
  review_status?: SkillReviewStatus | null;
  review_notes?: string | null;
  reviewed_at?: string | null;
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
  tool_policy_mode: SkillToolPolicyMode;
  allowed_tools: string[];
  license: string | null;
  metadata: Record<string, string>;
  hooks: SkillHooks | null;
  source: SkillSource;
  priority: number;
  detect: SkillDetection | null;
  inject_into: InjectionPhase[];
  enabled: boolean;
  review_status?: SkillReviewStatus | null;
  review_notes?: string | null;
  reviewed_at?: string | null;
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

export interface SkillSourceInfo {
  name: string;
  source_type: SkillSourceInstallType | string;
  path?: string | null;
  repository?: string | null;
  url?: string | null;
  enabled: boolean;
  installed: boolean;
  skill_count: number;
}

export interface SkillSourceMutationResult {
  source: SkillSourceInfo;
  files_deleted: boolean;
}

export interface NonSelectedSkillDiagnostic {
  skill_id: string;
  skill_name: string;
  reason: string;
  source_type: string;
  path: string;
  review_status?: SkillReviewStatus | null;
}

type SkillSourceLegacyInput =
  | SkillSource
  | SkillSourceLabel
  | { external?: { source_name?: string } }
  | { builtin?: Record<string, never> }
  | { user?: Record<string, never> }
  | { project_local?: Record<string, never> }
  | { generated?: Record<string, never> }
  | null
  | undefined;

// ============================================================================
// Memory Types
// ============================================================================

/** Categories of project memory */
export type MemoryCategory = 'preference' | 'convention' | 'pattern' | 'correction' | 'fact';

/** Scope of memory storage */
export type MemoryScope = 'project' | 'global' | 'session';
export type MemoryStatus = 'active' | 'pending_review' | 'rejected' | 'archived' | 'deleted';
export type MemoryRiskTier = 'low' | 'medium' | 'high';
export type MemoryReviewDecision = 'approve' | 'reject' | 'archive' | 'restore';
export type MemoryPipelinePhase = 'idle' | 'extracting' | 'reviewing' | 'ready' | 'pending' | 'error';
export type MemoryPipelineReviewSource = 'manual_review' | 'auto_llm_review' | 'auto_approve' | 'routing_rule' | null;

/** All valid memory categories */
export const MEMORY_CATEGORIES: MemoryCategory[] = ['preference', 'convention', 'pattern', 'correction', 'fact'];

/** A single memory entry */
export interface MemoryEntry {
  id: string;
  project_path: string;
  scope?: MemoryScope;
  session_id?: string | null;
  category: MemoryCategory;
  content: string;
  keywords: string[];
  importance: number;
  access_count: number;
  source_session_id: string | null;
  source_context: string | null;
  status?: MemoryStatus;
  risk_tier?: MemoryRiskTier;
  conflict_flag?: boolean;
  trace_id?: string | null;
  sessionId?: string | null;
  riskTier?: MemoryRiskTier;
  conflictFlag?: boolean;
  traceId?: string | null;
  created_at: string;
  updated_at: string;
  last_accessed_at: string;
}

/** Memory search result with relevance */
export interface MemorySearchResult {
  entry: MemoryEntry;
  relevance_score: number;
}

export interface MemoryReviewCandidate {
  id: string;
  scope: MemoryScope;
  project_path: string | null;
  session_id: string | null;
  category: MemoryCategory;
  content: string;
  keywords: string[];
  importance: number;
  source_session_id: string | null;
  source_context: string | null;
  status: MemoryStatus;
  risk_tier: MemoryRiskTier;
  conflict_flag: boolean;
  created_at: string;
  updated_at: string;
}

/** Memory statistics */
export interface MemoryStats {
  total_count: number;
  category_counts: Record<string, number>;
  status_counts: Record<string, number>;
  avg_importance: number;
}

/** Result of automatic memory extraction from a session */
export interface MemoryExtractionResult {
  extracted_count: number;
  inserted_count: number;
  merged_count: number;
  skipped_count: number;
}

export interface MemoryPipelineSnapshot {
  rootSessionId: string;
  runtimeSessionId: string | null;
  phase: MemoryPipelinePhase;
  lastRunAt: string | null;
  extractedCount: number;
  approvedCount: number;
  rejectedCount: number;
  pendingCount: number;
  injectedCount: number;
  resolvedScopes: {
    global: number;
    project: number;
    session: number;
  };
  requiresReviewModel: boolean;
  messageKey: string | null;
  traceId: string | null;
  reviewSource: MemoryPipelineReviewSource;
}

export interface MemoryPipelineStatusEvent {
  rootSessionId: string;
  runtimeSessionId: string | null;
  phase: MemoryPipelinePhase;
  counts: {
    extracted: number;
    approved: number;
    rejected: number;
    pending: number;
    injected: number;
    scopes: {
      global: number;
      project: number;
      session: number;
    };
  };
  requiresReviewModel: boolean;
  messageKey?: string | null;
  traceId?: string | null;
  timestamp: string;
  reviewSource?: MemoryPipelineReviewSource;
}

// ============================================================================
// Helper Functions
// ============================================================================

/** Get the display label for a skill source */
export function normalizeSkillSource(source: SkillSourceLegacyInput): SkillSource {
  if (!source) {
    return { type: 'builtin' };
  }

  if (typeof source === 'string') {
    switch (source) {
      case 'external':
        return { type: 'external', source_name: '' };
      case 'user':
      case 'project_local':
      case 'generated':
      case 'builtin':
        return { type: source };
      default:
        return { type: 'builtin' };
    }
  }

  if ('type' in source && typeof source.type === 'string') {
    if (source.type === 'external') {
      return { type: 'external', source_name: 'source_name' in source ? (source.source_name ?? '') : '' };
    }
    return source as SkillSource;
  }

  if ('external' in source) {
    return { type: 'external', source_name: source.external?.source_name ?? '' };
  }
  if ('user' in source) return { type: 'user' };
  if ('project_local' in source) return { type: 'project_local' };
  if ('generated' in source) return { type: 'generated' };
  if ('builtin' in source) return { type: 'builtin' };

  return { type: 'builtin' };
}

export function normalizeSkillSummary(summary: SkillSummary): SkillSummary {
  return {
    ...summary,
    tool_policy_mode: summary.tool_policy_mode ?? 'advisory',
    source: normalizeSkillSource(summary.source),
  };
}

export function normalizeSkillDocument(document: SkillDocument): SkillDocument {
  return {
    ...document,
    tool_policy_mode: document.tool_policy_mode ?? 'advisory',
    source: normalizeSkillSource(document.source),
  };
}

export function getSkillSourceLabel(source: SkillSourceLegacyInput): SkillSourceLabel {
  return normalizeSkillSource(source).type;
}

/** Get a human-readable name for a skill source */
export function getSkillSourceDisplayName(source: SkillSourceLegacyInput): string {
  const normalized = normalizeSkillSource(source);
  switch (normalized.type) {
    case 'builtin':
      return 'Built-in';
    case 'external':
      return normalized.source_name || 'External';
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
