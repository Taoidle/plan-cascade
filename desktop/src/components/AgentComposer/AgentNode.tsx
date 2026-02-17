/**
 * AgentNode Component
 *
 * Renders a single agent step as a configurable node in the Agent Composer.
 * Supports LLM, Sequential, Parallel, and Conditional step types.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import type { AgentStep, AgentConfig } from '../../types/agentComposer';
import { DEFAULT_AGENT_CONFIG, createLlmStep } from '../../types/agentComposer';

interface AgentNodeProps {
  step: AgentStep;
  index: number;
  onUpdate: (index: number, step: AgentStep) => void;
  onRemove: (index: number) => void;
}

export function AgentNode({ step, index, onUpdate, onRemove }: AgentNodeProps) {
  const [expanded, setExpanded] = useState(false);

  const typeLabel = getStepTypeLabel(step.step_type);
  const typeColor = getStepTypeColor(step.step_type);

  return (
    <div
      className={clsx(
        'rounded-lg border-2 p-4 transition-all',
        typeColor,
        'bg-white dark:bg-gray-800'
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span
            className={clsx(
              'px-2 py-0.5 rounded text-xs font-medium',
              getStepTypeBadgeColor(step.step_type)
            )}
          >
            {typeLabel}
          </span>
          <span className="font-medium text-gray-900 dark:text-white text-sm">
            {step.name}
          </span>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setExpanded(!expanded)}
            className="p-1 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 text-xs"
            title={expanded ? 'Collapse' : 'Expand'}
          >
            {expanded ? 'Collapse' : 'Edit'}
          </button>
          <button
            onClick={() => onRemove(index)}
            className="p-1 text-red-400 hover:text-red-600 text-xs"
            title="Remove step"
          >
            Remove
          </button>
        </div>
      </div>

      {/* Expanded configuration */}
      {expanded && (
        <div className="mt-3 pt-3 border-t border-gray-200 dark:border-gray-700 space-y-3">
          {/* Name field */}
          <div>
            <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
              Name
            </label>
            <input
              type="text"
              value={step.name}
              onChange={(e) =>
                onUpdate(index, { ...step, name: e.target.value })
              }
              className="w-full px-2 py-1 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            />
          </div>

          {/* LLM-specific fields */}
          {step.step_type === 'llm_step' && (
            <LlmStepFields
              step={step}
              index={index}
              onUpdate={onUpdate}
            />
          )}

          {/* Sequential/Parallel sub-steps indicator */}
          {(step.step_type === 'sequential_step' || step.step_type === 'parallel_step') && (
            <div className="text-xs text-gray-500 dark:text-gray-400">
              {step.steps.length} sub-step(s)
            </div>
          )}

          {/* Conditional branches indicator */}
          {step.step_type === 'conditional_step' && (
            <div className="text-xs text-gray-500 dark:text-gray-400">
              Condition key: {step.condition_key} | {Object.keys(step.branches).length} branch(es)
            </div>
          )}
        </div>
      )}
    </div>
  );
}

interface LlmStepFieldsProps {
  step: Extract<AgentStep, { step_type: 'llm_step' }>;
  index: number;
  onUpdate: (index: number, step: AgentStep) => void;
}

function LlmStepFields({ step, index, onUpdate }: LlmStepFieldsProps) {
  return (
    <>
      <div>
        <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
          Instruction
        </label>
        <textarea
          value={step.instruction ?? ''}
          onChange={(e) =>
            onUpdate(index, {
              ...step,
              instruction: e.target.value || null,
            })
          }
          rows={3}
          className="w-full px-2 py-1 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white resize-y"
          placeholder="System prompt for this agent..."
        />
      </div>
      <div>
        <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
          Model (optional)
        </label>
        <input
          type="text"
          value={step.model ?? ''}
          onChange={(e) =>
            onUpdate(index, { ...step, model: e.target.value || null })
          }
          className="w-full px-2 py-1 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          placeholder="Leave empty for default model"
        />
      </div>
      <div>
        <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
          Tools (comma-separated, optional)
        </label>
        <input
          type="text"
          value={step.tools?.join(', ') ?? ''}
          onChange={(e) => {
            const tools = e.target.value
              ? e.target.value.split(',').map((t) => t.trim()).filter(Boolean)
              : null;
            onUpdate(index, { ...step, tools });
          }}
          className="w-full px-2 py-1 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          placeholder="read_file, grep, bash (empty = all tools)"
        />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div>
          <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
            Max Iterations
          </label>
          <input
            type="number"
            value={step.config.max_iterations}
            onChange={(e) =>
              onUpdate(index, {
                ...step,
                config: { ...step.config, max_iterations: parseInt(e.target.value) || 50 },
              })
            }
            min={1}
            max={200}
            className="w-full px-2 py-1 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
            Temperature
          </label>
          <input
            type="number"
            value={step.config.temperature ?? ''}
            onChange={(e) =>
              onUpdate(index, {
                ...step,
                config: {
                  ...step.config,
                  temperature: e.target.value ? parseFloat(e.target.value) : null,
                },
              })
            }
            step={0.1}
            min={0}
            max={2}
            className="w-full px-2 py-1 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
            placeholder="Default"
          />
        </div>
      </div>
    </>
  );
}

function getStepTypeLabel(type_: string): string {
  switch (type_) {
    case 'llm_step': return 'LLM';
    case 'sequential_step': return 'Sequential';
    case 'parallel_step': return 'Parallel';
    case 'conditional_step': return 'Conditional';
    default: return type_;
  }
}

function getStepTypeColor(type_: string): string {
  switch (type_) {
    case 'llm_step': return 'border-blue-300 dark:border-blue-700';
    case 'sequential_step': return 'border-green-300 dark:border-green-700';
    case 'parallel_step': return 'border-purple-300 dark:border-purple-700';
    case 'conditional_step': return 'border-amber-300 dark:border-amber-700';
    default: return 'border-gray-300 dark:border-gray-700';
  }
}

function getStepTypeBadgeColor(type_: string): string {
  switch (type_) {
    case 'llm_step': return 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300';
    case 'sequential_step': return 'bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300';
    case 'parallel_step': return 'bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-300';
    case 'conditional_step': return 'bg-amber-100 dark:bg-amber-900 text-amber-700 dark:text-amber-300';
    default: return 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300';
  }
}

export default AgentNode;
