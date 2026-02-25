/**
 * Graph Workflow Store
 *
 * Zustand store for managing graph workflow state in the Graph Workflow Editor.
 * Handles CRUD operations, node/edge selection, and execution state.
 */

import { create } from 'zustand';
import type { GraphWorkflow, GraphWorkflowInfo, GraphNode, Edge, GraphWorkflowEvent } from '../types/graphWorkflow';
import { createEmptyGraphWorkflow } from '../types/graphWorkflow';
import {
  listGraphWorkflows,
  getGraphWorkflow,
  createGraphWorkflow,
  updateGraphWorkflow,
  deleteGraphWorkflow,
} from '../lib/graphWorkflowApi';

interface GraphWorkflowState {
  /** List of all saved workflows (summary info) */
  workflows: GraphWorkflowInfo[];

  /** Currently selected/editing workflow */
  currentWorkflow: GraphWorkflow | null;

  /** ID of the current workflow (for persistence) */
  currentWorkflowId: string | null;

  /** Whether we're creating a new workflow */
  isCreating: boolean;

  /** Currently selected node ID */
  selectedNode: string | null;

  /** Currently selected edge index */
  selectedEdge: number | null;

  /** Whether a workflow is currently executing */
  isExecuting: boolean;

  /** Events from the currently running workflow */
  executionEvents: GraphWorkflowEvent[];

  /** Loading states */
  loading: {
    list: boolean;
    detail: boolean;
    save: boolean;
  };

  /** Error message */
  error: string | null;

  // Actions

  /** Fetch the list of all workflows */
  fetchWorkflows: () => Promise<void>;

  /** Select a workflow to view/edit */
  selectWorkflow: (id: string) => Promise<void>;

  /** Start creating a new workflow */
  startNewWorkflow: () => void;

  /** Update the current workflow in memory */
  updateCurrentWorkflow: (updates: Partial<GraphWorkflow>) => void;

  /** Add a node to the current workflow */
  addNode: (node: GraphNode) => void;

  /** Remove a node by ID */
  removeNode: (nodeId: string) => void;

  /** Update a node */
  updateNode: (nodeId: string, updates: Partial<GraphNode>) => void;

  /** Add an edge */
  addEdge: (edge: Edge) => void;

  /** Remove an edge by index */
  removeEdge: (index: number) => void;

  /** Set the selected node */
  setSelectedNode: (nodeId: string | null) => void;

  /** Set the selected edge */
  setSelectedEdge: (index: number | null) => void;

  /** Save the current workflow */
  saveWorkflow: () => Promise<void>;

  /** Delete a workflow */
  deleteWorkflow: (id: string) => Promise<void>;

  /** Clear selection */
  clearSelection: () => void;

  /** Add an execution event */
  addExecutionEvent: (event: GraphWorkflowEvent) => void;

  /** Clear execution events */
  clearExecutionEvents: () => void;

  /** Set error */
  setError: (error: string | null) => void;
}

