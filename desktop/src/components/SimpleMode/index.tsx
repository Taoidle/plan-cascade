/**
 * SimpleMode Component
 *
 * Container for Simple mode interface.
 * Provides a streamlined experience with:
 * - Single input for task description
 * - Automatic strategy selection
 * - Progress visualization with real-time streaming output
 * - Results display with error feedback
 * - Execution history
 *
 * Story 008: Added StreamingOutput, GlobalProgressBar, and ErrorState
 */

import { useEffect, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { InputBox } from './InputBox';
import { ProgressView } from './ProgressView';
import { ResultView } from './ResultView';
import { HistoryPanel } from './HistoryPanel';
import { ConnectionStatus } from './ConnectionStatus';
import { useExecutionStore } from '../../store/execution';
import { StreamingOutput, GlobalProgressBar, ErrorState } from '../shared';

export function SimpleMode() {
  const { t } = useTranslation('simpleMode');
  const {
    status,
    connectionStatus,
    isSubmitting,
    apiError,
    start,
    reset,
    result,
    initialize,
    cleanup,
    analyzeStrategy,
    strategyAnalysis,
    isAnalyzingStrategy,
    clearStrategyAnalysis,
  } = useExecutionStore();
  const [description, setDescription] = useState('');
  const [showHistory, setShowHistory] = useState(false);

  // Initialize WebSocket connection on mount
  useEffect(() => {
    initialize();
    return () => {
      cleanup();
    };
  }, [initialize, cleanup]);

  const handleStart = async () => {
    if (!description.trim() || isSubmitting || isAnalyzingStrategy) return;

    // Step 1: Analyze strategy automatically
    const analysis = await analyzeStrategy(description);

    // Step 2: Start execution with the auto-selected strategy
    if (analysis) {
      await start(description, 'simple');
    }
  };

  const handleNewTask = () => {
    reset();
    clearStrategyAnalysis();
    setDescription('');
  };

  const isRunning = status === 'running' || status === 'paused';
  const isCompleted = status === 'completed' || status === 'failed';
  const isDisabled = isRunning || isSubmitting || isAnalyzingStrategy;

  return (
    <div className="h-full flex flex-col p-6 3xl:p-8 5xl:p-10">
      {/* Header with Connection Status and History Toggle */}
      <div className="flex items-center justify-between mb-4 max-w-2xl 3xl:max-w-3xl 5xl:max-w-4xl mx-auto w-full">
        <ConnectionStatus status={connectionStatus} />
        <button
          onClick={() => setShowHistory(!showHistory)}
          className={clsx(
            'text-sm px-3 py-1.5 rounded-lg transition-colors',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
            showHistory && 'bg-gray-100 dark:bg-gray-800'
          )}
        >
          {t('history.button')}
        </button>
      </div>

      {/* Input Area */}
      <div className="max-w-2xl 3xl:max-w-3xl 5xl:max-w-4xl mx-auto w-full">
        <InputBox
          value={description}
          onChange={setDescription}
          onSubmit={handleStart}
          disabled={isDisabled}
          placeholder={t('input.placeholder')}
          isLoading={isSubmitting}
        />

        {/* API Error Message */}
        {apiError && (
          <div className="mt-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
            <p className="text-sm text-red-600 dark:text-red-400">
              {apiError}
            </p>
          </div>
        )}
      </div>

      {/* Strategy Analysis Banner */}
      {(isAnalyzingStrategy || strategyAnalysis) && (
        <div className="max-w-2xl mx-auto w-full mt-4">
          {isAnalyzingStrategy ? (
            <div className="p-3 rounded-lg bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 flex items-center gap-2">
              <div className="animate-spin h-4 w-4 border-2 border-blue-500 border-t-transparent rounded-full" />
              <p className="text-sm text-blue-600 dark:text-blue-400">
                {t('strategy.analyzing', { defaultValue: 'Analyzing task complexity...' })}
              </p>
            </div>
          ) : strategyAnalysis && (
            <div className="p-3 rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
              <div className="flex items-center justify-between">
                <p className="text-sm font-medium text-green-700 dark:text-green-300">
                  {t('strategy.selected', { defaultValue: 'Strategy' })}:{' '}
                  <span className="font-semibold">{strategyAnalysis.strategy.replace(/_/g, ' ')}</span>
                  <span className="ml-2 text-xs text-green-600 dark:text-green-400">
                    ({(strategyAnalysis.confidence * 100).toFixed(0)}% confidence)
                  </span>
                </p>
              </div>
              <p className="text-xs text-green-600 dark:text-green-400 mt-1">
                {strategyAnalysis.reasoning}
              </p>
            </div>
          )}
        </div>
      )}

      {/* Progress/Results/History Area */}
      <div className="flex-1 mt-8 overflow-auto">
        {showHistory ? (
          <HistoryPanel onClose={() => setShowHistory(false)} />
        ) : (
          <>
            {isRunning && (
              <div className="max-w-2xl 3xl:max-w-3xl 5xl:max-w-4xl mx-auto w-full space-y-4">
                {/* Global progress bar at the top */}
                <GlobalProgressBar compact showStoryLabels={false} />

                {/* Traditional progress view with stories */}
                <ProgressView />

                {/* Real-time streaming output */}
                <StreamingOutput
                  maxHeight="250px"
                  compact
                  showClear={false}
                />

                {/* Inline error states */}
                <ErrorState maxErrors={3} />
              </div>
            )}
            {isCompleted && (
              <div className="max-w-2xl 3xl:max-w-3xl 5xl:max-w-4xl mx-auto w-full space-y-4">
                {/* Error states for failed executions */}
                <ErrorState maxErrors={5} />

                <ResultView result={result} />
                <div className="flex justify-center">
                  <button
                    onClick={handleNewTask}
                    className={clsx(
                      'px-4 py-2 rounded-lg text-sm font-medium',
                      'bg-primary-600 text-white',
                      'hover:bg-primary-700',
                      'transition-colors'
                    )}
                  >
                    {t('buttons.startNewTask', { ns: 'common' })}
                  </button>
                </div>
              </div>
            )}

            {/* Empty state when idle */}
            {status === 'idle' && !apiError && (
              <div className="flex flex-col items-center justify-center h-full text-center">
                <div className="text-6xl mb-4">
                  <span role="img" aria-label="rocket">&#128640;</span>
                </div>
                <h2 className="text-xl font-semibold text-gray-700 dark:text-gray-300 mb-2">
                  {t('empty.title')}
                </h2>
                <p className="text-gray-500 dark:text-gray-400 max-w-md">
                  {t('empty.description')}
                </p>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

export default SimpleMode;
