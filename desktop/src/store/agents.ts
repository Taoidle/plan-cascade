/**
 * Agents Store
 *
 * Manages agent state for the Agents Library.
 * Uses Zustand for state management with Tauri command integration.
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  Agent,
  AgentWithStats,
  AgentCreateRequest,
  AgentUpdateRequest,
  AgentRun,
  AgentRunList,
  AgentStats,
  CommandResponse,
} from '../types/agent';

interface AgentsState {
  /** List of all agents with their stats */
  agents: AgentWithStats[];

  /** Currently selected agent */
  selectedAgent: Agent | null;

  /** Run history for selected agent */
  runHistory: AgentRunList | null;

  /** Stats for selected agent */
  selectedAgentStats: AgentStats | null;

  /** Search query for filtering agents */
  searchQuery: string;

  /** UI panel state */
  panelOpen: boolean;

  /** Dialog open state */
  dialogOpen: boolean;

  /** Agent being edited in dialog */
  editingAgent: Agent | null;

  /** Active agent for the current session */
  activeAgentForSession: Agent | null;

  /** Loading states */
  loading: {
    agents: boolean;
    agent: boolean;
    history: boolean;
    stats: boolean;
    creating: boolean;
    updating: boolean;
    deleting: boolean;
    running: boolean;
  };

  /** Error message */
  error: string | null;

  /** Actions */
  fetchAgents: () => Promise<void>;
  fetchAgent: (id: string) => Promise<Agent | null>;
  createAgent: (request: AgentCreateRequest) => Promise<Agent | null>;
  updateAgent: (id: string, request: AgentUpdateRequest) => Promise<Agent | null>;
  deleteAgent: (id: string) => Promise<boolean>;
  selectAgent: (agent: Agent | null) => void;
  fetchRunHistory: (agentId: string, limit?: number, offset?: number) => Promise<void>;
  fetchAgentStats: (agentId: string) => Promise<void>;
  runAgent: (agentId: string, input: string) => Promise<AgentRun | null>;
  exportAgents: (agentIds?: string[]) => Promise<string | null>;
  importAgents: (json: string) => Promise<Agent[] | null>;
  setSearchQuery: (query: string) => void;
  clearError: () => void;

  togglePanel: () => void;
  openDialog: (agent?: Agent | null) => void;
  closeDialog: () => void;
  setActiveAgentForSession: (agent: Agent | null) => void;
  clearActiveAgent: () => void;
}

