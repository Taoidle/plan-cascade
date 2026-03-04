/**
 * Context v2 API (IPC wrappers)
 *
 * Thin wrappers around `commands/context_v2.rs` for inspector/trace/artifact
 * and operations dashboards.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';
import type { ContextSourceConfig } from '../store/contextSources';

export interface ContextConversationTurn {
  role: string;
  content: string;
}

export interface ContextBudget {
  input_token_budget: number;
  reserved_output_tokens: number;
  hard_limit: number;
  used_input_tokens: number;
  over_budget: boolean;
}

export interface ContextSourceRef {
  id: string;
  kind: 'history' | 'memory' | 'knowledge' | 'rules' | 'skills' | 'manual';
  label: string;
  token_cost: number;
  included: boolean;
  reason: string;
}

export interface ContextBlock {
  source_id: string;
  title: string;
  content: string;
  token_cost: number;
  priority: number;
  reason: string;
  anchor: boolean;
}

export interface CompactionReport {
  triggered: boolean;
  trigger_reason: string;
  strategy: string;
  before_tokens: number;
  after_tokens: number;
  compaction_tokens: number;
  net_saving: number;
  quality_score: number;
  compaction_actions?: Array<{
    stage: string;
    action: string;
    source_id: string;
    before_tokens: number;
    after_tokens: number;
    reason: string;
  }>;
  quality_basis?: Record<string, unknown>;
}

export interface ContextDiagnostics {
  blocked_tools: string[];
  effective_statuses: string[];
  selected_skills: string[];
  effective_skill_ids: string[];
  effective_memory_ids: string[];
  memory_candidates_count: number;
  degraded_reason?: string | null;
  selection_reason: string;
  selection_origin?: 'auto' | 'explicit' | 'mixed' | null;
}

export interface ContextEnvelope {
  request_meta: {
    turn_id: string;
    session_id?: string | null;
    mode: string;
    query: string;
    intent?: string | null;
  };
  budget: ContextBudget;
  sources: ContextSourceRef[];
  blocks: ContextBlock[];
  compaction: CompactionReport;
  trace_id: string;
  assembled_prompt: string;
  diagnostics?: ContextDiagnostics;
}

export interface ContextAssemblyResponse {
  request_meta: {
    turn_id: string;
    session_id?: string | null;
    mode: string;
    query: string;
    intent?: string | null;
  };
  assembled_prompt: string;
  trace_id: string;
  budget: ContextBudget;
  sources: ContextSourceRef[];
  blocks: ContextBlock[];
  compaction: CompactionReport;
  injected_source_kinds: string[];
  fallback_used: boolean;
  fallback_reason?: string | null;
  diagnostics?: ContextDiagnostics;
}

export interface ContextTraceEvent {
  trace_id: string;
  event_type: string;
  source_kind?: string | null;
  source_id?: string | null;
  message: string;
  metadata?: Record<string, unknown> | null;
  created_at: string;
}

export interface ContextTrace {
  trace_id: string;
  events: ContextTraceEvent[];
}

export interface ContextPolicy {
  context_v2_pipeline: boolean;
  memory_v2_ranker: boolean;
  context_inspector_ui: boolean;
  pinned_sources: string[];
  excluded_sources: string[];
  soft_threshold_ratio: number;
  hard_threshold_ratio: number;
}

export interface ContextRolloutConfig {
  enabled: boolean;
  rollout_percentage: number;
  ab_mode: 'off' | 'shadow' | 'split' | string;
  experiment_key?: string | null;
  chaos_enabled: boolean;
  chaos_probability: number;
}

export interface ContextArtifactMeta {
  id: string;
  name: string;
  project_path: string;
  session_id?: string | null;
  created_at: string;
  updated_at: string;
}

export interface ContextOpsAlert {
  code: string;
  severity: string;
  message: string;
  value: number;
  threshold: number;
}

export interface ContextOpsVariantStat {
  variant: string;
  traces: number;
  degraded_rate: number;
  avg_latency_ms: number;
}

export interface ContextChaosRunMeta {
  run_id: string;
  project_path: string;
  session_id?: string | null;
  created_at: string;
  iterations: number;
  fallback_success_rate: number;
}

export interface ContextOpsDashboard {
  project_path: string;
  window_start: string;
  window_end: string;
  window_hours: number;
  total_traces: number;
  assembled_traces: number;
  availability: number;
  degraded_traces: number;
  degraded_rate: number;
  source_failure_traces: number;
  prepare_context_p50_ms: number;
  prepare_context_p95_ms: number;
  memory_query_p95_ms: number;
  empty_hit_rate: number;
  candidate_count: number;
  review_backlog: number;
  approve_rate: number;
  reject_rate: number;
  total_compaction_saving_tokens: number;
  avg_compaction_saving_tokens: number;
  ab_variants: ContextOpsVariantStat[];
  alerts: ContextOpsAlert[];
  policy: ContextPolicy;
  rollout: ContextRolloutConfig;
  recent_chaos_runs: ContextChaosRunMeta[];
  runbook_path: string;
}

export interface ContextChaosScenarioResult {
  scenario: string;
  injected: boolean;
  fallback_ok: boolean;
  warning_emitted: boolean;
}

export interface ContextChaosProbeReport {
  run_id: string;
  project_path: string;
  session_id?: string | null;
  created_at: string;
  iterations: number;
  failure_probability: number;
  injected_faults: number;
  fallback_success_rate: number;
  scenarios: ContextChaosScenarioResult[];
  recommendation: string;
}

function errorResponse<T>(error: unknown): CommandResponse<T> {
  return {
    success: false,
    data: null,
    error: error instanceof Error ? error.message : String(error),
  };
}

/**
 * @deprecated Use `assembleTurnContext` on all Simple/Task/Plan main paths.
 * This mapper remains only for backward compatibility.
 */
