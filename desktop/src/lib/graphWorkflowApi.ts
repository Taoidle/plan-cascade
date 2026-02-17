/**
 * Graph Workflow API
 *
 * TypeScript wrapper for the graph_workflow Tauri commands.
 * Provides typed access to workflow CRUD operations.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  GraphWorkflow,
  GraphWorkflowInfo,
  CommandResponse,
} from '../types/graphWorkflow';

/**
 * List all saved graph workflows (summary info only).
 */
export async function listGraphWorkflows(): Promise<GraphWorkflowInfo[]> {
  const response = await invoke<CommandResponse<GraphWorkflowInfo[]>>('list_graph_workflows');
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to list graph workflows');
}

/**
 * Get a single graph workflow by ID.
 */
export async function getGraphWorkflow(id: string): Promise<GraphWorkflow | null> {
  const response = await invoke<CommandResponse<GraphWorkflow | null>>('get_graph_workflow', { id });
  if (response.success) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to get graph workflow');
}

/**
 * Create a new graph workflow.
 */
export async function createGraphWorkflow(workflow: GraphWorkflow): Promise<GraphWorkflow> {
  const response = await invoke<CommandResponse<GraphWorkflow>>('create_graph_workflow', { workflow });
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to create graph workflow');
}

/**
 * Update an existing graph workflow.
 */
export async function updateGraphWorkflow(id: string, workflow: GraphWorkflow): Promise<GraphWorkflow> {
  const response = await invoke<CommandResponse<GraphWorkflow>>('update_graph_workflow', { id, workflow });
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to update graph workflow');
}

/**
 * Delete a graph workflow.
 */
export async function deleteGraphWorkflow(id: string): Promise<boolean> {
  const response = await invoke<CommandResponse<boolean>>('delete_graph_workflow', { id });
  if (response.success) {
    return response.data ?? false;
  }
  throw new Error(response.error ?? 'Failed to delete graph workflow');
}
