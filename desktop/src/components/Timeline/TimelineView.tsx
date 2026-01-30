/**
 * TimelineView Component
 *
 * Main timeline visualization component showing checkpoints as nodes
 * along a vertical axis with branch indicators.
 */

import { useEffect, useMemo, useCallback, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  PlusIcon,
  Link2Icon,
  ChevronDownIcon,
} from '@radix-ui/react-icons';
import { useTimelineStore } from '../../store/timeline';
import { TimelineSkeleton } from './TimelineSkeleton';
import { CheckpointNode } from './CheckpointNode';
import type { Checkpoint, CheckpointBranch } from '../../types/timeline';

interface TimelineViewProps {
  projectPath: string;
  sessionId: string;
  trackedFiles?: string[];
  onCheckpointCreate?: (checkpoint: Checkpoint) => void;
  onCheckpointRestore?: (checkpointId: string) => void;
}

export function TimelineView({
  projectPath,
  sessionId,
  trackedFiles = [],
  onCheckpointCreate,
  onCheckpointRestore,
}: TimelineViewProps) {
  const { t } = useTranslation();
  const [showBranchSelector, setShowBranchSelector] = useState(false);
  const [newCheckpointLabel, setNewCheckpointLabel] = useState('');
  const [showCreateForm, setShowCreateForm] = useState(false);

  const {
    timeline,
    selectedCheckpoint,
    loading,
    error,
    setSession,
    createCheckpoint,
    selectCheckpoint,
    forkBranch,
    switchBranch,
  } = useTimelineStore();

  // Initialize session
  useEffect(() => {
    setSession(projectPath, sessionId);
  }, [projectPath, sessionId, setSession]);

  // Get current branch
  const currentBranch = useMemo(() => {
    if (!timeline?.current_branch_id || !timeline.branches) return null;
    return timeline.branches.find((b) => b.id === timeline.current_branch_id) || null;
  }, [timeline]);

  // Get checkpoints sorted by timestamp (newest first)
  const sortedCheckpoints = useMemo(() => {
    if (!timeline?.checkpoints) return [];
    return [...timeline.checkpoints].sort(
      (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
    );
  }, [timeline]);

  // Get checkpoints for current branch view
  const branchCheckpoints = useMemo(() => {
    if (!timeline?.current_branch_id) return sortedCheckpoints;
    return sortedCheckpoints.filter(
      (cp) =>
        cp.branch_id === timeline.current_branch_id ||
        // Include checkpoints without branch (pre-branch creation)
        !cp.branch_id
    );
  }, [sortedCheckpoints, timeline?.current_branch_id]);

  // Handle create checkpoint
  const handleCreateCheckpoint = useCallback(async () => {
    if (!newCheckpointLabel.trim()) return;

    const checkpoint = await createCheckpoint(newCheckpointLabel.trim(), trackedFiles);
    if (checkpoint) {
      setNewCheckpointLabel('');
      setShowCreateForm(false);
      onCheckpointCreate?.(checkpoint);
    }
  }, [newCheckpointLabel, trackedFiles, createCheckpoint, onCheckpointCreate]);

  // Handle branch switch
  const handleBranchSwitch = useCallback(
    async (branch: CheckpointBranch) => {
      await switchBranch(branch.id);
      setShowBranchSelector(false);
    },
    [switchBranch]
  );

  // Handle fork branch
  const handleForkBranch = useCallback(
    async (checkpoint: Checkpoint) => {
      const branchName = prompt(t('timeline.enterBranchName'));
      if (branchName) {
        await forkBranch(checkpoint.id, branchName);
      }
    },
    [forkBranch, t]
  );

  // Loading state
  if (loading.timeline && !timeline) {
    return <TimelineSkeleton />;
  }

  // Error state
  if (error) {
    return (
      <div className="w-full p-4 text-center">
        <p className="text-red-500 dark:text-red-400">{error}</p>
      </div>
    );
  }

  // Empty state
  if (!timeline || branchCheckpoints.length === 0) {
    return (
      <div className="w-full p-4 text-center">
        <div className="mb-6">
          <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-2">
            {t('timeline.noCheckpoints')}
          </h3>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            {t('timeline.createFirstCheckpoint')}
          </p>
        </div>

        {showCreateForm ? (
          <div className="max-w-sm mx-auto">
            <input
              type="text"
              value={newCheckpointLabel}
              onChange={(e) => setNewCheckpointLabel(e.target.value)}
              placeholder={t('timeline.checkpointLabel')}
              className={clsx(
                'w-full px-3 py-2 rounded-lg mb-2',
                'border border-gray-300 dark:border-gray-600',
                'bg-white dark:bg-gray-800',
                'text-gray-900 dark:text-white',
                'focus:ring-2 focus:ring-primary-500 focus:border-transparent'
              )}
              autoFocus
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleCreateCheckpoint();
                if (e.key === 'Escape') setShowCreateForm(false);
              }}
            />
            <div className="flex gap-2">
              <button
                onClick={handleCreateCheckpoint}
                disabled={loading.checkpoint || !newCheckpointLabel.trim()}
                className={clsx(
                  'flex-1 px-4 py-2 rounded-lg',
                  'bg-primary-600 text-white',
                  'hover:bg-primary-700',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                  'transition-colors'
                )}
              >
                {loading.checkpoint ? t('common.adding') : t('buttons.save')}
              </button>
              <button
                onClick={() => setShowCreateForm(false)}
                className={clsx(
                  'px-4 py-2 rounded-lg',
                  'bg-gray-200 dark:bg-gray-700',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-300 dark:hover:bg-gray-600',
                  'transition-colors'
                )}
              >
                {t('buttons.cancel')}
              </button>
            </div>
          </div>
        ) : (
          <button
            onClick={() => setShowCreateForm(true)}
            className={clsx(
              'inline-flex items-center gap-2 px-4 py-2 rounded-lg',
              'bg-primary-600 text-white',
              'hover:bg-primary-700',
              'transition-colors'
            )}
          >
            <PlusIcon className="w-4 h-4" />
            {t('timeline.createCheckpoint')}
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="w-full h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
          {t('timeline.title')}
        </h2>

        <div className="flex items-center gap-2">
          {/* Branch Selector */}
          {timeline.branches.length > 0 && (
            <div className="relative">
              <button
                onClick={() => setShowBranchSelector(!showBranchSelector)}
                className={clsx(
                  'flex items-center gap-2 px-3 py-1.5 rounded-lg',
                  'bg-gray-100 dark:bg-gray-800',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                  'transition-colors text-sm'
                )}
              >
                <Link2Icon className="w-4 h-4" />
                <span>{currentBranch?.name || t('timeline.noBranch')}</span>
                <ChevronDownIcon className="w-4 h-4" />
              </button>

              {showBranchSelector && (
                <div
                  className={clsx(
                    'absolute right-0 mt-2 w-48 rounded-lg shadow-lg z-10',
                    'bg-white dark:bg-gray-800',
                    'border border-gray-200 dark:border-gray-700'
                  )}
                >
                  {timeline.branches.map((branch) => (
                    <button
                      key={branch.id}
                      onClick={() => handleBranchSwitch(branch)}
                      className={clsx(
                        'w-full px-4 py-2 text-left text-sm',
                        'hover:bg-gray-100 dark:hover:bg-gray-700',
                        'first:rounded-t-lg last:rounded-b-lg',
                        branch.id === currentBranch?.id
                          ? 'text-primary-600 dark:text-primary-400 font-medium'
                          : 'text-gray-700 dark:text-gray-300'
                      )}
                    >
                      <div className="flex items-center gap-2">
                        <Link2Icon className="w-3 h-3" />
                        <span>{branch.name}</span>
                        {branch.is_main && (
                          <span className="text-xs text-gray-400">(main)</span>
                        )}
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Create Checkpoint Button */}
          <button
            onClick={() => setShowCreateForm(true)}
            className={clsx(
              'flex items-center gap-2 px-3 py-1.5 rounded-lg',
              'bg-primary-600 text-white',
              'hover:bg-primary-700',
              'transition-colors text-sm'
            )}
          >
            <PlusIcon className="w-4 h-4" />
            {t('timeline.createCheckpoint')}
          </button>
        </div>
      </div>

      {/* Create Checkpoint Form (inline) */}
      {showCreateForm && (
        <div className="p-4 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900">
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={newCheckpointLabel}
              onChange={(e) => setNewCheckpointLabel(e.target.value)}
              placeholder={t('timeline.checkpointLabel')}
              className={clsx(
                'flex-1 px-3 py-2 rounded-lg',
                'border border-gray-300 dark:border-gray-600',
                'bg-white dark:bg-gray-800',
                'text-gray-900 dark:text-white',
                'focus:ring-2 focus:ring-primary-500 focus:border-transparent'
              )}
              autoFocus
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleCreateCheckpoint();
                if (e.key === 'Escape') setShowCreateForm(false);
              }}
            />
            <button
              onClick={handleCreateCheckpoint}
              disabled={loading.checkpoint || !newCheckpointLabel.trim()}
              className={clsx(
                'px-4 py-2 rounded-lg',
                'bg-primary-600 text-white',
                'hover:bg-primary-700',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors'
              )}
            >
              {loading.checkpoint ? t('common.adding') : t('buttons.save')}
            </button>
            <button
              onClick={() => setShowCreateForm(false)}
              className={clsx(
                'px-4 py-2 rounded-lg',
                'bg-gray-200 dark:bg-gray-700',
                'text-gray-700 dark:text-gray-300',
                'hover:bg-gray-300 dark:hover:bg-gray-600',
                'transition-colors'
              )}
            >
              {t('buttons.cancel')}
            </button>
          </div>
        </div>
      )}

      {/* Timeline Content */}
      <div className="flex-1 overflow-y-auto p-4">
        <div className="relative">
          {/* Timeline line */}
          <div
            className={clsx(
              'absolute left-[7px] top-0 bottom-0 w-0.5',
              'bg-gray-200 dark:bg-gray-700'
            )}
          />

          {/* Checkpoint nodes */}
          <div className="space-y-4">
            {branchCheckpoints.map((checkpoint, index) => (
              <CheckpointNode
                key={checkpoint.id}
                checkpoint={checkpoint}
                isSelected={selectedCheckpoint?.id === checkpoint.id}
                isCurrent={timeline.current_checkpoint_id === checkpoint.id}
                isFirst={index === 0}
                onClick={() => selectCheckpoint(checkpoint)}
                onFork={() => handleForkBranch(checkpoint)}
                onRestore={() => onCheckpointRestore?.(checkpoint.id)}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

export default TimelineView;
