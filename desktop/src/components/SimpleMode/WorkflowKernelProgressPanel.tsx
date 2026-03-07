/**
 * WorkflowKernelProgressPanel
 *
 * Session-state-driven workflow progress + event timeline panel.
 */

import { useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import type { WorkflowMode, WorkflowEventV2 } from '../../types/workflowKernel';

interface WorkflowKernelProgressPanelProps {
  workflowMode: WorkflowMode;
  workflowPhase: string;
}

const CHAT_PHASES = ['ready', 'submitting', 'streaming', 'paused', 'failed', 'cancelled', 'interrupted'];
const PLAN_PHASES = [
  'idle',
  'analyzing',
  'clarifying',
  'clarification_error',
  'planning',
  'reviewing_plan',
  'executing',
  'completed',
  'failed',
  'cancelled',
];
const TASK_PHASES = [
  'idle',
  'analyzing',
  'configuring',
  'interviewing',
  'exploring',
  'requirement_analysis',
  'generating_prd',
  'reviewing_prd',
  'architecture_review',
  'generating_design_doc',
  'executing',
  'completed',
  'failed',
  'cancelled',
];

function humanizeSnakeCase(value: string): string {
  const normalized = value.replace(/[_-]+/g, ' ').trim();
  if (!normalized) return value;
  return normalized.charAt(0).toUpperCase() + normalized.slice(1);
}

function getPhases(mode: WorkflowMode): string[] {
  if (mode === 'plan') return PLAN_PHASES;
  if (mode === 'task') return TASK_PHASES;
  return CHAT_PHASES;
}

function modeLabel(t: TFunction<'simpleMode'>, mode: string): string {
  return t(`workflow.progress.kernel.mode.${mode}`, {
    defaultValue: humanizeSnakeCase(mode),
  });
}

function statusLabel(t: TFunction<'simpleMode'>, status: string): string {
  return t(`workflow.progress.kernel.status.${status}`, {
    defaultValue: humanizeSnakeCase(status),
  });
}

function phaseLabel(t: TFunction<'simpleMode'>, phase: string): string {
  return t(`workflow.progress.kernel.phase.${phase}`, {
    defaultValue: humanizeSnakeCase(phase),
  });
}

function eventKindLabel(t: TFunction<'simpleMode'>, kind: string): string {
  return t(`workflow.progress.kernel.eventKind.${kind}`, {
    defaultValue: humanizeSnakeCase(kind),
  });
}

function normalizeReasonCode(value: string): string {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
  return normalized || 'unknown_reason';
}

function reasonCodeLabel(t: TFunction<'simpleMode'>, reasonCode: string, defaultValue?: string): string {
  return t(`workflow.progress.kernel.reasonCode.${reasonCode}`, {
    defaultValue: defaultValue ?? humanizeSnakeCase(reasonCode),
  });
}

function summarizeEvent(t: TFunction<'simpleMode'>, event: WorkflowEventV2): string {
  const payload = event.payload ?? {};
  if ('reasonCode' in payload && typeof payload.reasonCode === 'string') {
    return reasonCodeLabel(t, normalizeReasonCode(payload.reasonCode));
  }
  if ('reason' in payload && typeof payload.reason === 'string') {
    return reasonCodeLabel(t, normalizeReasonCode(payload.reason), payload.reason);
  }
  if ('stepId' in payload && typeof payload.stepId === 'string') {
    return t('workflow.progress.kernel.eventSummary.step', {
      stepId: payload.stepId,
      defaultValue: `step ${payload.stepId}`,
    });
  }
  if ('sourceMode' in payload && 'targetMode' in payload) {
    return t('workflow.progress.kernel.eventSummary.transition', {
      source: modeLabel(t, String(payload.sourceMode)),
      target: modeLabel(t, String(payload.targetMode)),
      defaultValue: `${String(payload.sourceMode)} -> ${String(payload.targetMode)}`,
    });
  }
  if ('phase' in payload && typeof payload.phase === 'string') {
    return t('workflow.progress.kernel.eventSummary.phase', {
      phase: phaseLabel(t, payload.phase),
      defaultValue: `phase ${payload.phase}`,
    });
  }
  if ('eventCount' in payload && typeof payload.eventCount === 'number') {
    return t('workflow.progress.kernel.eventSummary.events', {
      count: payload.eventCount,
      defaultValue: `events ${payload.eventCount}`,
    });
  }
  return '';
}

function formatEventTime(ts: string): string {
  const date = new Date(ts);
  if (Number.isNaN(date.getTime())) return ts;
  return date.toLocaleTimeString();
}

export function WorkflowKernelProgressPanel({ workflowMode, workflowPhase }: WorkflowKernelProgressPanelProps) {
  const { t } = useTranslation('simpleMode');
  const session = useWorkflowKernelStore((s) => s.session);
  const events = useWorkflowKernelStore((s) => s.events);
  const checkpoints = useWorkflowKernelStore((s) => s.checkpoints);
  const error = useWorkflowKernelStore((s) => s.error);

  const activeMode = session?.activeMode ?? workflowMode;
  const fallbackPhase = activeMode === 'chat' ? 'ready' : 'idle';
  const activePhase =
    activeMode === 'task'
      ? (session?.modeSnapshots.task?.phase ?? (workflowMode === 'task' ? workflowPhase : fallbackPhase))
      : activeMode === 'plan'
        ? (session?.modeSnapshots.plan?.phase ?? (workflowMode === 'plan' ? workflowPhase : fallbackPhase))
        : (session?.modeSnapshots.chat?.phase ?? (workflowMode === 'chat' ? workflowPhase : fallbackPhase));

  const phaseList = getPhases(activeMode);
  const sessionStatus = session?.status ?? 'active';
  const isChatMode = activeMode === 'chat';
  const isCompletedState = !isChatMode && (activePhase === 'completed' || sessionStatus === 'completed');
  const isFailedState = activePhase === 'failed' || sessionStatus === 'failed';
  const isCancelledState = activePhase === 'cancelled' || sessionStatus === 'cancelled';
  const isInterruptedState = activePhase === 'interrupted';
  const isTerminalState = isCompletedState || isFailedState || isCancelledState;
  const isInProgressState = isChatMode
    ? activePhase === 'submitting' || activePhase === 'streaming' || activePhase === 'paused'
    : !isTerminalState;
  const rawPhaseIndex = phaseList.indexOf(activePhase);
  const phaseIndex = rawPhaseIndex >= 0 ? rawPhaseIndex : isTerminalState ? phaseList.length - 1 : 0;
  const recentEvents = useMemo(() => [...events].slice(-20).reverse(), [events]);

  return (
    <div className="p-3 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 space-y-3">
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0">
          <p className="text-xs font-medium text-gray-700 dark:text-gray-300">
            {t('workflow.progress.title', { defaultValue: 'Workflow Progress' })}
          </p>
          <p className="text-2xs text-gray-500 dark:text-gray-400 truncate">
            {session?.sessionId ?? t('workflow.progress.kernel.noSession', { defaultValue: 'No workflow session' })}
          </p>
        </div>
        <div className="text-right">
          <p className="text-xs font-semibold text-gray-700 dark:text-gray-200">{modeLabel(t, activeMode)}</p>
          <p className="text-2xs text-gray-500 dark:text-gray-400">{statusLabel(t, session?.status ?? 'inactive')}</p>
        </div>
      </div>

      <div>
        <div className="flex items-center justify-between mb-1">
          <span className="text-2xs text-gray-500 dark:text-gray-400">
            {t('workflow.progress.kernel.labels.phase', { defaultValue: 'phase' })}
          </span>
          <span className="text-2xs font-medium text-gray-700 dark:text-gray-300">{phaseLabel(t, activePhase)}</span>
        </div>
        <div className="flex items-center gap-1">
          {phaseList.map((phase, index) => (
            <div key={phase} className="flex-1">
              <div
                className={clsx(
                  'h-1.5 rounded-full transition-colors',
                  index < phaseIndex
                    ? 'bg-green-500'
                    : index === phaseIndex
                      ? isCompletedState
                        ? 'bg-green-500'
                        : isFailedState
                          ? 'bg-red-500'
                          : isCancelledState
                            ? 'bg-amber-500'
                            : isInterruptedState
                              ? 'bg-slate-400'
                              : isInProgressState
                                ? 'bg-blue-500 animate-pulse'
                                : 'bg-slate-300 dark:bg-slate-600'
                      : 'bg-gray-200 dark:bg-gray-700',
                )}
              />
              <p className="mt-1 text-2xs text-gray-400 dark:text-gray-500 truncate">{phaseLabel(t, phase)}</p>
            </div>
          ))}
        </div>
      </div>

      <div className="grid grid-cols-3 gap-2 text-2xs">
        <div className="rounded border border-gray-200 dark:border-gray-700 px-2 py-1">
          <p className="text-gray-500 dark:text-gray-400">
            {t('workflow.progress.kernel.labels.events', { defaultValue: 'events' })}
          </p>
          <p className="text-gray-700 dark:text-gray-300 font-medium">{events.length}</p>
        </div>
        <div className="rounded border border-gray-200 dark:border-gray-700 px-2 py-1">
          <p className="text-gray-500 dark:text-gray-400">
            {t('workflow.progress.kernel.labels.checkpoints', { defaultValue: 'checkpoints' })}
          </p>
          <p className="text-gray-700 dark:text-gray-300 font-medium">{checkpoints.length}</p>
        </div>
        <div className="rounded border border-gray-200 dark:border-gray-700 px-2 py-1">
          <p className="text-gray-500 dark:text-gray-400">
            {t('workflow.progress.kernel.labels.updated', { defaultValue: 'updated' })}
          </p>
          <p className="text-gray-700 dark:text-gray-300 font-medium">
            {session?.updatedAt ? formatEventTime(session.updatedAt) : '-'}
          </p>
        </div>
      </div>

      {error && (
        <div className="rounded border border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20 px-2 py-1.5 text-2xs text-red-600 dark:text-red-300">
          {error}
        </div>
      )}

      {recentEvents.length > 0 && (
        <div>
          <p className="text-xs font-medium text-gray-700 dark:text-gray-300 mb-1.5">
            {t('workflow.progress.kernel.labels.timeline', { defaultValue: 'Event Timeline' })}
          </p>
          <div className="max-h-28 overflow-y-auto rounded border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-950 p-2 space-y-1.5">
            {recentEvents.map((event) => {
              const summary = summarizeEvent(t, event);
              return (
                <div key={event.eventId} className="text-2xs">
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium text-gray-700 dark:text-gray-300 truncate">
                      {eventKindLabel(t, event.kind)} [{modeLabel(t, event.mode)}]
                    </span>
                    <span className="text-gray-500 dark:text-gray-400 shrink-0">
                      {formatEventTime(event.createdAt)}
                    </span>
                  </div>
                  {summary && <p className="text-gray-500 dark:text-gray-400 truncate">{summary}</p>}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
