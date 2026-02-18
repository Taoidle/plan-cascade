/**
 * Task Mode Panel
 *
 * Main panel displaying the current Task Mode phase including:
 * - PRD generation/review
 * - Batch execution progress with per-story status
 * - Quality gate results per story
 * - Execution report summary
 *
 * Story 007: Frontend Task Mode Store and UI Components
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  ChevronDownIcon,
  ChevronRightIcon,
  CheckCircledIcon,
  CrossCircledIcon,
  ReloadIcon,
  StopIcon,
  ExitIcon,
  RocketIcon,
  GearIcon,
  FileTextIcon,
  PlayIcon,
  ClockIcon,
  UpdateIcon,
} from '@radix-ui/react-icons';
import { useTaskModeStore } from '../../store/taskMode';
import type { TaskStory, TaskModeSessionStatus } from '../../store/taskMode';
import { QualityGatesSummary } from './QualityGateResults';

// ============================================================================
// Phase indicator
// ============================================================================

const phaseConfig: Record<
  TaskModeSessionStatus | 'idle',
  { icon: React.ReactNode; color: string }
> = {
  idle: { icon: <ClockIcon className="w-4 h-4" />, color: 'text-gray-500' },
  initialized: { icon: <GearIcon className="w-4 h-4" />, color: 'text-blue-500' },
  generating_prd: { icon: <UpdateIcon className="w-4 h-4 animate-spin" />, color: 'text-blue-500' },
  reviewing_prd: { icon: <FileTextIcon className="w-4 h-4" />, color: 'text-amber-500' },
  executing: { icon: <PlayIcon className="w-4 h-4" />, color: 'text-blue-500' },
  completed: { icon: <CheckCircledIcon className="w-4 h-4" />, color: 'text-green-500' },
  failed: { icon: <CrossCircledIcon className="w-4 h-4" />, color: 'text-red-500' },
  cancelled: { icon: <StopIcon className="w-4 h-4" />, color: 'text-gray-500' },
};

// Map backend status (snake_case) to the keys we use
function normalizeStatus(status: string): TaskModeSessionStatus | 'idle' {
  const map: Record<string, TaskModeSessionStatus | 'idle'> = {
    initialized: 'initialized',
    generating_prd: 'generating_prd',
    reviewing_prd: 'reviewing_prd',
    executing: 'executing',
    completed: 'completed',
    failed: 'failed',
    cancelled: 'cancelled',
    idle: 'idle',
  };
  return map[status] ?? 'idle';
}

// Map status to i18n key (camelCase)
function statusToI18nKey(status: TaskModeSessionStatus | 'idle'): string {
  const map: Record<string, string> = {
    idle: 'initialized',
    initialized: 'initialized',
    generating_prd: 'generatingPrd',
    reviewing_prd: 'reviewingPrd',
    executing: 'executing',
    completed: 'completed',
    failed: 'failed',
    cancelled: 'cancelled',
  };
  return map[status] ?? 'initialized';
}

// ============================================================================
// Story Row
// ============================================================================

interface StoryRowProps {
  story: TaskStory;
  status?: string;
  isEditable: boolean;
}

function StoryRow({ story, status, isEditable: _isEditable }: StoryRowProps) {
  const { t } = useTranslation('taskMode');
  const [isExpanded, setIsExpanded] = useState(false);

  const statusIcon = useMemo(() => {
    switch (status) {
      case 'completed':
        return <CheckCircledIcon className="w-4 h-4 text-green-500" />;
      case 'running':
        return <UpdateIcon className="w-4 h-4 text-blue-500 animate-spin" />;
      case 'failed':
        return <CrossCircledIcon className="w-4 h-4 text-red-500" />;
      default:
        return <ClockIcon className="w-4 h-4 text-gray-400" />;
    }
  }, [status]);

  const statusLabel = status
    ? t(`panel.storyStatus.${status}`, { defaultValue: status })
    : t('panel.storyStatus.pending');

  return (
    <div
      className={clsx(
        'border border-gray-200 dark:border-gray-700 rounded-lg',
        'bg-white dark:bg-gray-800',
        'transition-all'
      )}
    >
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={clsx(
          'w-full flex items-center gap-2 px-3 py-2',
          'text-left text-sm',
          'hover:bg-gray-50 dark:hover:bg-gray-700/50',
          'transition-colors rounded-lg'
        )}
      >
        {isExpanded ? (
          <ChevronDownIcon className="w-4 h-4 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRightIcon className="w-4 h-4 text-gray-400 flex-shrink-0" />
        )}
        {statusIcon}
        <span className="font-medium text-gray-800 dark:text-gray-200 flex-1 truncate">
          {story.title}
        </span>
        <span
          className={clsx(
            'text-xs px-1.5 py-0.5 rounded',
            story.priority === 'high' && 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
            story.priority === 'medium' && 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300',
            story.priority === 'low' && 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400'
          )}
        >
          {story.priority}
        </span>
        <span className="text-xs text-gray-500 dark:text-gray-400">
          {statusLabel}
        </span>
      </button>

      {/* Expanded content */}
      {isExpanded && (
        <div className="px-3 pb-3 space-y-2 border-t border-gray-100 dark:border-gray-700 pt-2">
          <p className="text-xs text-gray-600 dark:text-gray-400">
            {story.description}
          </p>

          {/* Dependencies */}
          <div className="flex items-center gap-1 text-xs">
            <span className="text-gray-500 dark:text-gray-400 font-medium">
              {t('panel.prd.dependencies')}:
            </span>
            {story.dependencies.length > 0 ? (
              <span className="text-gray-600 dark:text-gray-300">
                {story.dependencies.join(', ')}
              </span>
            ) : (
              <span className="text-gray-400 dark:text-gray-500 italic">
                {t('panel.prd.noDependencies')}
              </span>
            )}
          </div>

          {/* Acceptance Criteria */}
          {story.acceptanceCriteria.length > 0 && (
            <div className="space-y-0.5">
              <span className="text-xs text-gray-500 dark:text-gray-400 font-medium">
                {t('panel.prd.acceptanceCriteria')}:
              </span>
              <ul className="list-disc list-inside space-y-0.5">
                {story.acceptanceCriteria.map((ac, i) => (
                  <li key={i} className="text-xs text-gray-600 dark:text-gray-300">
                    {ac}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Main Panel
// ============================================================================

export function TaskModePanel() {
  const { t } = useTranslation('taskMode');

  const {
    isTaskMode,
    sessionStatus,
    prd,
    currentBatch,
    totalBatches,
    storyStatuses,
    qualityGateResults,
    report,
    isLoading,
    error,
    generatePrd,
    approvePrd,
    cancelExecution,
    fetchReport,
    exitTaskMode,
    refreshStatus,
  } = useTaskModeStore();

  // Auto-refresh execution status while executing
  useEffect(() => {
    if (sessionStatus !== 'executing') return;
    const interval = setInterval(() => {
      refreshStatus();
    }, 3000);
    return () => clearInterval(interval);
  }, [sessionStatus, refreshStatus]);

  // Auto-fetch report when completed/failed/cancelled
  useEffect(() => {
    if (
      sessionStatus === 'completed' ||
      sessionStatus === 'failed' ||
      sessionStatus === 'cancelled'
    ) {
      if (!report) {
        fetchReport();
      }
    }
  }, [sessionStatus, report, fetchReport]);

  const handleApprove = useCallback(() => {
    if (prd) {
      approvePrd(prd);
    }
  }, [prd, approvePrd]);

  const handleRegenerate = useCallback(() => {
    generatePrd();
  }, [generatePrd]);

  if (!isTaskMode) {
    return null;
  }

  const normalizedStatus = normalizeStatus(sessionStatus);
  const phase = phaseConfig[normalizedStatus];
  const i18nKey = statusToI18nKey(normalizedStatus);

  // Count stories
  const totalStories = prd?.stories.length ?? 0;
  const completedStories = Object.values(storyStatuses).filter(
    (s) => s === 'completed'
  ).length;
  const failedStories = Object.values(storyStatuses).filter(
    (s) => s === 'failed'
  ).length;

  return (
    <div
      className={clsx(
        'flex flex-col h-full',
        'bg-white dark:bg-gray-900',
        'border-l border-gray-200 dark:border-gray-700'
      )}
      data-testid="task-mode-panel"
    >
      {/* Header */}
      <div
        className={clsx(
          'flex items-center gap-2 px-4 py-3',
          'border-b border-gray-200 dark:border-gray-700'
        )}
      >
        <RocketIcon className="w-5 h-5 text-blue-600 dark:text-blue-400" />
        <h2 className="text-sm font-semibold text-gray-800 dark:text-gray-200 flex-1">
          {t('panel.title')}
        </h2>
        <div className={clsx('flex items-center gap-1.5 text-xs', phase.color)}>
          {phase.icon}
          <span>{t(`panel.phase.${i18nKey}`)}</span>
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div
          className={clsx(
            'px-4 py-2 text-xs',
            'bg-red-50 dark:bg-red-900/20',
            'text-red-700 dark:text-red-300',
            'border-b border-red-200 dark:border-red-800'
          )}
        >
          {t('panel.error', { message: error })}
        </div>
      )}

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Initialized - show Generate PRD button */}
        {normalizedStatus === 'initialized' && (
          <div className="text-center py-8">
            <button
              onClick={handleRegenerate}
              disabled={isLoading}
              className={clsx(
                'px-4 py-2 rounded-lg text-sm font-medium',
                'bg-blue-600 hover:bg-blue-700 text-white',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors'
              )}
            >
              {isLoading ? (
                <span className="flex items-center gap-2">
                  <UpdateIcon className="w-4 h-4 animate-spin" />
                  {t('panel.phase.generatingPrd')}
                </span>
              ) : (
                t('panel.prd.generate')
              )}
            </button>
          </div>
        )}

        {/* Generating PRD */}
        {normalizedStatus === 'generating_prd' && (
          <div className="flex items-center justify-center gap-2 py-8 text-sm text-gray-500 dark:text-gray-400">
            <UpdateIcon className="w-4 h-4 animate-spin" />
            {t('panel.phase.generatingPrd')}
          </div>
        )}

        {/* Reviewing PRD */}
        {normalizedStatus === 'reviewing_prd' && prd && (
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">
                {t('panel.prd.stories', { count: prd.stories.length })}
              </h3>
            </div>

            {prd.stories.length === 0 ? (
              <p className="text-sm text-gray-500 dark:text-gray-400 italic">
                {t('panel.prd.noStories')}
              </p>
            ) : (
              <div className="space-y-2">
                {prd.stories.map((story) => (
                  <StoryRow
                    key={story.id}
                    story={story}
                    isEditable={true}
                  />
                ))}
              </div>
            )}

            {/* Action buttons */}
            <div className="flex items-center gap-2 pt-2">
              <button
                onClick={handleApprove}
                disabled={isLoading || prd.stories.length === 0}
                className={clsx(
                  'flex-1 px-3 py-2 rounded-lg text-sm font-medium',
                  'bg-green-600 hover:bg-green-700 text-white',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                  'transition-colors'
                )}
              >
                {t('panel.prd.approve')}
              </button>
              <button
                onClick={handleRegenerate}
                disabled={isLoading}
                className={clsx(
                  'px-3 py-2 rounded-lg text-sm',
                  'border border-gray-300 dark:border-gray-600',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-100 dark:hover:bg-gray-700',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                  'transition-colors'
                )}
              >
                <ReloadIcon className="w-4 h-4" />
              </button>
            </div>
          </div>
        )}

        {/* Executing */}
        {normalizedStatus === 'executing' && prd && (
          <div className="space-y-4">
            {/* Progress indicator */}
            <div className="space-y-2">
              <div className="flex items-center justify-between text-sm">
                <span className="text-gray-600 dark:text-gray-400">
                  {t('panel.execution.batch', {
                    current: currentBatch + 1,
                    total: totalBatches,
                  })}
                </span>
                <span className="text-gray-500 dark:text-gray-400">
                  {t('panel.execution.storiesCompleted', {
                    completed: completedStories,
                    total: totalStories,
                  })}
                </span>
              </div>

              {/* Progress bar */}
              <div className="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                <div
                  className={clsx(
                    'h-full rounded-full transition-all duration-300',
                    failedStories > 0 ? 'bg-amber-500' : 'bg-blue-500'
                  )}
                  style={{
                    width: `${totalStories > 0 ? ((completedStories + failedStories) / totalStories) * 100 : 0}%`,
                  }}
                />
              </div>

              {failedStories > 0 && (
                <p className="text-xs text-red-600 dark:text-red-400">
                  {t('panel.execution.storiesFailed', { count: failedStories })}
                </p>
              )}
            </div>

            {/* Story list with status */}
            <div className="space-y-2">
              {prd.stories.map((story) => (
                <StoryRow
                  key={story.id}
                  story={story}
                  status={storyStatuses[story.id]}
                  isEditable={false}
                />
              ))}
            </div>

            {/* Quality gate results */}
            {Object.keys(qualityGateResults).length > 0 && (
              <div className="space-y-2">
                <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">
                  {t('qualityGates.title')}
                </h3>
                <QualityGatesSummary results={qualityGateResults} />
              </div>
            )}

            {/* Cancel button */}
            <button
              onClick={cancelExecution}
              disabled={isLoading}
              className={clsx(
                'w-full px-3 py-2 rounded-lg text-sm font-medium',
                'border border-red-300 dark:border-red-700',
                'text-red-700 dark:text-red-300',
                'hover:bg-red-50 dark:hover:bg-red-900/20',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors'
              )}
            >
              {isLoading ? t('panel.execution.cancelling') : t('panel.execution.cancel')}
            </button>
          </div>
        )}

        {/* Completed / Failed / Cancelled - Report */}
        {(normalizedStatus === 'completed' ||
          normalizedStatus === 'failed' ||
          normalizedStatus === 'cancelled') && (
          <div className="space-y-4">
            {/* Story list with final status */}
            {prd && (
              <div className="space-y-2">
                {prd.stories.map((story) => (
                  <StoryRow
                    key={story.id}
                    story={story}
                    status={storyStatuses[story.id]}
                    isEditable={false}
                  />
                ))}
              </div>
            )}

            {/* Report */}
            {report && (
              <div
                className={clsx(
                  'p-4 rounded-lg space-y-3',
                  'border',
                  report.success
                    ? 'border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20'
                    : 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20'
                )}
              >
                <h3
                  className={clsx(
                    'text-sm font-semibold',
                    report.success
                      ? 'text-green-800 dark:text-green-200'
                      : 'text-red-800 dark:text-red-200'
                  )}
                >
                  {t('panel.report.title')}
                </h3>

                <div className="grid grid-cols-2 gap-2 text-xs">
                  <div>
                    <span className="text-gray-500 dark:text-gray-400">
                      {t('panel.report.totalStories')}
                    </span>
                    <p className="font-semibold text-gray-800 dark:text-gray-200">
                      {report.totalStories}
                    </p>
                  </div>
                  <div>
                    <span className="text-gray-500 dark:text-gray-400">
                      {t('panel.report.completed')}
                    </span>
                    <p className="font-semibold text-green-700 dark:text-green-300">
                      {report.storiesCompleted}
                    </p>
                  </div>
                  <div>
                    <span className="text-gray-500 dark:text-gray-400">
                      {t('panel.report.failed')}
                    </span>
                    <p className="font-semibold text-red-700 dark:text-red-300">
                      {report.storiesFailed}
                    </p>
                  </div>
                  <div>
                    <span className="text-gray-500 dark:text-gray-400">
                      {t('panel.report.duration')}
                    </span>
                    <p className="font-semibold text-gray-800 dark:text-gray-200">
                      {t('panel.report.durationValue', {
                        seconds: (report.totalDurationMs / 1000).toFixed(1),
                      })}
                    </p>
                  </div>
                </div>
              </div>
            )}

            {/* Quality gate results */}
            {Object.keys(qualityGateResults).length > 0 && (
              <div className="space-y-2">
                <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">
                  {t('qualityGates.title')}
                </h3>
                <QualityGatesSummary results={qualityGateResults} />
              </div>
            )}

            {/* Exit button */}
            <button
              onClick={exitTaskMode}
              disabled={isLoading}
              className={clsx(
                'w-full flex items-center justify-center gap-2',
                'px-3 py-2 rounded-lg text-sm font-medium',
                'bg-gray-600 hover:bg-gray-700 text-white',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors'
              )}
            >
              <ExitIcon className="w-4 h-4" />
              {t('panel.report.exitTaskMode')}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

export default TaskModePanel;
