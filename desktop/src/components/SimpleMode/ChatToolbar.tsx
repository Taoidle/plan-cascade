/**
 * ChatToolbar Component
 *
 * Toolbar positioned above the input box. Contains:
 * - Left: Chat/Task mode toggle, file attach button
 * - Center: Execution controls (pause/resume, cancel) — shown only when running
 * - Right: Output panel toggle with count badge
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  FilePlusIcon,
  PauseIcon,
  PlayIcon,
  Cross2Icon,
} from '@radix-ui/react-icons';

type WorkflowMode = 'chat' | 'task';

interface ChatToolbarProps {
  // Mode toggle
  workflowMode: WorkflowMode;
  onWorkflowModeChange: (mode: WorkflowMode) => void;
  // File attach
  onFilePick: () => void;
  isFilePickDisabled: boolean;
  // Execution controls
  executionStatus: 'idle' | 'running' | 'paused' | 'completed' | 'failed';
  onPause: () => void;
  onResume: () => void;
  onCancel: () => void;
  // Right panel
  rightPanelOpen: boolean;
  rightPanelTab: 'output' | 'git';
  onToggleOutput: () => void;
  detailLineCount: number;
}

export function ChatToolbar({
  workflowMode,
  onWorkflowModeChange,
  onFilePick,
  isFilePickDisabled,
  executionStatus,
  onPause,
  onResume,
  onCancel,
  rightPanelOpen,
  rightPanelTab,
  onToggleOutput,
  detailLineCount,
}: ChatToolbarProps) {
  const { t } = useTranslation('simpleMode');

  const isExecuting = executionStatus === 'running' || executionStatus === 'paused';

  return (
    <div
      className={clsx(
        'shrink-0 flex items-center justify-between px-3 py-1.5',
        'border-t border-gray-200 dark:border-gray-700'
      )}
    >
      {/* Left group: mode toggle + file attach */}
      <div className="flex items-center gap-2">
        {/* Chat/Task segmented control */}
        <div className="flex items-center rounded-lg border border-gray-300 dark:border-gray-700 overflow-hidden">
          <button
            onClick={() => onWorkflowModeChange('chat')}
            className={clsx(
              'px-3 py-1.5 text-xs font-medium transition-colors',
              workflowMode === 'chat'
                ? 'bg-primary-600 text-white'
                : 'bg-white dark:bg-gray-900 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800'
            )}
          >
            {t('workflowMode.chat', { defaultValue: 'Chat' })}
          </button>
          <button
            onClick={() => onWorkflowModeChange('task')}
            className={clsx(
              'px-3 py-1.5 text-xs font-medium transition-colors',
              workflowMode === 'task'
                ? 'bg-primary-600 text-white'
                : 'bg-white dark:bg-gray-900 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800'
            )}
          >
            {t('workflowMode.task', { defaultValue: 'Task' })}
          </button>
        </div>

        {/* File attach button */}
        <button
          onClick={onFilePick}
          disabled={isFilePickDisabled}
          className={clsx(
            'flex items-center justify-center',
            'w-7 h-7 rounded-md',
            'text-gray-500 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-700',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'transition-colors'
          )}
          title={t('chatToolbar.attachFile', { defaultValue: 'Attach file' })}
        >
          <FilePlusIcon className="w-4 h-4" />
        </button>
      </div>

      {/* Center group: execution controls (only when running/paused) */}
      {isExecuting && (
        <div className="flex items-center gap-1">
          {executionStatus === 'running' ? (
            <button
              onClick={onPause}
              className={clsx(
                'flex items-center justify-center',
                'w-7 h-7 rounded-md',
                'text-gray-600 dark:text-gray-300',
                'hover:bg-gray-100 dark:hover:bg-gray-700',
                'transition-colors'
              )}
              title={t('chatToolbar.pause', { defaultValue: 'Pause' })}
            >
              <PauseIcon className="w-4 h-4" />
            </button>
          ) : (
            <button
              onClick={onResume}
              className={clsx(
                'flex items-center justify-center',
                'w-7 h-7 rounded-md',
                'text-gray-600 dark:text-gray-300',
                'hover:bg-gray-100 dark:hover:bg-gray-700',
                'transition-colors'
              )}
              title={t('chatToolbar.resume', { defaultValue: 'Resume' })}
            >
              <PlayIcon className="w-4 h-4" />
            </button>
          )}
          <button
            onClick={onCancel}
            className={clsx(
              'flex items-center justify-center',
              'w-7 h-7 rounded-md',
              'text-red-500 dark:text-red-400',
              'hover:bg-red-50 dark:hover:bg-red-900/20',
              'transition-colors'
            )}
            title={t('chatToolbar.cancel', { defaultValue: 'Cancel' })}
          >
            <Cross2Icon className="w-4 h-4" />
          </button>
        </div>
      )}

      {/* Right group: Output toggle */}
      <div className="flex items-center">
        <button
          onClick={onToggleOutput}
          className={clsx(
            'text-xs px-2.5 py-1.5 rounded-md transition-colors',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
            rightPanelOpen && rightPanelTab === 'output' && 'bg-gray-100 dark:bg-gray-800'
          )}
          title={t('chatToolbar.toggleOutput', { defaultValue: 'Output' })}
        >
          {t('chatToolbar.toggleOutput', { defaultValue: 'Output' })}
          {detailLineCount > 0 && (
            <span className="ml-1 px-1.5 py-0.5 rounded-full bg-gray-200 dark:bg-gray-700 text-2xs">
              {detailLineCount}
            </span>
          )}
        </button>
      </div>
    </div>
  );
}
