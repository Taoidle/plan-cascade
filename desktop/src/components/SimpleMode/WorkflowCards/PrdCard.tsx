/**
 * PrdCard
 *
 * Displays the generated PRD with collapsible story list, dependency badges,
 * batch grouping, and interactive Approve & Execute / Edit buttons.
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { PrdCardData, PrdStoryData } from '../../../types/workflowCard';
import { Collapsible } from '../Collapsible';
import { useWorkflowOrchestratorStore } from '../../../store/workflowOrchestrator';
import type { TaskStory } from '../../../store/taskMode';
import { useWorkflowKernelStore } from '../../../store/workflowKernel';
import { submitWorkflowActionIntentViaCoordinator } from '../../../store/simpleWorkflowCoordinator';
import { reportInteractiveActionFailure } from '../../../lib/workflowObservability';
import type { StreamLine } from '../../../store/execution';

export function PrdCard({ data, interactive, cardId }: { data: PrdCardData; interactive: boolean; cardId?: string }) {
  const { t } = useTranslation('simpleMode');
  const [expandedStories, setExpandedStories] = useState<Set<string>>(new Set());
  const [isEditing, setIsEditing] = useState(false);
  const [isApproving, setIsApproving] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const approvePrd = useWorkflowOrchestratorStore((s) => s.approvePrd);
  const editablePrd = useWorkflowOrchestratorStore((s) => s.editablePrd);
  const updateEditableStory = useWorkflowOrchestratorStore((s) => s.updateEditableStory);
  const orchestratorPhase = useWorkflowOrchestratorStore((s) => s.phase);
  const workflowSession = useWorkflowKernelStore((s) => s.session);
  const latestInteractivePrdCardId = useWorkflowKernelStore((state) => {
    const rootSessionId = state.session?.sessionId;
    if (!rootSessionId) return null;
    const lines = state.getCachedModeTranscript(rootSessionId, 'task').lines as StreamLine[];
    for (let index = lines.length - 1; index >= 0; index -= 1) {
      const line = lines[index];
      if (line.type !== 'card' || !line.cardPayload?.interactive) continue;
      if (line.cardPayload.cardType === 'prd_card') {
        return line.cardPayload.cardId;
      }
    }
    return null;
  });

  const isKernelTaskActive = workflowSession?.status === 'active' && workflowSession.activeMode === 'task';
  const kernelTaskPhase = workflowSession?.modeSnapshots.task?.phase ?? 'idle';
  const isLatestCard = !cardId || !latestInteractivePrdCardId || latestInteractivePrdCardId === cardId;
  const isReviewingPrd = orchestratorPhase === 'reviewing_prd' || kernelTaskPhase === 'reviewing_prd';
  const isActive = interactive && isReviewingPrd && isKernelTaskActive && isLatestCard;
  const primaryAction = data.primaryAction ?? 'approve_and_execute';
  const primaryActionLabel =
    primaryAction === 'submit_architecture_review'
      ? t('workflow.prd.submitArchitectureReview')
      : t('workflow.prd.approveAndExecute');

  // When editing, read stories from the live editablePrd; otherwise use the snapshot card data
  const displayStories: PrdStoryData[] = isEditing && editablePrd ? editablePrd.stories : data.stories;

  const toggleStory = useCallback((storyId: string) => {
    setExpandedStories((prev) => {
      const next = new Set(prev);
      if (next.has(storyId)) {
        next.delete(storyId);
      } else {
        next.add(storyId);
      }
      return next;
    });
  }, []);

  const handleApprove = useCallback(() => {
    if (!isActive || isApproving) return;
    setIsApproving(true);
    setSubmitError(null);
    void (async () => {
      try {
        await submitWorkflowActionIntentViaCoordinator({
          mode: 'task',
          type: 'execution_control',
          source: 'prd_card',
          action:
            primaryAction === 'submit_architecture_review' ? 'submit_architecture_review' : 'approve_prd_execution',
          content:
            primaryAction === 'submit_architecture_review' ? 'submit_architecture_review' : 'approve_prd_execution',
          metadata: {
            storyCount: data.stories.length,
            batchCount: data.batches.length,
            isEdited: !!editablePrd,
            primaryAction,
          },
        });
      } catch {
        // Keep orchestration available even if kernel logging fails.
      }
      try {
        const result = await approvePrd();
        if (!result.ok) {
          const message = result.message || 'Failed to approve PRD';
          setSubmitError(message);
          await reportInteractiveActionFailure({
            card: 'prd_card',
            action:
              primaryAction === 'submit_architecture_review' ? 'submit_architecture_review' : 'approve_prd_execution',
            errorCode:
              result.errorCode ||
              (primaryAction === 'submit_architecture_review'
                ? 'submit_architecture_review_failed'
                : 'approve_prd_execution_failed'),
            message,
            session: workflowSession,
          });
        }
      } finally {
        setIsApproving(false);
      }
    })();
  }, [
    approvePrd,
    data.batches.length,
    data.stories.length,
    editablePrd,
    isActive,
    isApproving,
    primaryAction,
    workflowSession,
  ]);

  // Group stories by batch
  const storyBatchMap = new Map<string, number>();
  data.batches.forEach((batch) => {
    batch.storyIds.forEach((id) => storyBatchMap.set(id, batch.index));
  });

  return (
    <div className="rounded-lg border border-emerald-200 dark:border-emerald-800 bg-emerald-50 dark:bg-emerald-900/20 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-emerald-100/50 dark:bg-emerald-900/30 border-b border-emerald-200 dark:border-emerald-800">
        <div className="flex items-center justify-between">
          <span className="text-xs font-semibold text-emerald-700 dark:text-emerald-300 uppercase tracking-wide">
            {t('workflow.prd.title')}
          </span>
          <div className="flex items-center gap-2">
            {data.revisionSource === 'architecture_updated' && (
              <span className="text-2xs px-1.5 py-0.5 rounded bg-cyan-200 dark:bg-cyan-800 text-cyan-700 dark:text-cyan-300">
                {t('workflow.prd.architectureUpdated')}
              </span>
            )}
            <span className="text-2xs text-emerald-600 dark:text-emerald-400">
              {t('workflow.prd.storiesAndBatches', { stories: data.stories.length, batches: data.batches.length })}
            </span>
          </div>
        </div>
        <p className="text-sm font-medium text-emerald-800 dark:text-emerald-200 mt-1">{data.title}</p>
        {data.description && (
          <p className="text-xs text-emerald-700/80 dark:text-emerald-300/80 mt-0.5">{data.description}</p>
        )}
      </div>

      {/* Story list grouped by batch */}
      <div className="divide-y divide-emerald-200/60 dark:divide-emerald-800/60">
        {data.batches.map((batch, batchIdx) => (
          <div key={batch.index ?? batchIdx}>
            {/* Batch header */}
            <div className="px-3 py-1 bg-emerald-100/30 dark:bg-emerald-900/20">
              <span className="text-2xs font-medium text-emerald-600 dark:text-emerald-400">
                {t('workflow.prd.batch', { index: (batch.index ?? batchIdx) + 1 })}
                <span className="ml-1 text-emerald-500/60 dark:text-emerald-400/60">
                  (
                  {batch.storyIds.length === 1
                    ? t('workflow.prd.storyCount', { count: batch.storyIds.length })
                    : t('workflow.prd.storyCountPlural', { count: batch.storyIds.length })}
                  )
                </span>
              </span>
            </div>

            {/* Stories in this batch */}
            {batch.storyIds.map((storyId) => {
              const story = displayStories.find((s) => s.id === storyId);
              if (!story) return null;
              return (
                <StoryRow
                  key={story.id}
                  story={story}
                  expanded={expandedStories.has(story.id)}
                  onToggle={() => toggleStory(story.id)}
                  isEditing={isEditing}
                  onUpdate={updateEditableStory}
                />
              );
            })}
          </div>
        ))}

        {/* Stories not in any batch (edge case) */}
        {displayStories
          .filter((s) => !storyBatchMap.has(s.id))
          .map((story) => (
            <StoryRow
              key={story.id}
              story={story}
              expanded={expandedStories.has(story.id)}
              onToggle={() => toggleStory(story.id)}
              isEditing={isEditing}
              onUpdate={updateEditableStory}
            />
          ))}
      </div>

      {/* Actions */}
      {isActive && (
        <div className="px-3 py-2 border-t border-emerald-200 dark:border-emerald-800 flex items-center gap-2">
          <button
            onClick={handleApprove}
            disabled={isApproving}
            className="px-3 py-1.5 text-xs font-medium rounded-md bg-emerald-600 text-white hover:bg-emerald-700 disabled:opacity-60 disabled:cursor-not-allowed transition-colors"
          >
            {primaryActionLabel}
          </button>
          <button
            onClick={() => setIsEditing(!isEditing)}
            className={clsx(
              'px-3 py-1.5 text-xs font-medium rounded-md border transition-colors',
              isEditing
                ? 'border-emerald-600 bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300'
                : 'border-emerald-300 dark:border-emerald-700 text-emerald-600 dark:text-emerald-400 hover:bg-emerald-100/50 dark:hover:bg-emerald-900/30',
            )}
          >
            {isEditing ? t('workflow.prd.doneEditing') : t('workflow.prd.edit')}
          </button>
          {submitError && <span className="text-2xs text-rose-600 dark:text-rose-300">{submitError}</span>}
        </div>
      )}
    </div>
  );
}