export const useAgentsStore = create<AgentsState>((set, get) => ({
  agents: [],
  selectedAgent: null,
  runHistory: null,
  selectedAgentStats: null,
  searchQuery: '',
  panelOpen: false,
  dialogOpen: false,
  editingAgent: null,
  activeAgentForSession: null,
  loading: {
    agents: false,
    agent: false,
    history: false,
    stats: false,
    creating: false,
    updating: false,
    deleting: false,
    running: false,
  },
  error: null,

  fetchAgents: async () => {
    set((state) => ({
      loading: { ...state.loading, agents: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<AgentWithStats[]>>('list_agents_with_stats');

      if (response.success && response.data) {
        set((state) => ({
          agents: response.data!,
          loading: { ...state.loading, agents: false },
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to fetch agents',
          loading: { ...state.loading, agents: false },
        }));
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to fetch agents',
        loading: { ...state.loading, agents: false },
      }));
    }
  },

  fetchAgent: async (id: string) => {
    set((state) => ({
      loading: { ...state.loading, agent: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<Agent | null>>('get_agent', { id });

      if (response.success) {
        set((state) => ({
          selectedAgent: response.data,
          loading: { ...state.loading, agent: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to fetch agent',
          loading: { ...state.loading, agent: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to fetch agent',
        loading: { ...state.loading, agent: false },
      }));
      return null;
    }
  },

  createAgent: async (request: AgentCreateRequest) => {
    set((state) => ({
      loading: { ...state.loading, creating: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<Agent>>('create_agent', { request });

      if (response.success && response.data) {
        // Refresh agents list
        await get().fetchAgents();
        set((state) => ({
          loading: { ...state.loading, creating: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to create agent',
          loading: { ...state.loading, creating: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to create agent',
        loading: { ...state.loading, creating: false },
      }));
      return null;
    }
  },

  updateAgent: async (id: string, request: AgentUpdateRequest) => {
    set((state) => ({
      loading: { ...state.loading, updating: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<Agent>>('update_agent', { id, request });

      if (response.success && response.data) {
        // Refresh agents list
        await get().fetchAgents();
        set((state) => ({
          selectedAgent: state.selectedAgent?.id === id ? response.data : state.selectedAgent,
          loading: { ...state.loading, updating: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to update agent',
          loading: { ...state.loading, updating: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to update agent',
        loading: { ...state.loading, updating: false },
      }));
      return null;
    }
  },

  deleteAgent: async (id: string) => {
    set((state) => ({
      loading: { ...state.loading, deleting: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<void>>('delete_agent', { id });

      if (response.success) {
        // Refresh agents list
        await get().fetchAgents();
        set((state) => ({
          selectedAgent: state.selectedAgent?.id === id ? null : state.selectedAgent,
          loading: { ...state.loading, deleting: false },
        }));
        return true;
      } else {
        set((state) => ({
          error: response.error || 'Failed to delete agent',
          loading: { ...state.loading, deleting: false },
        }));
        return false;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to delete agent',
        loading: { ...state.loading, deleting: false },
      }));
      return false;
    }
  },

  selectAgent: (agent: Agent | null) => {
    set({ selectedAgent: agent, runHistory: null, selectedAgentStats: null });

    if (agent) {
      // Fetch related data
      get().fetchRunHistory(agent.id);
      get().fetchAgentStats(agent.id);
    }
  },

  fetchRunHistory: async (agentId: string, limit = 50, offset = 0) => {
    set((state) => ({
      loading: { ...state.loading, history: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<AgentRunList>>('get_agent_history', {
        agentId,
        limit,
        offset,
      });

      if (response.success && response.data) {
        set((state) => ({
          runHistory: response.data,
          loading: { ...state.loading, history: false },
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to fetch run history',
          loading: { ...state.loading, history: false },
        }));
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to fetch run history',
        loading: { ...state.loading, history: false },
      }));
    }
  },

  fetchAgentStats: async (agentId: string) => {
    set((state) => ({
      loading: { ...state.loading, stats: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<AgentStats>>('get_agent_stats', {
        agentId,
      });

      if (response.success && response.data) {
        set((state) => ({
          selectedAgentStats: response.data,
          loading: { ...state.loading, stats: false },
        }));
      } else {
        set((state) => ({
          error: response.error || 'Failed to fetch agent stats',
          loading: { ...state.loading, stats: false },
        }));
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to fetch agent stats',
        loading: { ...state.loading, stats: false },
      }));
    }
  },

  runAgent: async (agentId: string, input: string) => {
    set((state) => ({
      loading: { ...state.loading, running: true },
      error: null,
    }));

    try {
      const response = await invoke<CommandResponse<AgentRun>>('run_agent', {
        agentId,
        input,
      });

      if (response.success && response.data) {
        // Refresh history after run
        await get().fetchRunHistory(agentId);
        await get().fetchAgentStats(agentId);
        await get().fetchAgents(); // Refresh stats in list
        set((state) => ({
          loading: { ...state.loading, running: false },
        }));
        return response.data;
      } else {
        set((state) => ({
          error: response.error || 'Failed to run agent',
          loading: { ...state.loading, running: false },
        }));
        return null;
      }
    } catch (err) {
      set((state) => ({
        error: err instanceof Error ? err.message : 'Failed to run agent',
        loading: { ...state.loading, running: false },
      }));
      return null;
    }
  },

  exportAgents: async (agentIds?: string[]) => {
    try {
      const response = await invoke<CommandResponse<string>>('export_agents', {
        agentIds: agentIds || null,
      });

      if (response.success && response.data) {
        return response.data;
      } else {
        set({ error: response.error || 'Failed to export agents' });
        return null;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to export agents' });
      return null;
    }
  },

  importAgents: async (json: string) => {
    try {
      const response = await invoke<CommandResponse<Agent[]>>('import_agents', { json });

      if (response.success && response.data) {
        // Refresh agents list
        await get().fetchAgents();
        return response.data;
      } else {
        set({ error: response.error || 'Failed to import agents' });
        return null;
      }
    } catch (err) {
      set({ error: err instanceof Error ? err.message : 'Failed to import agents' });
      return null;
    }
  },

  setSearchQuery: (query: string) => {
    set({ searchQuery: query });
  },

  clearError: () => {
    set({ error: null });
  },

  togglePanel: () => set((s) => ({ panelOpen: !s.panelOpen })),
  openDialog: (agent = null) => set({ dialogOpen: true, editingAgent: agent ?? null }),
  closeDialog: () => set({ dialogOpen: false, editingAgent: null }),
  setActiveAgentForSession: (agent) => set({ activeAgentForSession: agent }),
  clearActiveAgent: () => set({ activeAgentForSession: null }),
}));

/** Get filtered agents based on search query */
export function getFilteredAgents(agents: AgentWithStats[], searchQuery: string): AgentWithStats[] {
  if (!searchQuery.trim()) {
    return agents;
  }

  const query = searchQuery.toLowerCase();
  return agents.filter(
    (agent) =>
      agent.name.toLowerCase().includes(query) ||
      (agent.description && agent.description.toLowerCase().includes(query)),
  );
}

export default useAgentsStore;
