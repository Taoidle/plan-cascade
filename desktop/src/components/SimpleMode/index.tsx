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
 * - Multi-turn chat with Claude Code backend
 *
 * Story 008: Added StreamingOutput, GlobalProgressBar, and ErrorState
 */

import { useEffect, useRef, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { InputBox } from './InputBox';
import { ProgressView } from './ProgressView';
import { ResultView } from './ResultView';
import { HistoryPanel } from './HistoryPanel';
import { ConnectionStatus } from './ConnectionStatus';
import { useExecutionStore } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';
import { StreamingOutput, GlobalProgressBar, ErrorState, ProjectSelector } from '../shared';

export function SimpleMode() {
  const { t } = useTranslation('simpleMode');
  const {
    status,
    connectionStatus,
    isSubmitting,
    apiError,
    start,
    sendFollowUp,
    reset,
    result,
    initialize,
    cleanup,
    analyzeStrategy,
    strategyAnalysis,
    isAnalyzingStrategy,
    clearStrategyAnalysis,
    isChatSession,
    streamingOutput,
    standaloneTurns,
  } = useExecutionStore();
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const [description, setDescription] = useState('');
  const [showHistory, setShowHistory] = useState(false);

  // Initialize Tauri event listeners on mount
  useEffect(() => {
    initialize();
    return () => {
      cleanup();
    };
  }, [initialize, cleanup]);

  // When workspacePath changes while a chat session is active, reset it
  // so the next task starts a fresh session with the new directory.
  const prevPathRef = useRef(workspacePath);
  useEffect(() => {
    if (prevPathRef.current !== workspacePath && isChatSession) {
      reset();
      clearStrategyAnalysis();
      setDescription('');
    }
    prevPathRef.current = workspacePath;
  }, [workspacePath, isChatSession, reset, clearStrategyAnalysis]);

  const handleStart = async () => {
    if (!description.trim() || isSubmitting || isAnalyzingStrategy) return;

    // Step 1: Analyze strategy automatically
    const analysis = await analyzeStrategy(description);

    // Step 2: Start execution with the auto-selected strategy
    if (analysis) {
      await start(description, 'simple');
      setDescription('');
    }
  };

  const handleFollowUp = useCallback(async () => {
    if (!description.trim() || isSubmitting) return;
    const prompt = description;
    setDescription('');
    await sendFollowUp(prompt);
  }, [description, isSubmitting, sendFollowUp]);

  const handleNewTask = () => {
    reset();
    clearStrategyAnalysis();
    setDescription('');
  };

  const isRunning = status === 'running' || status === 'paused';
  const isCompleted = status === 'completed' || status === 'failed';
  const isDisabled = isRunning || isSubmitting || isAnalyzingStrategy;

  // Show conversation layout whenever there is streamed content.
  // This keeps Simple mode interactive after standalone responses too.
  const hasChatContent = streamingOutput.length > 0;

  return (
    <div className="h-full flex flex-col">
      {/* Header with Connection Status and History Toggle */}
      <div className="flex items-center justify-between px-6 py-3 shrink-0 max-w-2xl 3xl:max-w-3xl 5xl:max-w-4xl mx-auto w-full">
        <div className="flex items-center gap-2">
          <ConnectionStatus status={connectionStatus} />
          <ProjectSelector compact />
        </div>
        <div className="flex items-center gap-2">
          {(isChatSession || standaloneTurns.length > 0) && (
            <button
              onClick={handleNewTask}
              className={clsx(
                'text-sm px-3 py-1.5 rounded-lg transition-colors',
                'text-gray-600 dark:text-gray-400',
                'hover:bg-gray-100 dark:hover:bg-gray-800'
              )}
            >
              {t('buttons.startNewTask', { ns: 'common', defaultValue: 'New Chat' })}
            </button>
          )}
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
      </div>

      {showHistory ? (
        <div className="flex-1 overflow-auto px-6">
          <HistoryPanel onClose={() => setShowHistory(false)} />
        </div>
      ) : hasChatContent || isRunning ? (
        /* ============================================================
         * Chat / Streaming layout: output fills available space, input at bottom
         * ============================================================ */
        <>
          {/* Streaming output area — fills available space */}
          <div className="flex-1 overflow-auto px-6 min-h-0">
            <div className="max-w-2xl 3xl:max-w-3xl 5xl:max-w-4xl mx-auto w-full space-y-4">
              {/* Strategy Analysis Banner */}
              {(isAnalyzingStrategy || strategyAnalysis) && (
                <StrategyBanner
                  isAnalyzing={isAnalyzingStrategy}
                  analysis={strategyAnalysis}
                  t={t}
                />
              )}

              {isRunning && !isChatSession && (
                <>
                  <GlobalProgressBar compact showStoryLabels={false} />
                  <ProgressView />
                </>
              )}

              <StreamingOutput
                maxHeight="none"
                compact={false}
                showClear={false}
              />

              {isRunning && !isChatSession && <ErrorState maxErrors={3} />}
            </div>
          </div>

          {/* Input at bottom — always visible during chat */}
          <div className="shrink-0 px-6 py-4 border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
            <div className="max-w-2xl 3xl:max-w-3xl 5xl:max-w-4xl mx-auto w-full">
              <InputBox
                value={description}
                onChange={setDescription}
                onSubmit={isChatSession ? handleFollowUp : handleStart}
                disabled={isDisabled}
                placeholder={
                  isRunning
                    ? t('input.waitingPlaceholder', { defaultValue: 'Waiting for response...' })
                    : t('input.followUpPlaceholder', { defaultValue: 'Send a follow-up message...' })
                }
                isLoading={isRunning}
              />
              {apiError && (
                <div className="mt-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                  <p className="text-sm text-red-600 dark:text-red-400">{apiError}</p>
                </div>
              )}
            </div>
          </div>
        </>
      ) : isCompleted ? (
        /* ============================================================
         * Non-chat completed: show result view (PRD execution etc.)
         * ============================================================ */
        <div className="flex-1 overflow-auto px-6 pt-8">
          <div className="max-w-2xl 3xl:max-w-3xl 5xl:max-w-4xl mx-auto w-full space-y-4">
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
        </div>
      ) : (
        /* ============================================================
         * Idle (no chat): initial input + empty state
         * ============================================================ */
        <div className="flex-1 flex flex-col px-6 pt-6">
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

            {apiError && (
              <div className="mt-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                <p className="text-sm text-red-600 dark:text-red-400">{apiError}</p>
              </div>
            )}
          </div>

          {/* Strategy Analysis Banner */}
          {(isAnalyzingStrategy || strategyAnalysis) && (
            <div className="max-w-2xl mx-auto w-full mt-4">
              <StrategyBanner
                isAnalyzing={isAnalyzingStrategy}
                analysis={strategyAnalysis}
                t={t}
              />
            </div>
          )}

          {/* Empty state */}
          <div className="flex-1 flex flex-col items-center justify-center text-center">
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
        </div>
      )}
    </div>
  );
}

// ============================================================================
// StrategyBanner Component
// ============================================================================

function StrategyBanner({
  isAnalyzing,
  analysis,
  t,
}: {
  isAnalyzing: boolean;
  analysis: ReturnType<typeof useExecutionStore.getState>['strategyAnalysis'];
  t: ReturnType<typeof useTranslation>['t'];
}) {
  if (isAnalyzing) {
    return (
      <div className="p-3 rounded-lg bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 flex items-center gap-2">
        <div className="animate-spin h-4 w-4 border-2 border-blue-500 border-t-transparent rounded-full" />
        <p className="text-sm text-blue-600 dark:text-blue-400">
          {t('strategy.analyzing', { defaultValue: 'Analyzing task complexity...' })}
        </p>
      </div>
    );
  }

  if (analysis) {
    return (
      <div className="p-3 rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
        <div className="flex items-center justify-between">
          <p className="text-sm font-medium text-green-700 dark:text-green-300">
            {t('strategy.selected', { defaultValue: 'Strategy' })}:{' '}
            <span className="font-semibold">{analysis.strategy.replace(/_/g, ' ')}</span>
            <span className="ml-2 text-xs text-green-600 dark:text-green-400">
              ({(analysis.confidence * 100).toFixed(0)}% confidence)
            </span>
          </p>
        </div>
        <p className="text-xs text-green-600 dark:text-green-400 mt-1">
          {analysis.reasoning}
        </p>
      </div>
    );
  }

  return null;
}

export default SimpleMode;
