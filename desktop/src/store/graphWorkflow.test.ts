/**
 * Graph Workflow Store Tests
 *
 * Tests for the Zustand graph workflow store including state management,
 * CRUD operations, node/edge manipulation, and IPC action mocking.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useGraphWorkflowStore } from './graphWorkflow';
import type {
  GraphWorkflow,
  GraphNode,
  Edge,
  GraphWorkflowInfo,
} from '../types/graphWorkflow';
import { createEmptyGraphWorkflow } from '../types/graphWorkflow';

// Mock invoke is already mocked in test setup
const mockInvoke = vi.mocked(invoke);

// Helper factories
function createMockWorkflowInfo(overrides: Partial<GraphWorkflowInfo> = {}): GraphWorkflowInfo {
  return {
    id: 'wf-1',
    name: 'Test Workflow',
    node_count: 2,
    edge_count: 1,
    ...overrides,
  };
}

function createMockWorkflow(overrides: Partial<GraphWorkflow> = {}): GraphWorkflow {
  return {
    name: 'Test Workflow',
    description: null,
    nodes: {
      'node-1': {
        id: 'node-1',
        agent_step: { step_type: 'llm_step', name: 'LLM 1', instruction: null, agent_config: { max_iterations: 50, max_total_tokens: 1000000, model_override: null, provider_override: null } },
        position: { x: 100, y: 100 },
      },
      'node-2': {
        id: 'node-2',
        agent_step: { step_type: 'llm_step', name: 'LLM 2', instruction: null, agent_config: { max_iterations: 50, max_total_tokens: 1000000, model_override: null, provider_override: null } },
        position: { x: 300, y: 100 },
      },
    },
    edges: [
      { edge_type: 'direct' as const, from: 'node-1', to: 'node-2' },
    ],
    entry_node: 'node-1',
    state_schema: { channels: {}, reducers: {} },
    ...overrides,
  };
}

function createMockNode(id: string): GraphNode {
  return {
    id,
    agent_step: {
      step_type: 'llm_step',
      name: `Node ${id}`,
      instruction: null,
      agent_config: { max_iterations: 50, max_total_tokens: 1000000, model_override: null, provider_override: null },
    },
    position: { x: 200, y: 200 },
  };
}

describe('useGraphWorkflowStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store to initial state
    useGraphWorkflowStore.setState({
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
    });
  });

  // ========================================================================
  // Initial State Tests
  // ========================================================================

  describe('Initial State', () => {
    it('should initialize with default values', () => {
      const state = useGraphWorkflowStore.getState();
      expect(state.workflows).toHaveLength(0);
      expect(state.currentWorkflow).toBeNull();
      expect(state.currentWorkflowId).toBeNull();
      expect(state.isCreating).toBe(false);
      expect(state.selectedNode).toBeNull();
      expect(state.selectedEdge).toBeNull();
      expect(state.isExecuting).toBe(false);
      expect(state.executionEvents).toHaveLength(0);
      expect(state.error).toBeNull();
    });
  });

  // ========================================================================
  // fetchWorkflows Tests
  // ========================================================================

  describe('fetchWorkflows', () => {
    it('should load workflows successfully', async () => {
      const mockList = [
        createMockWorkflowInfo({ id: 'wf-1', name: 'Workflow A' }),
        createMockWorkflowInfo({ id: 'wf-2', name: 'Workflow B' }),
      ];
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: mockList,
        error: null,
      });

      await useGraphWorkflowStore.getState().fetchWorkflows();

      const state = useGraphWorkflowStore.getState();
      expect(state.workflows).toHaveLength(2);
      expect(state.workflows[0].name).toBe('Workflow A');
      expect(state.loading.list).toBe(false);
      expect(state.error).toBeNull();
      expect(mockInvoke).toHaveBeenCalledWith('list_graph_workflows');
    });

    it('should handle fetch error', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      await useGraphWorkflowStore.getState().fetchWorkflows();

      const state = useGraphWorkflowStore.getState();
      expect(state.error).toBe('Network error');
      expect(state.loading.list).toBe(false);
    });
  });

  // ========================================================================
  // selectWorkflow Tests
  // ========================================================================

  describe('selectWorkflow', () => {
    it('should load and select a workflow by ID', async () => {
      const mockWf = createMockWorkflow();
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: mockWf,
        error: null,
      });

      await useGraphWorkflowStore.getState().selectWorkflow('wf-1');

      const state = useGraphWorkflowStore.getState();
      expect(state.currentWorkflow).not.toBeNull();
      expect(state.currentWorkflow?.name).toBe('Test Workflow');
      expect(state.currentWorkflowId).toBe('wf-1');
      expect(state.loading.detail).toBe(false);
      expect(state.isCreating).toBe(false);
    });

    it('should handle select error', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Not found'));

      await useGraphWorkflowStore.getState().selectWorkflow('bad-id');

      const state = useGraphWorkflowStore.getState();
      expect(state.error).toBe('Not found');
      expect(state.loading.detail).toBe(false);
    });
  });

  // ========================================================================
  // startNewWorkflow Tests
  // ========================================================================

  describe('startNewWorkflow', () => {
    it('should create a new empty workflow', () => {
      useGraphWorkflowStore.getState().startNewWorkflow();

      const state = useGraphWorkflowStore.getState();
      expect(state.currentWorkflow).not.toBeNull();
      expect(state.currentWorkflow?.name).toBe('New Workflow');
      expect(state.currentWorkflowId).toBeNull();
      expect(state.isCreating).toBe(true);
      expect(state.selectedNode).toBeNull();
      expect(state.selectedEdge).toBeNull();
      expect(state.error).toBeNull();
    });
  });

  // ========================================================================
  // Node Operations Tests
  // ========================================================================

  describe('Node Operations', () => {
    beforeEach(() => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createEmptyGraphWorkflow(),
        isCreating: true,
      });
    });

    it('should add a node and set as entry if first node', () => {
      const node = createMockNode('node-a');
      useGraphWorkflowStore.getState().addNode(node);

      const wf = useGraphWorkflowStore.getState().currentWorkflow!;
      expect(Object.keys(wf.nodes)).toHaveLength(1);
      expect(wf.nodes['node-a']).toBeDefined();
      expect(wf.entry_node).toBe('node-a');
    });

    it('should add multiple nodes without changing entry', () => {
      const nodeA = createMockNode('node-a');
      const nodeB = createMockNode('node-b');

      useGraphWorkflowStore.getState().addNode(nodeA);
      useGraphWorkflowStore.getState().addNode(nodeB);

      const wf = useGraphWorkflowStore.getState().currentWorkflow!;
      expect(Object.keys(wf.nodes)).toHaveLength(2);
      expect(wf.entry_node).toBe('node-a');
    });

    it('should remove a node and its connected edges', () => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createMockWorkflow(),
      });

      useGraphWorkflowStore.getState().removeNode('node-1');

      const wf = useGraphWorkflowStore.getState().currentWorkflow!;
      expect(Object.keys(wf.nodes)).toHaveLength(1);
      expect(wf.nodes['node-1']).toBeUndefined();
      expect(wf.edges).toHaveLength(0);
      expect(wf.entry_node).toBe('');
    });

    it('should update a node', () => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createMockWorkflow(),
      });

      useGraphWorkflowStore.getState().updateNode('node-1', {
        position: { x: 500, y: 500 },
      });

      const wf = useGraphWorkflowStore.getState().currentWorkflow!;
      expect(wf.nodes['node-1'].position).toEqual({ x: 500, y: 500 });
    });

    it('should not update a non-existent node', () => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createMockWorkflow(),
      });

      useGraphWorkflowStore.getState().updateNode('non-existent', {
        position: { x: 0, y: 0 },
      });

      const wf = useGraphWorkflowStore.getState().currentWorkflow!;
      expect(wf.nodes['non-existent']).toBeUndefined();
    });
  });

  // ========================================================================
  // Edge Operations Tests
  // ========================================================================

  describe('Edge Operations', () => {
    it('should add an edge', () => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createMockWorkflow({ edges: [] }),
      });

      const edge: Edge = { edge_type: 'direct', from: 'node-1', to: 'node-2' };
      useGraphWorkflowStore.getState().addEdge(edge);

      const wf = useGraphWorkflowStore.getState().currentWorkflow!;
      expect(wf.edges).toHaveLength(1);
      expect(wf.edges[0]).toEqual(edge);
    });

    it('should add a conditional edge', () => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createMockWorkflow({ edges: [] }),
      });

      const edge: Edge = {
        edge_type: 'conditional',
        from: 'node-1',
        condition: { condition_key: 'decision' },
        branches: { yes: 'node-2', no: 'node-3' },
        default_branch: 'node-2',
      };
      useGraphWorkflowStore.getState().addEdge(edge);

      const wf = useGraphWorkflowStore.getState().currentWorkflow!;
      expect(wf.edges).toHaveLength(1);
      expect(wf.edges[0].edge_type).toBe('conditional');
    });

    it('should remove an edge by index', () => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createMockWorkflow(),
      });

      useGraphWorkflowStore.getState().removeEdge(0);

      const wf = useGraphWorkflowStore.getState().currentWorkflow!;
      expect(wf.edges).toHaveLength(0);
    });

    it('should clear selectedEdge when removing the selected edge', () => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createMockWorkflow(),
        selectedEdge: 0,
      });

      useGraphWorkflowStore.getState().removeEdge(0);

      expect(useGraphWorkflowStore.getState().selectedEdge).toBeNull();
    });
  });

  // ========================================================================
  // Selection Tests
  // ========================================================================

  describe('Selection', () => {
    it('should set selected node and clear selected edge', () => {
      useGraphWorkflowStore.setState({ selectedEdge: 0 });

      useGraphWorkflowStore.getState().setSelectedNode('node-1');

      const state = useGraphWorkflowStore.getState();
      expect(state.selectedNode).toBe('node-1');
      expect(state.selectedEdge).toBeNull();
    });

    it('should set selected edge and clear selected node', () => {
      useGraphWorkflowStore.setState({ selectedNode: 'node-1' });

      useGraphWorkflowStore.getState().setSelectedEdge(2);

      const state = useGraphWorkflowStore.getState();
      expect(state.selectedEdge).toBe(2);
      expect(state.selectedNode).toBeNull();
    });

    it('should clear all selection state', () => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createMockWorkflow(),
        currentWorkflowId: 'wf-1',
        isCreating: true,
        selectedNode: 'node-1',
        selectedEdge: 0,
      });

      useGraphWorkflowStore.getState().clearSelection();

      const state = useGraphWorkflowStore.getState();
      expect(state.currentWorkflow).toBeNull();
      expect(state.currentWorkflowId).toBeNull();
      expect(state.isCreating).toBe(false);
      expect(state.selectedNode).toBeNull();
      expect(state.selectedEdge).toBeNull();
    });
  });

  // ========================================================================
  // saveWorkflow Tests
  // ========================================================================

  describe('saveWorkflow', () => {
    it('should create a new workflow when isCreating is true', async () => {
      const workflow = createMockWorkflow();
      useGraphWorkflowStore.setState({
        currentWorkflow: workflow,
        isCreating: true,
        currentWorkflowId: null,
      });

      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: workflow,
        error: null,
      });
      // For fetchWorkflows after save
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: [createMockWorkflowInfo()],
        error: null,
      });

      await useGraphWorkflowStore.getState().saveWorkflow();

      const state = useGraphWorkflowStore.getState();
      expect(state.isCreating).toBe(false);
      expect(state.loading.save).toBe(false);
      expect(mockInvoke).toHaveBeenCalledWith('create_graph_workflow', { workflow });
    });

    it('should update an existing workflow', async () => {
      const workflow = createMockWorkflow();
      useGraphWorkflowStore.setState({
        currentWorkflow: workflow,
        isCreating: false,
        currentWorkflowId: 'wf-1',
      });

      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: workflow,
        error: null,
      });
      // For fetchWorkflows after save
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: [createMockWorkflowInfo()],
        error: null,
      });

      await useGraphWorkflowStore.getState().saveWorkflow();

      const state = useGraphWorkflowStore.getState();
      expect(state.loading.save).toBe(false);
      expect(mockInvoke).toHaveBeenCalledWith('update_graph_workflow', { id: 'wf-1', workflow });
    });

    it('should handle save error', async () => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createMockWorkflow(),
        isCreating: true,
      });

      mockInvoke.mockRejectedValueOnce(new Error('Save failed'));

      await useGraphWorkflowStore.getState().saveWorkflow();

      const state = useGraphWorkflowStore.getState();
      expect(state.error).toBe('Save failed');
      expect(state.loading.save).toBe(false);
    });

    it('should not save when no current workflow', async () => {
      await useGraphWorkflowStore.getState().saveWorkflow();
      expect(mockInvoke).not.toHaveBeenCalled();
    });
  });

  // ========================================================================
  // deleteWorkflow Tests
  // ========================================================================

  describe('deleteWorkflow', () => {
    it('should delete workflow and clear current if same ID', async () => {
      useGraphWorkflowStore.setState({
        currentWorkflow: createMockWorkflow(),
        currentWorkflowId: 'wf-1',
      });

      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: true,
        error: null,
      });
      // For fetchWorkflows after delete
      mockInvoke.mockResolvedValueOnce({
        success: true,
        data: [],
        error: null,
      });

      await useGraphWorkflowStore.getState().deleteWorkflow('wf-1');

      const state = useGraphWorkflowStore.getState();
      expect(state.currentWorkflow).toBeNull();
      expect(state.currentWorkflowId).toBeNull();
    });

    it('should handle delete error', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Delete failed'));

      await useGraphWorkflowStore.getState().deleteWorkflow('wf-bad');

      const state = useGraphWorkflowStore.getState();
      expect(state.error).toBe('Delete failed');
    });
  });

  // ========================================================================
  // Execution Events Tests
  // ========================================================================

  describe('Execution Events', () => {
    it('should add execution events', () => {
      useGraphWorkflowStore.getState().addExecutionEvent({
        type: 'graph_node_started',
        node_id: 'node-1',
      });
      useGraphWorkflowStore.getState().addExecutionEvent({
        type: 'graph_node_completed',
        node_id: 'node-1',
        output: 'result',
      });

      const state = useGraphWorkflowStore.getState();
      expect(state.executionEvents).toHaveLength(2);
      expect(state.executionEvents[0].type).toBe('graph_node_started');
      expect(state.executionEvents[1].type).toBe('graph_node_completed');
    });

    it('should clear execution events', () => {
      useGraphWorkflowStore.setState({
        executionEvents: [
          { type: 'graph_node_started', node_id: 'node-1' },
        ],
        isExecuting: true,
      });

      useGraphWorkflowStore.getState().clearExecutionEvents();

      const state = useGraphWorkflowStore.getState();
      expect(state.executionEvents).toHaveLength(0);
      expect(state.isExecuting).toBe(false);
    });
  });

  // ========================================================================
  // Error Handling Tests
  // ========================================================================

  describe('Error Handling', () => {
    it('should set and clear error', () => {
      useGraphWorkflowStore.getState().setError('Something went wrong');
      expect(useGraphWorkflowStore.getState().error).toBe('Something went wrong');

      useGraphWorkflowStore.getState().setError(null);
      expect(useGraphWorkflowStore.getState().error).toBeNull();
    });
  });
});
