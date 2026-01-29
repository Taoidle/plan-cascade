/**
 * Strategy Selector Component
 *
 * Radio button group for selecting execution strategy
 * with descriptions and recommendations.
 */

import { clsx } from 'clsx';
import { usePRDStore, ExecutionStrategy } from '../../store/prd';
import * as Tooltip from '@radix-ui/react-tooltip';
import {
  RocketIcon,
  LayersIcon,
  CubeIcon,
  InfoCircledIcon,
} from '@radix-ui/react-icons';

interface StrategyOption {
  value: ExecutionStrategy;
  label: string;
  description: string;
  details: string;
  icon: React.ReactNode;
  minStories: number;
  maxStories: number;
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
  },
  {
    value: 'hybrid_auto',
    label: 'Hybrid Auto',
    description: 'Automatic PRD generation with story-based execution',
    details: 'Ideal for medium-sized tasks. The system generates a PRD with multiple stories and executes them in dependency order. Provides good balance of structure and efficiency.',
    icon: <LayersIcon className="w-5 h-5" />,
    minStories: 2,
    maxStories: 10,
  },
  {
    value: 'mega_plan',
    label: 'Mega Plan',
    description: 'Full project planning with feature breakdown',
    details: 'For complex, multi-feature projects. Creates a comprehensive plan with features, stories, and dependencies. Uses parallel execution with Git worktrees for isolation.',
    icon: <CubeIcon className="w-5 h-5" />,
    minStories: 10,
    maxStories: Infinity,
  },
];

export function StrategySelector() {
  const { prd, setStrategy } = usePRDStore();
  const storyCount = prd.stories.length;

  // Determine recommended strategy based on story count
  const recommendedStrategy = strategyOptions.find(
    (opt) => storyCount >= opt.minStories && storyCount <= opt.maxStories
  )?.value || 'hybrid_auto';

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

      <div className="space-y-2">
        {strategyOptions.map((option) => {
          const isSelected = prd.strategy === option.value;
          const isRecommended = option.value === recommendedStrategy && storyCount > 0;

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
                  {isRecommended && (
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

export default StrategySelector;
