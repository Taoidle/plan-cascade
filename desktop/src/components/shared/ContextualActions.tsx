/**
 * ContextualActions Component
 *
 * Renders action buttons that update dynamically based on the current
 * mode and execution state. Shows relevant quick-actions for the
 * user's current workflow position.
 *
 * Story 005: Navigation Flow Refinement
 */

import { useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  PlayIcon,
  StopIcon,
  ResetIcon,
  PlusIcon,
  DownloadIcon,
} from '@radix-ui/react-icons';
import { Mode, useModeStore } from '../../store/mode';
import { useExecutionStore, ExecutionStatus } from '../../store/execution';

// ============================================================================
// Types
// ============================================================================

interface ContextualAction {
  id: string;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  onClick: () => void;
  variant: 'primary' | 'secondary' | 'danger';
  disabled?: boolean;
  title?: string;
}

interface ContextualActionsProps {
  className?: string;
  onStartExecution?: () => void;
  onPauseExecution?: () => void;
  onResumeExecution?: () => void;
  onCancelExecution?: () => void;
  onResetExecution?: () => void;
  onNewChat?: () => void;
  onExportChat?: () => void;
  onRefreshProjects?: () => void;
  onRefreshAnalytics?: () => void;
}

// ============================================================================
// Variant Styles
// ============================================================================

const VARIANT_CLASSES: Record<string, string> = {
  primary: clsx(
    'bg-primary-600 hover:bg-primary-700',
    'text-white',
    'shadow-sm'
  ),
  secondary: clsx(
    'bg-gray-100 dark:bg-gray-800',
    'hover:bg-gray-200 dark:hover:bg-gray-700',
    'text-gray-700 dark:text-gray-300',
    'border border-gray-200 dark:border-gray-700'
  ),
  danger: clsx(
    'bg-red-600 hover:bg-red-700',
    'text-white',
    'shadow-sm'
  ),
};

// ============================================================================
// ContextualActions Component
// ============================================================================

export function ContextualActions({
  className,
  onStartExecution,
  onPauseExecution,
  onResumeExecution,
  onCancelExecution,
  onResetExecution,
  onNewChat,
  onExportChat,
  onRefreshProjects,
  onRefreshAnalytics,
}: ContextualActionsProps) {
  const { t } = useTranslation();
  const { mode } = useModeStore();
  const { status } = useExecutionStore();

  const actions = useMemo(() => {
    return getActionsForContext(mode, status, {
      t,
      onStartExecution,
      onPauseExecution,
      onResumeExecution,
      onCancelExecution,
      onResetExecution,
      onNewChat,
      onExportChat,
      onRefreshProjects,
      onRefreshAnalytics,
    });
  }, [
    mode,
    status,
    t,
    onStartExecution,
    onPauseExecution,
    onResumeExecution,
    onCancelExecution,
    onResetExecution,
    onNewChat,
    onExportChat,
    onRefreshProjects,
    onRefreshAnalytics,
  ]);

  if (actions.length === 0) return null;

  return (
    <div
      className={clsx('flex items-center gap-2', className)}
      role="toolbar"
      aria-label="Contextual actions"
    >
      {actions.map((action) => (
        <button
          key={action.id}
          onClick={action.onClick}
          disabled={action.disabled}
          title={action.title || action.label}
          className={clsx(
            'inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg',
            'text-sm font-medium',
            'transition-all duration-150',
            'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            VARIANT_CLASSES[action.variant]
          )}
        >
          <action.icon className="w-4 h-4" />
          <span className="hidden sm:inline">{action.label}</span>
        </button>
      ))}
    </div>
  );
}

// ============================================================================
// Action Builder
// ============================================================================

/* eslint-disable @typescript-eslint/no-explicit-any */
function getActionsForContext(
  mode: Mode,
  status: ExecutionStatus,
  ctx: {
    t: any;
    onStartExecution?: () => void;
    onPauseExecution?: () => void;
    onResumeExecution?: () => void;
    onCancelExecution?: () => void;
    onResetExecution?: () => void;
    onNewChat?: () => void;
    onExportChat?: () => void;
    onRefreshProjects?: () => void;
    onRefreshAnalytics?: () => void;
  }
): ContextualAction[] {
  const actions: ContextualAction[] = [];

  // --- Simple / Expert mode: execution-state-driven actions ---
  if (mode === 'simple' || mode === 'expert') {
    if (status === 'idle') {
      if (ctx.onStartExecution) {
        actions.push({
          id: 'start-execution',
          label: 'Execute',
          icon: PlayIcon,
          onClick: ctx.onStartExecution,
          variant: 'primary',
          title: 'Start execution',
        });
      }
    }

    if (status === 'running') {
      if (ctx.onPauseExecution) {
        actions.push({
          id: 'pause-execution',
          label: 'Pause',
          icon: StopIcon,
          onClick: ctx.onPauseExecution,
          variant: 'secondary',
          title: 'Pause execution',
        });
      }
      if (ctx.onCancelExecution) {
        actions.push({
          id: 'cancel-execution',
          label: 'Cancel',
          icon: StopIcon,
          onClick: ctx.onCancelExecution,
          variant: 'danger',
          title: 'Cancel execution',
        });
      }
    }

    if (status === 'paused') {
      if (ctx.onResumeExecution) {
        actions.push({
          id: 'resume-execution',
          label: 'Resume',
          icon: PlayIcon,
          onClick: ctx.onResumeExecution,
          variant: 'primary',
          title: 'Resume execution',
        });
      }
      if (ctx.onCancelExecution) {
        actions.push({
          id: 'cancel-execution',
          label: 'Cancel',
          icon: StopIcon,
          onClick: ctx.onCancelExecution,
          variant: 'danger',
          title: 'Cancel execution',
        });
      }
    }

    if (status === 'completed' || status === 'failed') {
      if (ctx.onResetExecution) {
        actions.push({
          id: 'reset-execution',
          label: 'New Task',
          icon: ResetIcon,
          onClick: ctx.onResetExecution,
          variant: 'secondary',
          title: 'Reset and start a new task',
        });
      }
    }
  }

  // --- Claude Code mode ---
  if (mode === 'claude-code') {
    if (ctx.onNewChat) {
      actions.push({
        id: 'new-chat',
        label: 'New Chat',
        icon: PlusIcon,
        onClick: ctx.onNewChat,
        variant: 'primary',
        title: 'Start a new conversation',
      });
    }
    if (ctx.onExportChat) {
      actions.push({
        id: 'export-chat',
        label: 'Export',
        icon: DownloadIcon,
        onClick: ctx.onExportChat,
        variant: 'secondary',
        title: 'Export conversation',
      });
    }
  }

  // --- Projects mode ---
  if (mode === 'projects') {
    if (ctx.onRefreshProjects) {
      actions.push({
        id: 'refresh-projects',
        label: 'Refresh',
        icon: ResetIcon,
        onClick: ctx.onRefreshProjects,
        variant: 'secondary',
        title: 'Refresh project list',
      });
    }
  }

  // --- Analytics mode ---
  if (mode === 'analytics') {
    if (ctx.onRefreshAnalytics) {
      actions.push({
        id: 'refresh-analytics',
        label: 'Refresh',
        icon: ResetIcon,
        onClick: ctx.onRefreshAnalytics,
        variant: 'secondary',
        title: 'Refresh analytics data',
      });
    }
  }

  return actions;
}

export default ContextualActions;
