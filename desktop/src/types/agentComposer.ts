/**
 * Agent Composer Types
 *
 * TypeScript interfaces matching the Rust types in
 * desktop/src-tauri/src/services/agent_composer/types.rs
 */

/** Configuration for an agent step */
export interface AgentConfig {
  /** Maximum number of agentic loop iterations */
  max_iterations: number;
  /** Maximum total tokens to consume */
  max_total_tokens: number;
  /** Whether to enable streaming output */
  streaming: boolean;
  /** Whether to enable automatic context compaction */
  enable_compaction: boolean;
  /** LLM temperature setting */
  temperature: number | null;
}

/** Default agent configuration */
export const DEFAULT_AGENT_CONFIG: AgentConfig = {
  max_iterations: 50,
  max_total_tokens: 1_000_000,
  streaming: true,
  enable_compaction: true,
  temperature: null,
};

/** Configuration for an LLM agent step */
export interface LlmStepConfig {
  /** Agent name */
  name: string;
  /** Optional system instruction/prompt */
  instruction: string | null;
  /** Optional model override */
  model: string | null;
  /** Optional tool filter */
  tools: string[] | null;
  /** Agent-specific configuration */
  config: AgentConfig;
}

/** A single step in an agent pipeline (discriminated union) */
export type AgentStep =
  | { step_type: 'llm_step'; name: string; instruction: string | null; model: string | null; tools: string[] | null; config: AgentConfig }
  | { step_type: 'sequential_step'; name: string; steps: AgentStep[] }
  | { step_type: 'parallel_step'; name: string; steps: AgentStep[] }
  | { step_type: 'conditional_step'; name: string; condition_key: string; branches: Record<string, AgentStep>; default_branch: AgentStep | null };

/** Serializable definition of an agent pipeline */
export interface AgentPipeline {
  /** Unique pipeline identifier */
  pipeline_id: string;
  /** Human-readable pipeline name */
  name: string;
  /** Optional description */
  description: string | null;
  /** Ordered list of agent steps */
  steps: AgentStep[];
  /** When created (ISO 8601) */
  created_at: string;
  /** When last updated (ISO 8601) */
  updated_at: string | null;
}

/** Summary information about a pipeline (for list views) */
export interface AgentPipelineInfo {
  /** Pipeline identifier */
  pipeline_id: string;
  /** Pipeline name */
  name: string;
  /** Optional description */
  description: string | null;
  /** Number of steps */
  step_count: number;
  /** When created */
  created_at: string;
  /** When last updated */
  updated_at: string | null;
}

/** Events emitted during agent execution (unified enum — story-001) */
export type AgentEvent =
  | { type: 'started'; run_id: string }
  | { type: 'text_delta'; content: string }
  | { type: 'tool_call'; name: string; args: string; id?: string; input?: unknown }
  | { type: 'tool_result'; name: string; result: string; id?: string; is_error?: boolean }
  | { type: 'thinking_delta'; content: string }
  | { type: 'state_update'; key: string; value: unknown }
  | { type: 'agent_transfer'; target: string; message: string }
  | { type: 'graph_node_started'; node_id: string }
  | { type: 'graph_node_completed'; node_id: string; output?: string | null }
  | { type: 'human_review_required'; node_id: string; context: string }
  | { type: 'rich_content'; component_type: string; data: unknown; surface_id?: string }
  | { type: 'actions'; actions: unknown }
  | { type: 'completed'; run_id: string; output: string; duration_ms: number }
  | { type: 'failed'; run_id: string; error: string; duration_ms: number }
  | { type: 'cancelled'; run_id: string; duration_ms: number }
  | { type: 'usage'; input_tokens: number; output_tokens: number }
  | { type: 'done'; output: string | null };

/** Standard Tauri command response wrapper */
export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

/** Helper to create a new LLM step with defaults */
export function createLlmStep(name: string): AgentStep {
  return {
    step_type: 'llm_step',
    name,
    instruction: null,
    model: null,
    tools: null,
    config: { ...DEFAULT_AGENT_CONFIG },
  };
}

/** Helper to create a new empty pipeline */
export function createEmptyPipeline(): AgentPipeline {
  return {
    pipeline_id: '',
    name: 'New Pipeline',
    description: null,
    steps: [],
    created_at: new Date().toISOString(),
    updated_at: null,
  };
}
