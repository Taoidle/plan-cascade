/**
 * Agent Composer API
 *
 * TypeScript wrapper for the agent_composer Tauri commands.
 * Provides typed access to pipeline CRUD operations.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  AgentPipeline,
  AgentPipelineInfo,
  CommandResponse,
} from '../types/agentComposer';

/**
 * List all saved agent pipelines (summary info only).
 */
export async function listAgentPipelines(): Promise<AgentPipelineInfo[]> {
  const response = await invoke<CommandResponse<AgentPipelineInfo[]>>('list_agent_pipelines');
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to list agent pipelines');
}

/**
 * Get a single agent pipeline by ID.
 */
export async function getAgentPipeline(id: string): Promise<AgentPipeline | null> {
  const response = await invoke<CommandResponse<AgentPipeline | null>>('get_agent_pipeline', { id });
  if (response.success) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to get agent pipeline');
}

/**
 * Create a new agent pipeline.
 */
export async function createAgentPipeline(pipeline: AgentPipeline): Promise<AgentPipeline> {
  const response = await invoke<CommandResponse<AgentPipeline>>('create_agent_pipeline', { pipeline });
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to create agent pipeline');
}

/**
 * Update an existing agent pipeline.
 */
export async function updateAgentPipeline(id: string, pipeline: AgentPipeline): Promise<AgentPipeline> {
  const response = await invoke<CommandResponse<AgentPipeline>>('update_agent_pipeline', { id, pipeline });
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to update agent pipeline');
}

/**
 * Delete an agent pipeline.
 */
export async function deleteAgentPipeline(id: string): Promise<boolean> {
  const response = await invoke<CommandResponse<boolean>>('delete_agent_pipeline', { id });
  if (response.success) {
    return response.data ?? false;
  }
  throw new Error(response.error ?? 'Failed to delete agent pipeline');
}
