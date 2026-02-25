/**
 * StagingArea Component
 *
 * Three collapsible sections: Staged Changes, Unstaged Changes, Untracked Files.
 * Each section header shows a file count and Stage All / Unstage All button.
 * Uses FileEntry component for each file.
 * Staged section includes an AI Review button (feature-005).
 *
 * Feature-002, Story-004 + Feature-005 (AI Code Review)
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useGitStore } from '../../../../store/git';
import { useSettingsStore } from '../../../../store/settings';
import { FileEntry } from './FileEntry';
import { AIReviewPanel } from './AIReviewPanel';
import { useGitAI } from '../../../../hooks/useGitAI';
import { useToast } from '../../../shared/Toast';

// ============================================================================
// Section Header
// ============================================================================

interface SectionProps {
  title: string;
  count: number;
  defaultOpen?: boolean;
  actionLabel?: string;
  onAction?: () => void;
  extraActions?: React.ReactNode;
  children: React.ReactNode;
}

function CollapsibleSection({
  title,
  count,
  defaultOpen = true,
  actionLabel,
  onAction,
  extraActions,
  children,
}: SectionProps) {
  const { t } = useTranslation('git');
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className="border-b border-gray-200 dark:border-gray-700 last:border-b-0">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-1.5 bg-gray-50 dark:bg-gray-800/50">
        <button
          onClick={() => setOpen((v) => !v)}
          className="flex items-center gap-1.5 text-xs font-medium text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-gray-100 transition-colors"
        >
          <svg
            className={clsx('w-3 h-3 transition-transform', open && 'rotate-90')}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
          </svg>
          <span>{title}</span>
          <span className="text-2xs text-gray-500 dark:text-gray-400 ml-1">({count})</span>
        </button>

        <div className="flex items-center gap-2">
          {extraActions}
          {actionLabel && onAction && count > 0 && (
            <button
              onClick={onAction}
              className="text-2xs px-2 py-0.5 rounded text-primary-600 dark:text-primary-400 hover:bg-primary-50 dark:hover:bg-primary-900/20 transition-colors"
            >
              {actionLabel}
            </button>
          )}
        </div>
      </div>

      {/* Content */}
      {open && count > 0 && <div>{children}</div>}
      {open && count === 0 && (
        <div className="px-3 py-3 text-center text-2xs text-gray-400 dark:text-gray-500">
          {t('stagingArea.noFiles')}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Spinner
// ============================================================================

function Spinner({ className }: { className?: string }) {
  return (
    <svg className={clsx('animate-spin', className)} fill="none" viewBox="0 0 24 24">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );
}

// ============================================================================
// StagingArea Component
// ============================================================================

export function StagingArea() {
  const { t } = useTranslation('git');
  const status = useGitStore((s) => s.status);
  const stageFiles = useGitStore((s) => s.stageFiles);
  const unstageFiles = useGitStore((s) => s.unstageFiles);
  const workspacePath = useSettingsStore((s) => s.workspacePath);

  const { isAvailable, isReviewing, reviewDiff, unavailableReason } = useGitAI();
  const { showToast } = useToast();

  const [reviewText, setReviewText] = useState<string | null>(null);

  const staged = status?.staged ?? [];
  const unstaged = status?.unstaged ?? [];
  const untracked = status?.untracked ?? [];

  const handleUnstageAll = useCallback(() => {
    if (staged.length > 0) {
      unstageFiles(staged.map((f) => f.path));
    }
  }, [staged, unstageFiles]);

  const handleStageAllUnstaged = useCallback(() => {
    if (unstaged.length > 0) {
      stageFiles(unstaged.map((f) => f.path));
    }
  }, [unstaged, stageFiles]);

  const handleStageAllUntracked = useCallback(() => {
    if (untracked.length > 0) {
      stageFiles(untracked.map((f) => f.path));
    }
  }, [untracked, stageFiles]);

  const handleAIReview = useCallback(async () => {
    if (!workspacePath || !isAvailable) return;
    const result = await reviewDiff(workspacePath);
    if (result.data) {
      setReviewText(result.data);
      showToast(t('stagingArea.aiReviewComplete'), 'success');
    } else {
      showToast(result.error || t('stagingArea.aiReviewFailed'), 'error');
    }
  }, [workspacePath, isAvailable, reviewDiff, showToast]);

  const handleDismissReview = useCallback(() => {
    setReviewText(null);
  }, []);

  if (!status) {
    return (
      <div className="flex items-center justify-center py-8 text-sm text-gray-500 dark:text-gray-400">
        <div className="animate-spin h-4 w-4 border-2 border-gray-400 border-t-transparent rounded-full mr-2" />
        {t('stagingArea.loadingStatus')}
      </div>
    );
  }

  const totalChanges = staged.length + unstaged.length + untracked.length;
  if (totalChanges === 0 && status.conflicted.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-8 text-sm text-gray-500 dark:text-gray-400">
        <svg className="w-8 h-8 mb-2 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M5 13l4 4L19 7" />
        </svg>
        <p>{t('stagingArea.workingTreeClean')}</p>
        <p className="text-2xs text-gray-400 dark:text-gray-500 mt-1">
          {status.branch && t('stagingArea.onBranch', { branch: status.branch })}
          {status.upstream && ` ${t('stagingArea.tracking', { upstream: status.upstream })}`}
        </p>
      </div>
    );
  }

  const canReview = isAvailable && staged.length > 0 && !isReviewing;

  // AI Review button for staged section header
  const stagedExtraActions = (
    <button
      onClick={handleAIReview}
      disabled={!canReview}
      className={clsx(
        'flex items-center gap-1 text-2xs px-2 py-0.5 rounded transition-colors',
        canReview
          ? 'text-purple-600 dark:text-purple-400 hover:bg-purple-50 dark:hover:bg-purple-900/20'
          : 'text-gray-400 dark:text-gray-500 cursor-not-allowed opacity-50',
      )}
      title={
        !isAvailable
          ? unavailableReason
          : staged.length === 0
            ? t('stagingArea.stageChangesFirstReview')
            : isReviewing
              ? t('stagingArea.reviewing')
              : t('stagingArea.aiReviewStaged')
      }
    >
      {isReviewing ? (
        <Spinner className="w-3 h-3" />
      ) : (
        <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09z"
          />
        </svg>
      )}
      {isReviewing ? t('stagingArea.reviewing') : t('stagingArea.aiReview')}
    </button>
  );

  return (
    <div>
      {/* Staged Changes */}
      <CollapsibleSection
        title={t('stagingArea.stagedChanges')}
        count={staged.length}
        defaultOpen={true}
        actionLabel={t('stagingArea.unstageAll')}
        onAction={handleUnstageAll}
        extraActions={staged.length > 0 ? stagedExtraActions : undefined}
      >
        {staged.map((file) => (
          <FileEntry key={`staged-${file.path}`} file={file} isStaged={true} />
        ))}
      </CollapsibleSection>

      {/* AI Review results (shown below staged section) */}
      {reviewText && <AIReviewPanel reviewText={reviewText} onDismiss={handleDismissReview} />}

      {/* Unstaged Changes */}
      <CollapsibleSection
        title={t('stagingArea.changes')}
        count={unstaged.length}
        defaultOpen={true}
        actionLabel={t('stagingArea.stageAll')}
        onAction={handleStageAllUnstaged}
      >
        {unstaged.map((file) => (
          <FileEntry key={`unstaged-${file.path}`} file={file} isStaged={false} />
        ))}
      </CollapsibleSection>

      {/* Untracked Files */}
      <CollapsibleSection
        title={t('stagingArea.untracked')}
        count={untracked.length}
        defaultOpen={true}
        actionLabel={t('stagingArea.stageAll')}
        onAction={handleStageAllUntracked}
      >
        {untracked.map((file) => (
          <FileEntry key={`untracked-${file.path}`} file={file} isStaged={false} isUntracked={true} />
        ))}
      </CollapsibleSection>
    </div>
  );
}

export default StagingArea;
