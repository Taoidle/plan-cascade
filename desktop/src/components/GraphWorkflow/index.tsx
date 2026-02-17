/**
 * GraphWorkflowEditor Component
 *
 * Main container for the Graph Workflow Editor in Expert Mode.
 * Provides a canvas-like interface for building graph workflows with:
 * - Workflow list sidebar
 * - Draggable node canvas
 * - Edge connections (SVG lines)
 * - Node and edge configuration panels
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useGraphWorkflowStore } from '../../store/graphWorkflow';
import { GraphWorkflowList } from './GraphWorkflowList';
import { GraphNodeComponent } from './GraphNodeComponent';
import { GraphEdgeComponent } from './GraphEdgeComponent';
import { GraphToolbar } from './GraphToolbar';
import { GraphNodeConfig } from './GraphNodeConfig';
import { GraphEdgeConfig } from './GraphEdgeConfig';
import type { GraphNode, Edge, NodePosition } from '../../types/graphWorkflow';
import { createLlmStep } from '../../types/agentComposer';

export function GraphWorkflowEditor() {
  const { t } = useTranslation('expertMode');
  const {
    currentWorkflow,
    isCreating,
    loading,
    error,
    selectedNode,
    selectedEdge,
    updateCurrentWorkflow,
    addNode,
    removeNode,
    updateNode,
    addEdge,
    removeEdge,
    setSelectedNode,
    setSelectedEdge,
    saveWorkflow,
    clearSelection,
  } = useGraphWorkflowStore();

  const [edgeMode, setEdgeMode] = useState<string | null>(null);

  const handleAddNode = useCallback((type: string) => {
    if (!currentWorkflow) return;
    const nodeCount = Object.keys(currentWorkflow.nodes).length;
    const nodeId = `node-${Date.now()}`;
    const name = `${type.charAt(0).toUpperCase() + type.slice(1)} ${nodeCount + 1}`;

    let agentStep;
    switch (type) {
      case 'llm':
        agentStep = createLlmStep(name);
        break;
      case 'sequential':
        agentStep = { step_type: 'sequential_step' as const, name, steps: [] };
        break;
      case 'parallel':
        agentStep = { step_type: 'parallel_step' as const, name, steps: [] };
        break;
      default:
        agentStep = createLlmStep(name);
    }

    const node: GraphNode = {
      id: nodeId,
      agent_step: agentStep,
      position: {
        x: 100 + (nodeCount % 4) * 220,
        y: 80 + Math.floor(nodeCount / 4) * 150,
      },
    };

    addNode(node);
  }, [currentWorkflow, addNode]);

  const handleAddEdge = useCallback((type: 'direct' | 'conditional') => {
    setEdgeMode(type);
  }, []);

  const handleNodeClick = useCallback((nodeId: string) => {
    if (edgeMode) {
      // We're in edge-creation mode
      if (!selectedNode) {
        // First click - select source
        setSelectedNode(nodeId);
      } else {
        // Second click - create edge
        if (selectedNode !== nodeId) {
          const edge: Edge = edgeMode === 'direct'
            ? { edge_type: 'direct', from: selectedNode, to: nodeId }
            : {
                edge_type: 'conditional',
                from: selectedNode,
                condition: { condition_key: 'decision' },
                branches: {},
                default_branch: nodeId,
              };
          addEdge(edge);
        }
        setSelectedNode(null);
        setEdgeMode(null);
      }
    } else {
      setSelectedNode(nodeId);
    }
  }, [edgeMode, selectedNode, addEdge, setSelectedNode]);

  const handleNodeDrag = useCallback((nodeId: string, position: NodePosition) => {
    updateNode(nodeId, { position });
  }, [updateNode]);

  const handleDeleteSelected = useCallback(() => {
    if (selectedNode) {
      removeNode(selectedNode);
    } else if (selectedEdge !== null) {
      removeEdge(selectedEdge);
    }
  }, [selectedNode, selectedEdge, removeNode, removeEdge]);

  return (
    <div className="h-full flex">
      {/* Left sidebar: Workflow list */}
      <div
        className={clsx(
          'w-64 min-w-[16rem] p-4 overflow-auto',
          'border-r border-gray-200 dark:border-gray-700',
          'bg-gray-50 dark:bg-gray-900'
        )}
      >
        <GraphWorkflowList />
      </div>

      {/* Main content area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {currentWorkflow ? (
          <>
            {/* Header */}
            <div
              className={clsx(
                'flex items-center justify-between px-6 py-3',
                'border-b border-gray-200 dark:border-gray-700'
              )}
            >
              <div className="flex items-center gap-3 min-w-0">
                <input
                  type="text"
                  value={currentWorkflow.name}
                  onChange={(e) => updateCurrentWorkflow({ name: e.target.value })}
                  className={clsx(
                    'text-lg font-semibold bg-transparent border-none outline-none',
                    'text-gray-900 dark:text-white',
                    'focus:ring-1 focus:ring-primary-500 rounded px-1'
                  )}
                  placeholder={t('graphWorkflow.workflowName')}
                />
                {isCreating && (
                  <span className="text-xs text-primary-600 dark:text-primary-400 font-medium">
                    {t('graphWorkflow.new')}
                  </span>
                )}
                {edgeMode && (
                  <span className="text-xs text-amber-600 dark:text-amber-400 font-medium px-2 py-0.5 bg-amber-50 dark:bg-amber-900/20 rounded">
                    {t('graphWorkflow.edgeMode')}
                  </span>
                )}
              </div>

              <div className="flex items-center gap-2">
                {edgeMode && (
                  <button
                    onClick={() => { setEdgeMode(null); setSelectedNode(null); }}
                    className="px-3 py-1.5 text-xs font-medium rounded-lg bg-amber-100 dark:bg-amber-900 text-amber-600 dark:text-amber-400"
                  >
                    {t('graphWorkflow.cancelEdge')}
                  </button>
                )}
                <button
                  onClick={clearSelection}
                  className="px-3 py-1.5 text-xs font-medium rounded-lg bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                >
                  {t('graphWorkflow.close')}
                </button>
                <button
                  onClick={saveWorkflow}
                  disabled={loading.save || !currentWorkflow.name.trim()}
                  className={clsx(
                    'px-4 py-1.5 text-xs font-medium rounded-lg transition-colors',
                    'bg-primary-600 text-white hover:bg-primary-700',
                    'disabled:opacity-50 disabled:cursor-not-allowed'
                  )}
                >
                  {loading.save ? t('graphWorkflow.saving') : t('graphWorkflow.save')}
                </button>
              </div>
            </div>

            {/* Toolbar */}
            <GraphToolbar
              onAddNode={handleAddNode}
              onAddEdge={handleAddEdge}
              onDeleteSelected={handleDeleteSelected}
              hasSelection={selectedNode !== null || selectedEdge !== null}
            />

            {/* Error display */}
            {error && (
              <div className="mx-6 mb-2 p-2 rounded-lg bg-red-50 dark:bg-red-900/20 text-xs text-red-600 dark:text-red-400">
                {error}
              </div>
            )}

            {/* Canvas + Config Panel */}
            <div className="flex-1 flex overflow-hidden">
              {/* Canvas area */}
              <div
                className="flex-1 relative overflow-auto bg-gray-100 dark:bg-gray-800"
                onClick={(e) => {
                  if (e.target === e.currentTarget) {
                    setSelectedNode(null);
                    setSelectedEdge(null);
                  }
                }}
              >
                {/* SVG layer for edges */}
                <svg
                  className="absolute inset-0 w-full h-full pointer-events-none"
                  style={{ minWidth: '100%', minHeight: '100%' }}
                >
                  {currentWorkflow.edges.map((edge, index) => (
                    <GraphEdgeComponent
                      key={index}
                      edge={edge}
                      index={index}
                      nodes={currentWorkflow.nodes}
                      isSelected={selectedEdge === index}
                      onClick={() => setSelectedEdge(index)}
                    />
                  ))}
                </svg>

                {/* Node layer */}
                {Object.values(currentWorkflow.nodes).map((node) => (
                  <GraphNodeComponent
                    key={node.id}
                    node={node}
                    isSelected={selectedNode === node.id}
                    isEntryNode={currentWorkflow.entry_node === node.id}
                    onClick={() => handleNodeClick(node.id)}
                    onDrag={(pos) => handleNodeDrag(node.id, pos)}
                  />
                ))}

                {/* Empty state */}
                {Object.keys(currentWorkflow.nodes).length === 0 && (
                  <div className="absolute inset-0 flex items-center justify-center">
                    <div className="text-center text-gray-500 dark:text-gray-400">
                      <p className="text-sm">{t('graphWorkflow.emptyCanvas')}</p>
                      <p className="text-xs mt-1">{t('graphWorkflow.emptyCanvasHint')}</p>
                    </div>
                  </div>
                )}
              </div>

              {/* Config sidebar */}
              {(selectedNode || selectedEdge !== null) && (
                <div
                  className={clsx(
                    'w-80 min-w-[20rem] p-4 overflow-auto',
                    'border-l border-gray-200 dark:border-gray-700',
                    'bg-white dark:bg-gray-900'
                  )}
                >
                  {selectedNode && currentWorkflow.nodes[selectedNode] && (
                    <GraphNodeConfig
                      node={currentWorkflow.nodes[selectedNode]}
                      isEntryNode={currentWorkflow.entry_node === selectedNode}
                      onUpdate={(updates) => updateNode(selectedNode, updates)}
                      onSetEntry={() => updateCurrentWorkflow({ entry_node: selectedNode })}
                      onDelete={() => removeNode(selectedNode)}
                    />
                  )}
                  {selectedEdge !== null && currentWorkflow.edges[selectedEdge] && (
                    <GraphEdgeConfig
                      edge={currentWorkflow.edges[selectedEdge]}
                      onUpdate={(updatedEdge) => {
                        const edges = [...currentWorkflow.edges];
                        edges[selectedEdge] = updatedEdge;
                        updateCurrentWorkflow({ edges });
                      }}
                      onDelete={() => removeEdge(selectedEdge)}
                    />
                  )}
                </div>
              )}
            </div>
          </>
        ) : (
          /* Empty state */
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center">
              <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-2">
                {t('graphWorkflow.title')}
              </h2>
              <p className="text-gray-500 dark:text-gray-400 mb-4 max-w-md">
                {t('graphWorkflow.description')}
              </p>
              <button
                onClick={() => useGraphWorkflowStore.getState().startNewWorkflow()}
                className={clsx(
                  'px-4 py-2 text-sm font-medium rounded-lg',
                  'bg-primary-600 text-white hover:bg-primary-700',
                  'transition-colors'
                )}
              >
                {t('graphWorkflow.createFirst')}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default GraphWorkflowEditor;
