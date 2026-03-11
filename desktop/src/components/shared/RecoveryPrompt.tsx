/**
 * RecoveryPrompt Component
 *
 * Auto-displays on app launch when incomplete executions are detected.
 * Shows execution summary with mode, progress percentage, and last activity time.
 * Provides Resume and Discard actions for each interrupted task.
 *
 * Story-004: Resume & Recovery System
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useRecoveryStore, EXECUTION_MODE_LABELS, type IncompleteTask, type ExecutionMode } from '../../store/recovery';
import { buildDebugStateChips, summarizeDebugCase } from '../../lib/debugLabels';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import { useSettingsStore } from '../../store/settings';

// ============================================================================
// Helper Functions
// ============================================================================

/** Format a timestamp into a relative "time ago" string */
function formatTimeAgo(
  timestamp: string | null,
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  if (!timestamp) return t('common:time.unknown', { defaultValue: 'Unknown' });

  const date = new Date(timestamp);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMinutes = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMinutes / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMinutes < 1) return t('common:time.justNow', { defaultValue: 'Just now' });
  if (diffMinutes < 60) return t('common:time.minutesAgo', { count: diffMinutes, defaultValue: `${diffMinutes}m ago` });
  if (diffHours < 24) return t('common:time.hoursAgo', { count: diffHours, defaultValue: `${diffHours}h ago` });
  if (diffDays < 7) return t('common:time.daysAgo', { count: diffDays, defaultValue: `${diffDays}d ago` });
  return date.toLocaleDateString();
}

/** Get a color class based on execution mode */
function getModeColor(mode: ExecutionMode): string {
  switch (mode) {
    case 'direct':
      return 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300';
    case 'hybrid_auto':
      return 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300';
    case 'hybrid_worktree':
      return 'bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300';
    case 'mega_plan':
      return 'bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300';
    default:
      return 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300';
  }
}

/** Get a color for the progress bar */
function getProgressColor(progress: number): string {
  if (progress >= 75) return 'bg-green-500';
  if (progress >= 50) return 'bg-blue-500';
  if (progress >= 25) return 'bg-yellow-500';
  return 'bg-red-500';
}

// ============================================================================
// TaskCard Component
// ============================================================================

interface TaskCardProps {
  task: IncompleteTask;
  isResuming: boolean;
  resumingTaskId: string | null;
  onResume: (taskId: string) => void;
  onDiscard: (taskId: string) => void;
}

interface WorkflowRecoveryCandidate {
  sessionId: string;
  title: string;
  workspacePath: string | null;
  phase: string;
  environment: string | null;
  severity: string | null;
  statusChips: string[];
  summary: string | null;
}

