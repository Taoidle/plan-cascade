/**
 * PRD Generation Form Component
 *
 * Form for entering task requirements with strategy suggestions
 * based on input complexity.
 */

import { useState, useRef, useEffect } from 'react';
import { clsx } from 'clsx';
import { usePRDStore } from '../../store/prd';
import { RocketIcon, MagicWandIcon, LayersIcon, CubeIcon } from '@radix-ui/react-icons';

interface StrategySuggestion {
  strategy: 'direct' | 'hybrid_auto' | 'mega_plan';
  label: string;
  description: string;
  icon: React.ReactNode;
  threshold: number; // character count threshold
}

const strategies: StrategySuggestion[] = [
  {
    strategy: 'direct',
    label: 'Direct',
    description: 'Simple, single-step task',
    icon: <RocketIcon className="w-4 h-4" />,
    threshold: 200,
  },
  {
    strategy: 'hybrid_auto',
    label: 'Hybrid Auto',
    description: 'Multi-step task with dependencies',
    icon: <LayersIcon className="w-4 h-4" />,
    threshold: 500,
  },
  {
    strategy: 'mega_plan',
    label: 'Mega Plan',
    description: 'Complex project with multiple features',
    icon: <CubeIcon className="w-4 h-4" />,
    threshold: Infinity,
  },
];

export function PRDGenerationForm() {
  const { generatePRD, isGenerating, generationError, setStrategy, prd } = usePRDStore();
  const [requirements, setRequirements] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Determine suggested strategy based on input length
  const suggestedStrategy = strategies.find((s) => requirements.length < s.threshold) || strategies[2];

  // Auto-resize textarea
  useEffect(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, 400)}px`;
    }
  }, [requirements]);

  const handleGenerate = async () => {
    if (!requirements.trim() || isGenerating) return;
    setStrategy(suggestedStrategy.strategy);
    await generatePRD(requirements);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleGenerate();
    }
  };

  return (
    <div className="space-y-6">
      {/* Requirements Input */}
      <div className="space-y-2">
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Task Requirements</label>
        <div
          className={clsx(
            'relative rounded-xl border-2 transition-colors',
            'bg-white dark:bg-gray-800',
            'border-gray-200 dark:border-gray-700',
            'focus-within:border-primary-500 dark:focus-within:border-primary-500',
          )}
        >
          <textarea
            ref={textareaRef}
            value={requirements}
            onChange={(e) => setRequirements(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={isGenerating}
            placeholder="Describe what you want to build in detail. Include features, requirements, and any specific constraints..."
            rows={6}
            className={clsx(
              'w-full p-4 rounded-xl resize-none',
              'bg-transparent',
              'text-gray-900 dark:text-white',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'focus:outline-none',
              'text-base leading-relaxed',
              isGenerating && 'opacity-50 cursor-not-allowed',
            )}
          />

          {/* Character count */}
          <div className="absolute bottom-3 right-3 text-xs text-gray-400">{requirements.length} characters</div>
        </div>
      </div>

      {/* Strategy Suggestions */}
      <div className="space-y-2">
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Suggested Strategy</label>
        <div className="grid grid-cols-3 gap-3">
          {strategies.map((strategy) => (
            <button
              key={strategy.strategy}
              onClick={() => setStrategy(strategy.strategy)}
              className={clsx(
                'relative p-4 rounded-lg border-2 transition-all text-left',
                strategy.strategy === (prd.strategy || suggestedStrategy.strategy)
                  ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                  : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 hover:border-gray-300 dark:hover:border-gray-600',
              )}
            >
              {strategy.strategy === suggestedStrategy.strategy && requirements.length > 0 && (
                <span className="absolute -top-2 -right-2 px-2 py-0.5 text-xs font-medium bg-primary-600 text-white rounded-full">
                  Recommended
                </span>
              )}
              <div className="flex items-center gap-2 mb-1">
                <span
                  className={clsx(
                    'text-gray-600 dark:text-gray-400',
                    strategy.strategy === (prd.strategy || suggestedStrategy.strategy) &&
                      'text-primary-600 dark:text-primary-400',
                  )}
                >
                  {strategy.icon}
                </span>
                <span
                  className={clsx(
                    'font-medium',
                    strategy.strategy === (prd.strategy || suggestedStrategy.strategy)
                      ? 'text-primary-700 dark:text-primary-300'
                      : 'text-gray-900 dark:text-white',
                  )}
                >
                  {strategy.label}
                </span>
              </div>
              <p className="text-xs text-gray-500 dark:text-gray-400">{strategy.description}</p>
            </button>
          ))}
        </div>
      </div>

      {/* Error Message */}
      {generationError && (
        <div className="p-4 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
          <p className="text-sm text-red-700 dark:text-red-300">{generationError}</p>
        </div>
      )}

      {/* Generate Button */}
      <button
        onClick={handleGenerate}
        disabled={!requirements.trim() || isGenerating}
        className={clsx(
          'w-full flex items-center justify-center gap-2 py-3 px-4 rounded-lg',
          'bg-primary-600 text-white font-medium',
          'hover:bg-primary-700',
          'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
          'dark:focus:ring-offset-gray-900',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          'transition-colors',
        )}
      >
        {isGenerating ? (
          <>
            <LoadingSpinner />
            <span>Generating PRD...</span>
          </>
        ) : (
          <>
            <MagicWandIcon className="w-5 h-5" />
            <span>Generate PRD</span>
          </>
        )}
      </button>

      {/* Help text */}
      <p className="text-sm text-gray-500 dark:text-gray-400 text-center">
        Press <kbd className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-xs">Cmd+Enter</kbd> to generate
      </p>
    </div>
  );
}

function LoadingSpinner() {
  return (
    <svg className="animate-spin h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );
}

export default PRDGenerationForm;
