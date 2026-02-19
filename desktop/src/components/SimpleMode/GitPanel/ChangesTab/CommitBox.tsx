/**
 * CommitBox Component
 *
 * Commit message textarea with conventional commit placeholder,
 * commit button, amend checkbox, and AI commit message generation.
 *
 * Feature-002, Story-005 + Feature-005 (AI Commit Message)
 */

import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useGitStore } from '../../../../store/git';
import { useSettingsStore } from '../../../../store/settings';
import { useGitAI } from '../../../../hooks/useGitAI';
import { useToast } from '../../../shared/Toast';

// ---------------------------------------------------------------------------
// Sparkle icon for AI button
// ---------------------------------------------------------------------------

function SparkleIcon({ className }: { className?: string }) {
  return (
    <svg className={className} fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={2}
        d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 002.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.455 2.456L21.75 6l-1.036.259a3.375 3.375 0 00-2.455 2.456zM16.894 20.567L16.5 21.75l-.394-1.183a2.25 2.25 0 00-1.423-1.423L13.5 18.75l1.183-.394a2.25 2.25 0 001.423-1.423l.394-1.183.394 1.183a2.25 2.25 0 001.423 1.423l1.183.394-1.183.394a2.25 2.25 0 00-1.423 1.423z"
      />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Spinner icon
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// CommitBox Component
// ---------------------------------------------------------------------------

export function CommitBox() {
  const { t } = useTranslation('git');
  const commitMessage = useGitStore((s) => s.commitMessage);
  const setCommitMessage = useGitStore((s) => s.setCommitMessage);
  const isAmend = useGitStore((s) => s.isAmend);
  const setIsAmend = useGitStore((s) => s.setIsAmend);
  const commit = useGitStore((s) => s.commit);
  const status = useGitStore((s) => s.status);
  const isLoading = useGitStore((s) => s.isLoading);
  const workspacePath = useSettingsStore((s) => s.workspacePath);

  const { isAvailable, isGeneratingCommit, generateCommitMessage, unavailableReason } = useGitAI();
  const { showToast } = useToast();

  const stagedCount = status?.staged.length ?? 0;
  const canCommit = stagedCount > 0 && commitMessage.trim().length > 0 && !isLoading;
  const canGenerate = isAvailable && stagedCount > 0 && !isGeneratingCommit && !isLoading;

  const handleCommit = useCallback(async () => {
    if (!canCommit) return;
    await commit(commitMessage, isAmend);
  }, [canCommit, commit, commitMessage, isAmend]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Cmd/Ctrl + Enter to commit
      if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
        e.preventDefault();
        handleCommit();
      }
    },
    [handleCommit]
  );

  const handleGenerateMessage = useCallback(async () => {
    if (!canGenerate || !workspacePath) return;
    const message = await generateCommitMessage(workspacePath);
    if (message) {
      setCommitMessage(message);
      showToast(t('commitBox.aiMessageGenerated'), 'success');
    } else {
      showToast(t('commitBox.aiMessageFailed'), 'error');
    }
  }, [canGenerate, workspacePath, generateCommitMessage, setCommitMessage, showToast]);

  return (
    <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700">
      {/* Message input */}
      <textarea
        value={commitMessage}
        onChange={(e) => setCommitMessage(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={t('commitBox.placeholder')}
        rows={3}
        className={clsx(
          'w-full text-xs px-2.5 py-2 rounded-md border resize-y min-h-[60px] max-h-[200px]',
          'bg-white dark:bg-gray-900',
          'border-gray-300 dark:border-gray-600',
          'text-gray-800 dark:text-gray-200',
          'placeholder-gray-400 dark:placeholder-gray-500',
          'focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500',
          'transition-colors'
        )}
      />

      {/* Controls row */}
      <div className="flex items-center justify-between mt-2">
        <div className="flex items-center gap-3">
          {/* Amend checkbox */}
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input
              type="checkbox"
              checked={isAmend}
              onChange={(e) => setIsAmend(e.target.checked)}
              className="w-3 h-3 rounded border-gray-300 dark:border-gray-600 text-primary-600 focus:ring-primary-500"
            />
            <span className="text-2xs text-gray-500 dark:text-gray-400">{t('commitBox.amend')}</span>
          </label>

          {/* AI Generate button */}
          <button
            onClick={handleGenerateMessage}
            disabled={!canGenerate}
            className={clsx(
              'text-2xs px-2 py-1 rounded border transition-colors',
              'flex items-center gap-1',
              canGenerate
                ? 'text-purple-600 dark:text-purple-400 border-purple-300 dark:border-purple-600 hover:bg-purple-50 dark:hover:bg-purple-900/20'
                : 'text-gray-400 dark:text-gray-500 border-dashed border-gray-300 dark:border-gray-600 opacity-50 cursor-not-allowed'
            )}
            title={
              !isAvailable
                ? unavailableReason
                : stagedCount === 0
                  ? t('commitBox.stageChangesFirst')
                  : isGeneratingCommit
                    ? t('commitBox.generating')
                    : t('commitBox.generateAIMessage')
            }
          >
            {isGeneratingCommit ? (
              <Spinner className="w-3 h-3" />
            ) : (
              <SparkleIcon className="w-3 h-3" />
            )}
            {isGeneratingCommit ? t('commitBox.generating') : t('commitBox.ai')}
          </button>
        </div>

        {/* Commit button */}
        <button
          onClick={handleCommit}
          disabled={!canCommit}
          className={clsx(
            'text-xs px-3 py-1.5 rounded-md font-medium transition-colors',
            canCommit
              ? 'bg-primary-600 hover:bg-primary-700 text-white'
              : 'bg-gray-200 dark:bg-gray-700 text-gray-400 dark:text-gray-500 cursor-not-allowed'
          )}
        >
          {isAmend ? t('commitBox.amend') : t('commitBox.commit')}
          {stagedCount > 0 && (
            <span className="ml-1 opacity-75">({stagedCount})</span>
          )}
        </button>
      </div>
    </div>
  );
}

export default CommitBox;
