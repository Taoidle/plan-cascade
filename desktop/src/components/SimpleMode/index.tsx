/**
 * SimpleMode Component
 *
 * Container for Simple mode interface.
 * Provides a streamlined experience with:
 * - Single input for task description
 * - Automatic strategy selection
 * - Progress visualization
 * - Results display
 * - Execution history
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
    if (!description.trim() || isSubmitting) return;
    await start(description, 'simple');
  };

  const handleNewTask = () => {
    reset();
    setDescription('');
  };

  const isRunning = status === 'running' || status === 'paused';
  const isCompleted = status === 'completed' || status === 'failed';
  const isDisabled = isRunning || isSubmitting;

  return (
    <div className="h-full flex flex-col p-6">
      {/* Header with Connection Status and History Toggle */}
      <div className="flex items-center justify-between mb-4 max-w-2xl mx-auto w-full">
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
      <div className="max-w-2xl mx-auto w-full">
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

      {/* Progress/Results/History Area */}
      <div className="flex-1 mt-8 overflow-auto">
        {showHistory ? (
          <HistoryPanel onClose={() => setShowHistory(false)} />
        ) : (
          <>
            {isRunning && <ProgressView />}
            {isCompleted && (
              <div className="space-y-4">
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
