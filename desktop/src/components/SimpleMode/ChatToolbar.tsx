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
import { FilePlusIcon, PauseIcon, PlayIcon, Cross2Icon, CameraIcon } from '@radix-ui/react-icons';
import { ContextSourceBar } from '../shared/ContextSourceBar';

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
  // Task workflow controls
  taskWorkflowActive?: boolean;
  onCancelWorkflow?: () => void;
  // Export image
  onExportImage: () => void;
  isExportDisabled: boolean;
  isCapturing: boolean;
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
  taskWorkflowActive,
  onCancelWorkflow,
  onExportImage,
  isExportDisabled,
  isCapturing,
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
        'border-t border-gray-200 dark:border-gray-700',
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
                : 'bg-white dark:bg-gray-900 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
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
                : 'bg-white dark:bg-gray-900 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
            )}
          >
            {t('workflowMode.task', { defaultValue: 'Task' })}
          </button>
        </div>

        {/* Context source toggles */}
        <ContextSourceBar />

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
            'transition-colors',
          )}
          title={t('chatToolbar.attachFile', { defaultValue: 'Attach file' })}
        >
          <FilePlusIcon className="w-4 h-4" />
        </button>
      </div>

      {/* Center group: execution controls (chat running/paused OR task workflow active) */}
      {(isExecuting || taskWorkflowActive) && (
        <div className="flex items-center gap-1">
          {isExecuting && executionStatus === 'running' && (
            <button
              onClick={onPause}
              className={clsx(
                'flex items-center justify-center',
                'w-7 h-7 rounded-md',
                'text-gray-600 dark:text-gray-300',
                'hover:bg-gray-100 dark:hover:bg-gray-700',
                'transition-colors',
              )}
              title={t('chatToolbar.pause', { defaultValue: 'Pause' })}
            >
              <PauseIcon className="w-4 h-4" />
            </button>
          )}
          {isExecuting && executionStatus === 'paused' && (
            <button
              onClick={onResume}
              className={clsx(
                'flex items-center justify-center',
                'w-7 h-7 rounded-md',
                'text-gray-600 dark:text-gray-300',
                'hover:bg-gray-100 dark:hover:bg-gray-700',
                'transition-colors',
              )}
              title={t('chatToolbar.resume', { defaultValue: 'Resume' })}
            >
              <PlayIcon className="w-4 h-4" />
            </button>
          )}
          {isExecuting && (
            <button
              onClick={onCancel}
              className={clsx(
                'flex items-center justify-center',
                'w-7 h-7 rounded-md',
                'text-red-500 dark:text-red-400',
                'hover:bg-red-50 dark:hover:bg-red-900/20',
                'transition-colors',
              )}
              title={t('chatToolbar.cancel', { defaultValue: 'Cancel' })}
            >
              <Cross2Icon className="w-4 h-4" />
            </button>
          )}
          {taskWorkflowActive && !isExecuting && onCancelWorkflow && (
            <button
              onClick={onCancelWorkflow}
              className={clsx(
                'flex items-center gap-1',
                'px-2 py-1 rounded-md text-xs',
                'text-red-500 dark:text-red-400',
                'hover:bg-red-50 dark:hover:bg-red-900/20',
                'transition-colors',
              )}
              title={t('workflow.cancelWorkflow')}
            >
              <Cross2Icon className="w-3.5 h-3.5" />
              <span>{t('workflow.cancelWorkflow')}</span>
            </button>
          )}
        </div>
      )}

      {/* Right group: Export image + Output toggle */}
      <div className="flex items-center gap-1">
        {/* Export chat as image */}
        <button
          onClick={onExportImage}
          disabled={isExportDisabled || isCapturing}
          className={clsx(
            'flex items-center justify-center',
            'w-7 h-7 rounded-md',
            'text-gray-500 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-700',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'transition-colors',
          )}
          title={t('chatToolbar.exportImage', { defaultValue: 'Export as image' })}
        >
          {isCapturing ? (
            <svg className="animate-spin w-4 h-4" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path
                className="opacity-75"
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
              />
            </svg>
          ) : (
            <CameraIcon className="w-4 h-4" />
          )}
        </button>
        <button
          onClick={onToggleOutput}
          className={clsx(
            'text-xs px-2.5 py-1.5 rounded-md transition-colors',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
            rightPanelOpen && rightPanelTab === 'output' && 'bg-gray-100 dark:bg-gray-800',
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