function TaskCard({ task, isResuming, resumingTaskId, onResume, onDiscard }: TaskCardProps) {
  const { t } = useTranslation();
  const isThisResuming = isResuming && resumingTaskId === task.id;
  const [confirmDiscard, setConfirmDiscard] = useState(false);

  const handleDiscard = useCallback(() => {
    if (confirmDiscard) {
      onDiscard(task.id);
      setConfirmDiscard(false);
    } else {
      setConfirmDiscard(true);
      // Auto-reset confirmation after 3 seconds
      setTimeout(() => setConfirmDiscard(false), 3000);
    }
  }, [confirmDiscard, onDiscard, task.id]);

  return (
    <div
      className={clsx(
        'rounded-lg border p-4',
        'bg-white dark:bg-gray-800',
        'border-gray-200 dark:border-gray-700',
        'transition-all duration-200',
        isThisResuming && 'opacity-75',
      )}
    >
      {/* Header: Name + Mode Badge */}
      <div className="flex items-start justify-between gap-3 mb-3">
        <div className="min-w-0 flex-1">
          <h4 className="text-sm font-medium text-gray-900 dark:text-white truncate">
            {task.name || t('common:recoveryPrompt.fallbackTaskTitle', { defaultValue: 'Untitled execution' })}
          </h4>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5 truncate">{task.project_path}</p>
        </div>
        <span
          className={clsx(
            'inline-flex items-center px-2 py-0.5 rounded text-xs font-medium shrink-0',
            getModeColor(task.execution_mode),
          )}
        >
          {EXECUTION_MODE_LABELS[task.execution_mode] || task.execution_mode}
        </span>
      </div>

      {/* Progress */}
      <div className="mb-3">
        <div className="flex items-center justify-between text-xs text-gray-600 dark:text-gray-400 mb-1">
          <span>
            {task.completed_stories} / {task.total_stories} stories
          </span>
          <span>{Math.round(task.progress)}%</span>
        </div>
        <div className="w-full h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
          <div
            className={clsx('h-full rounded-full transition-all', getProgressColor(task.progress))}
            style={{ width: `${Math.min(task.progress, 100)}%` }}
          />
        </div>
      </div>

      {/* Meta info */}
      <div className="flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400 mb-3">
        <span className="flex items-center gap-1">
          <ClockIcon />
          {formatTimeAgo(task.last_checkpoint_timestamp, t)}
        </span>
        {task.checkpoint_count > 0 && (
          <span className="flex items-center gap-1">
            <CheckpointIcon />
            {t('common:recoveryPrompt.checkpoints', {
              count: task.checkpoint_count,
              defaultValue: `${task.checkpoint_count} checkpoints`,
            })}
          </span>
        )}
        {task.error_message && (
          <span className="flex items-center gap-1 text-red-500 dark:text-red-400">
            <ErrorIcon />
            {t('common:status.failed', { defaultValue: 'Failed' })}
          </span>
        )}
      </div>

      {/* Recovery note */}
      {task.recovery_note && (
        <p className="text-xs text-gray-500 dark:text-gray-400 mb-3 italic">{task.recovery_note}</p>
      )}

      {/* Actions */}
      <div className="flex items-center gap-2">
        <button
          onClick={() => onResume(task.id)}
          disabled={!task.recoverable || isResuming}
          className={clsx(
            'flex-1 px-3 py-1.5 rounded-md text-sm font-medium',
            'transition-colors duration-150',
            task.recoverable && !isResuming
              ? 'bg-primary-600 text-white hover:bg-primary-700 active:bg-primary-800'
              : 'bg-gray-200 text-gray-400 dark:bg-gray-700 dark:text-gray-500 cursor-not-allowed',
          )}
        >
          {isThisResuming ? (
            <span className="flex items-center justify-center gap-1.5">
              <SpinnerIcon />
              {t('common:recoveryPrompt.resuming', { defaultValue: 'Resuming...' })}
            </span>
          ) : (
            t('common:buttons.resume', { defaultValue: 'Resume' })
          )}
        </button>
        <button
          onClick={handleDiscard}
          disabled={isResuming}
          className={clsx(
            'px-3 py-1.5 rounded-md text-sm font-medium',
            'transition-colors duration-150',
            confirmDiscard
              ? 'bg-red-600 text-white hover:bg-red-700'
              : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600',
            isResuming && 'opacity-50 cursor-not-allowed',
          )}
        >
          {confirmDiscard
            ? t('common:recoveryPrompt.confirmDiscard', { defaultValue: 'Confirm Discard' })
            : t('common:recoveryPrompt.discard', { defaultValue: 'Discard' })}
        </button>
      </div>
    </div>
  );
}