export async function prepareTurnContextV2(request: {
  project_path: string;
  query: string;
  session_id?: string;
  mode?: string;
  intent?: string;
  conversation_history?: ContextConversationTurn[];
  context_sources?: ContextSourceConfig;
}): Promise<CommandResponse<ContextEnvelope>> {
  try {
    const assembled = await assembleTurnContext(request);
    if (!assembled.success || !assembled.data) {
      return {
        success: false,
        data: null,
        error: assembled.error || 'assemble_turn_context returned no data',
      };
    }

    return {
      success: true,
      data: {
        request_meta: assembled.data.request_meta,
        assembled_prompt: assembled.data.assembled_prompt,
        trace_id: assembled.data.trace_id,
        budget: assembled.data.budget,
        sources: assembled.data.sources,
        blocks: assembled.data.blocks,
        compaction: assembled.data.compaction,
        diagnostics: assembled.data.diagnostics,
      },
      error: null,
    };
  } catch (error) {
    return errorResponse(error);
  }
}

export async function assembleTurnContext(request: {
  project_path: string;
  query: string;
  project_id?: string;
  session_id?: string;
  mode?: string;
  intent?: string;
  phase?: string;
  conversation_history?: ContextConversationTurn[];
  context_sources?: ContextSourceConfig;
  manual_blocks?: Array<{
    id?: string;
    title?: string;
    content: string;
    priority?: number;
  }>;
  input_token_budget?: number;
  reserved_output_tokens?: number;
  hard_limit?: number;
  enforce_user_skill_selection?: boolean;
}): Promise<CommandResponse<ContextAssemblyResponse>> {
  try {
    return await invoke<CommandResponse<ContextAssemblyResponse>>('assemble_turn_context', { request });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function getContextTrace(traceId: string): Promise<CommandResponse<ContextTrace>> {
  try {
    return await invoke<CommandResponse<ContextTrace>>('get_context_trace', { traceId });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function getContextPolicy(): Promise<CommandResponse<ContextPolicy>> {
  try {
    return await invoke<CommandResponse<ContextPolicy>>('get_context_policy');
  } catch (error) {
    return errorResponse(error);
  }
}

export async function setContextPolicy(
  policy: ContextPolicy,
): Promise<CommandResponse<{ key: string; updated_at: string }>> {
  try {
    return await invoke<CommandResponse<{ key: string; updated_at: string }>>('set_context_policy', { policy });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function getContextRollout(): Promise<CommandResponse<ContextRolloutConfig>> {
  try {
    return await invoke<CommandResponse<ContextRolloutConfig>>('get_context_rollout');
  } catch (error) {
    return errorResponse(error);
  }
}

export async function setContextRollout(
  config: ContextRolloutConfig,
): Promise<CommandResponse<{ key: string; updated_at: string }>> {
  try {
    return await invoke<CommandResponse<{ key: string; updated_at: string }>>('set_context_rollout', { config });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function saveContextArtifact(input: {
  name: string;
  project_path: string;
  session_id?: string | null;
  envelope: ContextEnvelope;
}): Promise<CommandResponse<ContextArtifactMeta>> {
  try {
    return await invoke<CommandResponse<ContextArtifactMeta>>('save_context_artifact', { input });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function listContextArtifacts(params: {
  project_path: string;
  session_id?: string | null;
  limit?: number;
}): Promise<CommandResponse<ContextArtifactMeta[]>> {
  try {
    return await invoke<CommandResponse<ContextArtifactMeta[]>>('list_context_artifacts', {
      projectPath: params.project_path,
      sessionId: params.session_id ?? null,
      limit: params.limit ?? null,
    });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function applyContextArtifact(
  artifactId: string,
  sessionId?: string | null,
): Promise<
  CommandResponse<{ artifact_id: string; session_id?: string | null; applied: boolean; envelope: ContextEnvelope }>
> {
  try {
    return await invoke<
      CommandResponse<{ artifact_id: string; session_id?: string | null; applied: boolean; envelope: ContextEnvelope }>
    >('apply_context_artifact', {
      artifactId,
      sessionId: sessionId ?? null,
    });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function deleteContextArtifact(artifactId: string): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('delete_context_artifact', { artifactId });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function getContextOpsDashboard(
  projectPath: string,
  windowHours?: number,
): Promise<CommandResponse<ContextOpsDashboard>> {
  try {
    return await invoke<CommandResponse<ContextOpsDashboard>>('get_context_ops_dashboard', {
      request: {
        project_path: projectPath,
        window_hours: windowHours ?? null,
      },
    });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function runContextChaosProbe(request: {
  project_path: string;
  session_id?: string | null;
  iterations?: number;
  failure_probability?: number;
}): Promise<CommandResponse<ContextChaosProbeReport>> {
  try {
    return await invoke<CommandResponse<ContextChaosProbeReport>>('run_context_chaos_probe', { request });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function listContextChaosRuns(
  projectPath: string,
  limit?: number,
): Promise<CommandResponse<ContextChaosRunMeta[]>> {
  try {
    return await invoke<CommandResponse<ContextChaosRunMeta[]>>('list_context_chaos_runs', {
      projectPath,
      limit: limit ?? null,
    });
  } catch (error) {
    return errorResponse(error);
  }
}
