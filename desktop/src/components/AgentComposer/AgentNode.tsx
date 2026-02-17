/**
 * AgentNode Component
 *
 * Renders a single agent step as a configurable node in the Agent Composer.
 * Supports LLM, Sequential, Parallel, and Conditional step types.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { AgentStep } from '../../types/agentComposer';

interface AgentNodeProps {
  step: AgentStep;
  index: number;
  onUpdate: (index: number, step: AgentStep) => void;
  onRemove: (index: number) => void;
}

export function AgentNode({ step, index, onUpdate, onRemove }: AgentNodeProps) {
  const { t } = useTranslation('expertMode');
  const [expanded, setExpanded] = useState(false);

  const typeLabel = getStepTypeLabel(step.step_type, t);
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
            title={expanded ? t('agentComposer.node.collapse') : t('agentComposer.node.expand')}
          >
            {expanded ? t('agentComposer.node.collapse') : t('agentComposer.node.edit')}
          </button>
          <button
            onClick={() => onRemove(index)}
            className="p-1 text-red-400 hover:text-red-600 text-xs"
            title={t('agentComposer.node.removeStep')}
          >
            {t('agentComposer.node.remove')}
          </button>
        </div>
      </div>

      {/* Expanded configuration */}
      {expanded && (
        <div className="mt-3 pt-3 border-t border-gray-200 dark:border-gray-700 space-y-3">
          {/* Name field */}
          <div>
            <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
              {t('agentComposer.node.name')}
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
              {step.steps.length} {t('agentComposer.node.subSteps')}
            </div>
          )}

          {/* Conditional branches indicator */}
          {step.step_type === 'conditional_step' && (
            <div className="text-xs text-gray-500 dark:text-gray-400">
              {t('agentComposer.node.conditionKey')} {step.condition_key} | {Object.keys(step.branches).length} {t('agentComposer.node.branches')}
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
  const { t } = useTranslation('expertMode');

  return (
    <>
      <div>
        <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
          {t('agentComposer.node.instruction')}
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
          placeholder={t('agentComposer.node.instructionPlaceholder')}
        />
      </div>
      <div>
        <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
          {t('agentComposer.node.model')}
        </label>
        <input
          type="text"
          value={step.model ?? ''}
          onChange={(e) =>
            onUpdate(index, { ...step, model: e.target.value || null })
          }
          className="w-full px-2 py-1 text-sm rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          placeholder={t('agentComposer.node.modelPlaceholder')}
        />
      </div>
      <div>
        <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
          {t('agentComposer.node.tools')}
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
          placeholder={t('agentComposer.node.toolsPlaceholder')}
        />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div>
          <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
            {t('agentComposer.node.maxIterations')}
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
            {t('agentComposer.node.temperature')}
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
            placeholder={t('agentComposer.node.default')}
          />
        </div>
      </div>
    </>
  );
}

function getStepTypeLabel(type_: string, t: (key: string) => string): string {
  switch (type_) {
    case 'llm_step': return t('agentComposer.node.typeLlm');
    case 'sequential_step': return t('agentComposer.node.typeSequential');
    case 'parallel_step': return t('agentComposer.node.typeParallel');
    case 'conditional_step': return t('agentComposer.node.typeConditional');
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