function WorkflowRecoveryCard({
  candidate,
  isRecovering,
  onRecover,
  onDismiss,
}: {
  candidate: WorkflowRecoveryCandidate;
  isRecovering: boolean;
  onRecover: (sessionId: string, workspacePath: string | null) => void;
  onDismiss: (sessionId: string) => void;
}) {
  const { t } = useTranslation();
  const detailChips = [candidate.phase, candidate.environment, candidate.severity, ...candidate.statusChips].filter(
    (value): value is string => Boolean(value),
  );
  return (
    <div
      className={clsx(
        'rounded-lg border p-4',
        'bg-white dark:bg-gray-800',
        'border-gray-200 dark:border-gray-700',
        'transition-all duration-200',
        isRecovering && 'opacity-75',
      )}
    >
      <div className="flex items-start justify-between gap-3 mb-3">
        <div className="min-w-0 flex-1">
          <h4 className="text-sm font-medium text-gray-900 dark:text-white truncate">
            {candidate.title ||
              t('common:recoveryPrompt.fallbackDebugTitle', { defaultValue: 'Interrupted debug case' })}
          </h4>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5 truncate">
            {candidate.workspacePath || t('simpleMode:sidebar.noWorkspace', { defaultValue: 'No Workspace' })}
          </p>
        </div>
        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium shrink-0 bg-amber-100 text-amber-700 dark:bg-amber-900 dark:text-amber-300">
          {t('debugMode:modeLabel', { defaultValue: 'Debug' })}
        </span>
      </div>

      <div className="flex flex-wrap items-center gap-2 text-xs text-gray-500 dark:text-gray-400 mb-3">
        {detailChips.map((chip) => (
          <span
            key={`${candidate.sessionId}:${chip}`}
            className="inline-flex items-center rounded-full bg-gray-100 px-2 py-0.5 dark:bg-gray-700"
          >
            {chip}
          </span>
        ))}
      </div>

      {candidate.summary ? (
        <p className="text-xs text-gray-500 dark:text-gray-400 mb-3 line-clamp-2">{candidate.summary}</p>
      ) : (
        <p className="text-xs text-gray-500 dark:text-gray-400 mb-3">
          {t('common:recoveryPrompt.debugDescription', {
            defaultValue: 'This debug session was interrupted and can be recovered.',
          })}
        </p>
      )}

      <div className="flex items-center gap-2">
        <button
          onClick={() => onRecover(candidate.sessionId, candidate.workspacePath)}
          disabled={isRecovering}
          className={clsx(
            'flex-1 px-3 py-1.5 rounded-md text-sm font-medium',
            'transition-colors duration-150',
            !isRecovering
              ? 'bg-primary-600 text-white hover:bg-primary-700 active:bg-primary-800'
              : 'bg-gray-200 text-gray-400 dark:bg-gray-700 dark:text-gray-500 cursor-not-allowed',
          )}
        >
          {isRecovering ? (
            <span className="flex items-center justify-center gap-1.5">
              <SpinnerIcon />
              {t('common:recoveryPrompt.recovering', { defaultValue: 'Recovering...' })}
            </span>
          ) : (
            t('common:recoveryPrompt.recoverDebugCase', { defaultValue: 'Recover Debug Case' })
          )}
        </button>
        <button
          onClick={() => onDismiss(candidate.sessionId)}
          disabled={isRecovering}
          className={clsx(
            'px-3 py-1.5 rounded-md text-sm font-medium',
            'transition-colors duration-150',
            'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600',
            isRecovering && 'opacity-50 cursor-not-allowed',
          )}
        >
          {t('common:recoveryPrompt.dismiss', { defaultValue: 'Dismiss' })}
        </button>
      </div>
    </div>
  );
}

// ============================================================================
// RecoveryPrompt Component
// ============================================================================

