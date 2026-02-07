/**
 * Strategy Selector Component
 *
 * Radio button group for selecting execution strategy
 * with descriptions, recommendations, and AI-powered analysis.
 * In Expert mode, displays the auto-analyzer recommendation
 * with confidence score and allows manual override.
 */

import { useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { usePRDStore, ExecutionStrategy } from '../../store/prd';
import { useExecutionStore, StrategyAnalysis } from '../../store/execution';
import * as Tooltip from '@radix-ui/react-tooltip';
import {
  RocketIcon,
  LayersIcon,
  CubeIcon,
  InfoCircledIcon,
  MixIcon,
} from '@radix-ui/react-icons';

interface StrategyOption {
  value: ExecutionStrategy;
  label: string;
  description: string;
  details: string;
  icon: React.ReactNode;
  minStories: number;
  maxStories: number;
  /** Strategy key used by the Rust analyzer */
  analyzerKey: string;
}

const strategyOptions: StrategyOption[] = [
  {
    value: 'direct',
    label: 'Direct',
    description: 'Execute task directly without PRD breakdown',
    details: 'Best for simple, single-step tasks that can be completed in one pass. The AI will execute the task immediately without creating intermediate stories.',
    icon: <RocketIcon className="w-5 h-5" />,
    minStories: 0,
    maxStories: 1,
    analyzerKey: 'direct',
  },
  {
    value: 'hybrid_auto',
    label: 'Hybrid Auto',
    description: 'Automatic PRD generation with story-based execution',
    details: 'Ideal for medium-sized tasks. The system generates a PRD with multiple stories and executes them in dependency order. Provides good balance of structure and efficiency.',
    icon: <LayersIcon className="w-5 h-5" />,
    minStories: 2,
    maxStories: 10,
    analyzerKey: 'hybrid_auto',
  },
  {
    value: 'mega_plan',
    label: 'Mega Plan',
    description: 'Full project planning with feature breakdown',
    details: 'For complex, multi-feature projects. Creates a comprehensive plan with features, stories, and dependencies. Uses parallel execution with Git worktrees for isolation.',
    icon: <CubeIcon className="w-5 h-5" />,
    minStories: 10,
    maxStories: Infinity,
    analyzerKey: 'mega_plan',
  },
];

/** Map analyzer strategy key to the PRD store ExecutionStrategy value */
function mapAnalyzerStrategy(analyzerStrategy: string): ExecutionStrategy | null {
  switch (analyzerStrategy) {
    case 'direct': return 'direct';
    case 'hybrid_auto': return 'hybrid_auto';
    case 'hybrid_worktree': return 'hybrid_auto'; // Worktree mapped to hybrid_auto in PRD store
    case 'mega_plan': return 'mega_plan';
    default: return null;
  }
}

/** Format strategy name for display */
function formatStrategyName(strategy: string): string {
  return strategy
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Confidence level label */
function confidenceLabel(confidence: number): { text: string; color: string } {
  if (confidence >= 0.8) return { text: 'High', color: 'text-green-600 dark:text-green-400' };
  if (confidence >= 0.6) return { text: 'Medium', color: 'text-yellow-600 dark:text-yellow-400' };
  return { text: 'Low', color: 'text-red-600 dark:text-red-400' };
}

interface StrategySelectorProps {
  /** Task description for auto-analysis (optional) */
  taskDescription?: string;
}

export function StrategySelector({ taskDescription }: StrategySelectorProps) {
  const { prd, setStrategy } = usePRDStore();
  const {
    strategyAnalysis,
    isAnalyzingStrategy,
    analyzeStrategy,
  } = useExecutionStore();
  const storyCount = prd.stories.length;

  // Auto-analyze when task description changes (debounced)
  const runAnalysis = useCallback(async () => {
    if (taskDescription && taskDescription.trim().length > 10) {
      await analyzeStrategy(taskDescription);
    }
  }, [taskDescription, analyzeStrategy]);

  useEffect(() => {
    const timer = setTimeout(runAnalysis, 500);
    return () => clearTimeout(timer);
  }, [runAnalysis]);

  // Determine recommended strategy: use analyzer result if available, else story count heuristic
  const analyzerRecommendation = strategyAnalysis
    ? mapAnalyzerStrategy(strategyAnalysis.strategy)
    : null;

  const recommendedStrategy = analyzerRecommendation
    || strategyOptions.find(
      (opt) => storyCount >= opt.minStories && storyCount <= opt.maxStories
    )?.value
    || 'hybrid_auto';

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
          Execution Strategy
        </label>
        {storyCount > 0 && (
          <span className="text-xs text-gray-500 dark:text-gray-400">
            {storyCount} {storyCount === 1 ? 'story' : 'stories'}
          </span>
        )}
      </div>

      {/* AI Analysis Banner */}
      {(isAnalyzingStrategy || strategyAnalysis) && (
        <AnalysisBanner
          analysis={strategyAnalysis}
          isAnalyzing={isAnalyzingStrategy}
        />
      )}

      <div className="space-y-2">
        {strategyOptions.map((option) => {
          const isSelected = prd.strategy === option.value;
          const isRecommended = option.value === recommendedStrategy;
          const isAnalyzerPick = analyzerRecommendation === option.value;

          return (
            <label
              key={option.value}
              className={clsx(
                'relative flex items-start gap-3 p-4 rounded-lg border-2 cursor-pointer transition-all',
                isSelected
                  ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                  : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 hover:border-gray-300 dark:hover:border-gray-600'
              )}
            >
              <input
                type="radio"
                name="strategy"
                value={option.value}
                checked={isSelected}
                onChange={() => setStrategy(option.value)}
                className="sr-only"
              />

              {/* Radio indicator */}
              <div
                className={clsx(
                  'w-5 h-5 rounded-full border-2 flex items-center justify-center mt-0.5 shrink-0',
                  isSelected
                    ? 'border-primary-600 bg-primary-600'
                    : 'border-gray-300 dark:border-gray-600'
                )}
              >
                {isSelected && (
                  <div className="w-2 h-2 rounded-full bg-white" />
                )}
              </div>

              {/* Icon */}
              <div
                className={clsx(
                  'w-10 h-10 rounded-lg flex items-center justify-center shrink-0',
                  isSelected
                    ? 'bg-primary-100 dark:bg-primary-800 text-primary-600 dark:text-primary-400'
                    : 'bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400'
                )}
              >
                {option.icon}
              </div>

              {/* Content */}
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span
                    className={clsx(
                      'font-medium',
                      isSelected
                        ? 'text-primary-700 dark:text-primary-300'
                        : 'text-gray-900 dark:text-white'
                    )}
                  >
                    {option.label}
                  </span>
                  {isAnalyzerPick && strategyAnalysis && (
                    <span className="px-2 py-0.5 text-xs font-medium rounded-full bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300 flex items-center gap-1">
                      <MixIcon className="w-3 h-3" />
                      AI Pick ({(strategyAnalysis.confidence * 100).toFixed(0)}%)
                    </span>
                  )}
                  {isRecommended && !isAnalyzerPick && storyCount > 0 && (
                    <span className="px-2 py-0.5 text-xs font-medium rounded-full bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300">
                      Recommended
                    </span>
                  )}
                  <Tooltip.Provider>
                    <Tooltip.Root>
                      <Tooltip.Trigger asChild>
                        <button className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">
                          <InfoCircledIcon className="w-4 h-4" />
                        </button>
                      </Tooltip.Trigger>
                      <Tooltip.Portal>
                        <Tooltip.Content
                          className={clsx(
                            'max-w-xs px-3 py-2 rounded-lg text-sm',
                            'bg-gray-900 dark:bg-gray-700 text-white',
                            'shadow-lg'
                          )}
                          sideOffset={5}
                        >
                          {option.details}
                          <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-700" />
                        </Tooltip.Content>
                      </Tooltip.Portal>
                    </Tooltip.Root>
                  </Tooltip.Provider>
                </div>
                <p className="text-sm text-gray-500 dark:text-gray-400 mt-0.5">
                  {option.description}
                </p>
              </div>
            </label>
          );
        })}
      </div>
    </div>
  );
}

/** Banner showing the AI analysis result with dimension scores */
function AnalysisBanner({
  analysis,
  isAnalyzing,
}: {
  analysis: StrategyAnalysis | null;
  isAnalyzing: boolean;
}) {
  if (isAnalyzing) {
    return (
      <div className="p-3 rounded-lg bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 flex items-center gap-2">
        <div className="animate-spin h-4 w-4 border-2 border-blue-500 border-t-transparent rounded-full" />
        <p className="text-sm text-blue-600 dark:text-blue-400">
          Analyzing task complexity...
        </p>
      </div>
    );
  }

  if (!analysis) return null;

  const { text: confLabel, color: confColor } = confidenceLabel(analysis.confidence);

  return (
    <div className="p-3 rounded-lg bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          <MixIcon className="w-4 h-4 text-indigo-600 dark:text-indigo-400" />
          <span className="text-sm font-medium text-indigo-700 dark:text-indigo-300">
            AI Recommendation: {formatStrategyName(analysis.strategy)}
          </span>
        </div>
        <span className={clsx('text-xs font-medium', confColor)}>
          {confLabel} confidence ({(analysis.confidence * 100).toFixed(0)}%)
        </span>
      </div>

      <p className="text-xs text-indigo-600 dark:text-indigo-400 mb-2">
        {analysis.reasoning}
      </p>

      {/* Dimension scores bar chart */}
      <div className="grid grid-cols-4 gap-2">
        {[
          { label: 'Scope', value: analysis.dimension_scores.scope },
          { label: 'Complexity', value: analysis.dimension_scores.complexity },
          { label: 'Risk', value: analysis.dimension_scores.risk },
          { label: 'Parallel', value: analysis.dimension_scores.parallelization },
        ].map((dim) => (
          <div key={dim.label}>
            <div className="flex items-center justify-between mb-0.5">
              <span className="text-[10px] text-indigo-500 dark:text-indigo-400">{dim.label}</span>
              <span className="text-[10px] text-indigo-500 dark:text-indigo-400">
                {(dim.value * 100).toFixed(0)}%
              </span>
            </div>
            <div className="h-1.5 rounded-full bg-indigo-100 dark:bg-indigo-800 overflow-hidden">
              <div
                className="h-full rounded-full bg-indigo-500 dark:bg-indigo-400 transition-all"
                style={{ width: `${dim.value * 100}%` }}
              />
            </div>
          </div>
        ))}
      </div>

      {/* Estimates */}
      <div className="flex gap-4 mt-2 text-[10px] text-indigo-500 dark:text-indigo-400">
        <span>~{analysis.estimated_stories} stories</span>
        {analysis.estimated_features > 1 && (
          <span>~{analysis.estimated_features} features</span>
        )}
        <span>~{analysis.estimated_duration_hours}h estimated</span>
      </div>
    </div>
  );
}

export default StrategySelector;