export const useGraphWorkflowStore = create<GraphWorkflowState>((set, get) => ({
  workflows: [],
  currentWorkflow: null,
  currentWorkflowId: null,
  isCreating: false,
  selectedNode: null,
  selectedEdge: null,
  isExecuting: false,
  executionEvents: [],
  loading: { list: false, detail: false, save: false },
  error: null,

  fetchWorkflows: async () => {
    set((s) => ({ loading: { ...s.loading, list: true }, error: null }));
    try {
      const workflows = await listGraphWorkflows();
      set({ workflows, loading: { ...get().loading, list: false } });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set((s) => ({ error: msg, loading: { ...s.loading, list: false } }));
    }
  },

  selectWorkflow: async (id: string) => {
    set((s) => ({
      loading: { ...s.loading, detail: true },
      error: null,
      isCreating: false,
      selectedNode: null,
      selectedEdge: null,
    }));
    try {
      const workflow = await getGraphWorkflow(id);
      set({
        currentWorkflow: workflow,
        currentWorkflowId: id,
        loading: { ...get().loading, detail: false },
      });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set((s) => ({ error: msg, loading: { ...s.loading, detail: false } }));
    }
  },

  startNewWorkflow: () => {
    set({
      currentWorkflow: createEmptyGraphWorkflow(),
      currentWorkflowId: null,
      isCreating: true,
      selectedNode: null,
      selectedEdge: null,
      error: null,
    });
  },

  updateCurrentWorkflow: (updates) => {
    const current = get().currentWorkflow;
    if (!current) return;
    set({ currentWorkflow: { ...current, ...updates } });
  },

  addNode: (node) => {
    const current = get().currentWorkflow;
    if (!current) return;
    const nodes = { ...current.nodes, [node.id]: node };
    // If this is the first node, set it as entry
    const entry_node = Object.keys(nodes).length === 1 ? node.id : current.entry_node;
    set({ currentWorkflow: { ...current, nodes, entry_node } });
  },

  removeNode: (nodeId) => {
    const current = get().currentWorkflow;
    if (!current) return;
    const nodes = { ...current.nodes };
    delete nodes[nodeId];
    // Remove edges referencing this node
    const edges = current.edges.filter((e) => {
      if (e.edge_type === 'direct') {
        return e.from !== nodeId && e.to !== nodeId;
      }
      return e.from !== nodeId;
    });
    // Reset entry if it was the removed node
    const entry_node = current.entry_node === nodeId ? '' : current.entry_node;
    set({
      currentWorkflow: { ...current, nodes, edges, entry_node },
      selectedNode: get().selectedNode === nodeId ? null : get().selectedNode,
    });
  },

  updateNode: (nodeId, updates) => {
    const current = get().currentWorkflow;
    if (!current || !current.nodes[nodeId]) return;
    const nodes = {
      ...current.nodes,
      [nodeId]: { ...current.nodes[nodeId], ...updates },
    };
    set({ currentWorkflow: { ...current, nodes } });
  },

  addEdge: (edge) => {
    const current = get().currentWorkflow;
    if (!current) return;
    set({ currentWorkflow: { ...current, edges: [...current.edges, edge] } });
  },

  removeEdge: (index) => {
    const current = get().currentWorkflow;
    if (!current) return;
    const edges = current.edges.filter((_, i) => i !== index);
    set({
      currentWorkflow: { ...current, edges },
      selectedEdge: get().selectedEdge === index ? null : get().selectedEdge,
    });
  },

  setSelectedNode: (nodeId) => {
    set({ selectedNode: nodeId, selectedEdge: null });
  },

  setSelectedEdge: (index) => {
    set({ selectedEdge: index, selectedNode: null });
  },

  saveWorkflow: async () => {
    const { currentWorkflow, currentWorkflowId, isCreating } = get();
    if (!currentWorkflow) return;

    set((s) => ({ loading: { ...s.loading, save: true }, error: null }));
    try {
      if (isCreating || !currentWorkflowId) {
        const saved = await createGraphWorkflow(currentWorkflow);
        set({
          currentWorkflow: saved,
          isCreating: false,
          loading: { ...get().loading, save: false },
        });
      } else {
        const saved = await updateGraphWorkflow(currentWorkflowId, currentWorkflow);
        set({
          currentWorkflow: saved,
          loading: { ...get().loading, save: false },
        });
      }
      await get().fetchWorkflows();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set((s) => ({ error: msg, loading: { ...s.loading, save: false } }));
    }
  },

  deleteWorkflow: async (id) => {
    set({ error: null });
    try {
      await deleteGraphWorkflow(id);
      if (get().currentWorkflowId === id) {
        set({ currentWorkflow: null, currentWorkflowId: null, isCreating: false });
      }
      await get().fetchWorkflows();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ error: msg });
    }
  },

  clearSelection: () => {
    set({
      currentWorkflow: null,
      currentWorkflowId: null,
      isCreating: false,
      selectedNode: null,
      selectedEdge: null,
      error: null,
    });
  },

  addExecutionEvent: (event) => {
    set((s) => ({
      executionEvents: [...s.executionEvents, event],
    }));
  },

  clearExecutionEvents: () => {
    set({ executionEvents: [], isExecuting: false });
  },

  setError: (error) => {
    set({ error });
  },
}));
