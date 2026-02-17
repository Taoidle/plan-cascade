/**
 * AgentComposer Component
 *
 * Main container for the Agent Composer feature in Expert Mode.
 * Provides a canvas-like interface for composing agent pipelines with:
 * - Pipeline list sidebar
 * - Agent node canvas for composition
 * - Execution runner for real-time events
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useAgentComposerStore } from '../../store/agentComposer';
import { AgentNode } from './AgentNode';
import { AgentPipelineList } from './AgentPipelineList';
import { AgentPipelineRunner } from './AgentPipelineRunner';
import { createLlmStep, DEFAULT_AGENT_CONFIG } from '../../types/agentComposer';
import type { AgentStep } from '../../types/agentComposer';

export function AgentComposer() {
  const {
    currentPipeline,
    isCreating,
    loading,
    error,
    updateCurrentPipeline,
    addStep,
    removeStep,
    updateStep,
    savePipeline,
    clearSelection,
  } = useAgentComposerStore();

  const [showRunner, setShowRunner] = useState(false);

  const handleAddStep = (type: string) => {
    let step: AgentStep;
    switch (type) {
      case 'llm':
        step = createLlmStep(`Agent ${(currentPipeline?.steps.length ?? 0) + 1}`);
        break;
      case 'sequential':
        step = {
          step_type: 'sequential_step',
          name: `Sequential ${(currentPipeline?.steps.length ?? 0) + 1}`,
          steps: [],
        };
        break;
      case 'parallel':
        step = {
          step_type: 'parallel_step',
          name: `Parallel ${(currentPipeline?.steps.length ?? 0) + 1}`,
          steps: [],
        };
        break;
      case 'conditional':
        step = {
          step_type: 'conditional_step',
          name: `Conditional ${(currentPipeline?.steps.length ?? 0) + 1}`,
          condition_key: 'decision',
          branches: {},
          default_branch: null,
        };
        break;
      default:
        return;
    }
    addStep(step);
  };

  return (
    <div className="h-full flex">
      {/* Left sidebar: Pipeline list */}
      <div
        className={clsx(
          'w-64 min-w-[16rem] p-4 overflow-auto',
          'border-r border-gray-200 dark:border-gray-700',
          'bg-gray-50 dark:bg-gray-900'
        )}
      >
        <AgentPipelineList />
      </div>

      {/* Main content area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {currentPipeline ? (
          <>
            {/* Pipeline editor header */}
            <div
              className={clsx(
                'flex items-center justify-between px-6 py-3',
                'border-b border-gray-200 dark:border-gray-700'
              )}
            >
              <div className="flex items-center gap-3 min-w-0">
                <input
                  type="text"
                  value={currentPipeline.name}
                  onChange={(e) =>
                    updateCurrentPipeline({ name: e.target.value })
                  }
                  className={clsx(
                    'text-lg font-semibold bg-transparent border-none outline-none',
                    'text-gray-900 dark:text-white',
                    'focus:ring-1 focus:ring-primary-500 rounded px-1'
                  )}
                  placeholder="Pipeline name"
                />
                {isCreating && (
                  <span className="text-xs text-primary-600 dark:text-primary-400 font-medium">
                    NEW
                  </span>
                )}
              </div>

              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowRunner(!showRunner)}
                  className={clsx(
                    'px-3 py-1.5 text-xs font-medium rounded-lg transition-colors',
                    showRunner
                      ? 'bg-primary-100 dark:bg-primary-900 text-primary-600 dark:text-primary-400'
                      : 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-600'
                  )}
                >
                  Runner
                </button>
                <button
                  onClick={clearSelection}
                  className="px-3 py-1.5 text-xs font-medium rounded-lg bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                >
                  Close
                </button>
                <button
                  onClick={savePipeline}
                  disabled={loading.save || !currentPipeline.name.trim()}
                  className={clsx(
                    'px-4 py-1.5 text-xs font-medium rounded-lg transition-colors',
                    'bg-primary-600 text-white hover:bg-primary-700',
                    'disabled:opacity-50 disabled:cursor-not-allowed'
                  )}
                >
                  {loading.save ? 'Saving...' : 'Save'}
                </button>
              </div>
            </div>

            {/* Description */}
            <div className="px-6 py-2">
              <input
                type="text"
                value={currentPipeline.description ?? ''}
                onChange={(e) =>
                  updateCurrentPipeline({
                    description: e.target.value || null,
                  })
                }
                className="w-full text-sm text-gray-600 dark:text-gray-400 bg-transparent border-none outline-none focus:ring-1 focus:ring-primary-500 rounded px-1"
                placeholder="Add a description..."
              />
            </div>

            {/* Error display */}
            {error && (
              <div className="mx-6 mb-2 p-2 rounded-lg bg-red-50 dark:bg-red-900/20 text-xs text-red-600 dark:text-red-400">
                {error}
              </div>
            )}

            {/* Main content: Canvas + Runner */}
            <div className="flex-1 flex overflow-hidden">
              {/* Canvas area */}
              <div className="flex-1 overflow-auto p-6">
                {/* Add step buttons */}
                <div className="flex gap-2 mb-4">
                  <AddStepButton
                    label="+ LLM Agent"
                    color="blue"
                    onClick={() => handleAddStep('llm')}
                  />
                  <AddStepButton
                    label="+ Sequential"
                    color="green"
                    onClick={() => handleAddStep('sequential')}
                  />
                  <AddStepButton
                    label="+ Parallel"
                    color="purple"
                    onClick={() => handleAddStep('parallel')}
                  />
                  <AddStepButton
                    label="+ Conditional"
                    color="amber"
                    onClick={() => handleAddStep('conditional')}
                  />
                </div>

                {/* Agent nodes */}
                {currentPipeline.steps.length === 0 ? (
                  <div className="flex items-center justify-center h-48 rounded-lg border-2 border-dashed border-gray-300 dark:border-gray-600 text-sm text-gray-500 dark:text-gray-400">
                    Add agent steps above to build your pipeline
                  </div>
                ) : (
                  <div className="space-y-3">
                    {currentPipeline.steps.map((step, index) => (
                      <div key={index} className="relative">
                        {/* Connection line between nodes */}
                        {index > 0 && (
                          <div className="flex justify-center -mt-1 mb-1">
                            <div className="w-0.5 h-4 bg-gray-300 dark:bg-gray-600" />
                          </div>
                        )}
                        <AgentNode
                          step={step}
                          index={index}
                          onUpdate={updateStep}
                          onRemove={removeStep}
                        />
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* Runner sidebar */}
              {showRunner && (
                <div
                  className={clsx(
                    'w-80 min-w-[20rem] p-4',
                    'border-l border-gray-200 dark:border-gray-700',
                    'bg-gray-50 dark:bg-gray-900'
                  )}
                >
                  <AgentPipelineRunner />
                </div>
              )}
            </div>
          </>
        ) : (
          /* Empty state */
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center">
              <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-2">
                Agent Composer
              </h2>
              <p className="text-gray-500 dark:text-gray-400 mb-4 max-w-md">
                Build composable agent pipelines by combining LLM agents with
                sequential, parallel, and conditional control flow.
              </p>
              <button
                onClick={() => useAgentComposerStore.getState().startNewPipeline()}
                className={clsx(
                  'px-4 py-2 text-sm font-medium rounded-lg',
                  'bg-primary-600 text-white hover:bg-primary-700',
                  'transition-colors'
                )}
              >
                Create Your First Pipeline
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

interface AddStepButtonProps {
  label: string;
  color: string;
  onClick: () => void;
}

function AddStepButton({ label, color, onClick }: AddStepButtonProps) {
  const colorClasses: Record<string, string> = {
    blue: 'border-blue-300 dark:border-blue-700 text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/20',
    green: 'border-green-300 dark:border-green-700 text-green-600 dark:text-green-400 hover:bg-green-50 dark:hover:bg-green-900/20',
    purple: 'border-purple-300 dark:border-purple-700 text-purple-600 dark:text-purple-400 hover:bg-purple-50 dark:hover:bg-purple-900/20',
    amber: 'border-amber-300 dark:border-amber-700 text-amber-600 dark:text-amber-400 hover:bg-amber-50 dark:hover:bg-amber-900/20',
  };

  return (
    <button
      onClick={onClick}
      className={clsx(
        'px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors',
        colorClasses[color] ?? colorClasses.blue
      )}
    >
      {label}
    </button>
  );
}

export default AgentComposer;