function StoryRow({
  story,
  expanded,
  onToggle,
  isEditing,
  onUpdate,
}: {
  story: PrdStoryData;
  expanded: boolean;
  onToggle: () => void;
  isEditing: boolean;
  onUpdate: (
    storyId: string,
    updates: Partial<Pick<TaskStory, 'title' | 'description' | 'priority' | 'acceptanceCriteria'>>,
  ) => void;
}) {
  const { t } = useTranslation('simpleMode');
  const priorityColor =
    {
      high: 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400',
      medium: 'bg-amber-100 dark:bg-amber-900/30 text-amber-600 dark:text-amber-400',
      low: 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400',
    }[story.priority] || 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400';

  const inputClass =
    'w-full px-1.5 py-0.5 text-xs rounded border border-emerald-300 dark:border-emerald-700 bg-white dark:bg-gray-800 text-emerald-800 dark:text-emerald-200 focus:outline-none focus:ring-1 focus:ring-emerald-500';

  return (
    <div className="px-3 py-1.5">
      <button onClick={onToggle} className="w-full flex items-center gap-2 text-left group">
        <svg
          className={clsx(
            'w-3 h-3 shrink-0 text-emerald-500 dark:text-emerald-400 transition-transform',
            expanded && 'rotate-90',
          )}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
        </svg>

        <span className="text-2xs text-emerald-500/60 dark:text-emerald-400/60 shrink-0 w-14">{story.id}</span>

        {isEditing ? (
          <input
            className={clsx(inputClass, 'flex-1')}
            value={story.title}
            onClick={(e) => e.stopPropagation()}
            onChange={(e) => onUpdate(story.id, { title: e.target.value })}
          />
        ) : (
          <span className="text-xs text-emerald-800 dark:text-emerald-200 flex-1 truncate group-hover:underline">
            {story.title}
          </span>
        )}

        {isEditing ? (
          <select
            className="text-2xs px-1.5 py-0.5 rounded border border-emerald-300 dark:border-emerald-700 bg-white dark:bg-gray-800 text-emerald-700 dark:text-emerald-300"
            value={story.priority}
            onClick={(e) => e.stopPropagation()}
            onChange={(e) => onUpdate(story.id, { priority: e.target.value })}
          >
            <option value="high">{t('workflow.prd.priorityHigh')}</option>
            <option value="medium">{t('workflow.prd.priorityMedium')}</option>
            <option value="low">{t('workflow.prd.priorityLow')}</option>
          </select>
        ) : (
          <span className={clsx('text-2xs px-1.5 py-0.5 rounded', priorityColor)}>{story.priority}</span>
        )}

        {!isEditing && story.dependencies.length > 0 && (
          <span className="text-2xs text-emerald-500/50 dark:text-emerald-400/50">
            {t('workflow.prd.deps', { deps: story.dependencies.join(', ') })}
          </span>
        )}
      </button>

      <Collapsible open={expanded}>
        <div className="ml-5 mt-1 space-y-1">
          {isEditing ? (
            <textarea
              className={clsx(inputClass, 'min-h-[3rem] resize-y')}
              value={story.description}
              onChange={(e) => onUpdate(story.id, { description: e.target.value })}
            />
          ) : (
            <p className="text-xs text-emerald-700/80 dark:text-emerald-300/80">{story.description}</p>
          )}

          {story.acceptanceCriteria.length > 0 && (
            <div className="space-y-0.5">
              <span className="text-2xs font-medium text-emerald-600 dark:text-emerald-400">
                {t('workflow.prd.acceptanceCriteria')}
              </span>
              {story.acceptanceCriteria.map((ac, i) => (
                <p key={i} className="text-2xs text-emerald-600/70 dark:text-emerald-400/70 pl-2">
                  • {ac}
                </p>
              ))}
            </div>
          )}
        </div>
      </Collapsible>
    </div>
  );
}
