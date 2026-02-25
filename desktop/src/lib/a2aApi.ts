/**
 * A2A Remote Agent API
 *
 * TypeScript wrapper for the A2A Tauri commands.
 * Provides typed access to remote agent discovery, registration, listing, and removal.
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

/** Agent card metadata from a remote A2A agent */
export interface AgentCard {
  name: string;
  description: string;
  capabilities: string[];
  endpoint: string;
  version: string;
  auth_required: boolean;
  supported_inputs: string[];
}

/** Result of discovering a remote agent */
export interface DiscoveredAgent {
  base_url: string;
  agent_card: AgentCard;
}

/** A registered remote agent persisted in the database */
export interface RegisteredRemoteAgent {
  id: string;
  base_url: string;
  name: string;
  description: string;
  capabilities: string[];
  endpoint: string;
  version: string;
  auth_required: boolean;
  supported_inputs: string[];
  created_at: string | null;
  updated_at: string | null;
}

/** Standard Tauri command response */
interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

// ============================================================================
// API Functions
// ============================================================================

/**
 * Discover a remote A2A agent at the given base URL.
 *
 * Fetches the agent card from `{baseUrl}/.well-known/agent.json` and validates it.
 * Does NOT register the agent -- call `registerA2aAgent` for that.
 */
export async function discoverA2aAgent(baseUrl: string): Promise<DiscoveredAgent> {
  const response = await invoke<CommandResponse<DiscoveredAgent>>('discover_a2a_agent', {
    baseUrl,
  });
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to discover remote agent');
}

/**
 * List all registered remote A2A agents.
 */
export async function listA2aAgents(): Promise<RegisteredRemoteAgent[]> {
  const response = await invoke<CommandResponse<RegisteredRemoteAgent[]>>('list_a2a_agents');
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to list remote agents');
}

/**
 * Register a remote A2A agent for use in pipelines.
 *
 * If an agent with the same URL already exists, it is updated.
 */
export async function registerA2aAgent(baseUrl: string, agentCard: AgentCard): Promise<RegisteredRemoteAgent> {
  const response = await invoke<CommandResponse<RegisteredRemoteAgent>>('register_a2a_agent', {
    baseUrl,
    agentCard,
  });
  if (response.success && response.data) {
    return response.data;
  }
  throw new Error(response.error ?? 'Failed to register remote agent');
}

/**
 * Remove (unregister) a remote A2A agent by its ID.
 */
export async function removeA2aAgent(id: string): Promise<boolean> {
  const response = await invoke<CommandResponse<boolean>>('remove_a2a_agent', { id });
  if (response.success) {
    return response.data ?? false;
  }
  throw new Error(response.error ?? 'Failed to remove remote agent');
}
