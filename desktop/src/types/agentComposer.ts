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

/** Events emitted during agent execution */
export type AgentEvent =
  | { type: 'text_delta'; content: string }
  | { type: 'tool_call'; name: string; args: string }
  | { type: 'tool_result'; name: string; result: string }
  | { type: 'thinking_delta'; content: string }
  | { type: 'state_update'; key: string; value: unknown }
  | { type: 'agent_transfer'; target: string; message: string }
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
