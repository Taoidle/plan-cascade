/**
 * GraphWorkflowList Component
 *
 * Sidebar list of saved graph workflows with create/edit/delete actions.
 */

import { useEffect } from 'react';
import { clsx } from 'clsx';
import { useGraphWorkflowStore } from '../../store/graphWorkflow';

export function GraphWorkflowList() {
  const {
    workflows,
    currentWorkflowId,
    loading,
    fetchWorkflows,
    selectWorkflow,
    startNewWorkflow,
    deleteWorkflow,
  } = useGraphWorkflowStore();

  useEffect(() => {
    fetchWorkflows();
  }, [fetchWorkflows]);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-white">
          Workflows
        </h3>
        <button
          onClick={startNewWorkflow}
          className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
        >
          + New
        </button>
      </div>

      {loading.list ? (
        <div className="text-xs text-gray-500 dark:text-gray-400">Loading...</div>
      ) : workflows.length === 0 ? (
        <div className="text-xs text-gray-500 dark:text-gray-400">
          No workflows yet
        </div>
      ) : (
        <div className="space-y-1">
          {workflows.map((wf) => (
            <div
              key={wf.id}
              className={clsx(
                'flex items-center justify-between p-2 rounded-lg cursor-pointer transition-colors',
                currentWorkflowId === wf.id
                  ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                  : 'hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-700 dark:text-gray-300'
              )}
              onClick={() => selectWorkflow(wf.id)}
            >
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium truncate">{wf.name}</p>
                <p className="text-xs text-gray-500 dark:text-gray-400">
                  {wf.node_count} nodes, {wf.edge_count} edges
                </p>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  if (confirm(`Delete workflow "${wf.name}"?`)) {
                    deleteWorkflow(wf.id);
                  }
                }}
                className="text-xs text-gray-400 hover:text-red-500 ml-2 shrink-0"
                title="Delete workflow"
              >
                x
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
