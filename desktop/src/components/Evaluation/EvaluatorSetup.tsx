/**
 * EvaluatorSetup Component
 *
 * Manages evaluator definitions: list existing evaluators,
 * create new ones, and configure evaluation criteria
 * (tool trajectory, response similarity, LLM judge).
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useEvaluationStore } from '../../store/evaluation';
import type { ToolTrajectoryConfig, ResponseSimilarityConfig, LlmJudgeConfig } from '../../types/evaluation';

export function EvaluatorSetup() {
  const {
    evaluators,
    currentEvaluator,
    isCreatingEvaluator,
    loading,
    startNewEvaluator,
    selectEvaluator,
    updateCurrentEvaluator,
    updateCriteria,
    saveEvaluator,
    removeEvaluator,
    clearCurrentEvaluator,
  } = useEvaluationStore();

  return (
    <div className="h-full flex">
      {/* Left sidebar: Evaluator list */}
      <div
        className={clsx(
          'w-64 min-w-[16rem] p-4 overflow-auto',
          'border-r border-gray-200 dark:border-gray-700',
          'bg-gray-50 dark:bg-gray-900'
        )}
      >
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-sm font-semibold text-gray-900 dark:text-white">
            Evaluators
          </h3>
          <button
            onClick={startNewEvaluator}
            className="px-2 py-1 text-xs font-medium rounded bg-primary-600 text-white hover:bg-primary-700 transition-colors"
          >
            + New
          </button>
        </div>

        {loading.evaluators ? (
          <div className="text-xs text-gray-500 dark:text-gray-400">Loading...</div>
        ) : evaluators.length === 0 ? (
          <div className="text-xs text-gray-500 dark:text-gray-400 italic">
            No evaluators yet
          </div>
        ) : (
          <div className="space-y-1">
            {evaluators.map((ev) => (
              <div
                key={ev.id}
                onClick={() => selectEvaluator(ev)}
                className={clsx(
                  'p-2 rounded cursor-pointer transition-colors',
                  currentEvaluator?.id === ev.id
                    ? 'bg-primary-100 dark:bg-primary-900/30 border border-primary-300 dark:border-primary-700'
                    : 'hover:bg-gray-100 dark:hover:bg-gray-800'
                )}
              >
                <div className="text-sm font-medium text-gray-900 dark:text-white truncate">
                  {ev.name}
                </div>
                <div className="flex gap-1 mt-1">
                  {ev.has_tool_trajectory && (
                    <span className="text-[10px] px-1 py-0.5 rounded bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400">
                      Trajectory
                    </span>
                  )}
                  {ev.has_response_similarity && (
                    <span className="text-[10px] px-1 py-0.5 rounded bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-400">
                      Similarity
                    </span>
                  )}
                  {ev.has_llm_judge && (
                    <span className="text-[10px] px-1 py-0.5 rounded bg-purple-100 dark:bg-purple-900/30 text-purple-600 dark:text-purple-400">
                      LLM Judge
                    </span>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Main content: Editor */}
      <div className="flex-1 overflow-auto p-6">
        {currentEvaluator ? (
          <div className="max-w-2xl mx-auto space-y-6">
            {/* Header */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
                  {isCreatingEvaluator ? 'New Evaluator' : 'Edit Evaluator'}
                </h2>
                {isCreatingEvaluator && (
                  <span className="text-xs text-primary-600 dark:text-primary-400 font-medium">
                    NEW
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={clearCurrentEvaluator}
                  className="px-3 py-1.5 text-xs font-medium rounded-lg bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                >
                  Close
                </button>
                {!isCreatingEvaluator && (
                  <button
                    onClick={() => removeEvaluator(currentEvaluator.id)}
                    className="px-3 py-1.5 text-xs font-medium rounded-lg border border-red-300 dark:border-red-700 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                  >
                    Delete
                  </button>
                )}
                <button
                  onClick={saveEvaluator}
                  disabled={loading.save || !currentEvaluator.name.trim()}
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

            {/* Name */}
            <div>
              <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
                Evaluator Name
              </label>
              <input
                type="text"
                value={currentEvaluator.name}
                onChange={(e) => updateCurrentEvaluator({ name: e.target.value })}
                className="w-full text-sm px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
                placeholder="e.g., Code Quality Check"
              />
            </div>

            {/* Criteria Sections */}
            <div className="space-y-4">
              <h3 className="text-sm font-semibold text-gray-900 dark:text-white">
                Evaluation Criteria
              </h3>

              {/* Tool Trajectory */}
              <CriteriaSection
                title="Tool Trajectory"
                description="Compare tool calls against expected tools"
                color="blue"
                enabled={currentEvaluator.criteria.tool_trajectory !== null && currentEvaluator.criteria.tool_trajectory !== undefined}
                onToggle={(enabled) => {
                  updateCriteria({
                    tool_trajectory: enabled ? { expected_tools: [], order_matters: false } : null,
                  });
                }}
              >
                {currentEvaluator.criteria.tool_trajectory && (
                  <ToolTrajectoryEditor
                    config={currentEvaluator.criteria.tool_trajectory}
                    onChange={(config) => updateCriteria({ tool_trajectory: config })}
                  />
                )}
              </CriteriaSection>

              {/* Response Similarity */}
              <CriteriaSection
                title="Response Similarity"
                description="Compare response against a reference using string distance"
                color="green"
                enabled={currentEvaluator.criteria.response_similarity !== null && currentEvaluator.criteria.response_similarity !== undefined}
                onToggle={(enabled) => {
                  updateCriteria({
                    response_similarity: enabled ? { reference_response: '', threshold: 0.8 } : null,
                  });
                }}
              >
                {currentEvaluator.criteria.response_similarity && (
                  <ResponseSimilarityEditor
                    config={currentEvaluator.criteria.response_similarity}
                    onChange={(config) => updateCriteria({ response_similarity: config })}
                  />
                )}
              </CriteriaSection>

              {/* LLM Judge */}
              <CriteriaSection
                title="LLM Judge"
                description="Use a separate LLM to judge response quality"
                color="purple"
                enabled={currentEvaluator.criteria.llm_judge !== null && currentEvaluator.criteria.llm_judge !== undefined}
                onToggle={(enabled) => {
                  updateCriteria({
                    llm_judge: enabled ? { judge_model: '', judge_provider: 'anthropic', rubric: '' } : null,
                  });
                }}
              >
                {currentEvaluator.criteria.llm_judge && (
                  <LlmJudgeEditor
                    config={currentEvaluator.criteria.llm_judge}
                    onChange={(config) => updateCriteria({ llm_judge: config })}
                  />
                )}
              </CriteriaSection>
            </div>
          </div>
        ) : (
          /* Empty state */
          <div className="flex-1 flex items-center justify-center h-full">
            <div className="text-center">
              <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-2">
                Evaluation Setup
              </h2>
              <p className="text-gray-500 dark:text-gray-400 mb-4 max-w-md">
                Define evaluators with criteria to assess agent performance
                across different models.
              </p>
              <button
                onClick={startNewEvaluator}
                className="px-4 py-2 text-sm font-medium rounded-lg bg-primary-600 text-white hover:bg-primary-700 transition-colors"
              >
                Create Your First Evaluator
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Sub-Components
// ============================================================================

interface CriteriaSectionProps {
  title: string;
  description: string;
  color: string;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
  children: React.ReactNode;
}

function CriteriaSection({ title, description, color, enabled, onToggle, children }: CriteriaSectionProps) {
  const colorClasses: Record<string, string> = {
    blue: 'border-blue-200 dark:border-blue-800',
    green: 'border-green-200 dark:border-green-800',
    purple: 'border-purple-200 dark:border-purple-800',
  };

  return (
    <div
      className={clsx(
        'rounded-lg border p-4',
        enabled ? colorClasses[color] : 'border-gray-200 dark:border-gray-700',
        enabled ? 'bg-white dark:bg-gray-800/50' : 'bg-gray-50 dark:bg-gray-900'
      )}
    >
      <div className="flex items-center justify-between mb-2">
        <div>
          <h4 className="text-sm font-medium text-gray-900 dark:text-white">{title}</h4>
          <p className="text-xs text-gray-500 dark:text-gray-400">{description}</p>
        </div>
        <label className="relative inline-flex items-center cursor-pointer">
          <input
            type="checkbox"
            checked={enabled}
            onChange={(e) => onToggle(e.target.checked)}
            className="sr-only peer"
          />
          <div className="w-9 h-5 bg-gray-200 dark:bg-gray-700 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-primary-600" />
        </label>
      </div>
      {enabled && <div className="mt-3 pt-3 border-t border-gray-100 dark:border-gray-700/50">{children}</div>}
    </div>
  );
}

function ToolTrajectoryEditor({
  config,
  onChange,
}: {
  config: ToolTrajectoryConfig;
  onChange: (config: ToolTrajectoryConfig) => void;
}) {
  const [newTool, setNewTool] = useState('');

  return (
    <div className="space-y-3">
      <div>
        <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
          Expected Tools
        </label>
        <div className="flex flex-wrap gap-1 mb-2">
          {config.expected_tools.map((tool, i) => (
            <span
              key={i}
              className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900/30 text-xs text-blue-700 dark:text-blue-300"
            >
              {tool}
              <button
                onClick={() => {
                  const tools = config.expected_tools.filter((_, idx) => idx !== i);
                  onChange({ ...config, expected_tools: tools });
                }}
                className="text-blue-400 hover:text-blue-600"
              >
                x
              </button>
            </span>
          ))}
        </div>
        <div className="flex gap-1">
          <input
            type="text"
            value={newTool}
            onChange={(e) => setNewTool(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && newTool.trim()) {
                onChange({ ...config, expected_tools: [...config.expected_tools, newTool.trim()] });
                setNewTool('');
              }
            }}
            className="flex-1 text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800"
            placeholder="Tool name (press Enter to add)"
          />
          <button
            onClick={() => {
              if (newTool.trim()) {
                onChange({ ...config, expected_tools: [...config.expected_tools, newTool.trim()] });
                setNewTool('');
              }
            }}
            disabled={!newTool.trim()}
            className="px-2 py-1 text-xs rounded bg-blue-600 text-white disabled:opacity-50"
          >
            +
          </button>
        </div>
      </div>
      <label className="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300">
        <input
          type="checkbox"
          checked={config.order_matters}
          onChange={(e) => onChange({ ...config, order_matters: e.target.checked })}
          className="rounded"
        />
        Order matters
      </label>
    </div>
  );
}

function ResponseSimilarityEditor({
  config,
  onChange,
}: {
  config: ResponseSimilarityConfig;
  onChange: (config: ResponseSimilarityConfig) => void;
}) {
  return (
    <div className="space-y-3">
      <div>
        <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
          Reference Response
        </label>
        <textarea
          value={config.reference_response}
          onChange={(e) => onChange({ ...config, reference_response: e.target.value })}
          className="w-full text-sm px-2 py-1.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white h-20 resize-y"
          placeholder="Expected response to compare against..."
        />
      </div>
      <div>
        <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
          Threshold ({(config.threshold * 100).toFixed(0)}%)
        </label>
        <input
          type="range"
          min={0}
          max={100}
          value={config.threshold * 100}
          onChange={(e) => onChange({ ...config, threshold: parseInt(e.target.value) / 100 })}
          className="w-full"
        />
      </div>
    </div>
  );
}

function LlmJudgeEditor({
  config,
  onChange,
}: {
  config: LlmJudgeConfig;
  onChange: (config: LlmJudgeConfig) => void;
}) {
  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
            Judge Provider
          </label>
          <select
            value={config.judge_provider}
            onChange={(e) => onChange({ ...config, judge_provider: e.target.value })}
            className="w-full text-sm px-2 py-1.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
          >
            <option value="anthropic">Anthropic</option>
            <option value="openai">OpenAI</option>
            <option value="deepseek">DeepSeek</option>
            <option value="ollama">Ollama</option>
          </select>
        </div>
        <div>
          <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
            Judge Model
          </label>
          <input
            type="text"
            value={config.judge_model}
            onChange={(e) => onChange({ ...config, judge_model: e.target.value })}
            className="w-full text-sm px-2 py-1.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white"
            placeholder="e.g., claude-sonnet-4-20250514"
          />
        </div>
      </div>
      <div>
        <label className="text-xs text-gray-500 dark:text-gray-400 block mb-1">
          Rubric
        </label>
        <textarea
          value={config.rubric}
          onChange={(e) => onChange({ ...config, rubric: e.target.value })}
          className="w-full text-sm px-2 py-1.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-900 dark:text-white h-24 resize-y"
          placeholder="Describe how to evaluate the response (0.0-1.0 scale)..."
        />
      </div>
    </div>
  );
}
