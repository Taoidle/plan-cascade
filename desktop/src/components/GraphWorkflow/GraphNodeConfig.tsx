/**
 * GraphNodeConfig Component
 *
 * Sidebar panel for editing a selected graph node's configuration.
 */

import { clsx } from 'clsx';
import type { GraphNode } from '../../types/graphWorkflow';

interface GraphNodeConfigProps {
  node: GraphNode;
  isEntryNode: boolean;
  onUpdate: (updates: Partial<GraphNode>) => void;
  onSetEntry: () => void;
  onDelete: () => void;
}

export function GraphNodeConfig({
  node,
  isEntryNode,
  onUpdate,
  onSetEntry,
  onDelete,
}: GraphNodeConfigProps) {
  const stepType = node.agent_step.step_type;

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-semibold text-gray-900 dark:text-white">
        Node Configuration
      </h3>

      {/* Node ID */}
      <div>
        <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
          Node ID
        </label>
        <div className="text-xs text-gray-700 dark:text-gray-300 font-mono bg-gray-50 dark:bg-gray-800 px-2 py-1 rounded">
          {node.id}
        </div>
      </div>

      {/* Step Name */}
      <div>
        <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
          Name
        </label>
        <input
          type="text"
          value={node.agent_step.name}
          onChange={(e) => {
            onUpdate({
              agent_step: { ...node.agent_step, name: e.target.value },
            });
          }}
          className="w-full text-sm px-2 py-1.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
        />
      </div>

      {/* Step Type (read-only) */}
      <div>
        <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
          Type
        </label>
        <div className="text-xs text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-800 px-2 py-1 rounded">
          {stepType}
        </div>
      </div>

      {/* LLM-specific: Instruction */}
      {stepType === 'llm_step' && 'instruction' in node.agent_step && (
        <div>
          <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
            System Instruction
          </label>
          <textarea
            value={node.agent_step.instruction ?? ''}
            onChange={(e) => {
              onUpdate({
                agent_step: {
                  ...node.agent_step,
                  instruction: e.target.value || null,
                },
              });
            }}
            className="w-full text-sm px-2 py-1.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white h-24 resize-y"
            placeholder="Optional system prompt..."
          />
        </div>
      )}

      {/* Entry node toggle */}
      <div>
        {isEntryNode ? (
          <span className="text-xs text-green-600 dark:text-green-400 font-medium">
            This is the entry node
          </span>
        ) : (
          <button
            onClick={onSetEntry}
            className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
          >
            Set as entry node
          </button>
        )}
      </div>

      {/* Delete */}
      <button
        onClick={onDelete}
        className={clsx(
          'w-full py-1.5 text-xs font-medium rounded transition-colors',
          'border border-red-300 dark:border-red-700',
          'text-red-600 dark:text-red-400',
          'hover:bg-red-50 dark:hover:bg-red-900/20'
        )}
      >
        Delete Node
      </button>
    </div>
  );
}