export function RecoveryPrompt() {
  const { t } = useTranslation();
  const {
    incompleteTasks,
    isResuming,
    resumingTaskId,
    showPrompt,
    error,
    resumeTask,
    discardTask,
    dismissPrompt,
    clearError,
  } = useRecoveryStore();
  const workflowSessionCatalog = useWorkflowKernelStore((s) => s.sessionCatalog);
  const recoverWorkflowSession = useWorkflowKernelStore((s) => s.recoverSession);
  const setWorkspacePath = useSettingsStore((s) => s.setWorkspacePath);

  const [isVisible, setIsVisible] = useState(false);
  const [dismissedWorkflowSessionIds, setDismissedWorkflowSessionIds] = useState<Set<string>>(() => new Set());
  const [recoveringWorkflowSessionId, setRecoveringWorkflowSessionId] = useState<string | null>(null);

  const workflowRecoveryCandidates = useMemo<WorkflowRecoveryCandidate[]>(
    () =>
      workflowSessionCatalog
        .filter((session) => {
          if (session.activeMode !== 'debug') return false;
          const debugMeta = session.modeRuntimeMeta?.debug;
          return (
            session.backgroundState === 'interrupted' ||
            debugMeta?.isInterrupted === true ||
            session.status === 'failed'
          );
        })
        .filter((session) => !dismissedWorkflowSessionIds.has(session.sessionId))
        .map((session) => ({
          sessionId: session.sessionId,
          title:
            session.displayTitle ||
            session.modeSnapshots.debug?.title ||
            t('common:recoveryPrompt.fallbackDebugTitle', { defaultValue: 'Interrupted debug case' }),
          workspacePath: session.workspacePath,
          phase:
            buildDebugStateChips(session.modeSnapshots.debug, {
              includeEnvironment: false,
              includeSeverity: false,
              includeDerivedStatus: false,
              max: 1,
            })[0] || t('common:recoveryPrompt.interrupted', { defaultValue: 'Interrupted' }),
          environment:
            buildDebugStateChips(session.modeSnapshots.debug, {
              includePhase: false,
              includeSeverity: false,
              includeDerivedStatus: false,
              max: 1,
            })[0] || null,
          severity:
            buildDebugStateChips(session.modeSnapshots.debug, {
              includePhase: false,
              includeEnvironment: false,
              includeDerivedStatus: false,
              max: 1,
            })[0] || null,
          statusChips: buildDebugStateChips(session.modeSnapshots.debug, {
            includePhase: false,
            includeEnvironment: false,
            includeSeverity: false,
            max: 3,
          }),
          summary: summarizeDebugCase(session.modeSnapshots.debug, session.lastError),
        })),
    [dismissedWorkflowSessionIds, t, workflowSessionCatalog],
  );

  // Animate in when showPrompt becomes true
  useEffect(() => {
    if ((showPrompt && incompleteTasks.length > 0) || workflowRecoveryCandidates.length > 0) {
      // Small delay for slide-in animation
      const timer = setTimeout(() => setIsVisible(true), 50);
      return () => clearTimeout(timer);
    } else {
      setIsVisible(false);
    }
  }, [showPrompt, incompleteTasks.length, workflowRecoveryCandidates.length]);

  const handleResume = useCallback(
    async (taskId: string) => {
      clearError();
      await resumeTask(taskId);
    },
    [resumeTask, clearError],
  );

  const handleDiscard = useCallback(
    async (taskId: string) => {
      clearError();
      await discardTask(taskId);
    },
    [discardTask, clearError],
  );

  const handleDismiss = useCallback(() => {
    setIsVisible(false);
    if (workflowRecoveryCandidates.length > 0) {
      setDismissedWorkflowSessionIds((prev) => {
        const next = new Set(prev);
        workflowRecoveryCandidates.forEach((candidate) => next.add(candidate.sessionId));
        return next;
      });
    }
    // Wait for animation to complete before actually hiding
    setTimeout(() => dismissPrompt(), 300);
  }, [dismissPrompt, workflowRecoveryCandidates]);

  const handleRecoverWorkflow = useCallback(
    async (sessionId: string, workspacePath: string | null) => {
      setRecoveringWorkflowSessionId(sessionId);
      clearError();
      try {
        const recovered = await recoverWorkflowSession(sessionId);
        if (recovered && workspacePath) {
          setWorkspacePath(workspacePath);
        }
        if (recovered) {
          setDismissedWorkflowSessionIds((prev) => {
            const next = new Set(prev);
            next.add(sessionId);
            return next;
          });
        }
      } finally {
        setRecoveringWorkflowSessionId(null);
      }
    },
    [clearError, recoverWorkflowSession, setWorkspacePath],
  );

  // Don't render if no tasks or prompt is not shown
  if ((!showPrompt || incompleteTasks.length === 0) && workflowRecoveryCandidates.length === 0) {
    return null;
  }

  return (
    <div
      className={clsx(
        'fixed top-0 left-0 right-0 z-50',
        'transform transition-transform duration-300 ease-out',
        isVisible ? 'translate-y-0' : '-translate-y-full',
      )}
    >
      {/* Backdrop */}
      <div
        className={clsx(
          'absolute inset-0 h-screen',
          'bg-black/20 dark:bg-black/40',
          'transition-opacity duration-300',
          isVisible ? 'opacity-100' : 'opacity-0',
        )}
        onClick={handleDismiss}
      />

      {/* Panel */}
      <div
        className={clsx(
          'relative mx-auto max-w-2xl mt-4 mx-4',
          'bg-white dark:bg-gray-900',
          'rounded-xl shadow-2xl',
          'border border-gray-200 dark:border-gray-700',
          'overflow-hidden',
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3 border-b border-gray-200 dark:border-gray-700">
          <div className="flex items-center gap-2">
            <RecoveryIcon />
            <h3 className="text-sm font-semibold text-gray-900 dark:text-white">
              {t('common:recoveryPrompt.title', { defaultValue: 'Interrupted Recovery Items Detected' })}
            </h3>
            <span className="inline-flex items-center justify-center px-2 py-0.5 rounded-full text-xs font-medium bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300">
              {incompleteTasks.length + workflowRecoveryCandidates.length}
            </span>
          </div>
          <button
            onClick={handleDismiss}
            className="p-1 rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            aria-label={t('common:recoveryPrompt.dismiss', { defaultValue: 'Dismiss' })}
          >
            <CloseIcon />
          </button>
        </div>

        {/* Error banner */}
        {error && (
          <div className="px-5 py-2 bg-red-50 dark:bg-red-900/30 border-b border-red-200 dark:border-red-800">
            <p className="text-xs text-red-600 dark:text-red-400">{error}</p>
          </div>
        )}

        {/* Task list */}
        <div className="p-4 space-y-3 max-h-[60vh] overflow-y-auto">
          {incompleteTasks.map((task) => (
            <TaskCard
              key={task.id}
              task={task}
              isResuming={isResuming}
              resumingTaskId={resumingTaskId}
              onResume={handleResume}
              onDiscard={handleDiscard}
            />
          ))}
          {workflowRecoveryCandidates.map((candidate) => (
            <WorkflowRecoveryCard
              key={candidate.sessionId}
              candidate={candidate}
              isRecovering={recoveringWorkflowSessionId === candidate.sessionId}
              onRecover={handleRecoverWorkflow}
              onDismiss={(sessionId) =>
                setDismissedWorkflowSessionIds((prev) => {
                  const next = new Set(prev);
                  next.add(sessionId);
                  return next;
                })
              }
            />
          ))}
        </div>

        {/* Footer */}
        <div className="px-5 py-3 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
          <p className="text-xs text-gray-500 dark:text-gray-400 text-center">
            {t('common:recoveryPrompt.footer', {
              defaultValue:
                'These tasks or debug sessions were interrupted during a previous session. You can resume them from their last checkpoint or dismiss them.',
            })}
          </p>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Icon Components
// ============================================================================

function RecoveryIcon() {
  return (
    <svg className="w-5 h-5 text-yellow-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
      />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
    </svg>
  );
}

function ClockIcon() {
  return (
    <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  );
}

function CheckpointIcon() {
  return (
    <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
    </svg>
  );
}

function ErrorIcon() {
  return (
    <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
      />
    </svg>
  );
}

function SpinnerIcon() {
  return (
    <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );
}

export default RecoveryPrompt;
