/**
 * Graph Workflow Types
 *
 * TypeScript interfaces matching the Rust types in
 * desktop/src-tauri/src/services/agent_composer/graph_types.rs
 */

import type { AgentStep, AgentConfig } from './agentComposer';

/** A graph-based workflow definition */
export interface GraphWorkflow {
  /** Workflow name */
  name: string;
  /** Optional description */
  description: string | null;
  /** Map of node IDs to graph nodes */
  nodes: Record<string, GraphNode>;
  /** Edges connecting nodes */
  edges: Edge[];
  /** ID of the entry node */
  entry_node: string;
  /** State schema for channels and reducers */
  state_schema: StateSchema;
}

/** A node in a graph workflow */
export interface GraphNode {
  /** Unique node identifier */
  id: string;
  /** The agent step to execute */
  agent_step: AgentStep;
  /** Optional UI position */
  position: NodePosition | null;
}

/** UI position for a graph node */
export interface NodePosition {
  x: number;
  y: number;
}

/** Edge types (discriminated union) */
export type Edge =
  | { edge_type: 'direct'; from: string; to: string }
  | {
      edge_type: 'conditional';
      from: string;
      condition: ConditionConfig;
      branches: Record<string, string>;
      default_branch: string | null;
    };

/** Configuration for a conditional edge */
export interface ConditionConfig {
  condition_key: string;
}

/** State schema for graph execution */
export interface StateSchema {
  channels: Record<string, ChannelConfig>;
  reducers: Record<string, Reducer>;
}

/** Channel configuration */
export interface ChannelConfig {
  channel_type: string;
  default_value: unknown | null;
}

/** Reducer types */
export type Reducer = 'overwrite' | 'append' | 'sum';

/** Summary information about a graph workflow */
export interface GraphWorkflowInfo {
  id: string;
  name: string;
  node_count: number;
  edge_count: number;
}

/** Standard Tauri command response wrapper */
export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

/** Graph workflow execution events */
export type GraphWorkflowEvent =
  | { type: 'graph_node_started'; node_id: string }
  | { type: 'graph_node_completed'; node_id: string; output: string | null }
  | { type: 'human_review_required'; node_id: string; context: string }
  | { type: 'text_delta'; content: string }
  | { type: 'tool_call'; name: string; args: string }
  | { type: 'done'; output: string | null };

/** Helper to create an empty graph workflow */
export function createEmptyGraphWorkflow(): GraphWorkflow {
  return {
    name: 'New Workflow',
    description: null,
    nodes: {},
    edges: [],
    entry_node: '',
    state_schema: { channels: {}, reducers: {} },
  };
}

/** Helper to create a new graph node */
export function createGraphNode(id: string, agentStep: AgentStep): GraphNode {
  return {
    id,
    agent_step: agentStep,
    position: null,
  };
}
