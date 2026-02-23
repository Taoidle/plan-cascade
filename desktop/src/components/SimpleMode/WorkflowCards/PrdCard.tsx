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
import { useWorkflowOrchestratorStore } from '../../../store/workflowOrchestrator';

export function PrdCard({ data, interactive }: { data: PrdCardData; interactive: boolean }) {
  const { t } = useTranslation('simpleMode');
  const [expandedStories, setExpandedStories] = useState<Set<string>>(new Set());
  const [isEditing, setIsEditing] = useState(false);
  const phase = useWorkflowOrchestratorStore((s) => s.phase);
  const approvePrd = useWorkflowOrchestratorStore((s) => s.approvePrd);

  const isActive = interactive && phase === 'reviewing_prd';

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
    approvePrd();
  }, [approvePrd]);

  // Group stories by batch
  const storyBatchMap = new Map<string, number>();
  data.batches.forEach((batch) => {
    batch.storyIds.forEach((id) => storyBatchMap.set(id, batch.batchIndex));
  });

  return (
    <div className="rounded-lg border border-emerald-200 dark:border-emerald-800 bg-emerald-50 dark:bg-emerald-900/20 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-emerald-100/50 dark:bg-emerald-900/30 border-b border-emerald-200 dark:border-emerald-800">
        <div className="flex items-center justify-between">
          <span className="text-xs font-semibold text-emerald-700 dark:text-emerald-300 uppercase tracking-wide">
            {t('workflow.prd.title')}
          </span>
          <span className="text-2xs text-emerald-600 dark:text-emerald-400">
            {t('workflow.prd.storiesAndBatches', { stories: data.stories.length, batches: data.batches.length })}
          </span>
        </div>
        <p className="text-sm font-medium text-emerald-800 dark:text-emerald-200 mt-1">{data.title}</p>
        {data.description && (
          <p className="text-xs text-emerald-700/80 dark:text-emerald-300/80 mt-0.5">{data.description}</p>
        )}
      </div>

      {/* Story list grouped by batch */}
      <div className="divide-y divide-emerald-200/60 dark:divide-emerald-800/60">
        {data.batches.map((batch) => (
          <div key={batch.batchIndex}>
            {/* Batch header */}
            <div className="px-3 py-1 bg-emerald-100/30 dark:bg-emerald-900/20">
              <span className="text-2xs font-medium text-emerald-600 dark:text-emerald-400">
                {t('workflow.prd.batch', { index: batch.batchIndex + 1 })}
                <span className="ml-1 text-emerald-500/60 dark:text-emerald-400/60">
                  ({batch.storyIds.length === 1 ? t('workflow.prd.storyCount', { count: batch.storyIds.length }) : t('workflow.prd.storyCountPlural', { count: batch.storyIds.length })})
                </span>
              </span>
            </div>

            {/* Stories in this batch */}
            {batch.storyIds.map((storyId) => {
              const story = data.stories.find((s) => s.id === storyId);
              if (!story) return null;
              return (
                <StoryRow
                  key={story.id}
                  story={story}
                  expanded={expandedStories.has(story.id)}
                  onToggle={() => toggleStory(story.id)}
                  isEditing={isEditing}
                />
              );
            })}
          </div>
        ))}

        {/* Stories not in any batch (edge case) */}
        {data.stories
          .filter((s) => !storyBatchMap.has(s.id))
          .map((story) => (
            <StoryRow
              key={story.id}
              story={story}
              expanded={expandedStories.has(story.id)}
              onToggle={() => toggleStory(story.id)}
              isEditing={isEditing}
            />
          ))}
      </div>

      {/* Actions */}
      {isActive && (
        <div className="px-3 py-2 border-t border-emerald-200 dark:border-emerald-800 flex items-center gap-2">
          <button
            onClick={handleApprove}
            className="px-3 py-1.5 text-xs font-medium rounded-md bg-emerald-600 text-white hover:bg-emerald-700 transition-colors"
          >
            {t('workflow.prd.approveAndExecute')}
          </button>
          <button
            onClick={() => setIsEditing(!isEditing)}
            className={clsx(
              'px-3 py-1.5 text-xs font-medium rounded-md border transition-colors',
              isEditing
                ? 'border-emerald-600 bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300'
                : 'border-emerald-300 dark:border-emerald-700 text-emerald-600 dark:text-emerald-400 hover:bg-emerald-100/50 dark:hover:bg-emerald-900/30'
            )}
          >
            {isEditing ? t('workflow.prd.doneEditing') : t('workflow.prd.edit')}
          </button>
        </div>
      )}
    </div>
  );
}

function StoryRow({
  story,
  expanded,
  onToggle,
  isEditing: _isEditing,
}: {
  story: PrdStoryData;
  expanded: boolean;
  onToggle: () => void;
  isEditing: boolean;
}) {
  const { t } = useTranslation('simpleMode');
  const priorityColor = {
    high: 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400',
    medium: 'bg-amber-100 dark:bg-amber-900/30 text-amber-600 dark:text-amber-400',
    low: 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400',
  }[story.priority] || 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400';

  return (
    <div className="px-3 py-1.5">
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-2 text-left group"
      >
        <svg
          className={clsx(
            'w-3 h-3 shrink-0 text-emerald-500 dark:text-emerald-400 transition-transform',
            expanded && 'rotate-90'
          )}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
        </svg>

        <span className="text-2xs text-emerald-500/60 dark:text-emerald-400/60 shrink-0 w-14">
          {story.id}
        </span>

        <span className="text-xs text-emerald-800 dark:text-emerald-200 flex-1 truncate group-hover:underline">
          {story.title}
        </span>

        <span className={clsx('text-2xs px-1.5 py-0.5 rounded', priorityColor)}>
          {story.priority}
        </span>

        {story.dependencies.length > 0 && (
          <span className="text-2xs text-emerald-500/50 dark:text-emerald-400/50">
            {t('workflow.prd.deps', { deps: story.dependencies.join(', ') })}
          </span>
        )}
      </button>

      {expanded && (
        <div className="ml-5 mt-1 space-y-1">
          <p className="text-xs text-emerald-700/80 dark:text-emerald-300/80">{story.description}</p>

          {story.acceptanceCriteria.length > 0 && (
            <div className="space-y-0.5">
              <span className="text-2xs font-medium text-emerald-600 dark:text-emerald-400">{t('workflow.prd.acceptanceCriteria')}</span>
              {story.acceptanceCriteria.map((ac, i) => (
                <p key={i} className="text-2xs text-emerald-600/70 dark:text-emerald-400/70 pl-2">
                  • {ac}
                </p>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
