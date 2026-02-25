/**
 * Task Mode Switcher
 *
 * A compact indicator/button shown in the chat input area when strategy
 * analysis recommends Task Mode. Provides accept and dismiss actions.
 *
 * Story 007: Frontend Task Mode Store and UI Components
 */

import { useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { RocketIcon, Cross2Icon, LightningBoltIcon, ChatBubbleIcon } from '@radix-ui/react-icons';
import { useTaskModeStore } from '../../store/taskMode';

export function TaskModeSwitcher() {
  const { t } = useTranslation('taskMode');

  const { strategyAnalysis, suggestionDismissed, isTaskMode, isLoading, enterTaskMode, dismissSuggestion } =
    useTaskModeStore();

  const handleAccept = useCallback(() => {
    if (strategyAnalysis) {
      enterTaskMode(strategyAnalysis.strategyDecision.reasoning);
    }
  }, [strategyAnalysis, enterTaskMode]);

  const handleDismiss = useCallback(() => {
    dismissSuggestion();
  }, [dismissSuggestion]);

  // Don't show if already in task mode, no analysis, or dismissed
  if (isTaskMode || !strategyAnalysis || suggestionDismissed) {
    return null;
  }

  // Only show when task mode is recommended
  if (strategyAnalysis.recommendedMode !== 'task') {
    return null;
  }

  const confidencePercent = Math.round(strategyAnalysis.confidence * 100);

  return (
    <div
      className={clsx(
        'flex items-center gap-2 px-3 py-2 rounded-lg',
        'bg-blue-50 dark:bg-blue-900/30',
        'border border-blue-200 dark:border-blue-700',
        'text-sm',
        'animate-in fade-in slide-in-from-bottom-2 duration-200',
      )}
      data-testid="task-mode-switcher"
    >
      {/* Icon */}
      <RocketIcon className="w-4 h-4 text-blue-600 dark:text-blue-400 flex-shrink-0" />

      {/* Content */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className="font-medium text-blue-800 dark:text-blue-200">{t('switcher.suggested')}</span>
          <span className="text-blue-600 dark:text-blue-400 text-xs">
            {t('switcher.confidence', { confidence: confidencePercent })}
          </span>
        </div>
        <p className="text-xs text-blue-600 dark:text-blue-400 truncate mt-0.5">{strategyAnalysis.reasoning}</p>
      </div>

      {/* Mode indicators */}
      <div className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
        <ChatBubbleIcon className="w-3 h-3" />
        <span className="text-gray-400 dark:text-gray-500">/</span>
        <LightningBoltIcon className="w-3 h-3 text-blue-600 dark:text-blue-400" />
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1 flex-shrink-0">
        <button
          onClick={handleAccept}
          disabled={isLoading}
          className={clsx(
            'px-2.5 py-1 rounded-md text-xs font-medium',
            'bg-blue-600 hover:bg-blue-700 text-white',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'transition-colors',
          )}
          data-testid="task-mode-accept"
        >
          {isLoading ? t('switcher.analyzing') : t('switcher.accept')}
        </button>
        <button
          onClick={handleDismiss}
          className={clsx(
            'p-1 rounded-md',
            'text-gray-500 dark:text-gray-400',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'transition-colors',
          )}
          aria-label={t('switcher.dismiss')}
          data-testid="task-mode-dismiss"
        >
          <Cross2Icon className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  );
}

export default TaskModeSwitcher;
